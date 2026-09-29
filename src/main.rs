//! SplatCrab: a small MATLAB-compatible interpreter.
//!
//!   splatcrab              start the REPL, with the line editor at a
//!                          terminal (cycle 13, `term.rs`)
//!   splatcrab script.m     run a script file
//!   splatcrab --help       print the usage and exit 0
//!   splatcrab --version    print `SplatCrab <version>` and exit 0
//!   splatcrab --protocol   serve the evaluation protocol on stdin/stdout:
//!                          one JSON request per line, one JSON response
//!                          per line, one session (docs/modules/U0-ui-foundations.md)
//!   splatcrab --ui [--port N] [--no-browser] [--token T]
//!                          serve the command window on 127.0.0.1 only, on
//!                          port N or one the system picks, print its URL
//!                          and open it in the browser unless --no-browser;
//!                          --token fixes the session token, for tests
//!   splatcrab --http-stdio --port N --token T
//!                          answer HTTP requests read from stdin as --ui
//!                          answers them on port N with token T, one after
//!                          another, each response followed by a newline
//!                          (docs/modules/U1-ui-server.md)

mod term;

use std::io::{self, BufRead, IsTerminal, Write};

use splatcrab::error::{self, MError};
use splatcrab::{env, http, interp, protocol, server, syntax};

/// The version, from `Cargo.toml`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What `--help` prints.
const USAGE: &str = "\
Usage:
  splatcrab                 start the REPL
  splatcrab <script.m>      run a script file
  splatcrab --protocol      serve the evaluation protocol on stdin and stdout
                            (one JSON request per line, one response per line)
  splatcrab --ui [--port N] [--no-browser] [--token T]
                            serve the command window on 127.0.0.1 and open it
  splatcrab --http-stdio --port N --token T
                            answer HTTP requests read from stdin, as --ui would
  splatcrab --help          print this text
  splatcrab --version       print the version

In the REPL, 'exit' or 'quit' ends the session, and exit(n) exits with code n.
";

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

    match args.get(1).map(String::as_str) {
        Some("--help" | "-h") => {
            print!("{}", USAGE);
            return 0;
        }
        Some("--version" | "-V") => {
            println!("SplatCrab {}", VERSION);
            return 0;
        }
        _ => {}
    }
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
    if let Some(mode) = args.get(1).filter(|a| *a == "--ui" || *a == "--http-stdio") {
        return match ui_options(mode, &args[2..]) {
            Ok(opts) if mode == "--ui" => ui(opts),
            Ok(opts) => http_stdio(opts),
            Err(e) => {
                eprintln!("Error: {}", e);
                1
            }
        };
    }

    // Output to stdout and warnings to stderr, for a script and the REPL.
    let mut it = interp::Interp::new();
    // `clc` writes only to a terminal, and a bare `pause` waits only at one.
    it.stdout_tty = io::stdout().is_terminal();
    it.stdin_tty = io::stdin().is_terminal();

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
            // Since cycle 13b a UTF-16 file is recognised and decoded too.
            Ok(bytes) => splatcrab::lexer::decode_source(&bytes),
            Err(e) => {
                eprintln!("Error: {}", error::cannot_read(path, &e));
                return 1;
            }
        };
        let result = it.run(&src);
        // Flush what the script printed before anything else: `main` no
        // longer returns normally, so nothing else will.
        let _ = it.out.flush();
        if let Err(e) = result {
            // `exit(n)` ends the script with its code; it is not an error.
            if let Some(code) = e.exit_code() {
                return code;
            }
            // `MError`'s Display supplies the `Line N: ` part when a line is
            // known, so the prefix is spelled in exactly one place. The
            // line is the script's own; the functions the error came out of
            // follow, innermost first.
            eprintln!("Error: {}", e);
            eprint!("{}", e.trace());
            return 1;
        }
        return 0;
    }

    println!("SplatCrab {}  (type 'exit' to quit)\n", VERSION);
    let stdin = io::stdin();
    // Figures are shown only to a person at a terminal (cycle 12).
    let interactive = stdin.is_terminal();
    // The line editor, only when a person is at a terminal on both ends
    // (cycle 13); anything piped reads plain lines exactly as before.
    let mut editor = if interactive && io::stdout().is_terminal() {
        term::LineReader::new()
    } else {
        None
    };
    let mut buf = String::new();
    loop {
        let prompt = if buf.is_empty() { ">> " } else { "   " };
        let line = match &mut editor {
            Some(ed) => {
                let mut complete =
                    |p: &str| env::completions(p, it.vars(), it.builtins(), &it.path_dirs());
                match ed.read_line(prompt, &mut complete) {
                    term::Line::Text(t) => t + "\n",
                    // Ctrl-C drops the whole unfinished entry.
                    term::Line::Cleared => {
                        buf.clear();
                        continue;
                    }
                    term::Line::Eof => break,
                }
            }
            None => {
                print!("{}", prompt);
                io::stdout().flush().ok();
                let mut line = String::new();
                match stdin.lock().read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => line,
                }
            }
        };
        buf.push_str(&line);
        if !syntax::is_complete(&buf) {
            continue;
        }
        // A command-line entry: a `function` block is refused. `exit` and
        // `quit` are statements since cycle 13, wherever they are in the
        // entry, and `exit(n)` ends the session with code `n`.
        if let Err(e) = it.run_command(&buf) {
            if let Some(code) = e.exit_code() {
                let _ = it.out.flush();
                return code;
            }
            report(&mut it, &e);
        }
        if interactive {
            show_figures(&mut it);
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

/// Hands every figure the last entry changed to the operating system's
/// viewer (cycle 12): each is written to a temporary SVG, one file per
/// figure number, and opened as `--ui` opens its page. Only the interactive
/// REPL calls this, when standard input is a terminal, so a script, a
/// golden case, CI, `--protocol`, `--ui` and `--http-stdio` never open a
/// viewer. A failure to write or to open the file is ignored: the figure
/// is still there for `saveas`.
fn show_figures(it: &mut interp::Interp) {
    for n in it.take_changed_figures() {
        let Some(svg) = it.figure_svg(n) else {
            continue;
        };
        let name = format!("splatcrab-{}-figure-{}.svg", std::process::id(), n);
        let path = std::env::temp_dir().join(name);
        if std::fs::write(&path, svg).is_ok() {
            if let Some(p) = path.to_str() {
                server::open_browser(p);
            }
        }
    }
}

/// The options of `--ui` and `--http-stdio`.
struct UiOptions {
    port: Option<u16>,
    token: Option<String>,
    browser: bool,
}

/// Parses the arguments after `--ui` or `--http-stdio`. `--http-stdio`
/// needs both `--port` and `--token`, since there is no socket to pick a
/// port and nobody to read a generated token; `--no-browser` is `--ui`'s.
fn ui_options(mode: &str, rest: &[String]) -> Result<UiOptions, MError> {
    let mut opts = UiOptions {
        port: None,
        token: None,
        browser: true,
    };
    let mut rest = rest.iter();
    while let Some(opt) = rest.next() {
        match opt.as_str() {
            "--port" => {
                let text = rest.next().ok_or_else(|| error::option_needs_value(opt))?;
                opts.port = Some(text.parse().map_err(|_| error::bad_port(text))?);
            }
            "--token" => {
                let text = rest.next().ok_or_else(|| error::option_needs_value(opt))?;
                // It travels in a URL fragment and a header, and on
                // Windows through `cmd /C start`, which would read `&`,
                // `|`, `^`, `<`, `>` and `%` as its own syntax: so only
                // characters that mean nothing to any of the three.
                if text.is_empty()
                    || !text
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._~-".contains(&b))
                {
                    return Err(error::bad_token());
                }
                opts.token = Some(text.clone());
            }
            "--no-browser" if mode == "--ui" => opts.browser = false,
            _ => return Err(error::unknown_option(mode, opt)),
        }
    }
    if mode == "--http-stdio" {
        if opts.port.is_none() {
            return Err(error::missing_option(mode, "--port"));
        }
        if opts.token.is_none() {
            return Err(error::missing_option(mode, "--token"));
        }
    }
    Ok(opts)
}

/// `splatcrab --ui`: binds, prints the URL, opens it, and serves until
/// killed. It returns only when binding fails.
fn ui(opts: UiOptions) -> i32 {
    let wanted = opts.port.unwrap_or(0);
    let listener = match server::bind(wanted) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error: {}", error::cannot_listen(wanted, &e));
            return 1;
        }
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => {
            eprintln!("Error: {}", error::cannot_listen(wanted, &e));
            return 1;
        }
    };
    let token = opts.token.unwrap_or_else(server::new_token);
    let url = server::url(port, &token);
    println!("SplatCrab UI: {}", url);
    io::stdout().flush().ok();
    if opts.browser {
        server::open_browser(&url);
    }
    let cfg = http::Config { port, token };
    // Nothing is written outside an `eval`, which captures both sinks.
    let mut it = interp::Interp::with_sinks(Box::new(io::sink()), Box::new(io::sink()));
    server::serve(&listener, &mut it, &cfg)
}

/// `splatcrab --http-stdio`: exits 0 at end of input whatever the requests
/// were, and 1, silently as `--protocol` does, only if stdin or stdout fails.
fn http_stdio(opts: UiOptions) -> i32 {
    let cfg = http::Config {
        port: opts.port.unwrap_or_default(),
        token: opts.token.unwrap_or_default(),
    };
    match http::serve_stdio(io::stdin().lock(), io::stdout().lock(), &cfg) {
        Ok(()) => 0,
        Err(_) => 1,
    }
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
/// rather than information. The trace of the functions the error came out
/// of follows it, as in script mode.
fn report(it: &mut interp::Interp, e: &splatcrab::error::MError) {
    let _ = it.out.flush();
    io::stdout().flush().ok();
    eprintln!("Error: {}", e.msg);
    eprint!("{}", e.trace());
}
