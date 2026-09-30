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
    /// An imaginary literal (cycle 10), `1i`, `2.5j`, `1e3i`: the number
    /// written before its `i` or `j`, which the value multiplies.
    Imag(f64),
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
    /// `'` after a value: the conjugate transpose, `A'`.
    Transpose,
    /// `.'`: the plain transpose, which keeps the imaginary parts' signs
    /// (cycle 10; before it both spellings were one token).
    DotTranspose,

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
    /// `@`, which starts a function handle, `@name` or `@(x) body`
    /// (cycle 06).
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
            Token::Imag(v) => return write!(f, "'{}i'", v),
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
            Token::DotTranspose => ".'",

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
            | Token::Imag(_)
            | Token::Ident(_)
            | Token::Str(_)
            | Token::RParen
            | Token::RBracket
            | Token::RBrace
            | Token::Transpose
            | Token::DotTranspose
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

/// The integer-type suffixes a hexadecimal or binary literal may end in
/// (cycle 16), each with its width in bits and whether it is signed: the
/// eight of MATLAB's "Hexadecimal and Binary Values" page, and nothing else.
const INT_SUFFIXES: [(&str, u32, bool); 8] = [
    ("u8", 8, false),
    ("u16", 16, false),
    ("u32", 32, false),
    ("u64", 64, false),
    ("s8", 8, true),
    ("s16", 16, true),
    ("s32", 32, true),
    ("s64", 64, true),
];

/// Where the hexadecimal or binary literal starting at `start` ends, and
/// its value (cycle 16). `chars[start]` is the `0` and `chars[start + 1]`
/// its `x`, `X`, `b` or `B`. The literal is the prefix and the whole run of
/// letters, digits and underscores after it, so a malformed one is refused
/// as the text it is, `0x1Fz` whole, rather than read as a number and a
/// name. The value is `None` when the run is malformed; see
/// [`radix_value`].
fn radix_literal(chars: &[char], start: usize) -> (usize, Option<f64>) {
    let base = if matches!(chars[start + 1], 'x' | 'X') {
        16
    } else {
        2
    };
    let mut end = start + 2;
    while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    (end, radix_value(&chars[start + 2..end], base))
}

/// The value of a literal's run after its prefix, in `base` 16 or 2: one or
/// more digits of the base, then at most one of [`INT_SUFFIXES`], and
/// nothing else, or `None`.
///
/// The digits' value is computed exactly. Every type holds less than 2^64,
/// so accumulation stops as soon as the value passes 2^64 - 1: a `u128`
/// then never overflows, and a run of a million digits costs one pass over
/// its characters. Leading zeros add nothing, so the value, not the digit
/// count, decides whether a type holds it. With no suffix the value must be
/// below 2^64, with `uN` below 2^N, and with `sN` below 2^N too, a value of
/// 2^(N-1) or more then standing for itself minus 2^N, its two's
/// complement. The result is the double nearest the value, exact up to
/// 2^53, since SplatCrab has no integer classes.
fn radix_value(run: &[char], base: u32) -> Option<f64> {
    let digits = run.iter().take_while(|c| c.is_digit(base)).count();
    if digits == 0 {
        return None;
    }
    let rest = &run[digits..];
    let (bits, signed) = if rest.is_empty() {
        (64, false)
    } else {
        INT_SUFFIXES
            .iter()
            .find(|(s, _, _)| s.chars().eq(rest.iter().copied()))
            .map(|&(_, bits, signed)| (bits, signed))?
    };
    let mut value: u128 = 0;
    for c in &run[..digits] {
        value = value * u128::from(base) + u128::from(c.to_digit(base)?);
        if value > u128::from(u64::MAX) {
            return None;
        }
    }
    if value >= 1u128 << bits {
        return None;
    }
    if signed && value >= 1u128 << (bits - 1) {
        // Between -2^(N-1) and -1: the pattern read in two's complement.
        return Some((value as i128 - (1i128 << bits)) as f64);
    }
    Some(value as f64)
}

/// The text of a source file's bytes (cycle 13b). A file that starts with a
/// UTF-16 byte-order mark, `FF FE` little-endian or `FE FF` big-endian, is
/// UTF-16 and the mark is dropped; so is one without a mark whose first 64
/// bytes show the UTF-16 pattern of ASCII text, a zero byte in every odd
/// (little-endian) or every even (big-endian) position and none in the
/// others. Everything else is read as it always was: UTF-8, invalid bytes
/// decoded leniently as U+FFFD, and a leading UTF-8 mark left for
/// [`scan_known`] to skip. A lone surrogate or an odd trailing byte of a
/// UTF-16 file is U+FFFD too, so no file is ever refused for its encoding.
pub fn decode_source(bytes: &[u8]) -> String {
    let (body, big) = match bytes {
        [0xFF, 0xFE, rest @ ..] => (rest, false),
        [0xFE, 0xFF, rest @ ..] => (rest, true),
        _ => match utf16_pattern(bytes) {
            Some(big) => (bytes, big),
            None => return String::from_utf8_lossy(bytes).into_owned(),
        },
    };
    let units = body.chunks(2).map(|p| match (p, big) {
        ([a, b], false) => Ok(u16::from_le_bytes([*a, *b])),
        ([a, b], true) => Ok(u16::from_be_bytes([*a, *b])),
        // The odd byte out has no partner to make a code unit with.
        _ => Err(()),
    });
    let mut out = String::with_capacity(body.len() / 2);
    let mut pending = Vec::new();
    let flush = |pending: &mut Vec<u16>, out: &mut String| {
        out.extend(
            char::decode_utf16(pending.drain(..)).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)),
        );
    };
    for u in units {
        match u {
            Ok(u) => pending.push(u),
            Err(()) => {
                flush(&mut pending, &mut out);
                out.push(char::REPLACEMENT_CHARACTER);
            }
        }
    }
    flush(&mut pending, &mut out);
    out
}

/// Whether bytes without a mark look like UTF-16 ASCII text: `Some(false)`
/// for little-endian, `Some(true)` for big-endian. Only the first 64 bytes
/// are looked at, an even number of them, and at least one pair.
fn utf16_pattern(bytes: &[u8]) -> Option<bool> {
    let n = bytes.len().min(64) & !1;
    if n < 2 {
        return None;
    }
    let head = &bytes[..n];
    let zero_at = |parity: usize| {
        head.iter()
            .enumerate()
            .all(|(k, &b)| (b == 0) == (k % 2 == parity))
    };
    if zero_at(1) {
        Some(false)
    } else if zero_at(0) {
        Some(true)
    } else {
        None
    }
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
    // the way a bracket separates elements. `P` is the parameter list of an
    // `@(...)`, and `A` the body of an anonymous function written directly
    // inside a bracket or a brace (cycle 06): the body is one expression,
    // so whitespace inside it separates nothing, and it ends at the `,`,
    // `;`, newline or closer that ends the element it is.
    let mut open: Vec<char> = Vec::new();
    // The token index of the `)` that closed the last `@(...)` parameter
    // list: a quote straight after it opens a string, `@() 'hi'`, where
    // after any other `)` it would be a transpose.
    let mut params_close: Option<usize> = None;
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
        // What ends an element ends an anonymous function's body inside it.
        if matches!(c, ',' | ';' | '\n' | ']' | '}' | ')') {
            while open.last() == Some(&'A') {
                open.pop();
            }
        }
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

        // A hexadecimal or binary literal (cycle 16), `0x2A`, `0b101010`,
        // `0xFFs8`: a number that starts with `0` straight followed by `x`,
        // `X`, `b` or `B`. Any other number, `00x1F` and `1x2` among them,
        // is read below as it always was.
        if c == '0' && matches!(chars.get(i + 1), Some('x' | 'X' | 'b' | 'B')) {
            let start = i;
            let (end, value) = radix_literal(&chars, i);
            i = end;
            match value {
                Some(v) => toks.push(Token::Num(v), line),
                None => {
                    let text: String = chars[start..end].iter().collect();
                    bail!(error::invalid_number(&text).at(line));
                }
            }
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
            // An `i` or a `j` straight after the digits, and not the start
            // of a longer name, makes the literal imaginary (cycle 10): `1i`,
            // `2.5j`, `1e3i`. `2ix` and `3j_` are not imaginary literals.
            let unit = i < n && matches!(chars[i], 'i' | 'j');
            let word_goes_on = chars
                .get(i + 1)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_');
            if unit && !word_goes_on {
                i += 1;
                toks.push(Token::Imag(v), line);
                continue;
            }
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
            let after_params = params_close.is_some_and(|k| k + 1 == toks.len());
            if toks.last().is_some_and(ends_value) && !after_params {
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
            ('.', Some('\'')) => (Token::DotTranspose, 2),
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
                open.push(if toks.last() == Some(&Token::At) {
                    'P'
                } else {
                    '('
                });
                (Token::LParen, 1)
            }
            (')', _) => {
                if open.pop() == Some('P') {
                    params_close = Some(toks.len());
                    if matches!(open.last(), Some('[' | 'C' | '{')) {
                        open.push('A');
                    }
                }
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
            // whitespace rule belongs to the bracket alone. A brace that
            // opens a value rather than indexing one, a cell literal (cycle
            // 07) or the brace of a `case {2 3}` list, separates its values
            // by whitespace and its rows by newlines, as a bracket does: it
            // is the brace that does not follow the end of a value.
            ('{', _) => {
                let literal = !toks.last().is_some_and(ends_value);
                open.push(if literal { 'C' } else { '{' });
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

    /// Since cycle 10 `.'` is its own token, the plain transpose, and `'`
    /// the conjugate one.
    #[test]
    fn dot_quote_is_the_plain_transpose() {
        assert_eq!(lx(".'"), vec![Token::DotTranspose, Token::Eof]);
        assert_eq!(lx("a.'"), vec![id("a"), Token::DotTranspose, Token::Eof]);
        assert_eq!(
            lx("a.''"),
            vec![id("a"), Token::DotTranspose, Token::Transpose, Token::Eof]
        );
    }

    #[test]
    fn imaginary_literals() {
        assert_eq!(lx("1i"), vec![Token::Imag(1.0), Token::Eof]);
        assert_eq!(lx("2.5j"), vec![Token::Imag(2.5), Token::Eof]);
        assert_eq!(lx("1e3i"), vec![Token::Imag(1000.0), Token::Eof]);
        assert_eq!(lx(".5i"), vec![Token::Imag(0.5), Token::Eof]);
        assert_eq!(
            lx("3+4i"),
            vec![Token::Num(3.0), Token::Plus, Token::Imag(4.0), Token::Eof]
        );
        // In brackets it is one element, and a quote after it transposes.
        assert_eq!(
            lx("[1 2i]'"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::Imag(2.0),
                Token::RBracket,
                Token::Transpose,
                Token::Eof,
            ]
        );
        assert_eq!(
            lx("2i'"),
            vec![Token::Imag(2.0), Token::Transpose, Token::Eof]
        );
        // A longer word after the digits is not the unit.
        assert_eq!(lx("2ix"), vec![Token::Num(2.0), id("ix"), Token::Eof]);
        assert_eq!(lx("3j_"), vec![Token::Num(3.0), id("j_"), Token::Eof]);
        // The bare names are identifiers; the evaluator decides what they are.
        assert_eq!(lx("i"), vec![id("i"), Token::Eof]);
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
            vec![Token::Num(3.0), Token::DotTranspose, Token::Eof]
        );
    }

    // ---- hexadecimal and binary literals (cycle 16) ----------------------

    fn num(v: f64) -> Vec<Token> {
        vec![Token::Num(v), Token::Eof]
    }

    /// `0x`, `0X`, `0b` and `0B` each start one number, with or without any
    /// of the eight suffixes. The literal ends and starts a value as any
    /// number does, and every other number, and a name, lexes as before.
    #[test]
    fn hex_and_binary_literals_are_numbers() {
        for src in ["0x2A", "0X2a", "0x2a", "0X2A", "0b101010", "0B101010"] {
            assert_eq!(lx(src), num(42.0), "{src}");
        }
        assert_eq!(lx("0b10010110"), num(150.0));
        assert_eq!(lx("0x0"), num(0.0));
        assert_eq!(lx("0b0"), num(0.0));
        assert_eq!(lx("0xABCDEF"), num(11_259_375.0));
        // `e` is a hexadecimal digit, never an exponent.
        assert_eq!(lx("0x1e3"), num(483.0));
        for s in ["u8", "u16", "u32", "u64", "s8", "s16", "s32", "s64"] {
            assert_eq!(lx(&format!("0x2A{s}")), num(42.0), "{s}");
            assert_eq!(lx(&format!("0X2a{s}")), num(42.0), "{s}");
            assert_eq!(lx(&format!("0b101010{s}")), num(42.0), "{s}");
            assert_eq!(lx(&format!("0B101010{s}")), num(42.0), "{s}");
        }
        // One token, which ends a value and starts one.
        assert_eq!(
            lx("[0x1 0x2 0b11]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::Num(2.0),
                Token::Comma,
                Token::Num(3.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
        assert_eq!(
            lx("[0x1 -0b1]"),
            vec![
                Token::LBracket,
                Token::Num(1.0),
                Token::Comma,
                Token::Minus,
                Token::Num(1.0),
                Token::RBracket,
                Token::Eof,
            ]
        );
        assert_eq!(
            lx("-0x10"),
            vec![Token::Minus, Token::Num(16.0), Token::Eof]
        );
        assert_eq!(
            lx("0x10'"),
            vec![Token::Num(16.0), Token::Transpose, Token::Eof]
        );
        assert_eq!(
            lx("0x10+0b1"),
            vec![Token::Num(16.0), Token::Plus, Token::Num(1.0), Token::Eof]
        );
        assert_eq!(
            lx("v(0x2)"),
            vec![
                id("v"),
                Token::LParen,
                Token::Num(2.0),
                Token::RParen,
                Token::Eof
            ]
        );
        // The run ends at anything but a letter, a digit or an underscore.
        assert_eq!(
            lx("0x1F.^2"),
            vec![
                Token::Num(31.0),
                Token::DotCaret,
                Token::Num(2.0),
                Token::Eof
            ]
        );
        assert_eq!(
            lx("0xFFs8;"),
            vec![Token::Num(-1.0), Token::Semi, Token::Eof]
        );
        // A command's word is text, as any word is.
        assert_eq!(
            lx("disp 0x1F"),
            vec![
                id("disp"),
                Token::LParen,
                st("0x1F"),
                Token::RParen,
                Token::Eof
            ]
        );
        // A number that starts any other way, and a name, are unchanged.
        assert_eq!(lx("00x1F"), vec![Token::Num(0.0), id("x1F"), Token::Eof]);
        assert_eq!(lx("1x2"), vec![Token::Num(1.0), id("x2"), Token::Eof]);
        assert_eq!(lx("10b1"), vec![Token::Num(10.0), id("b1"), Token::Eof]);
        assert_eq!(lx("0.0x1"), vec![Token::Num(0.0), id("x1"), Token::Eof]);
        assert_eq!(lx(".0x1"), vec![Token::Num(0.0), id("x1"), Token::Eof]);
        assert_eq!(lx("a0x1"), vec![id("a0x1"), Token::Eof]);
        assert_eq!(lx("x0b1"), vec![id("x0b1"), Token::Eof]);
        assert_eq!(lx("0"), num(0.0));
        assert_eq!(lx("0.5"), num(0.5));
        assert_eq!(lx("0e5"), num(0.0));
        assert_eq!(lx("0i"), vec![Token::Imag(0.0), Token::Eof]);
        assert_eq!(lx("1i"), vec![Token::Imag(1.0), Token::Eof]);
    }

    /// `sN` reads the digits as an `N`-bit pattern in two's complement: at
    /// each width, zero and the largest positive value are themselves, and
    /// 2^(N-1) up to 2^N - 1 are those values minus 2^N.
    #[test]
    fn a_signed_suffix_reads_twos_complement() {
        let lit = |v: u64, radix: u32, s: &str| match radix {
            16 => format!("0x{v:X}{s}"),
            _ => format!("0b{v:b}{s}"),
        };
        for (bits, s) in [(8u32, "s8"), (16, "s16"), (32, "s32"), (64, "s64")] {
            let half = 1u64 << (bits - 1);
            let all = u64::MAX >> (64 - bits);
            for radix in [16, 2] {
                let at = |v: u64| lx(&lit(v, radix, s));
                assert_eq!(at(0), num(0.0), "{s}");
                assert_eq!(at(1), num(1.0), "{s}");
                assert_eq!(at(half - 1), num((half - 1) as f64), "{s}");
                assert_eq!(at(half), num(-(half as f64)), "{s}");
                assert_eq!(at(half + 1), num(-((half - 1) as f64)), "{s}");
                assert_eq!(at(all - 1), num(-2.0), "{s}");
                assert_eq!(at(all), num(-1.0), "{s}");
            }
        }
        // The page's own values.
        assert_eq!(lx("0x2As32"), num(42.0));
        assert_eq!(lx("0xFFs8"), num(-1.0));
        assert_eq!(lx("0x7Fs8"), num(127.0));
        assert_eq!(lx("0x80s8"), num(-128.0));
        assert_eq!(lx("0xFFFFs16"), num(-1.0));
        assert_eq!(lx("0xFFFFFFFFs32"), num(-1.0));
        assert_eq!(lx("0xFFFFFFFFFFFFFFFFs64"), num(-1.0));
        assert_eq!(lx("0b10010110s8"), num(-106.0));
        // Past 2^53 a value is the double nearest it, the value the page's
        // own conversion through a double gives.
        let low = -72_057_594_035_891_654_i64 as f64;
        let high = 81_997_179_153_022_975_i64 as f64;
        assert_eq!(low as i64, -72_057_594_035_891_656);
        assert_eq!(high as i64, 81_997_179_153_022_976);
        assert_eq!(lx("0xFF000000001F123As64"), num(low));
        assert_eq!(lx("0x1234FFFFFFFFFFFs64"), num(high));
        // An unsigned suffix never reads a negative value.
        assert_eq!(lx("0xFFu8"), num(255.0));
        assert_eq!(lx("0x80u8"), num(128.0));
        assert_eq!(lx("0xFFFFFFFFu32"), num(4_294_967_295.0));
        assert_eq!(lx("0xFFFFFFFFFFFFFFFFu64"), num(u64::MAX as f64));
    }

    /// The value, not the number of digits, decides whether a type holds a
    /// literal: below 2^64 with no suffix, below 2^N with `uN` or `sN`.
    /// Leading zeros count for nothing, and a million digits lex in one pass.
    #[test]
    fn a_literal_fits_its_type_by_value() {
        let refused = |src: &str| {
            assert_eq!(
                lex(src).unwrap_err().msg,
                format!("invalid number '{src}'"),
                "{src}"
            );
        };
        // 2^64 - 1 is the largest literal with no suffix, and 2^64 too large.
        assert_eq!(lx("0xFFFFFFFFFFFFFFFF"), num(u64::MAX as f64));
        assert_eq!(lx(&format!("0b{}", "1".repeat(64))), num(u64::MAX as f64));
        refused("0x10000000000000000");
        refused(&format!("0b1{}", "0".repeat(64)));
        refused("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");
        // 2^53 exactly, the last integer every double below it can hold.
        assert_eq!(lx("0x20000000000000"), num(9_007_199_254_740_992.0));
        // Each width's largest value, and one more.
        for bits in [8u32, 16, 32, 64] {
            let max = u64::MAX >> (64 - bits);
            let over = u128::from(max) + 1;
            for s in [format!("u{bits}"), format!("s{bits}")] {
                let top = lx(&format!("0x{max:X}{s}"));
                let want = if s.starts_with('u') { max as f64 } else { -1.0 };
                assert_eq!(top, num(want), "{s}");
                refused(&format!("0x{over:X}{s}"));
                refused(&format!("0b{over:b}{s}"));
            }
        }
        // Leading zeros are free.
        assert_eq!(lx("0x000000000000000000FF"), num(255.0));
        assert_eq!(lx("0x00"), num(0.0));
        assert_eq!(lx(&format!("0x{}FFu8", "0".repeat(100))), num(255.0));
        assert_eq!(lx(&format!("0b{}1s8", "0".repeat(100))), num(1.0));
        assert_eq!(
            lx(&format!("0x{}FFFFFFFFFFFFFFFF", "0".repeat(1000))),
            num(u64::MAX as f64)
        );
        // A million digits, small or too large, in one pass.
        assert_eq!(lx(&format!("0x{}F", "0".repeat(1_000_000))), num(15.0));
        assert_eq!(lx(&format!("0b{}1", "0".repeat(1_000_000))), num(1.0));
        refused(&format!("0x{}", "F".repeat(1_000_000)));
        refused(&format!("0b{}", "1".repeat(1_000_000)));
    }

    /// Each malformed form is `invalid number` naming the whole literal as
    /// written, at its line: no digit after the prefix, a digit outside the
    /// base, a letter that begins no suffix or a suffix not among the eight,
    /// and a value its type cannot hold.
    #[test]
    fn a_malformed_literal_is_an_invalid_number() {
        for src in [
            "0x",
            "0X",
            "0b",
            "0B",
            "0xu8",
            "0bs8",
            "0x_1",
            "0xG",
            "0b102",
            "0b2",
            "0b1F",
            "0b1e3",
            "0x1Fz",
            "0x1Fu9",
            "0x1Fi",
            "0x1Fj",
            "0x1u",
            "0x1s",
            "0x1U8",
            "0x1S8",
            "0x1u08",
            "0x1u128",
            "0x1s7",
            "0x1u8x",
            "0x1s64_",
            "0x1Fu8u8",
            "0x1_0",
            "0x100u8",
            "0x100s8",
            "0x10000000000000000",
        ] {
            let e = lex(&format!("x = {src};")).unwrap_err();
            assert_eq!(e.msg, format!("invalid number '{src}'"), "{src}");
        }
        // Inside brackets, and after a statement on an earlier line.
        assert_eq!(lex("[1 0b12 3]").unwrap_err().msg, "invalid number '0b12'");
        let e = scan("disp(1)\n\nx = 0b2").unwrap_err();
        assert_eq!((e.msg.as_str(), e.line), ("invalid number '0b2'", Some(3)));
        let e = scan("disp(1)\nx = 0x100u8").unwrap_err();
        assert_eq!(
            (e.msg.as_str(), e.line),
            ("invalid number '0x100u8'", Some(2))
        );
        // A tail of a million letters is still the one literal.
        let tail = format!("0x1{}", "z".repeat(1_000_000));
        assert_eq!(
            lex(&tail).unwrap_err().msg,
            format!("invalid number '{tail}'")
        );
        // The run's letters and digits are ASCII, as a name's are: a letter
        // past ASCII ends the literal and is the stray character it is after
        // any number, and a digit past ASCII is no digit of the base.
        assert_eq!(
            lex("x = 0x1F\u{e9}").unwrap_err().msg,
            "unexpected character '\u{e9}'"
        );
        assert_eq!(
            lex("x = 0x\u{ff11}").unwrap_err().msg,
            "invalid number '0x'"
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

    /// Cycle 11: a control character is named by its code point, never
    /// written raw into the message; a printable one is quoted as itself.
    #[test]
    fn an_unexpected_control_character_is_named_not_echoed() {
        let msg = |src: &str| lex(src).unwrap_err().msg;
        assert_eq!(msg("x = \u{0}"), "unexpected character U+0000");
        assert_eq!(msg("\u{7}"), "unexpected character U+0007");
        assert_eq!(msg("x\u{1B}"), "unexpected character U+001B");
        assert_eq!(msg("\u{200B}"), "unexpected character U+200B");
        assert_eq!(msg("$"), "unexpected character '$'");
        assert_eq!(msg("é"), "unexpected character 'é'");
        assert!(!msg("\u{0}").contains('\u{0}'));
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
        assert_eq!(lx("x.'"), vec![id("x"), Token::DotTranspose, Token::Eof]);
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
        assert_eq!(lx("s.'"), vec![id("s"), Token::DotTranspose, Token::Eof]);
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

    // ---- function handles (cycle 06) ---------------------------------

    /// `@(x) x + 1` as the lexer gives it anywhere: no separator inside.
    fn anon_x_plus_1() -> Vec<Token> {
        vec![
            Token::At,
            Token::LParen,
            id("x"),
            Token::RParen,
            id("x"),
            Token::Plus,
            Token::Num(1.0),
        ]
    }

    /// Acceptance test 12, the lexer half: inside a brace or a bracket, the
    /// whitespace of an anonymous function's body separates nothing, and
    /// the comma after it is the one separator between the two elements.
    #[test]
    fn an_anonymous_body_in_braces_or_brackets_is_one_element() {
        for (src, open, close) in [
            ("{@(x) x + 1, 2}", Token::LBrace, Token::RBrace),
            ("[@(x) x + 1, 2]", Token::LBracket, Token::RBracket),
            ("case {@(x) x + 1, 2}", Token::LBrace, Token::RBrace),
        ] {
            let toks = lx(src);
            let toks = &toks[toks.len() - 12..];
            let mut want = vec![open];
            want.extend(anon_x_plus_1());
            want.extend([Token::Comma, Token::Num(2.0), close, Token::Eof]);
            assert_eq!(toks, want, "{src}");
        }
        // Without spaces around the operator, the same.
        let mut want = vec![Token::LBracket];
        want.extend(anon_x_plus_1());
        want.extend([Token::RBracket, Token::Eof]);
        assert_eq!(lx("[@(x) x+1]"), want);
        // A space inside the body separates nothing: `x 1` stays in it,
        // for the parser to refuse.
        assert!(!lx("[@(x) x 1]").contains(&Token::Comma));
        // Inside parentheses within the body, a comma is the call's.
        assert_eq!(
            lx("[@(x) f(x, 1) 2]")
                .iter()
                .filter(|t| **t == Token::Comma)
                .count(),
            1
        );
    }

    /// The body ends where its element does, so what follows is lexed by
    /// the bracket's own rules again.
    #[test]
    fn an_anonymous_body_ends_with_its_element() {
        assert_eq!(
            lx("[@(x) x; 1 -2]")[6..],
            [
                Token::Semi,
                Token::Num(1.0),
                Token::Comma,
                Token::Minus,
                Token::Num(2.0),
                Token::RBracket,
                Token::Eof
            ]
        );
        // A newline in a bracket is still a row break after a body.
        assert_eq!(lx("[@(x) x\n2]")[6], Token::Semi);
        // A named handle is an element like any other.
        assert_eq!(
            lx("[a @f]"),
            vec![
                Token::LBracket,
                id("a"),
                Token::Comma,
                Token::At,
                id("f"),
                Token::RBracket,
                Token::Eof
            ]
        );
        // Outside every bracket nothing changes.
        let mut want = anon_x_plus_1();
        want.push(Token::Eof);
        assert_eq!(lx("@(x) x + 1"), want);
        // A parameter list's own whitespace separates nothing either.
        assert!(
            !lx("[@(a, b) a]")
                .windows(2)
                .any(|w| w == [Token::Comma, Token::Comma])
        );
    }

    /// After a parameter list's `)`, a quote opens a string; after any
    /// other `)` it is still a transpose.
    #[test]
    fn a_quote_after_a_parameter_list_is_a_string() {
        assert_eq!(
            lx("@() 'hi'"),
            vec![
                Token::At,
                Token::LParen,
                Token::RParen,
                st("hi"),
                Token::Eof
            ]
        );
        assert_eq!(lx("(x)'")[3], Token::Transpose);
        assert_eq!(lx("@(x) (x)'")[7], Token::Transpose);
    }

    // ---- cell literals (cycle 07) ------------------------------------

    /// A brace that does not follow the end of a value opens a cell
    /// literal, where whitespace separates elements and a newline starts a
    /// row, as in a bracket; a brace after a value indexes, and whitespace
    /// inside it separates nothing.
    #[test]
    fn a_cell_literal_brace_separates_like_a_bracket() {
        let n = Token::Num;
        assert_eq!(
            lx("{1 -2}"),
            vec![
                Token::LBrace,
                n(1.0),
                Token::Comma,
                Token::Minus,
                n(2.0),
                Token::RBrace,
                Token::Eof
            ]
        );
        assert_eq!(
            lx("c{1 -2}"),
            vec![
                id("c"),
                Token::LBrace,
                n(1.0),
                Token::Minus,
                n(2.0),
                Token::RBrace,
                Token::Eof
            ]
        );
        assert_eq!(
            lx("{1\n2}"),
            vec![
                Token::LBrace,
                n(1.0),
                Token::Semi,
                n(2.0),
                Token::RBrace,
                Token::Eof
            ]
        );
        // After `=`, a comma or another brace, a brace is a literal too.
        assert_eq!(
            lx("x = {{1} 2}"),
            vec![
                id("x"),
                Token::Assign,
                Token::LBrace,
                Token::LBrace,
                n(1.0),
                Token::RBrace,
                Token::Comma,
                n(2.0),
                Token::RBrace,
                Token::Eof
            ]
        );
        // A brace index after a closing brace: `c{1}{2}`.
        assert_eq!(
            lx("c{1}{2 -1}"),
            vec![
                id("c"),
                Token::LBrace,
                n(1.0),
                Token::RBrace,
                Token::LBrace,
                n(2.0),
                Token::Minus,
                n(1.0),
                Token::RBrace,
                Token::Eof
            ]
        );
    }

    /// Cycle 13b: a UTF-16 file is decoded as UTF-16, with a mark or by the
    /// zero-byte pattern of ASCII text, and anything else as lenient UTF-8.
    #[test]
    fn decode_source_reads_utf16_and_leaves_utf8_alone() {
        let le: Vec<u8> = "disp(7)\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let be: Vec<u8> = "disp(7)\n"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        let with = |mark: &[u8], body: &[u8]| [mark, body].concat();
        assert_eq!(decode_source(&with(&[0xFF, 0xFE], &le)), "disp(7)\n");
        assert_eq!(decode_source(&with(&[0xFE, 0xFF], &be)), "disp(7)\n");
        assert_eq!(decode_source(&le), "disp(7)\n");
        assert_eq!(decode_source(&be), "disp(7)\n");
        // A character outside the BMP is a surrogate pair, and survives.
        let smile: Vec<u8> = "\u{1F600}"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(decode_source(&with(&[0xFF, 0xFE], &smile)), "\u{1F600}");
        // An odd byte out and a lone surrogate are U+FFFD, never a refusal.
        assert_eq!(decode_source(&[0xFF, 0xFE, b'a', 0, b'b']), "a\u{FFFD}");
        assert_eq!(
            decode_source(&[0xFF, 0xFE, 0x00, 0xD8, b'a', 0]),
            "\u{FFFD}a"
        );
        assert_eq!(decode_source(&[0xFF, 0xFE]), "");
        // UTF-8 is read as it always was: the UTF-8 mark stays for the
        // lexer to skip, a stray byte is U+FFFD, and NULs that do not make
        // the UTF-16 pattern stay NULs.
        assert_eq!(decode_source(b"\xEF\xBB\xBFx"), "\u{FEFF}x");
        assert_eq!(decode_source(b"caf\xE9"), "caf\u{FFFD}");
        assert_eq!(decode_source(b"a\0b\0cd"), "a\0b\0cd");
        assert_eq!(decode_source(b"disp(7)\0\n"), "disp(7)\0\n");
        assert_eq!(decode_source(b"\0\0\0\0"), "\0\0\0\0");
        assert_eq!(decode_source(b"1"), "1");
        assert_eq!(decode_source(b""), "");
        // Only the first 64 bytes are judged: a later non-ASCII character
        // does not stop the file being UTF-16.
        let long = format!("{}\u{4E2D}", "a".repeat(40));
        let long16: Vec<u8> = long.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_source(&long16), long);
    }
}
