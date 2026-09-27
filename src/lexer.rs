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
                let prev_ends = toks.last().map_or(false, ends_value);
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
            toks.push(if in_bracket { Token::Semi } else { Token::Newline });
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
            if toks.last().map_or(false, ends_value) {
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
