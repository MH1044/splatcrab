//! Golden-file runner.
//!
//! Every `tests/cases/<module>/<name>.m` that has a sibling `<name>.out` is a
//! case. An `.m` file with no `.out` is a helper (a function or script file the
//! case calls by name). Each case runs with the built `splatcrab` binary, with
//! the working directory set to the case's own directory so helpers resolve.
//!
//! A `<name>.repl` file is a case too, and the one thing a `.m` cannot test:
//! it is spawned with no script argument, so the binary enters the REPL, and
//! the file is typed at the prompt through stdin. Everything else -- `.out`,
//! `.err`, the exit-code rule -- is identical.
//!
//! A `<name>.proto` file is a case in the same way, for the evaluation
//! protocol of cycle U0: the binary is spawned with `--protocol`, and the file
//! less its `% covers:` line is typed on stdin, one JSON request per line.
//! The `.out` holds one JSON response per line. A `.proto` case with no `.err`
//! also asserts that stderr is empty, since the protocol writes nothing there.
//!
//! Every case asserts an exact exit code. `.err` holds a substring that must
//! appear on stderr; `.exit` holds the expected code when it is not the one
//! the other files imply (1 with an `.err`, 0 without).
//!
//!   cargo test --test golden
//!   GOLDEN_FILTER=00-baseline cargo test --test golden    # path substring
//!   UPDATE_GOLDEN=1 cargo test --test golden              # rewrite .out
//!
//! A `.out` file is the specification of intended behaviour, not a cache.
//! Never regenerate one to turn a red test green without knowing why the
//! output changed; read `git diff tests/cases` line by line afterwards.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_DIFF_LINES: usize = 200;

fn cases_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("cases")
}

/// True when a `.m` file declares itself a case by opening with the
/// `% covers:` marker. This is what lets a brand new case, which has no `.out`
/// yet, still be discovered so `UPDATE_GOLDEN=1` can create one. A helper
/// function or script file carries no marker and is never run as a case.
fn declares_itself_a_case(p: &Path) -> bool {
    match fs::read_to_string(p) {
        Ok(s) => s
            .lines()
            .next()
            .is_some_and(|l| l.trim_start().starts_with("% covers:")),
        Err(_) => false,
    }
}

/// True when this case drives the REPL rather than running a script: the
/// binary is spawned with no argument and the file is typed at the prompt.
fn is_repl_case(p: &Path) -> bool {
    p.extension().is_some_and(|x| x == "repl")
}

/// True when this case drives the evaluation protocol: the binary is spawned
/// with `--protocol` and the file is its stdin, one request per line.
fn is_proto_case(p: &Path) -> bool {
    p.extension().is_some_and(|x| x == "proto")
}

/// The lines a `.repl` or `.proto` case types on stdin: the whole file except
/// its `% covers:` marker, which documents the case rather than being typed.
fn session_input(path: &Path) -> Vec<u8> {
    let text = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    };
    match text.split_once('\n') {
        Some((first, rest)) if first.trim_start().starts_with("% covers:") => rest.into(),
        _ => text.into_bytes(),
    }
}

/// Collects case files, in a deterministic order. A `.m`, `.repl` or `.proto`
/// file is a case when it has a sibling `.out`, or when it opens with
/// `% covers:`.
fn collect_cases(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(e) => panic!("cannot read {}: {e}", dir.display()),
    };
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_cases(&p, out);
        } else if p
            .extension()
            .is_some_and(|x| x == "m" || x == "repl" || x == "proto")
            && (p.with_extension("out").exists() || declares_itself_a_case(&p))
        {
            out.push(p);
        }
    }
}

/// CRLF to LF, trailing whitespace stripped per line, trailing blank lines
/// dropped. Interior blank lines and leading whitespace are compared exactly:
/// MATLAB's column alignment is part of the specification.
fn normalize(s: &str) -> String {
    let mut lines: Vec<&str> = s.lines().map(str::trim_end).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

struct Outcome {
    stdout: String,
    stderr: String,
    code: Option<i32>,
    timed_out: bool,
}

/// Spawns the binary. stdout and stderr are drained on threads so a large
/// output cannot fill the pipe and deadlock while we poll for exit.
fn run_case(path: &Path) -> Outcome {
    let dir = path.parent().expect("case has a parent directory");
    let stdin_data = if is_repl_case(path) || is_proto_case(path) {
        Some(session_input(path))
    } else {
        fs::read(path.with_extension("stdin")).ok()
    };

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_splatcrab"));
    // A REPL case takes no script argument: that argument is what makes the
    // binary run a file instead of reading the prompt. A protocol case takes
    // the flag that selects the protocol instead of the file.
    if is_proto_case(path) {
        cmd.arg("--protocol");
    } else if !is_repl_case(path) {
        cmd.arg(path);
    }
    cmd.current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin_data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = cmd.spawn().expect("failed to spawn splatcrab");

    if let Some(data) = stdin_data {
        let mut si = child.stdin.take().expect("stdin was piped");
        std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
    }
    let mut so = child.stdout.take().expect("stdout was piped");
    let mut se = child.stderr.take().expect("stderr was piped");
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        so.read_to_end(&mut b).ok();
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        se.read_to_end(&mut b).ok();
        b
    });

    let start = Instant::now();
    let (status, timed_out) = loop {
        if let Some(s) = child.try_wait().expect("try_wait failed") {
            break (Some(s), false);
        }
        if start.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
        std::thread::sleep(Duration::from_millis(5));
    };

    Outcome {
        stdout: String::from_utf8_lossy(&t_out.join().expect("stdout thread")).into_owned(),
        stderr: String::from_utf8_lossy(&t_err.join().expect("stderr thread")).into_owned(),
        code: status.and_then(|s| s.code()),
        timed_out,
    }
}

/// Line-level LCS diff rendered as `- expected` / `+ actual`.
fn diff(expected: &str, actual: &str) -> String {
    let a: Vec<&str> = expected.lines().collect();
    let b: Vec<&str> = actual.lines().collect();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut out = String::new();
    let (mut i, mut j, mut shown) = (0, 0, 0);
    while i < n || j < m {
        if shown == MAX_DIFF_LINES {
            out.push_str("  ... (diff truncated)\n");
            break;
        }
        shown += 1;
        if i < n && j < m && a[i] == b[j] {
            out.push_str(&format!("  {}\n", a[i]));
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
            out.push_str(&format!("- {}\n", a[i]));
            i += 1;
        } else {
            out.push_str(&format!("+ {}\n", b[j]));
            j += 1;
        }
    }
    out
}

/// The exit code a case must produce.
///
/// It is asserted in every case and never left free, because the exit code is
/// the harness's main tripwire: a panic is 101 and a stack overflow or an
/// allocator abort is 134, so a case that checked only the message text would
/// pass on the very abort it was written to catch.
///
/// The default is what the other files imply: 1 when an `.err` names a
/// diagnostic, 0 when there is none. An `.exit` file states the code outright
/// where that default is wrong. The case it exists for is a REPL session that
/// reports an error on stderr and then carries on to a clean `exit`: the
/// diagnostic is the point of the case, and the exit code of 0 is half of it,
/// since it is what says the session survived.
fn expected_code(exit_path: &Path, has_err: bool) -> Result<i32, String> {
    match fs::read_to_string(exit_path) {
        Ok(s) => s.trim().parse().map_err(|_| {
            format!(
                "{} must hold a decimal exit code, not {:?}",
                exit_path.display(),
                s.trim()
            )
        }),
        Err(_) => Ok(if has_err { 1 } else { 0 }),
    }
}

fn check_case(m: &Path, update: bool) -> Result<(), String> {
    let out_path = m.with_extension("out");
    let err_path = m.with_extension("err");
    let exit_path = m.with_extension("exit");
    let r = run_case(m);
    if r.timed_out {
        return Err(format!("timed out after {TIMEOUT:?} (infinite loop?)"));
    }
    let actual = normalize(&r.stdout);
    let expect_err = fs::read_to_string(&err_path)
        .ok()
        .map(|s| s.trim().to_string());
    let expect_code = expected_code(&exit_path, expect_err.is_some())?;

    if update {
        let text = if actual.is_empty() {
            String::new()
        } else {
            format!("{actual}\n")
        };
        fs::write(&out_path, text)
            .map_err(|e| format!("cannot write {}: {e}", out_path.display()))?;
    }
    let expected = match fs::read_to_string(&out_path) {
        Ok(s) => normalize(&s),
        Err(_) => {
            return Err(format!(
                "missing {} (create it, or run with UPDATE_GOLDEN=1 and review the diff)",
                out_path.display()
            ));
        }
    };

    let mut problems = Vec::new();
    if r.code != Some(expect_code) {
        problems.push(format!(
            "expected exit code {expect_code} but the process exited {:?}; stderr was:\n{}",
            r.code,
            r.stderr.trim_end()
        ));
    }
    if let Some(sub) = &expect_err {
        if !r.stderr.contains(sub.as_str()) {
            problems.push(format!(
                "stderr did not contain {sub:?}; stderr was:\n{}",
                r.stderr.trim_end()
            ));
        }
    } else if is_proto_case(m) && !r.stderr.is_empty() {
        // The protocol answers every failure on stdout and writes nothing
        // at all to stderr (the U0 spec), so a protocol case with no `.err`
        // asserts silence there, not merely "whatever stderr held".
        problems.push(format!(
            "a protocol case must write nothing to stderr; stderr was:\n{}",
            r.stderr.trim_end()
        ));
    }
    if expected != actual {
        problems.push(format!(
            "stdout differs (- expected, + actual):\n{}",
            diff(&expected, &actual)
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[test]
fn golden_cases() {
    let root = cases_root();
    let mut cases = Vec::new();
    collect_cases(&root, &mut cases);
    let filter = std::env::var("GOLDEN_FILTER").ok();
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1");

    let mut ran = 0;
    let mut failures = Vec::new();
    for m in &cases {
        let rel = m
            .strip_prefix(&root)
            .unwrap_or(m)
            .to_string_lossy()
            .replace('\\', "/");
        if filter.as_deref().is_some_and(|f| !rel.contains(f)) {
            continue;
        }
        ran += 1;
        match check_case(m, update) {
            Ok(()) => println!("ok   {rel}"),
            Err(msg) => {
                println!("FAIL {rel}");
                failures.push(format!("--- {rel} ---\n{msg}"));
            }
        }
    }

    assert!(ran > 0, "no golden cases found under {}", root.display());
    if update {
        println!(
            "UPDATE_GOLDEN=1: rewrote .out for {ran} cases -- review `git diff tests/cases` before committing"
        );
    }
    if !failures.is_empty() {
        panic!(
            "\n{} of {ran} golden cases failed:\n\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
