//! Constants, constructors, shape queries, output, the workspace and timing.

use std::collections::HashSet;
use std::f64::consts::PI;
use std::rc::Rc;

use super::args::{
    at_most, check_dims, check_shape, dim, mat, need, scalar, shape, shape_dims, string,
};
pub use super::printf::{MAX_FIELD, format_printf};
use super::{Registry, add, none, one, one_as, one_mat};
use crate::error;
use crate::interp::{Callee, Interp, R};
use crate::value::{Class, Func, Matrix, StructArray, Value};

/// The registration table is one line per builtin on purpose: it is the index
/// of the library, and rustfmt would otherwise spread each entry over five
/// lines and hide it.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    // ---- constants ---------------------------------------------------
    add(r, "pi", pi, "pi - ratio of a circle's circumference to its diameter.");
    add(r, "Inf", inf, "Inf, Inf(n), Inf(r,c), Inf(sz) - infinity.");
    add(r, "inf", inf, "inf, inf(n), inf(r,c), inf(sz) - infinity.");
    add(r, "NaN", nan, "NaN, NaN(n), NaN(r,c), NaN(sz) - not-a-number.");
    add(r, "nan", nan, "nan, nan(n), nan(r,c), nan(sz) - not-a-number.");
    add(r, "eps", eps, "eps, eps(x), eps('double') - spacing of doubles, at 1 or at abs(x).");
    add(r, "true", tru, "true, true(n), true(r,c), true(sz) - logical ones.");
    add(r, "false", fls, "false, false(n), false(r,c), false(sz) - logical zeros.");

    // ---- constructors ------------------------------------------------
    add(r, "zeros", zeros, "zeros(n), zeros(r,c), zeros(sz) - a matrix of zeros.");
    add(r, "ones", ones, "ones(n), ones(r,c), ones(sz) - a matrix of ones.");
    add(r, "eye", eye, "eye(n), eye(r,c), eye(sz) - ones on the main diagonal.");
    add(r, "rand", rand, "rand(n), rand(r,c), rand(sz) - uniform values in [0, 1).");
    add(r, "linspace", linspace, "linspace(a,b,n) - floor(n) points evenly spaced from a to b.");

    // ---- shape queries -----------------------------------------------
    add(r, "size", size, "size(A), size(A,dim), [r,c] = size(A) - the dimensions of A.");
    add(r, "ndims", ndims, "ndims(A) - the number of dimensions of A, 2 or more.");
    add(r, "numel", numel, "numel(A) - the number of elements of A.");
    add(r, "length", length, "length(A) - the longest dimension, or 0 if empty.");
    add(r, "isempty", isempty, "isempty(A) - true when A has no elements.");
    add(r, "isscalar", isscalar, "isscalar(A) - true when A is 1x1.");
    add(r, "isvector", isvector, "isvector(A) - true when A is 1-by-N or N-by-1, N >= 0.");

    // ---- classes -----------------------------------------------------
    add(r, "class", class, "class(A) - the class of A: 'double', 'logical', 'char', 'cell', 'struct', 'function_handle' or 'MException'.");
    add(r, "islogical", islogical, "islogical(A) - true when A is logical.");
    add(r, "ischar", ischar, "ischar(A) - true when A is a char array.");
    add(r, "isnumeric", isnumeric, "isnumeric(A) - true when A is numeric; logical and char are not.");
    add(r, "isa", isa, "isa(A,'name') - true when A is of class name, or of the group 'numeric' or 'float'.");
    add(r, "logical", logical, "logical(A) - convert to logical: non-zero is true; NaN is an error.");
    add(r, "char", char_fn, "char(A) - convert to char: each element becomes the UTF-16 code unit it names.");
    add(r, "double", double, "double(A) - convert to double: a char becomes its code units.");

    // ---- output ------------------------------------------------------
    add(r, "disp", disp, "disp(X) - display X without printing its name.");
    add(r, "sprintf", sprintf, "sprintf(fmt,...) - format text into a string.");
    add(r, "error", error, "error(msg), error(fmt,...), error(id,fmt,...) - raise an error.");
    add(r, "rethrow", rethrow, "rethrow(e) - raise the caught MException e again, unchanged.");
    add(r, "lasterr", lasterr, "lasterr - the message of the last error raised.");
    add(r, "warning", warning, "warning(msg), warning(fmt,...), warning(id,fmt,...) - print a warning.");
    add(r, "assert", assert, "assert(cond), assert(cond,fmt,...) - raise an error unless cond holds.");
    add(r, "isequal", isequal, "isequal(A,B,...) - true when every argument has A's size and values.");

    // ---- workspace ---------------------------------------------------
    add(r, "clear", clear, "clear, clear('a'), clear all - remove variables from the workspace.");
    add(r, "clc", clc, "clc - clear the terminal; writes nothing when the output is not a terminal.");
    add(r, "who", who, "who - list the names of the variables in the workspace.");
    add(r, "whos", whos, "whos - list the workspace variables with their size, bytes and class.");

    // ---- functions and the path (cycle 05) ---------------------------
    add(r, "nargin", nargin, "nargin - how many arguments the running function was called with.");
    add(r, "nargout", nargout, "nargout - how many outputs the running function was asked for.");
    add(r, "exist", exist, "exist(name) - 1 for a variable, 2 for a file on the path, 5 for a builtin, 0 otherwise.");
    add(r, "feval", feval, "feval(f,...) - call the function handle or function name f with the other arguments.");
    add(r, "addpath", addpath, "addpath(d1,...) - put folders at the front of the search path.");
    add(r, "rmpath", rmpath, "rmpath(d1,...) - take folders off the search path.");

    // ---- function handles (cycle 06) ---------------------------------
    add(r, "arrayfun", arrayfun, "arrayfun(f,A,...,'UniformOutput',tf) - call f on each element of A, ..., and collect the results in A's shape.");
    add(r, "func2str", func2str, "func2str(f) - the text of a function handle: its name, or @(x)... .");
    add(r, "str2func", str2func, "str2func(s) - a function handle from a name or an '@(x) ...' text.");

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

/// A constant that fills a matrix when given a size, as `NaN(2)` does. It
/// takes every size form a constructor does, trailing ones included, and
/// since cycle 14 any number of sizes: `NaN(2, 1, 3)`.
fn filled_constant(args: &[Value], name: &str, v: f64, class: Class) -> R<Vec<Value>> {
    let dims = check_dims(&shape_dims(args, 0, name)?)?;
    one_as(Matrix::filled_dims(&dims, v).with_class(class))
}

fn pi(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    constant(a, "pi", PI)
}

/// `eps`, `eps(x)` and `eps('double')`. `eps(2)` is the spacing at 2, not a
/// 2x2: `eps` is the one constant whose argument is a value.
fn eps(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "eps")?;
    match a.first() {
        None => one_mat(Matrix::scalar(f64::EPSILON)),
        Some(v) => match v.text() {
            Some(s) if s.eq_ignore_ascii_case("double") => one_mat(Matrix::scalar(f64::EPSILON)),
            Some(_) => Err(error::eps_class()),
            None => one_mat(v.mat()?.map(eps_at)),
        },
    }
}

/// The distance from `abs(x)` to the next larger double, read off the
/// exponent field rather than computed as `next - x`: the successor of the
/// largest double is `Inf`, and `eps(1e308)` must be `2^971`. Zero and the
/// subnormals share the smallest spacing, `2^-1074`; `Inf` and `NaN` give
/// `NaN`, as the MATLAB page says.
pub fn eps_at(x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    let biased = (x.abs().to_bits() >> 52) as i32;
    if biased == 0 {
        return f64::from_bits(1);
    }
    pow2(biased - 1023 - 52)
}

/// `2^k` for `-1074 <= k <= 1023`, built from its bits so it is exact.
fn pow2(k: i32) -> f64 {
    if k >= -1022 {
        f64::from_bits(((k + 1023) as u64) << 52)
    } else {
        f64::from_bits(1u64 << (k + 1074))
    }
}

/// `true` and `false` fill like `NaN` and `Inf`, in every size form, and
/// are logical, as they have been since cycle 02 (they were doubles before).
fn tru(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    filled_constant(a, "true", 1.0, Class::Logical)
}

fn fls(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    filled_constant(a, "false", 0.0, Class::Logical)
}

fn inf(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    filled_constant(a, "Inf", f64::INFINITY, Class::Double)
}

fn nan(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    filled_constant(a, "NaN", f64::NAN, Class::Double)
}

// ---- constructors ----------------------------------------------------

/// Which of the four constructors that share one implementation is running.
#[derive(Clone, Copy)]
enum Fill {
    Zeros,
    Ones,
    Eye,
    Rand,
}

/// The shape comes from `args::shape_dims`, which takes a scalar, a size
/// vector or several sizes, trailing ones dropped, and since cycle 14 any
/// number of them: `zeros(2, 3, 4)` is 2x3x4 and `ones(2, 3, 0)` an empty
/// 2x3x0. `eye` alone keeps MATLAB's limit of two sizes, as arguments and
/// as the elements of a size vector, through `args::shape`.
fn construct(it: &mut Interp, args: &[Value], name: &str, kind: Fill) -> R<Vec<Value>> {
    if matches!(kind, Fill::Eye) {
        at_most(args, 2, name)?;
        let (r, c) = shape(args, 0, name, 2)?;
        let (r, c) = check_shape(r, c)?;
        return one_mat(Matrix::identity(r, c));
    }
    let dims = check_dims(&shape_dims(args, 0, name)?)?;
    let m = match kind {
        Fill::Ones => Matrix::filled_dims(&dims, 1.0),
        Fill::Rand => {
            let mut m = Matrix::filled_dims(&dims, 0.0);
            for v in &mut m.data {
                *v = it.next_rand();
            }
            m
        }
        Fill::Zeros | Fill::Eye => Matrix::filled_dims(&dims, 0.0),
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

/// `linspace(a, b, n)` gives `floor(n)` points, and none for an `n` below 1,
/// as the MATLAB page says. A `NaN` count is no points, as in Octave.
fn linspace(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 3, "linspace")?;
    let a = scalar(args, 0, "linspace")?;
    let b = scalar(args, 1, "linspace")?;
    let n = if args.len() >= 3 {
        if args.get(2).is_some_and(Value::is_char) {
            return Err(error::bad_size_arg("linspace"));
        }
        let n = scalar(args, 2, "linspace")?.floor();
        if n.is_nan() { 0.0 } else { n.max(0.0) }
    } else {
        100.0
    };
    let (_, n) = check_shape(1.0, n)?;
    // "linspace always includes the endpoints", so the last element is `b`
    // itself rather than `a + (b - a) * (n - 1) / (n - 1)`, which rounds off:
    // `linspace(-2.9, 1.17, 7)` used to end a few ulps short of `1.17`. Only
    // the end point needs pinning; the colon's two-sided computation buys
    // nothing here, because every interior point is already one multiply and
    // one add from `a` rather than a running sum.
    let data = (0..n)
        .map(|k| {
            if k + 1 == n {
                b
            } else {
                a + (b - a) * k as f64 / (n - 1) as f64
            }
        })
        .collect();
    one_mat(Matrix::row(data))
}

// ---- shape queries ---------------------------------------------------

/// `size(A)` is the row of every dimension, `[rows cols]` of a 2-D value
/// and `[2 3 4]` of a 2x3x4 (cycle 14). Asked for several outputs, it
/// gives one dimension each, by the MathWorks `size` page's rules: with
/// fewer outputs than `ndims(A)`, "all remaining dimension lengths are
/// collapsed into the last argument", so `[r, c] = size(zeros(2, 3, 4))`
/// gives `c` 12, and with more, "the extra trailing arguments are returned
/// as 1". `[n] = size(A)` is still the row. `size(A, dim)` is one value,
/// and `1` for a dimension past `ndims(A)`.
fn size(_: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(args, 2, "size")?;
    let dims = arg_dims(args, 0, "size")?;
    let d: Vec<f64> = dims.iter().map(|&k| k as f64).collect();
    if args.len() >= 2 {
        // A dimension past the array's is a singleton; `0` is an error.
        let k = dim(args, 1, "size")?;
        let v = d.get(k - 1).copied().unwrap_or(1.0);
        one_mat(Matrix::scalar(v))
    } else if nargout >= 2 {
        // In `f64`, so the product of an empty's huge dimensions is named
        // as it is rather than saturated.
        Ok((0..nargout)
            .map(|k| {
                let v = if k + 1 < nargout {
                    d.get(k).copied().unwrap_or(1.0)
                } else {
                    d.get(k..).map_or(1.0, |rest| rest.iter().product())
                };
                Value::Mat(Matrix::scalar(v))
            })
            .collect())
    } else {
        one_mat(Matrix::row(d))
    }
}

/// `ndims(A)` (cycle 14): 2 for every 2-D value, cells, structs, handles
/// and `MException`s included, and one more per dimension past the second.
fn ndims(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "ndims")?;
    let dims = arg_dims(args, 0, "ndims")?;
    one_mat(Matrix::scalar(dims.len() as f64))
}

/// The dimensions of argument `i`, of any value (cycle 07): an array's,
/// every one of them since cycle 14, a cell's or a struct array's own, and
/// `1x1` for a function handle or an `MException`, each of which is one
/// object.
fn arg_dims(args: &[Value], i: usize, name: &str) -> R<Vec<usize>> {
    args.get(i)
        .map(Value::dims)
        .ok_or_else(|| error::not_enough_args(name))
}

/// `numel(A)`: the product of every dimension.
fn numel(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "numel")?;
    need(args, 1, "numel")?;
    one_mat(Matrix::scalar(args[0].numel() as f64))
}

/// `length(A)`: 0 when any dimension is 0, and the largest dimension
/// otherwise.
fn length(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "length")?;
    let dims = arg_dims(args, 0, "length")?;
    let n = if dims.contains(&0) {
        0
    } else {
        dims.iter().copied().max().unwrap_or(0)
    };
    one_mat(Matrix::scalar(n as f64))
}

/// A predicate on the shape of one argument of any value, answered with a
/// logical scalar, as every MATLAB `is*` function answers (cycle 07 let it
/// answer for every value). It sees every dimension (cycle 14).
fn shape_predicate(args: &[Value], name: &str, test: impl Fn(&[usize]) -> bool) -> R<Vec<Value>> {
    at_most(args, 1, name)?;
    let dims = arg_dims(args, 0, name)?;
    one_as(Matrix::from_bool(test(&dims)))
}

/// A predicate on the class of one argument of any value; a value that is
/// not a matrix is of none of the matrix classes.
fn class_predicate(args: &[Value], name: &str, test: impl Fn(Class) -> bool) -> R<Vec<Value>> {
    at_most(args, 1, name)?;
    let v = args.first().ok_or_else(|| error::not_enough_args(name))?;
    one_as(Matrix::from_bool(
        matches!(v, Value::Mat(m) if test(m.class)),
    ))
}

/// True when any dimension is 0.
fn isempty(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    shape_predicate(args, "isempty", |d| d.contains(&0))
}

/// False for every N-D array, which never has one element.
fn isscalar(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    shape_predicate(args, "isscalar", |d| d.iter().all(|&k| k == 1))
}

/// 1-by-N or N-by-1, and so false for every N-D array.
fn isvector(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    shape_predicate(args, "isvector", |d| {
        d.len() == 2 && (d[0] == 1 || d[1] == 1)
    })
}

// ---- classes ---------------------------------------------------------

fn class(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "class")?;
    need(args, 1, "class")?;
    one(Value::str(args[0].class_name()))
}

fn islogical(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    class_predicate(args, "islogical", |c| c == Class::Logical)
}

fn ischar(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    class_predicate(args, "ischar", |c| c == Class::Char)
}

/// Only `double` is numeric of the three classes; MATLAB counts neither
/// logical nor char, so `isnumeric(true)` is false.
fn isnumeric(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    class_predicate(args, "isnumeric", |c| c == Class::Double)
}

/// `isa(A, name)`: `name` is a class name or one of MATLAB's groups.
/// `'numeric'` and `'float'` both hold `double` alone here, since the
/// integer and single classes do not exist; `'integer'` therefore holds
/// nothing. The name is matched exactly, as MATLAB matches it.
///
/// Every value answers (cycle 07): a function handle is of the class
/// `'function_handle'`, a caught error of `'MException'`, a cell of
/// `'cell'` and a struct of `'struct'`, and none of them is in a group.
fn isa(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 2, "isa")?;
    need(args, 1, "isa")?;
    let name = string(args, 1, "isa")?;
    let yes = match (&args[0], name.as_str()) {
        (Value::Mat(m), "numeric" | "float") => m.class == Class::Double,
        (v, n) => n == v.class_name(),
    };
    one_as(Matrix::from_bool(yes))
}

/// `logical(A)`: non-zero is true. A `NaN` is refused with MATLAB's text,
/// the same refusal `if NaN` makes.
fn logical(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "logical")?;
    one_as(mat(args, 0, "logical")?.to_class(Class::Logical)?)
}

/// `char(A)`: each element becomes the UTF-16 code unit it names, so
/// `char([72 105])` is `'Hi'`. A char comes back as it was.
fn char_fn(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "char")?;
    one_as(mat(args, 0, "char")?.to_class(Class::Char)?)
}

/// `double(A)`: the same elements as doubles, so `double('A')` is `65`.
fn double(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "double")?;
    one_mat(mat(args, 0, "double")?)
}

// ---- output ----------------------------------------------------------

/// `disp(x)`: the value's display body with no `x =` header and no class
/// header. A char is its bare text, one line per row.
///
/// An empty matrix prints nothing at all, as in MATLAB; it used to print
/// `     []`. `disp('')` still prints its empty line, because an empty char
/// is a line with no characters on it rather than no output.
fn disp(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "disp")?;
    need(args, 1, "disp")?;
    // An N-D array is written a page at a time (cycle 14).
    it.emit_disp(&args[0])?;
    none()
}

fn sprintf(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "sprintf")?;
    one(Value::str(&format_printf(args)?))
}

// ---- errors and warnings ---------------------------------------------

/// The identifier and the message that `error`, `warning` and `assert`'s
/// message arguments spell, by the rules of the MATLAB `error` page (QA D9).
///
/// - One argument is the message itself, literal: no format and no escape
///   processing, so `error('100% sure')` says `100% sure` and `error('a\nb')`
///   keeps its backslash.
/// - With more arguments, the first is an identifier when it contains a
///   colon and no whitespace; the message is then the rest, formatted as
///   `sprintf` formats it. Otherwise every argument is the format and its
///   values, and there is no identifier.
///
/// `None` when the first argument is not text at all.
pub fn message_args(args: &[Value]) -> R<Option<(String, String)>> {
    let Some(first) = args.first().and_then(Value::text) else {
        return Ok(None);
    };
    if args.len() == 1 {
        return Ok(Some((String::new(), first)));
    }
    if is_identifier(&first) {
        return Ok(Some((first, format_printf(&args[1..])?)));
    }
    Ok(Some((String::new(), format_printf(args)?)))
}

/// MATLAB's test for a message identifier in first position: a colon, and
/// no whitespace.
pub fn is_identifier(s: &str) -> bool {
    s.contains(':') && !s.chars().any(char::is_whitespace)
}

/// True when every argument is empty, which makes `error` a no-op: "If all
/// inputs to error are empty, MATLAB does not throw an error".
fn all_empty(args: &[Value]) -> bool {
    args.iter()
        .all(|v| matches!(v.mat(), Ok(m) if m.is_empty()))
}

/// `error(msg)`, `error(fmt, A1, ...)` and `error(id, fmt, A1, ...)`.
fn error(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    if !args.is_empty() && all_empty(args) {
        return none();
    }
    match message_args(args)? {
        Some((id, msg)) => Err(error::raised(msg, id)),
        None => Err(error::raised_default()),
    }
}

/// `rethrow(e)`: raises the caught error again, unchanged, its identifier
/// and its line included.
fn rethrow(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "rethrow")?;
    need(args, 1, "rethrow")?;
    match &args[0] {
        Value::Exception(e) => Err(e.clone()),
        v => Err(error::no_method("rethrow", v.class_name())),
    }
}

/// `lasterr`: the message of the last error raised, caught or not, and
/// `''` before the first.
fn lasterr(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "lasterr")?;
    one(Value::str(&it.last_err))
}

/// `warning(msg)`, `warning(fmt, A1, ...)` and `warning(id, fmt, A1, ...)`:
/// `Warning: <msg>` on the error sink, read by the same rules as `error`.
/// Execution goes on.
fn warning(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "warning")?;
    // `warning('')` prints nothing, as `error('')` raises nothing.
    if all_empty(args) {
        return none();
    }
    let (_, msg) = message_args(args)?.ok_or_else(error::format_not_a_string)?;
    it.emit_err(&error::warning_line(&msg))?;
    none()
}

/// `assert(cond)` and `assert(cond, msg, ...)`: nothing when `cond` holds,
/// and otherwise the error, `Assertion failed.` without a message and the
/// message read as `error` reads its arguments with one. The condition holds
/// by the rule `if` uses: non-empty, and every element non-zero.
fn assert(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "assert")?;
    if args[0].mat()?.truth()? {
        return none();
    }
    if args.len() == 1 {
        return Err(error::assertion_failed());
    }
    match message_args(&args[1..])? {
        Some((id, msg)) => Err(error::raised(msg, id)),
        None => Err(error::format_not_a_string()),
    }
}

/// `isequal(A, B, ...)`: true when every argument is equal to the first by
/// [`values_equal`]. The class is not compared, so `isequal('a', 97)` is
/// true; a `NaN` is equal to nothing, itself included.
fn isequal(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "isequal")?;
    let same = args[1..].iter().all(|b| values_equal(&args[0], b));
    one_as(Matrix::from_bool(same))
}

/// One pair for [`isequal`], by the MathWorks `isequal` page's rules.
///
/// Two arrays are equal when they have the same size and the same values:
/// every dimension is compared before any element (cycle 14), so arrays of
/// different shapes are unequal and no comparison reads past either array,
/// and `isequal(zeros(2, 2, 2), zeros(2, 2))` is false where the rows and
/// columns alone agree. Two `MException`s are equal when their message and
/// identifier are. Since cycle 15 two function handles compare by
/// [`handles_equal`]; two cell arrays are equal when they have the same
/// size and every pair of elements in the same place is equal by these
/// rules, and two struct arrays when they have the same size, the same
/// field names in any order ("Fields need not be in the same order as long
/// as the contents are equal") and every field of every element is equal.
/// Values of two different kinds are never equal: a cell never equals a
/// struct or an array, and a handle equals nothing but a handle.
///
/// Nested cells and structs are walked with a worklist of pairs, never by
/// recursion, so a nesting of any depth compares in time proportional to
/// its elements and never overflows the stack (invariant 6). A pair of
/// containers is compared once however many paths reach it, which only a
/// container held in more than one place allows: `c = {c, c}` repeated
/// builds a nest with exponentially many paths through a few containers.
pub fn values_equal(a: &Value, b: &Value) -> bool {
    let mut todo: Vec<(&Value, &Value)> = vec![(a, b)];
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    while let Some(pair) = todo.pop() {
        match pair {
            // Complex values compare both parts, as `==` does (cycle 10), so
            // `complex(1, 0)` equals `1`: the storage is not the value.
            (Value::Mat(a), Value::Mat(b)) => {
                let same = a.dims() == b.dims()
                    && a.numel() == b.numel()
                    && (0..a.numel()).all(|k| a.c(k) == b.c(k));
                if !same {
                    return false;
                }
            }
            (Value::Exception(a), Value::Exception(b)) => {
                if a.msg != b.msg || a.identifier() != b.identifier() {
                    return false;
                }
            }
            (Value::Func(a), Value::Func(b)) => {
                if !handles_equal(a, b) {
                    return false;
                }
            }
            // A pair of containers met before was judged then, and what
            // it holds is already on the list or compared.
            (Value::Cell(a), Value::Cell(b)) => {
                if !first_visit(&mut seen, a, b) {
                    continue;
                }
                if (a.rows, a.cols) != (b.rows, b.cols) {
                    return false;
                }
                todo.extend(a.data.iter().zip(&b.data));
            }
            (Value::Struct(a), Value::Struct(b)) => {
                if !first_visit(&mut seen, a, b) {
                    continue;
                }
                if (a.rows, a.cols) != (b.rows, b.cols) {
                    return false;
                }
                let Some(places) = field_places(a, b) else {
                    return false;
                };
                for (x, y) in a.elems.iter().zip(&b.elems) {
                    todo.extend(x.iter().zip(places.iter().map(|&g| &y[g])));
                }
            }
            _ => return false,
        }
    }
    true
}

/// True the first time the pair of containers `a` and `b` is met by
/// [`values_equal`]'s walk. Only a container held in more than one place
/// can be reached along two paths, so a pair of containers each held once
/// is not recorded, and a nesting built as `c = {c}` costs nothing here.
fn first_visit<T>(seen: &mut HashSet<(usize, usize)>, a: &Rc<T>, b: &Rc<T>) -> bool {
    if Rc::strong_count(a) == 1 && Rc::strong_count(b) == 1 {
        return true;
    }
    seen.insert((Rc::as_ptr(a) as usize, Rc::as_ptr(b) as usize))
}

/// Where each field of `a` is among the fields of `b`, when the two have
/// the same field names in any order, and `None` otherwise. Each name is
/// looked up through the struct's own field index, a hash map past a few
/// fields, so matching 100,000 fields written in the reverse order costs
/// time in proportion to the fields and is never quadratic.
fn field_places(a: &StructArray, b: &StructArray) -> Option<Vec<usize>> {
    if a.fields.len() != b.fields.len() {
        return None;
    }
    let mut taken = vec![false; b.fields.len()];
    let mut places = Vec::with_capacity(a.fields.len());
    for f in &a.fields {
        let g = b.field_index(f)?;
        if std::mem::replace(&mut taken[g], true) {
            return None;
        }
        places.push(g);
    }
    Some(places)
}

/// Two function handles by the MathWorks "Compare Function Handles" page
/// (cycle 15). Two named handles are equal when they name the same
/// function: the same name, bound where each was made to the same local
/// function of the same file, or to no local function for either, so
/// `isequal(@sin, str2func('sin'))` is true. An anonymous function is equal
/// only to its copies, the same handle passed on by assignment, as an
/// argument or through a cell or a field, since two made separately are
/// "unequal because MATLAB cannot guarantee that the frozen values of
/// nonargument variables are the same", whatever their text. A named
/// handle never equals an anonymous one.
fn handles_equal(a: &Rc<Func>, b: &Rc<Func>) -> bool {
    match (&**a, &**b) {
        (Func::Named { name: m, local: p }, Func::Named { name: n, local: q }) => {
            m == n
                && match (p, q) {
                    (None, None) => true,
                    (Some((ua, fa)), Some((ub, fb))) => Rc::ptr_eq(ua, ub) && Rc::ptr_eq(fa, fb),
                    _ => false,
                }
        }
        (Func::Anon { .. }, Func::Anon { .. }) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

// ---- workspace -------------------------------------------------------

fn clear(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    if args.is_empty() {
        it.vars_mut().clear();
    } else {
        // `clear('a')` clears only `a`. Clearing a name that is not there is
        // not an error in MATLAB either. `clear all`, and so `clear('all')`,
        // clears everything, as a bare `clear` does: the functions and
        // globals it also clears in MATLAB do not exist yet.
        let names = (0..args.len())
            .map(|i| string(args, i, "clear"))
            .collect::<R<Vec<String>>>()?;
        if names.iter().any(|n| n == "all") {
            it.vars_mut().clear();
        }
        for name in &names {
            it.vars_mut().remove(name);
        }
    }
    none()
}

/// `clc` writes the terminal's clear only when standard output is a
/// terminal (cycle 13): a script piped or redirected, `--protocol`, `--ui`,
/// `--http-stdio` and `evalc` get nothing, since the escape bytes would be
/// output rather than a clear.
fn clc(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "clc")?;
    if it.stdout_tty {
        it.emit("\x1B[2J\x1B[H")?;
    }
    none()
}

/// The running workspace's variable names, in byte order.
fn sorted_names(it: &Interp) -> Vec<String> {
    let mut names: Vec<String> = it.vars().keys().cloned().collect();
    names.sort();
    names
}

/// `who` (cycle 13): the names alone, after MATLAB's heading, as many to a
/// line as fit in the display's width, two spaces apart. An empty
/// workspace prints nothing.
fn who(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "who")?;
    let names = sorted_names(it);
    if names.is_empty() {
        return none();
    }
    it.emit(&who_text(&names))?;
    none()
}

/// The text `who` prints for `names`.
fn who_text(names: &[String]) -> String {
    let mut text = String::from("Your variables are:\n\n");
    let mut line = String::new();
    for n in names {
        if !line.is_empty() && line.len() + 2 + n.len() > crate::value::TERM_WIDTH {
            text.push_str(&line);
            text.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push_str("  ");
        }
        line.push_str(n);
    }
    text.push_str(&line);
    text.push_str("\n\n");
    text
}

/// `whos` (cycle 13): one row per variable, in byte order, of its name,
/// size, bytes and class. The layout is SplatCrab's own, the columns of
/// the typed table `who` printed before cycle 13 with a bytes column added:
/// two spaces, the names left-aligned to the widest and at least twelve
/// wide, a space, the sizes right-aligned to the widest, the bytes
/// right-aligned three past the widest, two spaces and the class, so
/// `x = 1:3` is `  x            1x3   24  double`. There is no heading
/// row. An empty workspace prints nothing.
fn whos(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "whos")?;
    let rows: Vec<[String; 4]> = sorted_names(it)
        .into_iter()
        .map(|n| {
            let v = &it.vars()[&n];
            [
                n.clone(),
                crate::value::dims_text(&v.dims(), "x"),
                bytes(v).to_string(),
                v.class_name().to_string(),
            ]
        })
        .collect();
    if rows.is_empty() {
        return none();
    }
    it.emit(&whos_text(&rows))?;
    none()
}

/// The table `whos` prints for rows of name, size, bytes and class.
///
/// Every column is padded by hand (cycle 14): an N-D array's size text
/// has no length limit, `2x1x1x...x2`, nor has a name, and Rust's
/// formatter panics on a runtime width past 65,535.
fn whos_text(rows: &[[String; 4]]) -> String {
    use crate::value::{push_left, push_right};
    let width = |k: usize| rows.iter().map(|r| r[k].chars().count()).max().unwrap_or(0);
    let (wn, ws, wb) = (width(0).max(12), width(1), width(2) + 3);
    let mut text = String::new();
    for [name, size, bytes, class] in rows {
        text.push_str("  ");
        push_left(&mut text, name, wn);
        text.push(' ');
        push_right(&mut text, size, ws);
        push_right(&mut text, bytes, wb);
        text.push_str("  ");
        text.push_str(class);
        text.push('\n');
    }
    text.push('\n');
    text
}

/// The bytes `whos` reports for a value (cycle 13): 8 for each element of
/// a double, twice that when it is complex, 1 for each logical and 2 for
/// each char; a cell or a struct is the sum of the values it holds, and a
/// function handle or an `MException` holds no array and counts 0. A
/// nested container is walked with a worklist rather than recursion, so no
/// nesting depth can exhaust the stack.
pub(crate) fn bytes(v: &Value) -> u64 {
    let mut total = 0u64;
    let mut pending = vec![v];
    while let Some(v) = pending.pop() {
        match v {
            Value::Mat(m) => {
                let per = match m.class {
                    Class::Double if m.is_complex() => 16,
                    Class::Double => 8,
                    Class::Logical => 1,
                    Class::Char => 2,
                };
                total = total.saturating_add(per * m.numel() as u64);
            }
            Value::Cell(c) => pending.extend(c.data.iter()),
            Value::Struct(st) => pending.extend(st.elems.iter().flatten()),
            Value::Func(_) | Value::Exception(_) => {}
        }
    }
    total
}

// ---- functions and the path -----------------------------------------

/// `nargin` inside a function: the arguments it was called with.
fn nargin(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "nargin")?;
    let (n, _) = it.call_counts()?;
    one_mat(Matrix::scalar(n as f64))
}

/// `nargout` inside a function: the outputs it was asked for, `0` when it
/// was called as a statement.
fn nargout(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "nargout")?;
    let (_, n) = it.call_counts()?;
    one_mat(Matrix::scalar(n as f64))
}

/// `exist(name)`; see `Interp::exist` for the values.
fn exist(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "exist")?;
    at_most(args, 1, "exist")?;
    let name = string(args, 0, "exist")?;
    one_mat(Matrix::scalar(it.exist(&name)))
}

/// `feval(f, args...)`: calls the function handle `f`, or the function
/// named `f` as a call written in the source would, variables excepted,
/// and passes the caller's `nargout` on.
///
/// A run of leading `'feval'` names, or unbound `@feval` handles, that
/// reach this builtin is peeled off first, so `feval('feval', @feval, 'f',
/// x)` is one call of `f`: without it every one re-entered the interpreter
/// and copied the rest of the arguments, quadratic in the length of the
/// run. The call itself goes through `call_nested`, which counts it
/// against the nesting budget.
fn feval(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(args, 1, "feval")?;
    let is_feval = |v: &Value| match v {
        Value::Func(f) => matches!(&**f, Func::Named { name, local: None } if name == "feval"),
        v => v.text().is_some_and(|t| t == "feval"),
    };
    let mut first = 0;
    if is_feval(&args[0]) && it.reaches_builtin("feval") {
        while is_feval(&args[first]) && first + 1 < args.len() {
            first += 1;
        }
    }
    let rest = args[first + 1..].to_vec();
    match &args[first] {
        Value::Func(f) => it.call_nested(Callee::Handle(f), rest, nargout),
        _ => {
            let name = string(args, first, "feval")?;
            it.call_nested(Callee::Name(&name), rest, nargout)
        }
    }
}

// ---- function handles (cycle 06) -------------------------------------

/// Argument `i` as a function handle.
fn handle(args: &[Value], i: usize, name: &str) -> R<std::rc::Rc<Func>> {
    match args.get(i) {
        Some(Value::Func(f)) => Ok(f.clone()),
        Some(_) => Err(error::arg_not_a_handle(i + 1, name)),
        None => Err(error::not_enough_args(name)),
    }
}

/// `arrayfun(f, A1, ..., An)` and its `'UniformOutput', false` form (cycle 07):
/// see `cells::map_elements`, which `cellfun` shares. Since cycle 14c the
/// arrays may be N-D, of one size, and a uniform result has that size;
/// `'UniformOutput', false` refuses an N-D array, whose cell of results
/// would be N-D.
fn arrayfun(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    super::cells::map_elements(it, args, nargout, "arrayfun")
}

/// `func2str(f)`: a named handle's name, or an anonymous function's text
/// rendered from its tree, `@(x)x.^2+1`.
fn func2str(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "func2str")?;
    let f = handle(args, 0, "func2str")?;
    one(Value::str(&f.text()))
}

/// `str2func(s)`: see `Interp::str2func`.
fn str2func(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "str2func")?;
    let text = string(args, 0, "str2func")?;
    one(it.str2func(&text)?)
}

/// The folder arguments of `addpath` and `rmpath`.
fn folders(args: &[Value], name: &str) -> R<Vec<String>> {
    need(args, 1, name)?;
    (0..args.len()).map(|i| string(args, i, name)).collect()
}

fn addpath(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let dirs = folders(args, "addpath")?;
    it.add_path(&dirs)?;
    none()
}

fn rmpath(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let dirs = folders(args, "rmpath")?;
    it.remove_path(&dirs)?;
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
    it.tic_mark = Some(now);
    none()
}

/// A bare `toc` needs an earlier bare `tic`; `t = tic` does not count, since
/// it leaves the shared mark alone. `toc(t)` needs only its handle.
fn toc(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(args, 1, "toc")?;
    let base = if args.is_empty() {
        it.tic_mark.ok_or_else(error::toc_without_tic)?
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
            other => panic!("expected a matrix, got {other:?}"),
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
            other => panic!("expected a matrix, got {other:?}"),
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
        let e = call(zeros, &[num(1e10)], 1).unwrap_err().msg;
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
            other => panic!("expected a matrix, got {other:?}"),
        }
        assert_eq!(shape_of(inf, &[]), (1, 1));
        // pi has the single syntax `p = pi` on the MATLAB page.
        assert_eq!(
            call(pi, &[num(2.0)], 1).unwrap_err().msg,
            "Too many input arguments."
        );
    }

    fn mat_of(f: super::super::BuiltinFn, args: &[Value]) -> Matrix {
        call(f, args, 1).unwrap()[0].clone().into_mat().unwrap()
    }

    fn row(v: &[f64]) -> Value {
        Value::Mat(Matrix::row(v.to_vec()))
    }

    #[test]
    fn true_and_false_fill_like_nan_and_inf() {
        assert_eq!(shape_of(tru, &[]), (1, 1));
        assert_eq!(shape_of(tru, &[num(2.0)]), (2, 2));
        assert_eq!(shape_of(fls, &[num(2.0), num(3.0)]), (2, 3));
        assert_eq!(shape_of(tru, &[row(&[1.0, 4.0])]), (1, 4));
        assert_eq!(shape_of(fls, &[num(-1.0)]), (0, 0));
        assert_eq!(shape_of(tru, &[num(2.0), num(2.0), num(1.0)]), (2, 2));
        assert!(mat_of(tru, &[num(3.0)]).data.iter().all(|v| *v == 1.0));
        assert!(mat_of(fls, &[num(3.0)]).data.iter().all(|v| *v == 0.0));
        // Cycle 14: any number of sizes, and still logical.
        let t = mat_of(tru, &[num(2.0), num(2.0), num(2.0)]);
        assert_eq!((t.dims(), t.class), (vec![2, 2, 2], Class::Logical));
        assert!(t.data.iter().all(|v| *v == 1.0));
    }

    #[test]
    fn eps_is_the_spacing_at_abs_x() {
        let at = |x: f64| eps_at(x);
        assert_eq!(at(1.0), f64::EPSILON);
        assert_eq!(at(2.0), 2.0 * f64::EPSILON);
        assert_eq!(at(-2.0), at(2.0));
        // Just below a power of two the spacing is the smaller one.
        assert_eq!(at(1.9999), f64::EPSILON);
        assert_eq!(at(0.75), f64::EPSILON / 2.0);
        assert!((at(1e10) - 1.907_348_632_812_5e-6).abs() < 1e-20);
        // Zero and every subnormal share the smallest spacing, 2^-1074.
        let tiny = f64::from_bits(1);
        assert_eq!(at(0.0), tiny);
        assert_eq!(at(-0.0), tiny);
        assert_eq!(at(1e-320), tiny);
        assert_eq!(at(f64::MIN_POSITIVE / 2.0), tiny);
        // The smallest normal is spaced like the subnormals below it.
        assert_eq!(at(f64::MIN_POSITIVE), tiny);
        // The largest doubles are 2^971 apart, not Inf.
        assert_eq!(at(1e308), 2f64.powi(971));
        assert_eq!(at(f64::MAX), 2f64.powi(971));
        assert!(at(f64::INFINITY).is_nan());
        assert!(at(f64::NEG_INFINITY).is_nan());
        assert!(at(f64::NAN).is_nan());
        // Each spacing is exact: x + eps(x) is the next double up.
        for x in [1.0, 3.0, 1e-300, 12345.678, 1e300] {
            assert_eq!(x + at(x), f64::from_bits(x.to_bits() + 1), "{x}");
        }
    }

    #[test]
    fn eps_takes_a_value_or_the_class_name_double() {
        assert_eq!(mat_of(eps, &[]).data, [f64::EPSILON]);
        assert_eq!(mat_of(eps, &[num(2.0)]).data, [2.0 * f64::EPSILON]);
        let m = mat_of(eps, &[row(&[1.0, 4.0])]);
        assert_eq!((m.rows, m.cols), (1, 2));
        assert_eq!(m.data, [f64::EPSILON, 4.0 * f64::EPSILON]);
        let double = Value::str("double");
        assert_eq!(mat_of(eps, &[double]).data, [f64::EPSILON]);
        assert_eq!(
            call(eps, &[Value::str("single")], 1).unwrap_err().msg,
            "Only 'double' is supported as a class name for 'eps'."
        );
        assert!(call(eps, &[num(1.0), num(2.0)], 1).is_err());
    }

    #[test]
    fn constructors_take_a_size_vector_and_trailing_ones() {
        assert_eq!(shape_of(zeros, &[row(&[2.0, 3.0])]), (2, 3));
        assert_eq!(shape_of(ones, &[row(&[3.0, 1.0])]), (3, 1));
        assert_eq!(shape_of(rand, &[row(&[2.0, 3.0, 1.0])]), (2, 3));
        assert_eq!(shape_of(nan, &[row(&[2.0, 3.0])]), (2, 3));
        assert_eq!(shape_of(zeros, &[row(&[4.0])]), (4, 4));
        assert_eq!(
            shape_of(ones, &[num(2.0), num(3.0), num(1.0), num(1.0)]),
            (2, 3)
        );
        match &call(eye, &[row(&[2.0, 3.0])], 1).unwrap()[0] {
            Value::Mat(m) => assert_eq!(m.data, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            other => panic!("expected a matrix, got {other:?}"),
        }
        // Cycle 14: three sizes or more make an N-D array; a trailing 1
        // is still dropped, one before another size is not.
        assert_eq!(
            mat_of(zeros, &[num(2.0), num(3.0), num(4.0)]).dims(),
            [2, 3, 4]
        );
        assert_eq!(mat_of(ones, &[row(&[2.0, 2.0, 2.0])]).dims(), [2, 2, 2]);
        assert_eq!(
            mat_of(zeros, &[num(2.0), num(3.0), num(1.0), num(4.0)]).dims(),
            [2, 3, 1, 4]
        );
        assert_eq!(
            mat_of(zeros, &[num(2.0), num(3.0), num(1.0)]).dims(),
            [2, 3]
        );
        let empty = mat_of(ones, &[num(2.0), num(3.0), num(0.0)]);
        assert_eq!((empty.dims(), empty.data.len()), (vec![2, 3, 0], 0));
        let r = mat_of(rand, &[num(2.0), num(2.0), num(2.0)]);
        assert!(r.data.iter().all(|&v| v > 0.0 && v < 1.0) && r.data.len() == 8);
        assert!(
            mat_of(nan, &[num(2.0), num(1.0), num(3.0)])
                .data
                .iter()
                .all(|v| v.is_nan())
        );
        // A 1x1x3 is no size vector, though it has one row.
        let nd_size = Value::Mat(Matrix::from_dims(&[1, 1, 3], vec![2.0; 3]));
        assert_eq!(
            call(zeros, &[nd_size], 1).unwrap_err().msg,
            "Size vector for 'zeros' must be a row vector."
        );
        // Every size is named when the shape is too large.
        assert_eq!(
            call(zeros, &[num(1e5), num(1e5), num(1e5)], 1)
                .unwrap_err()
                .msg,
            "Requested 100000x100000x100000 array exceeds the maximum array size."
        );
        let nd = "N-D arrays are not supported.";
        // eye keeps its two-size limit.
        assert_eq!(
            call(eye, &[num(2.0), num(3.0), num(1.0)], 1)
                .unwrap_err()
                .msg,
            "Too many input arguments."
        );
        assert_eq!(call(eye, &[row(&[2.0, 3.0, 1.0])], 1).unwrap_err().msg, nd);
        let col = Value::Mat(Matrix::col(vec![2.0, 3.0]));
        assert_eq!(
            call(zeros, &[col], 1).unwrap_err().msg,
            "Size vector for 'zeros' must be a row vector."
        );
        // A char is never read as its character codes.
        assert!(call(zeros, &[Value::str("a")], 1).is_err());
    }

    #[test]
    fn a_size_past_usize_is_named_as_asked() {
        let e = call(zeros, &[num(1e300)], 1).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 1e+300x1e+300 array exceeds the maximum array size."
        );
        let e = call(ones, &[row(&[1.0, 1e300])], 1).unwrap_err().msg;
        assert!(e.contains("1x1e+300"), "{e}");
    }

    #[test]
    fn linspace_floors_its_count() {
        let lin = |n: f64| mat_of(linspace, &[num(0.0), num(1.0), num(n)]);
        assert_eq!(lin(2.7).data, [0.0, 1.0]);
        assert_eq!(lin(3.0).data, [0.0, 0.5, 1.0]);
        assert_eq!(lin(1.5).data, [1.0]);
        for none in [0.5, 0.0, -2.0, f64::NAN] {
            let m = lin(none);
            assert_eq!((m.rows, m.cols), (1, 0), "{none}");
        }
        let e = call(linspace, &[num(0.0), num(1.0), num(f64::INFINITY)], 1)
            .unwrap_err()
            .msg;
        assert!(e.contains("1xInf"), "{e}");
        assert_eq!(mat_of(linspace, &[num(0.0), num(1.0)]).numel(), 100);
    }

    /// "linspace always includes the endpoints": the last element is the end
    /// point itself, not `a + (b - a) * (n - 1) / (n - 1)`, which rounds off.
    #[test]
    fn linspace_lands_exactly_on_both_end_points() {
        let lin = |a: f64, b: f64, n: f64| mat_of(linspace, &[num(a), num(b), num(n)]);
        for (a, b, n) in [
            (0.0, 1.0, 7.0),
            (-2.9, 1.17, 7.0),
            (1.0, 0.0, 5.0),
            (-1.0, 1.0, 101.0),
            (0.1, 0.3, 3.0),
            (1e-15, 1e15, 9.0),
        ] {
            let m = lin(a, b, n);
            assert_eq!(m.numel(), n as usize, "{a}:{b} in {n}");
            assert_eq!(m.data[0], a, "start of {a}..{b}");
            assert_eq!(*m.data.last().unwrap(), b, "end of {a}..{b}");
        }
        // A single point is the end point, as MATLAB documents.
        assert_eq!(lin(0.0, 1.0, 1.0).data, [1.0]);
        // The interior is unchanged.
        assert_eq!(lin(0.0, 1.0, 5.0).data, [0.0, 0.25, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn isvector_counts_an_empty_row_or_column() {
        let isv = |m: Matrix| mat_of(isvector, &[Value::Mat(m)]).data[0];
        assert_eq!(isv(Matrix::new(1, 0, vec![])), 1.0);
        assert_eq!(isv(Matrix::new(0, 1, vec![])), 1.0);
        assert_eq!(isv(Matrix::empty()), 0.0);
        assert_eq!(isv(Matrix::new(2, 0, vec![])), 0.0);
        assert_eq!(isv(Matrix::scalar(5.0)), 1.0);
        assert_eq!(isv(Matrix::filled(2, 2, 0.0)), 0.0);
    }

    #[test]
    fn extra_arguments_are_rejected() {
        assert_eq!(
            call(numel, &[num(1.0), num(2.0)], 1).unwrap_err().msg,
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
        let e = call(size, &args, 1).unwrap_err().msg;
        assert!(e.contains("positive integer"), "{e}");
        // A dimension past the array's is a singleton.
        let args = [a[0].clone(), num(3.0)];
        assert_eq!(
            call(size, &args, 1).unwrap()[0]
                .clone()
                .into_mat()
                .unwrap()
                .data,
            [1.0]
        );
    }

    // ---- workspace and timing ----------------------------------------

    #[test]
    fn clear_with_a_name_clears_only_that_name() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        it.vars_mut().insert("a".to_string(), num(1.0));
        it.vars_mut().insert("b".to_string(), num(2.0));
        clear(&mut it, &[Value::str("a")], 0).unwrap();
        assert!(!it.vars().contains_key("a"));
        assert!(it.vars().contains_key("b"));
        // Clearing a name that is not there is not an error.
        clear(&mut it, &[Value::str("nope")], 0).unwrap();
        // With no arguments it still clears everything.
        clear(&mut it, &[], 0).unwrap();
        assert!(it.vars().is_empty());
    }

    #[test]
    fn tic_and_toc_depend_on_nargout() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        // Before any bare tic, a bare toc has nothing to measure from, and a
        // `t = tic` does not count as one.
        let msg = "You must call TIC without an output argument before calling TOC \
                   without an input argument.";
        assert_eq!(toc(&mut it, &[], 0).unwrap_err().msg, msg);
        let handle = tic(&mut it, &[], 1).unwrap();
        assert_eq!(toc(&mut it, &[], 1).unwrap_err().msg, msg);
        // toc(t) needs only its handle.
        assert_eq!(toc(&mut it, &handle, 1).unwrap().len(), 1);
        // As a statement, tic produces no value and toc prints one.
        assert!(tic(&mut it, &[], 0).unwrap().is_empty());
        assert!(toc(&mut it, &[], 0).unwrap().is_empty());
        // Asked for a value, both produce one.
        let handle = tic(&mut it, &[], 1).unwrap();
        assert_eq!(handle.len(), 1);
        let elapsed = toc(&mut it, &handle, 1).unwrap()[0]
            .clone()
            .into_mat()
            .unwrap();
        assert!(elapsed.scalar_value().unwrap() >= 0.0);
        let bare = toc(&mut it, &[], 1).unwrap()[0].clone().into_mat().unwrap();
        assert!(bare.scalar_value().unwrap() >= 0.0);
    }

    // ---- classes -----------------------------------------------------

    fn class_name(f: super::super::BuiltinFn, args: &[Value]) -> &'static str {
        mat_of(f, args).class.name()
    }

    #[test]
    fn class_names_the_class_and_the_predicates_are_logical() {
        let text_of = |args: &[Value]| call(class, args, 1).unwrap()[0].text().unwrap();
        assert_eq!(text_of(&[num(5.0)]), "double");
        assert_eq!(text_of(&[Value::str("a")]), "char");
        assert_eq!(text_of(&[Value::Mat(Matrix::from_bool(true))]), "logical");
        // Every predicate answers with a logical scalar.
        for f in [isempty, isscalar, isvector, islogical, ischar, isnumeric] {
            assert_eq!(class_name(f, &[num(1.0)]), "logical");
        }
        let yes = |f: super::super::BuiltinFn, v: Value| mat_of(f, &[v]).data[0] == 1.0;
        assert!(yes(islogical, Value::Mat(Matrix::from_bool(false))));
        assert!(!yes(islogical, num(1.0)));
        assert!(yes(ischar, Value::str("a")));
        assert!(yes(ischar, Value::str("")));
        assert!(!yes(ischar, num(97.0)));
        assert!(yes(isnumeric, num(2.0)));
        assert!(!yes(isnumeric, Value::str("a")));
        assert!(!yes(isnumeric, Value::Mat(Matrix::from_bool(true))));
    }

    #[test]
    fn isa_matches_a_class_name_or_a_group() {
        let isa_of = |v: Value, name: &str| mat_of(isa, &[v, Value::str(name)]);
        let t = || Value::Mat(Matrix::from_bool(true));
        for (v, name, want) in [
            (num(2.0), "double", true),
            (num(2.0), "numeric", true),
            (num(2.0), "float", true),
            (num(2.0), "integer", false),
            (num(2.0), "char", false),
            (t(), "logical", true),
            (t(), "numeric", false),
            (Value::str("a"), "char", true),
            (Value::str("a"), "numeric", false),
        ] {
            let got = isa_of(v, name);
            assert_eq!(got.class, Class::Logical);
            assert_eq!(got.data[0] == 1.0, want, "{name}");
        }
        assert!(call(isa, &[num(1.0), num(2.0)], 1).is_err());
        assert!(call(isa, &[num(1.0)], 1).is_err());
    }

    #[test]
    fn logical_char_and_double_convert() {
        let l = mat_of(logical, &[row(&[2.0, 0.0, -1.0])]);
        assert_eq!(
            (l.class, l.data.as_slice()),
            (Class::Logical, &[1.0, 0.0, 1.0][..])
        );
        assert_eq!(
            call(logical, &[num(f64::NAN)], 1).unwrap_err().msg,
            "NaN's cannot be converted to logicals."
        );
        let c = mat_of(char_fn, &[row(&[72.0, 105.0])]);
        assert_eq!(c.class, Class::Char);
        assert_eq!(c.text(), "Hi");
        // A char stays itself, code units and all.
        assert_eq!(mat_of(char_fn, &[Value::str("😀")]).text(), "😀");
        let d = mat_of(double, &[Value::str("A")]);
        assert_eq!((d.class, d.data.as_slice()), (Class::Double, &[65.0][..]));
        assert_eq!(
            class_name(double, &[Value::Mat(Matrix::from_bool(true))]),
            "double"
        );
        for f in [logical, char_fn, double] {
            assert!(call(f, &[num(1.0), num(2.0)], 1).is_err());
        }
    }

    #[test]
    fn true_and_false_are_logical_in_every_size_form() {
        for args in [
            vec![],
            vec![num(2.0)],
            vec![num(2.0), num(3.0)],
            vec![row(&[1.0, 4.0])],
        ] {
            assert_eq!(class_name(tru, &args), "logical");
            assert_eq!(class_name(fls, &args), "logical");
        }
        // The other filled constants stay double.
        assert_eq!(class_name(nan, &[num(2.0)]), "double");
        assert_eq!(class_name(inf, &[]), "double");
    }

    /// `who` is the names alone, and `whos` the table of size, bytes and
    /// class (cycle 13).
    #[test]
    fn who_names_and_whos_bytes() {
        let buf = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        struct Shared(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);
        impl std::io::Write for Shared {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.borrow_mut().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut it = Interp::with_output(Box::new(Shared(buf.clone())));
        it.run("c = ['ab'; 'cd']; t = true(1, 3); x = 1;").unwrap();
        who(&mut it, &[], 0).unwrap();
        let out = String::from_utf8(buf.borrow().clone()).unwrap();
        assert_eq!(out, "Your variables are:\n\nc  t  x\n\n");
        buf.borrow_mut().clear();
        whos(&mut it, &[], 0).unwrap();
        let out = String::from_utf8(buf.borrow().clone()).unwrap();
        assert_eq!(
            out,
            "  c            2x2   8  char\n  t            1x3   3  logical\n  x            1x1   8  double\n\n"
        );
        // The spec's example: 8 per double, 2 per char.
        let rows = [
            ["s".to_string(), "1x2".into(), "4".into(), "char".into()],
            ["x".to_string(), "1x3".into(), "24".into(), "double".into()],
        ];
        assert_eq!(
            whos_text(&rows),
            "  s            1x2    4  char\n  x            1x3   24  double\n\n"
        );
        // Complex is twice a double; containers sum what they hold,
        // nested ones included; a handle holds no array.
        let z = Value::Mat(Matrix::complex_parts(1, 2, vec![1.0, 2.0], vec![1.0, 0.0]));
        assert_eq!(bytes(&z), 32);
        let inner = Value::cell(crate::value::CellArray::row(vec![
            num(1.0),
            Value::str("ab"),
        ]));
        let outer = Value::cell(crate::value::CellArray::row(vec![inner, z]));
        assert_eq!(bytes(&outer), 8 + 4 + 32);
        let st = Value::strukt(crate::value::StructArray::scalar(
            vec!["a".into()],
            vec![Value::Mat(Matrix::filled(2, 2, 0.0))],
        ));
        assert_eq!(bytes(&st), 32);
        // A long list of names wraps at the display's width.
        let names: Vec<String> = (0..30).map(|k| format!("name{k:02}")).collect();
        let text = who_text(&names);
        assert!(text.lines().all(|l| l.len() <= crate::value::TERM_WIDTH));
        assert_eq!(text.lines().count(), 2 + 3 + 1);
    }

    /// `[r, c] = size(A)`, and the last output's product rule: outputs past
    /// the second are the trailing singletons.
    #[test]
    fn size_answers_one_dimension_per_output() {
        let a = Value::Mat(Matrix::filled(2, 5, 0.0));
        let dims = |n: usize| -> Vec<f64> {
            call(size, std::slice::from_ref(&a), n)
                .unwrap()
                .into_iter()
                .map(|v| v.into_mat().unwrap().data[0])
                .collect()
        };
        assert_eq!(dims(2), [2.0, 5.0]);
        assert_eq!(dims(4), [2.0, 5.0, 1.0, 1.0]);
        // One output, or none at statement level, is the size row.
        for n in [0, 1] {
            let out = call(size, std::slice::from_ref(&a), n).unwrap();
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].mat().unwrap().data, [2.0, 5.0]);
        }
        // `size(A, dim)` is one value however many are asked for.
        assert_eq!(call(size, &[a, num(2.0)], 2).unwrap().len(), 1);
    }

    // ---- errors and warnings (cycle 04) ----------------------------------

    fn s(text: &str) -> Value {
        Value::str(text)
    }

    fn msg_of(args: &[Value]) -> (String, String) {
        message_args(args).unwrap().expect("text arguments")
    }

    /// QA D9: one argument is literal, and the identifier is the first of
    /// several arguments when it has a colon and no whitespace.
    #[test]
    fn the_message_argument_rules() {
        let none = String::new;
        assert_eq!(msg_of(&[s("100% sure")]), (none(), "100% sure".into()));
        assert_eq!(msg_of(&[s("a\\nb")]), (none(), "a\\nb".into()));
        // One argument is never an identifier, colon or not.
        assert_eq!(msg_of(&[s("a:b")]), (none(), "a:b".into()));
        assert_eq!(
            msg_of(&[s("MyPkg:myid"), s("Value %d bad"), num(7.0)]),
            ("MyPkg:myid".into(), "Value 7 bad".into())
        );
        // With an identifier the message is a format, escapes included.
        assert_eq!(
            msg_of(&[s("a:b"), s("x\\ny")]),
            ("a:b".into(), "x\ny".into())
        );
        // A colon with whitespace is a format, not an identifier.
        assert_eq!(
            msg_of(&[s("Value: %d"), num(5.0)]),
            (none(), "Value: 5".into())
        );
        // No colon: every argument is the format and its values.
        assert_eq!(msg_of(&[s("n = %d"), num(3.0)]), (none(), "n = 3".into()));
        assert_eq!(message_args(&[num(1.0)]).unwrap(), None);
        assert_eq!(message_args(&[]).unwrap(), None);
    }

    #[test]
    fn error_throws_nothing_when_every_input_is_empty() {
        assert!(call(error, &[s("")], 0).unwrap().is_empty());
        assert!(
            call(error, &[s(""), Value::Mat(Matrix::empty())], 0)
                .unwrap()
                .is_empty()
        );
        let e = call(error, &[s("100% sure")], 0).unwrap_err();
        assert_eq!((e.msg.as_str(), e.identifier()), ("100% sure", ""));
        let e = call(error, &[s("p:q"), s("v %d"), num(2.0)], 0).unwrap_err();
        assert_eq!((e.msg.as_str(), e.identifier()), ("v 2", "p:q"));
        assert_eq!(call(error, &[num(1.0)], 0).unwrap_err().msg, "error");
    }

    #[test]
    fn assert_passes_or_raises_its_message() {
        assert!(
            call(assert, &[Value::Mat(Matrix::from_bool(true))], 0)
                .unwrap()
                .is_empty()
        );
        assert!(call(assert, &[num(2.0)], 0).unwrap().is_empty());
        let no = || Value::Mat(Matrix::from_bool(false));
        assert_eq!(
            call(assert, &[no()], 0).unwrap_err().msg,
            "Assertion failed."
        );
        assert_eq!(
            call(assert, &[no(), s("nope %d"), num(3.0)], 0)
                .unwrap_err()
                .msg,
            "nope 3"
        );
        assert_eq!(call(assert, &[no(), s("50%")], 0).unwrap_err().msg, "50%");
        let e = call(assert, &[no(), s("a:b"), s("m")], 0).unwrap_err();
        assert_eq!((e.msg.as_str(), e.identifier()), ("m", "a:b"));
        // The condition is judged as `if` judges it.
        assert!(call(assert, &[Value::Mat(Matrix::empty())], 0).is_err());
        assert!(call(assert, &[Value::Mat(Matrix::row(vec![1.0, 0.0]))], 0).is_err());
        assert!(call(assert, &[Value::Mat(Matrix::scalar(f64::NAN))], 0).is_err());
    }

    #[test]
    fn isequal_compares_sizes_and_values_but_not_classes() {
        let yes = |args: &[Value]| {
            let v = call(isequal, args, 1)
                .unwrap()
                .remove(0)
                .into_mat()
                .unwrap();
            assert_eq!(v.class, Class::Logical);
            v.data[0] == 1.0
        };
        let r = |d: &[f64]| Value::Mat(Matrix::row(d.to_vec()));
        assert!(yes(&[r(&[1.0, 2.0]), r(&[1.0, 2.0])]));
        assert!(yes(&[s("a"), s("a"), s("a")]));
        assert!(!yes(&[r(&[1.0, 2.0]), r(&[1.0, 2.0, 3.0])]));
        assert!(!yes(&[
            r(&[1.0, 2.0]),
            Value::Mat(Matrix::col(vec![1.0, 2.0]))
        ]));
        assert!(yes(&[s("a"), num(97.0)]));
        assert!(yes(&[Value::Mat(Matrix::from_bool(true)), num(1.0)]));
        assert!(!yes(&[num(f64::NAN), num(f64::NAN)]));
        assert!(yes(&[Value::Mat(Matrix::empty()), s("")]));
        assert!(!yes(&[num(1.0), num(1.0), num(2.0)]));
        assert_eq!(
            call(isequal, &[num(1.0)], 1).unwrap_err().msg,
            "Not enough input arguments for 'isequal'."
        );
        let e = |m: &str| Value::Exception(crate::error::MError::new(m));
        assert!(yes(&[e("x"), e("x")]));
        assert!(!yes(&[e("x"), e("y")]));
        assert!(!yes(&[e("x"), num(1.0)]));
    }

    /// Since cycle 07 the shape and class queries answer for every value: a
    /// handle and an `MException` are 1x1 and never empty, a cell and a
    /// struct have their own sizes, and none of them is of a matrix class.
    #[test]
    fn the_queries_answer_for_every_value() {
        use crate::value::{CellArray, StructArray};
        let handle = Value::Func(std::rc::Rc::new(Func::Named {
            name: "sin".into(),
            local: None,
        }));
        let caught = Value::Exception(error::raised("m".into(), "a:b".into()));
        let cell = Value::cell(CellArray::blanks(1, 3));
        let empty = Value::cell(CellArray::default());
        let strukt = Value::strukt(StructArray::scalar(vec!["a".into()], vec![num(1.0)]));
        let yes = |f: super::super::BuiltinFn, v: &Value| {
            mat_of(f, std::slice::from_ref(v)).data[0] == 1.0
        };
        for v in [&handle, &caught, &strukt] {
            assert_eq!(shape_of(size, std::slice::from_ref(v)), (1, 2));
            assert_eq!(mat_of(size, std::slice::from_ref(v)).data, [1.0, 1.0]);
            assert_eq!(mat_of(numel, std::slice::from_ref(v)).data, [1.0]);
            assert!(yes(isscalar, v) && !yes(isempty, v) && yes(isvector, v));
        }
        assert_eq!(mat_of(size, std::slice::from_ref(&cell)).data, [1.0, 3.0]);
        assert_eq!(mat_of(length, std::slice::from_ref(&cell)).data, [3.0]);
        assert!(yes(isempty, &empty) && !yes(isempty, &cell));
        for v in [&handle, &caught, &cell, &strukt] {
            assert!(!yes(islogical, v) && !yes(ischar, v) && !yes(isnumeric, v));
            assert_eq!(class_name(isempty, std::slice::from_ref(v)), "logical");
        }
        let isa_of = |v: &Value, name: &str| mat_of(isa, &[v.clone(), Value::str(name)]).data[0];
        assert_eq!(isa_of(&caught, "MException"), 1.0);
        assert_eq!(isa_of(&handle, "function_handle"), 1.0);
        assert_eq!(isa_of(&cell, "cell"), 1.0);
        assert_eq!(isa_of(&strukt, "struct"), 1.0);
        assert_eq!(isa_of(&cell, "numeric"), 0.0);
        assert_eq!(isa_of(&strukt, "cell"), 0.0);
        let text_of = |v: &Value| {
            call(class, std::slice::from_ref(v), 1).unwrap()[0]
                .text()
                .unwrap()
        };
        assert_eq!(text_of(&cell), "cell");
        assert_eq!(text_of(&strukt), "struct");
    }

    fn nd(dims: &[usize]) -> Value {
        Value::Mat(Matrix::filled_dims(dims, 0.0))
    }

    /// Cycle 14: the shape queries by the MathWorks `size` page's rules.
    #[test]
    fn shape_queries_see_every_dimension() {
        let a = nd(&[2, 3, 4]);
        assert_eq!(mat_of(size, std::slice::from_ref(&a)).data, [2.0, 3.0, 4.0]);
        let outs = |n: usize| -> Vec<f64> {
            call(size, std::slice::from_ref(&a), n)
                .unwrap()
                .iter()
                .map(|v| v.mat().unwrap().data[0])
                .collect()
        };
        assert_eq!(outs(2), [2.0, 12.0]);
        assert_eq!(outs(3), [2.0, 3.0, 4.0]);
        assert_eq!(outs(4), [2.0, 3.0, 4.0, 1.0]);
        assert_eq!(mat_of(size, &[a.clone(), num(3.0)]).data, [4.0]);
        assert_eq!(mat_of(size, &[a.clone(), num(5.0)]).data, [1.0]);
        let ndims_of = |v: Value| mat_of(ndims, &[v]).data[0];
        assert_eq!(ndims_of(a.clone()), 3.0);
        assert_eq!(ndims_of(nd(&[1, 1, 1, 2])), 4.0);
        assert_eq!(ndims_of(num(5.0)), 2.0);
        assert_eq!(ndims_of(Value::str("ab")), 2.0);
        assert_eq!(
            ndims_of(Value::cell(crate::value::CellArray::default())),
            2.0
        );
        assert_eq!(mat_of(numel, std::slice::from_ref(&a)).data, [24.0]);
        assert_eq!(mat_of(length, &[nd(&[2, 5, 3])]).data, [5.0]);
        assert_eq!(mat_of(length, &[nd(&[2, 0, 3])]).data, [0.0]);
        assert_eq!(mat_of(isempty, &[nd(&[2, 0, 3])]).data, [1.0]);
        assert_eq!(mat_of(isvector, &[nd(&[1, 1, 3])]).data, [0.0]);
        assert_eq!(mat_of(isscalar, &[nd(&[1, 1, 3])]).data, [0.0]);
        assert!(call(ndims, &[], 1).is_err());
    }

    /// Cycle 14: `values_equal` compares every dimension before any
    /// element, so arrays of different shapes are unequal and neither is
    /// read past its end.
    #[test]
    fn values_equal_compares_every_dimension_first() {
        let a = nd(&[2, 2, 2]);
        assert!(values_equal(&a, &nd(&[2, 2, 2])));
        assert!(!values_equal(&a, &nd(&[2, 4])));
        assert!(!values_equal(&a, &nd(&[2, 2])));
        assert!(!values_equal(&nd(&[2, 2]), &a));
        assert!(!values_equal(&a, &nd(&[2, 2, 1, 2])));
        assert!(!values_equal(&nd(&[2, 1, 2]), &nd(&[2, 2, 1])));
    }

    fn named(
        name: &str,
        local: Option<(Rc<crate::interp::Unit>, Rc<crate::parser::Function>)>,
    ) -> Value {
        Value::Func(Rc::new(Func::Named {
            name: name.into(),
            local,
        }))
    }

    fn function(name: &str) -> Rc<crate::parser::Function> {
        Rc::new(crate::parser::Function {
            name: name.into(),
            outputs: Vec::new(),
            params: Vec::new(),
            body: Vec::new(),
            line: 1,
        })
    }

    /// A handle made from `def`, as each evaluation of `@(x) ...` makes one.
    fn anon_of(def: &Rc<crate::parser::AnonFn>) -> Value {
        Value::Func(Rc::new(Func::Anon {
            def: def.clone(),
            captured: vec![("A".into(), num(5.0))],
            unit: Rc::new(crate::interp::Unit::default()),
        }))
    }

    /// `@(x) name(x)`, made afresh.
    fn anon(name: &str) -> Value {
        use crate::parser::{Access, AnonFn, Expr};
        let body = Expr::Access(
            name.into(),
            vec![Access::Paren(vec![Expr::Ident("x".into())])],
        );
        anon_of(&Rc::new(AnonFn::new(vec!["x".into()], body)))
    }

    /// Cycle 15: two named handles are equal when they name the same
    /// function, the same name bound to the same local function of the same
    /// file where each was made, or to none for either (S4's `@sin` twice).
    #[test]
    fn named_handles_are_equal_when_they_bind_the_same_function() {
        assert!(values_equal(&named("sin", None), &named("sin", None)));
        assert!(!values_equal(&named("sin", None), &named("cos", None)));
        let unit = Rc::new(crate::interp::Unit::default());
        let loc = function("loc");
        let bound = || named("loc", Some((unit.clone(), loc.clone())));
        assert!(values_equal(&bound(), &bound()));
        // A local function of the name is not the name resolved when
        // called, and another file's function of the name is another.
        assert!(!values_equal(&bound(), &named("loc", None)));
        assert!(!values_equal(&named("loc", None), &bound()));
        let other = named(
            "loc",
            Some((Rc::new(crate::interp::Unit::default()), loc.clone())),
        );
        assert!(!values_equal(&bound(), &other));
        let again = named("loc", Some((unit.clone(), function("loc"))));
        assert!(!values_equal(&bound(), &again));
        // A handle equals nothing but a handle.
        assert!(!values_equal(&named("sin", None), &num(1.0)));
        assert!(!values_equal(&num(1.0), &named("sin", None)));
        assert!(!values_equal(&named("sin", None), &Value::str("sin")));
        let yes = |args: &[Value]| mat_of(isequal, args).data[0] == 1.0;
        assert!(yes(&[
            named("sin", None),
            named("sin", None),
            named("sin", None)
        ]));
        assert!(!yes(&[
            named("sin", None),
            named("sin", None),
            named("cos", None)
        ]));
    }

    /// Cycle 15: an anonymous function is equal only to its copies, the
    /// same handle passed on; two made from the same text are unequal, and
    /// a named handle never equals an anonymous one (S4).
    #[test]
    fn anonymous_handles_are_equal_only_to_their_copies() {
        // Two evaluations of one `@(x) A * x.^2`: one text, one capture.
        let def = Rc::new(crate::parser::AnonFn::new(
            vec!["x".into()],
            crate::parser::Expr::Ident("x".into()),
        ));
        let (h1, h2) = (anon_of(&def), anon_of(&def));
        assert!(!values_equal(&h1, &h2));
        assert!(!values_equal(&anon("sin"), &anon("sin")));
        let copy = h1.clone();
        assert!(values_equal(&h1, &copy));
        assert!(values_equal(&h1, &h1));
        let held = Value::cell(crate::value::CellArray::new(1, 1, vec![h1.clone()]));
        let Value::Cell(c) = &held else {
            unreachable!()
        };
        assert!(values_equal(&c.data[0], &h1));
        assert!(!values_equal(&named("sin", None), &anon("@(x) sin(x)")));
        assert!(!values_equal(&anon("@(x) sin(x)"), &named("sin", None)));
    }

    fn cell_of(rows: usize, cols: usize, data: Vec<Value>) -> Value {
        Value::cell(crate::value::CellArray::new(rows, cols, data))
    }

    fn struct_of(fields: &[&str], values: Vec<Value>) -> Value {
        Value::strukt(StructArray::scalar(
            fields.iter().map(|f| f.to_string()).collect(),
            values,
        ))
    }

    /// Cycle 15: cells and structs compare element by element, by the rules
    /// arrays compare by, and never equal a value of another kind (S3).
    #[test]
    fn cells_and_structs_compare_every_element() {
        let pair = |a: Value, b: Value| cell_of(1, 2, vec![a, b]);
        assert!(values_equal(
            &pair(num(1.0), Value::str("a")),
            &pair(num(1.0), Value::str("a"))
        ));
        assert!(!values_equal(
            &pair(num(1.0), Value::str("a")),
            &pair(num(1.0), Value::str("b"))
        ));
        // The same elements in another shape are another cell.
        let col = cell_of(2, 1, vec![num(1.0), num(2.0)]);
        assert!(!values_equal(&pair(num(1.0), num(2.0)), &col));
        // The class is not compared, and a NaN equals nothing.
        assert!(values_equal(
            &cell_of(1, 1, vec![Value::str("a")]),
            &cell_of(1, 1, vec![num(97.0)])
        ));
        let nan = || cell_of(1, 1, vec![num(f64::NAN)]);
        assert!(!values_equal(&nan(), &nan()));
        let shared = nan();
        assert!(!values_equal(&shared, &shared));
        assert!(values_equal(&cell_of(0, 0, vec![]), &cell_of(0, 0, vec![])));
        // Nested cells, and handles inside them.
        let nest = || cell_of(1, 1, vec![pair(num(1.0), cell_of(1, 1, vec![num(2.0)]))]);
        assert!(values_equal(&nest(), &nest()));
        let sin = || cell_of(1, 1, vec![named("sin", None)]);
        assert!(values_equal(&sin(), &sin()));
        // A cell never equals an array or a struct, nor a struct an array.
        let one = cell_of(1, 1, vec![num(1.0)]);
        assert!(!values_equal(&one, &num(1.0)));
        assert!(!values_equal(&num(1.0), &one));
        let s = struct_of(&["a"], vec![num(1.0)]);
        assert!(!values_equal(&s, &one));
        assert!(!values_equal(&one, &s));
        assert!(!values_equal(&s, &num(1.0)));
        // A struct array: the same size and every field of every element.
        let arr = |second: f64| {
            Value::strukt(StructArray::new(
                1,
                2,
                vec!["a".into()],
                vec![vec![num(1.0)], vec![num(second)]],
            ))
        };
        assert!(values_equal(&arr(2.0), &arr(2.0)));
        assert!(!values_equal(&arr(2.0), &arr(3.0)));
        assert!(!values_equal(&arr(2.0), &s));
        let nested = |v: f64| {
            struct_of(
                &["c"],
                vec![cell_of(1, 1, vec![struct_of(&["d"], vec![num(v)])])],
            )
        };
        assert!(values_equal(&nested(1.0), &nested(1.0)));
        assert!(!values_equal(&nested(1.0), &nested(2.0)));
        // An MException inside a cell compares as it does alone.
        let caught = |m: &str| cell_of(1, 1, vec![Value::Exception(crate::error::MError::new(m))]);
        assert!(values_equal(&caught("x"), &caught("x")));
        assert!(!values_equal(&caught("x"), &caught("y")));
        let yes = |args: &[Value]| mat_of(isequal, args).data[0] == 1.0;
        let three = || pair(num(1.0), num(2.0));
        assert!(yes(&[three(), three(), three()]));
        assert!(!yes(&[three(), three(), col.clone()]));
    }

    /// Cycle 15: "Fields need not be in the same order as long as the
    /// contents are equal" (S3), and the fields are matched through the
    /// struct's field index, so 100,000 fields in the reverse order are
    /// matched in time proportional to their count.
    #[test]
    fn struct_fields_compare_in_any_order() {
        let s = struct_of(&["a", "b"], vec![num(1.0), Value::str("x")]);
        let t = struct_of(&["b", "a"], vec![Value::str("x"), num(1.0)]);
        assert!(values_equal(&s, &t));
        let t2 = struct_of(&["b", "a"], vec![Value::str("y"), num(1.0)]);
        assert!(!values_equal(&s, &t2));
        // Missing, extra or other fields are unequal.
        assert!(!values_equal(&s, &struct_of(&["a"], vec![num(1.0)])));
        assert!(!values_equal(&struct_of(&["a"], vec![num(1.0)]), &s));
        assert!(!values_equal(
            &s,
            &struct_of(&["a", "c"], vec![num(1.0), Value::str("x")])
        ));
        // A name repeated in one struct matches no struct of distinct names.
        let twice = struct_of(&["a", "a"], vec![num(1.0), num(1.0)]);
        assert!(!values_equal(
            &twice,
            &struct_of(&["a", "b"], vec![num(1.0), num(1.0)])
        ));
        let n = 100_000;
        let names: Vec<String> = (0..n).map(|k| format!("f{k}")).collect();
        let forward = StructArray::scalar(names.clone(), (0..n).map(|k| num(k as f64)).collect());
        let reverse = StructArray::scalar(
            names.iter().rev().cloned().collect(),
            (0..n).rev().map(|k| num(k as f64)).collect(),
        );
        let started = std::time::Instant::now();
        assert!(values_equal(
            &Value::strukt(forward.clone()),
            &Value::strukt(reverse.clone())
        ));
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        let mut changed = reverse;
        changed.elems[0][0] = num(-1.0);
        assert!(!values_equal(
            &Value::strukt(forward),
            &Value::strukt(changed)
        ));
    }

    /// Cycle 15: two nestings 100,000 deep, and deeper, are compared by the
    /// worklist without recursion, so neither overflows the stack; a nest
    /// with exponentially many paths through few containers is compared
    /// once per pair of containers.
    #[test]
    fn a_deep_nesting_compares_without_recursion() {
        let depth = 200_000;
        let cells = |inner: f64| {
            let mut v = num(inner);
            for _ in 0..depth {
                v = cell_of(1, 1, vec![v]);
            }
            v
        };
        let (a, b, c) = (cells(1.0), cells(1.0), cells(2.0));
        assert!(values_equal(&a, &b));
        assert!(!values_equal(&a, &c));
        let structs = |inner: f64| {
            let mut v = num(inner);
            for _ in 0..depth {
                v = struct_of(&["f"], vec![v]);
            }
            v
        };
        let (s, t, u) = (structs(1.0), structs(1.0), structs(2.0));
        assert!(values_equal(&s, &t));
        assert!(!values_equal(&s, &u));
        // `e = {e, e}` sixty times: 2^60 paths, 61 containers.
        let mut e = num(1.0);
        let mut f = num(1.0);
        for _ in 0..60 {
            e = cell_of(1, 2, vec![e.clone(), e]);
            f = cell_of(1, 2, vec![f.clone(), f]);
        }
        assert!(values_equal(&e, &f));
        assert!(values_equal(&e, &e));
    }

    #[test]
    fn whos_writes_every_dimension() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        it.vars_mut().insert("A".into(), nd(&[2, 3, 4]));
        let rows: Vec<[String; 4]> = vec![[
            "A".into(),
            crate::value::dims_text(&it.vars()["A"].dims(), "x"),
            bytes(&it.vars()["A"]).to_string(),
            "double".into(),
        ]];
        assert_eq!(whos_text(&rows), "  A            2x3x4   192  double\n\n");
    }

    /// Cycle 14: a size text or a name longer than the 65,535 characters
    /// Rust's formatter takes as a width is written whole, padded by hand,
    /// where a formatted width panicked.
    #[test]
    fn whos_pads_a_column_wider_than_any_format_width() {
        let mut dims = vec![2usize];
        dims.extend(std::iter::repeat_n(1, 40_000));
        dims.push(2);
        let long = crate::value::dims_text(&dims, "x");
        assert!(long.len() > 65_535);
        let name = "b".repeat(70_000);
        let rows: Vec<[String; 4]> = vec![
            ["A".into(), long.clone(), "32".into(), "double".into()],
            [name.clone(), "1x1".into(), "8".into(), "double".into()],
        ];
        let text = whos_text(&rows);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        let first = format!("  A{} {long}   32  double", " ".repeat(69_999));
        assert_eq!(lines[0], first);
        let pad = " ".repeat(long.len() - 3);
        assert_eq!(lines[1], format!("  {name} {pad}1x1    8  double"));
        assert_eq!(lines[2], "");
    }
}
