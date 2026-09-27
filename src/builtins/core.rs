//! Constants, constructors, shape queries, output, the workspace and timing.

use std::f64::consts::{E, PI};

use super::args::{at_most, check_size, dim, mat, need, scalar, size_arg, string};
use super::{Registry, add, none, one, one_mat};
use crate::interp::{Interp, R, fmt_e, fmt_g};
use crate::value::{Matrix, Value, nonfinite};

/// The registration table is one line per builtin on purpose: it is the index
/// of the library, and rustfmt would otherwise spread each entry over five
/// lines and hide it.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    // ---- constants ---------------------------------------------------
    add(r, "pi", pi, "pi - ratio of a circle's circumference to its diameter.");
    add(r, "e", e, "e - base of the natural logarithm.");
    add(r, "Inf", inf, "Inf, Inf(n), Inf(r,c) - infinity.");
    add(r, "inf", inf, "inf, inf(n), inf(r,c) - infinity.");
    add(r, "NaN", nan, "NaN, NaN(n), NaN(r,c) - not-a-number.");
    add(r, "nan", nan, "nan, nan(n), nan(r,c) - not-a-number.");
    add(r, "eps", eps, "eps - distance from 1.0 to the next larger double.");
    add(r, "true", tru, "true - logical 1. true(n) waits for the logical class.");
    add(r, "false", fls, "false - logical 0. false(n) waits for the logical class.");

    // ---- constructors ------------------------------------------------
    add(r, "zeros", zeros, "zeros(n), zeros(r,c) - a matrix of zeros.");
    add(r, "ones", ones, "ones(n), ones(r,c) - a matrix of ones.");
    add(r, "eye", eye, "eye(n), eye(r,c) - ones on the main diagonal.");
    add(r, "rand", rand, "rand(n), rand(r,c) - uniform values in [0, 1).");
    add(r, "linspace", linspace, "linspace(a,b,n) - n points evenly spaced from a to b.");

    // ---- shape queries -----------------------------------------------
    add(r, "size", size, "size(A), size(A,dim) - the dimensions of A.");
    add(r, "numel", numel, "numel(A) - the number of elements of A.");
    add(r, "length", length, "length(A) - the longest dimension, or 0 if empty.");
    add(r, "isempty", isempty, "isempty(A) - true when A has no elements.");
    add(r, "isscalar", isscalar, "isscalar(A) - true when A is 1x1.");
    add(r, "isvector", isvector, "isvector(A) - true when A is a non-empty vector.");

    // ---- output ------------------------------------------------------
    add(r, "disp", disp, "disp(X) - display X without printing its name.");
    add(r, "fprintf", fprintf, "fprintf(fmt,...) - write formatted text.");
    add(r, "sprintf", sprintf, "sprintf(fmt,...) - format text into a string.");
    add(r, "num2str", num2str_fn, "num2str(x) - convert a number to text.");
    add(r, "error", error, "error(fmt,...) - raise an error with a message.");

    // ---- workspace ---------------------------------------------------
    add(r, "clear", clear, "clear, clear('a') - remove variables from the workspace.");
    add(r, "clc", clc, "clc - clear the screen.");
    add(r, "who", who, "who - list the variables in the workspace.");
    add(r, "whos", who, "whos - list the workspace variables with their sizes.");

    // ---- timing ------------------------------------------------------
    add(r, "tic", tic, "tic - start a stopwatch; t = tic returns a handle.");
    add(r, "toc", toc, "toc, toc(t) - elapsed seconds since tic.");
}

// ---- constants -------------------------------------------------------

/// A constant that takes no arguments at all.
fn constant(args: &[Value], name: &str, v: f64) -> R<Vec<Value>> {
    at_most(args, 0, name)?;
    one_mat(Matrix::scalar(v))
}

/// A constant that fills a matrix when given a size, as `NaN(2)` does.
fn filled_constant(args: &[Value], name: &str, v: f64) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    let (r, c) = shape(args, name)?;
    check_size(r, c)?;
    one_mat(Matrix::filled(r, c, v))
}

fn pi(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    constant(a, "pi", PI)
}

fn e(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    constant(a, "e", E)
}

fn eps(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    constant(a, "eps", f64::EPSILON)
}

/// `true` and `false` stay scalars: `true(n)` needs the logical class, which
/// is cycle 02.
fn tru(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    constant(a, "true", 1.0)
}

fn fls(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    constant(a, "false", 0.0)
}

fn inf(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    filled_constant(a, "Inf", f64::INFINITY)
}

fn nan(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    filled_constant(a, "NaN", f64::NAN)
}

// ---- constructors ----------------------------------------------------

/// The `(rows, cols)` shared by every constructor: none, one, or two size
/// arguments. A negative size is `0`, exactly as in MATLAB.
fn shape(args: &[Value], name: &str) -> R<(usize, usize)> {
    match args.len() {
        0 => Ok((1, 1)),
        1 => {
            let n = size_arg(args, 0, name)?;
            Ok((n, n))
        }
        _ => Ok((size_arg(args, 0, name)?, size_arg(args, 1, name)?)),
    }
}

/// Which of the four constructors that share one implementation is running.
#[derive(Clone, Copy)]
enum Fill {
    Zeros,
    Ones,
    Eye,
    Rand,
}

fn construct(it: &mut Interp, args: &[Value], name: &str, kind: Fill) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    let (r, c) = shape(args, name)?;
    check_size(r, c)?;
    let m = match kind {
        Fill::Zeros => Matrix::filled(r, c, 0.0),
        Fill::Ones => Matrix::filled(r, c, 1.0),
        Fill::Eye => Matrix::identity(r, c),
        Fill::Rand => {
            let mut m = Matrix::filled(r, c, 0.0);
            for v in &mut m.data {
                *v = it.next_rand();
            }
            m
        }
    };
    one_mat(m)
}

fn zeros(it: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    construct(it, a, "zeros", Fill::Zeros)
}

fn ones(it: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    construct(it, a, "ones", Fill::Ones)
}

fn eye(it: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    construct(it, a, "eye", Fill::Eye)
}

fn rand(it: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    construct(it, a, "rand", Fill::Rand)
}

fn linspace(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 3, "linspace")?;
    let a = scalar(args, 0, "linspace")?;
    let b = scalar(args, 1, "linspace")?;
    let n = if args.len() >= 3 {
        size_arg(args, 2, "linspace")?
    } else {
        100
    };
    check_size(1, n)?;
    let data = (0..n)
        .map(|k| {
            if n == 1 {
                b
            } else {
                a + (b - a) * k as f64 / (n - 1) as f64
            }
        })
        .collect();
    one_mat(Matrix::row(data))
}

// ---- shape queries ---------------------------------------------------

fn size(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 2, "size")?;
    let m = mat(args, 0, "size")?;
    if args.len() >= 2 {
        // A dimension past the array's is a singleton; `0` is an error.
        let v = match dim(args, 1, "size")? {
            1 => m.rows,
            2 => m.cols,
            _ => 1,
        };
        one_mat(Matrix::scalar(v as f64))
    } else {
        one_mat(Matrix::row(vec![m.rows as f64, m.cols as f64]))
    }
}

fn numel(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "numel")?;
    one_mat(Matrix::scalar(mat(args, 0, "numel")?.numel() as f64))
}

fn length(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "length")?;
    let m = mat(args, 0, "length")?;
    let n = if m.is_empty() { 0 } else { m.rows.max(m.cols) };
    one_mat(Matrix::scalar(n as f64))
}

fn isempty(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "isempty")?;
    one_mat(Matrix::scalar(
        mat(args, 0, "isempty")?.is_empty() as u8 as f64
    ))
}

fn isscalar(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "isscalar")?;
    one_mat(Matrix::scalar(
        mat(args, 0, "isscalar")?.is_scalar() as u8 as f64
    ))
}

fn isvector(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "isvector")?;
    one_mat(Matrix::scalar(
        mat(args, 0, "isvector")?.is_vector() as u8 as f64
    ))
}

// ---- output ----------------------------------------------------------

fn disp(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "disp")?;
    need(args, 1, "disp")?;
    let text = match &args[0] {
        Value::Str(s) => format!("{s}\n"),
        Value::Mat(m) => m.format(),
    };
    it.emit(&text)?;
    none()
}

fn fprintf(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "fprintf")?;
    let text = format_printf(args)?;
    it.emit(&text)?;
    none()
}

fn sprintf(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "sprintf")?;
    one(Value::Str(format_printf(args)?))
}

fn num2str_fn(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "num2str")?;
    need(args, 1, "num2str")?;
    let s = match &args[0] {
        Value::Str(s) => s.clone(),
        Value::Mat(m) => m
            .data
            .iter()
            .map(|v| num2str(*v))
            .collect::<Vec<_>>()
            .join("  "),
    };
    one(Value::Str(s))
}

fn error(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let msg = match args.first() {
        Some(Value::Str(_)) => format_printf(args)?,
        _ => "error".to_string(),
    };
    Err(msg)
}

// ---- workspace -------------------------------------------------------

fn clear(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    if args.is_empty() {
        it.vars.clear();
    } else {
        // `clear('a')` clears only `a`. Clearing a name that is not there is
        // not an error in MATLAB either.
        for i in 0..args.len() {
            let name = string(args, i, "clear")?;
            it.vars.remove(&name);
        }
    }
    none()
}

fn clc(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "clc")?;
    it.emit("\x1B[2J\x1B[H")?;
    none()
}

fn who(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "who")?;
    let mut names: Vec<String> = it.vars.keys().cloned().collect();
    names.sort();
    if names.is_empty() {
        return none();
    }
    let mut text = String::from("Your variables are:\n\n");
    for n in &names {
        let row = match &it.vars[n] {
            Value::Mat(m) => format!("  {:<12} {}x{} double\n", n, m.rows, m.cols),
            Value::Str(s) => format!("  {:<12} 1x{} char\n", n, s.chars().count()),
        };
        text.push_str(&row);
    }
    text.push('\n');
    it.emit(&text)?;
    none()
}

// ---- timing ----------------------------------------------------------

fn tic(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(args, 0, "tic")?;
    let now = it.clock_nanos();
    if nargout >= 1 {
        // `t = tic` hands back a handle instead of touching the shared mark,
        // so a nested tic/toc pair cannot disturb an outer one.
        return one_mat(Matrix::scalar(now));
    }
    it.tic_mark = now;
    none()
}

fn toc(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(args, 1, "toc")?;
    let base = if args.is_empty() {
        it.tic_mark
    } else {
        scalar(args, 0, "toc")?
    };
    let secs = (it.clock_nanos() - base) / 1e9;
    if nargout >= 1 {
        return one_mat(Matrix::scalar(secs));
    }
    it.emit(&format!("Elapsed time is {:.6} seconds.\n", secs))?;
    none()
}

// ---- number to text --------------------------------------------------

fn num2str(v: f64) -> String {
    // Non-finite first: log10(Inf) is Inf, whose cast to i32 saturates, and
    // the `+ 5` below then overflows and aborts the process.
    if !v.is_finite() {
        return nonfinite(v);
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let digits = if v.abs() >= 1.0 {
            (v.abs().log10().floor() as i32).saturating_add(5).max(5) as usize
        } else {
            5
        };
        fmt_g(v, digits)
    }
}

// ---- printf ----------------------------------------------------------

/// One flattened `printf` argument.
#[derive(Clone, Copy)]
enum PArg {
    Num(f64),
    /// One character of a char argument, tagged with the argument it came
    /// from. MATLAB expands a char array to one argument per character, so
    /// `fprintf('%d %d', 'AB')` prints `65 66`; the tag is what lets a later
    /// `%s` put the run back together and print `AB`.
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

/// `%d`, `%i` and `%u`. Precision zero-pads the digits, as in C.
fn int_body(v: f64, prec: Option<usize>) -> String {
    if !v.is_finite() {
        return nonfinite(v);
    }
    if v.fract() != 0.0 {
        // MATLAB switches an integer conversion to %e, not %g, for a value
        // that is not an integer.
        return fmt_e(v, prec.unwrap_or(6));
    }
    // Known bug, scheduled to cycle 11: this saturates at 64 bits.
    let n = v as i64;
    let digits = format!("{}", n.unsigned_abs());
    let pad = prec.unwrap_or(0).saturating_sub(digits.len());
    format!(
        "{}{}{}",
        if n < 0 { "-" } else { "" },
        "0".repeat(pad),
        digits
    )
}

/// The character a number denotes, when it denotes one at all.
fn char_code(v: f64) -> Option<char> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..=f64::from(char::MAX as u32)).contains(&v) {
        char::from_u32(v as u32)
    } else {
        None
    }
}

/// `%s` given a number: MATLAB prints the character with that code, and falls
/// back to the number itself when the value is not a character code.
fn str_body(v: f64, prec: Option<usize>) -> String {
    match char_code(v) {
        Some(c) => c.to_string(),
        None => fmt_g(v, prec.unwrap_or(6)),
    }
}

/// Shared implementation of fprintf / sprintf. Cycles the format over the
/// flattened arguments like MATLAB does.
pub fn format_printf(args: &[Value]) -> R<String> {
    let fmt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err("The first argument must be a format string.".to_string()),
    };
    let mut flat: Vec<PArg> = Vec::new();
    for (group, a) in args[1..].iter().enumerate() {
        match a {
            Value::Str(s) => flat.extend(s.chars().map(|c| PArg::Chr(c as u32 as f64, group))),
            Value::Mat(m) => flat.extend(m.data.iter().map(|v| PArg::Num(*v))),
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
                i += 1;
                match chars[i] {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '\\' => out.push('\\'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
                i += 1;
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
            let mut width = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                width.push(chars[i]);
                i += 1;
            }
            let mut prec: Option<usize> = None;
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                let mut p = String::new();
                while i < chars.len() && chars[i].is_ascii_digit() {
                    p.push(chars[i]);
                    i += 1;
                }
                prec = Some(p.parse().unwrap_or(0));
            }
            if i >= chars.len() {
                return Err("Invalid format specifier.".to_string());
            }
            let conv = chars[i];
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
            // Only a numeric conversion takes a sign flag or zero padding.
            let mut numeric = true;
            let mut body = match (conv, arg) {
                (_, None) => {
                    numeric = false;
                    String::new()
                }
                ('d' | 'i' | 'u', Some(a)) => int_body(a.value(), prec),
                ('f' | 'F', Some(a)) => {
                    let v = a.value();
                    if v.is_finite() {
                        format!("{:.*}", prec.unwrap_or(6), v)
                    } else {
                        nonfinite(v)
                    }
                }
                ('e' | 'E', Some(a)) => fmt_e(a.value(), prec.unwrap_or(6)),
                ('g' | 'G', Some(a)) => fmt_g(a.value(), prec.unwrap_or(6)),
                ('c', Some(a)) => {
                    numeric = false;
                    char_of(a.value()).to_string()
                }
                ('s', Some(PArg::Chr(v, group))) => {
                    numeric = false;
                    // %s takes the whole char argument, not one character.
                    let mut s = String::from(char_of(v));
                    while let Some(PArg::Chr(next, g)) = flat.get(ai).copied() {
                        if g != group {
                            break;
                        }
                        s.push(char_of(next));
                        ai += 1;
                    }
                    s
                }
                ('s', Some(PArg::Num(v))) => {
                    numeric = false;
                    str_body(v, prec)
                }
                (other, _) => return Err(format!("Unsupported format specifier '%{}'.", other)),
            };
            if numeric && !body.starts_with('-') {
                if flags.contains('+') {
                    body.insert(0, '+');
                } else if flags.contains(' ') {
                    body.insert(0, ' ');
                }
            }
            let width: usize = width.parse().unwrap_or(0);
            let len = body.chars().count();
            if len >= width {
                out.push_str(&body);
            } else if flags.contains('-') {
                out.push_str(&body);
                out.extend(std::iter::repeat_n(' ', width - len));
            } else if flags.contains('0') && numeric {
                let (sign, digits) = match body.strip_prefix(['-', '+', ' ']) {
                    Some(d) => (&body[..1], d.to_string()),
                    None => ("", body.clone()),
                };
                out.push_str(sign);
                out.extend(std::iter::repeat_n('0', width - len));
                out.push_str(&digits);
            } else {
                out.extend(std::iter::repeat_n(' ', width - len));
                out.push_str(&body);
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

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    fn call(f: super::super::BuiltinFn, args: &[Value], nargout: usize) -> R<Vec<Value>> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        f(&mut it, args, nargout)
    }

    fn shape_of(f: super::super::BuiltinFn, args: &[Value]) -> (usize, usize) {
        match &call(f, args, 1).unwrap()[0] {
            Value::Mat(m) => (m.rows, m.cols),
            Value::Str(_) => panic!("expected a matrix"),
        }
    }

    // ---- constructors ------------------------------------------------

    #[test]
    fn constructors_dispatch_to_the_right_fill() {
        assert_eq!(shape_of(zeros, &[num(2.0), num(3.0)]), (2, 3));
        assert_eq!(shape_of(ones, &[num(2.0)]), (2, 2));
        assert_eq!(shape_of(eye, &[num(3.0)]), (3, 3));
        assert_eq!(shape_of(rand, &[num(2.0), num(2.0)]), (2, 2));
        match &call(eye, &[num(2.0)], 1).unwrap()[0] {
            Value::Mat(m) => assert_eq!(m.data, [1.0, 0.0, 0.0, 1.0]),
            Value::Str(_) => panic!("expected a matrix"),
        }
        // No arguments at all is a 1x1, as in MATLAB.
        assert_eq!(shape_of(zeros, &[]), (1, 1));
    }

    #[test]
    fn a_negative_size_is_empty_rather_than_an_error() {
        assert_eq!(shape_of(zeros, &[num(-1.0)]), (0, 0));
        assert_eq!(shape_of(zeros, &[num(2.0), num(-3.0)]), (2, 0));
        assert_eq!(shape_of(ones, &[num(0.0)]), (0, 0));
    }

    #[test]
    fn a_size_that_would_overflow_is_an_error_not_a_panic() {
        let e = call(zeros, &[num(1e10)], 1).unwrap_err();
        assert!(e.contains("10000000000x10000000000"), "{e}");
        assert!(call(ones, &[num(1e10)], 1).is_err());
        assert!(call(rand, &[num(1e10)], 1).is_err());
        assert!(call(eye, &[num(1e10)], 1).is_err());
        assert!(call(linspace, &[num(0.0), num(1.0), num(1e10)], 1).is_err());
    }

    #[test]
    fn constants_fill_a_matrix_only_where_matlab_does() {
        assert_eq!(shape_of(nan, &[num(2.0)]), (2, 2));
        assert_eq!(shape_of(inf, &[num(2.0), num(3.0)]), (2, 3));
        match &call(nan, &[num(2.0)], 1).unwrap()[0] {
            Value::Mat(m) => assert!(m.data.iter().all(|v| v.is_nan())),
            Value::Str(_) => panic!("expected a matrix"),
        }
        assert_eq!(shape_of(inf, &[]), (1, 1));
        // pi and true take no size argument yet; see "Known bugs".
        assert!(call(pi, &[num(2.0)], 1).is_err());
        assert!(call(tru, &[num(2.0)], 1).is_err());
    }

    #[test]
    fn extra_arguments_are_rejected() {
        assert_eq!(
            call(numel, &[num(1.0), num(2.0)], 1).unwrap_err(),
            "Too many input arguments."
        );
        assert!(call(disp, &[num(1.0), num(2.0)], 0).is_err());
        assert!(call(clc, &[num(1.0)], 0).is_err());
        assert!(call(size, &[num(1.0), num(1.0), num(1.0)], 1).is_err());
    }

    #[test]
    fn size_rejects_dimension_zero() {
        let a = [Value::Mat(Matrix::row(vec![1.0, 2.0, 3.0]))];
        let args = [a[0].clone(), num(0.0)];
        let e = call(size, &args, 1).unwrap_err();
        assert!(e.contains("positive integer"), "{e}");
        // A dimension past the array's is a singleton.
        let args = [a[0].clone(), num(3.0)];
        assert_eq!(
            call(size, &args, 1).unwrap()[0].clone().into_mat().data,
            [1.0]
        );
    }

    // ---- workspace and timing ----------------------------------------

    #[test]
    fn clear_with_a_name_clears_only_that_name() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        it.vars.insert("a".to_string(), num(1.0));
        it.vars.insert("b".to_string(), num(2.0));
        clear(&mut it, &[Value::Str("a".to_string())], 0).unwrap();
        assert!(!it.vars.contains_key("a"));
        assert!(it.vars.contains_key("b"));
        // Clearing a name that is not there is not an error.
        clear(&mut it, &[Value::Str("nope".to_string())], 0).unwrap();
        // With no arguments it still clears everything.
        clear(&mut it, &[], 0).unwrap();
        assert!(it.vars.is_empty());
    }

    #[test]
    fn tic_and_toc_depend_on_nargout() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        // As a statement, tic produces no value and toc prints one.
        assert!(tic(&mut it, &[], 0).unwrap().is_empty());
        assert!(toc(&mut it, &[], 0).unwrap().is_empty());
        // Asked for a value, both produce one.
        let handle = tic(&mut it, &[], 1).unwrap();
        assert_eq!(handle.len(), 1);
        let elapsed = toc(&mut it, &handle, 1).unwrap()[0].clone().into_mat();
        assert!(elapsed.scalar_value().unwrap() >= 0.0);
        let bare = toc(&mut it, &[], 1).unwrap()[0].clone().into_mat();
        assert!(bare.scalar_value().unwrap() >= 0.0);
    }

    // ---- num2str -----------------------------------------------------

    #[test]
    fn num2str_handles_the_non_finite_values_that_used_to_panic() {
        assert_eq!(num2str(f64::INFINITY), "Inf");
        assert_eq!(num2str(f64::NEG_INFINITY), "-Inf");
        assert_eq!(num2str(f64::NAN), "NaN");
        assert_eq!(num2str(7.0), "7");
        assert_eq!(num2str(3.5), "3.5");
        assert_eq!(num2str(-0.25), "-0.25");
        assert_eq!(num2str(1.0e17), "100000000000000000");
    }

    // ---- printf ------------------------------------------------------

    fn pf(fmt: &str, nums: &[f64]) -> String {
        let mut args = vec![Value::Str(fmt.to_string())];
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

    #[test]
    fn printf_honours_precision_on_integers() {
        assert_eq!(pf("[%.3d]", &[5.0]), "[005]");
        assert_eq!(pf("[%.3d]", &[-5.0]), "[-005]");
        assert_eq!(pf("[%.2d]", &[12345.0]), "[12345]");
        assert_eq!(pf("[%6.3d]", &[5.0]), "[   005]");
    }

    #[test]
    fn printf_switches_a_non_integer_under_percent_d_to_scientific() {
        assert_eq!(pf("[%d]", &[std::f64::consts::PI]), "[3.141593e+00]");
        assert_eq!(pf("[%d]", &[1.5]), "[1.500000e+00]");
        // Integers, infinities and NaN are untouched.
        assert_eq!(pf("[%d]", &[42.0]), "[42]");
        assert_eq!(pf("[%d]", &[f64::INFINITY]), "[Inf]");
        assert_eq!(pf("[%d]", &[f64::NAN]), "[NaN]");
    }

    #[test]
    fn percent_s_prints_a_number_as_a_character() {
        assert_eq!(pf("[%s]", &[65.0]), "[A]");
        assert_eq!(pf("[%s]", &[42.0]), "[*]");
        // A value that is not a character code falls back to %g.
        assert_eq!(pf("[%s]", &[3.5]), "[3.5]");
        assert_eq!(pf("[%s]", &[-1.0]), "[-1]");
    }

    #[test]
    fn a_char_argument_expands_one_argument_per_character() {
        let args = [
            Value::Str("[%d %d]".to_string()),
            Value::Str("AB".to_string()),
        ];
        assert_eq!(format_printf(&args).unwrap(), "[65 66]");
        // ... but %s still takes the whole char argument.
        let args = [Value::Str("[%s]".to_string()), Value::Str("AB".to_string())];
        assert_eq!(format_printf(&args).unwrap(), "[AB]");
        // Mixed: the char array is one %s, the number is one %d.
        let args = [
            Value::Str("%s=%d".to_string()),
            Value::Str("ab".to_string()),
            num(5.0),
        ];
        assert_eq!(format_printf(&args).unwrap(), "ab=5");
        // A numeric conversion consumes one character, leaving the rest.
        let args = [Value::Str("%d%s".to_string()), Value::Str("AB".to_string())];
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
        assert_eq!(pf("%05.1f", &[3.5]), "003.5");
        assert_eq!(pf("%05d", &[-42.0]), "-0042");
        assert_eq!(pf("%2d", &[12345.0]), "12345");
        assert_eq!(pf("100%%", &[]), "100%");
        assert_eq!(pf("a\\tb\\nc", &[]), "a\tb\nc");
        assert_eq!(pf("\\q", &[]), "\\q");
        assert_eq!(pf("[%d]", &[]), "[]");
        assert_eq!(pf("%d %d\\n", &[1.0, 2.0, 3.0, 4.0]), "1 2\n3 4\n");
        assert_eq!(pf("%d %d %d\\n", &[1.0, 2.0]), "1 2 ");
        assert_eq!(pf("%d-", &[1.0, 2.0, 3.0]), "1-2-3-");
        assert_eq!(pf("hi", &[1.0, 2.0]), "hi");
        assert!(format_printf(&[Value::Mat(Matrix::scalar(1.0))]).is_err());
        assert!(format_printf(&[Value::Str("%q".to_string()), num(1.0)]).is_err());
    }
}
