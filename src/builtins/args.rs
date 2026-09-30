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

/// Argument `i` as a matrix, class and all. A char is its code units, so a
/// numeric builtin reads `'a'` as `97`.
pub fn mat(args: &[Value], i: usize, name: &str) -> R<Matrix> {
    args.get(i)
        .ok_or_else(|| error::not_enough_args(name))?
        .clone()
        .into_mat()
}

/// Argument `i` as a real scalar. A complex one is refused rather than
/// read by its real part (cycle 10), which is what keeps a dimension, a
/// size or an option value of a builtin that takes complex data honest.
pub fn scalar(args: &[Value], i: usize, name: &str) -> R<f64> {
    let m = mat(args, i, name)?;
    m.require_real(name)?;
    m.scalar_value()
        .ok_or_else(|| error::arg_not_a_scalar(i + 1, name))
}

/// Argument `i` as a dimension: a positive integer. MATLAB rejects `0` and a
/// fractional dimension; a dimension past the array's is a singleton, which is
/// the caller's business, not this helper's.
///
/// A char argument is never a dimension. `sum(A, 'x')` used to read `'x'` as
/// its character code and reduce along dimension 120.
pub fn dim(args: &[Value], i: usize, name: &str) -> R<usize> {
    if args.get(i).is_some_and(Value::is_char) {
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
    match option(args, i) {
        Some(s) if s.eq_ignore_ascii_case("all") => Ok(Along::All),
        _ => dim(args, i, name).map(Along::Dim),
    }
}

/// Argument `i` when it is a char option such as `'descend'`, and `None` when
/// it is numeric or missing. The caller decides which options it knows.
pub fn option(args: &[Value], i: usize) -> Option<String> {
    args.get(i).and_then(Value::text)
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
    if args.get(i).is_some_and(Value::is_char) {
        return Err(error::bad_size_arg(name));
    }
    size_value(scalar(args, i, name)?, name)
}

/// Argument `i` as text: a char, decoded from its UTF-16 code units in
/// column-major order. Any other class is refused.
pub fn string(args: &[Value], i: usize, name: &str) -> R<String> {
    match args.get(i) {
        Some(v) => v.text().ok_or_else(|| error::arg_not_a_string(i + 1, name)),
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
    for a in rest {
        a.mat()?.require_real(name)?;
    }
    if let [one] = rest {
        if one.is_char() {
            return Err(error::bad_size_arg(name));
        }
        let m = one.mat()?;
        if let Some(n) = m.scalar_value() {
            let n = size_value(n, name)?;
            return Ok(vec![Some(n), Some(n)]);
        }
        if m.is_empty() {
            return Ok(vec![Some(0.0), Some(0.0)]);
        }
        // A 1x1x3 has one row, and is still no row vector (cycle 14).
        if m.rows != 1 || m.is_nd() {
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
        .map(|(k, a)| match a.mat()? {
            m if m.is_char() => Err(error::bad_size_arg(name)),
            m if m.is_scalar() => size_value(m.data[0], name).map(Some),
            m if auto && m.is_empty() => Ok(None),
            _ => Err(error::arg_not_a_scalar(from + k + 1, name)),
        })
        .collect()
}

/// Two dimensions out of a list of sizes. Trailing sizes of `1` are dropped,
/// as MATLAB drops them; any other third or later size, `0` included, would
/// need an N-D array, which the callers that come here (`eye`, `cell` and
/// `repmat`, through [`shape`]) cannot make yet. `dims` has at least two
/// entries.
pub fn trailing_ones(dims: &[f64]) -> R<(f64, f64)> {
    if dims[2..].iter().any(|&d| d != 1.0) {
        return Err(error::nd_unsupported());
    }
    Ok((dims[0], dims[1]))
}

/// The requested `(rows, cols)` of a builtin that makes a 2-D array only,
/// `eye`, `cell` and `repmat` since cycle 14 gave the other constructors
/// [`shape_dims`]: no size is 1x1, and otherwise [`size_list`] then
/// [`trailing_ones`]. More than `max_dims` sizes is the N-D error even when
/// they are ones, which is how `eye` keeps its two-size limit for a size
/// vector.
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

/// The sizes of a constructor that takes any number of dimensions (cycle
/// 14): no size is 1x1, and otherwise [`size_list`], with every trailing
/// size of `1` past the second dropped, as MATLAB drops it, so
/// `zeros(2, 3, 1)` asks for 2x3 and `zeros(2, 3, 1, 4)` for 2x3x1x4. The
/// sizes stay `f64` for [`check_dims`] to judge and name.
pub fn shape_dims(args: &[Value], from: usize, name: &str) -> R<Vec<f64>> {
    let list = size_list(args, from, name, false)?;
    if list.is_empty() {
        return Ok(vec![1.0, 1.0]);
    }
    let mut dims: Vec<f64> = list.into_iter().map(|d| d.unwrap_or(0.0)).collect();
    while dims.len() > 2 && dims.last() == Some(&1.0) {
        dims.pop();
    }
    Ok(dims)
}

/// A shape of any number of dimensions (cycle 14), judged before anything
/// is allocated: every size a non-negative integer or `+Inf`, still as
/// `f64`, below `usize`, and their product at most [`MAX_ELEMS`]. The
/// refusal is [`check_shape`]'s, naming every size as it was asked for:
/// `Requested 100000x100000x100000 array exceeds the maximum array size.`
/// Every N-D shape the interpreter computes, a constructor's, a reshape's,
/// a broadcast's, a read's and a growth's, comes here.
pub fn check_dims(dims: &[f64]) -> R<Vec<usize>> {
    // `usize::MAX as f64` rounds up to 2^64, so anything below it fits.
    let limit = usize::MAX as f64;
    if dims.iter().all(|&d| d < limit) {
        let out: Vec<usize> = dims.iter().map(|&d| d as usize).collect();
        // A size of 0 makes the product 0 wherever it stands, so an empty
        // shape is judged alike in any order: `zeros(2^60, 2^60, 0)` as
        // `zeros(0, 2^60, 2^60)`, where a running product would overflow
        // before it reached the 0.
        let total = if out.contains(&0) {
            Some(0)
        } else {
            out.iter().try_fold(1usize, |n, &d| n.checked_mul(d))
        };
        if total.is_some_and(|n| n <= MAX_ELEMS) {
            return Ok(out);
        }
    }
    Err(overflow(dims))
}

/// The refusal of a shape too large to hold, every size named.
fn overflow(dims: &[f64]) -> error::MError {
    let named: Vec<String> = dims.iter().map(|&d| fmt_dim(d)).collect();
    error::size_overflow(&named)
}

/// A shape the user asked for, judged before anything is allocated. Both
/// sizes are non-negative integers or `+Inf`, still as `f64`, so a size past
/// `usize` is refused under its own name: `zeros(1e300)` reports
/// `1e+300x1e+300`, not the `usize::MAX` it used to saturate to. Constructors,
/// `reshape`, `repmat`, `diag`, `linspace`, the `:` operator and, since cycle
/// 03, indexed growth all come here; `check_size`, which took sizes already
/// saturated to `usize`, went with the growth path that used it. It is
/// [`check_dims`] of two sizes.
pub fn check_shape(rows: f64, cols: f64) -> R<(usize, usize)> {
    let d = check_dims(&[rows, cols])?;
    Ok((d[0], d[1]))
}

/// The memory budget of any one array: what a double array at
/// [`MAX_ELEMS`] takes, 2^28 elements of 8 bytes, 2 GiB (cycle 13b).
pub const MAX_BYTES: usize = MAX_ELEMS * std::mem::size_of::<f64>();

/// The bytes one element of a cell array is counted as: a whole value.
pub const CELL_UNIT: usize = std::mem::size_of::<Value>();

/// The bytes one element of a struct array with `fields` fields is counted
/// as: a value per field, and one more for the element's own field list.
pub fn struct_unit(fields: usize) -> usize {
    CELL_UNIT.saturating_mul(fields.saturating_add(1))
}

/// [`check_shape`] for an array whose elements take `unit` bytes each: the
/// element cap, and then the byte budget a double array has. A cell or a
/// struct element costs many times a double, so the element cap alone let
/// `cell(1, 2^27)` ask for gigabytes (cycle 13b). The refusal is
/// `check_shape`'s own, naming the size that was asked for.
pub fn check_bytes(rows: f64, cols: f64, unit: usize) -> R<(usize, usize)> {
    let (r, c) = check_shape(rows, cols)?;
    if (r * c).checked_mul(unit).is_some_and(|b| b <= MAX_BYTES) {
        return Ok((r, c));
    }
    Err(overflow(&[rows, cols]))
}

/// [`check_bytes`] for a cell array of `rows x cols`.
pub fn check_cell(rows: usize, cols: usize) -> R<(usize, usize)> {
    check_bytes(rows as f64, cols as f64, CELL_UNIT)
}

/// [`check_bytes`] for a struct array of `rows x cols` with `fields` fields.
pub fn check_struct(rows: usize, cols: usize, fields: usize) -> R<(usize, usize)> {
    check_bytes(rows as f64, cols as f64, struct_unit(fields))
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
        let a = [Value::str("AB")];
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
        let a = [num(1.0), Value::str("x")];
        let e = dim(&a, 1, "sum").unwrap_err().msg;
        assert_eq!(
            e,
            "Dimension argument to 'sum' must be a positive integer scalar."
        );
        let all = [num(1.0), Value::str("all")];
        assert!(dim(&all, 1, "sum").is_err());
        assert_eq!(dim_or_all(&all, 1, "sum").unwrap(), Along::All);
        let upper = [num(1.0), Value::str("ALL")];
        assert_eq!(dim_or_all(&upper, 1, "sum").unwrap(), Along::All);
        assert_eq!(
            dim_or_all(&[num(1.0), num(2.0)], 1, "sum").unwrap(),
            Along::Dim(2)
        );
        assert!(dim_or_all(&a, 1, "sum").is_err());
        assert_eq!(option(&all, 1).as_deref(), Some("all"));
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
            size_arg(&[Value::str("a")], 0, "zeros").unwrap_err().msg,
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
            sizes(&[num(2.0), Value::str("a")]).unwrap_err().msg,
            size_msg()
        );
        assert_eq!(sizes(&[row(&[2.0, 1.5])]).unwrap_err().msg, size_msg());
    }

    #[test]
    fn the_size_parser_drops_trailing_ones_and_nothing_else() {
        // `shape` is the two-size parser `eye`, `cell` and `repmat` keep
        // (cycle 14): a third size other than 1 is still its refusal. The
        // constructors that make N-D arrays read `shape_dims` instead.
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

    /// Cycle 13b: a cell or struct array is held to the bytes a double
    /// array at the element cap takes, 2^28 x 8 bytes.
    #[test]
    fn cells_and_structs_are_bounded_by_bytes() {
        assert_eq!(MAX_BYTES, 1 << 31);
        assert_eq!(CELL_UNIT, std::mem::size_of::<Value>());
        assert_eq!(struct_unit(0), CELL_UNIT);
        assert_eq!(struct_unit(2), 3 * CELL_UNIT);
        assert_eq!(struct_unit(usize::MAX), usize::MAX);
        let most = MAX_BYTES / CELL_UNIT;
        assert_eq!(check_cell(1, most).unwrap(), (1, most));
        let msg = check_cell(1, most + 1).unwrap_err().msg;
        assert_eq!(
            msg,
            format!(
                "Requested 1x{} array exceeds the maximum array size.",
                most + 1
            )
        );
        assert!(check_cell(1, 1 << 27).is_err());
        assert!(check_cell(1000, 1).is_ok());
        assert!(check_cell(0, 1 << 40).is_ok());
        // A struct element costs a value per field and one more.
        let per = MAX_BYTES / struct_unit(1);
        assert!(check_struct(per, 1, 1).is_ok());
        assert!(check_struct(per + 1, 1, 1).is_err());
        assert!(check_struct(1, 1 << 27, 1).is_err());
        assert!(check_struct(1000, 1, 50).is_ok());
        // The element cap still comes first, with its own message.
        assert!(
            check_bytes(1e300, 1.0, 1)
                .unwrap_err()
                .msg
                .contains("1e+300x1")
        );
        // A double array is judged the same by both.
        assert!(check_bytes(MAX_ELEMS as f64, 1.0, 8).is_ok());
    }

    /// Cycle 14: any number of sizes, judged together, at and past the
    /// element cap, and refused naming every size as it was asked for.
    #[test]
    fn check_dims_judges_every_size_before_allocating() {
        assert_eq!(check_dims(&[2.0, 3.0, 4.0]).unwrap(), [2, 3, 4]);
        assert_eq!(
            check_dims(&[MAX_ELEMS as f64 / 4.0, 2.0, 2.0]).unwrap()[0],
            MAX_ELEMS / 4
        );
        let at = [1024.0, 1024.0, (MAX_ELEMS / (1 << 20)) as f64];
        assert!(check_dims(&at).is_ok());
        let past = [1024.0, 1024.0, (MAX_ELEMS / (1 << 20)) as f64 + 1.0];
        assert_eq!(
            check_dims(&past).unwrap_err().msg,
            "Requested 1024x1024x257 array exceeds the maximum array size."
        );
        assert_eq!(
            check_dims(&[1e5, 1e5, 1e5]).unwrap_err().msg,
            "Requested 100000x100000x100000 array exceeds the maximum array size."
        );
        // A product that would wrap `usize` is refused, not wrapped.
        let wrap = [4294967296.0, 4294967296.0, 2.0];
        assert!(check_dims(&wrap).is_err());
        // An empty shape is free, but a size past `usize` is still named.
        assert_eq!(
            check_dims(&[0.0, 1e15, 3.0]).unwrap(),
            [0, 1_000_000_000_000_000, 3]
        );
        assert!(
            check_dims(&[0.0, 1e300, 3.0])
                .unwrap_err()
                .msg
                .contains("0x1e+300x3")
        );
        // A 0 anywhere makes the product 0, so the order of the sizes
        // never decides: sizes whose running product would pass `usize`
        // before the 0 are empty too.
        let huge = 2f64.powi(60);
        assert_eq!(
            check_dims(&[huge, huge, 0.0]).unwrap(),
            [1 << 60, 1 << 60, 0]
        );
        assert_eq!(
            check_dims(&[huge, 0.0, huge]).unwrap(),
            [1 << 60, 0, 1 << 60]
        );
        assert!(check_dims(&[huge, huge, 1.0]).is_err());
        // check_shape is check_dims of two sizes.
        assert_eq!(check_shape(2.0, 3.0).unwrap(), (2, 3));
    }

    #[test]
    fn shape_dims_drops_trailing_ones_and_keeps_every_other_size() {
        let dims = |a: &[Value]| shape_dims(a, 0, "zeros").unwrap();
        assert_eq!(dims(&[]), [1.0, 1.0]);
        assert_eq!(dims(&[num(3.0)]), [3.0, 3.0]);
        assert_eq!(dims(&[num(2.0), num(3.0), num(4.0)]), [2.0, 3.0, 4.0]);
        assert_eq!(dims(&[num(2.0), num(3.0), num(1.0)]), [2.0, 3.0]);
        assert_eq!(
            dims(&[num(2.0), num(3.0), num(1.0), num(4.0)]),
            [2.0, 3.0, 1.0, 4.0]
        );
        assert_eq!(dims(&[row(&[2.0, 2.0, 2.0, 1.0])]), [2.0, 2.0, 2.0]);
        assert_eq!(dims(&[num(2.0), num(3.0), num(0.0)]), [2.0, 3.0, 0.0]);
        // A size vector is a row, and a 1x1x3 is none.
        let nd = Value::Mat(Matrix::from_dims(&[1, 1, 3], vec![1.0; 3]));
        assert_eq!(
            shape_dims(&[nd], 0, "zeros").unwrap_err().msg,
            "Size vector for 'zeros' must be a row vector."
        );
    }
}
