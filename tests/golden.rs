//! Golden-file runner.
//!
//! Every `tests/cases/<module>/<name>.m` that has a sibling `<name>.out` is a
//! case. An `.m` file with no `.out` is a helper (a function or script file the
//! case calls by name). Each case runs with the built `splatcrab` binary, with
//! the working directory set to the case's own directory so helpers resolve.
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

/// Collects `.m` files that have a sibling `.out`, in a deterministic order.
fn collect_cases(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(e) => panic!("cannot read {}: {e}", dir.display()),
    };
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_cases(&p, out);
        } else if p.extension().is_some_and(|x| x == "m") && p.with_extension("out").exists() {
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
    let stdin_path = path.with_extension("stdin");
    let stdin_data = fs::read(&stdin_path).ok();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_splatcrab"));
    cmd.arg(path)
        .current_dir(dir)
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

fn check_case(m: &Path, update: bool) -> Result<(), String> {
    let out_path = m.with_extension("out");
    let err_path = m.with_extension("err");
    let r = run_case(m);
    if r.timed_out {
        return Err(format!("timed out after {TIMEOUT:?} (infinite loop?)"));
    }
    let actual = normalize(&r.stdout);
    let expect_err = fs::read_to_string(&err_path)
        .ok()
        .map(|s| s.trim().to_string());

    if update {
        let text = if actual.is_empty() {
            String::new()
        } else {
            format!("{actual}\n")
        };
        fs::write(&out_path, text).map_err(|e| format!("cannot write {}: {e}", out_path.display()))?;
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
    match (&expect_err, r.code) {
        (None, Some(0)) => {}
        (None, code) => problems.push(format!(
            "exit code {code:?} but there is no .err file; stderr was:\n{}",
            r.stderr.trim_end()
        )),
        (Some(sub), Some(1)) if r.stderr.contains(sub.as_str()) => {}
        (Some(sub), Some(1)) => problems.push(format!(
            "stderr did not contain {sub:?}; stderr was:\n{}",
            r.stderr.trim_end()
        )),
        (Some(sub), code) => problems.push(format!(
            "expected an error containing {sub:?} but the exit code was {code:?}"
        )),
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
