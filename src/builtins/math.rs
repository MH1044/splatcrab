//! Reductions, element-wise math and the two-argument numeric functions.

use super::args::{Along, at_most, check_dims, dim, dim_or_all, mat, need, option};
use super::{Registry, add, one_as, one_mat};
use crate::builtins::complex::C;
use crate::error;
use crate::interp::R;
use crate::value::{Class, Matrix, Value, along_dim, normalize_dims};

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
    add(r, "abs", |_, a, _| abs(a), "abs(X) - absolute value; of a complex number, its modulus.");
    add(r, "sqrt", |_, a, _| complex_unary(a, "sqrt", C::sqrt), "sqrt(X) - square root; of a negative number, complex.");
    add(r, "exp", |_, a, _| complex_unary(a, "exp", C::exp), "exp(X) - e raised to the power X.");
    add(r, "log", |_, a, _| complex_unary(a, "log", C::ln), "log(X) - natural logarithm; of a negative number, complex.");
    add(r, "log2", |_, a, _| complex_unary(a, "log2", C::log2), "log2(X) - base 2 logarithm; of a negative number, complex.");
    add(r, "log10", |_, a, _| complex_unary(a, "log10", C::log10), "log10(X) - base 10 logarithm; of a negative number, complex.");
    add(r, "sin", |_, a, _| complex_unary(a, "sin", C::sin), "sin(X) - sine of X in radians.");
    add(r, "cos", |_, a, _| complex_unary(a, "cos", C::cos), "cos(X) - cosine of X in radians.");
    add(r, "tan", |_, a, _| unary(a, "tan", f64::tan), "tan(X) - tangent of X in radians.");
    add(r, "asin", |_, a, _| complex_unary(a, "asin", C::asin), "asin(X) - inverse sine, in radians; complex outside [-1, 1].");
    add(r, "acos", |_, a, _| complex_unary(a, "acos", C::acos), "acos(X) - inverse cosine, in radians; complex outside [-1, 1].");
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
    add(r, "power", |_, a, _| power(a), "power(X,Y) - element-wise X raised to the power Y; complex where it must be.");
}

/// `power(X, Y)`, which is `X .^ Y`: `interp.rs` computes `^` and `.^`
/// through the same [`C::pow`], so the three spellings `(-8)^(1/3)`,
/// `(-8).^(1/3)` and `power(-8, 1/3)` agree, on `1 + 1.7321i` since cycle
/// 10 (a refusal before it).
fn power(args: &[Value]) -> R<Vec<Value>> {
    at_most(args, 2, "power")?;
    need(args, 2, "power")?;
    let a = mat(args, 0, "power")?;
    let b = mat(args, 1, "power")?;
    one_mat(a.zip_c(&b, "power", C::pow)?)
}

/// `abs(X)`: the modulus of a complex element, the magnitude of a real one.
fn abs(args: &[Value]) -> R<Vec<Value>> {
    at_most(args, 1, "abs")?;
    let m = mat(args, 0, "abs")?;
    if !m.is_complex() {
        return one_mat(m.map(f64::abs));
    }
    let data = (0..m.numel()).map(|k| m.c(k).abs()).collect();
    // Every dimension kept (cycle 14b), as the real path's `map` keeps it.
    one_mat(Matrix::from_dims(&m.dims(), data))
}

/// A one-argument element-wise function of complex scalars (cycle 10),
/// which is real wherever its [`C`] method is: `sqrt`, `exp`, `log`,
/// `log2`, `log10`, `sin`, `cos`, `asin` and `acos`. A negative argument to
/// `sqrt` or a logarithm, and one outside `[-1, 1]` to `asin` or `acos`,
/// gives the complex value on the principal branch; the branch cuts are
/// recorded in `builtins/complex.rs`. The result is stored by the flag
/// rule, so `sqrt([4 9])` stays real.
fn complex_unary(args: &[Value], name: &str, f: fn(C) -> C) -> R<Vec<Value>> {
    at_most(args, 1, name)?;
    one_mat(mat(args, 0, name)?.map_c(f))
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
    if m.is_complex() {
        return one_mat(complex_reduction(&m, along, kind, name)?);
    }
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
    // `any` and `all` answer with a logical, as MATLAB's do. Along a
    // dimension of size 1 within `ndims` each element is reduced on its
    // own, a `NaN` ignored (cycle 14b); past `ndims` the argument itself
    // is converted, as it always was: `any([2 0], 3)` is the logical `1 0`,
    // and `any(NaN, 3)` refuses the `NaN`.
    match kind {
        Red::Any | Red::All => one_as(out.to_class(Class::Logical)?),
        _ => one_mat(out),
    }
}

/// `sum`, `prod` and `mean` of a complex array (cycle 10). A sum and a mean
/// are linear, so each is the real reduction of the two parts; a product
/// multiplies complex scalars. `any` and `all` refuse one, as the
/// registry's gate already does before they run.
fn complex_reduction(m: &Matrix, along: Option<Along>, kind: Red, name: &str) -> R<Matrix> {
    let parts = |f: fn(&[f64]) -> f64| -> R<Matrix> {
        let (re, im) = (m.real_part(), m.imag_part());
        let (re, im) = match along {
            None => (reduce(&re, None, f)?, reduce(&im, None, f)?),
            Some(Along::Dim(d)) => (reduce(&re, Some(d), f)?, reduce(&im, Some(d), f)?),
            Some(Along::All) => (reduce_all(&re, f), reduce_all(&im, f)),
        };
        Ok(re.with_im(Some(im.data)))
    };
    match kind {
        Red::Sum => parts(sum0),
        Red::Mean => parts(|xs| sum0(xs) / xs.len() as f64),
        Red::Any | Red::All => Err(error::complex_argument(name)),
        Red::Prod => {
            let prod = |z: &[C]| z.iter().fold(C::real(1.0), |acc, &v| acc * v);
            match along {
                Some(Along::All) => {
                    let z: Vec<C> = (0..m.numel()).map(|k| m.c(k)).collect();
                    Ok(Matrix::from_c(1, 1, vec![prod(&z)]))
                }
                Some(Along::Dim(d)) => reduce_c(m, Some(d), prod),
                None => reduce_c(m, None, prod),
            }
        }
    }
}

/// [`reduce`] over complex scalars, with the same dimension rule, the same
/// shapes and the same size check, stored by the flag rule. Past `ndims`
/// the argument is handed back with its storage, as [`reduce`] hands it
/// back: `prod(complex(1, 0), 3)` stays complex.
fn reduce_c(m: &Matrix, dim: Option<usize>, f: impl Fn(&[C]) -> C) -> R<Matrix> {
    let dims = m.dims();
    let d = match dim {
        Some(d) => d,
        None if is_zero_by_zero(m) => return Ok(Matrix::from_c(1, 1, vec![f(&[])])),
        None => default_dim(&dims),
    };
    if past_ndims(m, d) {
        return Ok(m.clone());
    }
    let out = judged(&reduced_dims(&dims, d))?;
    let z: Vec<C> = (0..m.numel()).map(|k| m.c(k)).collect();
    let (re, im): (Vec<f64>, Vec<f64>) = each_slice(&z, &dims, d, f)
        .into_iter()
        .map(|z| (z.re, z.im))
        .unzip();
    Ok(Matrix::from_dims(&out, re).with_im(Some(im)))
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

/// Where `max` or `min` reduces a one- or three-argument call: the
/// dimension asked for, or the first whose size is not 1, which for a 0x0
/// is the first.
fn extremum_along_arg(args: &[Value], m: &Matrix, name: &str) -> R<Along> {
    Ok(if args.len() >= 3 {
        dim_or_all(args, 2, name)?
    } else {
        Along::Dim(default_dim(&m.dims()))
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
        // Past `ndims` the first output is the argument itself, and every
        // slice one element, the first of its slice.
        Along::Dim(d) if past_ndims(&m, d) => Matrix::filled_dims(&m.dims(), 1.0),
        // The empty the first output is, with no elements to index.
        Along::Dim(d) if along_dim(&m.dims(), d).1 == 0 => Matrix::from_dims(&m.dims(), Vec::new()),
        // Along a dimension of size 1 every slice is one element, so every
        // index is 1.
        Along::Dim(d) => reduce(&m, Some(d), f)?,
    })
}

fn extremum_value(args: &[Value], name: &str, is_max: bool) -> R<Vec<Value>> {
    at_most(args, 3, name)?;
    let m = mat(args, 0, name)?;
    // `max(A, B, dim)` read the dimension and dropped `B` in silence, a
    // wrong answer, so a second array with elements beside a dimension is
    // refused (cycle 14b); the placeholder `[]` of `max(A, [], dim)` has
    // none.
    if args.len() == 3 && args[1].numel() > 0 {
        return Err(error::extremum_two_arrays_and_dim(name));
    }
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
/// size as A." Otherwise the reduced dimension becomes 1, even when another
/// one is 0, so `max(zeros(3, 0))` is 1x0 and `max(zeros(0, 3), [], 2)` is
/// 0x1. `max` has no identity element, which is why this is not `sum`'s rule.
/// Any dimension of an N-D array alike (cycle 14b).
fn extremum_along(m: &Matrix, d: usize, f: impl Fn(&[f64]) -> f64) -> R<Matrix> {
    if along_dim(&m.dims(), d).1 == 0 {
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

/// MATLAB's default dimension for a reduction, a scan and `max` and `min`,
/// by the `sum` page's rule (cycle 14b): "the first array dimension whose
/// size does not equal 1", and dimension 1 when every size is 1. For a 2-D
/// matrix it is the rows unless the matrix is a row, as it always was, and
/// for a 1x1x3 it is the third.
pub fn default_dim(dims: &[usize]) -> usize {
    dims.iter().position(|&d| d != 1).map_or(1, |k| k + 1)
}

/// The 2-D 0x0 empty, whose reduction with no dimension is today's special
/// case: `sum([])` is `0` and `prod([])` is `1`.
fn is_zero_by_zero(m: &Matrix) -> bool {
    m.rows == 0 && m.cols == 0 && !m.is_nd()
}

/// True when `d` is past `ndims(m)`, where every reduction and scan hands
/// `m` back as it is, values and storage alike (cycle 14b): the path a
/// matrix and a dimension past 2 always took, so `1/sum(-0, 3)` is `-Inf`
/// and `prod(complex(1, 0), 3)` keeps its complex storage, and an N-D
/// array past its own `ndims` takes the same path. `any` and `all` then
/// convert `m` to a logical, as they always did there, which refuses a
/// `NaN`.
fn past_ndims(m: &Matrix, d: usize) -> bool {
    d > m.ndims()
}

/// The shape of a reduction of an array of shape `dims` along `d`, by the
/// `sum` page's rule: "the size of S in this dimension becomes 1 while the
/// sizes of all other dimensions remain the same as in A", trailing sizes
/// of 1 dropped. Along a dimension past `ndims` it is the array's own.
fn reduced_dims(dims: &[usize], d: usize) -> Vec<usize> {
    let mut out = dims.to_vec();
    if let Some(size) = out.get_mut(d.saturating_sub(1)) {
        *size = 1;
    }
    normalize_dims(&out)
}

/// A shape computed from an operand's, judged by `check_dims` before
/// anything is allocated for it.
fn judged(dims: &[usize]) -> R<Vec<usize>> {
    let asked: Vec<f64> = dims.iter().map(|&d| d as f64).collect();
    check_dims(&asked)
}

/// `f` of every slice of an array of shape `dims` along dimension `d`, in
/// the column-major order of the result: the one kernel every reduction
/// runs (cycle 14b), over the array seen as `[before, n, after]`
/// ([`along_dim`]), so slice `(b, a)` is the `n` elements at `b + before *
/// (k + n * a)` and its answer lands at `b + before * a`. Along a
/// dimension of size 1 each slice is one element (past `ndims` the callers
/// hand the argument back before coming here, [`past_ndims`]); along a
/// dimension of size 0 each is empty, and `f` answers for nothing. There
/// are `before * after` slices, the result's elements, which the caller
/// has judged; no loop runs over a dimension of an empty array, since an
/// empty result has no slices and an empty slice no elements.
fn each_slice<T: Copy, U>(
    src: &[T],
    dims: &[usize],
    d: usize,
    mut f: impl FnMut(&[T]) -> U,
) -> Vec<U> {
    let (before, n, after) = along_dim(dims, d);
    let count = before.saturating_mul(after);
    let mut out = Vec::with_capacity(count);
    if count == 0 {
        return out;
    }
    if n == 0 {
        for _ in 0..count {
            out.push(f(&[]));
        }
    } else if before == 1 {
        // Each slice is contiguous: a column of a matrix, or the elements
        // along the first dimension that is not 1.
        for s in src.chunks_exact(n) {
            out.push(f(s));
        }
    } else {
        // At most `src.len()`, since every slice here has elements.
        let mut buf = Vec::with_capacity(n);
        for a in 0..after {
            for b in 0..before {
                buf.clear();
                buf.extend((0..n).map(|k| src[b + before * (k + n * a)]));
                out.push(f(&buf));
            }
        }
    }
    out
}

/// Reduce along a dimension: `f` of each slice along `d` (see
/// [`each_slice`]), with the result's shape by [`reduced_dims`]. Without a
/// dimension it is [`default_dim`]'s, and a 2-D 0x0 collapses to `f` of
/// nothing: `sum([])` is 0, but `sum([], 1)` keeps MATLAB's 1x0 empty.
///
/// Since cycle 14b any dimension of any array: `sum(A, 3)` of a 2x3x4 is
/// 2x3, a dimension of size 1 within `ndims` gives each element's own
/// reduction (`sum(-0, 1)` is `+0`, as it always was), and one of size 0
/// gives `f` of nothing in every element. A dimension past `ndims` hands
/// the argument back, "sum returns A when dim is greater than ndims(A)"
/// (the MathWorks `sum` page), as a matrix and a dimension past 2 always
/// did, so a `-0` stays `-0` there: [`past_ndims`]. `0` never reaches
/// here, because `args::dim` rejects it.
/// The result shape comes from the operand, and one dimension of an operand
/// can be enormous while the operand itself is empty: `sum(zeros(0, 1e15))`
/// is a 1x1e15 row built from no elements at all, and used to abort in the
/// allocator. The shape therefore goes through `args::check_dims` first.
pub fn reduce(m: &Matrix, dim: Option<usize>, f: impl Fn(&[f64]) -> f64) -> R<Matrix> {
    let dims = m.dims();
    let d = match dim {
        Some(d) => d,
        None if is_zero_by_zero(m) => return Ok(Matrix::scalar(f(&[]))),
        None => default_dim(&dims),
    };
    if past_ndims(m, d) {
        return Ok(m.clone());
    }
    let out = judged(&reduced_dims(&dims, d))?;
    Ok(Matrix::from_dims(&out, each_slice(&m.data, &dims, d, f)))
}

fn cumulative(args: &[Value], name: &str, is_sum: bool) -> R<Vec<Value>> {
    at_most(args, 2, name)?;
    let m = mat(args, 0, name)?;
    let d = if args.len() >= 2 {
        Some(dim(args, 1, name)?)
    } else {
        None
    };
    if m.is_complex() {
        // Only `cumsum` gets here, a running sum, which is linear: the
        // registry's gate refuses a complex argument to `cumprod`.
        let re = scan(&m.real_part(), d, is_sum);
        let im = scan(&m.imag_part(), d, is_sum);
        return one_mat(re.with_im(Some(im.data)));
    }
    one_mat(scan(&m, d, is_sum))
}

/// Running sum or product along a dimension. The dimension argument used to
/// be read and then ignored, which always gave the down-the-columns answer.
///
/// Since cycle 14b along any dimension of any array, by the reductions'
/// rule: the default is [`default_dim`]'s, every size is kept, and the
/// array is seen as `[before, n, after]` ([`along_dim`]), each of its
/// `before * after` slices accumulated in order. Along a dimension of size
/// 1 within `ndims`, each element is its own running value; past `ndims`
/// the argument is handed back as it is, as [`reduce`] hands it back, so
/// `1/cumsum(-0, 3)` is `-Inf`.
pub fn scan(m: &Matrix, dim: Option<usize>, is_sum: bool) -> Matrix {
    let dims = m.dims();
    let d = dim.unwrap_or_else(|| default_dim(&dims));
    // An empty array has nothing to scan, and one of its dimensions alone
    // can be enormous (cycle 13b).
    if past_ndims(m, d) || m.numel() == 0 {
        return m.clone();
    }
    let step = |acc: f64, v: f64| if is_sum { acc + v } else { acc * v };
    let seed = if is_sum { 0.0 } else { 1.0 };
    let (before, n, after) = along_dim(&dims, d);
    let mut out = m.data.clone();
    for a in 0..after {
        for b in 0..before {
            let mut acc = seed;
            for k in 0..n {
                let at = b + before * (k + n * a);
                acc = step(acc, m.data[at]);
                out[at] = acc;
            }
        }
    }
    Matrix::from_dims(&dims, out)
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

    fn out(v: R<Vec<Value>>) -> Matrix {
        v.unwrap()[0].clone().into_mat().unwrap()
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1.0)
    }

    /// Cycle 10 replaced cycle 01d's refusals with the complex values, on
    /// the principal branch, and kept a real result stored real.
    #[test]
    fn the_functions_of_a_negative_number_are_complex() {
        let pi = std::f64::consts::PI;
        let s = out(complex_unary(&[val(Matrix::scalar(-4.0))], "sqrt", C::sqrt));
        assert_eq!((s.data[0], s.im.as_deref()), (0.0, Some(&[2.0][..])));
        let l = out(complex_unary(&[val(Matrix::scalar(-1.0))], "log", C::ln));
        assert!(near(l.data[0], 0.0) && near(l.im.as_ref().unwrap()[0], pi));
        let l2 = out(complex_unary(&[val(Matrix::scalar(-8.0))], "log2", C::log2));
        assert!(near(l2.data[0], 3.0));
        assert!(near(
            l2.im.as_ref().unwrap()[0],
            pi / std::f64::consts::LN_2
        ));
        let l10 = out(complex_unary(
            &[val(Matrix::scalar(-100.0))],
            "log10",
            C::log10,
        ));
        assert!(near(l10.data[0], 2.0));
        let a = out(complex_unary(&[val(Matrix::scalar(2.0))], "asin", C::asin));
        assert!(near(a.data[0], pi / 2.0));
        let a = out(complex_unary(&[val(Matrix::scalar(2.0))], "acos", C::acos));
        assert!(near(a.data[0], 0.0) && a.im.as_ref().unwrap()[0] > 0.0);
        // On the real domain the values are the real functions', stored
        // real, and NaN stays NaN.
        let r = out(complex_unary(
            &[val(Matrix::row(vec![4.0, 0.0, f64::INFINITY]))],
            "sqrt",
            C::sqrt,
        ));
        assert!(!r.is_complex());
        assert_eq!(r.data, [2.0, 0.0, f64::INFINITY]);
        let n = out(complex_unary(
            &[val(Matrix::scalar(f64::NAN))],
            "log",
            C::ln,
        ));
        assert!(!n.is_complex() && n.data[0].is_nan());
        // One negative element makes the whole result complex.
        let m = out(complex_unary(
            &[val(Matrix::row(vec![1.0, -4.0, 9.0]))],
            "sqrt",
            C::sqrt,
        ));
        assert_eq!(m.data, [1.0, 0.0, 3.0]);
        assert_eq!(m.im.as_deref(), Some(&[0.0, 2.0, 0.0][..]));
    }

    #[test]
    fn a_negative_base_with_a_fractional_exponent_is_complex() {
        let a = val(Matrix::row(vec![-8.0, 8.0]));
        let b = val(Matrix::scalar(1.0 / 3.0));
        let p = out(power(&[a, b]));
        assert!(near(p.data[0], 1.0) && near(p.im.as_ref().unwrap()[0], 3.0f64.sqrt()));
        assert!(near(p.data[1], 2.0) && p.im.as_ref().unwrap()[1] == 0.0);
        // An integer or infinite exponent of a negative base stays real.
        let q = out(power(&[
            val(Matrix::row(vec![-8.0, -2.0])),
            val(Matrix::row(vec![2.0, f64::INFINITY])),
        ]));
        assert!(!q.is_complex());
        assert_eq!(q.data, [64.0, f64::INFINITY]);
    }

    #[test]
    fn complex_reductions_and_abs() {
        let z = Matrix::complex_parts(2, 2, vec![1.0, 2.0, 3.0, 4.0], vec![1.0, -1.0, 2.0, 0.0]);
        let s = out(reduction(&[val(z.clone())], "sum", Red::Sum));
        assert_eq!(
            (s.data.clone(), s.im.clone()),
            (vec![3.0, 7.0], Some(vec![0.0, 2.0]))
        );
        let m = out(reduction(&[val(z.clone()), text("all")], "mean", Red::Mean));
        assert_eq!((m.data[0], m.im.as_ref().unwrap()[0]), (2.5, 0.5));
        // (1+1i)(2-1i) = 3+1i; (3+2i)(4) = 12+8i.
        let p = out(reduction(&[val(z.clone())], "prod", Red::Prod));
        assert_eq!(p.data, [3.0, 12.0]);
        assert_eq!(p.im.as_deref(), Some(&[1.0, 8.0][..]));
        // (1+1i)(1-1i) = 2, stored real by the flag rule.
        let w = Matrix::complex_parts(1, 2, vec![1.0, 1.0], vec![1.0, -1.0]);
        let p = out(reduction(&[val(w.clone())], "prod", Red::Prod));
        assert!(!p.is_complex());
        assert_eq!(p.data, [2.0]);
        let c = out(cumulative(&[val(w)], "cumsum", true));
        assert_eq!(c.data, [1.0, 2.0]);
        assert_eq!(c.im.as_deref(), Some(&[1.0, 0.0][..]));
        let a = out(abs(&[val(Matrix::complex_parts(
            1,
            1,
            vec![3.0],
            vec![-4.0],
        ))]));
        assert_eq!(a.data, [5.0]);
        assert!(!a.is_complex());
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
        let placeholder = [one[0].clone(), Value::Mat(Matrix::empty()), one[0].clone()];
        assert!(extremum(&placeholder, "max", true, 1).is_ok());
        // Three arguments are within the arity; two arrays beside a
        // dimension are refused for what they are (cycle 14b).
        assert_eq!(
            extremum(&three, "max", true, 1).unwrap_err().msg,
            "max takes two arrays or one array and a dimension, not both."
        );
        assert_eq!(
            extremum(&four, "max", true, 1).unwrap_err().msg,
            "Too many input arguments."
        );
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

    // ---- N-D reductions (cycle 14b) -----------------------------------

    /// `reshape(1:24, 2, 3, 4)`, whose element `(i, j, k)` is `i + 2*(j-1)
    /// + 6*(k-1)`: the spec's running example.
    fn nd24() -> Matrix {
        Matrix::from_dims(&[2, 3, 4], (1..=24).map(f64::from).collect())
    }

    fn dim_arg(d: f64) -> Value {
        val(Matrix::scalar(d))
    }

    #[test]
    fn the_default_dimension_is_the_first_whose_size_is_not_one() {
        assert_eq!(default_dim(&[2, 3]), 1);
        assert_eq!(default_dim(&[1, 3]), 2);
        assert_eq!(default_dim(&[1, 1]), 1);
        assert_eq!(default_dim(&[0, 0]), 1);
        assert_eq!(default_dim(&[1, 0]), 2);
        assert_eq!(default_dim(&[1, 1, 3]), 3);
        assert_eq!(default_dim(&[1, 1, 1, 4]), 4);
        assert_eq!(default_dim(&[2, 3, 4]), 1);
        // Through the builtins: `sum(ones(1, 1, 3))` is 3.
        let v = val(Matrix::filled_dims(&[1, 1, 3], 1.0));
        assert_eq!(
            call(std::slice::from_ref(&v), "sum", Red::Sum).unwrap(),
            Matrix::scalar(3.0)
        );
        let c = out(cumulative(&[v], "cumsum", true));
        assert_eq!((c.dims(), c.data), (vec![1, 1, 3], vec![1.0, 2.0, 3.0]));
    }

    /// The one kernel sees any array along `d` as three numbers, and every
    /// slice lands where the `sum` page's shape rule puts it.
    #[test]
    fn a_reduction_runs_over_the_three_number_view() {
        assert_eq!(along_dim(&[2, 3, 4], 1), (1, 2, 12));
        assert_eq!(along_dim(&[2, 3, 4], 2), (2, 3, 4));
        assert_eq!(along_dim(&[2, 3, 4], 3), (6, 4, 1));
        assert_eq!(along_dim(&[2, 3, 4], 5), (24, 1, 1));
        assert_eq!(along_dim(&[2, 3], 1), (1, 2, 3));
        assert_eq!(along_dim(&[2, 3], 2), (2, 3, 1));
        let a = nd24();
        let s3 = sum_of(&a, Some(3));
        assert_eq!(s3.dims(), [2, 3]);
        assert_eq!(s3.data, [40.0, 44.0, 48.0, 52.0, 56.0, 60.0]);
        let s1 = sum_of(&a, None);
        assert_eq!(s1.dims(), [1, 3, 4]);
        assert_eq!(s1.data[..4], [3.0, 7.0, 11.0, 15.0]);
        let s2 = sum_of(&a, Some(2));
        assert_eq!(s2.dims(), [2, 1, 4]);
        // Row 1 of page 1 is 1 + 3 + 5; row 2 of page 4 is 20 + 22 + 24.
        assert_eq!((s2.data[0], s2.data[7]), (9.0, 66.0));
        // Every slice against a direct sum of its elements.
        for d in 1..=3 {
            let s = sum_of(&a, Some(d));
            let dims = [2, 3, 4];
            for i in 0..2 {
                for j in 0..3 {
                    for k in 0..4 {
                        let at = [i, j, k];
                        let mut want = 0.0;
                        let mut sub = at;
                        for t in 0..dims[d - 1] {
                            sub[d - 1] = t;
                            want += a.data[sub[0] + 2 * (sub[1] + 3 * sub[2])];
                        }
                        let mut r = at;
                        r[d - 1] = 0;
                        let rd = reduced_dims(&dims, d);
                        let rd: Vec<usize> =
                            (0..3).map(|q| rd.get(q).copied().unwrap_or(1)).collect();
                        assert_eq!(s.data[r[0] + rd[0] * (r[1] + rd[1] * r[2])], want);
                    }
                }
            }
        }
        // mean, prod, any and all through the same kernel.
        let m = call(&[val(a.clone()), dim_arg(3.0)], "mean", Red::Mean).unwrap();
        assert_eq!(m.data, [10.0, 11.0, 12.0, 13.0, 14.0, 15.0]);
        let p = call(
            &[val(Matrix::filled_dims(&[2, 2, 2], 2.0)), dim_arg(3.0)],
            "prod",
            Red::Prod,
        )
        .unwrap();
        assert_eq!((p.dims(), p.data), (vec![2, 2], vec![4.0; 4]));
        let big = a.map(|x| (x > 22.0) as u8 as f64);
        let y = call(&[val(big), dim_arg(3.0)], "any", Red::Any).unwrap();
        assert_eq!(
            (y.class, y.data),
            (Class::Logical, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0])
        );
    }

    /// Along a dimension of size 1 each element is its own reduction, and
    /// past `ndims` the argument is handed back, each element as it was;
    /// along one of size 0 each reduces nothing.
    #[test]
    fn a_dimension_of_size_one_or_past_ndims_reduces_each_element() {
        let a = nd24();
        for d in [5.0, 9.0] {
            let s = call(&[val(a.clone()), dim_arg(d)], "sum", Red::Sum).unwrap();
            assert_eq!(s, a);
            let y = call(&[val(a.clone()), dim_arg(d)], "any", Red::Any).unwrap();
            assert_eq!((y.dims(), y.class), (vec![2, 3, 4], Class::Logical));
            assert!(y.data.iter().all(|&v| v == 1.0));
        }
        let row = val(Matrix::from_dims(
            &[1, 3, 2],
            vec![1.0, 0.0, 2.0, 3.0, 0.0, 4.0],
        ));
        let y = call(&[row.clone(), dim_arg(1.0)], "all", Red::All).unwrap();
        assert_eq!(
            (y.dims(), y.data),
            (vec![1, 3, 2], vec![1.0, 0.0, 1.0, 1.0, 0.0, 1.0])
        );
        let (m, i) = two(&[val(a.clone()), val(Matrix::empty()), dim_arg(5.0)], true);
        assert_eq!(m, a);
        assert_eq!((i.dims(), i.data), (vec![2, 3, 4], vec![1.0; 24]));
        let c = scan(&a, Some(4), true);
        assert_eq!(c, a);
        // Past `ndims` the argument is handed back, values and storage
        // alike, as a matrix and a dimension past 2 always were: a `-0`
        // stays `-0`, where along a dimension of size 1 within `ndims` it
        // is reduced on its own to `+0`.
        let neg = Matrix::scalar(-0.0);
        let sign = |m: &Matrix| m.data[0].is_sign_negative();
        for (kind, name) in [(Red::Sum, "sum"), (Red::Mean, "mean"), (Red::Prod, "prod")] {
            let past = call(&[val(neg.clone()), dim_arg(3.0)], name, kind).unwrap();
            assert!(sign(&past), "{name}");
        }
        assert!(sign(&scan(&neg, Some(3), true)));
        let within = call(&[val(neg.clone()), dim_arg(1.0)], "sum", Red::Sum).unwrap();
        assert!(!sign(&within));
        assert!(!sign(&scan(&neg, Some(1), true)));
        let pages = Matrix::from_dims(&[1, 1, 2], vec![-0.0, -0.0]);
        let signs = |m: Matrix| {
            m.data
                .iter()
                .map(|v| v.is_sign_negative())
                .collect::<Vec<_>>()
        };
        assert_eq!(signs(reduce(&pages, Some(4), sum0).unwrap()), [true, true]);
        assert_eq!(
            signs(reduce(&pages, Some(2), sum0).unwrap()),
            [false, false]
        );
        let z = val(Matrix::complex_parts(1, 1, vec![1.0], vec![0.0]));
        let p = call(&[z.clone(), dim_arg(3.0)], "prod", Red::Prod).unwrap();
        assert!(p.is_complex());
        let s = call(&[z, dim_arg(3.0)], "sum", Red::Sum).unwrap();
        assert!(!s.is_complex());
        // `any` and `all` along a dimension of size 1 reduce each element,
        // a `NaN` ignored; past `ndims` they convert the argument, which
        // refuses a `NaN`, as they always did.
        let nan = val(Matrix::from_dims(&[1, 1, 2], vec![f64::NAN, 0.0]));
        let y = call(&[nan.clone(), dim_arg(1.0)], "any", Red::Any).unwrap();
        assert_eq!((y.dims(), y.data), (vec![1, 1, 2], vec![0.0, 0.0]));
        let y = call(&[nan.clone(), dim_arg(2.0)], "all", Red::All).unwrap();
        assert_eq!(y.data, [1.0, 0.0]);
        for (kind, name) in [(Red::Any, "any"), (Red::All, "all")] {
            let e = call(&[nan.clone(), dim_arg(4.0)], name, kind).unwrap_err();
            assert_eq!(e.msg, "NaN's cannot be converted to logicals.", "{name}");
        }
        // The index of `max` past `ndims` is all 1s, an empty's empty.
        let e = val(Matrix::new(0, 3, Vec::new()));
        let (m, i) = two(&[e, val(Matrix::empty()), dim_arg(3.0)], true);
        assert_eq!((m.dims(), i.dims()), (vec![0, 3], vec![0, 3]));
        // A dimension of size 0: sum 0, prod 1, mean NaN, any false, all
        // true, each the result's element, and max an empty along it.
        let e = val(Matrix::from_dims(&[2, 0, 3], Vec::new()));
        let s = call(&[e.clone(), dim_arg(2.0)], "sum", Red::Sum).unwrap();
        assert_eq!((s.dims(), s.data), (vec![2, 1, 3], vec![0.0; 6]));
        let p = call(&[e.clone(), dim_arg(2.0)], "prod", Red::Prod).unwrap();
        assert_eq!(p.data, [1.0; 6]);
        let m = call(&[e.clone(), dim_arg(2.0)], "mean", Red::Mean).unwrap();
        assert!(m.data.iter().all(|v| v.is_nan()));
        let y = call(&[e.clone(), dim_arg(2.0)], "any", Red::Any).unwrap();
        assert_eq!(y.data, [0.0; 6]);
        let y = call(&[e.clone(), dim_arg(2.0)], "all", Red::All).unwrap();
        assert_eq!(y.data, [1.0; 6]);
        let (m, i) = two(&[e.clone(), val(Matrix::empty()), dim_arg(2.0)], true);
        assert_eq!((m.dims(), i.dims()), (vec![2, 0, 3], vec![2, 0, 3]));
        let (m, _) = two(&[e, val(Matrix::empty()), dim_arg(3.0)], false);
        assert_eq!(m.dims(), [2, 0]);
        // An empty N-D shape that is not 0x0 takes the general rule.
        let z = val(Matrix::from_dims(&[0, 0, 3], Vec::new()));
        assert_eq!(call(&[z], "sum", Red::Sum).unwrap().dims(), [1, 0, 3]);
        // A result too large to hold is judged before it is allocated.
        let wide = Matrix::from_dims(&[0, 1 << 20, 1 << 20], Vec::new());
        let e = reduce(&wide, Some(1), sum0).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 1x1048576x1048576 array exceeds the maximum array size."
        );
    }

    #[test]
    fn max_and_min_reduce_and_index_along_any_dimension() {
        let a = nd24();
        let (m, i) = two(&[val(a.clone()), val(Matrix::empty()), dim_arg(3.0)], true);
        assert_eq!(m.data, [19.0, 20.0, 21.0, 22.0, 23.0, 24.0]);
        assert_eq!((i.dims(), i.data), (vec![2, 3], vec![4.0; 6]));
        let (m, i) = two(&[val(a.clone()), val(Matrix::empty()), dim_arg(2.0)], false);
        assert_eq!(m.dims(), [2, 1, 4]);
        assert_eq!(m.data, [1.0, 2.0, 7.0, 8.0, 13.0, 14.0, 19.0, 20.0]);
        assert_eq!(i.data, [1.0; 8]);
        let (m, _) = two(&[val(a.clone())], true);
        assert_eq!(m.dims(), [1, 3, 4]);
        // The two-array form broadcasts across every dimension.
        let b = out(extremum(
            &[val(a.clone()), val(Matrix::scalar(12.0))],
            "max",
            true,
            1,
        ));
        assert_eq!(b.dims(), [2, 3, 4]);
        assert_eq!(b.data[..12], [12.0; 12]);
        assert_eq!(b.data[12], 13.0);
        let col = val(Matrix::col(vec![10.0, 20.0]));
        let b = out(extremum(&[val(a.clone()), col], "min", false, 1));
        assert_eq!(b.data[..4], [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(b.data[22..], [10.0, 20.0]);
        let e = extremum(
            &[val(a), val(Matrix::filled_dims(&[2, 3, 5], 0.0))],
            "max",
            true,
            1,
        )
        .unwrap_err()
        .msg;
        assert_eq!(
            e,
            "Arrays have incompatible sizes for operator 'max' (2x3x4 vs 2x3x5)."
        );
        // Two arrays and a dimension are refused, where the second array
        // used to be dropped in silence; an empty second argument is the
        // placeholder of the one-array form.
        let (x, y) = (Matrix::row(vec![1.0, 5.0]), Matrix::row(vec![3.0, 4.0]));
        for (name, is_max) in [("max", true), ("min", false)] {
            for nargout in [1, 2] {
                let args = [val(x.clone()), val(y.clone()), dim_arg(2.0)];
                let e = extremum(&args, name, is_max, nargout).unwrap_err().msg;
                assert_eq!(
                    e,
                    format!("{name} takes two arrays or one array and a dimension, not both.")
                );
            }
            let nd = val(nd24());
            let e = extremum(&[nd.clone(), nd, dim_arg(3.0)], name, is_max, 1);
            assert!(e.unwrap_err().msg.starts_with(name));
            let placeholder = val(Matrix::new(1, 0, Vec::new()));
            let m = out(extremum(
                &[val(x.clone()), placeholder, dim_arg(2.0)],
                name,
                is_max,
                1,
            ));
            assert_eq!(m.data, [if is_max { 5.0 } else { 1.0 }]);
        }
    }

    #[test]
    fn the_element_wise_math_keeps_every_dimension() {
        let a = nd24();
        let dims = |m: Matrix| m.dims();
        assert_eq!(
            dims(out(unary(&[val(a.clone())], "floor", f64::floor))),
            [2, 3, 4]
        );
        assert_eq!(
            dims(out(complex_unary(&[val(a.clone())], "sqrt", C::sqrt))),
            [2, 3, 4]
        );
        assert_eq!(
            dims(out(round(&[val(a.clone()), val(Matrix::scalar(1.0))]))),
            [2, 3, 4]
        );
        let n = out(mask(&[val(a.clone())], "isnan", f64::is_nan));
        assert_eq!((n.dims(), n.class), (vec![2, 3, 4], Class::Logical));
        let r = out(binary(
            &[val(a.clone()), val(Matrix::scalar(5.0))],
            "mod",
            modulo,
        ));
        assert_eq!(r.dims(), [2, 3, 4]);
        assert_eq!(r.data[..6], [1.0, 2.0, 3.0, 4.0, 0.0, 1.0]);
        let mut z = a.clone();
        z.im = Some(a.data.iter().map(|x| -x).collect());
        let m = out(abs(&[val(z.clone())]));
        assert_eq!(m.dims(), [2, 3, 4]);
        assert!(near(m.data[23], 24.0 * 2f64.sqrt()));
        let e = out(complex_unary(&[val(z)], "exp", C::exp));
        assert_eq!(e.dims(), [2, 3, 4]);
        let h = out(binary(
            &[
                val(Matrix::filled_dims(&[1, 1, 2], 3.0)),
                val(Matrix::col(vec![4.0, 4.0])),
            ],
            "hypot",
            f64::hypot,
        ));
        assert_eq!((h.dims(), h.data), (vec![2, 1, 2], vec![5.0; 4]));
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
