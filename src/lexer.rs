//! Tokenizer for MATLAB-style source.
//!
//! Two MATLAB quirks live here rather than in the parser:
//!  * Inside `[ ... ]`, whitespace separates elements (`[1 -2]` is two elements,
//!    `[1 - 2]` is one) and a newline acts like `;`.
//!  * `'` is a transpose after a value and a string delimiter otherwise.

use std::fmt;

use crate::bail;
use crate::error::{self, R};

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Num(f64),
    Ident(String),
    Str(String),

    Plus,
    Minus,
    Star,
    Slash,
    Backslash,
    Caret,
    DotStar,
    DotSlash,
    DotBackslash,
    DotCaret,
    Transpose,

    Assign,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    AndAnd,
    OrOr,
    Not,

    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Newline,
    Colon,

    If,
    ElseIf,
    Else,
    End,
    For,
    While,
    Break,
    Continue,

    Eof,
}

/// How a token is named to a human, as opposed to `Debug`, which names the
/// Rust variant.
///
/// Every parse message renders the offending token through this, so
/// `y = x + ;` reports `unexpected ';' in expression` rather than
/// `unexpected Semi in expression`. One rule covers the whole form: a
/// token's text is quoted whatever the token is, so a number reads `'0.3'`
/// beside `')'` and an identifier `'x1F'`. The two tokens with no spelling at
/// all, the end of a line and the end of the input, say so in words instead.
///
/// The one rendering that reads worse than the rest is `Transpose`, whose
/// spelling is itself a quote: it comes out as `'''`. A string and an
/// identifier also render alike, since both are `'text'`; the surrounding
/// message says which was expected.
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let word = match self {
            // `{}` on an `f64` is the shortest round-tripping form, so `0.3`
            // stays `0.3` and `2.0` shows as `2`, the way it was written.
            Token::Num(v) => return write!(f, "'{}'", v),
            Token::Ident(s) | Token::Str(s) => return write!(f, "'{}'", s),
            Token::Newline => return f.write_str("end of line"),
            Token::Eof => return f.write_str("end of input"),

            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Backslash => "\\",
            Token::Caret => "^",
            Token::DotStar => ".*",
            Token::DotSlash => "./",
            Token::DotBackslash => ".\\",
            Token::DotCaret => ".^",
            Token::Transpose => "'",

            Token::Assign => "=",
            Token::Eq => "==",
            Token::Ne => "~=",
            Token::Lt => "<",
            Token::Le => "<=",
            Token::Gt => ">",
            Token::Ge => ">=",
            Token::And => "&",
            Token::Or => "|",
            Token::AndAnd => "&&",
            Token::OrOr => "||",
            Token::Not => "~",

            Token::LParen => "(",
            Token::RParen => ")",
            Token::LBracket => "[",
            Token::RBracket => "]",
            Token::Comma => ",",
            Token::Semi => ";",
            Token::Colon => ":",

            Token::If => "if",
            Token::ElseIf => "elseif",
            Token::Else => "else",
            Token::End => "end",
            Token::For => "for",
            Token::While => "while",
            Token::Break => "break",
            Token::Continue => "continue",
        };
        write!(f, "'{}'", word)
    }
}

/// Can this token be the last token of a value?  Used to decide whether a
/// following `'` is a transpose and whether whitespace inside brackets is a
/// separator.
fn ends_value(t: &Token) -> bool {
    matches!(
        t,
        Token::Num(_)
            | Token::Ident(_)
            | Token::Str(_)
            | Token::RParen
            | Token::RBracket
            | Token::Transpose
            | Token::End
    )
}

/// True when a `...` line continuation starts at `i`.
fn is_continuation(chars: &[char], i: usize) -> bool {
    chars.get(i) == Some(&'.') && chars.get(i + 1) == Some(&'.') && chars.get(i + 2) == Some(&'.')
}

/// The lexer's full result: the token stream, and the source line each token
/// came from.
///
/// The lines live beside the tokens rather than inside them. Pairing each
/// token with its line would have changed what a `Token` is, and with it all
/// 43 token-stream assertions in this file's tests; a parallel vector leaves
/// both them and `lex` below untouched, and the parser reads `lines[pos]`
/// exactly where it already reads `toks[pos]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    /// `lines[k]` is the 1-based source line `tokens[k]` starts on. Always the
    /// same length as `tokens`.
    pub lines: Vec<u32>,
}

/// Tokens and their lines, pushed together so the two can never drift.
struct Out {
    tokens: Vec<Token>,
    lines: Vec<u32>,
}

impl Out {
    fn push(&mut self, t: Token, line: u32) {
        self.tokens.push(t);
        self.lines.push(line);
    }

    fn last(&self) -> Option<&Token> {
        self.tokens.last()
    }
}

/// The token stream alone, for callers that have no use for the lines: the
/// parser's own tests, and the REPL's `needs_more`.
pub fn lex(src: &str) -> R<Vec<Token>> {
    Ok(scan(src)?.tokens)
}

/// A UTF-8 byte-order mark, which a Windows editor or `Out-File` writes at the
/// start of a file. It is an encoding marker, not source, and MATLAB and
/// Octave both skip it; SplatCrab used to report
/// `unexpected character '\u{feff}'` on line 1 (QA D29).
const BOM: char = '\u{feff}';

pub fn scan(src: &str) -> R<Lexed> {
    // Only a *leading* mark is skipped. One in the middle of a file is a real
    // stray character and still reported as one.
    let src = src.strip_prefix(BOM).unwrap_or(src);
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut line: u32 = 1;
    let mut toks = Out {
        tokens: Vec::new(),
        lines: Vec::new(),
    };
    // Stack of open delimiters so we know whether the innermost one is `[`.
    let mut open: Vec<char> = Vec::new();

    while i < n {
        let c = chars[i];
        let in_bracket = open.last() == Some(&'[');

        // Comment to end of line.
        if c == '%' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Whitespace and `...` continuations are both gaps between tokens, and
        // inside brackets a gap may separate two elements. They are handled
        // together so both kinds reach the separator check below: a `...` that
        // skipped straight past its newline used to leave the cursor on the
        // next character, so `[1 ...` newline `-2]` came out as one element
        // worth `-1` instead of two.
        if matches!(c, ' ' | '\t' | '\r') || is_continuation(&chars, i) {
            while i < n {
                if matches!(chars[i], ' ' | '\t' | '\r') {
                    i += 1;
                } else if is_continuation(&chars, i) {
                    while i < n && chars[i] != '\n' {
                        i += 1;
                    }
                    // Step over the newline the continuation hides. It is
                    // still a line of the file, so it still counts.
                    if i < n {
                        line += 1;
                    }
                    i += 1;
                } else {
                    break;
                }
            }
            if in_bracket && i < n {
                let prev_ends = toks.last().is_some_and(ends_value);
                let next = chars[i];
                let next1 = chars.get(i + 1).copied().unwrap_or(' ');
                let starts_value = next.is_ascii_alphanumeric()
                    || next == '_'
                    || next == '('
                    || next == '['
                    || next == '\''
                    || next == '"'
                    || next == '~'
                    || (next == '.' && next1.is_ascii_digit());
                let signed_value =
                    (next == '+' || next == '-') && !matches!(next1, ' ' | '\t' | '\r' | '\n');
                if prev_ends && (starts_value || signed_value) {
                    toks.push(Token::Comma, line);
                }
            }
            continue;
        }

        if c == '\n' {
            i += 1;
            toks.push(
                if in_bracket {
                    Token::Semi
                } else {
                    Token::Newline
                },
                line,
            );
            line += 1;
            continue;
        }

        // Numbers: 12, 1.5, .5, 1e-3, 2.5E+2
        if c.is_ascii_digit() || (c == '.' && i + 1 < n && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < n && chars[i].is_ascii_digit() {
                i += 1;
            }
            // `2.*x` must not swallow the dot of `.*`. The list covers every
            // character that can follow the dot of a dotted operator: the
            // backslash of `.\`, which would otherwise leave `2.\x` silently
            // meaning `2 \ x`, and the dot of `...`, which would otherwise
            // make `a = 1...` lex as `1.` followed by a stray `..`.
            if i < n
                && chars[i] == '.'
                && !(i + 1 < n && matches!(chars[i + 1], '*' | '/' | '\\' | '^' | '\'' | '.'))
            {
                i += 1;
                while i < n && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < n && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < n && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < n && chars[j].is_ascii_digit() {
                    while j < n && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let text: String = chars[start..i].iter().collect();
            let v: f64 = match text.parse() {
                Ok(v) => v,
                Err(_) => bail!(error::invalid_number(&text).at(line)),
            };
            toks.push(Token::Num(v), line);
            continue;
        }

        // Identifiers and keywords.
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < n && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let tok = match word.as_str() {
                "if" => Token::If,
                "elseif" => Token::ElseIf,
                "else" => Token::Else,
                "end" => Token::End,
                "for" => Token::For,
                "while" => Token::While,
                "break" => Token::Break,
                "continue" => Token::Continue,
                _ => Token::Ident(word),
            };
            toks.push(tok, line);
            continue;
        }

        // Transpose or single-quoted string.
        if c == '\'' {
            if toks.last().is_some_and(ends_value) {
                i += 1;
                toks.push(Token::Transpose, line);
                continue;
            }
            i += 1;
            let mut s = String::new();
            loop {
                if i >= n || chars[i] == '\n' {
                    bail!(error::unterminated_string().at(line));
                }
                if chars[i] == '\'' {
                    if i + 1 < n && chars[i + 1] == '\'' {
                        s.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(chars[i]);
                i += 1;
            }
            toks.push(Token::Str(s), line);
            continue;
        }

        // Double-quoted string.
        if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                if i >= n || chars[i] == '\n' {
                    bail!(error::unterminated_string().at(line));
                }
                if chars[i] == '"' {
                    if i + 1 < n && chars[i + 1] == '"' {
                        s.push('"');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(chars[i]);
                i += 1;
            }
            toks.push(Token::Str(s), line);
            continue;
        }

        // Operators and punctuation.
        let next = chars.get(i + 1).copied();
        let (tok, len) = match (c, next) {
            ('=', Some('=')) => (Token::Eq, 2),
            ('~', Some('=')) => (Token::Ne, 2),
            ('<', Some('=')) => (Token::Le, 2),
            ('>', Some('=')) => (Token::Ge, 2),
            ('&', Some('&')) => (Token::AndAnd, 2),
            ('|', Some('|')) => (Token::OrOr, 2),
            ('.', Some('*')) => (Token::DotStar, 2),
            ('.', Some('/')) => (Token::DotSlash, 2),
            ('.', Some('\\')) => (Token::DotBackslash, 2),
            ('.', Some('^')) => (Token::DotCaret, 2),
            ('.', Some('\'')) => (Token::Transpose, 2),
            ('+', _) => (Token::Plus, 1),
            ('-', _) => (Token::Minus, 1),
            ('*', _) => (Token::Star, 1),
            ('/', _) => (Token::Slash, 1),
            ('\\', _) => (Token::Backslash, 1),
            ('^', _) => (Token::Caret, 1),
            ('=', _) => (Token::Assign, 1),
            ('<', _) => (Token::Lt, 1),
            ('>', _) => (Token::Gt, 1),
            ('&', _) => (Token::And, 1),
            ('|', _) => (Token::Or, 1),
            ('~', _) => (Token::Not, 1),
            ('(', _) => {
                open.push('(');
                (Token::LParen, 1)
            }
            (')', _) => {
                open.pop();
                (Token::RParen, 1)
            }
            ('[', _) => {
                open.push('[');
                (Token::LBracket, 1)
            }
            (']', _) => {
                open.pop();
                (Token::RBracket, 1)
            }
            (',', _) => (Token::Comma, 1),
            (';', _) => (Token::Semi, 1),
            (':', _) => (Token::Colon, 1),
            _ => bail!(error::unexpected_char(c).at(line)),
        };
        toks.push(tok, line);
        i += len;
    }

    toks.push(Token::Eof, line);
    Ok(Lexed {
        tokens: toks.tokens,
        lines: toks.lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lx(src: &str) -> Vec<Token> {
        lex(src).expect("lex should succeed")
    }

    fn id(s: &str) -> Token {
        Token::Ident(s.to_string())
    }

    fn st(s: &str) -> Token {
        Token::Str(s.to_string())
    }

    // ---- whitespace inside brackets ----------------------------------

    #[test]
    fn space_before_signed_number_separates_elements() {
        assert_eq!(
            lx("[1 -2]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::Minus,
                Token::Num(2.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn spaces_on_both_sides_of_minus_stay_binary() {
        let expected = vec![
            Token::LBracket,
            Token::Num(1.0),
            Token::Minus,
            Token::Num(2.0),
            Token::RBracket,
            Token::Eof,
        ];
        assert_eq!(lx("[1 - 2]"), expected);
        assert_eq!(lx("[1-2]"), expected);
    }

    #[test]
    fn space_separated_transposes_are_two_elements() {
        assert_eq!(
            lx("[a' b']"),
            vec![
                Token::LBracket,
                id("a"),
                Token::Transpose,
                Token::Comma,
                id("b"),
                Token::Transpose,
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn signed_identifier_separates_but_spaced_minus_does_not() {
        assert_eq!(
            lx("[x -y]"),
            vec![
                Token::LBracket,
                id("x"),
                Token::Comma,
                Token::Minus,
                id("y"),
                Token::RBracket,
                Token::Eof,
            ]
        );
        assert_eq!(
            lx("[x - y]"),
            vec![
                Token::LBracket,
                id("x"),
                Token::Minus,
                id("y"),
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn space_before_paren_separates_elements() {
        assert_eq!(
            lx("[1 (2)]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::LParen,
                Token::Num(2.0),
                Token::RParen,
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn whitespace_outside_brackets_never_separates() {
        assert_eq!(
            lx("1 -2"),
            vec![Token::Num(1.0), Token::Minus, Token::Num(2.0), Token::Eof]
        );
    }

    // ---- quote disambiguation ----------------------------------------

    #[test]
    fn quote_after_identifier_is_transpose() {
        assert_eq!(lx("a'"), vec![id("a"), Token::Transpose, Token::Eof]);
    }

    #[test]
    fn leading_quote_is_a_string() {
        assert_eq!(lx("'abc'"), vec![st("abc"), Token::Eof]);
    }

    #[test]
    fn doubled_single_quote_is_an_escape() {
        assert_eq!(lx("'it''s'"), vec![st("it's"), Token::Eof]);
    }

    #[test]
    fn doubled_double_quote_is_an_escape() {
        assert_eq!(lx("\"dq\"\"x\""), vec![st("dq\"x"), Token::Eof]);
    }

    #[test]
    fn quote_after_rparen_is_transpose() {
        assert_eq!(
            lx("A(1)'"),
            vec![
                id("A"),
                Token::LParen,
                Token::Num(1.0),
                Token::RParen,
                Token::Transpose,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn quote_after_assign_is_a_string() {
        assert_eq!(
            lx("x = 'a'"),
            vec![id("x"), Token::Assign, st("a"), Token::Eof]
        );
    }

    #[test]
    fn quote_after_rbracket_is_transpose() {
        assert_eq!(
            lx("[1 2]'"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::Num(2.0),
                Token::RBracket,
                Token::Transpose,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn dot_quote_is_transpose() {
        assert_eq!(lx(".'"), vec![Token::Transpose, Token::Eof]);
        assert_eq!(lx("a.'"), vec![id("a"), Token::Transpose, Token::Eof]);
    }

    // ---- numbers ------------------------------------------------------

    #[test]
    fn number_literal_forms() {
        assert_eq!(lx("12"), vec![Token::Num(12.0), Token::Eof]);
        assert_eq!(lx("1.5"), vec![Token::Num(1.5), Token::Eof]);
        assert_eq!(lx(".5"), vec![Token::Num(0.5), Token::Eof]);
        assert_eq!(lx("1e-3"), vec![Token::Num(0.001), Token::Eof]);
        assert_eq!(lx("2.5E+2"), vec![Token::Num(250.0), Token::Eof]);
    }

    #[test]
    fn number_does_not_swallow_dot_of_dot_star() {
        assert_eq!(
            lx("2.*x"),
            vec![Token::Num(2.0), Token::DotStar, id("x"), Token::Eof]
        );
    }

    #[test]
    fn number_does_not_swallow_dot_of_dot_caret() {
        assert_eq!(
            lx("1.^2"),
            vec![
                Token::Num(1.0),
                Token::DotCaret,
                Token::Num(2.0),
                Token::Eof
            ]
        );
    }

    #[test]
    fn number_does_not_swallow_dot_of_dot_slash() {
        assert_eq!(
            lx("4./2"),
            vec![
                Token::Num(4.0),
                Token::DotSlash,
                Token::Num(2.0),
                Token::Eof
            ]
        );
    }

    #[test]
    fn number_does_not_swallow_dot_of_dot_backslash() {
        // Without the backslash in the exclusion list this was `Num(2.0)`
        // followed by `Backslash`, so `2.\x` silently meant `2 \ x`.
        assert_eq!(
            lx("2.\\x"),
            vec![Token::Num(2.0), Token::DotBackslash, id("x"), Token::Eof]
        );
        assert_eq!(
            lx("2 .\\ 8"),
            vec![
                Token::Num(2.0),
                Token::DotBackslash,
                Token::Num(8.0),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn number_does_not_swallow_the_dot_of_a_continuation() {
        // `1...` used to lex as `1.` plus a stray `..`.
        assert_eq!(
            lx("1...\n+ 2"),
            vec![Token::Num(1.0), Token::Plus, Token::Num(2.0), Token::Eof]
        );
        assert_eq!(
            lx("1.0...\n+ 2"),
            vec![Token::Num(1.0), Token::Plus, Token::Num(2.0), Token::Eof]
        );
    }

    #[test]
    fn number_followed_by_dot_transpose() {
        assert_eq!(
            lx("3.'"),
            vec![Token::Num(3.0), Token::Transpose, Token::Eof]
        );
    }

    // ---- newlines -----------------------------------------------------

    #[test]
    fn newline_inside_brackets_is_a_row_separator() {
        assert_eq!(
            lx("[1\n2]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Semi,
                Token::Num(2.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn newline_outside_brackets_is_a_statement_separator() {
        assert_eq!(
            lx("1\n2"),
            vec![Token::Num(1.0), Token::Newline, Token::Num(2.0), Token::Eof]
        );
    }

    #[test]
    fn crlf_lexes_the_same_as_lf() {
        assert_eq!(lx("x = 1\r\ny = 2"), lx("x = 1\ny = 2"));
        assert_eq!(
            lx("[1 2\r\n3 4]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::Num(2.0),
                Token::Semi,
                Token::Num(3.0),
                Token::Comma,
                Token::Num(4.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
        assert_eq!(lx("[1 2\r\n3 4]"), lx("[1 2\n3 4]"));
    }

    // ---- comments and continuations -----------------------------------

    #[test]
    fn comment_runs_to_end_of_line_only() {
        assert_eq!(
            lx("x = 1 % set x\ny = 2"),
            vec![
                id("x"),
                Token::Assign,
                Token::Num(1.0),
                Token::Newline,
                id("y"),
                Token::Assign,
                Token::Num(2.0),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn comment_at_end_of_input_is_dropped() {
        assert_eq!(
            lx("a = 1 % trailing"),
            vec![id("a"), Token::Assign, Token::Num(1.0), Token::Eof]
        );
    }

    #[test]
    fn line_continuation_joins_lines() {
        assert_eq!(
            lx("1 + ...\n2"),
            vec![Token::Num(1.0), Token::Plus, Token::Num(2.0), Token::Eof]
        );
    }

    #[test]
    fn a_continuation_may_follow_any_token() {
        let joined = vec![Token::Num(1.0), Token::Plus, Token::Num(2.0), Token::Eof];
        assert_eq!(lx("1 ...\n+ 2"), joined);
        assert_eq!(lx("1...\n+ 2"), joined);
        assert_eq!(
            lx("x...\n+ 2"),
            vec![id("x"), Token::Plus, Token::Num(2.0), Token::Eof]
        );
        assert_eq!(
            lx("(1)...\n+ 2"),
            vec![
                Token::LParen,
                Token::Num(1.0),
                Token::RParen,
                Token::Plus,
                Token::Num(2.0),
                Token::Eof,
            ]
        );
        assert_eq!(
            lx("y = [1]...\n+ 2"),
            vec![
                id("y"),
                Token::Assign,
                Token::LBracket,
                Token::Num(1.0),
                Token::RBracket,
                Token::Plus,
                Token::Num(2.0),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn a_continuation_inside_brackets_still_separates_elements() {
        // The `...` branch used to skip past the newline and leave the cursor
        // on the minus, so no separating `Comma` was inserted and `[1 ...`
        // newline `-2]` collapsed to the single element `-1`.
        let two = vec![
            Token::LBracket,
            Token::Num(1.0),
            Token::Comma,
            Token::Minus,
            Token::Num(2.0),
            Token::RBracket,
            Token::Eof,
        ];
        assert_eq!(lx("[1 ...\n-2]"), two);
        // The leading-space form, which was correct by accident.
        assert_eq!(lx("[1 ...\n -2]"), two);
        // A continuation does not invent a separator where a space would not:
        // spaces on both sides of the minus still make it binary.
        assert_eq!(
            lx("[1 ...\n- 2]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Minus,
                Token::Num(2.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn a_continuation_inside_brackets_separates_plain_elements() {
        assert_eq!(
            lx("[1 ...\n2]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::Num(2.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    // ---- keywords and operators ----------------------------------------

    #[test]
    fn keywords_are_their_own_tokens() {
        assert_eq!(
            lx("if elseif else end for while break continue"),
            vec![
                Token::If,
                Token::ElseIf,
                Token::Else,
                Token::End,
                Token::For,
                Token::While,
                Token::Break,
                Token::Continue,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn keyword_prefix_is_an_identifier() {
        assert_eq!(lx("endx"), vec![id("endx"), Token::Eof]);
        assert_eq!(lx("iffy"), vec![id("iffy"), Token::Eof]);
        assert_eq!(lx("_x1"), vec![id("_x1"), Token::Eof]);
    }

    #[test]
    fn not_versus_not_equal() {
        assert_eq!(lx("a~=b"), vec![id("a"), Token::Ne, id("b"), Token::Eof]);
        assert_eq!(lx("~a"), vec![Token::Not, id("a"), Token::Eof]);
    }

    #[test]
    fn two_character_operators() {
        assert_eq!(
            lx("== ~= <= >= && || .* ./ .^"),
            vec![
                Token::Eq,
                Token::Ne,
                Token::Le,
                Token::Ge,
                Token::AndAnd,
                Token::OrOr,
                Token::DotStar,
                Token::DotSlash,
                Token::DotCaret,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn dot_backslash_is_its_own_operator() {
        assert_eq!(
            lx("a.\\b"),
            vec![id("a"), Token::DotBackslash, id("b"), Token::Eof]
        );
        assert_eq!(
            lx("a .\\ b"),
            vec![id("a"), Token::DotBackslash, id("b"), Token::Eof]
        );
        // A lone backslash is still matrix left division.
        assert_eq!(
            lx("a\\b"),
            vec![id("a"), Token::Backslash, id("b"), Token::Eof]
        );
    }

    #[test]
    fn single_character_operators() {
        assert_eq!(
            lx("+ - * / \\ ^ = < > & | ~ ( ) , ; :"),
            vec![
                Token::Plus,
                Token::Minus,
                Token::Star,
                Token::Slash,
                Token::Backslash,
                Token::Caret,
                Token::Assign,
                Token::Lt,
                Token::Gt,
                Token::And,
                Token::Or,
                Token::Not,
                Token::LParen,
                Token::RParen,
                Token::Comma,
                Token::Semi,
                Token::Colon,
                Token::Eof,
            ]
        );
    }

    // ---- errors --------------------------------------------------------

    #[test]
    fn unterminated_string_is_an_error() {
        assert!(lex("'abc").is_err());
        assert!(lex("x = 'abc\ny = 1").is_err());
        assert!(lex("\"abc").is_err());
    }

    #[test]
    fn unexpected_character_is_an_error() {
        assert!(lex("a @ b").is_err());
        assert!(lex("#").is_err());
    }

    // ---- line numbers ---------------------------------------------------

    #[test]
    fn every_token_carries_its_source_line() {
        let l = scan("x = 1\ny = 2").unwrap();
        assert_eq!(l.tokens.len(), l.lines.len());
        // Ident Assign Num Newline | Ident Assign Num Eof.
        // The newline that ends line 1 belongs to line 1.
        assert_eq!(l.lines, vec![1, 1, 1, 1, 2, 2, 2, 2]);
    }

    #[test]
    fn a_continuation_still_advances_the_line_count() {
        // The `+` continues the statement on line 1 but sits on line 2, and
        // that is the line an error in it has to name.
        let l = scan("a = 1...\n+ 2\nb = 3").unwrap();
        assert_eq!(l.lines, vec![1, 1, 1, 2, 2, 2, 3, 3, 3, 3]);
    }

    #[test]
    fn a_newline_inside_brackets_counts_as_a_line() {
        let l = scan("x = [1\n2]\ny = 3").unwrap();
        // Ident Assign LBracket Num(1) Semi Num(2) RBracket Newline ...
        assert_eq!(l.lines[..8], [1, 1, 1, 1, 1, 2, 2, 2]);
        assert_eq!(*l.lines.last().unwrap(), 3);
    }

    #[test]
    fn crlf_and_lf_number_the_lines_the_same() {
        assert_eq!(
            scan("x = 1\r\ny = 2\r\nz = 3").unwrap().lines,
            scan("x = 1\ny = 2\nz = 3").unwrap().lines
        );
    }

    #[test]
    fn a_lexer_error_names_the_line_it_happened_on() {
        assert_eq!(scan("x = 1\ny = @").unwrap_err().line, Some(2));
        assert_eq!(scan("x = 1\n\ny = 'abc").unwrap_err().line, Some(3));
        // A continuation before the bad character still moves the count on.
        assert_eq!(scan("x = 1 ...\n@").unwrap_err().line, Some(2));
    }
}
