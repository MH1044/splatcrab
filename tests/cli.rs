//! The command-line flags no golden case can reach, since every case runs
//! the binary with fixed flags (cycle 13): `--version` and `--help`, each
//! exiting 0 with nothing on stderr.
//!
//!   cargo test --test cli

use std::process::{Command, Output, Stdio};

fn splatcrab(flag: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_splatcrab"))
        .arg(flag)
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs")
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout is UTF-8")
}

#[test]
fn version_prints_the_version_from_cargo_toml() {
    let out = splatcrab("--version");
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out).replace("\r\n", "\n"),
        format!("SplatCrab {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn help_names_the_modes_and_the_script_argument() {
    let out = splatcrab("--help");
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    for word in [
        "Usage:",
        "--protocol",
        "--ui",
        "--http-stdio",
        "<script.m>",
        "--version",
    ] {
        assert!(text.contains(word), "--help does not name {word}:\n{text}");
    }
    assert!(out.stderr.is_empty());
}

/// The banner reads its version from the same place, and a piped REPL
/// reads plain lines, so `exit(n)` there ends it with `n`.
#[test]
fn the_piped_repl_shows_the_version_and_exits_with_the_code_asked() {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_splatcrab"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(b"disp(1)\nexit(7)\ndisp(2)\n")
        .expect("the REPL reads its input");
    let out = child.wait_with_output().expect("the REPL ends");
    assert_eq!(out.status.code(), Some(7));
    let text = stdout(&out).replace("\r\n", "\n");
    assert_eq!(
        text,
        format!(
            "SplatCrab {}  (type 'exit' to quit)\n\n>>      1\n>> ",
            env!("CARGO_PKG_VERSION")
        )
    );
}
