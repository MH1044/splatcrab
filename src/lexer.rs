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
    /// `{`, which opens a brace index `c{1}` (and, from cycle 07, a cell
    /// literal).
    LBrace,
    RBrace,
    Comma,
    Semi,
    Newline,
    Colon,
    /// A lone `.`, the field access of `s.a` and `s.(name)`. Every dotted
    /// operator (`.*`, `./`, `.\`, `.^`, `.'`), a number's decimal point and
    /// a `...` continuation are recognised before this is.
    Dot,
    /// `@`, which cycle 06 gives function handles. Until then it lexes, and
    /// the parser refuses it.
    At,

    If,
    ElseIf,
    Else,
    End,
    For,
    While,
    Break,
    Continue,
    Switch,
    Case,
    Otherwise,
    Try,
    Catch,
    /// `function`, which opens a definition (cycle 05).
    Function,
    /// `return`, which leaves the running function or script (cycle 05).
    Return,

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
            Token::LBrace => "{",
            Token::RBrace => "}",
            Token::Comma => ",",
            Token::Semi => ";",
            Token::Colon => ":",
            Token::Dot => ".",
            Token::At => "@",

            Token::If => "if",
            Token::ElseIf => "elseif",
            Token::Else => "else",
            Token::End => "end",
            Token::For => "for",
            Token::While => "while",
            Token::Break => "break",
            Token::Continue => "continue",
            Token::Switch => "switch",
            Token::Case => "case",
            Token::Otherwise => "otherwise",
            Token::Try => "try",
            Token::Catch => "catch",
            Token::Function => "function",
            Token::Return => "return",
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
            | Token::RBrace
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
    /// True when a `%{` block comment was still open at the end of the
    /// source. A script takes the rest of the file as the comment; the REPL
    /// and the protocol's `complete` read it as an unfinished entry.
    pub open_comment: bool,
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

    fn len(&self) -> usize {
        self.tokens.len()
    }
}

/// The token stream alone, for callers that have no use for the lines: the
/// parser's own tests, and the REPL's `needs_more`.
pub fn lex(src: &str) -> R<Vec<Token>> {
    Ok(scan(src)?.tokens)
}

/// [`scan_known`] with no variables known beforehand: a script, which starts
/// from an empty workspace, and the questions `syntax.rs` answers.
pub fn scan(src: &str) -> R<Lexed> {
    scan_known(src, &|_| false)
}

/// True when the line holding position `i` is `mark` and nothing else but
/// whitespace: how `%{` and `%}` are told from ordinary comments.
fn line_is(chars: &[char], i: usize, mark: &str) -> bool {
    let start = chars[..i]
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |k| k + 1);
    let end = chars[i..]
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |k| i + k);
    let text: String = chars[start..end].iter().collect();
    text.trim_matches(|c| matches!(c, ' ' | '\t' | '\r')) == mark
}

/// The index of the newline ending the line that holds `i`, or the length.
fn line_end(chars: &[char], i: usize) -> usize {
    chars[i..]
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |k| i + k)
}

/// The length of the operator starting at `k`, if one does: what MATLAB's
/// command-syntax rule looks for after the first word.
fn operator_len(chars: &[char], k: usize) -> Option<usize> {
    const TWO: [&str; 10] = ["==", "~=", "<=", ">=", "&&", "||", ".*", "./", ".\\", ".^"];
    let pair: String = chars[k..chars.len().min(k + 2)].iter().collect();
    if TWO.contains(&pair.as_str()) {
        return Some(2);
    }
    "+-*/\\^<>&|:=".contains(chars[k]).then_some(1)
}

/// Whether the name ending at `i`, first on its statement and not a
/// variable, is a command: `clear x`, `disp hello`, `hold on`.
///
/// MATLAB's rule, from its "Command vs. Function Syntax" page: whitespace
/// after the name, then a word that is not an operator followed by
/// whitespace. So `disp hello` and `x -1` are commands, while `x - 1`,
/// `x = 1`, `x == 1` and `disp (1)` are not, and neither is a name alone
/// or followed by a comment, a separator or a continuation.
fn is_command(chars: &[char], i: usize) -> bool {
    let n = chars.len();
    if !matches!(chars.get(i), Some(' ' | '\t')) {
        return false;
    }
    let mut k = i;
    while k < n && matches!(chars[k], ' ' | '\t') {
        k += 1;
    }
    if k >= n || matches!(chars[k], '\n' | '\r' | ',' | ';' | '%' | '(') {
        return false;
    }
    if is_continuation(chars, k) {
        return false;
    }
    // A lone `=` is an assignment, spaced or not: `x =1` assigns.
    if chars[k] == '=' && chars.get(k + 1) != Some(&'=') {
        return false;
    }
    match operator_len(chars, k) {
        Some(len) => !matches!(chars.get(k + len), None | Some(' ' | '\t' | '\r' | '\n')),
        None => true,
    }
}

/// The arguments of a command from `k`, and where they end: each word is one
/// argument, a quoted part of a word may hold spaces (`disp 'a b'`, with `''`
/// for a quote), and the command ends at a newline, a comma, a semicolon or
/// a comment outside quotes.
fn command_words(chars: &[char], mut k: usize, line: u32) -> R<(Vec<String>, usize)> {
    let n = chars.len();
    let ends = |c: char| matches!(c, '\n' | ',' | ';' | '%');
    let mut words = Vec::new();
    loop {
        while k < n && matches!(chars[k], ' ' | '\t' | '\r') {
            k += 1;
        }
        if k >= n || ends(chars[k]) {
            return Ok((words, k));
        }
        let mut w = String::new();
        while k < n && !ends(chars[k]) && !matches!(chars[k], ' ' | '\t' | '\r') {
            if chars[k] != '\'' {
                w.push(chars[k]);
                k += 1;
                continue;
            }
            k += 1;
            loop {
                if k >= n || chars[k] == '\n' {
                    bail!(error::unterminated_string().at(line));
                }
                if chars[k] == '\'' {
                    if chars.get(k + 1) == Some(&'\'') {
                        w.push('\'');
                        k += 2;
                        continue;
                    }
                    k += 1;
                    break;
                }
                w.push(chars[k]);
                k += 1;
            }
        }
        words.push(w);
    }
}

/// The names a statement just assigned, found when its `=` is lexed:
/// `x = ...` and `x(2) = ...` assign `x`, `for k = ...` assigns `k`, and
/// `[a, ~, c] = ...` assigns every name the bracket lists. A later statement
/// that starts with one of them is never command syntax.
fn assigned_names(stmt: &[Token]) -> Vec<String> {
    match stmt {
        [Token::Ident(n), ..] | [Token::For, Token::Ident(n), ..] => vec![n.clone()],
        [Token::LBracket, rest @ ..] => {
            let mut names = Vec::new();
            let mut depth = 0usize;
            let mut prev = &Token::LBracket;
            for t in rest {
                match t {
                    Token::LBracket | Token::LParen | Token::LBrace => depth += 1,
                    Token::RBracket if depth == 0 => break,
                    Token::RBracket | Token::RParen | Token::RBrace => {
                        depth = depth.saturating_sub(1)
                    }
                    Token::Ident(n)
                        if depth == 0 && matches!(prev, Token::LBracket | Token::Comma) =>
                    {
                        names.push(n.clone())
                    }
                    _ => {}
                }
                prev = t;
            }
            names
        }
        _ => Vec::new(),
    }
}

/// The outputs and parameters a `function` line names, from its `function`
/// token to where it ends: every identifier but the function's own name,
/// which is the one straight after the `=`, or the first when there is none.
fn header_names(line: &[Token]) -> Vec<String> {
    let has_outputs = line.contains(&Token::Assign);
    let mut past_assign = !has_outputs;
    let mut named = false;
    let mut names = Vec::new();
    for t in line.iter().skip(1) {
        match t {
            Token::Assign => past_assign = true,
            Token::Ident(_) if past_assign && !named => named = true,
            Token::Ident(n) => names.push(n.clone()),
            _ => {}
        }
    }
    names
}

/// A UTF-8 byte-order mark, which a Windows editor or `Out-File` writes at the
/// start of a file. It is an encoding marker, not source, and MATLAB and
/// Octave both skip it; SplatCrab used to report
/// `unexpected character '\u{feff}'` on line 1 (QA D29).
const BOM: char = '\u{feff}';

/// Tokens and their lines, with `known` saying which names are variables
/// already, which is what decides command syntax for a name the source has
/// not assigned itself: `x -1` is `x - 1` when `x` is a variable and the
/// command `x('-1')` when it is not.
///
/// Whether a name is a variable is judged here, before anything runs, from
/// the workspace the source starts in and the names the source assigned
/// earlier, as MATLAB judges it in a file. A command is desugared on the
/// spot into the call it means, `name('word', ...)`, so the parser never
/// sees command syntax at all.
pub fn scan_known(src: &str, known: &dyn Fn(&str) -> bool) -> R<Lexed> {
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
    // `C` is the brace of a `case {...}` list, which separates its values
    // the way a bracket separates elements.
    let mut open: Vec<char> = Vec::new();
    // Where the statement being lexed began, as a token index: a name there
    // is where command syntax can start.
    let mut stmt_start = 0;
    // Names this source has assigned so far.
    let mut assigned: std::collections::HashSet<String> = std::collections::HashSet::new();
    // The statement whose assigned names were last collected.
    let mut named_at: Option<usize> = None;
    let mut open_comment = false;
    // Where the `function` line being lexed began, until it ends.
    let mut header: Option<usize> = None;
    // Past the first `function`, a workspace of its own: the variables the
    // source started with are not its variables.
    let mut in_function = false;

    while i < n {
        let c = chars[i];
        let in_bracket = matches!(open.last(), Some('[' | 'C'));
        // A statement starts after a separator outside every bracket, and
        // after the keywords that a statement may follow on the same line.
        // A comma last with nothing open was pushed with nothing open,
        // since closing a bracket pushes a token of its own.
        if open.is_empty()
            && toks.last().is_none_or(|t| {
                matches!(
                    t,
                    Token::Newline
                        | Token::Semi
                        | Token::Comma
                        | Token::Else
                        | Token::Try
                        | Token::Otherwise
                )
            })
        {
            stmt_start = toks.len();
            // A `function` line just ended: its outputs and its parameters
            // are the function's first variables, so `a -1` in its body is
            // an expression, as MATLAB reads it.
            if let Some(h) = header.take() {
                assigned.extend(header_names(&toks.tokens[h..]));
            }
        }

        // A block comment: `%{` alone on its line, through the matching
        // `%}` alone on its line. They nest, and they are counted rather
        // than recursed into. Left open, it runs to the end of the source.
        if c == '%' && line_is(&chars, i, "%{") {
            let mut depth = 1usize;
            i = line_end(&chars, i);
            while depth > 0 {
                if i >= n {
                    open_comment = true;
                    break;
                }
                // `i` is on a newline: step onto the next line.
                i += 1;
                line += 1;
                if i < n && line_is(&chars, i, "%{") {
                    depth += 1;
                } else if i < n && line_is(&chars, i, "%}") {
                    depth -= 1;
                }
                i = line_end(&chars, i.min(n));
            }
            continue;
        }

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
                    // `[a {1}]` and `[a @f]` are two elements, never the
                    // brace index `a{1}` or a stray `@` after `a`.
                    || next == '{'
                    || next == '@'
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
                "switch" => Token::Switch,
                "case" => Token::Case,
                "otherwise" => Token::Otherwise,
                "try" => Token::Try,
                "catch" => Token::Catch,
                "function" => Token::Function,
                "return" => Token::Return,
                _ => Token::Ident(word),
            };
            // Each function is a workspace of its own, so what the source
            // assigned before this one says nothing about names inside it.
            if tok == Token::Function && toks.len() == stmt_start {
                assigned.clear();
                in_function = true;
                header = Some(toks.len());
            }
            if let Token::Ident(name) = &tok {
                // `catch e` binds `e`.
                if toks.last() == Some(&Token::Catch) {
                    assigned.insert(name.clone());
                } else if toks.len() == stmt_start
                    && open.is_empty()
                    && (in_function || !known(name))
                    && !assigned.contains(name)
                    && is_command(&chars, i)
                {
                    let (words, end) = command_words(&chars, i, line)?;
                    toks.push(tok, line);
                    toks.push(Token::LParen, line);
                    for (k, w) in words.into_iter().enumerate() {
                        if k > 0 {
                            toks.push(Token::Comma, line);
                        }
                        toks.push(Token::Str(w), line);
                    }
                    toks.push(Token::RParen, line);
                    i = end;
                    continue;
                }
            }
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
            // A brace is pushed like a bracket so that `[c{1 2} 3]` knows,
            // inside the braces, that it is not directly inside `[`: the
            // whitespace rule belongs to the bracket alone. Whether it also
            // applies directly inside a cell literal `{1 -2}` is cycle 07's.
            // The brace of a `case {2 3}` list does separate its values by
            // whitespace, as a bracket does.
            ('{', _) => {
                open.push(if toks.last() == Some(&Token::Case) {
                    'C'
                } else {
                    '{'
                });
                (Token::LBrace, 1)
            }
            ('}', _) => {
                open.pop();
                (Token::RBrace, 1)
            }
            (',', _) => (Token::Comma, 1),
            (';', _) => (Token::Semi, 1),
            (':', _) => (Token::Colon, 1),
            // Every dotted operator matched above, and a number's point and
            // a continuation were taken before this match, so what is left
            // is the field access of `s.a` and `s.(n)`.
            ('.', _) => (Token::Dot, 1),
            ('@', _) => (Token::At, 1),
            _ => bail!(error::unexpected_char(c).at(line)),
        };
        // Once per statement, so that `a = b = c = ...` stays linear.
        if tok == Token::Assign && open.is_empty() && named_at != Some(stmt_start) {
            named_at = Some(stmt_start);
            assigned.extend(assigned_names(&toks.tokens[stmt_start.min(toks.len())..]));
        }
        toks.push(tok, line);
        i += len;
    }

    toks.push(Token::Eof, line);
    Ok(Lexed {
        tokens: toks.tokens,
        lines: toks.lines,
        open_comment,
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
        // Not `a # b`, which is the command `a('#', 'b')` since cycle 04.
        assert!(lex("x = a # b").is_err());
        assert!(lex("#").is_err());
        assert!(lex("$").is_err());
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
        assert_eq!(scan("x = 1\ny = #").unwrap_err().line, Some(2));
        assert_eq!(scan("x = 1\n\ny = 'abc").unwrap_err().line, Some(3));
        // A continuation before the bad character still moves the count on.
        assert_eq!(scan("x = 1 ...\n#").unwrap_err().line, Some(2));
    }

    // ---- braces, the field dot and `@` (cycle 03) ----------------------

    #[test]
    fn braces_dot_and_at_are_tokens() {
        assert_eq!(
            lx("c{2}"),
            vec![
                id("c"),
                Token::LBrace,
                Token::Num(2.0),
                Token::RBrace,
                Token::Eof
            ]
        );
        assert_eq!(lx("s.a"), vec![id("s"), Token::Dot, id("a"), Token::Eof]);
        assert_eq!(
            lx("s.(n)"),
            vec![
                id("s"),
                Token::Dot,
                Token::LParen,
                id("n"),
                Token::RParen,
                Token::Eof
            ]
        );
        assert_eq!(lx("@"), vec![Token::At, Token::Eof]);
        assert_eq!(lx("@sin"), vec![Token::At, id("sin"), Token::Eof]);
        assert_eq!(
            lx("c{1}(2).b"),
            vec![
                id("c"),
                Token::LBrace,
                Token::Num(1.0),
                Token::RBrace,
                Token::LParen,
                Token::Num(2.0),
                Token::RParen,
                Token::Dot,
                id("b"),
                Token::Eof,
            ]
        );
    }

    /// The field dot must not take over any of the dotted forms that already
    /// existed: a decimal point, the five dotted operators and a `...`.
    #[test]
    fn the_field_dot_leaves_every_older_dot_alone() {
        assert_eq!(lx("1.5"), vec![Token::Num(1.5), Token::Eof]);
        assert_eq!(lx(".5"), vec![Token::Num(0.5), Token::Eof]);
        assert_eq!(
            lx("x.^2"),
            vec![id("x"), Token::DotCaret, Token::Num(2.0), Token::Eof]
        );
        assert_eq!(
            lx("x.*y"),
            vec![id("x"), Token::DotStar, id("y"), Token::Eof]
        );
        assert_eq!(
            lx("x./y"),
            vec![id("x"), Token::DotSlash, id("y"), Token::Eof]
        );
        assert_eq!(
            lx("x.\\y"),
            vec![id("x"), Token::DotBackslash, id("y"), Token::Eof]
        );
        assert_eq!(lx("x.'"), vec![id("x"), Token::Transpose, Token::Eof]);
        assert_eq!(
            lx("a...\n+ b"),
            vec![id("a"), Token::Plus, id("b"), Token::Eof]
        );
        assert_eq!(
            lx("a = 1...\n+ 2"),
            vec![
                id("a"),
                Token::Assign,
                Token::Num(1.0),
                Token::Plus,
                Token::Num(2.0),
                Token::Eof
            ]
        );
        // `2.5.^x` keeps its decimal point and its operator.
        assert_eq!(
            lx("2.5.^x"),
            vec![Token::Num(2.5), Token::DotCaret, id("x"), Token::Eof]
        );
    }

    /// A quote after a closing brace or a field name is a transpose.
    #[test]
    fn a_quote_after_a_brace_or_a_field_is_a_transpose() {
        assert_eq!(
            lx("c{1}'"),
            vec![
                id("c"),
                Token::LBrace,
                Token::Num(1.0),
                Token::RBrace,
                Token::Transpose,
                Token::Eof
            ]
        );
        assert_eq!(
            lx("s.a'"),
            vec![id("s"), Token::Dot, id("a"), Token::Transpose, Token::Eof]
        );
        // A quote straight after the dot is the `.'` operator, as before.
        assert_eq!(lx("s.'"), vec![id("s"), Token::Transpose, Token::Eof]);
        // After an opening brace it still opens a string.
        assert_eq!(
            lx("c{'a'}"),
            vec![id("c"), Token::LBrace, st("a"), Token::RBrace, Token::Eof]
        );
    }

    /// Inside brackets a brace or an `@` after whitespace starts an element,
    /// and inside a brace index the bracket's whitespace rule does not apply.
    #[test]
    fn braces_and_at_meet_the_whitespace_rule() {
        assert_eq!(
            lx("[a {1}]"),
            vec![
                Token::LBracket,
                id("a"),
                Token::Comma,
                Token::LBrace,
                Token::Num(1.0),
                Token::RBrace,
                Token::RBracket,
                Token::Eof,
            ]
        );
        assert_eq!(
            lx("[a @f]"),
            vec![
                Token::LBracket,
                id("a"),
                Token::Comma,
                Token::At,
                id("f"),
                Token::RBracket,
                Token::Eof,
            ]
        );
        assert_eq!(
            lx("[c{1 -2} 3]"),
            vec![
                Token::LBracket,
                id("c"),
                Token::LBrace,
                Token::Num(1.0),
                Token::Minus,
                Token::Num(2.0),
                Token::RBrace,
                Token::Comma,
                Token::Num(3.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
        // `[s.a s.b]` is two elements.
        assert_eq!(
            lx("[s.a s.b]"),
            vec![
                Token::LBracket,
                id("s"),
                Token::Dot,
                id("a"),
                Token::Comma,
                id("s"),
                Token::Dot,
                id("b"),
                Token::RBracket,
                Token::Eof,
            ]
        );
        // `[a, ~] = ...` and `[a ~]` both separate the placeholder.
        assert_eq!(
            lx("[a ~]"),
            vec![
                Token::LBracket,
                id("a"),
                Token::Comma,
                Token::Not,
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn the_new_tokens_display_as_written() {
        assert_eq!(Token::LBrace.to_string(), "'{'");
        assert_eq!(Token::RBrace.to_string(), "'}'");
        assert_eq!(Token::Dot.to_string(), "'.'");
        assert_eq!(Token::At.to_string(), "'@'");
    }

    // ---- switch, try, block comments and commands (cycle 04) -----------

    #[test]
    fn switch_and_try_keywords_are_their_own_tokens() {
        assert_eq!(
            lx("switch case otherwise try catch"),
            vec![
                Token::Switch,
                Token::Case,
                Token::Otherwise,
                Token::Try,
                Token::Catch,
                Token::Eof,
            ]
        );
        assert_eq!(lx("switches"), vec![id("switches"), Token::Eof]);
        assert_eq!(lx("catcher"), vec![id("catcher"), Token::Eof]);
        assert_eq!(Token::Otherwise.to_string(), "'otherwise'");
        assert_eq!(Token::Catch.to_string(), "'catch'");
    }

    #[test]
    fn a_block_comment_hides_its_lines_and_keeps_the_count() {
        let l = scan("%{\ndisp(111)\n%}\nx = 1").unwrap();
        assert_eq!(
            l.tokens,
            vec![
                Token::Newline,
                id("x"),
                Token::Assign,
                Token::Num(1.0),
                Token::Eof
            ]
        );
        // The newline after `%}` is line 3's; `x` is on line 4.
        assert_eq!(l.lines, vec![3, 4, 4, 4, 4]);
        assert!(!l.open_comment);
    }

    #[test]
    fn block_comment_markers_may_have_whitespace_and_nest() {
        let src = "  %{ \t\n  %{\ny = 2\n  %}\nz = 3\n%}  \r\nx = 1";
        let toks = lx(src);
        assert_eq!(
            toks,
            vec![
                Token::Newline,
                id("x"),
                Token::Assign,
                Token::Num(1.0),
                Token::Eof
            ]
        );
    }

    #[test]
    fn a_marker_with_anything_else_on_its_line_is_an_ordinary_comment() {
        assert_eq!(
            lx("%{ not a block\nx = 1"),
            vec![
                Token::Newline,
                id("x"),
                Token::Assign,
                Token::Num(1.0),
                Token::Eof
            ]
        );
        assert_eq!(
            lx("x = 1 %{\ny = 2"),
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
        // A `%}` with no opener is a comment too.
        assert_eq!(
            lx("%}\n1"),
            vec![Token::Newline, Token::Num(1.0), Token::Eof]
        );
    }

    #[test]
    fn an_unterminated_block_comment_runs_to_the_end() {
        let l = scan("x = 1\n%{\ny = 2\n%{\n%}\n").unwrap();
        assert_eq!(
            l.tokens,
            vec![
                id("x"),
                Token::Assign,
                Token::Num(1.0),
                Token::Newline,
                Token::Eof
            ]
        );
        assert!(l.open_comment);
        // Deep nesting is a count, not a recursion.
        let deep = "%{\n".repeat(200_000);
        assert!(scan(&deep).unwrap().open_comment);
        let closed = format!("{}{}", "%{\n".repeat(50_000), "%}\n".repeat(50_000));
        assert!(!scan(&closed).unwrap().open_comment);
    }

    fn call_tokens(name: &str, words: &[&str]) -> Vec<Token> {
        let mut t = vec![id(name), Token::LParen];
        for (k, w) in words.iter().enumerate() {
            if k > 0 {
                t.push(Token::Comma);
            }
            t.push(st(w));
        }
        t.push(Token::RParen);
        t
    }

    #[test]
    fn a_command_becomes_the_call_it_means() {
        let mut want = call_tokens("clear", &["x", "y"]);
        want.push(Token::Eof);
        assert_eq!(lx("clear x y"), want);
        let mut want = call_tokens("disp", &["hello"]);
        want.push(Token::Eof);
        assert_eq!(lx("disp hello"), want);
        // Quotes group words, and a doubled quote is a quote.
        let mut want = call_tokens("disp", &["a b", "it's"]);
        want.push(Token::Eof);
        assert_eq!(lx("disp 'a b' 'it''s'"), want);
        // A word that starts with an operator not followed by space.
        let mut want = call_tokens("x", &["-1"]);
        want.push(Token::Eof);
        assert_eq!(lx("x -1"), want);
    }

    #[test]
    fn a_command_ends_at_a_separator_or_a_comment() {
        let mut want = call_tokens("hold", &["on"]);
        want.extend([Token::Semi, id("y"), Token::Eof]);
        assert_eq!(lx("hold on; y"), want);
        let mut want = call_tokens("disp", &["a"]);
        want.extend([Token::Comma, id("b"), Token::Eof]);
        assert_eq!(lx("disp a, b"), want);
        let mut want = call_tokens("disp", &["a"]);
        want.push(Token::Eof);
        assert_eq!(lx("disp a % note"), want);
        // Inside quotes a separator is text.
        let mut want = call_tokens("disp", &["a;b,c%d"]);
        want.push(Token::Eof);
        assert_eq!(lx("disp 'a;b,c%d'"), want);
        assert!(lex("disp 'open").is_err());
    }

    #[test]
    fn what_is_not_a_command_lexes_as_before() {
        let expr = |src: &str, want: Vec<Token>| assert_eq!(lx(src), want, "{src}");
        expr(
            "x - 1",
            vec![id("x"), Token::Minus, Token::Num(1.0), Token::Eof],
        );
        expr(
            "x = 1",
            vec![id("x"), Token::Assign, Token::Num(1.0), Token::Eof],
        );
        expr(
            "x =1",
            vec![id("x"), Token::Assign, Token::Num(1.0), Token::Eof],
        );
        expr(
            "x == 1",
            vec![id("x"), Token::Eq, Token::Num(1.0), Token::Eof],
        );
        expr(
            "disp (1)",
            vec![
                id("disp"),
                Token::LParen,
                Token::Num(1.0),
                Token::RParen,
                Token::Eof,
            ],
        );
        expr("x ;", vec![id("x"), Token::Semi, Token::Eof]);
        expr("x % c", vec![id("x"), Token::Eof]);
        expr(
            "x ...\n+ 1",
            vec![id("x"), Token::Plus, Token::Num(1.0), Token::Eof],
        );
        // Only the first word of a statement: `f(a b)` and `[a -1]` are
        // not statements that start with `a`.
        expr(
            "y = a -1",
            vec![
                id("y"),
                Token::Assign,
                id("a"),
                Token::Minus,
                Token::Num(1.0),
                Token::Eof,
            ],
        );
    }

    #[test]
    fn a_variable_is_never_a_command() {
        let minus = vec![id("x"), Token::Minus, Token::Num(1.0), Token::Eof];
        // Known to the workspace.
        let known = scan_known("x -1", &|n| n == "x").unwrap().tokens;
        assert_eq!(known, minus);
        // Assigned earlier in the source, by each form of assignment.
        for src in [
            "x = 3; x -1",
            "x(2) = 3; x -1",
            "for x = 1:2, end, x -1",
            "[a, x] = size(1); x -1",
            "try, catch x, end, x -1",
        ] {
            let toks = lx(src);
            let tail = &toks[toks.len() - 4..];
            assert_eq!(tail, &minus[..], "{src}");
        }
    }

    #[test]
    fn a_command_can_start_any_statement() {
        let toks = lx("if 1, disp hi, end");
        assert!(toks.contains(&st("hi")), "{toks:?}");
        let toks = lx("try\ndisp hi\ncatch\nend");
        assert!(toks.contains(&st("hi")), "{toks:?}");
        let toks = lx("try disp hi, end");
        assert!(toks.contains(&st("hi")), "{toks:?}");
        // Never the catch variable.
        assert_eq!(lx("catch e"), vec![Token::Catch, id("e"), Token::Eof]);
    }

    #[test]
    fn a_case_list_separates_its_values_by_whitespace() {
        assert_eq!(
            lx("case {2 3}"),
            vec![
                Token::Case,
                Token::LBrace,
                Token::Num(2.0),
                Token::Comma,
                Token::Num(3.0),
                Token::RBrace,
                Token::Eof,
            ]
        );
        // A brace index keeps its old rule.
        assert_eq!(
            lx("c{1 -2}"),
            vec![
                id("c"),
                Token::LBrace,
                Token::Num(1.0),
                Token::Minus,
                Token::Num(2.0),
                Token::RBrace,
                Token::Eof,
            ]
        );
    }

    /// Cycle 05: `function` and `return` are keywords, and a function line
    /// makes its outputs and parameters variables of the body, while the
    /// names the script assigned, and the workspace, are not.
    #[test]
    fn a_function_line_names_its_variables() {
        assert_eq!(
            lx("function y = f(x)\nreturn"),
            vec![
                Token::Function,
                id("y"),
                Token::Assign,
                id("f"),
                Token::LParen,
                id("x"),
                Token::RParen,
                Token::Newline,
                Token::Return,
                Token::Eof,
            ]
        );
        let minus = [id("a"), Token::Minus, Token::Num(1.0)];
        for src in [
            "function r = f(a)\na -1",
            "function [a, b] = f()\na -1",
            "function f(q, a)\na -1",
        ] {
            let toks = lx(src);
            assert_eq!(&toks[toks.len() - 4..toks.len() - 1], &minus[..], "{src}");
        }
        // The function's own name is not one of them, and neither is a name
        // assigned before the function or known to the workspace.
        let cmd = [id("a"), Token::LParen, st("-1"), Token::RParen];
        for src in [
            "function r = a()\na -1",
            "function a\na -1",
            "a = 1;\nfunction g()\na -1",
        ] {
            let toks = lx(src);
            assert_eq!(&toks[toks.len() - 5..toks.len() - 1], &cmd[..], "{src}");
        }
        let toks = scan_known("function g()\na -1", &|n| n == "a")
            .unwrap()
            .tokens;
        assert_eq!(&toks[toks.len() - 5..toks.len() - 1], &cmd[..]);
    }
}
