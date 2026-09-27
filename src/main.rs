//! SplatCrab: a small MATLAB-compatible interpreter.
//!
//!   splatcrab            start the REPL
//!   splatcrab script.m   run a script file

use std::io::{self, BufRead, Write};

use splatcrab::interp;
use splatcrab::lexer::{self, Token};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut it = interp::Interp::new();

    if args.len() > 1 {
        let path = &args[1];
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Cannot read {}: {}", path, e);
                std::process::exit(1);
            }
        };
        if let Err(e) = it.run(&src) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
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
