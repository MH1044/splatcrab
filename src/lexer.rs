//! Tokenizer for MATLAB-style source.
//!
//! Two MATLAB quirks live here rather than in the parser:
//!  * Inside `[ ... ]`, whitespace separates elements (`[1 -2]` is two elements,
//!    `[1 - 2]` is one) and a newline acts like `;`.
//!  * `'` is a transpose after a value and a string delimiter otherwise.

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

pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut toks: Vec<Token> = Vec::new();
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

        // Line continuation `...`: skip the rest of the line including the newline.
        if c == '.' && i + 2 < n && chars[i + 1] == '.' && chars[i + 2] == '.' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            i += 1;
            continue;
        }

        // Whitespace: inside brackets it may separate elements.
        if c == ' ' || c == '\t' || c == '\r' {
            while i < n && matches!(chars[i], ' ' | '\t' | '\r') {
                i += 1;
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
                    toks.push(Token::Comma);
                }
            }
            continue;
        }

        if c == '\n' {
            i += 1;
            toks.push(if in_bracket {
                Token::Semi
            } else {
                Token::Newline
            });
            continue;
        }

        // Numbers: 12, 1.5, .5, 1e-3, 2.5E+2
        if c.is_ascii_digit() || (c == '.' && i + 1 < n && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < n && chars[i].is_ascii_digit() {
                i += 1;
            }
            // `2.*x` must not swallow the dot of `.*`
            if i < n
                && chars[i] == '.'
                && !(i + 1 < n && matches!(chars[i + 1], '*' | '/' | '^' | '\''))
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
            let v: f64 = text
                .parse()
                .map_err(|_| format!("invalid number '{}'", text))?;
            toks.push(Token::Num(v));
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
            toks.push(tok);
            continue;
        }

        // Transpose or single-quoted string.
        if c == '\'' {
            if toks.last().is_some_and(ends_value) {
                i += 1;
                toks.push(Token::Transpose);
                continue;
            }
            i += 1;
            let mut s = String::new();
            loop {
                if i >= n || chars[i] == '\n' {
                    return Err("unterminated string".to_string());
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
            toks.push(Token::Str(s));
            continue;
        }

        // Double-quoted string.
        if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                if i >= n || chars[i] == '\n' {
                    return Err("unterminated string".to_string());
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
            toks.push(Token::Str(s));
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
            _ => return Err(format!("unexpected character '{}'", c)),
        };
        toks.push(tok);
        i += len;
    }

    toks.push(Token::Eof);
    Ok(toks)
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
}
