//! JSON, hand-written: a value, a parser and a writer.
//!
//! The crate takes no dependencies, so the protocol of cycle U0 and the
//! server of cycle U1 share this one small module rather than a JSON crate.
//! It is the whole of RFC 8259 and no more: no comments, no trailing commas,
//! no `NaN`.
//!
//! An object keeps its keys in the order they were written, as a `Vec` of
//! pairs rather than a map. Key order is part of the protocol, so a response
//! built here is written byte for byte as it was built, and golden output is
//! deterministic.
//!
//! The parser recurses once per array or object level, so it counts levels
//! against [`MAX_DEPTH`] and refuses anything deeper (invariant 6): a line of
//! 100,000 `[` is a [`ParseError::TooDeep`], never a stack overflow.

use std::fmt;

/// How many arrays or objects may nest inside one another. A request is a
/// flat object, so the bound is far above anything the protocol sends and far
/// below anything that could trouble a stack: 128, the default of the widely
/// used Rust and Python parsers. A document nested exactly this deep parses;
/// one level more is refused.
pub const MAX_DEPTH: usize = 128;

/// A JSON value. `Object` keeps its keys in written order, duplicates
/// included; [`Json::get`] reads the last of a duplicated key, as a browser's
/// `JSON.parse` does.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Why a text is not a JSON value this parser accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Not JSON: a syntax error at this byte offset, trailing garbage
    /// included.
    Syntax(usize),
    /// Arrays or objects nested more than [`MAX_DEPTH`] levels.
    TooDeep,
}

impl Json {
    /// The value of `key` in an object, the last one when it is repeated, or
    /// `None` for a missing key or a value that is not an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The text of a string value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    /// An object from `(key, value)` pairs, in the order given.
    pub fn object<const N: usize>(pairs: [(&str, Json); N]) -> Json {
        Json::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// Appends this value's JSON text to `out`: no whitespace, keys in order,
    /// strings escaped by [`write_string`].
    pub fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => write_number(out, *n),
            Json::String(s) => write_string(out, s),
            Json::Array(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Object(pairs) => {
                out.push('{');
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::new();
        self.write(&mut s);
        f.write_str(&s)
    }
}

/// A number as JSON text. Rust's shortest round-trip form already writes an
/// integer with no decimal point (`3`, not `3.0`) and never uses an exponent,
/// so ids and sizes read naturally and every finite value is valid JSON.
/// JSON has no `NaN` or `Inf`; they are written as `null`, as `JSON.stringify`
/// writes them.
fn write_number(out: &mut String, n: f64) {
    if n.is_finite() {
        out.push_str(&n.to_string());
    } else {
        out.push_str("null");
    }
}

/// Appends `s` as a quoted JSON string. `"` `\` newline, carriage return and
/// tab take their short escapes; every other control character below U+0020
/// is `\u00xx` in lower-case hex, as `JSON.stringify` writes it. Everything
/// else, `×` included, is raw UTF-8.
pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let v = c as usize;
                out.push_str("\\u00");
                out.push(HEX[v >> 4] as char);
                out.push(HEX[v & 0xF] as char);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses one JSON value, surrounded by nothing but whitespace.
pub fn parse(src: &str) -> Result<Json, ParseError> {
    let mut p = Parser {
        src,
        b: src.as_bytes(),
        pos: 0,
        depth: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.pos != p.b.len() {
        return Err(ParseError::Syntax(p.pos));
    }
    Ok(v)
}

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    /// Arrays and objects open around the current position.
    depth: usize,
}

impl Parser<'_> {
    fn err<T>(&self) -> Result<T, ParseError> {
        Err(ParseError::Syntax(self.pos))
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    /// Consumes `lit` if the input continues with it.
    fn eat(&mut self, lit: &str) -> bool {
        if self.b[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<Json, ParseError> {
        match self.peek() {
            Some(b'{') => self.nested(Parser::object),
            Some(b'[') => self.nested(Parser::array),
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ if self.eat("null") => Ok(Json::Null),
            _ if self.eat("true") => Ok(Json::Bool(true)),
            _ if self.eat("false") => Ok(Json::Bool(false)),
            _ => self.err(),
        }
    }

    /// Runs `body` one level deeper, refusing to go past [`MAX_DEPTH`].
    fn nested(
        &mut self,
        body: fn(&mut Self) -> Result<Json, ParseError>,
    ) -> Result<Json, ParseError> {
        if self.depth == MAX_DEPTH {
            return Err(ParseError::TooDeep);
        }
        self.depth += 1;
        let v = body(self);
        self.depth -= 1;
        v
    }

    fn array(&mut self) -> Result<Json, ParseError> {
        self.pos += 1; // `[`
        let mut items = Vec::new();
        self.ws();
        if self.eat("]") {
            return Ok(Json::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("]") {
                return Ok(Json::Array(items));
            }
            return self.err();
        }
    }

    fn object(&mut self) -> Result<Json, ParseError> {
        self.pos += 1; // `{`
        let mut pairs = Vec::new();
        self.ws();
        if self.eat("}") {
            return Ok(Json::Object(pairs));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return self.err();
            }
            let k = self.string()?;
            self.ws();
            if !self.eat(":") {
                return self.err();
            }
            self.ws();
            let v = self.value()?;
            pairs.push((k, v));
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("}") {
                return Ok(Json::Object(pairs));
            }
            return self.err();
        }
    }

    /// `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`, exactly.
    fn number(&mut self) -> Result<Json, ParseError> {
        let start = self.pos;
        self.eat("-");
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return self.err(),
        }
        if self.eat(".") {
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return self.err();
            }
            self.digits();
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return self.err();
            }
            self.digits();
        }
        // The grammar above is a subset of what `f64`'s parser reads, so this
        // cannot fail; a huge exponent reads as an infinity, a tiny one as 0.
        match self.src[start..self.pos].parse::<f64>() {
            Ok(n) => Ok(Json::Number(n)),
            Err(_) => Err(ParseError::Syntax(start)),
        }
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
    }

    /// A string, from its opening quote to past its closing one.
    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // `"`
        let mut s = String::new();
        loop {
            // Copy the run of plain characters in one go. The input is a
            // `&str` and the run stops only at ASCII bytes, so it is whole
            // UTF-8 characters.
            let run = self.pos;
            while let Some(c) = self.peek() {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            s.push_str(&self.src[run..self.pos]);
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(s);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    self.escape(&mut s)?;
                }
                // End of input, or a raw control character, which JSON
                // requires to be escaped.
                _ => return self.err(),
            }
        }
    }

    /// One escape, after its backslash.
    fn escape(&mut self, s: &mut String) -> Result<(), ParseError> {
        let c = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.pos += 1;
                let hi = self.hex4()?;
                let c = if (0xD800..0xDC00).contains(&hi) {
                    // A high surrogate: with a low one after it, the pair is
                    // one character outside the Basic Multilingual Plane.
                    let save = self.pos;
                    if self.eat("\\u") {
                        let lo = self.hex4()?;
                        if (0xDC00..0xE000).contains(&lo) {
                            let v = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                            char::from_u32(v).unwrap_or('\u{FFFD}')
                        } else {
                            // Not a low surrogate: leave that escape to be
                            // read on its own.
                            self.pos = save;
                            '\u{FFFD}'
                        }
                    } else {
                        '\u{FFFD}'
                    }
                } else {
                    // A lone low surrogate has no Rust `char`; like a lone
                    // high one it becomes U+FFFD, the lenient reading that
                    // script mode gives an undecodable byte.
                    char::from_u32(hi).unwrap_or('\u{FFFD}')
                };
                s.push(c);
                return Ok(());
            }
            _ => return self.err(),
        };
        self.pos += 1;
        s.push(c);
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let mut v = 0;
        for _ in 0..4 {
            let d = match self.peek() {
                Some(c @ b'0'..=b'9') => c - b'0',
                Some(c @ b'a'..=b'f') => c - b'a' + 10,
                Some(c @ b'A'..=b'F') => c - b'A' + 10,
                _ => return self.err(),
            };
            v = v * 16 + u32::from(d);
            self.pos += 1;
        }
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(v: &Json) -> String {
        v.to_string()
    }

    fn round_trip(src: &str) {
        let v = parse(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
        assert_eq!(text(&v), src, "written form of {src}");
        assert_eq!(parse(&text(&v)).unwrap(), v);
    }

    #[test]
    fn every_kind_round_trips() {
        for src in [
            "null",
            "true",
            "false",
            "0",
            "-3",
            "1.5",
            "-0.25",
            "\"\"",
            "\"abc\"",
            "[]",
            "[1,[2,[]],\"x\",null]",
            "{}",
            "{\"id\":1,\"op\":\"eval\",\"code\":\"x = 1\"}",
            "{\"a\":{\"b\":[true,false]},\"c\":null}",
        ] {
            round_trip(src);
        }
    }

    #[test]
    fn whitespace_is_allowed_between_tokens_and_dropped_on_writing() {
        let v = parse(" { \"a\" : [ 1 , 2 ] ,\t\"b\":\r\n\"c\" } ").unwrap();
        assert_eq!(text(&v), "{\"a\":[1,2],\"b\":\"c\"}");
    }

    #[test]
    fn keys_keep_their_order_and_the_last_duplicate_wins() {
        let v = parse("{\"z\":1,\"a\":2,\"z\":3}").unwrap();
        assert_eq!(text(&v), "{\"z\":1,\"a\":2,\"z\":3}");
        assert_eq!(v.get("z"), Some(&Json::Number(3.0)));
        assert_eq!(v.get("a"), Some(&Json::Number(2.0)));
        assert_eq!(v.get("q"), None);
        assert_eq!(Json::Null.get("a"), None);
    }

    #[test]
    fn numbers_parse_by_the_json_grammar() {
        assert_eq!(parse("1e3").unwrap(), Json::Number(1000.0));
        assert_eq!(parse("-2.5E-1").unwrap(), Json::Number(-0.25));
        assert_eq!(parse("1e+2").unwrap(), Json::Number(100.0));
        for bad in [
            "01", "-", "1.", ".5", "+1", "1e", "1e+", "0x10", "NaN", "Infinity",
        ] {
            assert!(parse(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn integers_are_written_without_a_decimal_point() {
        assert_eq!(text(&Json::Number(3.0)), "3");
        assert_eq!(text(&Json::Number(-12.0)), "-12");
        assert_eq!(text(&Json::Number(0.5)), "0.5");
        assert_eq!(text(&Json::Number(1e21)), "1000000000000000000000");
        assert_eq!(text(&Json::Number(f64::NAN)), "null");
        assert_eq!(text(&Json::Number(f64::INFINITY)), "null");
    }

    #[test]
    fn strings_are_escaped_exactly_as_the_protocol_specifies() {
        let s = |x: &str| text(&Json::String(x.to_string()));
        assert_eq!(s("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(s("x\ty\n"), "\"x\\ty\\n\"");
        assert_eq!(s("\r"), "\"\\r\"");
        assert_eq!(s("\u{1B}[2J"), "\"\\u001b[2J\"");
        assert_eq!(s("\u{0}\u{1f}"), "\"\\u0000\\u001f\"");
        // Everything else is raw UTF-8, `×` and `/` included.
        assert_eq!(s("1×2 / é 😀\u{7f}"), "\"1×2 / é 😀\u{7f}\"");
    }

    #[test]
    fn every_escape_is_read() {
        let v = parse(r#""\"\\\/\b\f\n\r\t\u0041\u00e9\u00D7""#).unwrap();
        assert_eq!(v, Json::String("\"\\/\u{8}\u{c}\n\r\t\u{41}é×".into()));
        // What the writer escapes, the parser reads back.
        let all: String = (0u32..0x80).filter_map(char::from_u32).collect();
        let written = text(&Json::String(all.clone()));
        assert_eq!(parse(&written).unwrap(), Json::String(all));
    }

    #[test]
    fn a_surrogate_pair_decodes_to_one_character() {
        let v = parse(r#""\ud83d\ude00""#).unwrap();
        assert_eq!(v, Json::String("😀".into()));
        assert_eq!(v.as_str().unwrap().chars().count(), 1);
        assert_eq!(
            parse(r#""\uD834\uDD1E!""#).unwrap(),
            Json::String("𝄞!".into())
        );
    }

    #[test]
    fn a_lone_surrogate_becomes_the_replacement_character() {
        assert_eq!(
            parse(r#""\ud800x""#).unwrap(),
            Json::String("\u{FFFD}x".into())
        );
        assert_eq!(
            parse(r#""\udc00""#).unwrap(),
            Json::String("\u{FFFD}".into())
        );
        // A high surrogate followed by an escape that is not a low one keeps
        // that escape.
        assert_eq!(
            parse(r#""\ud800\u0041""#).unwrap(),
            Json::String("\u{FFFD}A".into())
        );
    }

    #[test]
    fn malformed_strings_are_refused() {
        for bad in [
            "\"abc",
            "\"a\\qb\"",
            "\"\\u12\"",
            "\"\\u12g4\"",
            "\"tab\there\"",
            "\"line\nbreak\"",
            "'single'",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn trailing_garbage_is_refused() {
        assert_eq!(parse("{} x"), Err(ParseError::Syntax(3)));
        assert_eq!(parse("1 2"), Err(ParseError::Syntax(2)));
        assert!(parse("[1]]").is_err());
        assert!(parse("nulls").is_err());
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("{\"a\":1,}").is_err());
        assert!(parse("{\"a\" 1}").is_err());
        assert!(parse("{1:2}").is_err());
    }

    #[test]
    fn the_depth_limit_holds_at_and_past_its_bound() {
        let arrays = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
        let objects = |n: usize| {
            let mut s = String::new();
            for _ in 1..n {
                s.push_str("{\"a\":");
            }
            s.push_str("{}");
            s.push_str(&"}".repeat(n - 1));
            s
        };
        assert!(parse(&arrays(MAX_DEPTH)).is_ok());
        assert_eq!(parse(&arrays(MAX_DEPTH + 1)), Err(ParseError::TooDeep));
        assert!(parse(&objects(MAX_DEPTH)).is_ok());
        assert_eq!(parse(&objects(MAX_DEPTH + 1)), Err(ParseError::TooDeep));
        // Far past it, and unclosed: refused at the bound, never recursed
        // into, so the test thread's small stack is never at risk.
        assert_eq!(parse(&"[".repeat(100_000)), Err(ParseError::TooDeep));
        // A value nested at the limit writes back out unchanged.
        let deep = arrays(MAX_DEPTH);
        assert_eq!(text(&parse(&deep).unwrap()), deep);
    }
}
