//! Questions about source text that stop short of parsing it.
//!
//! [`is_complete`] decides when an entry typed line by line has ended. It
//! lived in `src/main.rs` as the REPL's `needs_more` until cycle U0 moved it
//! here, so that the terminal and the protocol's `complete` operation (and the
//! interface after it) ask the same function and can never disagree about
//! when an entry ends.

use crate::lexer::{self, Token};

/// True when `src` is a finished entry: no bracket and no block is still
/// open, so it can be run as it stands. False while an opener waits for its
/// closer, which is when the REPL keeps reading lines (like MATLAB's
/// continuation for `for ... end`).
///
/// Text that does not lex is complete: running it is what reports the error,
/// rather than a prompt that waits for more input that can never fix it.
///
/// A new statement that opens a block must be counted here, beside `if`,
/// `for` and `while`.
pub fn is_complete(src: &str) -> bool {
    let toks = match lexer::lex(src) {
        Ok(t) => t,
        Err(_) => return true,
    };
    let mut brackets: i32 = 0;
    let mut parens: i32 = 0;
    let mut blocks: i32 = 0;
    for t in &toks {
        match t {
            Token::LBracket => brackets += 1,
            Token::RBracket => brackets -= 1,
            // An `end` inside `(...)` or `{...}` is an index's, not a block's.
            Token::LParen | Token::LBrace => parens += 1,
            Token::RParen | Token::RBrace => parens -= 1,
            Token::If | Token::For | Token::While => blocks += 1,
            Token::End if parens == 0 => blocks -= 1,
            _ => {}
        }
    }
    brackets <= 0 && blocks <= 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The inputs of the U0 spec's acceptance test 4.
    #[test]
    fn the_spec_inputs() {
        assert!(!is_complete("for k = 1:3"));
        assert!(is_complete("for k = 1:3\ndisp(k)\nend"));
        assert!(!is_complete("x = [1 2"));
        assert!(is_complete("c{end}"));
        assert!(is_complete("x = 1"));
    }

    #[test]
    fn blocks_nest_and_an_index_end_is_not_a_block_end() {
        assert!(!is_complete("if true\nfor k = 1:2\nend"));
        assert!(is_complete("if true\nfor k = 1:2\nend\nend"));
        assert!(!is_complete("while x(end) > 0"));
        assert!(!is_complete("for k = 1:3\ny = x(end)"));
        assert!(is_complete("for k = 1:3, y = x(end); end"));
    }

    #[test]
    fn brackets_across_lines() {
        assert!(!is_complete("x = [1 2\n3 4"));
        assert!(is_complete("x = [1 2\n3 4]"));
        // An unbalanced closer is complete: running it reports the error.
        assert!(is_complete("x = 1]"));
        assert!(is_complete("end"));
    }

    #[test]
    fn empty_and_unlexable_text_is_complete() {
        assert!(is_complete(""));
        assert!(is_complete("\n"));
        assert!(is_complete("x = 'unterminated"));
        assert!(is_complete("x = #"));
    }
}
