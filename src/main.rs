//! SplatCrab: a small MATLAB-compatible interpreter.
//!
//!   splatcrab              start the REPL
//!   splatcrab script.m     run a script file
//!   splatcrab --protocol   serve the evaluation protocol on stdin/stdout:
//!                          one JSON request per line, one JSON response
//!                          per line, one session (docs/modules/U0-ui-foundations.md)

use std::io::{self, BufRead, Write};

use splatcrab::{interp, protocol, syntax};

/// The interpreter recurses through the precedence chain once per nesting
/// level, in the parser and again in the evaluator, so a deeply nested
/// expression needs far more stack than Windows gives the main thread (1 MB).
/// Everything therefore runs on a thread we size ourselves.
const STACK: usize = 256 * 1024 * 1024;

fn main() {
    let code = match std::thread::Builder::new().stack_size(STACK).spawn(run) {
        Ok(h) => h.join().unwrap_or(101),
        // If the thread cannot be spawned, run on the main one rather than
        // refusing to start at all.
        Err(_) => run(),
    };
    std::process::exit(code);
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    /// Sets the code page the console decodes output bytes with. It is a
    /// raw declaration rather than a crate, because the project has none.
    fn SetConsoleOutputCP(code_page: u32) -> i32;
}

/// Makes a Windows console read what this program writes as UTF-8, so the
/// `×` of a `2×3 char array` header and any non-ASCII char text render
/// rather than arriving as code-page mojibake. Everything written is UTF-8
/// already; only the console's reading of it changes. When the output is a
/// pipe or a file there is no console, the call fails, and that is harmless,
/// so its result is ignored. Elsewhere terminals are UTF-8 by default.
fn console_utf8() {
    #[cfg(windows)]
    // SAFETY: a plain Win32 call with an integer argument and no pointers.
    unsafe {
        SetConsoleOutputCP(65001);
    }
}

/// Runs the CLI and returns the process exit code.
fn run() -> i32 {
    console_utf8();
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).is_some_and(|a| a == "--protocol") {
        // Exits 0 at end of input whatever the requests did, and writes
        // nothing to stderr: every failure a request can meet is an answer
        // on stdout. Only the transport failing, a read or a write to the
        // pipe itself, exits 1, and silently, since there is nobody left to
        // tell on the stream that broke.
        return match protocol::serve(io::stdin().lock(), io::stdout().lock()) {
            Ok(()) => 0,
            Err(_) => 1,
        };
    }

    let mut it = interp::Interp::new();

    if args.len() > 1 {
        let path = &args[1];
        // Read bytes and decode leniently rather than demanding valid UTF-8.
        // A Windows editor writes a `% café` comment in Windows-1252, and
        // `read_to_string` refused the whole file over it with a message that
        // was not even in the `Error:` format (QA D29). MATLAB and Octave
        // read such a file, so SplatCrab does too: an undecodable byte
        // becomes U+FFFD, which is harmless inside a comment or a string and
        // is an ordinary `unexpected character` error anywhere else. A
        // leading UTF-8 byte-order mark is skipped by the lexer.
        let src = match std::fs::read(path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                eprintln!("Error: Cannot read {}: {}", path, e);
                return 1;
            }
        };
        let result = it.run(&src);
        // Flush what the script printed before anything else: `main` no
        // longer returns normally, so nothing else will.
        let _ = it.out.flush();
        if let Err(e) = result {
            // `MError`'s Display supplies the `Line N: ` part when a line is
            // known, so the prefix is spelled in exactly one place.
            eprintln!("Error: {}", e);
            return 1;
        }
        return 0;
    }

    println!("SplatCrab 0.1.0  (type 'exit' to quit)\n");
    let stdin = io::stdin();
    let mut buf = String::new();
    loop {
        print!("{}", if buf.is_empty() { ">> " } else { "   " });
        io::stdout().flush().ok();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if buf.is_empty() && (trimmed == "exit" || trimmed == "quit") {
            break;
        }
        buf.push_str(&line);
        if !syntax::is_complete(&buf) {
            continue;
        }
        if let Err(e) = it.run(&buf) {
            report(&mut it, &e);
        }
        buf.clear();
    }
    // The input ran out inside an unfinished block. Piping `for k = 1:3` and
    // `disp(k)` with no `end` used to discard the buffer in silence and exit
    // 0 (QA D36). Nothing in the buffer is run: `is_complete` said an opener
    // was still waiting, so the statements inside it were never complete.
    let mut code = 0;
    if !buf.trim().is_empty() {
        report(&mut it, &splatcrab::error::unterminated_block());
        code = 1;
    }
    let _ = it.out.flush();
    code
}

/// Prints a REPL diagnostic.
///
/// It goes to stderr, as script mode's already does, so that a piped session
/// can separate diagnostics from output; it used to go to stdout, where
/// nothing downstream could tell the two apart. Whatever the entry printed
/// before failing is flushed first, so the two streams stay in order when
/// both land on the same terminal.
///
/// No line number: a REPL entry is one line, so `Line 1:` would be noise
/// rather than information.
fn report(it: &mut interp::Interp, e: &splatcrab::error::MError) {
    let _ = it.out.flush();
    io::stdout().flush().ok();
    eprintln!("Error: {}", e.msg);
}
