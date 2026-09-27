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
pub fn dim(args: &[Value], i: usize, name: &str) -> R<usize> {
    let v = scalar(args, i, name)?;
    if v.is_nan() || v < 1.0 || v.fract() != 0.0 {
        return Err(error::bad_dim_arg(name));
    }
    Ok(clamp_to_usize(v))
}

/// Argument `i` as a size: a non-negative integer, where a negative value is
/// `0` exactly as in MATLAB (`zeros(-1)` is `0x0`).
pub fn size_arg(args: &[Value], i: usize, name: &str) -> R<usize> {
    let v = scalar(args, i, name)?;
    if v.is_nan() || v.fract() != 0.0 {
        return Err(error::bad_size_arg(name));
    }
    if v < 0.0 {
        return Ok(0);
    }
    Ok(clamp_to_usize(v))
}

/// Argument `i` as a character vector.
pub fn string(args: &[Value], i: usize, name: &str) -> R<String> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.clone()),
        Some(Value::Mat(_)) => Err(error::arg_not_a_string(i + 1, name)),
        None => Err(error::not_enough_args(name)),
    }
}

/// `rows * cols`, refusing to overflow or to ask for an absurd allocation.
/// Every path that turns user input into an array length goes through here,
/// builtins and indexed growth alike.
pub fn check_size(rows: usize, cols: usize) -> R<usize> {
    match rows.checked_mul(cols) {
        Some(n) if n <= MAX_ELEMS => Ok(n),
        _ => Err(error::size_overflow(rows, cols)),
    }
}

/// A float that is already known to be non-negative and integral, as a length.
/// Values beyond `usize` saturate; `check_size` then rejects them.
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
    fn size_arg_treats_a_negative_size_as_zero() {
        assert_eq!(size_arg(&[num(3.0)], 0, "zeros").unwrap(), 3);
        assert_eq!(size_arg(&[num(0.0)], 0, "zeros").unwrap(), 0);
        assert_eq!(size_arg(&[num(-1.0)], 0, "zeros").unwrap(), 0);
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
    }
}
