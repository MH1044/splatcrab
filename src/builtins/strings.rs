//! The string functions (cycle 11): joining, splitting, replacing,
//! trimming, case, comparison, search, the conversions between numbers and
//! text, and `regexp` and `regexprep` over the engine in `regex.rs`.
//!
//! Text is a char array, and every function here works on its UTF-16 code
//! units, as the char class stores them, so an index into a result is a
//! MATLAB index less one and a surrogate pair survives every operation
//! whole.
//!
//! A cell of character vectors is taken where MATLAB takes one: `strcat`,
//! `strjoin` (which needs one), `strrep`, `strtrim`, `upper`, `lower`, the
//! `strcmp` family, `strfind`, `str2double` and `regexprep`, each mapping
//! over the elements; `regexp` of a cell answers a cell of per-element
//! outputs. `strsplit`, `strtok`, `blanks`, `mat2str`, `int2str`,
//! `num2str` and `str2num` take a single array.

use super::args::{MAX_ELEMS, at_most, check_cell, check_shape, check_struct, mat, need};
use super::printf::{escapes, format_printf};
use super::regex::{Groups, Regex};
use super::{Registry, add, one, one_as};
use crate::error;
use crate::interp::{Interp, R, fmt_g};
use crate::value::{CellArray, Class, Matrix, StructArray, Value, nonfinite};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "strcat", strcat, "strcat(s1,s2,...) - join texts horizontally, dropping each char argument's trailing whitespace.");
    add(r, "strsplit", strsplit, "[C,matches] = strsplit(s,delim) - split s at each delimiter (whitespace by default) into a cell.");
    add(r, "strjoin", strjoin, "strjoin(C,delim) - join a cell of texts with delim (a space by default) between them.");
    add(r, "strrep", strrep, "strrep(s,old,new) - replace every occurrence of old in s with new.");
    add(r, "strtrim", strtrim, "strtrim(s) - remove leading and trailing whitespace.");
    add(r, "upper", upper, "upper(s) - the text in upper case.");
    add(r, "lower", lower, "lower(s) - the text in lower case.");
    add(r, "strcmp", strcmp, "strcmp(a,b) - true when two texts are identical.");
    add(r, "strcmpi", strcmpi, "strcmpi(a,b) - strcmp ignoring case.");
    add(r, "strncmp", strncmp, "strncmp(a,b,n) - true when two texts agree in their first n characters.");
    add(r, "strncmpi", strncmpi, "strncmpi(a,b,n) - strncmp ignoring case.");
    add(r, "strfind", strfind, "strfind(s,pattern) - the start index of every occurrence of pattern in s.");
    add(r, "strtok", strtok, "[tok,rem] = strtok(s,delims) - the first token of s and the rest.");
    add(r, "int2str", int2str, "int2str(x) - x rounded to integers, as text.");
    add(r, "num2str", num2str_fn, "num2str(x), num2str(x,n), num2str(x,fmt) - numbers as text, one row per matrix row.");
    add(r, "str2double", str2double, "str2double(s) - the number a text denotes, or NaN.");
    add(r, "str2num", str2num, "[x,ok] = str2num(s) - the matrix a text of numbers denotes.");
    add(r, "mat2str", mat2str, "mat2str(A), mat2str(A,n) - the text of a matrix in MATLAB syntax.");
    add(r, "isspace", isspace, "isspace(s) - true where a character is whitespace.");
    add(r, "isletter", isletter, "isletter(s) - true where a character is a letter.");
    add(r, "blanks", blanks, "blanks(n) - a row of n spaces.");
    add(r, "regexp", regexp, "regexp(s,expr,options) - match a regular expression: 'match', 'tokens', 'names', 'start', 'end', 'split', 'once'.");
    add(r, "regexprep", regexprep, "regexprep(s,expr,rep) - replace every match of a regular expression; $N names a token.");
}

// ---- text helpers ------------------------------------------------------

/// Judges a text of `len` code units, or a total of that many across the
/// elements of a result, before it is built: a replacement or a join can
/// ask for far more than its inputs hold, `strrep(repmat('a', 1, 1e5),
/// 'a', repmat('b', 1, 1e5))` for 1e10.
fn fits(len: usize) -> R<()> {
    if len > MAX_ELEMS {
        check_shape(1.0, len as f64)?;
    }
    Ok(())
}

/// A char row of these code units: `1xN`, `1x0` when there are none.
fn chars(units: Vec<f64>) -> Value {
    let n = units.len();
    Value::Mat(Matrix::new(1, n, units).with_class(Class::Char))
}

/// The code units of a char value, column-major; `None` for any other.
fn units_of(v: &Value) -> Option<&[f64]> {
    match v {
        Value::Mat(m) if m.class == Class::Char => Some(&m.data),
        _ => None,
    }
}

/// Argument `i` as the code units of a char, refused otherwise.
fn text_arg<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a [f64]> {
    match args.get(i) {
        Some(v) => units_of(v).ok_or_else(|| error::arg_not_a_string(i + 1, name)),
        None => Err(error::not_enough_args(name)),
    }
}

/// A char matrix from rows of code units, the shorter ones padded on the
/// right with spaces.
pub(crate) fn char_rows(rows: &[Vec<f64>]) -> Matrix {
    let r = rows.len();
    let c = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut data = vec![32.0; r * c];
    for (i, row) in rows.iter().enumerate() {
        for (j, u) in row.iter().enumerate() {
            data[j * r + i] = *u;
        }
    }
    Matrix::new(r, c, data).with_class(Class::Char)
}

/// Row `r` of a matrix's code units.
fn row_units(m: &Matrix, r: usize) -> Vec<f64> {
    (0..m.cols).map(|c| m.get(r, c)).collect()
}

fn units(s: &str) -> Vec<f64> {
    s.encode_utf16().map(f64::from).collect()
}

/// The character a code unit is, when it is one on its own.
fn char_at(u: f64) -> Option<char> {
    char::from_u32(u as u32)
}

/// Whitespace, as `isspace` and `strtrim` read it.
fn is_white(u: f64) -> bool {
    char_at(u).is_some_and(char::is_whitespace)
}

/// The trailing whitespace `strcat` drops: MATLAB's ASCII whitespace.
fn is_ascii_white(u: f64) -> bool {
    matches!(u as u32, 9..=13 | 32)
}

pub(crate) fn lower_unit(u: f64) -> f64 {
    f64::from(super::regex::fold(u as u32))
}

pub(crate) fn upper_unit(u: f64) -> f64 {
    f64::from(super::regex::upper(u as u32))
}

/// Each element of a cell of texts through `f`, the result a cell of the
/// same shape. An element that is not text is refused in `name`'s words.
fn map_cell(c: &CellArray, name: &str, mut f: impl FnMut(&[f64]) -> R<Value>) -> R<Value> {
    let mut out = Vec::with_capacity(c.data.len());
    for v in &c.data {
        let u = units_of(v).ok_or_else(|| error::cell_not_text(name))?;
        out.push(f(u)?);
    }
    Ok(Value::cell(CellArray::new(c.rows, c.cols, out)))
}

// ---- joining and splitting -----------------------------------------------

/// `strcat`: char arguments lose their trailing whitespace and are joined
/// side by side, row for row; with a cell among the arguments the result
/// is a cell, each element joined, and nothing is trimmed. A numeric
/// argument is read as character codes.
fn strcat(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "strcat")?;
    let cells: Vec<&CellArray> = args
        .iter()
        .filter_map(|a| match a {
            Value::Cell(c) => Some(&**c),
            _ => None,
        })
        .collect();
    if let Some(first) = cells.iter().find(|c| c.data.len() != 1).or(cells.first()) {
        let (rows, cols) = (first.rows, first.cols);
        let n = rows * cols;
        if cells
            .iter()
            .any(|c| c.data.len() != 1 && (c.rows, c.cols) != (rows, cols))
        {
            return Err(error::strcat_cells());
        }
        let mut total: usize = 0;
        for a in args {
            total = total.saturating_add(match a {
                Value::Cell(c) if c.data.len() == 1 => units_of(&c.data[0])
                    .map_or(0, <[f64]>::len)
                    .saturating_mul(n),
                Value::Cell(c) => c
                    .data
                    .iter()
                    .map(|v| units_of(v).map_or(0, <[f64]>::len))
                    .sum(),
                other => other.mat()?.data.len().saturating_mul(n),
            });
        }
        fits(total)?;
        let mut out = vec![Vec::new(); n];
        for a in args {
            for (k, item) in out.iter_mut().enumerate() {
                let u = match a {
                    Value::Cell(c) => {
                        let v = if c.data.len() == 1 {
                            &c.data[0]
                        } else {
                            &c.data[k]
                        };
                        units_of(v).ok_or_else(|| error::cell_not_text("strcat"))?
                    }
                    other => &other.mat()?.data,
                };
                item.extend_from_slice(u);
            }
        }
        let data = out.into_iter().map(chars).collect();
        return one(Value::cell(CellArray::new(rows, cols, data)));
    }
    // Every argument an array: trailing whitespace goes, then the rows are
    // joined. An empty argument adds nothing.
    let mut rows: Option<Vec<Vec<f64>>> = None;
    for a in args {
        let m = a.mat()?;
        if m.is_empty() {
            continue;
        }
        let mut these: Vec<Vec<f64>> = (0..m.rows).map(|r| row_units(m, r)).collect();
        if m.class == Class::Char {
            // A column of trailing whitespace goes only when it is
            // whitespace in every row, so the rows stay one length.
            let keep = (0..m.cols)
                .rev()
                .find(|&c| these.iter().any(|row| !is_ascii_white(row[c])))
                .map_or(0, |c| c + 1);
            these.iter_mut().for_each(|row| row.truncate(keep));
        } else {
            these.iter_mut().for_each(|row| {
                row.iter_mut()
                    .for_each(|u| *u = crate::value::code_unit(*u))
            });
        }
        match &mut rows {
            None => rows = Some(these),
            Some(acc) if acc.len() == these.len() => {
                for (dst, src) in acc.iter_mut().zip(these) {
                    dst.extend(src);
                }
            }
            Some(_) => return Err(error::strcat_rows()),
        }
    }
    match rows {
        None => one(Value::str("")),
        Some(rows) => one_as(char_rows(&rows)),
    }
}

/// The delimiters of `strsplit` or `strtok`: a char or a cell of chars,
/// escapes processed as `sprintf` processes them.
fn delimiters(v: &Value, name: &str) -> R<Vec<Vec<f64>>> {
    let one_text = |v: &Value| -> R<Vec<f64>> {
        let t = v.text().ok_or_else(|| error::arg_not_a_string(2, name))?;
        Ok(units(&escapes(&t)))
    };
    match v {
        Value::Cell(c) => c
            .data
            .iter()
            .map(|d| {
                if d.is_char() {
                    one_text(d)
                } else {
                    Err(error::cell_not_text(name))
                }
            })
            .collect(),
        other => Ok(vec![one_text(other)?]),
    }
}

/// The longest delimiter that starts at `s[i]`, by its length.
fn delimiter_at(s: &[f64], i: usize, delims: &[Vec<f64>]) -> Option<usize> {
    delims
        .iter()
        .filter(|d| !d.is_empty() && s[i..].starts_with(d))
        .map(Vec::len)
        .max()
}

/// `[C, matches] = strsplit(s, delim, 'CollapseDelimiters', tf)`. Runs of
/// delimiters collapse into one by default, as in MATLAB; the default
/// delimiter is any whitespace character.
fn strsplit(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "strsplit")?;
    let s = text_arg(args, 0, "strsplit")?;
    let mut rest = &args[1..];
    let delims = if rest.len() % 2 == 1 {
        let d = delimiters(&rest[0], "strsplit")?;
        rest = &rest[1..];
        d
    } else {
        [" ", "\u{c}", "\n", "\r", "\t", "\u{b}"]
            .iter()
            .map(|d| units(d))
            .collect()
    };
    let mut collapse = true;
    for pair in rest.chunks(2) {
        let opt = pair[0].text().unwrap_or_default();
        if !opt.eq_ignore_ascii_case("CollapseDelimiters") {
            return Err(error::unrecognized_option(&opt, "strsplit"));
        }
        collapse = pair[1].mat()?.truth()?;
    }
    // Two passes: the first counts the pieces, so that a cell past the byte
    // budget is refused before any piece is copied. `strsplit` of a long
    // run of delimiters made one cell element per character (cycle 13b's
    // review).
    let mut count = 1;
    split_runs(s, &delims, collapse, |_, _, _| count += 1);
    check_cell(1, count)?;
    let (mut pieces, mut matches) = (Vec::with_capacity(count), Vec::with_capacity(count - 1));
    let last = split_runs(s, &delims, collapse, |start, from, to| {
        pieces.push(chars(s[start..from].to_vec()));
        matches.push(chars(s[from..to].to_vec()));
    });
    pieces.push(chars(s[last..].to_vec()));
    Ok(vec![
        Value::cell(CellArray::row(pieces)),
        Value::cell(CellArray::row(matches)),
    ])
}

/// `strsplit`'s walk over `s`: `visit(start, from, to)` for each run of
/// delimiters `from..to`, which ends the piece `start..from`. Answers where
/// the last piece starts.
fn split_runs(
    s: &[f64],
    delims: &[Vec<f64>],
    collapse: bool,
    mut visit: impl FnMut(usize, usize, usize),
) -> usize {
    let (mut start, mut i) = (0, 0);
    while i < s.len() {
        let Some(n) = delimiter_at(s, i, delims) else {
            i += 1;
            continue;
        };
        let from = i;
        i += n;
        if collapse {
            while i < s.len() {
                match delimiter_at(s, i, delims) {
                    Some(n) => i += n,
                    None => break,
                }
            }
        }
        visit(start, from, i);
        start = i;
    }
    start
}

/// `strjoin(C, delim)`: the texts of `C` in order, `delim` (escapes
/// processed; a space by default, or a cell of one delimiter per gap)
/// between each two.
fn strjoin(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "strjoin")?;
    at_most(args, 2, "strjoin")?;
    let Value::Cell(c) = &args[0] else {
        return Err(error::arg_not_a_cell(1, "strjoin"));
    };
    let gaps = c.data.len().saturating_sub(1);
    // One delimiter for every gap, or a cell of one per gap; the one is
    // never copied per gap, so a long one between many texts is judged
    // before it is repeated.
    let (one_delim, per_gap): (Vec<f64>, Vec<Vec<f64>>) = match args.get(1) {
        None => (units(" "), Vec::new()),
        Some(Value::Cell(d)) => {
            let d = delimiters(&Value::Cell(d.clone()), "strjoin")?;
            if d.len() != gaps {
                return Err(error::strjoin_delimiters());
            }
            (Vec::new(), d)
        }
        Some(d) => (delimiters(d, "strjoin")?.remove(0), Vec::new()),
    };
    let delim = |k: usize| per_gap.get(k).unwrap_or(&one_delim);
    let texts = c
        .data
        .iter()
        .map(|v| units_of(v).ok_or_else(|| error::cell_not_text("strjoin")))
        .collect::<R<Vec<_>>>()?;
    let total = texts
        .iter()
        .map(|t| t.len())
        .chain((0..gaps).map(|k| delim(k).len()))
        .fold(0usize, usize::saturating_add);
    fits(total)?;
    let mut out = Vec::with_capacity(total);
    for (k, v) in texts.into_iter().enumerate() {
        out.extend_from_slice(v);
        if k < gaps {
            out.extend_from_slice(delim(k));
        }
    }
    one(chars(out))
}

// ---- replacing, trimming and case ------------------------------------------

/// Every start of `pat` in `s`, overlapping ones included, by
/// Knuth-Morris-Pratt, so the search is linear in both: a naive scan of a
/// long run of `a`s for a long run of `a`s is quadratic.
fn find_all(s: &[f64], pat: &[f64]) -> Vec<usize> {
    if pat.is_empty() || pat.len() > s.len() {
        return Vec::new();
    }
    // fail[k]: the length of the longest proper border of pat[..=k].
    let mut fail = vec![0usize; pat.len()];
    let mut k = 0;
    for i in 1..pat.len() {
        while k > 0 && pat[i] != pat[k] {
            k = fail[k - 1];
        }
        if pat[i] == pat[k] {
            k += 1;
        }
        fail[i] = k;
    }
    let mut out = Vec::new();
    let mut k = 0;
    for (i, u) in s.iter().enumerate() {
        while k > 0 && *u != pat[k] {
            k = fail[k - 1];
        }
        if *u == pat[k] {
            k += 1;
        }
        if k == pat.len() {
            out.push(i + 1 - k);
            k = fail[k - 1];
        }
    }
    out
}

/// `strrep`'s replacement, overlapping matches each replaced, as MATLAB
/// replaces them: `strrep('2222', '22', '*')` is `'***'`.
fn replace(s: &[f64], old: &[f64], new: &[f64]) -> R<Vec<f64>> {
    let starts = find_all(s, old);
    if starts.is_empty() {
        return Ok(s.to_vec());
    }
    // At most every unit kept, and every replacement.
    fits(
        s.len()
            .saturating_add(starts.len().saturating_mul(new.len())),
    )?;
    let mut out = Vec::with_capacity(s.len());
    let mut next = starts.iter().peekable();
    let mut covered = 0;
    for (i, u) in s.iter().enumerate() {
        if next.peek() == Some(&&i) {
            next.next();
            out.extend_from_slice(new);
            covered = i + old.len();
        } else if i >= covered {
            out.push(*u);
        }
    }
    Ok(out)
}

fn strrep(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 3, "strrep")?;
    at_most(args, 3, "strrep")?;
    let old = text_arg(args, 1, "strrep")?.to_vec();
    let new = text_arg(args, 2, "strrep")?.to_vec();
    match &args[0] {
        Value::Cell(c) => one(map_cell(c, "strrep", |s| {
            Ok(chars(replace(s, &old, &new)?))
        })?),
        _ => {
            let s = text_arg(args, 0, "strrep")?;
            one(chars(replace(s, &old, &new)?))
        }
    }
}

/// Whitespace and NUL, which `strtrim` removes.
fn trims(u: f64) -> bool {
    u == 0.0 || is_white(u)
}

/// A char matrix less the columns at either end that are whitespace in
/// every row.
fn trim_matrix(m: &Matrix) -> Matrix {
    let blank = |c: usize| (0..m.rows).all(|r| trims(m.get(r, c)));
    let first = (0..m.cols).find(|&c| !blank(c));
    let Some(first) = first else {
        return Matrix::new(m.rows.min(1), 0, Vec::new()).with_class(Class::Char);
    };
    let last = (0..m.cols).rev().find(|&c| !blank(c)).unwrap_or(first);
    let rows: Vec<Vec<f64>> = (0..m.rows)
        .map(|r| (first..=last).map(|c| m.get(r, c)).collect())
        .collect();
    char_rows(&rows)
}

fn strtrim(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "strtrim")?;
    at_most(args, 1, "strtrim")?;
    match &args[0] {
        Value::Cell(c) => one(map_cell(c, "strtrim", |s| {
            Ok(Value::Mat(trim_matrix(&Matrix::row(s.to_vec()))))
        })?),
        Value::Mat(m) if m.class == Class::Char => {
            if m.is_empty() {
                return one(args[0].clone());
            }
            one_as(trim_matrix(m))
        }
        _ => Err(error::arg_not_a_string(1, "strtrim")),
    }
}

/// `upper` and `lower`: a char mapped unit by unit, a cell element by
/// element, and any other array unchanged, as MATLAB returns it.
fn change_case(args: &[Value], name: &str, f: fn(f64) -> f64) -> R<Vec<Value>> {
    need(args, 1, name)?;
    at_most(args, 1, name)?;
    match &args[0] {
        Value::Cell(c) => one(map_cell(c, name, |s| {
            Ok(chars(s.iter().map(|u| f(*u)).collect()))
        })?),
        Value::Mat(m) if m.class == Class::Char => one_as(m.map(f).with_class(Class::Char)),
        Value::Mat(_) => one(args[0].clone()),
        _ => Err(error::arg_not_a_string(1, name)),
    }
}

fn upper(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    change_case(args, "upper", upper_unit)
}

fn lower(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    change_case(args, "lower", lower_unit)
}

// ---- comparison and search ----------------------------------------------------

/// Two values as one comparison of the `strcmp` family judges them: both
/// chars of one size and the same units, case-folded when `icase`, and
/// with `n`, only their first `n` units, which must be there in both or
/// be missing from both.
fn same_text(a: &Value, b: &Value, n: Option<usize>, icase: bool) -> bool {
    let (Value::Mat(x), Value::Mat(y)) = (a, b) else {
        return false;
    };
    if x.class != Class::Char || y.class != Class::Char {
        return false;
    }
    let eq = |p: &[f64], q: &[f64]| {
        p.len() == q.len()
            && p.iter().zip(q).all(|(u, v)| {
                if icase {
                    lower_unit(*u) == lower_unit(*v)
                } else {
                    u == v
                }
            })
    };
    match n {
        None => {
            ((x.rows, x.cols) == (y.rows, y.cols) || (x.is_empty() && y.is_empty()))
                && eq(&x.data, &y.data)
        }
        Some(n) => eq(
            &x.data[..n.min(x.data.len())],
            &y.data[..n.min(y.data.len())],
        ),
    }
}

fn compare(args: &[Value], name: &str, counted: bool, icase: bool) -> R<Vec<Value>> {
    let want = if counted { 3 } else { 2 };
    need(args, want, name)?;
    at_most(args, want, name)?;
    let n = if counted {
        match mat(args, 2, name)?.scalar_value() {
            Some(n) if n >= 0.0 && n.fract() == 0.0 => Some(n.min(usize::MAX as f64) as usize),
            _ => return Err(error::strncmp_count(name)),
        }
    } else {
        None
    };
    let test = |a: &Value, b: &Value| f64::from(u8::from(same_text(a, b, n, icase)));
    let logical = |rows: usize, cols: usize, data: Vec<f64>| {
        one_as(Matrix::new(rows, cols, data).with_class(Class::Logical))
    };
    match (&args[0], &args[1]) {
        (Value::Cell(a), Value::Cell(b)) => {
            if a.data.len() == 1 && b.data.len() != 1 {
                return logical(
                    b.rows,
                    b.cols,
                    b.data.iter().map(|y| test(&a.data[0], y)).collect(),
                );
            }
            if b.data.len() == 1 {
                return logical(
                    a.rows,
                    a.cols,
                    a.data.iter().map(|x| test(x, &b.data[0])).collect(),
                );
            }
            if (a.rows, a.cols) != (b.rows, b.cols) {
                return Err(error::strcmp_sizes());
            }
            logical(
                a.rows,
                a.cols,
                a.data
                    .iter()
                    .zip(&b.data)
                    .map(|(x, y)| test(x, y))
                    .collect(),
            )
        }
        (Value::Cell(a), y) => logical(a.rows, a.cols, a.data.iter().map(|x| test(x, y)).collect()),
        (x, Value::Cell(b)) => logical(b.rows, b.cols, b.data.iter().map(|y| test(x, y)).collect()),
        (x, y) => one_as(Matrix::from_bool(test(x, y) != 0.0)),
    }
}

fn strcmp(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    compare(args, "strcmp", false, false)
}

fn strcmpi(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    compare(args, "strcmpi", false, true)
}

fn strncmp(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    compare(args, "strncmp", true, false)
}

fn strncmpi(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    compare(args, "strncmpi", true, true)
}

/// `strfind(s, pattern)`: every one-based start, overlapping ones
/// included, as a row; `1x0` when there is none.
fn strfind(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "strfind")?;
    at_most(args, 2, "strfind")?;
    let pat = text_arg(args, 1, "strfind")?.to_vec();
    let starts = |s: &[f64]| {
        Value::Mat(Matrix::row(
            find_all(s, &pat)
                .into_iter()
                .map(|i| (i + 1) as f64)
                .collect(),
        ))
    };
    match &args[0] {
        Value::Cell(c) => one(map_cell(c, "strfind", |s| Ok(starts(s)))?),
        _ => one(starts(text_arg(args, 0, "strfind")?)),
    }
}

/// `[tok, rem] = strtok(s, delims)`: leading delimiters skipped, the token
/// up to the next one, and the rest from that delimiter on. Whitespace is
/// the default delimiter.
fn strtok(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "strtok")?;
    at_most(args, 2, "strtok")?;
    let s = text_arg(args, 0, "strtok")?;
    let delims: Vec<f64> = match args.get(1) {
        Some(_) => text_arg(args, 1, "strtok")?.to_vec(),
        None => Vec::new(),
    };
    let is_delim = |u: f64| {
        if args.len() > 1 {
            delims.contains(&u)
        } else {
            is_white(u) || u == 0.0
        }
    };
    let start = s.iter().position(|u| !is_delim(*u)).unwrap_or(s.len());
    let end = s[start..]
        .iter()
        .position(|u| is_delim(*u))
        .map_or(s.len(), |k| start + k);
    Ok(vec![
        chars(s[start..end].to_vec()),
        chars(s[end..].to_vec()),
    ])
}

fn isspace(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    char_predicate(args, "isspace", is_white)
}

fn isletter(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    char_predicate(args, "isletter", |u| {
        char_at(u).is_some_and(char::is_alphabetic)
    })
}

/// A logical of the argument's shape: `test` of each unit of a char, and
/// false throughout for any other array.
fn char_predicate(args: &[Value], name: &str, test: fn(f64) -> bool) -> R<Vec<Value>> {
    need(args, 1, name)?;
    at_most(args, 1, name)?;
    let m = args[0].mat()?;
    let is_char = m.class == Class::Char;
    let data = m
        .data
        .iter()
        .map(|u| f64::from(u8::from(is_char && test(*u))))
        .collect();
    one_as(Matrix::new(m.rows, m.cols, data).with_class(Class::Logical))
}

/// `blanks(n)`: a `1xn` char of spaces.
fn blanks(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "blanks")?;
    at_most(args, 1, "blanks")?;
    let n = match mat(args, 0, "blanks")?.scalar_value() {
        Some(n) if n >= 0.0 && n.fract() == 0.0 => n,
        _ => return Err(error::arg_nonneg_int(1, "blanks")),
    };
    let (_, n) = check_shape(1.0, n)?;
    one(chars(vec![32.0; n]))
}

// ---- numbers as text ------------------------------------------------------------

/// `num2str` of a scalar with no precision: an integer in full, anything
/// else `%g` with four decimals' worth of significant digits past its
/// integer part, as MATLAB's `max(log10(|x|) + 5, 5)` gives.
pub fn num2str(v: f64) -> String {
    // Non-finite first: log10(Inf) is Inf, whose cast to i32 saturates.
    if !v.is_finite() {
        return nonfinite(v);
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        fmt_g(v, significant(v.abs()))
    }
}

/// `max(floor(log10(m)) + 5, 5)`: the significant digits `num2str` gives a
/// value of magnitude `m`.
fn significant(m: f64) -> usize {
    if m >= 1.0 && m.is_finite() {
        (m.log10().floor() as i32).saturating_add(5).max(5) as usize
    } else {
        5
    }
}

/// The precision of `num2str(x, n)`: a positive integer. Any `n` past 800 is
/// clamped, because no double has more than about 770 significant digits to
/// show, so `num2str(pi, 1e9)` neither panics nor allocates a gigabyte.
fn precision(p: &Matrix, name: &str) -> R<usize> {
    match p.scalar_value() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(n.min(800.0) as usize),
        _ if name == "num2str" => Err(error::num2str_precision()),
        _ => Err(error::precision_arg(name)),
    }
}

/// The rows of a numeric matrix, each element right-aligned in a column
/// `width` wide by `text`, and then the columns of spaces every row
/// starts with removed: MATLAB's `strtrim` of the `sprintf` rows.
fn aligned(m: &Matrix, width: usize, text: impl Fn(f64) -> String) -> R<Matrix> {
    check_shape(m.rows as f64, m.cols as f64 * width as f64)?;
    let rows: Vec<Vec<f64>> = (0..m.rows)
        .map(|r| {
            let mut row = String::new();
            for c in 0..m.cols {
                let t = text(m.get(r, c));
                row.push_str(&" ".repeat(width.saturating_sub(t.chars().count())));
                row.push_str(&t);
            }
            units(&row)
        })
        .collect();
    Ok(trim_leading(rows))
}

/// Rows of text less the leading spaces they all share, padded to one
/// length.
fn trim_leading(rows: Vec<Vec<f64>>) -> Matrix {
    let lead = rows
        .iter()
        .map(|r| r.iter().take_while(|u| **u == 32.0).count())
        .min()
        .unwrap_or(0);
    let rows: Vec<Vec<f64>> = rows.into_iter().map(|r| r[lead..].to_vec()).collect();
    let mut m = char_rows(&rows);
    // Trailing columns of spaces go too.
    let keep = (0..m.cols)
        .rev()
        .find(|&c| (0..m.rows).any(|r| m.get(r, c) != 32.0))
        .map_or(0, |c| c + 1);
    if keep < m.cols {
        let rows: Vec<Vec<f64>> = (0..m.rows)
            .map(|r| row_units(&m, r)[..keep].to_vec())
            .collect();
        m = char_rows(&rows);
    }
    m
}

/// `num2str` of a non-scalar (QA D13): one char row per matrix row. An
/// integer matrix gives each column the width of its widest magnitude plus
/// two; any other gives each element `%g` with the significant digits of
/// the largest magnitude, in a column that many plus seven wide, one more
/// with a negative element, as MATLAB's `num2str` lays them out.
fn num2str_matrix(m: &Matrix) -> R<Matrix> {
    let finite = m.data.iter().filter(|v| v.is_finite()).map(|v| v.abs());
    let largest = finite.fold(0.0f64, f64::max);
    let negative = m.data.iter().any(|v| *v < 0.0);
    let integers = m
        .data
        .iter()
        .all(|v| !v.is_nan() && v.fract() == 0.0 || v.is_infinite());
    if integers {
        let texts = |v: f64| {
            if v == 0.0 {
                "0".to_string()
            } else if v.is_finite() {
                format!("{:.0}", v)
            } else {
                nonfinite(v)
            }
        };
        let digits = m
            .data
            .iter()
            .map(|v| texts(v.abs()).len())
            .max()
            .unwrap_or(1);
        aligned(m, digits + 2, texts)
    } else {
        let p = significant(largest);
        aligned(m, p + 7 + usize::from(negative), |v| fmt_g(v, p))
    }
}

fn num2str_fn(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 2, "num2str")?;
    need(args, 1, "num2str")?;
    if args[0].is_char() {
        return one(args[0].clone());
    }
    let m = args[0].mat()?;
    if m.is_empty() {
        return one(Value::str(""));
    }
    match args.get(1) {
        None if m.is_scalar() => one(Value::str(&num2str(m.data[0]))),
        None => one_as(num2str_matrix(m)?),
        Some(fmt) if fmt.is_char() => {
            // sprintf(formatSpec, row) for each row, the leading spaces
            // trimmed even when the format asked for them: num2str(42.67,
            // '% 10.2f') is '42.67' on the MATLAB page.
            let rows = (0..m.rows)
                .map(|r| {
                    let row = Value::Mat(Matrix::row(row_units(m, r)));
                    Ok(units(&format_printf(&[fmt.clone(), row])?))
                })
                .collect::<R<Vec<_>>>()?;
            let t = trim_leading(rows);
            one_as(if m.rows == 1 {
                let text: Vec<f64> = t
                    .data
                    .iter()
                    .copied()
                    .skip_while(|u| is_white(*u))
                    .collect();
                Matrix::new(1, text.len(), text).with_class(Class::Char)
            } else {
                t
            })
        }
        Some(p) => {
            let n = precision(p.mat()?, "num2str")?;
            if m.is_scalar() {
                return one(Value::str(&fmt_g(m.data[0], n)));
            }
            let negative = m.data.iter().any(|v| *v < 0.0);
            one_as(aligned(m, n + 7 + usize::from(negative), |v| fmt_g(v, n))?)
        }
    }
}

/// `int2str(x)`: `x` rounded half away from zero, as `num2str` writes it.
fn int2str(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "int2str")?;
    at_most(args, 1, "int2str")?;
    let m = args[0].mat()?.map(f64::round).with_class(Class::Double);
    if m.is_empty() {
        return one(Value::str(""));
    }
    if m.is_scalar() {
        return one(Value::str(&num2str(m.data[0])));
    }
    one_as(num2str_matrix(&m)?)
}

/// The number a text denotes, as `str2double` reads one: optional
/// whitespace, a sign, digits with an optional point (commas allowed in
/// the integer part as thousands separators), an optional exponent
/// (`e`, `E`, `d` or `D`), or `Inf`, `Infinity` or `NaN` in any case.
/// `None` for anything else, a complex number included.
pub fn parse_double(text: &str) -> Option<f64> {
    let t = text.trim();
    let (negative, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let lower = body.to_ascii_lowercase();
    let v = if lower == "inf" || lower == "infinity" {
        f64::INFINITY
    } else if lower == "nan" {
        f64::NAN
    } else {
        let b = body.as_bytes();
        let mut i = 0;
        let mut mantissa = String::new();
        let mut digits = 0;
        while i < b.len() && (b[i].is_ascii_digit() || (b[i] == b',' && digits > 0)) {
            if b[i] != b',' {
                mantissa.push(b[i] as char);
                digits += 1;
            }
            i += 1;
        }
        if b.get(i.wrapping_sub(1)) == Some(&b',') {
            return None;
        }
        if i < b.len() && b[i] == b'.' {
            mantissa.push('.');
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                mantissa.push(b[i] as char);
                digits += 1;
                i += 1;
            }
        }
        if digits == 0 {
            return None;
        }
        if i < b.len() && matches!(b[i], b'e' | b'E' | b'd' | b'D') {
            mantissa.push('e');
            i += 1;
            if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
                mantissa.push(b[i] as char);
                i += 1;
            }
            let from = i;
            while i < b.len() && b[i].is_ascii_digit() {
                mantissa.push(b[i] as char);
                i += 1;
            }
            if i == from {
                return None;
            }
        }
        if i != b.len() {
            return None;
        }
        mantissa.parse::<f64>().ok()?
    };
    Some(if negative { -v } else { v })
}

/// `str2double(s)`: a text's number or `NaN`; a cell's element by element,
/// in its shape, anything but text `NaN`; any other value `NaN`.
fn str2double(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "str2double")?;
    at_most(args, 1, "str2double")?;
    let of = |v: &Value| match v {
        Value::Mat(m) if m.class == Class::Char && m.rows <= 1 => {
            parse_double(&m.text()).unwrap_or(f64::NAN)
        }
        _ => f64::NAN,
    };
    match &args[0] {
        Value::Cell(c) => one_as(Matrix::new(c.rows, c.cols, c.data.iter().map(of).collect())),
        v => one_as(Matrix::scalar(of(v))),
    }
}

/// `[x, ok] = str2num(s)`: the text read as the elements of a matrix, the
/// rows of a char matrix joined with `;`; `[]` and false when it is not
/// one. See `Interp::str2num_value` for what it reads.
fn str2num(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "str2num")?;
    at_most(args, 1, "str2num")?;
    let m = args[0].mat()?;
    if m.class != Class::Char {
        return Err(error::arg_not_a_string(1, "str2num"));
    }
    let text = (0..m.rows)
        .map(|r| m.row_text(r))
        .collect::<Vec<_>>()
        .join(";");
    match it.str2num_value(&text) {
        Some(v) => Ok(vec![v, Value::Mat(Matrix::from_bool(true))]),
        None => Ok(vec![
            Value::Mat(Matrix::empty()),
            Value::Mat(Matrix::from_bool(false)),
        ]),
    }
}

/// `mat2str(A)` and `mat2str(A, n)`: the matrix in MATLAB syntax, `%.15g`
/// (or `%.ng`) for each number, `true` and `false` for a logical, quoted
/// text for a char. An empty is `zeros(r,c)`, `false(r,c)`, `''` or
/// `char(zeros(r,c))`.
fn mat2str(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "mat2str")?;
    at_most(args, 2, "mat2str")?;
    let Value::Mat(m) = &args[0] else {
        return Err(error::mat2str_input());
    };
    let n = match args.get(1) {
        Some(p) => precision(p.mat()?, "mat2str")?,
        None => 15,
    };
    let (r, c) = (m.rows, m.cols);
    if m.is_empty() {
        let s = match m.class {
            Class::Char if r == 0 && c == 0 => "''".to_string(),
            Class::Char => format!("char(zeros({r},{c}))"),
            Class::Logical => format!("false({r},{c})"),
            Class::Double => format!("zeros({r},{c})"),
        };
        return one(Value::str(&s));
    }
    if m.class == Class::Char {
        let rows: Vec<String> = (0..r)
            .map(|k| format!("'{}'", m.row_text(k).replace('\'', "''")))
            .collect();
        let s = if r == 1 {
            rows.concat()
        } else {
            format!("[{}]", rows.join(";"))
        };
        return one(Value::str(&s));
    }
    let item = |v: f64| match m.class {
        Class::Logical => (if v != 0.0 { "true" } else { "false" }).to_string(),
        _ => fmt_g(v, n),
    };
    let mut body = String::new();
    for i in 0..r {
        if i > 0 {
            body.push(';');
        }
        for j in 0..c {
            if j > 0 {
                body.push(' ');
            }
            body.push_str(&item(m.get(i, j)));
        }
        fits(body.len())?;
    }
    one(Value::str(&if m.is_scalar() {
        body
    } else {
        format!("[{body}]")
    }))
}

// ---- regular expressions ----------------------------------------------------

/// What `regexp` hands back, in the order its options ask.
#[derive(Clone, Copy, PartialEq)]
enum Out {
    Start,
    End,
    TokenExtents,
    Match,
    Tokens,
    Names,
    Split,
}

/// The options of `regexp` and `regexprep`.
struct Options {
    outs: Vec<Out>,
    once: bool,
    icase: bool,
    empty: bool,
}

fn options(args: &[Value], name: &str, outputs: bool) -> R<Options> {
    let mut o = Options {
        outs: Vec::new(),
        once: false,
        icase: false,
        empty: false,
    };
    for (k, a) in args.iter().enumerate() {
        let opt = a
            .text()
            .ok_or_else(|| error::arg_not_a_string(k + 1 + if outputs { 2 } else { 3 }, name))?;
        let out = match opt.to_ascii_lowercase().as_str() {
            "match" => Some(Out::Match),
            "tokens" => Some(Out::Tokens),
            "names" => Some(Out::Names),
            "start" => Some(Out::Start),
            "end" => Some(Out::End),
            "split" => Some(Out::Split),
            "tokenextents" => Some(Out::TokenExtents),
            "once" => {
                o.once = true;
                None
            }
            "ignorecase" => {
                o.icase = true;
                None
            }
            "matchcase" => {
                o.icase = false;
                None
            }
            "emptymatch" => {
                o.empty = true;
                None
            }
            "noemptymatch" => {
                o.empty = false;
                None
            }
            _ => return Err(error::unrecognized_option(&opt, name)),
        };
        match out {
            Some(out) if outputs => o.outs.push(out),
            Some(_) => return Err(error::unrecognized_option(&opt, name)),
            None => {}
        }
    }
    Ok(o)
}

fn code_units(u: &[f64]) -> Vec<u32> {
    u.iter()
        .map(|v| crate::value::code_unit(*v) as u32)
        .collect()
}

fn compile(pattern: &[f64], icase: bool) -> R<Regex> {
    Regex::new(&code_units(pattern), icase)
}

/// The text of `s[a..b]` as a char row.
fn piece(s: &[f64], span: Option<(usize, usize)>) -> Value {
    match span {
        Some((a, b)) => chars(s[a..b].to_vec()),
        None => chars(Vec::new()),
    }
}

/// The tokens of one match: each capture group's text, or the whole match
/// when the pattern has no group, as MATLAB gives it.
fn tokens(re: &Regex, s: &[f64], g: &Groups) -> Vec<Value> {
    if re.captures() == 0 {
        vec![piece(s, g[0])]
    } else {
        g[1..].iter().map(|span| piece(s, *span)).collect()
    }
}

/// The extents of one match's tokens, one `[start end]` row each,
/// one-based and inclusive; a group that took no part is `[start-of-match
/// start-of-match-minus-one]`, an empty extent where the match starts.
fn extents(re: &Regex, g: &Groups) -> Matrix {
    let spans: Vec<(usize, usize)> = if re.captures() == 0 {
        vec![g[0].unwrap_or((0, 0))]
    } else {
        let at = g[0].map_or(0, |s| s.0);
        g[1..].iter().map(|s| s.unwrap_or((at, at))).collect()
    };
    let n = spans.len();
    let mut data = vec![0.0; n * 2];
    for (k, (a, b)) in spans.into_iter().enumerate() {
        data[k] = (a + 1) as f64;
        data[n + k] = b as f64;
    }
    Matrix::new(n, 2, data)
}

/// `regexp` of one subject: every requested output.
fn regexp_one(re: &Regex, s: &[f64], o: &Options) -> R<Vec<Value>> {
    let text = code_units(s);
    let found = re.find(&text, o.empty, if o.once { 1 } else { usize::MAX })?;
    // Nested groups can each hold the whole match, so the tokens can be a
    // hundred times the subject.
    let span_len = |g: &Option<(usize, usize)>| g.map_or(0, |(a, b)| b - a);
    let token_units = found
        .iter()
        .map(|g| g.iter().map(span_len).fold(0usize, usize::saturating_add))
        .fold(0usize, usize::saturating_add);
    fits(token_units)?;
    let outs: Vec<Out> = if o.outs.is_empty() {
        vec![
            Out::Start,
            Out::End,
            Out::TokenExtents,
            Out::Match,
            Out::Tokens,
            Out::Names,
            Out::Split,
        ]
    } else {
        o.outs.clone()
    };
    let span = |g: &Groups| g[0].unwrap_or((0, 0));
    let fields: Vec<String> = re.names.iter().map(|(n, _)| n.clone()).collect();
    let name_values =
        |g: &Groups| -> Vec<Value> { re.names.iter().map(|(_, k)| piece(s, g[*k])).collect() };
    // A cell or struct output holds an element per match, and a pattern that
    // matches the empty text matches at every position (cycle 13b's review).
    if !o.once {
        let n = found.len();
        for out in &outs {
            match out {
                Out::TokenExtents | Out::Match | Out::Tokens => {
                    check_cell(1, n)?;
                }
                Out::Split => {
                    check_cell(1, n + 1)?;
                }
                Out::Names if !fields.is_empty() && n > 1 => {
                    check_struct(1, n, fields.len())?;
                }
                _ => {}
            }
        }
    }
    Ok(outs
        .iter()
        .map(|out| match (out, o.once) {
            (Out::Start, true) => match found.first() {
                Some(g) => Value::Mat(Matrix::scalar((span(g).0 + 1) as f64)),
                None => Value::Mat(Matrix::empty()),
            },
            (Out::End, true) => match found.first() {
                Some(g) => Value::Mat(Matrix::scalar(span(g).1 as f64)),
                None => Value::Mat(Matrix::empty()),
            },
            (Out::Start, false) => Value::Mat(Matrix::row(
                found.iter().map(|g| (span(g).0 + 1) as f64).collect(),
            )),
            (Out::End, false) => Value::Mat(Matrix::row(
                found.iter().map(|g| span(g).1 as f64).collect(),
            )),
            (Out::TokenExtents, true) => match found.first() {
                Some(g) => Value::Mat(extents(re, g)),
                None => Value::Mat(Matrix::empty()),
            },
            (Out::TokenExtents, false) => Value::cell(CellArray::row(
                found.iter().map(|g| Value::Mat(extents(re, g))).collect(),
            )),
            (Out::Match, true) => match found.first() {
                Some(g) => piece(s, g[0]),
                None => Value::str(""),
            },
            (Out::Match, false) => Value::cell(CellArray::row(
                found.iter().map(|g| piece(s, g[0])).collect(),
            )),
            (Out::Tokens, true) => match found.first() {
                Some(g) => Value::cell(CellArray::row(tokens(re, s, g))),
                None => Value::cell(CellArray::new(1, 0, Vec::new())),
            },
            (Out::Tokens, false) => Value::cell(CellArray::row(
                found
                    .iter()
                    .map(|g| Value::cell(CellArray::row(tokens(re, s, g))))
                    .collect(),
            )),
            (Out::Names, once) => {
                if fields.is_empty() {
                    Value::strukt(StructArray::scalar(Vec::new(), Vec::new()))
                } else if once || found.len() == 1 {
                    let values = match found.first() {
                        Some(g) => name_values(g),
                        None => vec![Value::str(""); fields.len()],
                    };
                    Value::strukt(StructArray::scalar(fields.clone(), values))
                } else {
                    let elems: Vec<Vec<Value>> = found.iter().map(name_values).collect();
                    let n = elems.len();
                    let rows = usize::from(n > 0);
                    Value::strukt(StructArray::new(rows, n, fields.clone(), elems))
                }
            }
            (Out::Split, _) => {
                let mut parts = Vec::with_capacity(found.len() + 1);
                let mut at = 0;
                for g in &found {
                    let (a, b) = span(g);
                    parts.push(chars(s[at..a].to_vec()));
                    at = b;
                }
                parts.push(chars(s[at..].to_vec()));
                Value::cell(CellArray::row(parts))
            }
        })
        .collect())
}

/// `regexp(s, expr, options...)`. With no output option the outputs are
/// MATLAB's order: start, end, token extents, match, tokens, names, split.
fn regexp(_: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(args, 2, "regexp")?;
    let pattern = text_arg(args, 1, "regexp")?;
    let o = options(&args[2..], "regexp", true)?;
    let re = compile(pattern, o.icase)?;
    match &args[0] {
        Value::Cell(c) => {
            let count = if o.outs.is_empty() {
                nargout.clamp(1, 7)
            } else {
                o.outs.len()
            };
            let mut per: Vec<Vec<Value>> = vec![Vec::with_capacity(c.data.len()); count];
            for v in &c.data {
                let s = units_of(v).ok_or_else(|| error::cell_not_text("regexp"))?;
                for (k, out) in regexp_one(&re, s, &o)?.into_iter().take(count).enumerate() {
                    per[k].push(out);
                }
            }
            Ok(per
                .into_iter()
                .map(|items| Value::cell(CellArray::new(c.rows, c.cols, items)))
                .collect())
        }
        _ => regexp_one(&re, text_arg(args, 0, "regexp")?, &o),
    }
}

/// One part of a `regexprep` replacement.
enum Part {
    Text(Vec<f64>),
    Group(usize),
}

/// A `regexprep` replacement, parsed once, with where each token is used,
/// so that sizing a result costs the matches times the tokens and building
/// it costs what it writes, never the matches times the parts: a
/// replacement of 20,000 `$0`s over 100,000 matches is two billion parts.
struct Replacement {
    /// The parts in order, with no empty text, no two texts in a row and no
    /// token the pattern does not have, which is always empty.
    parts: Vec<Part>,
    /// The code units of all the text parts.
    text_len: usize,
    /// Where the text parts are in `parts`.
    texts: Vec<usize>,
    /// Each token the parts use, and where in `parts`.
    groups: Vec<(usize, Vec<usize>)>,
}

/// Whether `c` can be in the name of a named token.
fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A replacement text as parts: `$0` the whole match, `$N` token `N`,
/// `$<name>` a named token, the escapes `\n` `\t` `\r` `\f` `\v` `\a` and
/// `\\`, `\$` a dollar sign, and any other `\c` the character `c`. A
/// `$<` reads its name only as far as the first character a name cannot
/// hold, so the parse is linear however many `$<` the text has.
fn replacement(rep: &[f64], re: &Regex) -> Replacement {
    let mut parts = Vec::new();
    let mut text = Vec::new();
    let mut i = 0;
    let at = |k: usize| rep.get(k).copied().and_then(char_at);
    let group = |parts: &mut Vec<Part>, text: &mut Vec<f64>, k: usize| {
        if k <= re.captures() {
            if !text.is_empty() {
                parts.push(Part::Text(std::mem::take(text)));
            }
            parts.push(Part::Group(k));
        }
    };
    while i < rep.len() {
        match at(i) {
            Some('\\') if i + 1 < rep.len() => {
                text.push(match at(i + 1) {
                    Some('n') => 10.0,
                    Some('t') => 9.0,
                    Some('r') => 13.0,
                    Some('f') => 12.0,
                    Some('v') => 11.0,
                    Some('a') => 7.0,
                    _ => rep[i + 1],
                });
                i += 2;
            }
            Some('$') if at(i + 1).is_some_and(|c| c.is_ascii_digit()) => {
                let mut j = i + 1;
                let mut n: usize = 0;
                while let Some(d) = at(j).and_then(|c| c.to_digit(10)) {
                    n = n.saturating_mul(10).saturating_add(d as usize);
                    j += 1;
                }
                group(&mut parts, &mut text, n);
                i = j;
            }
            Some('$') if at(i + 1) == Some('<') => {
                let mut close = i + 2;
                while at(close).is_some_and(name_char) {
                    close += 1;
                }
                let named = (at(close) == Some('>'))
                    .then(|| {
                        let name: String = rep[i + 2..close]
                            .iter()
                            .filter_map(|u| char_at(*u))
                            .collect();
                        re.names.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
                    })
                    .flatten();
                match named {
                    Some(k) => {
                        group(&mut parts, &mut text, k);
                        i = close + 1;
                    }
                    None => {
                        text.push(rep[i]);
                        i += 1;
                    }
                }
            }
            _ => {
                text.push(rep[i]);
                i += 1;
            }
        }
    }
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    let mut text_len = 0;
    let mut texts = Vec::new();
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        match p {
            Part::Text(t) => {
                text_len += t.len();
                texts.push(i);
            }
            Part::Group(k) => match groups.iter_mut().find(|(g, _)| g == k) {
                Some((_, at)) => at.push(i),
                None => groups.push((*k, vec![i])),
            },
        }
    }
    Replacement {
        parts,
        text_len,
        texts,
        groups,
    }
}

fn replace_one(re: &Regex, s: &[f64], rep: &Replacement, o: &Options) -> R<Value> {
    let found = re.find(&code_units(s), o.empty, if o.once { 1 } else { usize::MAX })?;
    let len = |g: &Groups, k: usize| g.get(k).copied().flatten().map_or(0, |(x, y)| y - x);
    // Exactly what the parts would write, a token at a time, so the size a
    // refusal names is the one asked for.
    let mut total = s.len();
    for g in &found {
        total = total.saturating_add(rep.text_len);
        for (k, at) in &rep.groups {
            total = total.saturating_add(len(g, *k).saturating_mul(at.len()));
        }
    }
    fits(total)?;
    let mut out = Vec::with_capacity(s.len());
    let emit = |out: &mut Vec<f64>, g: &Groups, p: &Part| match p {
        Part::Text(t) => out.extend_from_slice(t),
        Part::Group(k) => {
            if let Some(Some((x, y))) = g.get(*k) {
                out.extend_from_slice(&s[*x..*y]);
            }
        }
    };
    let mut order: Vec<usize> = Vec::new();
    let mut at = 0;
    for g in &found {
        let (a, b) = g[0].unwrap_or((at, at));
        out.extend_from_slice(&s[at..a]);
        // The parts that write something here: every text, and each use of
        // a token that is not empty in this match.
        let live = rep.texts.len()
            + rep
                .groups
                .iter()
                .filter(|(k, _)| len(g, *k) > 0)
                .map(|(_, at)| at.len())
                .sum::<usize>();
        if 2 * live >= rep.parts.len() {
            for p in &rep.parts {
                emit(&mut out, g, p);
            }
        } else {
            // Mostly empty tokens: only the parts that write, in order.
            order.clear();
            order.extend_from_slice(&rep.texts);
            for (k, at) in &rep.groups {
                if len(g, *k) > 0 {
                    order.extend_from_slice(at);
                }
            }
            order.sort_unstable();
            for &i in &order {
                emit(&mut out, g, &rep.parts[i]);
            }
        }
        at = b;
    }
    out.extend_from_slice(&s[at..]);
    Ok(chars(out))
}

/// `regexprep(s, expr, rep, options...)`: every match replaced, or the
/// first with `'once'`; a cell subject element by element.
fn regexprep(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 3, "regexprep")?;
    let pattern = text_arg(args, 1, "regexprep")?;
    let rep = text_arg(args, 2, "regexprep")?;
    let o = options(&args[3..], "regexprep", false)?;
    let re = compile(pattern, o.icase)?;
    let parts = replacement(rep, &re);
    match &args[0] {
        Value::Cell(c) => one(map_cell(c, "regexprep", |s| {
            replace_one(&re, s, &parts, &o)
        })?),
        _ => one(replace_one(
            &re,
            text_arg(args, 0, "regexprep")?,
            &parts,
            &o,
        )?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `strsplit` counts its pieces in a first pass, so the byte budget is
    /// judged before any piece is copied (cycle 13b's review). The count
    /// must agree with the pieces the second pass makes; the refusal itself
    /// is `err_strsplit_over_byte_budget`, too large for a unit test.
    #[test]
    fn strsplit_counts_its_pieces_before_copying_them() {
        let comma = vec![units(",")];
        for (text, collapse, pieces) in [
            ("a,b,,c", true, 3),
            ("a,b,,c", false, 4),
            (",,", false, 3),
            ("", false, 1),
        ] {
            let s = units(text);
            let mut count = 1;
            split_runs(&s, &comma, collapse, |_, _, _| count += 1);
            assert_eq!(count, pieces, "{text} {collapse}");
            let mut it = Interp::with_output(Box::new(std::io::sink()));
            let mut args = vec![Value::str(text), Value::str(",")];
            if !collapse {
                args.push(Value::str("CollapseDelimiters"));
                args.push(Value::Mat(Matrix::scalar(0.0)));
            }
            let out = strsplit(&mut it, &args, 1).unwrap();
            assert_eq!(out[0].dims(), (1, pieces), "{text} {collapse}");
        }
    }

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    fn s(t: &str) -> Value {
        Value::str(t)
    }

    fn call(f: super::super::BuiltinFn, args: &[Value], nargout: usize) -> R<Vec<Value>> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        f(&mut it, args, nargout)
    }

    fn text(f: super::super::BuiltinFn, args: &[Value]) -> String {
        call(f, args, 1).unwrap()[0].text().expect("a char")
    }

    /// The rows of a char matrix.
    fn rows(v: &Value) -> Vec<String> {
        let m = v.mat().unwrap();
        (0..m.rows).map(|r| m.row_text(r)).collect()
    }

    fn mat_of(data: &[&[f64]]) -> Value {
        let r = data.len();
        let c = data[0].len();
        let mut v = vec![0.0; r * c];
        for (i, row) in data.iter().enumerate() {
            for (j, x) in row.iter().enumerate() {
                v[j * r + i] = *x;
            }
        }
        Value::Mat(Matrix::new(r, c, v))
    }

    #[test]
    fn num2str_handles_the_non_finite_values_that_used_to_panic() {
        assert_eq!(num2str(f64::INFINITY), "Inf");
        assert_eq!(num2str(f64::NEG_INFINITY), "-Inf");
        assert_eq!(num2str(f64::NAN), "NaN");
        assert_eq!(num2str(7.0), "7");
        assert_eq!(num2str(3.5), "3.5");
        assert_eq!(num2str(-0.25), "-0.25");
        assert_eq!(num2str(1.0e17), "100000000000000000");
        assert_eq!(num2str(std::f64::consts::PI), "3.1416");
    }

    #[test]
    fn num2str_with_a_precision_is_percent_g() {
        let p = |x: f64, n: f64| text(num2str_fn, &[num(x), num(n)]);
        assert_eq!(p(std::f64::consts::PI, 8.0), "3.1415927");
        assert_eq!(p(std::f64::consts::PI, 2.0), "3.1");
        assert_eq!(p(123456.0, 3.0), "1.23e+05");
        assert_eq!(p(7.0, 3.0), "7");
        assert_eq!(p(-0.5, 3.0), "-0.5");
        assert_eq!(p(f64::INFINITY, 3.0), "Inf");
        assert_eq!(p(1.0 / 3.0, 20.0), "0.33333333333333331483");
        let exact = "3.141592653589793115997963468544185161590576171875";
        assert_eq!(p(std::f64::consts::PI, 100.0), exact);
        // A huge precision is clamped rather than allocated.
        assert_eq!(p(std::f64::consts::PI, 1e9), exact);
        for bad in [0.0, -1.0, 1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                call(num2str_fn, &[num(1.0), num(bad)], 1).unwrap_err().msg,
                "Precision for 'num2str' must be a positive integer.",
                "{bad}"
            );
        }
    }

    #[test]
    fn num2str_with_a_format_is_sprintf_with_leading_spaces_trimmed() {
        let f = |x: f64, fmt: &str| text(num2str_fn, &[num(x), s(fmt)]);
        assert_eq!(f(std::f64::consts::PI, "%10.4f"), "3.1416");
        // The MATLAB page's own example: even a space flag is trimmed.
        assert_eq!(f(42.67, "% 10.2f"), "42.67");
        assert_eq!(f(5.0, "%d apples"), "5 apples");
        // A char input comes back unchanged.
        assert_eq!(text(num2str_fn, &[s("abc"), num(3.0)]), "abc");
        assert!(call(num2str_fn, &[num(1.0), num(2.0), num(3.0)], 1).is_err());
        // A matrix is formatted row by row.
        let m = mat_of(&[&[1.0, 2.0], &[3.0, 44.0]]);
        let out = &call(num2str_fn, &[m, s("%d,")], 1).unwrap()[0];
        assert_eq!(rows(out), ["1,2, ", "3,44,"]);
    }

    /// QA D13: one row per matrix row, each column as wide as the widest
    /// magnitude plus two, the shared leading spaces removed.
    #[test]
    fn num2str_of_a_matrix_is_one_row_per_row() {
        let n2s = |v: Value| rows(&call(num2str_fn, &[v], 1).unwrap()[0]);
        assert_eq!(n2s(mat_of(&[&[1.0, 2.0], &[3.0, 4.0]])), ["1  2", "3  4"]);
        assert_eq!(n2s(mat_of(&[&[1.0], &[22.0]])), [" 1", "22"]);
        assert_eq!(n2s(mat_of(&[&[1.0, 2.0, 3.0]])), ["1  2  3"]);
        assert_eq!(n2s(mat_of(&[&[1.0, -2.0, 300.0]])), ["1   -2  300"]);
        assert_eq!(n2s(mat_of(&[&[1.0, f64::INFINITY]])), ["1  Inf"]);
        // Not integers: %g at the significant digits of the largest value.
        let gap = |n: usize| " ".repeat(n);
        assert_eq!(n2s(mat_of(&[&[1.5, 2.25]])), [format!("1.5{}2.25", gap(8))]);
        assert_eq!(
            n2s(mat_of(&[&[1.5, f64::NAN]])),
            [format!("1.5{}NaN", gap(9))]
        );
        // One more column for a negative element.
        assert_eq!(
            n2s(mat_of(&[&[-1.5, 2.25]])),
            [format!("-1.5{}2.25", gap(9))]
        );
        let m = call(num2str_fn, &[mat_of(&[&[1.0, 2.0], &[3.0, 4.0]])], 1).unwrap();
        let m = m[0].mat().unwrap();
        assert_eq!((m.rows, m.cols, m.class), (2, 4, Class::Char));
        // An integer's full digits, past 2^53 too.
        let big = format!("1{}{}1", "0".repeat(20), " ".repeat(22));
        assert_eq!(n2s(mat_of(&[&[1e20, 1.0]])), [big]);
        assert_eq!(text(num2str_fn, &[Value::Mat(Matrix::empty())]), "");
        assert_eq!(text(int2str, &[num(2.7)]), "3");
        assert_eq!(text(int2str, &[num(-2.5)]), "-3");
        assert_eq!(
            rows(&call(int2str, &[mat_of(&[&[1.4, 2.6]])], 1).unwrap()[0]),
            ["1  3"]
        );
    }

    #[test]
    fn str2double_reads_numbers_and_nothing_else() {
        assert_eq!(parse_double("2.75"), Some(2.75));
        assert_eq!(parse_double("  -1e3 "), Some(-1000.0));
        assert_eq!(parse_double("2.5D2"), Some(250.0));
        assert_eq!(parse_double(".5"), Some(0.5));
        assert_eq!(parse_double("5."), Some(5.0));
        assert_eq!(parse_double("1,200.5"), Some(1200.5));
        assert_eq!(parse_double("-Inf"), Some(f64::NEG_INFINITY));
        assert!(parse_double("nan").unwrap().is_nan());
        for bad in [
            "", "abc", "1e", "1..2", "1 2", ",1", "1,", "e5", "1+2i", "0x10", "--1",
        ] {
            assert_eq!(parse_double(bad), None, "{bad}");
        }
        let v = call(str2double, &[num(5.0)], 1).unwrap();
        assert!(v[0].mat().unwrap().data[0].is_nan());
    }

    #[test]
    fn mat2str_writes_matlab_syntax() {
        assert_eq!(
            text(mat2str, &[mat_of(&[&[1.0, 2.0], &[3.0, 4.0]])]),
            "[1 2;3 4]"
        );
        assert_eq!(text(mat2str, &[mat_of(&[&[1.5, 2.0]])]), "[1.5 2]");
        assert_eq!(text(mat2str, &[num(-0.5)]), "-0.5");
        assert_eq!(
            text(mat2str, &[num(std::f64::consts::PI)]),
            "3.14159265358979"
        );
        assert_eq!(
            text(mat2str, &[num(std::f64::consts::PI), num(3.0)]),
            "3.14"
        );
        assert_eq!(
            text(mat2str, &[mat_of(&[&[f64::INFINITY, f64::NAN]])]),
            "[Inf NaN]"
        );
        assert_eq!(text(mat2str, &[s("it's")]), "'it''s'");
        let t = Value::Mat(Matrix::row(vec![1.0, 0.0]).with_class(Class::Logical));
        assert_eq!(text(mat2str, &[t]), "[true false]");
        assert_eq!(text(mat2str, &[Value::Mat(Matrix::empty())]), "zeros(0,0)");
        assert_eq!(text(mat2str, &[s("")]), "''");
    }

    #[test]
    fn strcat_trims_char_arguments_only() {
        assert_eq!(text(strcat, &[s("a "), s("b"), s(" c ")]), "ab c");
        let c = Value::cell(CellArray::row(vec![s("x "), s("y")]));
        let out = &call(strcat, &[c, s(" z")], 1).unwrap()[0];
        let Value::Cell(out) = out else { panic!() };
        assert_eq!(out.data[0].text().unwrap(), "x  z");
        assert_eq!(out.data[1].text().unwrap(), "y z");
    }

    #[test]
    fn strsplit_collapses_runs_by_default() {
        let parts = |args: &[Value]| -> Vec<String> {
            let v = call(strsplit, args, 1).unwrap();
            let Value::Cell(c) = &v[0] else { panic!() };
            c.data.iter().map(|x| x.text().unwrap()).collect()
        };
        assert_eq!(parts(&[s("a,b,c"), s(",")]), ["a", "b", "c"]);
        assert_eq!(parts(&[s("a,,b"), s(",")]), ["a", "b"]);
        assert_eq!(
            parts(&[s("a,,b"), s(","), s("CollapseDelimiters"), num(0.0)]),
            ["a", "", "b"]
        );
        assert_eq!(parts(&[s("one  two\tthree")]), ["one", "two", "three"]);
        assert_eq!(parts(&[s(",a"), s(",")]), ["", "a"]);
        assert_eq!(parts(&[s("a\tb"), s("\\t")]), ["a", "b"]);
        let d = Value::cell(CellArray::row(vec![s(", "), s(",")]));
        assert_eq!(parts(&[s("a, b,c"), d]), ["a", "b", "c"]);
        let joined = text(
            strjoin,
            &[
                Value::cell(CellArray::row(vec![s("a"), s("b"), s("c")])),
                s("\\n"),
            ],
        );
        assert_eq!(joined, "a\nb\nc");
    }

    #[test]
    fn strrep_replaces_overlapping_matches() {
        let r = |a: &str, b: &str, c: &str| text(strrep, &[s(a), s(b), s(c)]);
        assert_eq!(r("hello world", "o", "0"), "hell0 w0rld");
        assert_eq!(
            r("abc 2 def 22 ghi 222 jkl 2222", "22", "*"),
            "abc 2 def * ghi ** jkl ***"
        );
        assert_eq!(r("aaa", "", "x"), "aaa");
        assert_eq!(r("abc", "abc", ""), "");
    }

    #[test]
    fn the_comparisons() {
        let t = |f: super::super::BuiltinFn, args: &[Value]| -> Vec<f64> {
            call(f, args, 1).unwrap()[0].mat().unwrap().data.clone()
        };
        assert_eq!(t(strcmp, &[s("a"), s("a")]), [1.0]);
        assert_eq!(t(strcmp, &[s("a"), s("ab")]), [0.0]);
        assert_eq!(t(strcmp, &[s("a"), num(97.0)]), [0.0]);
        assert_eq!(t(strcmpi, &[s("ABC"), s("abc")]), [1.0]);
        assert_eq!(t(strncmp, &[s("abcd"), s("abxy"), num(2.0)]), [1.0]);
        assert_eq!(t(strncmp, &[s("abc"), s("abc"), num(10.0)]), [1.0]);
        assert_eq!(t(strncmp, &[s("abc"), s("abcd"), num(4.0)]), [0.0]);
        assert_eq!(t(strncmpi, &[s("ABcd"), s("abXY"), num(2.0)]), [1.0]);
        let c = Value::cell(CellArray::row(vec![s("a"), s("b"), num(1.0)]));
        assert_eq!(t(strcmp, &[c.clone(), s("a")]), [1.0, 0.0, 0.0]);
        let d = Value::cell(CellArray::row(vec![s("a"), s("c")]));
        assert!(call(strcmp, &[c, d], 1).is_err());
        assert_eq!(
            call(strcmp, &[s("a"), s("a")], 1).unwrap()[0]
                .mat()
                .unwrap()
                .class,
            Class::Logical
        );
        assert!(call(strncmp, &[s("a"), s("a"), num(-1.0)], 1).is_err());
    }

    #[test]
    fn strfind_strtok_and_the_predicates() {
        let v = call(strfind, &[s("abcabc"), s("bc")], 1).unwrap();
        assert_eq!(v[0].mat().unwrap().data, [2.0, 5.0]);
        let v = call(strfind, &[s("aaa"), s("aa")], 1).unwrap();
        assert_eq!(v[0].mat().unwrap().data, [1.0, 2.0]);
        let v = call(strtok, &[s("  hello world")], 2).unwrap();
        assert_eq!(v[0].text().unwrap(), "hello");
        assert_eq!(v[1].text().unwrap(), " world");
        let v = call(strtok, &[s("a,b"), s(",")], 2).unwrap();
        assert_eq!(
            (v[0].text().unwrap(), v[1].text().unwrap()),
            ("a".into(), ",b".into())
        );
        let v = call(isspace, &[s("a b\t")], 1).unwrap();
        assert_eq!(v[0].mat().unwrap().data, [0.0, 1.0, 0.0, 1.0]);
        let v = call(isletter, &[s("a1Bé")], 1).unwrap();
        assert_eq!(v[0].mat().unwrap().data, [1.0, 0.0, 1.0, 1.0]);
        assert_eq!(text(blanks, &[num(3.0)]), "   ");
        assert!(call(blanks, &[num(-1.0)], 1).is_err());
        assert!(call(blanks, &[num(1e12)], 1).is_err());
        assert_eq!(text(upper, &[s("abé")]), "ABÉ");
        assert_eq!(text(lower, &[s("ABC")]), "abc");
        assert_eq!(text(strtrim, &[s(" \t x y \n")]), "x y");
    }

    #[test]
    fn regexp_outputs_in_the_order_asked() {
        let out = call(
            regexp,
            &[s("ab12cd345"), s("\\d+"), s("match"), s("start")],
            2,
        )
        .unwrap();
        let Value::Cell(m) = &out[0] else { panic!() };
        assert_eq!(m.data[1].text().unwrap(), "345");
        assert_eq!(out[1].mat().unwrap().data, [3.0, 7.0]);
        let out = call(
            regexp,
            &[s("k=12"), s("(\\w)=(\\d+)"), s("tokens"), s("once")],
            1,
        )
        .unwrap();
        let Value::Cell(t) = &out[0] else { panic!() };
        assert_eq!(t.data[1].text().unwrap(), "12");
        let out = call(regexp, &[s("a1b2"), s("(?<d>\\d)"), s("names")], 1).unwrap();
        let Value::Struct(st) = &out[0] else { panic!() };
        assert_eq!((st.rows, st.cols), (1, 2));
        let out = call(regexp, &[s("a,b"), s(","), s("split")], 1).unwrap();
        let Value::Cell(p) = &out[0] else { panic!() };
        assert_eq!(p.data.len(), 2);
        // The default output order starts with the starts and the ends.
        let out = call(regexp, &[s("xaay"), s("a+")], 2).unwrap();
        assert_eq!(out[0].mat().unwrap().data, [2.0]);
        assert_eq!(out[1].mat().unwrap().data, [3.0]);
        assert!(call(regexp, &[s("aa"), s("(a)\\1")], 1).is_err());
        assert!(call(regexp, &[s("aa"), s("a"), s("bogus")], 1).is_err());
    }

    /// A result far larger than its inputs is refused before it is built,
    /// never an allocator abort; a long search is linear.
    #[test]
    fn a_result_too_large_is_refused_and_a_search_is_linear() {
        let a = |n: usize| chars(vec![97.0; n]);
        let b = chars(vec![98.0; 100_000]);
        let too_big = |r: R<Vec<Value>>| {
            let e = r.unwrap_err().msg;
            assert!(e.contains("exceeds the maximum array size"), "{e}");
        };
        too_big(call(strrep, &[a(100_000), s("a"), b.clone()], 1));
        too_big(call(regexprep, &[a(100_000), s("a"), b.clone()], 1));
        let many = Value::cell(CellArray::row(vec![s(""); 100_000]));
        too_big(call(strjoin, &[many.clone(), b.clone()], 1));
        too_big(call(strcat, &[many, b], 1));
        too_big(call(
            sprintf_like,
            &[s("%8192d"), Value::Mat(Matrix::row(vec![1.0; 40_000]))],
            1,
        ));
        let big = Value::Mat(Matrix::row(vec![0.5; 400_000]));
        too_big(call(
            num2str_fn,
            &[big, Value::Mat(Matrix::scalar(800.0))],
            1,
        ));
        // A run of a's searched for a long run of a's: linear, not
        // quadratic, and every overlapping start found.
        let start = std::time::Instant::now();
        let v = call(strfind, &[a(400_000), a(200_000)], 1).unwrap();
        assert_eq!(v[0].numel(), 200_001);
        assert!(start.elapsed().as_secs() < 20);
        assert_eq!(
            find_all(&[1.0, 2.0, 1.0, 2.0, 1.0], &[1.0, 2.0, 1.0]),
            [0, 2]
        );
        assert_eq!(find_all(&[1.0, 1.0, 2.0], &[1.0, 2.0]), [1]);
    }

    fn sprintf_like(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
        Ok(vec![Value::str(&format_printf(args)?)])
    }

    #[test]
    fn regexprep_replaces_with_tokens() {
        let r = |args: &[Value]| text(regexprep, args);
        assert_eq!(r(&[s("abc123"), s("\\d"), s("")]), "abc");
        assert_eq!(
            r(&[s("john smith"), s("(\\w+) (\\w+)"), s("$2, $1")]),
            "smith, john"
        );
        assert_eq!(r(&[s("aaa"), s("a"), s("b"), s("once")]), "baa");
        assert_eq!(r(&[s("ABC"), s("b"), s("x"), s("ignorecase")]), "AxC");
        assert_eq!(r(&[s("a.b"), s("\\."), s("\\n")]), "a\nb");
        assert_eq!(r(&[s("k=1"), s("(?<v>\\d)"), s("[$<v>]")]), "k=[1]");
        assert_eq!(r(&[s("abc"), s("x*"), s("-"), s("emptymatch")]), "-a-b-c-");
        assert_eq!(r(&[s("price 5"), s("\\d"), s("\\$$0")]), "price $5");
        // A `$<` whose name is not a group's, or never closes, is text.
        assert_eq!(r(&[s("k=1"), s("(?<v>\\d)"), s("$<w>$<v")]), "k=$<w>$<v");
        assert_eq!(r(&[s("k=1"), s("(?<v>\\d)"), s("$<v x>")]), "k=$<v x>");
        // A token the pattern does not have is empty.
        assert_eq!(r(&[s("ab"), s("(a)"), s("[$1$2$9]")]), "[a]b");
    }

    /// A `$<` reads its name only as far as a name goes: a scan to the end
    /// of the replacement for each of 50,000 of them took minutes.
    #[test]
    fn many_named_token_starts_parse_in_linear_time() {
        let start = std::time::Instant::now();
        let rep = "$<".repeat(50_000);
        let out = text(regexprep, &[s("abc"), s("b"), s(&rep)]);
        assert_eq!(out.len(), 100_002);
        assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    }

    /// Sizing a result costs the matches times the tokens, not times the
    /// parts, and the refusal names the whole size; building one skips the
    /// parts that write nothing.
    #[test]
    fn a_replacement_of_many_parts_is_sized_and_built_fast() {
        let start = std::time::Instant::now();
        let subject = s(&"a".repeat(100_000));
        let e = call(
            regexprep,
            &[subject.clone(), s("a"), s(&"$0".repeat(20_000))],
            1,
        )
        .unwrap_err()
        .msg;
        assert_eq!(
            e,
            "Requested 1x2000100000 array exceeds the maximum array size."
        );
        // 20,000 uses of a token that is empty at every match, and of one
        // that is empty at all but ten of 100,000.
        let out = text(regexprep, &[subject, s("(b?)a"), s(&"$1".repeat(20_000))]);
        assert!(out.is_empty());
        let rep = format!("<{}$2>", "$1".repeat(20_000));
        let subject = format!("{}{}", "x".repeat(99_990), "y".repeat(10));
        let out = text(regexprep, &[s(&subject), s("(y)|(x)"), s(&rep)]);
        let want = format!(
            "{}{}",
            "<x>".repeat(99_990),
            format!("<{}>", "y".repeat(20_000)).repeat(10)
        );
        assert_eq!(out, want);
        assert!(start.elapsed().as_secs() < 10, "{:?}", start.elapsed());
    }
}
