//! Handbook runner: every example in `docs/HANDBOOK.md` must print what the
//! handbook says it prints.
//!
//! An example is a ```` ```matlab ```` block followed by the next plain
//! ```` ``` ```` block, with prose allowed in between. A heading or a block in
//! another language before any plain block means the example has no output
//! block and is not checked. An output block ending in `...` is elided on
//! purpose and is skipped. The script runs through the built binary and its
//! stdout followed by its stderr is compared with the output block, normalised
//! as the golden harness normalises: CRLF to LF, trailing whitespace stripped
//! per line, trailing blank lines dropped.
//!
//!   cargo test --test handbook
//!
//! This replaced `tools/verify_handbook.py`, so the handbook is checked on
//! every CI run rather than when somebody remembers. A block that no longer
//! matches is fixed by rerunning its script and reading the diff: if the
//! change was deliberate, paste the binary's real output and reread the prose
//! around it; if it was not, the code is wrong.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(20);
/// Mismatches printed in full; the rest are counted.
const MAX_REPORTED: usize = 40;

#[derive(Debug, PartialEq)]
struct Example {
    script: String,
    expected: String,
    /// One-based line of the script's first line in the handbook.
    line: usize,
}

/// Pairs each `matlab` block with the plain block that follows it.
fn extract(text: &str) -> Vec<Example> {
    let lines: Vec<&str> = text.split('\n').collect();
    let fence_end = |from: usize| {
        let mut j = from;
        while j < lines.len() && lines[j].trim() != "```" {
            j += 1;
        }
        j
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() != "```matlab" {
            i += 1;
            continue;
        }
        let start = i + 1;
        let end = fence_end(start);
        let script = lines[start..end].join("\n");
        let mut k = end + 1;
        while k < lines.len() {
            let t = lines[k].trim();
            if t == "```" {
                let close = fence_end(k + 1);
                out.push(Example {
                    script,
                    expected: lines[(k + 1).min(lines.len())..close].join("\n"),
                    line: start + 1,
                });
                break;
            }
            if t.starts_with("```") || t.starts_with('#') {
                break;
            }
            k += 1;
        }
        i = end + 1;
    }
    out
}

/// An output block the handbook deliberately cuts short.
fn is_elided(expected: &str) -> bool {
    expected.trim_end().ends_with("...") && !expected.contains('…')
}

fn normalize(s: &str) -> String {
    let mut lines: Vec<&str> = s.lines().map(str::trim_end).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Runs one script, returning stdout then stderr, or `None` on a timeout.
/// Both pipes are drained on threads so a large output cannot deadlock.
fn run(script: &Path, dir: &Path) -> Option<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splatcrab"))
        .arg(script)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn splatcrab");
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
    let finished = loop {
        if child.try_wait().expect("try_wait failed").is_some() {
            break true;
        }
        if start.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut text = String::from_utf8_lossy(&t_out.join().expect("stdout thread")).into_owned();
    text.push_str(&String::from_utf8_lossy(
        &t_err.join().expect("stderr thread"),
    ));
    finished.then_some(text)
}

fn clip(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[test]
fn handbook_examples_print_what_the_handbook_says() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("docs").join("HANDBOOK.md"))
        .expect("docs/HANDBOOK.md is readable");
    let examples = extract(&text);
    // The handbook has held over ninety examples since it was written. Far
    // fewer means the extraction broke, and a broken extraction that finds
    // nothing would otherwise pass with zero mismatches.
    assert!(
        examples.len() >= 90,
        "found only {} handbook examples; the extraction is probably broken",
        examples.len()
    );

    let work: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join("handbook");
    fs::create_dir_all(&work).expect("create the handbook work directory");

    let mut report = String::new();
    let (mut checked, mut skipped, mut failed) = (0, 0, 0);
    for (n, ex) in examples.iter().enumerate() {
        if is_elided(&ex.expected) {
            skipped += 1;
            continue;
        }
        checked += 1;
        let path = work.join(format!("example{n:03}.m"));
        fs::write(&path, format!("{}\n", ex.script)).expect("write the example");
        let actual = match run(&path, &work) {
            Some(out) => normalize(&out),
            None => "<<TIMED OUT>>".to_string(),
        };
        let expected = normalize(&ex.expected);
        if actual != expected {
            failed += 1;
            if failed <= MAX_REPORTED {
                report.push_str(&format!(
                    "\n==== docs/HANDBOOK.md line {}\n--- script ---\n{}\n--- handbook ---\n{}\n--- binary ---\n{}\n",
                    ex.line,
                    clip(&ex.script, 400),
                    clip(&expected, 600),
                    clip(&actual, 600)
                ));
            }
        }
    }
    if failed > MAX_REPORTED {
        report.push_str(&format!("\n... and {} more\n", failed - MAX_REPORTED));
    }
    assert!(
        failed == 0,
        "{failed} of {checked} handbook examples do not match ({skipped} elided and skipped):\n{report}"
    );
}

#[cfg(test)]
mod extraction {
    use super::*;

    #[test]
    fn a_script_pairs_with_the_next_plain_block_across_prose() {
        let md = "# H\n\n```matlab\nx = 1\n```\n\nSome prose.\n\n```\nx =\n\n     1\n```\n";
        let ex = extract(md);
        assert_eq!(
            ex,
            vec![Example {
                script: "x = 1".into(),
                expected: "x =\n\n     1".into(),
                line: 4,
            }]
        );
    }

    #[test]
    fn a_heading_or_another_language_first_means_no_output_block() {
        let md = "```matlab\na = 1;\n```\n## Next\n```\nnot a\n```\n```matlab\nb = 2;\n```\n```text\nx\n```\n```\nnot b\n```\n";
        assert!(extract(md).is_empty());
    }

    #[test]
    fn an_output_block_ending_in_three_dots_is_elided() {
        assert!(is_elided("  Columns 1 through 8\n..."));
        assert!(!is_elided("     1     2"));
        // An ellipsis character in the text means the dots are content.
        assert!(!is_elided("see … below..."));
    }

    #[test]
    fn normalisation_matches_the_golden_harness() {
        assert_eq!(normalize("a  \r\nb\n\n\n"), "a\nb");
        assert_eq!(normalize("  x\n\n y"), "  x\n\n y");
    }
}
