//! SplatCrab: a small MATLAB-compatible interpreter.
//!
//!   splatcrab            start the REPL
//!   splatcrab script.m   run a script file

use std::io::{self, BufRead, Write};

use splatcrab::interp;
use splatcrab::lexer::{self, Token};

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

/// Runs the CLI and returns the process exit code.
fn run() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let mut it = interp::Interp::new();

    if args.len() > 1 {
        let path = &args[1];
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Cannot read {}: {}", path, e);
                return 1;
            }
        };
        let result = it.run(&src);
        // Flush what the script printed before anything else: `main` no
        // longer returns normally, so nothing else will.
        let _ = it.out.flush();
        if let Err(e) = result {
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
        if needs_more(&buf) {
            continue;
        }
        if let Err(e) = it.run(&buf) {
            println!("Error: {}", e);
        }
        buf.clear();
    }
    let _ = it.out.flush();
    0
}

/// True while the buffered input has an unclosed bracket or block, so the
/// REPL keeps reading lines (like MATLAB's continuation for `for ... end`).
fn needs_more(src: &str) -> bool {
    let toks = match lexer::lex(src) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let mut brackets: i32 = 0;
    let mut parens: i32 = 0;
    let mut blocks: i32 = 0;
    for t in &toks {
        match t {
            Token::LBracket => brackets += 1,
            Token::RBracket => brackets -= 1,
            Token::LParen => parens += 1,
            Token::RParen => parens -= 1,
            Token::If | Token::For | Token::While => blocks += 1,
            Token::End if parens == 0 => blocks -= 1,
            _ => {}
        }
    }
    brackets > 0 || blocks > 0
}
