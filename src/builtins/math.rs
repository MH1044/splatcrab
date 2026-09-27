//! Reductions, element-wise math and the two-argument numeric functions.

use super::args::{at_most, dim, mat, need};
use super::{Registry, add, one_mat};
use crate::interp::R;
use crate::value::{Matrix, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    // ---- reductions --------------------------------------------------
    add(r, "sum", |_, a, _| reduction(a, "sum", Red::Sum), "sum(A), sum(A,dim) - sum of the elements.");
    add(r, "prod", |_, a, _| reduction(a, "prod", Red::Prod), "prod(A), prod(A,dim) - product of the elements.");
    add(r, "mean", |_, a, _| reduction(a, "mean", Red::Mean), "mean(A), mean(A,dim) - average of the elements.");
    add(r, "any", |_, a, _| reduction(a, "any", Red::Any), "any(A), any(A,dim) - true if any element is non-zero.");
    add(r, "all", |_, a, _| reduction(a, "all", Red::All), "all(A), all(A,dim) - true if every element is non-zero.");
    add(r, "max", |_, a, _| extremum(a, "max", true), "max(A), max(A,B), max(A,[],dim) - largest elements.");
    add(r, "min", |_, a, _| extremum(a, "min", false), "min(A), min(A,B), min(A,[],dim) - smallest elements.");
    add(r, "cumsum", |_, a, _| cumulative(a, "cumsum", true), "cumsum(A), cumsum(A,dim) - cumulative sum.");
    add(r, "cumprod", |_, a, _| cumulative(a, "cumprod", false), "cumprod(A), cumprod(A,dim) - cumulative product.");

    // ---- element-wise math -------------------------------------------
    add(r, "abs", |_, a, _| unary(a, "abs", f64::abs), "abs(X) - absolute value.");
    add(r, "sqrt", |_, a, _| unary(a, "sqrt", f64::sqrt), "sqrt(X) - square root.");
    add(r, "exp", |_, a, _| unary(a, "exp", f64::exp), "exp(X) - e raised to the power X.");
    add(r, "log", |_, a, _| unary(a, "log", f64::ln), "log(X) - natural logarithm.");
    add(r, "log2", |_, a, _| unary(a, "log2", f64::log2), "log2(X) - base 2 logarithm.");
    add(r, "log10", |_, a, _| unary(a, "log10", f64::log10), "log10(X) - base 10 logarithm.");
    add(r, "sin", |_, a, _| unary(a, "sin", f64::sin), "sin(X) - sine of X in radians.");
    add(r, "cos", |_, a, _| unary(a, "cos", f64::cos), "cos(X) - cosine of X in radians.");
    add(r, "tan", |_, a, _| unary(a, "tan", f64::tan), "tan(X) - tangent of X in radians.");
    add(r, "asin", |_, a, _| unary(a, "asin", f64::asin), "asin(X) - inverse sine, in radians.");
    add(r, "acos", |_, a, _| unary(a, "acos", f64::acos), "acos(X) - inverse cosine, in radians.");
    add(r, "atan", |_, a, _| unary(a, "atan", f64::atan), "atan(X) - inverse tangent, in radians.");
    add(r, "sinh", |_, a, _| unary(a, "sinh", f64::sinh), "sinh(X) - hyperbolic sine.");
    add(r, "cosh", |_, a, _| unary(a, "cosh", f64::cosh), "cosh(X) - hyperbolic cosine.");
    add(r, "tanh", |_, a, _| unary(a, "tanh", f64::tanh), "tanh(X) - hyperbolic tangent.");
    add(r, "floor", |_, a, _| unary(a, "floor", f64::floor), "floor(X) - round towards minus infinity.");
    add(r, "ceil", |_, a, _| unary(a, "ceil", f64::ceil), "ceil(X) - round towards plus infinity.");
    add(r, "round", |_, a, _| unary(a, "round", f64::round), "round(X) - round to the nearest integer.");
    add(r, "fix", |_, a, _| unary(a, "fix", f64::trunc), "fix(X) - round towards zero.");
    add(r, "sign", |_, a, _| unary(a, "sign", sign_of), "sign(X) - -1, 0 or 1 by sign; NaN stays NaN.");

    // ---- predicates --------------------------------------------------
    add(r, "isnan", |_, a, _| unary(a, "isnan", |x| x.is_nan() as u8 as f64), "isnan(X) - true where X is NaN.");
    add(r, "isinf", |_, a, _| unary(a, "isinf", |x| x.is_infinite() as u8 as f64), "isinf(X) - true where X is infinite.");
    add(r, "isfinite", |_, a, _| unary(a, "isfinite", |x| x.is_finite() as u8 as f64), "isfinite(X) - true where X is finite.");

    // ---- two-argument math -------------------------------------------
    add(r, "mod", |_, a, _| binary(a, "mod", modulo), "mod(X,Y) - remainder after division, signed like Y.");
    add(r, "rem", |_, a, _| binary(a, "rem", remainder), "rem(X,Y) - remainder after division, signed like X.");
    add(r, "atan2", |_, a, _| binary(a, "atan2", f64::atan2), "atan2(Y,X) - four-quadrant inverse tangent.");
    add(r, "hypot", |_, a, _| binary(a, "hypot", f64::hypot), "hypot(X,Y) - sqrt(X^2 + Y^2) without overflow.");
    add(r, "power", |_, a, _| binary(a, "power", f64::powf), "power(X,Y) - element-wise X raised to the power Y.");
}

/// MATLAB's `mod`: the result takes the sign of the divisor, and a zero
/// divisor returns the dividend unchanged.
fn modulo(x: f64, y: f64) -> f64 {
    if y == 0.0 { x } else { x - (x / y).floor() * y }
}

/// MATLAB's `rem`: the result takes the sign of the dividend.
fn remainder(x: f64, y: f64) -> f64 {
    x - (x / y).trunc() * y
}

/// `sign`, with MATLAB's NaN: the old code let NaN fall through to the zero
/// branch and reported `0`.
fn sign_of(x: f64) -> f64 {
    if x.is_nan() {
        f64::NAN
    } else if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Every one-argument element-wise function.
fn unary(args: &[Value], name: &str, f: fn(f64) -> f64) -> R<Vec<Value>> {
    at_most(args, 1, name)?;
    one_mat(mat(args, 0, name)?.map(f))
}

/// Every two-argument element-wise function, with broadcasting.
fn binary(args: &[Value], name: &str, f: fn(f64, f64) -> f64) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    need(args, 2, name)?;
    let a = mat(args, 0, name)?;
    let b = mat(args, 1, name)?;
    one_mat(a.zip(&b, name, f)?)
}

// ---- reductions ------------------------------------------------------

/// Which of the five reductions that share one implementation is running.
#[derive(Clone, Copy)]
enum Red {
    Sum,
    Prod,
    Mean,
    Any,
    All,
}

fn reduction(args: &[Value], name: &str, kind: Red) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    let m = mat(args, 0, name)?;
    let d = if args.len() >= 2 {
        Some(dim(args, 1, name)?)
    } else {
        None
    };
    let f: fn(&[f64]) -> f64 = match kind {
        Red::Sum => |xs| xs.iter().sum(),
        Red::Prod => |xs| xs.iter().product(),
        Red::Mean => |xs| xs.iter().sum::<f64>() / xs.len() as f64,
        Red::Any => |xs| xs.iter().any(|v| *v != 0.0) as u8 as f64,
        Red::All => |xs| xs.iter().all(|v| *v != 0.0) as u8 as f64,
    };
    one_mat(reduce(&m, d, f))
}

fn extremum(args: &[Value], name: &str, is_max: bool) -> R<Vec<Value>> {
    at_most(args, 3, name)?;
    let m = mat(args, 0, name)?;
    if args.len() == 2 {
        // The two-array form: element-wise, ignoring NaN.
        let b = mat(args, 1, name)?;
        let f = move |x: f64, y: f64| {
            if x.is_nan() {
                y
            } else if y.is_nan() {
                x
            } else if is_max {
                x.max(y)
            } else {
                x.min(y)
            }
        };
        return one_mat(m.zip(&b, name, f)?);
    }
    if m.is_empty() {
        return one_mat(Matrix::empty());
    }
    let d = if args.len() >= 3 {
        Some(dim(args, 2, name)?)
    } else {
        None
    };
    let f = move |xs: &[f64]| {
        xs.iter()
            .copied()
            .filter(|v| !v.is_nan())
            .fold(f64::NAN, |acc, v| {
                if acc.is_nan() {
                    v
                } else if is_max {
                    acc.max(v)
                } else {
                    acc.min(v)
                }
            })
    };
    one_mat(reduce(&m, d, f))
}

/// Reduce along a dimension. Without one, MATLAB picks the first dimension
/// that is not a singleton, which for our two-dimensional arrays means rows
/// unless the input is a row vector.
///
/// A dimension beyond the array's is a singleton, so the answer is the input
/// unchanged; `0` never reaches here, because `args::dim` rejects it.
pub fn reduce(m: &Matrix, dim: Option<usize>, f: impl Fn(&[f64]) -> f64) -> Matrix {
    let d = match dim {
        Some(d) => d,
        None => {
            // Only the no-dimension form of a 0x0 collapses to the identity:
            // sum([]) is 0, but sum([], 1) keeps MATLAB's 1x0 empty.
            if m.rows == 0 && m.cols == 0 {
                return Matrix::scalar(f(&[]));
            }
            if m.rows == 1 { 2 } else { 1 }
        }
    };
    if d >= 3 {
        return m.clone();
    }
    if d == 1 {
        Matrix::row(
            (0..m.cols)
                .map(|c| f(&m.data[c * m.rows..(c + 1) * m.rows]))
                .collect(),
        )
    } else {
        Matrix::col(
            (0..m.rows)
                .map(|r| {
                    let xs: Vec<f64> = (0..m.cols).map(|c| m.get(r, c)).collect();
                    f(&xs)
                })
                .collect(),
        )
    }
}

fn cumulative(args: &[Value], name: &str, is_sum: bool) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    let m = mat(args, 0, name)?;
    let d = if args.len() >= 2 {
        Some(dim(args, 1, name)?)
    } else {
        None
    };
    one_mat(scan(&m, d, is_sum))
}

/// Running sum or product along a dimension. The dimension argument used to
/// be read and then ignored, which always gave the down-the-columns answer.
pub fn scan(m: &Matrix, dim: Option<usize>, is_sum: bool) -> Matrix {
    let d = dim.unwrap_or(if m.rows == 1 { 2 } else { 1 });
    if d >= 3 {
        return m.clone();
    }
    let step = |acc: f64, v: f64| if is_sum { acc + v } else { acc * v };
    let seed = if is_sum { 0.0 } else { 1.0 };
    let mut out = m.clone();
    if d == 1 {
        for c in 0..m.cols {
            let mut acc = seed;
            for r in 0..m.rows {
                acc = step(acc, m.get(r, c));
                out.set(r, c, acc);
            }
        }
    } else {
        for r in 0..m.rows {
            let mut acc = seed;
            for c in 0..m.cols {
                acc = step(acc, m.get(r, c));
                out.set(r, c, acc);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a matrix from elements given in reading (row-major) order.
    fn rmat(rows: usize, cols: usize, row_major: &[f64]) -> Matrix {
        assert_eq!(rows * cols, row_major.len());
        let mut m = Matrix::filled(rows, cols, 0.0);
        for (i, v) in row_major.iter().enumerate() {
            m.set(i / cols, i % cols, *v);
        }
        m
    }

    fn sum_of(m: &Matrix, d: Option<usize>) -> Matrix {
        reduce(m, d, |xs| xs.iter().sum())
    }

    #[test]
    fn reduce_picks_the_first_non_singleton_dimension() {
        let a = rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(sum_of(&a, None), Matrix::row(vec![4.0, 6.0]));
        assert_eq!(sum_of(&a, Some(1)), Matrix::row(vec![4.0, 6.0]));
        assert_eq!(sum_of(&a, Some(2)), Matrix::col(vec![3.0, 7.0]));
        // A row vector reduces along itself.
        let r = Matrix::row(vec![1.0, 2.0, 3.0]);
        assert_eq!(sum_of(&r, None), Matrix::scalar(6.0));
    }

    #[test]
    fn a_dimension_past_the_array_is_a_singleton() {
        let a = rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(sum_of(&a, Some(3)), a);
        assert_eq!(sum_of(&a, Some(9)), a);
    }

    #[test]
    fn an_empty_with_a_dimension_keeps_matlabs_empty_shape() {
        let e = Matrix::empty();
        // The no-dimension form still collapses to the identity element.
        assert_eq!(sum_of(&e, None), Matrix::scalar(0.0));
        // With a dimension it stays empty, 1x0 along 1 and 0x1 along 2.
        let one = sum_of(&e, Some(1));
        assert_eq!((one.rows, one.cols), (1, 0));
        let two = sum_of(&e, Some(2));
        assert_eq!((two.rows, two.cols), (0, 1));
        // Non-square empties are unchanged by this cycle.
        let z = Matrix::filled(0, 3, 0.0);
        assert_eq!(sum_of(&z, None), Matrix::row(vec![0.0, 0.0, 0.0]));
    }

    #[test]
    fn scan_honours_its_dimension_argument() {
        let a = rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        // Down the columns by default for a matrix.
        assert_eq!(scan(&a, None, true), rmat(2, 2, &[1.0, 2.0, 4.0, 6.0]));
        assert_eq!(scan(&a, Some(1), true), rmat(2, 2, &[1.0, 2.0, 4.0, 6.0]));
        assert_eq!(scan(&a, Some(2), true), rmat(2, 2, &[1.0, 3.0, 3.0, 7.0]));
        assert_eq!(scan(&a, Some(2), false), rmat(2, 2, &[1.0, 2.0, 3.0, 12.0]));
        // A row vector accumulates along itself, with or without the 2.
        let r = Matrix::row(vec![1.0, 2.0, 3.0]);
        assert_eq!(scan(&r, None, true), Matrix::row(vec![1.0, 3.0, 6.0]));
        assert_eq!(scan(&r, Some(1), true), r);
        assert_eq!(scan(&r, Some(3), true), r);
    }

    #[test]
    fn sign_keeps_nan() {
        assert!(sign_of(f64::NAN).is_nan());
        assert_eq!(sign_of(-3.0), -1.0);
        assert_eq!(sign_of(0.0), 0.0);
        assert_eq!(sign_of(-0.0), 0.0);
        assert_eq!(sign_of(5.0), 1.0);
        assert_eq!(sign_of(f64::INFINITY), 1.0);
        assert_eq!(sign_of(f64::NEG_INFINITY), -1.0);
    }

    #[test]
    fn arity_is_checked_at_both_ends() {
        let one = [Value::Mat(Matrix::scalar(1.0))];
        let two = [one[0].clone(), one[0].clone()];
        let three = [one[0].clone(), one[0].clone(), one[0].clone()];
        assert_eq!(
            unary(&two, "abs", f64::abs).unwrap_err(),
            "Too many input arguments."
        );
        assert_eq!(
            reduction(&three, "sum", Red::Sum).unwrap_err(),
            "Too many input arguments."
        );
        assert!(binary(&one, "mod", f64::atan2).is_err());
        assert!(binary(&two, "mod", f64::atan2).is_ok());
        assert!(extremum(&three, "max", true).is_ok());
    }

    #[test]
    fn a_reduction_rejects_dimension_zero() {
        let args = [
            Value::Mat(Matrix::row(vec![1.0, 2.0])),
            Value::Mat(Matrix::scalar(0.0)),
        ];
        let e = reduction(&args, "sum", Red::Sum).unwrap_err();
        assert!(e.contains("positive integer"), "{e}");
        let e = cumulative(&args, "cumsum", true).unwrap_err();
        assert!(e.contains("positive integer"), "{e}");
    }
}
