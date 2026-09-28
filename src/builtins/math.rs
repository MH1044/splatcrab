//! Reductions, element-wise math and the two-argument numeric functions.

use super::args::{Along, at_most, check_shape, dim, dim_or_all, mat, need, option};
use super::{Registry, add, one_as, one_mat};
use crate::error;
use crate::interp::R;
use crate::value::{Class, Matrix, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    // ---- reductions --------------------------------------------------
    add(r, "sum", |_, a, _| reduction(a, "sum", Red::Sum), "sum(A), sum(A,dim), sum(A,'all') - sum of the elements.");
    add(r, "prod", |_, a, _| reduction(a, "prod", Red::Prod), "prod(A), prod(A,dim), prod(A,'all') - product of the elements.");
    add(r, "mean", |_, a, _| reduction(a, "mean", Red::Mean), "mean(A), mean(A,dim), mean(A,'all') - average of the elements.");
    add(r, "any", |_, a, _| reduction(a, "any", Red::Any), "any(A), any(A,dim), any(A,'all') - true if any element is non-zero, ignoring NaN.");
    add(r, "all", |_, a, _| reduction(a, "all", Red::All), "all(A), all(A,dim), all(A,'all') - true if every element is non-zero.");
    add(r, "max", |_, a, n| extremum(a, "max", true, n), "max(A), max(A,B), max(A,[],dim), max(A,[],'all'), [M,I] = max(...) - largest elements.");
    add(r, "min", |_, a, n| extremum(a, "min", false, n), "min(A), min(A,B), min(A,[],dim), min(A,[],'all'), [M,I] = min(...) - smallest elements.");
    add(r, "cumsum", |_, a, _| cumulative(a, "cumsum", true), "cumsum(A), cumsum(A,dim) - cumulative sum.");
    add(r, "cumprod", |_, a, _| cumulative(a, "cumprod", false), "cumprod(A), cumprod(A,dim) - cumulative product.");

    // ---- element-wise math -------------------------------------------
    add(r, "abs", |_, a, _| unary(a, "abs", f64::abs), "abs(X) - absolute value.");
    add(r, "sqrt", |_, a, _| real_unary(a, "sqrt", f64::sqrt, Real::NonNegative), "sqrt(X) - square root.");
    add(r, "exp", |_, a, _| unary(a, "exp", f64::exp), "exp(X) - e raised to the power X.");
    add(r, "log", |_, a, _| real_unary(a, "log", f64::ln, Real::NonNegative), "log(X) - natural logarithm.");
    add(r, "log2", |_, a, _| real_unary(a, "log2", f64::log2, Real::NonNegative), "log2(X) - base 2 logarithm.");
    add(r, "log10", |_, a, _| real_unary(a, "log10", f64::log10, Real::NonNegative), "log10(X) - base 10 logarithm.");
    add(r, "sin", |_, a, _| unary(a, "sin", f64::sin), "sin(X) - sine of X in radians.");
    add(r, "cos", |_, a, _| unary(a, "cos", f64::cos), "cos(X) - cosine of X in radians.");
    add(r, "tan", |_, a, _| unary(a, "tan", f64::tan), "tan(X) - tangent of X in radians.");
    add(r, "asin", |_, a, _| real_unary(a, "asin", f64::asin, Real::UnitInterval), "asin(X) - inverse sine, in radians.");
    add(r, "acos", |_, a, _| real_unary(a, "acos", f64::acos, Real::UnitInterval), "acos(X) - inverse cosine, in radians.");
    add(r, "atan", |_, a, _| unary(a, "atan", f64::atan), "atan(X) - inverse tangent, in radians.");
    add(r, "sinh", |_, a, _| unary(a, "sinh", f64::sinh), "sinh(X) - hyperbolic sine.");
    add(r, "cosh", |_, a, _| unary(a, "cosh", f64::cosh), "cosh(X) - hyperbolic cosine.");
    add(r, "tanh", |_, a, _| unary(a, "tanh", f64::tanh), "tanh(X) - hyperbolic tangent.");
    add(r, "floor", |_, a, _| unary(a, "floor", f64::floor), "floor(X) - round towards minus infinity.");
    add(r, "ceil", |_, a, _| unary(a, "ceil", f64::ceil), "ceil(X) - round towards plus infinity.");
    add(r, "round", |_, a, _| round(a), "round(X), round(X,n), round(X,n,'significant') - round, ties away from zero.");
    add(r, "fix", |_, a, _| unary(a, "fix", f64::trunc), "fix(X) - round towards zero.");
    add(r, "sign", |_, a, _| unary(a, "sign", sign_of), "sign(X) - -1, 0 or 1 by sign; NaN stays NaN.");

    // ---- predicates --------------------------------------------------
    add(r, "isnan", |_, a, _| mask(a, "isnan", f64::is_nan), "isnan(X) - true where X is NaN.");
    add(r, "isinf", |_, a, _| mask(a, "isinf", f64::is_infinite), "isinf(X) - true where X is infinite.");
    add(r, "isfinite", |_, a, _| mask(a, "isfinite", f64::is_finite), "isfinite(X) - true where X is finite.");

    // ---- two-argument math -------------------------------------------
    add(r, "mod", |_, a, _| binary(a, "mod", modulo), "mod(X,Y) - remainder after division, signed like Y.");
    add(r, "rem", |_, a, _| binary(a, "rem", remainder), "rem(X,Y) - remainder after division, signed like X.");
    add(r, "atan2", |_, a, _| binary(a, "atan2", f64::atan2), "atan2(Y,X) - four-quadrant inverse tangent.");
    add(r, "hypot", |_, a, _| binary(a, "hypot", f64::hypot), "hypot(X,Y) - sqrt(X^2 + Y^2) without overflow.");
    add(r, "power", |_, a, _| real_binary(a, "power", powf_real), "power(X,Y) - element-wise X raised to the power Y.");
}

/// Where a real function stops being real. Until cycle 10 brings complex
/// numbers, an argument outside the domain is a clean error rather than the
/// `NaN` these used to hand back: `sqrt(-4)` is `2i` in MATLAB, and a `NaN`
/// that looks like a computed answer is worse than a refusal.
#[derive(Clone, Copy)]
enum Real {
    /// `sqrt`, `log`, `log2`, `log10`: a negative argument is complex.
    NonNegative,
    /// `asin`, `acos`: an argument outside [-1, 1] is complex.
    UnitInterval,
}

impl Real {
    /// `None` when `x` is in the real domain, and the error otherwise. `NaN`
    /// is in the domain of both: `sqrt(NaN)` is `NaN` in MATLAB too.
    fn check(self, name: &str, x: f64) -> Option<crate::error::MError> {
        match self {
            Real::NonNegative if x < 0.0 => Some(error::complex_negative(name)),
            Real::UnitInterval if x.abs() > 1.0 => Some(error::complex_outside_unit(name)),
            _ => None,
        }
    }
}

/// `x^y` where the result would be complex: a negative base raised to a
/// power that is neither an integer nor infinite. `(-8)^(1/3)` is complex in
/// MATLAB, while `(-2)^Inf` is a real `Inf` and `(-2)^3` a real `-8`.
///
/// `interp.rs` uses this for both `^` and `.^`, so the three spellings
/// `(-8)^(1/3)`, `(-8).^(1/3)` and `power(-8, 1/3)` agree.
pub fn powf_real(x: f64, y: f64) -> R<f64> {
    if x < 0.0 && y.is_finite() && y.fract() != 0.0 {
        return Err(error::complex_power());
    }
    Ok(x.powf(y))
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

/// An element-wise test, answered with a logical array the shape of its
/// argument. `isnan` returns a mask, which is what lets cycle 03's
/// `x(isnan(x))` select rather than read ones and zeros as positions.
fn mask(args: &[Value], name: &str, test: fn(f64) -> bool) -> R<Vec<Value>> {
    at_most(args, 1, name)?;
    let m = mat(args, 0, name)?.map(|x| test(x) as u8 as f64);
    one_as(m.with_class(Class::Logical))
}

/// [`unary`] for a function with a real domain smaller than the line. The
/// whole argument is judged before any of it is mapped, so one complex
/// element refuses the call rather than seeding the result with a `NaN`.
fn real_unary(args: &[Value], name: &str, f: fn(f64) -> f64, domain: Real) -> R<Vec<Value>> {
    at_most(args, 1, name)?;
    let m = mat(args, 0, name)?;
    for x in &m.data {
        if let Some(e) = domain.check(name, *x) {
            return Err(e);
        }
    }
    one_mat(m.map(f))
}

/// Every two-argument element-wise function, with broadcasting.
fn binary(args: &[Value], name: &str, f: fn(f64, f64) -> f64) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    need(args, 2, name)?;
    let a = mat(args, 0, name)?;
    let b = mat(args, 1, name)?;
    one_mat(a.zip(&b, name, f)?)
}

/// [`binary`] for a function that may refuse a pair of elements, which is
/// `power` and its complex results.
fn real_binary(args: &[Value], name: &str, f: fn(f64, f64) -> R<f64>) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    need(args, 2, name)?;
    let a = mat(args, 0, name)?;
    let b = mat(args, 1, name)?;
    one_mat(a.try_zip(&b, name, f)?)
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

/// The sum of `xs`, starting from `+0`. Rust's `f64` `Sum` starts from `-0`,
/// which made `fprintf('%.4f', sum([]))` print `-0.0000`; MATLAB gives `0`.
pub fn sum0(xs: &[f64]) -> f64 {
    xs.iter().fold(0.0, |acc, v| acc + v)
}

fn reduction(args: &[Value], name: &str, kind: Red) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    let m = mat(args, 0, name)?;
    let along = if args.len() >= 2 {
        Some(dim_or_all(args, 1, name)?)
    } else {
        None
    };
    let f: fn(&[f64]) -> f64 = match kind {
        Red::Sum => sum0,
        Red::Prod => |xs| xs.iter().product(),
        Red::Mean => |xs| sum0(xs) / xs.len() as f64,
        // "any ignores elements of A that are NaN" (the MATLAB page).
        Red::Any => |xs| xs.iter().any(|v| *v != 0.0 && !v.is_nan()) as u8 as f64,
        Red::All => |xs| xs.iter().all(|v| *v != 0.0) as u8 as f64,
    };
    let out = match along {
        None => reduce(&m, None, f)?,
        Some(Along::Dim(d)) => reduce(&m, Some(d), f)?,
        Some(Along::All) => reduce_all(&m, f),
    };
    // `any` and `all` answer with a logical, as MATLAB's do. A dimension
    // past the array's hands back the argument itself, which is converted
    // then: `any([2 0], 3)` is the logical `1 0`.
    match kind {
        Red::Any | Red::All => one_as(out.to_class(Class::Logical)?),
        _ => one_mat(out),
    }
}

/// The reduction over `A(:)`, which is what `'all'` means: one 1x1 answer
/// from every element at once. An empty `A` gives the identity element, so
/// `sum([], 'all')` is `0` and `prod([], 'all')` is `1`.
fn reduce_all(m: &Matrix, f: impl Fn(&[f64]) -> f64) -> Matrix {
    Matrix::scalar(f(&m.data))
}

/// `max` and `min`. Of a logical they stay logical, as MATLAB's do: the
/// largest of some trues and falses is itself a true or a false. Any other
/// argument, a char included, gives a double.
///
/// Asked for two outputs, the second is the index of each extremum along
/// the dimension reduced, a double: the first occurrence when there is a
/// tie, and `1` for a slice of `NaN` alone, whose extremum is that first
/// `NaN`. With `'all'` it is a linear index. The two-array form `max(A, B)`
/// has no index, so it produces one value and asking for two is "Too many
/// output arguments.".
fn extremum(args: &[Value], name: &str, is_max: bool, nargout: usize) -> R<Vec<Value>> {
    let out = extremum_value(args, name, is_max)?;
    let logical = |i: usize| {
        args.get(i)
            .is_some_and(|v| matches!(v.mat(), Ok(m) if m.class == Class::Logical))
    };
    let keep = if args.len() == 2 {
        logical(0) && logical(1)
    } else {
        logical(0)
    };
    let class = if keep { Class::Logical } else { Class::Double };
    let value = out
        .into_iter()
        .next()
        .unwrap()
        .into_mat()?
        .with_class(class);
    if nargout < 2 || args.len() == 2 {
        return one_as(value);
    }
    let index = extremum_index(args, name, is_max)?;
    Ok(vec![Value::Mat(value), Value::Mat(index)])
}

/// Where `max` or `min` reduces a one- or three-argument call.
fn extremum_along_arg(args: &[Value], m: &Matrix, name: &str) -> R<Along> {
    Ok(if args.len() >= 3 {
        dim_or_all(args, 2, name)?
    } else {
        // The first non-singleton dimension, which for a 0x0 is the first.
        Along::Dim(if m.rows == 1 { 2 } else { 1 })
    })
}

/// The one-based position of the extremum of `xs`, ignoring `NaN`: the first
/// of equal values, and `1` when every element is `NaN`.
pub fn arg_extremum(xs: &[f64], is_max: bool) -> usize {
    let mut best: Option<(usize, f64)> = None;
    for (k, &v) in xs.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        let better = match best {
            None => true,
            Some((_, b)) => (is_max && v > b) || (!is_max && v < b),
        };
        if better {
            best = Some((k, v));
        }
    }
    best.map_or(1, |(k, _)| k + 1)
}

/// The second output of `max` and `min`, shaped as the first is.
fn extremum_index(args: &[Value], name: &str, is_max: bool) -> R<Matrix> {
    let m = mat(args, 0, name)?;
    let f = move |xs: &[f64]| arg_extremum(xs, is_max) as f64;
    Ok(match extremum_along_arg(args, &m, name)? {
        Along::All if m.is_empty() => Matrix::empty(),
        Along::All => Matrix::scalar(f(&m.data)),
        Along::Dim(d) => {
            let len = match d {
                1 => m.rows,
                2 => m.cols,
                _ => 1,
            };
            if len == 0 {
                // The empty the first output is, with no elements to index.
                Matrix::new(m.rows, m.cols, Vec::new())
            } else if d >= 3 {
                // Every slice is one element, the first of its slice.
                Matrix::filled(m.rows, m.cols, 1.0)
            } else {
                reduce(&m, Some(d), f)?
            }
        }
    })
}

fn extremum_value(args: &[Value], name: &str, is_max: bool) -> R<Vec<Value>> {
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
    let along = extremum_along_arg(args, &m, name)?;
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
    match along {
        // No identity element, so an empty stays empty: `max([], [], 'all')`
        // is the 0x0 `[]`.
        Along::All if m.is_empty() => one_mat(Matrix::empty()),
        Along::All => one_mat(reduce_all(&m, f)),
        Along::Dim(d) => one_mat(extremum_along(&m, d, f)?),
    }
}

/// `max` or `min` along `d`, with MATLAB's rule for an empty: "If
/// size(A,dim) is 0, then max(A,dim) returns an empty array with the same
/// size as A." Otherwise the reduced dimension becomes 1, even when the other
/// one is 0, so `max(zeros(3, 0))` is 1x0 and `max(zeros(0, 3), [], 2)` is
/// 0x1. `max` has no identity element, which is why this is not `sum`'s rule.
fn extremum_along(m: &Matrix, d: usize, f: impl Fn(&[f64]) -> f64) -> R<Matrix> {
    let len = match d {
        1 => m.rows,
        2 => m.cols,
        _ => 1,
    };
    if len == 0 {
        return Ok(m.clone());
    }
    reduce(m, Some(d), f)
}

// ---- round -----------------------------------------------------------

/// `round(X)`, `round(X, n)`, `round(X, n, 'decimals')` and
/// `round(X, n, 'significant')`, element-wise, ties away from zero.
fn round(args: &[Value]) -> R<Vec<Value>> {
    at_most(args, 3, "round")?;
    let m = mat(args, 0, "round")?;
    if args.len() == 1 {
        return one_mat(m.map(f64::round));
    }
    let significant = match args.get(2) {
        None => false,
        Some(_) => match option(args, 2) {
            Some(t) if t.eq_ignore_ascii_case("decimals") => false,
            Some(t) if t.eq_ignore_ascii_case("significant") => true,
            _ => return Err(error::round_type()),
        },
    };
    let n = match (option(args, 1), mat(args, 1, "round")?.scalar_value()) {
        (None, Some(n)) if n.fract() == 0.0 => n,
        _ if significant => return Err(error::round_significant()),
        _ => return Err(error::round_digits()),
    };
    if significant {
        if n < 1.0 {
            return Err(error::round_significant());
        }
        return one_mat(m.map(|x| round_significant(x, n)));
    }
    one_mat(m.map(|x| round_decimals(x, n)))
}

/// `10^k` correctly rounded, `Inf` past the top of the range and `0` past the
/// bottom. The decimal parser is exact where `powi` accumulates error.
fn pow10(k: f64) -> f64 {
    format!("1e{}", k.clamp(-400.0, 400.0))
        .parse()
        .unwrap_or(f64::NAN)
}

/// `x` rounded to the nearest multiple of `10^-n`, for an integer `n`.
///
/// Two guards keep the scaling honest. For `n > 0`, when `10^n` or `x * 10^n`
/// is not finite, or `x * 10^n` is at least 2^52 and so already an integer,
/// `x` is its own answer: `round(pi, 20)` is `pi` and `round(1e307, 2)` is
/// `1e307`. For `n < 0`, a `10^-n` past the range makes every finite `x` a
/// tiny fraction of the step, so the answer is `0` (`round(5, -400)`), not
/// the `NaN` that `0 * Inf` would give; and an `x / 10^-n` of at least 2^52
/// is already a multiple of the step, so `x` is its own answer there too.
pub fn round_decimals(x: f64, n: f64) -> f64 {
    const INTEGRAL: f64 = 4_503_599_627_370_496.0; // 2^52
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    if n == 0.0 {
        return x.round();
    }
    if n > 0.0 {
        let p = pow10(n);
        let y = x * p;
        if !p.is_finite() || !y.is_finite() || y.abs() >= INTEGRAL {
            return x;
        }
        y.round() / p
    } else {
        let p = pow10(-n);
        if !p.is_finite() {
            return 0.0;
        }
        let y = x / p;
        if y.abs() >= INTEGRAL {
            return x;
        }
        y.round() * p
    }
}

/// `x` rounded to `n` significant digits, `n >= 1`: decimals
/// `n - floor(log10(abs(x))) - 1`. `0`, `Inf` and `NaN` pass through.
pub fn round_significant(x: f64, n: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    round_decimals(x, n - x.abs().log10().floor() - 1.0)
}

/// Reduce along a dimension. Without one, MATLAB picks the first dimension
/// that is not a singleton, which for our two-dimensional arrays means rows
/// unless the input is a row vector.
///
/// A dimension beyond the array's is a singleton, so the answer is the input
/// unchanged; `0` never reaches here, because `args::dim` rejects it.
/// The result shape comes from the operand, and one dimension of an operand
/// can be enormous while the operand itself is empty: `sum(zeros(0, 1e15))`
/// is a 1x1e15 row built from no elements at all, and used to abort in the
/// allocator. Both shapes therefore go through `args::check_shape` first.
pub fn reduce(m: &Matrix, dim: Option<usize>, f: impl Fn(&[f64]) -> f64) -> R<Matrix> {
    let d = match dim {
        Some(d) => d,
        None => {
            // Only the no-dimension form of a 0x0 collapses to the identity:
            // sum([]) is 0, but sum([], 1) keeps MATLAB's 1x0 empty.
            if m.rows == 0 && m.cols == 0 {
                return Ok(Matrix::scalar(f(&[])));
            }
            if m.rows == 1 { 2 } else { 1 }
        }
    };
    if d >= 3 {
        return Ok(m.clone());
    }
    if d == 1 {
        check_shape(1.0, m.cols as f64)?;
        Ok(Matrix::row(
            (0..m.cols)
                .map(|c| f(&m.data[c * m.rows..(c + 1) * m.rows]))
                .collect(),
        ))
    } else {
        check_shape(m.rows as f64, 1.0)?;
        Ok(Matrix::col(
            (0..m.rows)
                .map(|r| {
                    let xs: Vec<f64> = (0..m.cols).map(|c| m.get(r, c)).collect();
                    f(&xs)
                })
                .collect(),
        ))
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
        reduce(m, d, |xs| xs.iter().sum()).expect("shape fits")
    }

    /// Acceptance test 17, the `reduce` half. The operand is empty and one of
    /// its dimensions is enormous, so the result shape is the only thing that
    /// can be judged; the allocation used to happen anyway.
    #[test]
    fn reduce_checks_the_result_size_before_allocating() {
        let wide = Matrix::new(0, 1_000_000_000_000_000, Vec::new());
        let e = reduce(&wide, Some(1), sum0).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 1x1000000000000000 array exceeds the maximum array size."
        );
        let tall = Matrix::new(1_000_000_000_000_000, 0, Vec::new());
        let e = reduce(&tall, Some(2), sum0).unwrap_err().msg;
        assert!(e.contains("1000000000000000x1"), "{e}");
        // Through the builtin, which is how a script reaches it.
        assert!(
            call(
                &[val(wide.clone()), val(Matrix::scalar(1.0))],
                "sum",
                Red::Sum
            )
            .is_err()
        );
        assert!(call(&[val(tall), val(Matrix::scalar(2.0))], "mean", Red::Mean).is_err());
        // A dimension past the array's is a singleton and allocates nothing
        // new, so it is still allowed.
        assert!(reduce(&wide, Some(3), sum0).is_ok());
        // And an ordinary reduction is untouched.
        assert_eq!(
            reduce(&rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]), Some(1), sum0).unwrap(),
            Matrix::row(vec![4.0, 6.0])
        );
    }

    /// Acceptance test 14 at the level of the helpers: the domain test runs
    /// over the whole argument before any of it is mapped.
    #[test]
    fn a_real_function_refuses_a_complex_result() {
        let neg = |name: &str, f: fn(f64) -> f64, x: f64| {
            real_unary(&[val(Matrix::scalar(x))], name, f, Real::NonNegative)
        };
        for (name, f) in [
            ("sqrt", f64::sqrt as fn(f64) -> f64),
            ("log", f64::ln),
            ("log2", f64::log2),
            ("log10", f64::log10),
        ] {
            let e = neg(name, f, -1.0).unwrap_err().msg;
            assert_eq!(
                e,
                format!(
                    "Complex results are not supported. '{name}' of a negative number is complex."
                )
            );
            // On the domain, and at its edge, the value comes through.
            assert!(neg(name, f, 1.0).is_ok());
            assert!(neg(name, f, 0.0).is_ok());
            assert!(neg(name, f, -0.0).is_ok());
            assert!(neg(name, f, f64::NAN).is_ok());
            assert!(neg(name, f, f64::INFINITY).is_ok());
            assert!(neg(name, f, f64::NEG_INFINITY).is_err());
        }
        let unit = |name: &str, f: fn(f64) -> f64, x: f64| {
            real_unary(&[val(Matrix::scalar(x))], name, f, Real::UnitInterval)
        };
        for (name, f) in [("asin", f64::asin as fn(f64) -> f64), ("acos", f64::acos)] {
            let e = unit(name, f, 2.0).unwrap_err().msg;
            assert_eq!(
                e,
                format!(
                    "Complex results are not supported. \
                     '{name}' of a value outside [-1, 1] is complex."
                )
            );
            assert!(unit(name, f, -2.0).is_err());
            assert!(unit(name, f, f64::INFINITY).is_err());
            assert!(unit(name, f, 1.0).is_ok());
            assert!(unit(name, f, -1.0).is_ok());
            assert!(unit(name, f, f64::NAN).is_ok());
        }
        // One bad element in a matrix refuses the whole call.
        let m = val(Matrix::row(vec![1.0, -4.0, 9.0]));
        assert!(real_unary(&[m], "sqrt", f64::sqrt, Real::NonNegative).is_err());
    }

    #[test]
    fn a_negative_base_with_a_fractional_exponent_is_complex() {
        let e = powf_real(-8.0, 1.0 / 3.0).unwrap_err().msg;
        assert_eq!(
            e,
            "Complex results are not supported. \
             A negative number raised to a fractional power is complex."
        );
        assert!(powf_real(-2.0, 0.5).is_err());
        // An integer exponent is real, whatever its sign.
        assert_eq!(powf_real(-8.0, 2.0).unwrap(), 64.0);
        assert_eq!(powf_real(-2.0, -2.0).unwrap(), 0.25);
        assert_eq!(powf_real(-8.0, 0.0).unwrap(), 1.0);
        // So is an infinite one, which IEEE answers without going complex.
        assert_eq!(powf_real(-2.0, f64::INFINITY).unwrap(), f64::INFINITY);
        assert_eq!(powf_real(-2.0, f64::NEG_INFINITY).unwrap(), 0.0);
        // A non-negative base takes any exponent, and so does a NaN one.
        assert_eq!(powf_real(8.0, 1.0 / 3.0).unwrap(), 8.0f64.powf(1.0 / 3.0));
        assert_eq!(powf_real(-0.0, 0.5).unwrap(), 0.0);
        assert!(powf_real(f64::NAN, 0.5).unwrap().is_nan());
        assert!(powf_real(-2.0, f64::NAN).unwrap().is_nan());
        // The `power` builtin broadcasts through the same test.
        let a = val(Matrix::row(vec![-8.0, 8.0]));
        let b = val(Matrix::scalar(1.0 / 3.0));
        assert!(real_binary(&[a, b], "power", powf_real).is_err());
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
            unary(&two, "abs", f64::abs).unwrap_err().msg,
            "Too many input arguments."
        );
        assert_eq!(
            reduction(&three, "sum", Red::Sum).unwrap_err().msg,
            "Too many input arguments."
        );
        let four = [
            one[0].clone(),
            one[0].clone(),
            one[0].clone(),
            one[0].clone(),
        ];
        assert_eq!(round(&four).unwrap_err().msg, "Too many input arguments.");
        assert!(binary(&one, "mod", f64::atan2).is_err());
        assert!(binary(&two, "mod", f64::atan2).is_ok());
        assert!(extremum(&three, "max", true, 1).is_ok());
    }

    fn call(args: &[Value], name: &str, kind: Red) -> R<Matrix> {
        Ok(reduction(args, name, kind)?[0].clone().into_mat().unwrap())
    }

    fn val(m: Matrix) -> Value {
        Value::Mat(m)
    }

    fn text(s: &str) -> Value {
        Value::str(s)
    }

    #[test]
    fn an_empty_sum_is_positive_zero() {
        assert!(sum0(&[]).is_sign_positive());
        assert_eq!(sum0(&[1.0, 2.0]), 3.0);
        let s = call(&[val(Matrix::empty())], "sum", Red::Sum).unwrap();
        assert_eq!(s.data, [0.0]);
        assert!(s.data[0].is_sign_positive(), "sum([]) is -0");
        let z = call(&[val(Matrix::filled(0, 3, 0.0))], "sum", Red::Sum).unwrap();
        assert!(z.data.iter().all(|v| v.is_sign_positive()));
        assert!(
            call(&[val(Matrix::empty())], "mean", Red::Mean)
                .unwrap()
                .data[0]
                .is_nan()
        );
    }

    #[test]
    fn any_ignores_nan_and_all_does_not() {
        let any = |v: &[f64]| call(&[val(Matrix::row(v.to_vec()))], "any", Red::Any).unwrap();
        let all = |v: &[f64]| call(&[val(Matrix::row(v.to_vec()))], "all", Red::All).unwrap();
        assert_eq!(any(&[f64::NAN]).data, [0.0]);
        assert_eq!(any(&[f64::NAN, 0.0]).data, [0.0]);
        assert_eq!(any(&[f64::NAN, 1.0]).data, [1.0]);
        assert_eq!(all(&[f64::NAN]).data, [1.0]);
        let m = rmat(2, 2, &[f64::NAN, 0.0, 0.0, 2.0]);
        let by_row = call(&[val(m), val(Matrix::scalar(2.0))], "any", Red::Any).unwrap();
        assert_eq!(
            by_row,
            Matrix::col(vec![0.0, 1.0]).with_class(Class::Logical)
        );
    }

    #[test]
    fn all_reduces_over_every_element() {
        let a = val(rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]));
        let all = text("all");
        assert_eq!(
            call(&[a.clone(), all.clone()], "sum", Red::Sum)
                .unwrap()
                .data,
            [10.0]
        );
        assert_eq!(
            call(&[a.clone(), all.clone()], "prod", Red::Prod)
                .unwrap()
                .data,
            [24.0]
        );
        assert_eq!(
            call(&[a.clone(), all.clone()], "mean", Red::Mean)
                .unwrap()
                .data,
            [2.5]
        );
        assert_eq!(
            call(&[a.clone(), all.clone()], "all", Red::All)
                .unwrap()
                .data,
            [1.0]
        );
        let nan = val(rmat(2, 2, &[0.0, 0.0, 0.0, f64::NAN]));
        assert_eq!(
            call(&[nan, all.clone()], "any", Red::Any).unwrap().data,
            [0.0]
        );
        // An empty reduces to the identity element, as a 1x1.
        let e = call(
            &[val(Matrix::filled(0, 3, 0.0)), all.clone()],
            "sum",
            Red::Sum,
        )
        .unwrap();
        assert_eq!(e, Matrix::scalar(0.0));
        let e = call(&[val(Matrix::empty()), all.clone()], "prod", Red::Prod).unwrap();
        assert_eq!(e, Matrix::scalar(1.0));
        let max = |a: &[Value]| {
            extremum(a, "max", true, 1).unwrap()[0]
                .clone()
                .into_mat()
                .unwrap()
        };
        let none = val(Matrix::empty());
        assert_eq!(max(&[a.clone(), none.clone(), all.clone()]).data, [4.0]);
        let min = extremum(&[a.clone(), none.clone(), all.clone()], "min", false, 1).unwrap();
        assert_eq!(min[0].clone().into_mat().unwrap().data, [1.0]);
        // max has no identity element, so an empty stays the empty.
        let e = max(&[none.clone(), none.clone(), all]);
        assert_eq!((e.rows, e.cols), (0, 0));
        // Any other char is a bad dimension, never a character code.
        let e = reduction(&[a.clone(), text("x")], "sum", Red::Sum)
            .unwrap_err()
            .msg;
        assert_eq!(
            e,
            "Dimension argument to 'sum' must be a positive integer scalar."
        );
        assert!(extremum(&[a, none, text("x")], "max", true, 1).is_err());
    }

    #[test]
    fn max_and_min_of_an_empty_follow_the_empty_dimension_rule() {
        let shape = |m: Matrix, d: Option<f64>, is_max: bool| {
            let mut args = vec![val(m)];
            if let Some(d) = d {
                args.push(val(Matrix::empty()));
                args.push(val(Matrix::scalar(d)));
            }
            let r = extremum(&args, "max", is_max, 1).unwrap()[0]
                .clone()
                .into_mat()
                .unwrap();
            (r.rows, r.cols)
        };
        let z = |r, c| Matrix::filled(r, c, 0.0);
        assert_eq!(shape(z(3, 0), None, true), (1, 0));
        assert_eq!(shape(z(0, 3), None, true), (0, 3));
        assert_eq!(shape(z(0, 3), Some(2.0), true), (0, 1));
        assert_eq!(shape(z(3, 0), Some(2.0), false), (3, 0));
        assert_eq!(shape(z(0, 0), Some(1.0), true), (0, 0));
        assert_eq!(shape(z(0, 0), None, true), (0, 0));
        assert_eq!(shape(z(1, 0), None, false), (1, 0));
        assert_eq!(shape(z(0, 3), Some(3.0), true), (0, 3));
        // A non-empty input is unchanged by the rule.
        assert_eq!(shape(z(2, 3), None, true), (1, 3));
        assert_eq!(shape(z(2, 3), Some(2.0), true), (2, 1));
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1.0)
    }

    #[test]
    fn round_to_decimals_scales_by_a_power_of_ten() {
        let pi = std::f64::consts::PI;
        assert!(close(round_decimals(pi, 2.0), 314.0 / 100.0));
        assert!(close(round_decimals(pi, 3.0), 3142.0 / 1000.0));
        assert!(close(round_decimals(-pi, 1.0), -3.1));
        assert_eq!(round_decimals(2.5, 0.0), 3.0);
        assert_eq!(round_decimals(-2.5, 0.0), -3.0);
        // Negative n rounds to tens, hundreds, ...
        assert_eq!(round_decimals(863_178_137.0, -2.0), 863_178_100.0);
        assert_eq!(round_decimals(1234.0, -1.0), 1230.0);
        assert_eq!(round_decimals(-1250.1, -2.0), -1300.0);
        // Ties go away from zero.
        assert_eq!(round_decimals(150.0, -2.0), 200.0);
        assert_eq!(round_decimals(-150.0, -2.0), -200.0);
    }

    #[test]
    fn round_guards_against_scaling_out_of_range() {
        let pi = std::f64::consts::PI;
        // Already an integer at the scale: x itself, exactly.
        assert_eq!(round_decimals(pi, 20.0), pi);
        assert_eq!(round_decimals(pi, 16.0), pi);
        // The scaled value overflows: x itself.
        assert_eq!(round_decimals(1e307, 2.0), 1e307);
        // 10^n itself overflows: x itself.
        assert_eq!(round_decimals(pi, 400.0), pi);
        assert_eq!(round_decimals(pi, 1e300), pi);
        // 10^-n overflows: every finite x is a tiny part of the step.
        assert_eq!(round_decimals(5.0, -400.0), 0.0);
        assert_eq!(round_decimals(1e300, -1e300), 0.0);
        // A huge x with a negative n is already a multiple.
        assert_eq!(round_decimals(1e300, -2.0), 1e300);
        // Non-finite values and zero pass through.
        assert!(round_decimals(f64::NAN, 2.0).is_nan());
        assert_eq!(round_decimals(f64::INFINITY, -2.0), f64::INFINITY);
        assert_eq!(round_decimals(0.0, -400.0), 0.0);
        // A tiny x underflows cleanly to zero.
        assert_eq!(round_decimals(1e-300, 5.0), 0.0);
    }

    #[test]
    fn round_to_significant_digits() {
        assert_eq!(round_significant(1253.0, 2.0), 1300.0);
        assert!(close(round_significant(1.345, 2.0), 1.3));
        assert_eq!(round_significant(120.44, 2.0), 120.0);
        assert!(close(round_significant(0.012345, 3.0), 0.0123));
        assert_eq!(round_significant(-987.0, 1.0), -1000.0);
        assert_eq!(round_significant(0.0, 3.0), 0.0);
        assert!(round_significant(f64::NAN, 3.0).is_nan());
        assert_eq!(round_significant(f64::NEG_INFINITY, 3.0), f64::NEG_INFINITY);
        let pi = std::f64::consts::PI;
        assert_eq!(round_significant(pi, 30.0), pi);
    }

    #[test]
    fn round_checks_its_digit_count_and_type() {
        let num = |v: f64| val(Matrix::scalar(v));
        let pi = std::f64::consts::PI;
        let r = |a: &[Value]| round(a).map(|v| v[0].clone().into_mat().unwrap().data[0]);
        assert!(close(r(&[num(pi), num(2.0)]).unwrap(), 314.0 / 100.0));
        assert!(close(
            r(&[num(pi), num(2.0), text("decimals")]).unwrap(),
            314.0 / 100.0
        ));
        assert!(close(
            r(&[num(pi), num(2.0), text("significant")]).unwrap(),
            3.1
        ));
        assert_eq!(r(&[num(2.5)]).unwrap(), 3.0);
        let digits = "Number of digits for 'round' must be an integer scalar.";
        let sig = "Number of significant digits for 'round' must be a positive integer scalar.";
        let kind = "Rounding type for 'round' must be 'decimals' or 'significant'.";
        for bad in [1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(r(&[num(pi), num(bad)]).unwrap_err().msg, digits, "{bad}");
        }
        assert_eq!(r(&[num(pi), text("a")]).unwrap_err().msg, digits);
        assert_eq!(
            r(&[num(pi), num(0.0), text("significant")])
                .unwrap_err()
                .msg,
            sig
        );
        assert_eq!(
            r(&[num(pi), num(1.5), text("significant")])
                .unwrap_err()
                .msg,
            sig
        );
        assert_eq!(
            r(&[num(pi), num(2.0), text("banker")]).unwrap_err().msg,
            kind
        );
        assert_eq!(r(&[num(pi), num(2.0), num(1.0)]).unwrap_err().msg, kind);
        // Element-wise over an array, with the shape kept.
        let v = round(&[
            val(Matrix::row(vec![1253.0, 1.345, 120.44])),
            num(2.0),
            text("significant"),
        ])
        .unwrap()[0]
            .clone()
            .into_mat()
            .unwrap();
        assert_eq!((v.rows, v.cols), (1, 3));
        assert_eq!(v.data[0], 1300.0);
    }

    #[test]
    fn a_reduction_rejects_dimension_zero() {
        let args = [
            Value::Mat(Matrix::row(vec![1.0, 2.0])),
            Value::Mat(Matrix::scalar(0.0)),
        ];
        let e = reduction(&args, "sum", Red::Sum).unwrap_err().msg;
        assert!(e.contains("positive integer"), "{e}");
        let e = cumulative(&args, "cumsum", true).unwrap_err().msg;
        assert!(e.contains("positive integer"), "{e}");
    }

    // ---- the second output of max and min (cycle 03) -----------------

    fn two(args: &[Value], is_max: bool) -> (Matrix, Matrix) {
        let mut out = extremum(args, if is_max { "max" } else { "min" }, is_max, 2).unwrap();
        assert_eq!(out.len(), 2);
        let i = out.pop().unwrap().into_mat().unwrap();
        (out.pop().unwrap().into_mat().unwrap(), i)
    }

    #[test]
    fn arg_extremum_takes_the_first_of_a_tie_and_skips_nan() {
        assert_eq!(arg_extremum(&[3.0, 9.0, 2.0], true), 2);
        assert_eq!(arg_extremum(&[3.0, 9.0, 2.0], false), 3);
        assert_eq!(arg_extremum(&[5.0, 5.0, 1.0], true), 1);
        assert_eq!(arg_extremum(&[1.0, 5.0, 5.0], true), 2);
        assert_eq!(arg_extremum(&[f64::NAN, 2.0, 1.0], true), 2);
        assert_eq!(arg_extremum(&[f64::NAN, f64::NAN], true), 1);
        assert_eq!(arg_extremum(&[-f64::INFINITY, -1.0], false), 1);
    }

    #[test]
    fn max_and_min_give_an_index_along_the_reduced_dimension() {
        let v = val(Matrix::row(vec![3.0, 9.0, 2.0]));
        let (m, i) = two(std::slice::from_ref(&v), true);
        assert_eq!((m.data, i.data), (vec![9.0], vec![2.0]));
        let (m, i) = two(&[v], false);
        assert_eq!((m.data, i.data), (vec![2.0], vec![3.0]));
        // A matrix reduces down its columns; the index is a double.
        let a = val(rmat(2, 2, &[4.0, 1.0, 2.0, 3.0]));
        let (m, i) = two(std::slice::from_ref(&a), false);
        assert_eq!((m.data, i.data.clone()), (vec![2.0, 1.0], vec![2.0, 1.0]));
        assert_eq!((i.rows, i.cols, i.class), (1, 2, Class::Double));
        // Along the second dimension, and over 'all' as a linear index.
        let none = val(Matrix::empty());
        let (_, i) = two(&[a.clone(), none.clone(), val(Matrix::scalar(2.0))], true);
        assert_eq!((i.rows, i.cols, i.data), (2, 1, vec![1.0, 2.0]));
        let (m, i) = two(&[a.clone(), none.clone(), text("all")], true);
        assert_eq!((m.data, i.data), (vec![4.0], vec![1.0]));
        let (m, i) = two(&[a.clone(), none.clone(), text("all")], false);
        assert_eq!((m.data, i.data), (vec![1.0], vec![3.0]));
        // A dimension past the array's is every element, each at 1.
        let (_, i) = two(&[a, none.clone(), val(Matrix::scalar(3.0))], true);
        assert_eq!((i.rows, i.cols, i.data), (2, 2, vec![1.0; 4]));
        // An empty gives an empty index of the same shape as the value: a
        // reduced dimension of 0 keeps the whole shape, as the value does.
        let (m, i) = two(&[val(Matrix::new(0, 3, vec![]))], true);
        assert_eq!((m.rows, m.cols, i.rows, i.cols), (0, 3, 0, 3));
        let (m, i) = two(&[val(Matrix::empty())], true);
        assert_eq!((m.rows, m.cols, i.rows, i.cols), (0, 0, 0, 0));
        // A logical keeps its class in the value, never in the index.
        let t = val(Matrix::row(vec![0.0, 1.0]).with_class(Class::Logical));
        let (m, i) = two(&[t], true);
        assert_eq!(
            (m.class, i.class, i.data),
            (Class::Logical, Class::Double, vec![2.0])
        );
        // The two-array form has no index to give.
        let pair = [val(Matrix::scalar(1.0)), val(Matrix::scalar(2.0))];
        assert_eq!(extremum(&pair, "max", true, 2).unwrap().len(), 1);
    }
}
