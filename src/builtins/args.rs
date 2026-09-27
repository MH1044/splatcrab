//! Argument access for builtins.
//!
//! These replace the closures that used to be defined inside `call_builtin`,
//! and produce the same MATLAB-style messages. Every helper takes the
//! builtin's own name so the message can name the function the user called.
//!
//! The size helpers are also the single place where a user-supplied number
//! becomes an allocation length. `zeros(1e10)` used to abort the process when
//! `rows * cols` overflowed, which broke the "errors are values, not panics"
//! invariant, so every product here is checked.

use crate::error::{self, R};
use crate::interp::fmt_g;
use crate::value::{Matrix, Value};

/// Largest array a constructor will build: 2 GiB of `f64`. MATLAB has the
/// same idea under a different name ("maximum array size preference").
pub const MAX_ELEMS: usize = 1 << 28;

/// Lower bound on the argument count: "Not enough input arguments."
pub fn need(args: &[Value], n: usize, name: &str) -> R<()> {
    if args.len() < n {
        Err(error::not_enough_args(name))
    } else {
        Ok(())
    }
}

/// Upper bound on the argument count: "Too many input arguments."
pub fn at_most(args: &[Value], n: usize, _name: &str) -> R<()> {
    if args.len() > n {
        Err(error::too_many_args())
    } else {
        Ok(())
    }
}

/// Argument `i` as a matrix. A string becomes its character codes.
pub fn mat(args: &[Value], i: usize, name: &str) -> R<Matrix> {
    args.get(i)
        .cloned()
        .map(|v| v.into_mat())
        .ok_or_else(|| error::not_enough_args(name))
}

/// Argument `i` as a scalar.
pub fn scalar(args: &[Value], i: usize, name: &str) -> R<f64> {
    mat(args, i, name)?
        .scalar_value()
        .ok_or_else(|| error::arg_not_a_scalar(i + 1, name))
}

/// Argument `i` as a dimension: a positive integer. MATLAB rejects `0` and a
/// fractional dimension; a dimension past the array's is a singleton, which is
/// the caller's business, not this helper's.
///
/// A char argument is never a dimension. `sum(A, 'x')` used to read `'x'` as
/// its character code and reduce along dimension 120.
pub fn dim(args: &[Value], i: usize, name: &str) -> R<usize> {
    if let Some(Value::Str(_)) = args.get(i) {
        return Err(error::bad_dim_arg(name));
    }
    let v = scalar(args, i, name)?;
    if v.is_nan() || v < 1.0 || v.fract() != 0.0 {
        return Err(error::bad_dim_arg(name));
    }
    Ok(clamp_to_usize(v))
}

/// What a reduction runs along: one dimension, or every element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Along {
    Dim(usize),
    /// `'all'`: the reduction over `A(:)`.
    All,
}

/// Argument `i` as a dimension or the option `'all'`. Only the builtins that
/// accept `'all'` call this; any other char is the ordinary dimension error.
pub fn dim_or_all(args: &[Value], i: usize, name: &str) -> R<Along> {
    match args.get(i) {
        Some(Value::Str(s)) if s.eq_ignore_ascii_case("all") => Ok(Along::All),
        _ => dim(args, i, name).map(Along::Dim),
    }
}

/// Argument `i` when it is a char option such as `'descend'`, and `None` when
/// it is numeric or missing. The caller decides which options it knows.
pub fn option(args: &[Value], i: usize) -> Option<&str> {
    match args.get(i) {
        Some(Value::Str(s)) => Some(s),
        _ => None,
    }
}

/// One requested size: a non-negative integer, where a negative value is `0`
/// exactly as in MATLAB (`zeros(-1)` is `0x0`). It stays an `f64`, because a
/// size past `usize` must reach `check_shape` intact for the message to name
/// it.
pub fn size_value(v: f64, name: &str) -> R<f64> {
    if v.is_nan() || v.fract() != 0.0 {
        return Err(error::bad_size_arg(name));
    }
    Ok(v.max(0.0))
}

/// Argument `i` as one requested size; see [`size_value`]. A char is never a
/// size.
pub fn size_arg(args: &[Value], i: usize, name: &str) -> R<f64> {
    if let Some(Value::Str(_)) = args.get(i) {
        return Err(error::bad_size_arg(name));
    }
    size_value(scalar(args, i, name)?, name)
}

/// Argument `i` as a character vector.
pub fn string(args: &[Value], i: usize, name: &str) -> R<String> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.clone()),
        Some(Value::Mat(_)) => Err(error::arg_not_a_string(i + 1, name)),
        None => Err(error::not_enough_args(name)),
    }
}

/// The sizes in `args[from..]`, the one parser every constructor, `reshape`
/// and `repmat` read their shape through. It accepts:
///
/// - one scalar `n`, which is `n` by `n`;
/// - one row vector of sizes, which is how `zeros(size(A))` works. An empty
///   one is `0x0`, as in Octave; a column or a matrix is an error;
/// - two or more scalars. With `auto`, an empty argument among them is the
///   `[]` placeholder that `reshape` takes, returned as `None`.
///
/// No arguments give an empty list; the caller knows whether that means 1x1.
pub fn size_list(args: &[Value], from: usize, name: &str, auto: bool) -> R<Vec<Option<f64>>> {
    let rest = args.get(from..).unwrap_or(&[]);
    if let [one] = rest {
        let m = match one {
            Value::Str(_) => return Err(error::bad_size_arg(name)),
            Value::Mat(m) => m,
        };
        if let Some(n) = m.scalar_value() {
            let n = size_value(n, name)?;
            return Ok(vec![Some(n), Some(n)]);
        }
        if m.is_empty() {
            return Ok(vec![Some(0.0), Some(0.0)]);
        }
        if m.rows != 1 {
            return Err(error::size_vector_not_row(name));
        }
        return m
            .data
            .iter()
            .map(|&v| size_value(v, name).map(Some))
            .collect();
    }
    rest.iter()
        .enumerate()
        .map(|(k, a)| match a {
            Value::Str(_) => Err(error::bad_size_arg(name)),
            Value::Mat(m) if m.is_scalar() => size_value(m.data[0], name).map(Some),
            Value::Mat(m) if auto && m.is_empty() => Ok(None),
            Value::Mat(_) => Err(error::arg_not_a_scalar(from + k + 1, name)),
        })
        .collect()
}

/// Two dimensions out of a list of sizes. Trailing sizes of `1` are dropped,
/// as MATLAB drops them; any other third or later size, `0` included, would
/// need an N-D array. `dims` has at least two entries.
pub fn trailing_ones(dims: &[f64]) -> R<(f64, f64)> {
    if dims[2..].iter().any(|&d| d != 1.0) {
        return Err(error::nd_unsupported());
    }
    Ok((dims[0], dims[1]))
}

/// The requested `(rows, cols)` of a constructor: no size is 1x1, and
/// otherwise [`size_list`] then [`trailing_ones`]. More than `max_dims` sizes
/// is the N-D error even when they are ones, which is how `eye` keeps its
/// two-size limit for a size vector.
pub fn shape(args: &[Value], from: usize, name: &str, max_dims: usize) -> R<(f64, f64)> {
    let list = size_list(args, from, name, false)?;
    if list.is_empty() {
        return Ok((1.0, 1.0));
    }
    let dims: Vec<f64> = list.into_iter().map(|d| d.unwrap_or(0.0)).collect();
    if dims.len() > max_dims {
        return Err(error::nd_unsupported());
    }
    trailing_ones(&dims)
}

/// `rows * cols`, refusing to overflow or to ask for an absurd allocation.
/// Indexed growth reaches it with sizes that are already `usize`; every
/// requested size goes through `check_shape` instead.
pub fn check_size(rows: usize, cols: usize) -> R<usize> {
    match rows.checked_mul(cols) {
        Some(n) if n <= MAX_ELEMS => Ok(n),
        _ => Err(error::size_overflow(&rows.to_string(), &cols.to_string())),
    }
}

/// A shape the user asked for, judged before anything is allocated. Both
/// sizes are non-negative integers or `+Inf`, still as `f64`, so a size past
/// `usize` is refused under its own name: `zeros(1e300)` reports
/// `1e+300x1e+300`, not the `usize::MAX` it used to saturate to. Constructors,
/// `reshape`, `repmat`, `diag`, `linspace` and the `:` operator all come here.
pub fn check_shape(rows: f64, cols: f64) -> R<(usize, usize)> {
    // `usize::MAX as f64` rounds up to 2^64, so anything below it fits.
    let limit = usize::MAX as f64;
    if rows < limit && cols < limit {
        let (r, c) = (rows as usize, cols as usize);
        if r.checked_mul(c).is_some_and(|n| n <= MAX_ELEMS) {
            return Ok((r, c));
        }
    }
    Err(error::size_overflow(&fmt_dim(rows), &fmt_dim(cols)))
}

/// One requested dimension as a message names it. An integer below 2^53 is
/// exact in an `f64` and prints in full, so `10000000000x10000000000` and
/// `1x1000000000000000` read as they always have. Anything else prints in
/// `%g` form (`1e+300`), and an infinite count prints `Inf`.
pub fn fmt_dim(v: f64) -> String {
    const EXACT: f64 = 9_007_199_254_740_992.0; // 2^53
    if (0.0..EXACT).contains(&v) && v.fract() == 0.0 {
        format!("{}", v as u64)
    } else {
        fmt_g(v, 6)
    }
}

/// A float that is already known to be non-negative and integral, as a length.
/// Values beyond `usize` saturate.
fn clamp_to_usize(v: f64) -> usize {
    if v >= usize::MAX as f64 {
        usize::MAX
    } else {
        v as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    #[test]
    fn need_and_at_most_use_the_matlab_wording() {
        let a = [num(1.0)];
        assert!(need(&a, 1, "abs").is_ok());
        assert_eq!(
            need(&a, 2, "mod").unwrap_err().msg,
            "Not enough input arguments for 'mod'."
        );
        assert!(at_most(&a, 1, "abs").is_ok());
        assert_eq!(
            at_most(&a, 0, "clc").unwrap_err().msg,
            "Too many input arguments."
        );
    }

    #[test]
    fn scalar_rejects_a_non_scalar_and_names_the_position() {
        let a = [Value::Mat(Matrix::row(vec![1.0, 2.0]))];
        assert_eq!(
            scalar(&a, 0, "zeros").unwrap_err().msg,
            "Argument 1 to 'zeros' must be a scalar."
        );
        assert_eq!(scalar(&[num(3.0)], 0, "zeros").unwrap(), 3.0);
        // A missing argument is reported as too few, not as a bad scalar.
        assert_eq!(
            scalar(&[], 0, "sqrt").unwrap_err().msg,
            "Not enough input arguments for 'sqrt'."
        );
    }

    #[test]
    fn mat_converts_a_string_to_character_codes() {
        let a = [Value::Str("AB".to_string())];
        assert_eq!(mat(&a, 0, "abs").unwrap().data, [65.0, 66.0]);
        assert_eq!(string(&a, 0, "clear").unwrap(), "AB");
        assert!(string(&[num(1.0)], 0, "clear").is_err());
    }

    #[test]
    fn dim_requires_a_positive_integer() {
        assert_eq!(dim(&[num(2.0)], 0, "sum").unwrap(), 2);
        for bad in [0.0, -1.0, 1.5, f64::NAN] {
            let e = dim(&[num(bad)], 0, "sum").unwrap_err().msg;
            assert!(e.contains("positive integer"), "{bad}: {e}");
        }
    }

    #[test]
    fn dim_never_reads_a_char_as_its_character_code() {
        // 'x' is the scalar 120 once converted; it must not become dimension
        // 120, which is what made sum(A, 'x') return A unchanged.
        let a = [num(1.0), Value::Str("x".to_string())];
        let e = dim(&a, 1, "sum").unwrap_err().msg;
        assert_eq!(
            e,
            "Dimension argument to 'sum' must be a positive integer scalar."
        );
        let all = [num(1.0), Value::Str("all".to_string())];
        assert!(dim(&all, 1, "sum").is_err());
        assert_eq!(dim_or_all(&all, 1, "sum").unwrap(), Along::All);
        let upper = [num(1.0), Value::Str("ALL".to_string())];
        assert_eq!(dim_or_all(&upper, 1, "sum").unwrap(), Along::All);
        assert_eq!(
            dim_or_all(&[num(1.0), num(2.0)], 1, "sum").unwrap(),
            Along::Dim(2)
        );
        assert!(dim_or_all(&a, 1, "sum").is_err());
        assert_eq!(option(&all, 1), Some("all"));
        assert_eq!(option(&all, 0), None);
        assert_eq!(option(&all, 5), None);
    }

    #[test]
    fn size_arg_treats_a_negative_size_as_zero() {
        assert_eq!(size_arg(&[num(3.0)], 0, "zeros").unwrap(), 3.0);
        assert_eq!(size_arg(&[num(0.0)], 0, "zeros").unwrap(), 0.0);
        assert_eq!(size_arg(&[num(-1.0)], 0, "zeros").unwrap(), 0.0);
        // A size past usize is kept as asked, for check_shape to name.
        assert_eq!(size_arg(&[num(1e300)], 0, "zeros").unwrap(), 1e300);
        assert_eq!(
            size_arg(&[Value::Str("a".to_string())], 0, "zeros")
                .unwrap_err()
                .msg,
            size_msg()
        );
        assert_eq!(
            size_arg(&[num(-7.5)], 0, "zeros").unwrap_err().msg,
            size_msg()
        );
        assert_eq!(
            size_arg(&[num(f64::NAN)], 0, "zeros").unwrap_err().msg,
            size_msg()
        );
    }

    fn size_msg() -> String {
        "Size arguments to 'zeros' must be non-negative integers.".to_string()
    }

    fn row(v: &[f64]) -> Value {
        Value::Mat(Matrix::row(v.to_vec()))
    }

    fn sizes(args: &[Value]) -> R<(f64, f64)> {
        shape(args, 0, "zeros", usize::MAX)
    }

    #[test]
    fn the_size_parser_takes_a_scalar_a_size_vector_or_several_sizes() {
        assert_eq!(sizes(&[]).unwrap(), (1.0, 1.0));
        assert_eq!(sizes(&[num(3.0)]).unwrap(), (3.0, 3.0));
        assert_eq!(sizes(&[num(2.0), num(3.0)]).unwrap(), (2.0, 3.0));
        assert_eq!(sizes(&[row(&[2.0, 3.0])]).unwrap(), (2.0, 3.0));
        // A one-element size vector is a scalar, n by n.
        assert_eq!(sizes(&[row(&[4.0])]).unwrap(), (4.0, 4.0));
        // An empty size vector is 0x0, as in Octave.
        assert_eq!(sizes(&[Value::Mat(Matrix::empty())]).unwrap(), (0.0, 0.0));
        // Negative sizes are 0 in every form.
        assert_eq!(sizes(&[row(&[-2.0, 3.0])]).unwrap(), (0.0, 3.0));
        // A column or a matrix is not a size vector.
        let col = Value::Mat(Matrix::col(vec![2.0, 3.0]));
        assert_eq!(
            sizes(&[col]).unwrap_err().msg,
            "Size vector for 'zeros' must be a row vector."
        );
        // Several arguments must each be a scalar; a char is never a size.
        assert_eq!(
            sizes(&[num(2.0), row(&[3.0, 4.0])]).unwrap_err().msg,
            "Argument 2 to 'zeros' must be a scalar."
        );
        assert_eq!(
            sizes(&[num(2.0), Value::Str("a".to_string())])
                .unwrap_err()
                .msg,
            size_msg()
        );
        assert_eq!(sizes(&[row(&[2.0, 1.5])]).unwrap_err().msg, size_msg());
    }

    #[test]
    fn the_size_parser_drops_trailing_ones_and_nothing_else() {
        let nd = "N-D arrays are not supported.";
        assert_eq!(sizes(&[num(2.0), num(3.0), num(1.0)]).unwrap(), (2.0, 3.0));
        assert_eq!(
            sizes(&[num(2.0), num(3.0), num(1.0), num(1.0)]).unwrap(),
            (2.0, 3.0)
        );
        assert_eq!(sizes(&[row(&[2.0, 3.0, 1.0])]).unwrap(), (2.0, 3.0));
        assert_eq!(sizes(&[num(2.0), num(3.0), num(4.0)]).unwrap_err().msg, nd);
        // A trailing 0 is not a trailing 1: it would be a 2x3x0 array.
        assert_eq!(sizes(&[num(2.0), num(3.0), num(0.0)]).unwrap_err().msg, nd);
        assert_eq!(
            sizes(&[num(2.0), num(3.0), num(1.0), num(2.0)])
                .unwrap_err()
                .msg,
            nd
        );
        assert_eq!(sizes(&[row(&[2.0, 3.0, 4.0])]).unwrap_err().msg, nd);
        // eye's limit: two sizes, however they are written.
        let eye = |a: &[Value]| shape(a, 0, "eye", 2);
        assert_eq!(eye(&[row(&[2.0, 3.0])]).unwrap(), (2.0, 3.0));
        assert_eq!(eye(&[row(&[2.0, 3.0, 1.0])]).unwrap_err().msg, nd);
        assert_eq!(trailing_ones(&[2.0, 3.0]).unwrap(), (2.0, 3.0));
    }

    #[test]
    fn size_list_returns_placeholders_only_when_asked() {
        let empty = Value::Mat(Matrix::empty());
        let a = [num(6.0), empty.clone(), num(2.0)];
        assert_eq!(
            size_list(&a, 1, "reshape", true).unwrap(),
            [None, Some(2.0)]
        );
        assert_eq!(
            size_list(&a, 1, "zeros", false).unwrap_err().msg,
            "Argument 2 to 'zeros' must be a scalar."
        );
        assert!(size_list(&a, 3, "zeros", false).unwrap().is_empty());
    }

    #[test]
    fn a_requested_size_prints_in_full_below_2_pow_53_and_as_g_beyond() {
        assert_eq!(fmt_dim(0.0), "0");
        assert_eq!(fmt_dim(1e10), "10000000000");
        assert_eq!(fmt_dim(1e15), "1000000000000000");
        let below = 9_007_199_254_740_991.0; // 2^53 - 1
        assert_eq!(fmt_dim(below), "9007199254740991");
        assert_eq!(fmt_dim(9_007_199_254_740_992.0), "9.0072e+15");
        assert_eq!(fmt_dim(1e300), "1e+300");
        assert_eq!(fmt_dim(f64::INFINITY), "Inf");
    }

    #[test]
    fn check_shape_names_the_size_that_was_asked_for() {
        assert_eq!(check_shape(2.0, 3.0).unwrap(), (2, 3));
        assert_eq!(check_shape(0.0, 1e15).unwrap(), (0, 1_000_000_000_000_000));
        let msg = |r, c| check_shape(r, c).unwrap_err().msg;
        assert_eq!(
            msg(1e300, 1e300),
            "Requested 1e+300x1e+300 array exceeds the maximum array size."
        );
        assert_eq!(
            msg(1.0, f64::INFINITY),
            "Requested 1xInf array exceeds the maximum array size."
        );
        // The messages that were already right stay exactly as they were.
        assert!(msg(1e10, 1e10).contains("10000000000x10000000000"));
        assert!(msg(1.0, 1e15).contains("1x1000000000000000"));
        // A size past usize is refused even when the product is zero.
        assert!(msg(0.0, 1e300).contains("0x1e+300"));
        assert!(check_shape(MAX_ELEMS as f64, 1.0).is_ok());
        assert!(check_shape(MAX_ELEMS as f64 + 1.0, 1.0).is_err());
    }

    #[test]
    fn check_size_refuses_to_overflow_or_to_allocate_the_world() {
        assert_eq!(check_size(2, 3).unwrap(), 6);
        assert_eq!(check_size(0, 9).unwrap(), 0);
        // The shape that used to abort the process.
        let huge = 10_000_000_000usize;
        let e = check_size(huge, huge).unwrap_err().msg;
        assert!(e.contains("10000000000x10000000000"), "{e}");
        // Just over the cap is refused too, without overflowing.
        assert!(check_size(MAX_ELEMS + 1, 1).is_err());
        assert!(check_size(MAX_ELEMS, 1).is_ok());
        assert!(check_size(usize::MAX, 2).is_err());
        // Indexed growth still names the usize it saturated to; cycle 03
        // rewrites that path.
        let e = check_size(1, usize::MAX).unwrap_err().msg;
        assert!(e.contains("1x18446744073709551615"), "{e}");
    }
}
