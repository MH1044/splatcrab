//! The `printf` family's formatter: what `fprintf`, `sprintf`, `num2str`
//! with a format, `error`, `warning` and `assert` share.
//!
//! It moved here from `core.rs` in cycle 11, which took on QA D16, the
//! list of ways it spelled a conversion differently from MATLAB: `%E` and
//! `%G` now print an upper-case `E`, `%s` of a non-integer falls back to
//! `%e` as the MATLAB `sprintf` page's own example does, the `#` flag
//! works, the `0` flag pads a non-finite value with spaces, the escapes
//! `\xN`, `\N` (octal), `\a`, `\b`, `\f` and `\v` are processed, `%x`,
//! `%X`, `%o` and a `*` width or precision exist, and an invalid
//! conversion ends the output rather than raising an error, since MATLAB
//! "prints all text up to the invalid operator ... and discards the rest".
//!
//! Every width and precision is still bounded by [`MAX_FIELD`], cycle
//! 01d's rule, `*` ones included: the panics that bound prevents are in
//! Rust's formatter and do not go away.

use super::args::{MAX_ELEMS, check_shape};
use crate::error::{self, R};
use crate::interp::{fmt_e, fmt_g};
use crate::value::{Value, decode_units, nonfinite};

/// One flattened `printf` argument.
#[derive(Clone, Copy)]
enum PArg {
    Num(f64),
    /// One UTF-16 code unit of a char argument, tagged with the argument it
    /// came from. MATLAB expands a char array to one argument per element, so
    /// `fprintf('%d %d', 'AB')` prints `65 66`; the tag is what lets a later
    /// `%s` put the run back together and print `AB`, decoding a surrogate
    /// pair back into its one character.
    Chr(f64, usize),
}

impl PArg {
    fn value(self) -> f64 {
        match self {
            PArg::Num(v) | PArg::Chr(v, _) => v,
        }
    }
}

fn char_of(v: f64) -> char {
    if v.is_finite() && (0.0..=f64::from(char::MAX as u32)).contains(&v) {
        char::from_u32(v as u32).unwrap_or('?')
    } else {
        '?'
    }
}

/// 2^63: the first magnitude an `i64` cannot hold.
const I64_LIMIT: f64 = 9_223_372_036_854_775_808.0;

/// 2^64: the first magnitude a `u64` cannot hold, past which `%x` and `%o`
/// fall back to `%e`.
const U64_LIMIT: f64 = 18_446_744_073_709_551_616.0;

/// `%d`, `%i` and `%u`. Precision zero-pads the digits, as in C.
///
/// An integral value at or above `2^63` prints the value itself, from
/// `{:.0}`, the exact decimal expansion of the double: `1e30` prints as
/// `1000000000000000019884624838656`, which is what C's `%.0f` gives for
/// the same bits. A value that is not an integer switches to `%e`, as
/// MATLAB switches it.
fn int_body(v: f64, prec: Option<usize>) -> String {
    if !v.is_finite() {
        return nonfinite(v);
    }
    if v.fract() != 0.0 {
        return fmt_e(v, prec.unwrap_or(6));
    }
    let magnitude = v.abs();
    let digits = if magnitude < I64_LIMIT {
        format!("{}", magnitude as u64)
    } else {
        format!("{:.0}", magnitude)
    };
    let pad = prec.unwrap_or(0).saturating_sub(digits.len());
    format!(
        "{}{}{}",
        if v < 0.0 { "-" } else { "" },
        "0".repeat(pad),
        digits
    )
}

/// `%x`, `%X` and `%o`: an integer in base 16 or 8, a minus sign before a
/// negative one's magnitude, and precision zero-padding the digits as for
/// `%d`. A value that is not an integer, or one past `2^64`, switches to
/// `%e` as `%d` does; a non-finite one is `Inf` or `NaN`. With `#`, a
/// non-zero hexadecimal value gets its `0x` or `0X` and an octal one a
/// leading `0`.
fn radix_body(v: f64, conv: char, prec: Option<usize>, alt: bool) -> (String, String) {
    if !v.is_finite() {
        return (String::new(), nonfinite(v));
    }
    if v.fract() != 0.0 || v.abs() >= U64_LIMIT {
        return (String::new(), fmt_e(v, prec.unwrap_or(6)));
    }
    let m = v.abs() as u64;
    let mut digits = match conv {
        'x' => format!("{:x}", m),
        'X' => format!("{:X}", m),
        _ => format!("{:o}", m),
    };
    let pad = prec.unwrap_or(0).saturating_sub(digits.len());
    digits.insert_str(0, &"0".repeat(pad));
    let mut prefix = String::from(if v < 0.0 { "-" } else { "" });
    if alt && m != 0 {
        match conv {
            'x' => prefix.push_str("0x"),
            'X' => prefix.push_str("0X"),
            _ if !digits.starts_with('0') => digits.insert(0, '0'),
            _ => {}
        }
    }
    (prefix, digits)
}

/// The character a number denotes, when it denotes one at all.
fn char_code(v: f64) -> Option<char> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..=f64::from(char::MAX as u32)).contains(&v) {
        char::from_u32(v as u32)
    } else {
        None
    }
}

/// `%s` given a number: the character with that code, and when the value
/// is not a character code, the number itself: `%e` for a value that is
/// not an integer, which is the MATLAB `sprintf` page's `3.141593e+00` for
/// `sprintf('%s', pi)`, and `%g` for an integer that names no character.
///
/// Precision truncates a character as it truncates a char argument, and is
/// the number's precision on the two fallbacks.
fn str_body(v: f64, prec: Option<usize>) -> String {
    match char_code(v) {
        Some(c) => truncate(&c.to_string(), prec),
        None if v.is_finite() && v.fract() != 0.0 => fmt_e(v, prec.unwrap_or(6)),
        None => fmt_g(v, prec.unwrap_or(6)),
    }
}

/// `%.Ns`: C and MATLAB cut a string to `N` characters, and do it before the
/// field width pads, so `sprintf('[%5.2s]', 'abcdef')` is `[   ab]`.
fn truncate(s: &str, prec: Option<usize>) -> String {
    match prec {
        Some(n) => s.chars().take(n).collect(),
        None => s.to_string(),
    }
}

/// `%#g`: `%g` that keeps its trailing zeros and its decimal point.
fn fmt_g_alt(v: f64, prec: usize) -> String {
    if !v.is_finite() {
        return nonfinite(v);
    }
    let p = prec.max(1);
    let s = format!("{:.*e}", p - 1, v);
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = e.parse().unwrap_or(0);
    let with_point = |s: String| if s.contains('.') { s } else { s + "." };
    if v != 0.0 && (exp < -4 || exp >= p as i32) {
        format!(
            "{}e{}{:02}",
            with_point(m.to_string()),
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        )
    } else {
        let exp = if v == 0.0 { 0 } else { exp };
        let dec = (p as i32 - 1 - exp).max(0) as usize;
        with_point(format!("{:.*}", dec, v))
    }
}

/// `%e` and `%E` with the `#` flag's decimal point kept at precision 0.
fn fmt_e_flags(v: f64, prec: usize, alt: bool, upper: bool) -> String {
    let mut s = fmt_e(v, prec);
    if !v.is_finite() {
        return s;
    }
    if alt && prec == 0 {
        if let Some(k) = s.find('e') {
            s.insert(k, '.');
        }
    }
    if upper { s.replace('e', "E") } else { s }
}

/// The largest width or precision any conversion will accept.
///
/// Both numbers reach an allocation or a Rust formatter, and both used to be
/// unbounded. Rust holds a formatter's precision in a `u16`, so `%.65536f`
/// panicked with "Formatting argument out of range" and `%.65535e` tripped its
/// own `ndigits > 0` assertion one below that; `%2147483647d` built a
/// two-gigabyte pad and aborted in the allocator. Every one of them is a
/// single line of user input that killed the REPL.
///
/// 8192 is chosen to be far beyond any honest format and far below every
/// limit above. A double's longest exact decimal expansion is 1074 fractional
/// digits, so `%.8192f` still prints every value in full, and the widest
/// field that could be meant for a terminal is two orders of magnitude
/// narrower. It bounds every conversion, including the fallbacks `%d` and
/// `%s` take to `fmt_e` and `fmt_g` for a value that is not an integer,
/// which is how `%.65536d` reached the same panic, and since cycle 11 a
/// width or precision given by `*`.
pub const MAX_FIELD: usize = 8192;

/// One width or precision from a format specifier, bounded. An empty run of
/// digits is `absent` (`%5d` has no precision and `%.f` means `%.0f`), and
/// anything that does not fit is the same clean error as an absurd one.
fn field(digits: &str, absent: usize) -> R<usize> {
    if digits.is_empty() {
        return Ok(absent);
    }
    match digits.parse::<usize>() {
        Ok(n) if n <= MAX_FIELD => Ok(n),
        _ => Err(error::format_field_too_large(MAX_FIELD)),
    }
}

/// A `*` width or precision's value, bounded like a written one: its
/// magnitude, and whether it was negative. `None` for a value that is not
/// a finite number, which ends the output as an invalid conversion does.
fn star_field(v: f64) -> R<Option<(usize, bool)>> {
    if !v.is_finite() {
        return Ok(None);
    }
    let m = v.trunc().abs();
    if m > MAX_FIELD as f64 {
        return Err(error::format_field_too_large(MAX_FIELD));
    }
    Ok(Some((m as usize, v < 0.0)))
}

/// The escape after a backslash at `chars[i]`, the character after the
/// backslash: the text it stands for and the index after it. `\n`, `\t`,
/// `\r`, `\a`, `\b`, `\f`, `\v`, `\\`, `\xN` (hexadecimal digits, as many
/// as follow) and `\N` (one to three octal digits); any other character
/// keeps its backslash, as it always has.
fn escape(chars: &[char], i: usize) -> (String, usize) {
    let simple = |c: char| (c.to_string(), i + 1);
    match chars[i] {
        'n' => simple('\n'),
        't' => simple('\t'),
        'r' => simple('\r'),
        'a' => simple('\u{7}'),
        'b' => simple('\u{8}'),
        'f' => simple('\u{C}'),
        'v' => simple('\u{B}'),
        '\\' => simple('\\'),
        'x' if chars.get(i + 1).is_some_and(char::is_ascii_hexdigit) => {
            let mut j = i + 1;
            let mut v: u32 = 0;
            while let Some(d) = chars.get(j).and_then(|c| c.to_digit(16)) {
                v = v.saturating_mul(16).saturating_add(d);
                j += 1;
            }
            (char::from_u32(v).unwrap_or('\u{FFFD}').to_string(), j)
        }
        '0'..='7' => {
            let mut j = i;
            let mut v: u32 = 0;
            while j < i + 3 {
                match chars.get(j).and_then(|c| c.to_digit(8)) {
                    Some(d) => v = v * 8 + d,
                    None => break,
                }
                j += 1;
            }
            (char::from_u32(v).unwrap_or('\u{FFFD}').to_string(), j)
        }
        other => (format!("\\{}", other), i + 1),
    }
}

/// The escapes of `text` processed the way a format's are, and nothing
/// else: what `strsplit`, `strjoin` and `regexprep` do to the text they are
/// given.
pub fn escapes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            let (s, next) = escape(&chars, i + 1);
            out.push_str(&s);
            i = next;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Shared implementation of fprintf / sprintf. Cycles the format over the
/// flattened arguments like MATLAB does.
pub fn format_printf(args: &[Value]) -> R<String> {
    let fmt = match args.first().and_then(Value::text) {
        Some(s) => s,
        None => return Err(error::format_not_a_string()),
    };
    let mut flat: Vec<PArg> = Vec::new();
    for (group, a) in args[1..].iter().enumerate() {
        let m = a.mat()?;
        if m.is_char() {
            flat.extend(m.data.iter().map(|v| PArg::Chr(*v, group)));
        } else {
            flat.extend(m.data.iter().map(|v| PArg::Num(*v)));
        }
    }
    let has_args = !flat.is_empty();
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut ai = 0;
    loop {
        let mut i = 0;
        let mut specs_in_pass = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\\' && i + 1 < chars.len() {
                let (s, next) = escape(&chars, i + 1);
                out.push_str(&s);
                i = next;
                continue;
            }
            if c != '%' {
                out.push(c);
                i += 1;
                continue;
            }
            i += 1;
            if i < chars.len() && chars[i] == '%' {
                out.push('%');
                i += 1;
                continue;
            }
            let mut flags = String::new();
            while i < chars.len() && "-+ 0#".contains(chars[i]) {
                flags.push(chars[i]);
                i += 1;
            }
            // The width: digits, or `*` for the next argument.
            let width = if i < chars.len() && chars[i] == '*' {
                i += 1;
                if !has_args || ai >= flat.len() {
                    return Ok(out);
                }
                ai += 1;
                match star_field(flat[ai - 1].value())? {
                    Some((w, negative)) => {
                        if negative {
                            flags.push('-');
                        }
                        w
                    }
                    None => return Ok(out),
                }
            } else {
                let mut digits = String::new();
                while i < chars.len() && chars[i].is_ascii_digit() {
                    digits.push(chars[i]);
                    i += 1;
                }
                field(&digits, 0)?
            };
            let mut prec: Option<usize> = None;
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                if i < chars.len() && chars[i] == '*' {
                    i += 1;
                    if !has_args || ai >= flat.len() {
                        return Ok(out);
                    }
                    ai += 1;
                    match star_field(flat[ai - 1].value())? {
                        // A negative precision is taken as absent, as in C.
                        Some((p, negative)) => prec = (!negative).then_some(p),
                        None => return Ok(out),
                    }
                } else {
                    let mut p = String::new();
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        p.push(chars[i]);
                        i += 1;
                    }
                    prec = Some(field(&p, 0)?);
                }
            }
            // C's length modifiers, which MATLAB accepts before an integer
            // conversion and which change nothing here.
            while i < chars.len() && (chars[i] == 'l' || chars[i] == 'h') {
                i += 1;
            }
            // A `%` at the end, or an invalid conversion: MATLAB prints the
            // text up to it and discards the rest.
            let Some(&conv) = chars.get(i) else {
                return Ok(out);
            };
            if !"diufFeEgGxXocs".contains(conv) {
                return Ok(out);
            }
            i += 1;
            specs_in_pass += 1;
            if has_args && ai >= flat.len() {
                // Ran out of data: MATLAB stops here.
                return Ok(out);
            }
            let arg = if has_args {
                ai += 1;
                Some(flat[ai - 1])
            } else {
                None
            };
            let alt = flags.contains('#');
            // `prefix` is what zero padding goes after: a sign, and `0x`.
            // Only a numeric conversion takes a sign flag or zero padding,
            // and zero padding only for a finite value.
            let mut prefix = String::new();
            let mut numeric = true;
            let mut signed = true;
            let mut finite = true;
            let mut zero_ok = true;
            let mut body = match (conv, arg) {
                (_, None) => {
                    numeric = false;
                    String::new()
                }
                ('d' | 'i' | 'u', Some(a)) => {
                    // A precision turns the `0` flag off, as in C.
                    zero_ok = prec.is_none() || a.value().fract() != 0.0;
                    finite = a.value().is_finite();
                    int_body(a.value(), prec)
                }
                ('x' | 'X' | 'o', Some(a)) => {
                    signed = false;
                    zero_ok = prec.is_none() || a.value().fract() != 0.0;
                    finite = a.value().is_finite();
                    let (p, digits) = radix_body(a.value(), conv, prec, alt);
                    prefix = p;
                    digits
                }
                ('f' | 'F', Some(a)) => {
                    let v = a.value();
                    finite = v.is_finite();
                    if finite {
                        let p = prec.unwrap_or(6);
                        let s = format!("{:.*}", p, v);
                        if alt && p == 0 { s + "." } else { s }
                    } else {
                        nonfinite(v)
                    }
                }
                ('e' | 'E', Some(a)) => {
                    finite = a.value().is_finite();
                    fmt_e_flags(a.value(), prec.unwrap_or(6), alt, conv == 'E')
                }
                ('g' | 'G', Some(a)) => {
                    let v = a.value();
                    finite = v.is_finite();
                    let p = prec.unwrap_or(6);
                    let s = if alt { fmt_g_alt(v, p) } else { fmt_g(v, p) };
                    if conv == 'G' && finite {
                        s.replace('e', "E")
                    } else {
                        s
                    }
                }
                ('c', Some(a)) => {
                    numeric = false;
                    char_of(a.value()).to_string()
                }
                ('s', Some(PArg::Chr(v, group))) => {
                    numeric = false;
                    // %s takes the whole char argument, not one element.
                    let mut units = vec![v];
                    while let Some(PArg::Chr(next, g)) = flat.get(ai).copied() {
                        if g != group {
                            break;
                        }
                        units.push(next);
                        ai += 1;
                    }
                    let s = decode_units(units.into_iter());
                    // The whole argument is still consumed; only the text
                    // printed is cut, and it is cut before the width pads.
                    truncate(&s, prec)
                }
                (_, Some(a)) => {
                    // `%s` of a number.
                    numeric = false;
                    str_body(a.value(), prec)
                }
            };
            // The sign of a negative value belongs to the prefix, so that
            // zero padding goes after it.
            if let Some(rest) = body.strip_prefix('-') {
                if prefix.is_empty() {
                    prefix.push('-');
                }
                body = rest.to_string();
            }
            if numeric && signed && !prefix.starts_with('-') {
                if flags.contains('+') {
                    prefix.insert(0, '+');
                } else if flags.contains(' ') {
                    prefix.insert(0, ' ');
                }
            }
            let len = prefix.chars().count() + body.chars().count();
            if len >= width {
                out.push_str(&prefix);
                out.push_str(&body);
            } else if flags.contains('-') {
                out.push_str(&prefix);
                out.push_str(&body);
                out.extend(std::iter::repeat_n(' ', width - len));
            } else if flags.contains('0') && numeric && finite && zero_ok {
                out.push_str(&prefix);
                out.extend(std::iter::repeat_n('0', width - len));
                out.push_str(&body);
            } else {
                out.extend(std::iter::repeat_n(' ', width - len));
                out.push_str(&prefix);
                out.push_str(&body);
            }
            // The format cycles once per argument, so a bounded field over
            // many arguments can still ask for more text than a char array
            // may hold: `sprintf('%8192d', ones(1, 1e6))`. The text is
            // judged as it grows, by its bytes, which are never fewer than
            // its code units.
            if out.len() > MAX_ELEMS {
                check_shape(1.0, out.len() as f64)?;
            }
        }
        if !has_args || ai >= flat.len() || specs_in_pass == 0 {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{Class, Matrix};

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    fn pf(fmt: &str, nums: &[f64]) -> String {
        let mut args = vec![Value::str(fmt)];
        if !nums.is_empty() {
            args.push(Value::Mat(Matrix::row(nums.to_vec())));
        }
        format_printf(&args).unwrap()
    }

    #[test]
    fn printf_honours_the_sign_flags() {
        assert_eq!(pf("[%+d]", &[5.0]), "[+5]");
        assert_eq!(pf("[% d]", &[5.0]), "[ 5]");
        assert_eq!(pf("[%+d]", &[-5.0]), "[-5]");
        assert_eq!(pf("[% d]", &[-5.0]), "[-5]");
        assert_eq!(pf("[%+f]", &[1.5]), "[+1.500000]");
        assert_eq!(pf("[%+g]", &[1.5]), "[+1.5]");
        assert_eq!(pf("[%+e]", &[1.5]), "[+1.500000e+00]");
        // The sign counts towards the field width, and zero padding goes
        // after it.
        assert_eq!(pf("[%+06.1f]", &[1.5]), "[+001.5]");
        // A char conversion takes no sign.
        assert_eq!(pf("[%+c]", &[65.0]), "[A]");
    }

    /// Cycle 01d's acceptance test 18: the bound holds at the boundary and
    /// one past it, for every conversion, including the two fallbacks `%d`
    /// and `%s` take for a value they cannot print as themselves. Each of
    /// these used to panic (exit 101) or build a multi-gigabyte pad (exit
    /// 134). Cycle 11 added `%x`, `%X`, `%o` and the `*` forms.
    #[test]
    fn printf_bounds_its_width_and_precision_for_every_conversion() {
        let too_large = format!(
            "The width or precision in a format specifier must be at most {}.",
            MAX_FIELD
        );
        let try_fmt =
            |fmt: &str, v: f64| format_printf(&[Value::str(fmt), Value::Mat(Matrix::scalar(v))]);
        for (conv, v) in [
            ('d', 1.0),
            ('i', 1.0),
            ('u', 1.0),
            ('f', 1.0),
            ('F', 1.0),
            ('e', 1.0),
            ('E', 1.0),
            ('g', 1.0),
            ('G', 1.0),
            ('x', 255.0),
            ('X', 255.0),
            ('o', 8.0),
            ('c', 65.0),
            ('s', 65.0),
            ('d', std::f64::consts::PI),
            ('s', std::f64::consts::PI),
            ('x', std::f64::consts::PI),
        ] {
            let at = format!("%.{MAX_FIELD}{conv}");
            assert!(try_fmt(&at, v).is_ok(), "precision at the bound: {at}");
            let past = format!("%.{}{}", MAX_FIELD + 1, conv);
            assert_eq!(
                try_fmt(&past, v).unwrap_err().msg,
                too_large,
                "precision past the bound: {past}"
            );
            let at = format!("%{MAX_FIELD}{conv}");
            assert_eq!(
                try_fmt(&at, v).unwrap().chars().count(),
                MAX_FIELD,
                "width at the bound: {at}"
            );
            let past = format!("%{}{}", MAX_FIELD + 1, conv);
            assert_eq!(
                try_fmt(&past, v).unwrap_err().msg,
                too_large,
                "width past the bound: {past}"
            );
        }
        // The three inputs from cycle 01d's spec, by number.
        assert_eq!(try_fmt("%.65536f", 1.0).unwrap_err().msg, too_large);
        assert_eq!(try_fmt("%.65535e", 1.0).unwrap_err().msg, too_large);
        assert_eq!(try_fmt("%2147483647d", 1.0).unwrap_err().msg, too_large);
        assert_eq!(
            try_fmt("%99999999999999999999d", 1.0).unwrap_err().msg,
            too_large
        );
        assert_eq!(
            try_fmt("%.99999999999999999999f", 1.0).unwrap_err().msg,
            too_large
        );
        // A `*` field is bounded the same way, either sign.
        let star = |fmt: &str, vals: &[f64]| {
            format_printf(&[Value::str(fmt), Value::Mat(Matrix::row(vals.to_vec()))])
        };
        let past = (MAX_FIELD + 1) as f64;
        assert_eq!(star("%*d", &[past, 1.0]).unwrap_err().msg, too_large);
        assert_eq!(star("%*d", &[-past, 1.0]).unwrap_err().msg, too_large);
        assert_eq!(star("%.*f", &[past, 1.0]).unwrap_err().msg, too_large);
        assert_eq!(star("%*d", &[1e300, 1.0]).unwrap_err().msg, too_large);
        let at = MAX_FIELD as f64;
        assert_eq!(star("%*d", &[at, 1.0]).unwrap().chars().count(), MAX_FIELD);
        assert!(star("%.*f", &[at, 1.0]).is_ok());
        // An absent field is still absent, and `%.f` still means `%.0f`.
        assert_eq!(pf("[%d]", &[5.0]), "[5]");
        assert_eq!(pf("[%.f]", &[1.5]), "[2]");
        assert_eq!(field("", 6).unwrap(), 6);
        assert_eq!(field("0", 6).unwrap(), 0);
        assert_eq!(field(&MAX_FIELD.to_string(), 0).unwrap(), MAX_FIELD);
        assert!(field(&(MAX_FIELD + 1).to_string(), 0).is_err());
    }

    /// `%d` of an integral value at or above 2^63 prints the value in full.
    #[test]
    fn printf_d_prints_an_integer_past_64_bits_in_full() {
        assert_eq!(pf("%d", &[1e30]), "1000000000000000019884624838656");
        assert_eq!(pf("%d", &[-1e30]), "-1000000000000000019884624838656");
        assert_eq!(pf("%d", &[I64_LIMIT]), "9223372036854775808");
        assert_eq!(pf("%d", &[-I64_LIMIT]), "-9223372036854775808");
        let below = 9_223_372_036_854_774_784.0;
        assert!(below < I64_LIMIT);
        assert_eq!(pf("%d", &[below]), "9223372036854774784");
        assert_eq!(pf("%d", &[0.0]), "0");
        assert_eq!(pf("%d", &[-0.0]), "0");
        assert_eq!(pf("%d", &[-7.0]), "-7");
        assert_eq!(pf("%d", &[1e15]), "1000000000000000");
        assert_eq!(pf("[%d][%d]", &[f64::INFINITY, f64::NAN]), "[Inf][NaN]");
        assert_eq!(pf("%+d", &[1e30]), "+1000000000000000019884624838656");
        assert_eq!(pf("%.33d", &[1e30]), "001000000000000000019884624838656");
    }

    #[test]
    fn printf_truncates_a_string_to_its_precision() {
        let s =
            |fmt: &str, text: &str| format_printf(&[Value::str(fmt), Value::str(text)]).unwrap();
        assert_eq!(s("[%5.2s]", "abcdef"), "[   ab]");
        assert_eq!(s("[%-5.2s]", "abcdef"), "[ab   ]");
        assert_eq!(s("[%.2s]", "abcdef"), "[ab]");
        assert_eq!(s("[%.0s]", "abcdef"), "[]");
        assert_eq!(s("[%.9s]", "abc"), "[abc]");
        assert_eq!(s("[%5s]", "abcdef"), "[abcdef]");
        assert_eq!(s("[%s]", "abcdef"), "[abcdef]");
        assert_eq!(s("[%.1s]", "abc"), "[a]");
        assert_eq!(pf("[%.0s]", &[65.0]), "[]");
        assert_eq!(pf("[%.1s]", &[65.0]), "[A]");
        // On the `%e` fallback the precision is the digits after the point.
        assert_eq!(pf("[%.3s]", &[1.5]), "[1.500e+00]");
        assert_eq!(pf("[%.2s]", &[123.456]), "[1.23e+02]");
    }

    #[test]
    fn printf_honours_precision_on_integers() {
        assert_eq!(pf("[%.3d]", &[5.0]), "[005]");
        assert_eq!(pf("[%.3d]", &[-5.0]), "[-005]");
        assert_eq!(pf("[%.2d]", &[12345.0]), "[12345]");
        assert_eq!(pf("[%6.3d]", &[5.0]), "[   005]");
        // A precision turns zero padding off for an integer, as in C.
        assert_eq!(pf("[%06.3d]", &[5.0]), "[   005]");
    }

    #[test]
    fn printf_switches_a_non_integer_under_percent_d_to_scientific() {
        assert_eq!(pf("[%d]", &[std::f64::consts::PI]), "[3.141593e+00]");
        assert_eq!(pf("[%d]", &[1.5]), "[1.500000e+00]");
        assert_eq!(pf("[%d]", &[42.0]), "[42]");
        assert_eq!(pf("[%d]", &[f64::INFINITY]), "[Inf]");
        assert_eq!(pf("[%d]", &[f64::NAN]), "[NaN]");
    }

    /// QA D16 (b): `%s` of a non-integer is `%e`, the MATLAB page's own
    /// example; a number that is a character code is still the character.
    #[test]
    fn percent_s_prints_a_number_as_a_character_or_in_e_form() {
        assert_eq!(pf("[%s]", &[65.0]), "[A]");
        assert_eq!(pf("[%s]", &[42.0]), "[*]");
        assert_eq!(pf("%s", &[std::f64::consts::PI]), "3.141593e+00");
        assert_eq!(pf("[%s]", &[3.5]), "[3.500000e+00]");
        assert_eq!(pf("[%s]", &[-1.0]), "[-1]");
        assert_eq!(pf("[%s]", &[f64::NAN]), "[NaN]");
    }

    /// QA D16 (a): `%E` and `%G` print an upper-case `E`.
    #[test]
    fn upper_case_conversions_print_an_upper_case_e() {
        assert_eq!(pf("%E", &[12345.678]), "1.234568E+04");
        assert_eq!(pf("%G", &[1e-10]), "1E-10");
        assert_eq!(pf("%G", &[1.5]), "1.5");
        assert_eq!(pf("%E|%G", &[f64::INFINITY, f64::NAN]), "Inf|NaN");
        assert_eq!(pf("%e", &[12345.678]), "1.234568e+04");
    }

    /// QA D16 (c): the `#` flag.
    #[test]
    fn the_alternate_flag() {
        assert_eq!(pf("%#.0f", &[3.0]), "3.");
        assert_eq!(pf("%.0f", &[3.0]), "3");
        assert_eq!(pf("%#.0e", &[3.0]), "3.e+00");
        assert_eq!(pf("%#g", &[1.5]), "1.50000");
        assert_eq!(pf("%#g", &[2.0]), "2.00000");
        assert_eq!(pf("%#.3g", &[1e-10]), "1.00e-10");
        assert_eq!(pf("%#x", &[255.0]), "0xff");
        assert_eq!(pf("%#X", &[255.0]), "0XFF");
        assert_eq!(pf("%#o", &[8.0]), "010");
        assert_eq!(pf("%#x", &[0.0]), "0");
        assert_eq!(pf("%#08x", &[255.0]), "0x0000ff");
    }

    /// QA D16 (d): zero padding never goes into `Inf` or `NaN`.
    #[test]
    fn zero_padding_leaves_a_non_finite_value_alone() {
        assert_eq!(pf("%05d", &[f64::NEG_INFINITY]), " -Inf");
        assert_eq!(pf("%05d", &[f64::INFINITY]), "  Inf");
        assert_eq!(pf("%06.1f", &[f64::NAN]), "   NaN");
        assert_eq!(pf("%05d", &[-42.0]), "-0042");
        assert_eq!(pf("%05.1f", &[3.5]), "003.5");
    }

    /// QA D16 (e): the remaining escapes.
    #[test]
    fn every_escape_is_processed() {
        assert_eq!(pf("\\x41\\x3d", &[]), "A=");
        assert_eq!(pf("\\101\\60\\0", &[]), "A0\0");
        assert_eq!(pf("\\1011", &[]), "A1");
        assert_eq!(pf("\\a\\b\\f\\v", &[]), "\u{7}\u{8}\u{C}\u{B}");
        assert_eq!(pf("a\\tb\\nc\\r\\\\", &[]), "a\tb\nc\r\\");
        // An escape that means nothing keeps its backslash.
        assert_eq!(pf("\\q\\x", &[]), "\\q\\x");
        assert_eq!(escapes("a\\nb\\\\"), "a\nb\\");
    }

    /// QA D16 (f): `%x`, `%X`, `%o` and `*`.
    #[test]
    fn hexadecimal_octal_and_star() {
        assert_eq!(pf("%x", &[255.0]), "ff");
        assert_eq!(pf("%X", &[255.0]), "FF");
        assert_eq!(pf("%o", &[8.0]), "10");
        assert_eq!(pf("%x", &[-255.0]), "-ff");
        assert_eq!(pf("%.4x", &[255.0]), "00ff");
        assert_eq!(pf("%6x|", &[255.0]), "    ff|");
        assert_eq!(pf("%x", &[1.5]), "1.500000e+00");
        assert_eq!(pf("%x", &[f64::INFINITY]), "Inf");
        assert_eq!(pf("%x", &[1e20]), "1.000000e+20");
        assert_eq!(pf("%*d", &[5.0, 3.0]), "    3");
        assert_eq!(pf("%-*d|", &[5.0, 3.0]), "3    |");
        assert_eq!(pf("%*d|", &[-5.0, 3.0]), "3    |");
        assert_eq!(pf("%.*f", &[2.0, std::f64::consts::PI]), "3.14");
        assert_eq!(pf("%*.*f", &[8.0, 3.0, std::f64::consts::PI]), "   3.142");
        assert_eq!(pf("%.*f", &[-1.0, 1.5]), "1.500000");
        // A `*` with nothing left to read stops, as a conversion would.
        assert_eq!(pf("a%*d", &[5.0]), "a");
        assert_eq!(pf("%ld %hd %lx", &[1.0, 2.0, 255.0]), "1 2 ff");
    }

    /// QA D16 (g): an invalid conversion or a trailing `%` ends the output.
    #[test]
    fn an_invalid_conversion_ends_the_output() {
        assert_eq!(pf("abc%q", &[1.0]), "abc");
        assert_eq!(pf("abc%", &[]), "abc");
        assert_eq!(pf("abc%5", &[1.0]), "abc");
        assert_eq!(pf("x%d y%k z", &[1.0, 2.0]), "x1 y");
        assert_eq!(pf("%d %y", &[1.0, 2.0, 3.0]), "1 ");
    }

    #[test]
    fn a_char_argument_expands_one_argument_per_character() {
        let args = [Value::str("[%d %d]"), Value::str("AB")];
        assert_eq!(format_printf(&args).unwrap(), "[65 66]");
        let args = [Value::str("[%s]"), Value::str("AB")];
        assert_eq!(format_printf(&args).unwrap(), "[AB]");
        let args = [Value::str("%s=%d"), Value::str("ab"), num(5.0)];
        assert_eq!(format_printf(&args).unwrap(), "ab=5");
        let args = [Value::str("%d%s"), Value::str("AB")];
        assert_eq!(format_printf(&args).unwrap(), "65B");
    }

    #[test]
    fn printf_conversions_and_cycling_are_unchanged() {
        assert_eq!(pf("%d", &[42.0]), "42");
        assert_eq!(pf("%f", &[1.5]), "1.500000");
        assert_eq!(pf("%e", &[1.5]), "1.500000e+00");
        assert_eq!(pf("%g", &[1.5]), "1.5");
        assert_eq!(pf("%c", &[65.0]), "A");
        assert_eq!(pf("%6.3f", &[1.5]), " 1.500");
        assert_eq!(pf("%-5d|", &[42.0]), "42   |");
        assert_eq!(pf("%2d", &[12345.0]), "12345");
        assert_eq!(pf("100%%", &[]), "100%");
        assert_eq!(pf("[%d]", &[]), "[]");
        assert_eq!(pf("%d %d\\n", &[1.0, 2.0, 3.0, 4.0]), "1 2\n3 4\n");
        assert_eq!(pf("%d %d %d\\n", &[1.0, 2.0]), "1 2 ");
        assert_eq!(pf("%d-", &[1.0, 2.0, 3.0]), "1-2-3-");
        assert_eq!(pf("hi", &[1.0, 2.0]), "hi");
        assert!(format_printf(&[Value::Mat(Matrix::scalar(1.0))]).is_err());
    }

    /// `%s` decodes a char argument's code units, joining a surrogate pair.
    #[test]
    fn printf_decodes_utf16_code_units() {
        let s =
            |fmt: &str, text: &str| format_printf(&[Value::str(fmt), Value::str(text)]).unwrap();
        assert_eq!(s("[%s]", "😀"), "[😀]");
        assert_eq!(s("😀%s", "é"), "😀é");
        assert_eq!(s("%d %d", "😀"), "55357 56832");
        let two =
            Value::Mat(Matrix::new(2, 2, vec![97.0, 99.0, 98.0, 100.0]).with_class(Class::Char));
        assert_eq!(format_printf(&[Value::str("%s"), two]).unwrap(), "acbd");
    }
}
