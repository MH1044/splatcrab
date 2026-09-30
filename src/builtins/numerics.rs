//! Numerics on data (cycle 09): polynomials, interpolation, integration and
//! differences of samples, filtering, statistics, number theory, and the
//! grid constructors `logspace` and `meshgrid` with the counter `histc`.
//!
//! The solvers that call a user function are in `solvers.rs` and the set
//! functions in `sets.rs`. Every result shape computed from an operand's
//! goes through `check_shape` before it is allocated, directly or through
//! [`map_slices`] and `math::reduce`.
//!
//! A function that works along a dimension of a matrix takes MATLAB's
//! default, the first dimension that is not a singleton ([`first_dim`]), so
//! a matrix is worked on column by column and a row vector along its length.

use std::f64::consts::PI;

use super::args::{at_most, check_shape, dim, mat, need, option, scalar};
use super::complex::C;
use super::factor::{self, qr_iterations};
use super::math::{reduce, sum0};
use super::{Registry, add, one_as, one_mat};
use crate::error;
use crate::interp::{Interp, R};
use crate::value::{Class, Matrix, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    // ---- polynomials -------------------------------------------------
    add(r, "polyfit", polyfit, "p = polyfit(x,y,n) - least-squares polynomial of degree n, highest power first.");
    add(r, "polyval", polyval, "polyval(p,x) - the polynomial p evaluated at each element of x.");
    add(r, "roots", roots, "roots(p) - the roots of the polynomial p, a column; real roots only until cycle 10.");
    add(r, "conv", conv, "conv(u,v), conv(u,v,shape) - convolution of vectors, or the product of polynomials.");
    add(r, "deconv", deconv, "[q,r] = deconv(b,a) - polynomial division, b = conv(a,q) + r.");
    add(r, "filter", filter, "filter(b,a,x) - the rational transfer function b/a applied to x, along its first non-singleton dimension.");

    // ---- samples -----------------------------------------------------
    add(r, "interp1", interp1, "interp1(x,v,xq), interp1(v,xq), interp1(...,method,extrap) - 1-D interpolation: 'linear', 'nearest', 'previous', 'next'.");
    add(r, "trapz", trapz, "trapz(y), trapz(x,y), trapz(y,dim), trapz(x,y,dim) - trapezoidal integration.");
    add(r, "cumtrapz", cumtrapz, "cumtrapz(y), cumtrapz(x,y), cumtrapz(...,dim) - cumulative trapezoidal integration.");
    add(r, "diff", diff, "diff(X), diff(X,n), diff(X,n,dim) - differences between adjacent elements, n times.");

    // ---- statistics --------------------------------------------------
    add(r, "std", std, "std(X), std(X,w), std(X,w,dim) - standard deviation; w = 1 normalises by N, 0 by N-1.");
    add(r, "var", var, "var(X), var(X,w), var(X,w,dim) - variance; w = 1 normalises by N, 0 by N-1.");
    add(r, "median", median, "median(X), median(X,dim) - the middle value; NaN if any element is NaN.");
    add(r, "mode", mode, "mode(X), mode(X,dim), [M,F] = mode(...) - the most frequent value, the smallest of a tie.");

    // ---- number theory -----------------------------------------------
    add(r, "factorial", factorial, "factorial(n) - the product 1*2*...*n of each element.");
    add(r, "nchoosek", nchoosek, "nchoosek(n,k) - the binomial coefficient; nchoosek(v,k) - every combination of k elements of v, one per row.");
    add(r, "primes", primes, "primes(n) - the prime numbers up to n, a row.");
    add(r, "isprime", isprime, "isprime(X) - true where X is a prime number.");
    add(r, "gcd", gcd, "gcd(A,B) - greatest common divisor, element-wise.");
    add(r, "lcm", lcm, "lcm(A,B) - least common multiple, element-wise.");

    // ---- grids and counts --------------------------------------------
    add(r, "logspace", logspace, "logspace(a,b,n) - n points from 10^a to 10^b, evenly spaced in the exponent (n = 50).");
    add(r, "meshgrid", meshgrid, "[X,Y] = meshgrid(x,y), meshgrid(x) - 2-D grid coordinates.");
    add(r, "histc", histc, "[n,bin] = histc(x,edges) - counts of x in the bins edges(k) <= x < edges(k+1), the last bin x == edges(end).");
}

// ---- shared helpers --------------------------------------------------

/// Argument `i` as a vector, or an empty.
fn vector(args: &[Value], i: usize, name: &str) -> R<Matrix> {
    let m = mat(args, i, name)?;
    if !m.is_vector() && !m.is_empty() {
        return Err(error::arg_not_a_vector(i + 1, name));
    }
    Ok(m)
}

/// Argument `i` as a non-negative integer scalar: a degree, an order or a
/// count. A char is never one. A huge value saturates, which only means a
/// loop that stops early.
fn count(args: &[Value], i: usize, name: &str) -> R<usize> {
    if args.get(i).is_some_and(Value::is_char) {
        return Err(error::arg_nonneg_int(i + 1, name));
    }
    let v = scalar(args, i, name)?;
    if !is_count(v) {
        return Err(error::arg_nonneg_int(i + 1, name));
    }
    Ok(if v >= usize::MAX as f64 {
        usize::MAX
    } else {
        v as usize
    })
}

fn is_count(v: f64) -> bool {
    v.is_finite() && v >= 0.0 && v.fract() == 0.0
}

/// MATLAB's default working dimension: the first that is not a singleton,
/// and `1` when every dimension is one. It is the reductions' rule,
/// `math::default_dim`, since cycle 14b, so the two cannot disagree.
pub fn first_dim(m: &Matrix) -> usize {
    super::math::default_dim(&m.dims())
}

/// How many elements `m` has along dimension `d`.
pub fn extent(m: &Matrix, d: usize) -> usize {
    match d {
        1 => m.rows,
        2 => m.cols,
        _ => 1,
    }
}

/// `f` applied to every slice of `m` along dimension `d`, a slice being the
/// elements that vary along `d`, in order. Every slice comes back as `out`
/// values, which take the place of the extent of `d` in the result's shape,
/// judged by `check_shape` first. Along a dimension past the second each
/// element is a slice of one, and `out` must be `1` there: anything else is
/// an N-D shape.
pub fn map_slices(m: &Matrix, d: usize, out: usize, f: impl Fn(&[f64]) -> Vec<f64>) -> R<Matrix> {
    match d {
        1 => {
            let (r, c) = check_shape(out as f64, m.cols as f64)?;
            if r * c == 0 {
                // Either there are no slices or each gives nothing: a
                // 0x1e12 argument must not visit 1e12 of them (cycle 13b).
                return Ok(Matrix::new(r, c, Vec::new()));
            }
            let mut data = Vec::with_capacity(r * c);
            for j in 0..m.cols {
                let s = f(&m.data[j * m.rows..(j + 1) * m.rows]);
                debug_assert_eq!(s.len(), out);
                data.extend(s);
            }
            Ok(Matrix::new(r, c, data))
        }
        2 => Ok(map_slices(&m.transpose(), 1, out, f)?.transpose()),
        _ if out == 1 => Ok(Matrix::new(
            m.rows,
            m.cols,
            m.data.iter().map(|&x| f(&[x])[0]).collect(),
        )),
        _ => Err(error::nd_unsupported()),
    }
}

/// A reduction along `d`, or along the default dimension when `d` is
/// `None`. Unlike `math::reduce`, a dimension past `ndims` reduces each
/// element on its own rather than handing the argument back, which is what
/// makes `var(X, 0, 3)` zeros; a dimension within `ndims` is
/// `math::reduce`'s, whose rule for a dimension of size 1 is the same.
fn reduce_along(m: &Matrix, d: Option<usize>, f: impl Fn(&[f64]) -> f64) -> R<Matrix> {
    match d {
        Some(d) if d > m.ndims() => Ok(m.map(|x| f(&[x]))),
        _ => reduce(m, d, f),
    }
}

/// A row or a column of `data`, as `column` says.
fn oriented(data: Vec<f64>, column: bool) -> Matrix {
    if column {
        Matrix::col(data)
    } else {
        Matrix::row(data)
    }
}

fn is_column(m: &Matrix) -> bool {
    m.cols == 1 && m.rows != 1
}

// ---- polynomials -----------------------------------------------------

/// `polyfit(x, y, n)`: the coefficients, highest power first, of the
/// polynomial of degree `n` that fits the points in the least-squares
/// sense, a row. The Vandermonde system is solved by `factor::lstsq`, the
/// column-pivoted QR of cycle 08; a rank-deficient one (fewer distinct
/// points than coefficients) warns with that solver's warning and returns
/// its basic solution. Only the first output is given: `[p, S, mu]` is not
/// provided.
fn polyfit(it: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "polyfit")?;
    need(a, 3, "polyfit")?;
    let x = vector(a, 0, "polyfit")?;
    let y = vector(a, 1, "polyfit")?;
    if x.numel() != y.numel() {
        return Err(error::sample_length("polyfit"));
    }
    let n = count(a, 2, "polyfit")?;
    let m = x.numel();
    let (_, cols) = check_shape(m as f64, n as f64 + 1.0)?;
    let mut v = factor::zeros(m, cols)?;
    // The last column is x^0, and each column to its left one power higher.
    for i in 0..m {
        let mut p = 1.0;
        for j in (0..cols).rev() {
            v.set(i, j, p);
            p *= x.data[i];
        }
    }
    let (p, rank) = factor::lstsq(&v, &Matrix::col(y.data.clone()))?;
    if rank < cols {
        it.warn(Some(error::rank_deficient_warning(rank)))?;
    }
    one_mat(Matrix::row(p.data))
}

/// `polyval(p, x)` by Horner's rule, element-wise, in the shape of `x`. An
/// empty `p` is the zero polynomial.
fn polyval(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "polyval")?;
    need(a, 2, "polyval")?;
    let p = vector(a, 0, "polyval")?;
    let x = mat(a, 1, "polyval")?;
    one_mat(x.map(|t| horner(&p.data, t)))
}

/// `p` at `t`. The first coefficient seeds the sum, so a leading term is
/// never multiplied by a zero: `polyval([1 0], Inf)` is `Inf`, not `NaN`.
pub fn horner(p: &[f64], t: f64) -> f64 {
    match p.split_first() {
        None => 0.0,
        Some((&first, rest)) => rest.iter().fold(first, |acc, &c| acc * t + c),
    }
}

/// `roots(p)`: the eigenvalues of the companion matrix, from the general
/// eigensolver of cycle 08, as a column, followed by a `0` for every
/// trailing zero coefficient. Leading zeros are dropped. A constant or an
/// empty `p` has no roots, `0x1`. A complex pair of roots is a pair of
/// complex values (cycle 10; a refusal before it), and the column is
/// stored by the flag rule, so real roots stay real. A `NaN` or `Inf`
/// coefficient is refused as `eig` refuses one, and complex coefficients
/// by the registry's gate.
fn roots(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "roots")?;
    let p = vector(a, 0, "roots")?;
    if !factor::all_finite(&p) {
        return Err(error::nonfinite_input("roots"));
    }
    let c = &p.data;
    let (Some(first), Some(last)) = (
        c.iter().position(|&v| v != 0.0),
        c.iter().rposition(|&v| v != 0.0),
    ) else {
        return one_mat(Matrix::new(0, 1, Vec::new()));
    };
    let core = &c[first..=last];
    let n = core.len() - 1;
    let mut r: Vec<C> = Vec::with_capacity(n + c.len() - 1 - last);
    if n >= 1 {
        let mut comp = factor::zeros(n, n)?;
        for j in 0..n {
            comp.set(0, j, -core[j + 1] / core[0]);
        }
        for i in 1..n {
            comp.set(i, i - 1, 1.0);
        }
        let (vals, _) = factor::eig_general(&comp, qr_iterations(n))?;
        r.extend(vals);
    }
    r.extend(std::iter::repeat_n(C::real(0.0), c.len() - 1 - last));
    let len = r.len();
    one_mat(Matrix::from_c(len, 1, r))
}

/// The full convolution of `u` and `v`, `u.len() + v.len() - 1` values, or
/// none when either is empty.
pub fn convolve(u: &[f64], v: &[f64]) -> Vec<f64> {
    if u.is_empty() || v.is_empty() {
        return Vec::new();
    }
    let mut c = vec![0.0; u.len() + v.len() - 1];
    for (i, &x) in u.iter().enumerate() {
        for (j, &y) in v.iter().enumerate() {
            c[i + j] += x * y;
        }
    }
    c
}

/// `conv(u, v)`, `conv(u, v, 'full')`, `'same'` (the central part, as long
/// as `u`) and `'valid'` (only the parts computed without padding). The
/// result is a column when `u` is one, or when `u` is a scalar and `v` a
/// column, and a row otherwise; an empty operand gives `[]` for `'full'`.
fn conv(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "conv")?;
    need(a, 2, "conv")?;
    let u = vector(a, 0, "conv")?;
    let v = vector(a, 1, "conv")?;
    let shape = match a.get(2) {
        None => "full".to_string(),
        Some(_) => match option(a, 2) {
            Some(s)
                if ["full", "same", "valid"]
                    .iter()
                    .any(|t| s.eq_ignore_ascii_case(t)) =>
            {
                s.to_ascii_lowercase()
            }
            _ => return Err(error::conv_shape()),
        },
    };
    let (m, n) = (u.numel(), v.numel());
    let full = convolve(&u.data, &v.data);
    let data = match shape.as_str() {
        "same" => {
            let k = n / 2;
            (0..m)
                .map(|i| full.get(i + k).copied().unwrap_or(0.0))
                .collect()
        }
        "valid" if n > 0 && m >= n => full[n - 1..m].to_vec(),
        "valid" => Vec::new(),
        _ if full.is_empty() => return one_mat(Matrix::empty()),
        _ => full,
    };
    let column = if m > 1 { is_column(&u) } else { is_column(&v) };
    one_mat(oriented(data, column))
}

/// `[q, r] = deconv(b, a)`: long division of the polynomial `b` by `a`, so
/// that `b = conv(a, q) + r`. `q` has `length(b) - length(a) + 1`
/// coefficients, or is `0` when `a` is the longer; `r` has the shape of `b`,
/// its leading `length(q)` coefficients exactly zero. `q` is a column when
/// `b` is one.
fn deconv(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 2, "deconv")?;
    need(a, 2, "deconv")?;
    let b = vector(a, 0, "deconv")?;
    let d = vector(a, 1, "deconv")?;
    let d0 = match d.data.first() {
        Some(&v) if v != 0.0 => v,
        _ => return Err(error::leading_zero("deconv")),
    };
    let (nb, na) = (b.numel(), d.numel());
    let mut r = b.data.clone();
    let q = if na > nb {
        vec![0.0]
    } else {
        let mut q = vec![0.0; nb - na + 1];
        for k in 0..q.len() {
            q[k] = r[k] / d0;
            for (j, &dj) in d.data.iter().enumerate() {
                r[k + j] -= q[k] * dj;
            }
            r[k] = 0.0;
        }
        q
    };
    let q = oriented(q, is_column(&b));
    let r = Matrix::new(b.rows, b.cols, r);
    if nargout < 2 {
        return one_mat(q);
    }
    Ok(vec![Value::Mat(q), Value::Mat(r)])
}

/// `filter(b, a, x)`: the difference equation
/// `a(1) y(n) = b(1) x(n) + ... - a(2) y(n-1) - ...`, as the direct form II
/// transposed, along the first non-singleton dimension of `x`, from zero
/// initial conditions. Both coefficient vectors are normalised by `a(1)`,
/// which must be non-zero. The initial and final conditions, `zi` and `zf`,
/// and a dimension argument are not provided.
fn filter(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "filter")?;
    need(a, 3, "filter")?;
    let b = vector(a, 0, "filter")?;
    let den = vector(a, 1, "filter")?;
    let x = mat(a, 2, "filter")?;
    let a0 = match den.data.first() {
        Some(&v) if v != 0.0 => v,
        _ => return Err(error::leading_zero("filter")),
    };
    let n = b.numel().max(den.numel());
    let pad = |c: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|k| c.get(k).copied().unwrap_or(0.0) / a0)
            .collect()
    };
    let (bb, aa) = (pad(&b.data), pad(&den.data));
    let d = first_dim(&x);
    one_mat(map_slices(&x, d, extent(&x, d), |s| df2t(&bb, &aa, s))?)
}

/// The direct form II transposed over one sequence, with `b` and `a` of one
/// length and `a[0]` already `1`. The state `z` has one element more than it
/// needs, always zero, so every update has the same form.
pub fn df2t(b: &[f64], a: &[f64], x: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut z = vec![0.0; n];
    x.iter()
        .map(|&xi| {
            let yi = b[0] * xi + z[0];
            for i in 1..n {
                z[i - 1] = b[i] * xi + z[i] - a[i] * yi;
            }
            yi
        })
        .collect()
}

// ---- samples ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Method {
    Linear,
    Nearest,
    Previous,
    Next,
}

/// What `interp1` gives for a query outside the sample points.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Extrap {
    /// The default: `NaN`.
    Nan,
    /// `'extrap'`: the end interval's line for `'linear'`, and the nearest
    /// end value for the other methods.
    Extrap,
    /// A scalar given in place of `'extrap'`.
    Value(f64),
}

/// `interp1(x, v, xq)`, `interp1(v, xq)` (with `x = 1:n`), then optionally a
/// method (`'linear'`, the default, `'nearest'`, `'previous'` or `'next'`;
/// `'pchip'`, `'spline'` and the rest are refused) and an extrapolation
/// (`'extrap'` or a scalar; `NaN` outside the sample points otherwise).
///
/// The sample points need not be sorted but must be distinct and not
/// `NaN`, at least two of them. A vector `v` gives a result the shape of
/// `xq`; a matrix `v` is one set of values per column, `length(x)` rows,
/// and gives one column per column of `v`, one row per element of a vector
/// `xq`. A query exactly on a sample point returns that point's value. A
/// `'nearest'` query halfway between two points takes the upper one.
fn interp1(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 5, "interp1")?;
    need(a, 2, "interp1")?;
    let nums = a.iter().take(3).take_while(|v| !v.is_char()).count();
    let (x, v, xq) = match nums {
        2 => (None, mat(a, 0, "interp1")?, mat(a, 1, "interp1")?),
        3 => (
            Some(vector(a, 0, "interp1")?),
            mat(a, 1, "interp1")?,
            mat(a, 2, "interp1")?,
        ),
        _ => return Err(error::not_enough_args("interp1")),
    };
    let method = match a.get(nums) {
        None => Method::Linear,
        Some(_) => match option(a, nums).map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("linear") => Method::Linear,
            Some("nearest") => Method::Nearest,
            Some("previous") => Method::Previous,
            Some("next") => Method::Next,
            _ => return Err(error::interp_method()),
        },
    };
    let extrap = match a.get(nums + 1) {
        None => Extrap::Nan,
        Some(val) => match (option(a, nums + 1), val.mat()?.scalar_value()) {
            (Some(s), _) if s.eq_ignore_ascii_case("extrap") => Extrap::Extrap,
            (None, Some(e)) => Extrap::Value(e),
            _ => return Err(error::interp_extrap()),
        },
    };
    if a.len() > nums + 2 {
        return Err(error::too_many_args());
    }
    let (n, sets) = if v.is_vector() {
        (v.numel(), 1)
    } else {
        (v.rows, v.cols)
    };
    let xs: Vec<f64> = match &x {
        None => (1..=n).map(|k| k as f64).collect(),
        Some(x) if x.numel() != n => return Err(error::sample_length("interp1")),
        Some(x) => x.data.clone(),
    };
    if n < 2 || xs.iter().any(|t| t.is_nan()) {
        return Err(error::interp_points());
    }
    let mut perm: Vec<usize> = (0..n).collect();
    perm.sort_by(|&i, &j| xs[i].total_cmp(&xs[j]));
    let sorted: Vec<f64> = perm.iter().map(|&k| xs[k]).collect();
    if sorted.windows(2).any(|w| w[0] >= w[1]) {
        return Err(error::interp_points());
    }
    let column = |c: usize| -> Vec<f64> { perm.iter().map(|&k| v.data[c * n + k]).collect() };
    if sets == 1 {
        let vs = column(0);
        return one_mat(xq.map(|q| interp_at(&sorted, &vs, q, method, extrap)));
    }
    if !xq.is_vector() && !xq.is_empty() {
        return Err(error::nd_unsupported());
    }
    let rows = xq.numel();
    let (r, c) = check_shape(rows as f64, sets as f64)?;
    let mut out = Matrix::filled(r, c, 0.0);
    for s in 0..sets {
        let vs = column(s);
        for (i, &q) in xq.data.iter().enumerate() {
            out.set(i, s, interp_at(&sorted, &vs, q, method, extrap));
        }
    }
    one_mat(out)
}

/// One interpolated value: `xs` sorted and distinct, at least two, `vs` the
/// values at them.
pub fn interp_at(xs: &[f64], vs: &[f64], q: f64, method: Method, extrap: Extrap) -> f64 {
    let n = xs.len();
    if q.is_nan() {
        return f64::NAN;
    }
    let outside = q < xs[0] || q > xs[n - 1];
    if outside {
        match extrap {
            Extrap::Nan => return f64::NAN,
            Extrap::Value(e) => return e,
            Extrap::Extrap if method != Method::Linear => {
                return if q < xs[0] { vs[0] } else { vs[n - 1] };
            }
            Extrap::Extrap => {}
        }
    }
    // The interval `xs[k]..xs[k + 1]` that holds `q`, the first or the last
    // one for a query outside.
    let k = xs.partition_point(|&t| t <= q).saturating_sub(1).min(n - 2);
    if !outside {
        if xs[k] == q {
            return vs[k];
        }
        if xs[k + 1] == q {
            return vs[k + 1];
        }
    }
    match method {
        Method::Linear => vs[k] + (vs[k + 1] - vs[k]) * (q - xs[k]) / (xs[k + 1] - xs[k]),
        Method::Nearest => {
            if q - xs[k] < xs[k + 1] - q {
                vs[k]
            } else {
                vs[k + 1]
            }
        }
        Method::Previous => vs[k],
        Method::Next => vs[k + 1],
    }
}

/// The spacing `trapz` and `cumtrapz` integrate over.
enum Spacing {
    Unit,
    Uniform(f64),
    Points(Vec<f64>),
}

impl Spacing {
    /// The width of the `k`-th interval.
    fn width(&self, k: usize) -> f64 {
        match self {
            Spacing::Unit => 1.0,
            Spacing::Uniform(h) => *h,
            Spacing::Points(x) => x[k + 1] - x[k],
        }
    }
}

/// The arguments of `trapz` and `cumtrapz`: `(y)`, `(x, y)`, `(y, dim)` and
/// `(x, y, dim)`. Two arguments are `(y, dim)` when the second is a scalar,
/// MATLAB's rule. `x` is a scalar spacing, or one point per element of `y`
/// along the dimension.
fn trapz_args(a: &[Value], name: &str) -> R<(Spacing, Matrix, usize)> {
    at_most(a, 3, name)?;
    need(a, 1, name)?;
    let second_scalar = a.len() == 2 && a[1].mat().is_ok_and(Matrix::is_scalar);
    let (x, y, d) = match a.len() {
        1 => {
            let y = mat(a, 0, name)?;
            let d = first_dim(&y);
            (None, y, d)
        }
        2 if second_scalar => (None, mat(a, 0, name)?, dim(a, 1, name)?),
        2 => {
            let y = mat(a, 1, name)?;
            let d = first_dim(&y);
            (Some(vector(a, 0, name)?), y, d)
        }
        _ => (
            Some(vector(a, 0, name)?),
            mat(a, 1, name)?,
            dim(a, 2, name)?,
        ),
    };
    let spacing = match x {
        None => Spacing::Unit,
        Some(x) if x.is_scalar() => Spacing::Uniform(x.data[0]),
        Some(x) if x.numel() == extent(&y, d) => Spacing::Points(x.data),
        Some(_) => return Err(error::sample_length(name)),
    };
    Ok((spacing, y, d))
}

/// `trapz`: the trapezoidal rule along the first non-singleton dimension of
/// `y`, or along `dim`, which becomes `1` in the result. Fewer than two
/// samples integrate to `0`.
fn trapz(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    let (h, y, d) = trapz_args(a, "trapz")?;
    let f = |s: &[f64]| {
        let terms: Vec<f64> = (1..s.len())
            .map(|k| h.width(k - 1) * (s[k - 1] + s[k]) / 2.0)
            .collect();
        vec![sum0(&terms)]
    };
    one_mat(map_slices(&y, d, 1, f)?)
}

/// `cumtrapz`: the running trapezoidal integral, the shape of `y`, starting
/// from `0`.
fn cumtrapz(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    let (h, y, d) = trapz_args(a, "cumtrapz")?;
    let f = |s: &[f64]| {
        let mut acc = 0.0;
        let mut out = Vec::with_capacity(s.len());
        for k in 0..s.len() {
            if k > 0 {
                acc += h.width(k - 1) * (s[k - 1] + s[k]) / 2.0;
            }
            out.push(acc);
        }
        out
    };
    one_mat(map_slices(&y, d, extent(&y, d), f)?)
}

/// `n` rounds of adjacent differences of `s`: `s.len() - n` values, or none.
pub fn differences(s: &[f64], n: usize) -> Vec<f64> {
    let mut v = s.to_vec();
    for _ in 0..n.min(s.len()) {
        v = v.windows(2).map(|w| w[1] - w[0]).collect();
    }
    v
}

/// `diff(X)`, `diff(X, n)` and `diff(X, n, dim)`. Without `dim`, each of
/// the `n` rounds works along the first non-singleton dimension of what the
/// round before left, so `diff([1 2; 4 8], 2)` differences the column
/// differences along the row they form. With `dim`, all `n` rounds work
/// along it, and the result has `max(size(X, dim) - n, 0)` elements there.
fn diff(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "diff")?;
    need(a, 1, "diff")?;
    let mut m = mat(a, 0, "diff")?.with_class(Class::Double);
    let n = if a.len() >= 2 {
        count(a, 1, "diff")?
    } else {
        1
    };
    if a.len() >= 3 {
        let d = dim(a, 2, "diff")?;
        let out = extent(&m, d).saturating_sub(n);
        return one_mat(map_slices(&m, d, out, |s| differences(s, n))?);
    }
    for _ in 0..n {
        if m.is_empty() {
            break;
        }
        let d = first_dim(&m);
        let out = extent(&m, d) - 1;
        m = map_slices(&m, d, out, |s| differences(s, 1))?;
    }
    one_mat(m)
}

// ---- statistics ------------------------------------------------------

/// The variance of `xs`, normalised by `N` when `by_n` and by `N - 1`
/// otherwise; a single element is normalised by `1`, so its variance is
/// `0`, and no elements give `NaN`. Two passes: the mean, then the squared
/// deviations from it.
pub fn variance(xs: &[f64], by_n: bool) -> f64 {
    let n = xs.len();
    if n == 0 {
        return f64::NAN;
    }
    let mean = sum0(xs) / n as f64;
    let dev: Vec<f64> = xs.iter().map(|x| (x - mean) * (x - mean)).collect();
    let denom = if by_n || n == 1 { n } else { n - 1 };
    sum0(&dev) / denom as f64
}

/// The arguments of `std` and `var`: `(X)`, `(X, w)` and `(X, w, dim)`,
/// where `w` is `0` (or `[]`) for `N - 1` and `1` for `N`. A weight vector
/// is not provided.
fn weighted(a: &[Value], name: &str) -> R<(Matrix, bool, Option<usize>)> {
    at_most(a, 3, name)?;
    need(a, 1, name)?;
    let m = mat(a, 0, name)?;
    let by_n = match a.get(1) {
        None => false,
        Some(v) => {
            let w = v.mat()?;
            match (w.is_empty(), v.is_char(), w.scalar_value()) {
                (true, false, _) => false,
                (false, false, Some(0.0)) => false,
                (false, false, Some(1.0)) => true,
                _ => return Err(error::std_weight(name)),
            }
        }
    };
    let d = if a.len() >= 3 {
        Some(dim(a, 2, name)?)
    } else {
        None
    };
    Ok((m, by_n, d))
}

fn var(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    let (m, by_n, d) = weighted(a, "var")?;
    one_mat(reduce_along(&m, d, |xs| variance(xs, by_n))?)
}

fn std(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    let (m, by_n, d) = weighted(a, "std")?;
    one_mat(reduce_along(&m, d, |xs| variance(xs, by_n).sqrt())?)
}

/// The middle value of `xs`, the mean of the two middle ones for an even
/// count; `NaN` for no elements or when any element is `NaN`, MATLAB's
/// default.
pub fn median_of(xs: &[f64]) -> f64 {
    if xs.is_empty() || xs.iter().any(|x| x.is_nan()) {
        return f64::NAN;
    }
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n % 2 == 1 {
        return s[n / 2];
    }
    let (lo, hi) = (s[n / 2 - 1], s[n / 2]);
    let mid = (lo + hi) / 2.0;
    if mid.is_finite() || !lo.is_finite() || !hi.is_finite() {
        mid
    } else {
        lo / 2.0 + hi / 2.0
    }
}

/// The optional dimension argument of `median` and `mode`.
fn dim_arg(a: &[Value], name: &str) -> R<(Matrix, Option<usize>)> {
    at_most(a, 2, name)?;
    need(a, 1, name)?;
    let m = mat(a, 0, name)?;
    let d = if a.len() >= 2 {
        Some(dim(a, 1, name)?)
    } else {
        None
    };
    Ok((m, d))
}

fn median(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    let (m, d) = dim_arg(a, "median")?;
    one_mat(reduce_along(&m, d, median_of)?)
}

/// The most frequent value of `xs` and how often it occurs, ignoring `NaN`:
/// the smallest of the values tied for most frequent, and `(NaN, 0)` when
/// nothing is left.
pub fn mode_of(xs: &[f64]) -> (f64, usize) {
    let mut s: Vec<f64> = xs.iter().copied().filter(|x| !x.is_nan()).collect();
    if s.is_empty() {
        return (f64::NAN, 0);
    }
    s.sort_by(f64::total_cmp);
    let (mut best, mut best_n) = (s[0], 0);
    let mut k = 0;
    while k < s.len() {
        let mut j = k;
        while j < s.len() && s[j] == s[k] {
            j += 1;
        }
        if j - k > best_n {
            best = s[k];
            best_n = j - k;
        }
        k = j;
    }
    (best, best_n)
}

/// `mode(X)`, `mode(X, dim)`; asked for two outputs, the second is how
/// often each mode occurs.
fn mode(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    let (m, d) = dim_arg(a, "mode")?;
    let value = reduce_along(&m, d, |xs| mode_of(xs).0)?;
    if nargout < 2 {
        return one_mat(value);
    }
    let freq = reduce_along(&m, d, |xs| mode_of(xs).1 as f64)?;
    Ok(vec![Value::Mat(value), Value::Mat(freq)])
}

// ---- number theory ---------------------------------------------------

/// `n!` for a non-negative integer `n`: `Inf` past `170!`, the largest a
/// double holds.
pub fn fact(n: f64) -> f64 {
    if n > 170.0 {
        return f64::INFINITY;
    }
    (2..=n as u32).fold(1.0, |acc, k| acc * k as f64)
}

/// `factorial(N)`, element-wise. `NaN` gives `NaN` and `Inf` gives `Inf`;
/// any other element must be a non-negative integer.
fn factorial(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "factorial")?;
    let m = mat(a, 0, "factorial")?;
    one_mat(m.try_map(|x| {
        if x.is_nan() || x == f64::INFINITY {
            Ok(x)
        } else if is_count(x) {
            Ok(fact(x))
        } else {
            Err(error::arg_nonneg_int(1, "factorial"))
        }
    })?)
}

/// The binomial coefficient `n` choose `k`, `k <= n`, by the multiplicative
/// formula over the smaller of `k` and `n - k`. Every partial product is
/// itself a binomial coefficient, so each step is exact while it stays below
/// 2^53; past that the value is the nearest double the steps reach, and
/// once it is infinite it stays so.
pub fn binomial(n: f64, k: f64) -> f64 {
    let k = k.min(n - k);
    let mut r: f64 = 1.0;
    let mut i = 1.0;
    while i <= k && r.is_finite() {
        r = (r * (n - k + i) / i).round();
        i += 1.0;
    }
    r
}

/// The exact number of `k`-element combinations of `n`, or `None` past
/// `u64`.
fn combinations(n: usize, k: usize) -> Option<u64> {
    let k = k.min(n - k) as u128;
    let n = n as u128;
    let mut r: u128 = 1;
    for i in 1..=k {
        r = r.checked_mul(n - k + i)? / i;
        if r > u64::MAX as u128 {
            return None;
        }
    }
    Some(r as u64)
}

/// `nchoosek(n, k)` of a scalar `n`, the binomial coefficient, and of a
/// vector `v`, every combination of `k` of its elements, one per row, in
/// the lexicographic order of their positions and in `v`'s class. The
/// matrix's shape is judged before it is built.
fn nchoosek(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "nchoosek")?;
    need(a, 2, "nchoosek")?;
    let v = mat(a, 0, "nchoosek")?;
    let k = count(a, 1, "nchoosek")?;
    if v.numel() == 1 {
        let n = v.data[0];
        if !is_count(n) {
            return Err(error::arg_nonneg_int(1, "nchoosek"));
        }
        if k as f64 > n {
            return Err(error::nchoosek_k());
        }
        return one_mat(Matrix::scalar(binomial(n, k as f64)));
    }
    if !v.is_vector() && !v.is_empty() {
        return Err(error::arg_not_a_vector(1, "nchoosek"));
    }
    let n = v.numel();
    if k > n {
        let (r, c) = check_shape(0.0, k as f64)?;
        return one_as(Matrix::new(r, c, Vec::new()).with_class(v.class));
    }
    let total = combinations(n, k).map_or(f64::INFINITY, |c| c as f64);
    let (rows, cols) = check_shape(total, k as f64)?;
    let mut out = Matrix::filled(rows, cols, 0.0).with_class(v.class);
    let mut idx: Vec<usize> = (0..k).collect();
    for r in 0..rows {
        for (c, &i) in idx.iter().enumerate() {
            out.set(r, c, v.data[i]);
        }
        // The rightmost position that can still move, then everything to
        // its right just after it.
        let Some(i) = (0..k).rev().find(|&i| idx[i] < n - k + i) else {
            break;
        };
        idx[i] += 1;
        for j in i + 1..k {
            idx[j] = idx[j - 1] + 1;
        }
    }
    one_as(out)
}

/// `primes(n)`: the primes up to `n`, a row, by the sieve of Eratosthenes.
/// Below `2`, `NaN` included, there are none (`1x0`). The sieve's length is
/// judged by `check_shape` as the row it could become.
fn primes(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "primes")?;
    let n = scalar(a, 0, "primes")?;
    if n.is_nan() || n < 2.0 {
        return one_mat(Matrix::new(1, 0, Vec::new()));
    }
    let (_, lim) = check_shape(1.0, n.floor())?;
    one_mat(Matrix::row(
        sieve(lim).into_iter().map(|p| p as f64).collect(),
    ))
}

/// Every prime up to `lim`.
pub fn sieve(lim: usize) -> Vec<usize> {
    let mut composite = vec![false; lim + 1];
    let mut out = Vec::new();
    for p in 2..=lim {
        if composite[p] {
            continue;
        }
        out.push(p);
        let mut q = p.saturating_mul(p);
        while q <= lim {
            composite[q] = true;
            q += p;
        }
    }
    out
}

fn mul_mod(a: u64, b: u64, m: u64) -> u64 {
    ((a as u128 * b as u128) % m as u128) as u64
}

fn pow_mod(mut b: u64, mut e: u64, m: u64) -> u64 {
    let mut r = 1 % m;
    b %= m;
    while e > 0 {
        if e & 1 == 1 {
            r = mul_mod(r, b, m);
        }
        b = mul_mod(b, b, m);
        e >>= 1;
    }
    r
}

/// Whether `n` is prime: trial division by the first twelve primes, then
/// Miller-Rabin with the same twelve bases, which is deterministic for
/// every `n` below 3.3e24 and so for every `u64`.
pub fn is_prime(n: u64) -> bool {
    const BASES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    if n < 2 {
        return false;
    }
    for p in BASES {
        if n % p == 0 {
            return n == p;
        }
    }
    let (mut d, mut s) = (n - 1, 0);
    while d % 2 == 0 {
        d /= 2;
        s += 1;
    }
    'bases: for a in BASES {
        let mut x = pow_mod(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul_mod(x, x, n);
            if x == n - 1 {
                continue 'bases;
            }
        }
        return false;
    }
    true
}

/// `isprime(X)`: a logical of the shape of `X`, whose elements must be
/// non-negative integers. Every double from 2^53 on is even, so none of
/// them is prime.
fn isprime(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "isprime")?;
    let m = mat(a, 0, "isprime")?;
    const EXACT: f64 = 9_007_199_254_740_992.0; // 2^53
    let out = m.try_map(|x| {
        if !is_count(x) {
            return Err(error::arg_nonneg_int(1, "isprime"));
        }
        Ok((x < EXACT && is_prime(x as u64)) as u8 as f64)
    })?;
    one_as(out.with_class(Class::Logical))
}

/// The greatest common divisor of two integers, non-negative, by Euclid's
/// algorithm on doubles, whose remainder is exact.
pub fn gcd_of(a: f64, b: f64) -> f64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0.0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// The least common multiple of two integers, non-negative; `0` when either
/// is `0`.
pub fn lcm_of(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 {
        return 0.0;
    }
    (a / gcd_of(a, b) * b).abs()
}

/// `gcd` and `lcm`, element-wise with broadcasting. Every element must be a
/// finite integer, of either sign.
fn gcd_lcm(a: &[Value], name: &str, f: fn(f64, f64) -> f64) -> R<Vec<Value>> {
    at_most(a, 2, name)?;
    need(a, 2, name)?;
    let x = mat(a, 0, name)?;
    let y = mat(a, 1, name)?;
    for (pos, m) in [(1, &x), (2, &y)] {
        if m.data.iter().any(|v| !v.is_finite() || v.fract() != 0.0) {
            return Err(error::arg_integers(pos, name));
        }
    }
    one_mat(x.zip(&y, name, f)?)
}

fn gcd(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    gcd_lcm(a, "gcd", gcd_of)
}

fn lcm(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    gcd_lcm(a, "lcm", lcm_of)
}

// ---- grids and counts ------------------------------------------------

/// `10^p`, exact for an integer `p` up to 22 in magnitude, where both
/// `10^p` and its reciprocal's division are correctly rounded.
pub fn pow10(p: f64) -> f64 {
    if p.fract() == 0.0 && p.abs() <= 22.0 {
        let k = 10f64.powi(p.abs() as i32);
        if p >= 0.0 { k } else { 1.0 / k }
    } else {
        10f64.powf(p)
    }
}

/// `logspace(a, b)` and `logspace(a, b, n)`: `n` points (50 by default)
/// from `10^a` to `10^b`, evenly spaced in the exponent, the exponents
/// placed as `linspace` places them. MATLAB's special case holds: a `b` of
/// exactly `pi` ends the points at `pi` itself rather than at `10^pi`.
fn logspace(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "logspace")?;
    need(a, 2, "logspace")?;
    let lo = scalar(a, 0, "logspace")?;
    let mut hi = scalar(a, 1, "logspace")?;
    let ends_at_pi = hi == PI;
    if ends_at_pi {
        hi = PI.log10();
    }
    let n = if a.len() >= 3 {
        if a[2].is_char() {
            return Err(error::bad_size_arg("logspace"));
        }
        let n = scalar(a, 2, "logspace")?.floor();
        if n.is_nan() { 0.0 } else { n.max(0.0) }
    } else {
        50.0
    };
    let (_, n) = check_shape(1.0, n)?;
    let data = (0..n)
        .map(|k| {
            let p = if k + 1 == n {
                hi
            } else {
                lo + (hi - lo) * k as f64 / (n - 1) as f64
            };
            if k + 1 == n && ends_at_pi {
                PI
            } else {
                pow10(p)
            }
        })
        .collect();
    one_mat(Matrix::row(data))
}

/// `[X, Y] = meshgrid(x, y)` and `meshgrid(x)`, which is `meshgrid(x, x)`:
/// `X` repeats the elements of `x` along each of `numel(y)` rows and `Y`
/// the elements of `y` down each of `numel(x)` columns. A third input would
/// make a 3-D grid, which is the N-D refusal.
fn meshgrid(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(a, 1, "meshgrid")?;
    if a.len() > 2 {
        return Err(error::nd_unsupported());
    }
    let x = mat(a, 0, "meshgrid")?;
    let y = if a.len() == 2 {
        mat(a, 1, "meshgrid")?
    } else {
        x.clone()
    };
    let (r, c) = check_shape(y.numel() as f64, x.numel() as f64)?;
    let mut gx = Matrix::filled(r, c, 0.0);
    for j in 0..c {
        for i in 0..r {
            gx.set(i, j, x.data[j]);
        }
    }
    let mut out = vec![Value::Mat(gx)];
    if nargout >= 2 {
        let mut gy = Matrix::filled(r, c, 0.0);
        for j in 0..c {
            for i in 0..r {
                gy.set(i, j, y.data[i]);
            }
        }
        out.push(Value::Mat(gy));
    }
    Ok(out)
}

/// The zero-based bin of `v` among the sorted `edges`: `k` when
/// `edges[k] <= v < edges[k + 1]`, the last when `v` equals the last edge,
/// and `None` outside them or for `NaN`.
pub fn bin_of(edges: &[f64], v: f64) -> Option<usize> {
    let n = edges.len();
    if n == 0 || v.is_nan() {
        return None;
    }
    let k = edges.partition_point(|&e| e <= v).checked_sub(1)?;
    if k + 1 == n && v != edges[n - 1] {
        return None;
    }
    Some(k)
}

/// `histc(x, edges)`, and `[n, bin] = histc(...)`. For a vector `x` the
/// counts are one per edge, a row when `x` is a row and a column otherwise;
/// for a matrix, one column of counts per column. `bin` is the one-based bin
/// of each element, `0` for one outside every bin, in the shape of `x`. The
/// edges must be non-decreasing, with no `NaN`. A dimension argument is not
/// provided.
fn histc(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 2, "histc")?;
    need(a, 2, "histc")?;
    let x = mat(a, 0, "histc")?;
    let e = vector(a, 1, "histc")?;
    let edges = &e.data;
    if edges.iter().any(|v| v.is_nan()) || edges.windows(2).any(|w| w[0] > w[1]) {
        return Err(error::histc_edges());
    }
    let ne = edges.len();
    let bins: Vec<Option<usize>> = x.data.iter().map(|&v| bin_of(edges, v)).collect();
    let (sets, per) = if x.is_vector() || x.is_empty() {
        (1, x.numel())
    } else {
        (x.cols, x.rows)
    };
    let (r, c) = check_shape(ne as f64, sets as f64)?;
    let mut counts = Matrix::filled(r, c, 0.0);
    for (k, b) in bins.iter().enumerate() {
        if let Some(b) = b {
            let s = k / per.max(1);
            counts.data[s * ne + b] += 1.0;
        }
    }
    if sets == 1 && x.rows == 1 {
        counts = Matrix::row(counts.data);
    }
    if nargout < 2 {
        return one_mat(counts);
    }
    let bin = Matrix::new(
        x.rows,
        x.cols,
        bins.iter()
            .map(|b| b.map_or(0.0, |k| (k + 1) as f64))
            .collect(),
    );
    Ok(vec![Value::Mat(counts), Value::Mat(bin)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    fn row(v: &[f64]) -> Value {
        Value::Mat(Matrix::row(v.to_vec()))
    }

    fn col(v: &[f64]) -> Value {
        Value::Mat(Matrix::col(v.to_vec()))
    }

    fn text(s: &str) -> Value {
        Value::str(s)
    }

    /// A matrix from its rows, for readability.
    fn rows(r: usize, c: usize, row_major: &[f64]) -> Value {
        let mut m = Matrix::filled(r, c, 0.0);
        for (i, v) in row_major.iter().enumerate() {
            m.set(i / c, i % c, *v);
        }
        Value::Mat(m)
    }

    fn outputs(f: crate::builtins::BuiltinFn, args: &[Value], nargout: usize) -> R<Vec<Matrix>> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        f(&mut it, args, nargout)?
            .into_iter()
            .map(Value::into_mat)
            .collect()
    }

    fn call(f: crate::builtins::BuiltinFn, args: &[Value]) -> R<Matrix> {
        Ok(outputs(f, args, 1)?.remove(0))
    }

    fn err(f: crate::builtins::BuiltinFn, args: &[Value]) -> String {
        call(f, args).unwrap_err().msg
    }

    fn near(a: &[f64], b: &[f64], tol: f64) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
    }

    // ---- polynomials -------------------------------------------------

    #[test]
    fn polyfit_recovers_an_exact_polynomial() {
        let p = call(
            polyfit,
            &[row(&[1.0, 2.0, 3.0]), row(&[2.0, 4.0, 6.0]), num(1.0)],
        )
        .unwrap();
        assert_eq!((p.rows, p.cols), (1, 2));
        assert!(near(&p.data, &[2.0, 0.0], 1e-12), "{:?}", p.data);
        // y = 3x^2 - 2x + 1 at five points, fitted by degree 2.
        let x: Vec<f64> = (0..5).map(|k| k as f64 - 1.5).collect();
        let y: Vec<f64> = x.iter().map(|t| 3.0 * t * t - 2.0 * t + 1.0).collect();
        let p = call(polyfit, &[row(&x), col(&y), num(2.0)]).unwrap();
        assert!(near(&p.data, &[3.0, -2.0, 1.0], 1e-10), "{:?}", p.data);
        // A least-squares line through noisy-free symmetric points.
        let p = call(
            polyfit,
            &[row(&[-1.0, 0.0, 1.0]), row(&[1.0, 0.0, 1.0]), num(1.0)],
        )
        .unwrap();
        assert!(near(&p.data, &[0.0, 2.0 / 3.0], 1e-12), "{:?}", p.data);
        assert!(err(polyfit, &[row(&[1.0, 2.0]), row(&[1.0]), num(1.0)]).contains("same length"));
        assert!(
            err(polyfit, &[row(&[1.0, 2.0]), row(&[1.0, 2.0]), num(1.5)])
                .contains("non-negative integer")
        );
    }

    #[test]
    fn polyval_uses_horner_and_keeps_the_shape_of_x() {
        assert_eq!(
            call(polyval, &[row(&[1.0, 0.0, -1.0]), num(3.0)])
                .unwrap()
                .data,
            [8.0]
        );
        let y = call(
            polyval,
            &[row(&[2.0, 1.0]), rows(2, 2, &[0.0, 1.0, 2.0, 3.0])],
        )
        .unwrap();
        assert_eq!((y.rows, y.cols), (2, 2));
        assert_eq!(y.data, [1.0, 5.0, 3.0, 7.0]);
        assert_eq!(horner(&[1.0, 0.0], f64::INFINITY), f64::INFINITY);
        assert_eq!(horner(&[], 3.0), 0.0);
        assert!(err(polyval, &[rows(2, 2, &[1.0; 4]), num(1.0)]).contains("must be a vector"));
    }

    #[test]
    fn roots_are_the_companion_eigenvalues() {
        let mut r = call(roots, &[row(&[1.0, -3.0, 2.0])]).unwrap();
        assert_eq!((r.rows, r.cols), (2, 1));
        r.data.sort_by(f64::total_cmp);
        assert!(near(&r.data, &[1.0, 2.0], 1e-12), "{:?}", r.data);
        // (x - 1)(x + 2)(x - 3) x^2: two trailing zeros, one leading zero.
        let mut r = call(roots, &[row(&[0.0, 1.0, -2.0, -5.0, 6.0, 0.0, 0.0])]).unwrap();
        assert_eq!(r.numel(), 5);
        r.data.sort_by(f64::total_cmp);
        assert!(
            near(&r.data, &[-2.0, 0.0, 0.0, 1.0, 3.0], 1e-10),
            "{:?}",
            r.data
        );
        // Every root makes the polynomial vanish.
        let p = [2.0, -3.0, -11.0, 6.0];
        for x in call(roots, &[row(&p)]).unwrap().data {
            assert!(horner(&p, x).abs() < 1e-9, "{x}");
        }
        let none = call(roots, &[num(5.0)]).unwrap();
        assert_eq!((none.rows, none.cols), (0, 1));
        // A complex pair of roots is the pair since cycle 10.
        let r = call(roots, &[row(&[1.0, 0.0, 1.0])]).unwrap();
        assert!(near(&r.data, &[0.0, 0.0], 1e-15));
        assert!(near(r.im.as_deref().unwrap(), &[1.0, -1.0], 1e-15));
        // Real roots stay stored real.
        assert!(!call(roots, &[row(&[1.0, -3.0, 2.0])]).unwrap().is_complex());
        assert!(err(roots, &[row(&[1.0, f64::NAN])]).contains("NaN or Inf"));
    }

    #[test]
    fn conv_and_deconv_multiply_and_divide_polynomials() {
        assert_eq!(
            call(conv, &[row(&[1.0, 2.0]), row(&[1.0, 3.0])])
                .unwrap()
                .data,
            [1.0, 5.0, 6.0]
        );
        // The MATLAB page's 'same' example.
        let u = row(&[-1.0, 2.0, 3.0, -2.0, 0.0, 1.0, 2.0]);
        let v = row(&[2.0, 4.0, -1.0, 1.0]);
        assert_eq!(
            call(conv, &[u.clone(), v.clone(), text("same")])
                .unwrap()
                .data,
            [15.0, 5.0, -9.0, 7.0, 6.0, 7.0, -1.0]
        );
        assert_eq!(
            call(conv, &[u, v, text("valid")]).unwrap().data,
            [5.0, -9.0, 7.0, 6.0]
        );
        let c = call(conv, &[col(&[1.0, 1.0]), row(&[1.0, 1.0])]).unwrap();
        assert_eq!((c.rows, c.cols), (3, 1));
        assert!(
            err(conv, &[num(1.0), num(1.0), text("middle")]).contains("'full', 'same' or 'valid'")
        );
        let out = outputs(deconv, &[row(&[1.0, 5.0, 6.0]), row(&[1.0, 2.0])], 2).unwrap();
        assert_eq!(out[0].data, [1.0, 3.0]);
        assert_eq!(out[1].data, [0.0, 0.0, 0.0]);
        let out = outputs(deconv, &[row(&[1.0, 5.0, 7.0]), row(&[1.0, 2.0])], 2).unwrap();
        assert_eq!(out[0].data, [1.0, 3.0]);
        assert_eq!(out[1].data, [0.0, 0.0, 1.0]);
        let out = outputs(deconv, &[row(&[3.0]), row(&[1.0, 2.0])], 2).unwrap();
        assert_eq!(
            (out[0].data.clone(), out[1].data.clone()),
            (vec![0.0], vec![3.0])
        );
        assert!(err(deconv, &[row(&[1.0, 2.0]), row(&[0.0, 1.0])]).contains("non-zero"));
    }

    #[test]
    fn filter_is_the_difference_equation() {
        let y = call(
            filter,
            &[num(1.0), row(&[1.0, -0.5]), row(&[1.0, 0.0, 0.0])],
        )
        .unwrap();
        assert_eq!(y.data, [1.0, 0.5, 0.25]);
        let y = call(filter, &[row(&[0.5, 0.5]), num(1.0), row(&[2.0, 4.0, 6.0])]).unwrap();
        assert_eq!(y.data, [1.0, 3.0, 5.0]);
        // a(1) normalises both vectors.
        let y = call(filter, &[num(2.0), row(&[2.0, -1.0]), row(&[1.0, 0.0])]).unwrap();
        assert_eq!(y.data, [1.0, 0.5]);
        // A matrix is filtered column by column.
        let y = call(
            filter,
            &[
                row(&[1.0, 1.0]),
                num(1.0),
                rows(2, 2, &[1.0, 2.0, 3.0, 4.0]),
            ],
        )
        .unwrap();
        assert_eq!(y.data, [1.0, 4.0, 2.0, 6.0]);
        assert!(err(filter, &[num(1.0), row(&[0.0, 1.0]), num(1.0)]).contains("non-zero"));
    }

    // ---- samples -----------------------------------------------------

    #[test]
    fn interp1_is_linear_nearest_previous_or_next() {
        let x = row(&[1.0, 2.0, 3.0]);
        let v = row(&[10.0, 20.0, 30.0]);
        let at = |q: f64, extra: &[Value]| {
            let mut args = vec![x.clone(), v.clone(), num(q)];
            args.extend_from_slice(extra);
            call(interp1, &args).unwrap().data[0]
        };
        assert_eq!(at(2.5, &[]), 25.0);
        assert!(at(0.5, &[]).is_nan());
        assert_eq!(at(2.4, &[text("nearest")]), 20.0);
        assert_eq!(at(2.5, &[text("nearest")]), 30.0);
        assert_eq!(at(2.9, &[text("previous")]), 20.0);
        assert_eq!(at(2.1, &[text("next")]), 30.0);
        assert_eq!(at(3.0, &[text("previous")]), 30.0);
        assert_eq!(at(4.0, &[text("linear"), text("extrap")]), 40.0);
        assert_eq!(at(4.0, &[text("linear"), num(-1.0)]), -1.0);
        assert_eq!(at(0.0, &[text("nearest"), text("extrap")]), 10.0);
        // interp1(v, xq) samples at 1:n, and unsorted points are sorted.
        assert_eq!(call(interp1, &[v.clone(), num(1.5)]).unwrap().data, [15.0]);
        let q = call(
            interp1,
            &[
                row(&[3.0, 1.0, 2.0]),
                row(&[30.0, 10.0, 20.0]),
                row(&[1.25, 2.75]),
            ],
        )
        .unwrap();
        assert!(near(&q.data, &[12.5, 27.5], 1e-12));
        // A matrix of values interpolates each column.
        let m = rows(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let q = call(interp1, &[col(&[1.0, 2.0, 3.0]), m, row(&[1.5, 3.0])]).unwrap();
        assert_eq!((q.rows, q.cols), (2, 2));
        assert_eq!(q.data, [2.0, 5.0, 3.0, 6.0]);
        assert!(err(interp1, &[x.clone(), v.clone(), num(1.0), text("spline")]).contains("Method"));
        assert!(err(interp1, &[row(&[1.0, 1.0, 2.0]), v.clone(), num(1.0)]).contains("distinct"));
        assert!(err(interp1, &[row(&[1.0, 2.0]), v, num(1.0)]).contains("same length"));
    }

    #[test]
    fn trapz_and_cumtrapz_integrate_samples() {
        assert_eq!(call(trapz, &[row(&[1.0, 2.0, 3.0])]).unwrap().data, [4.0]);
        assert_eq!(
            call(trapz, &[row(&[0.0, 1.0, 2.0]), row(&[0.0, 1.0, 4.0])])
                .unwrap()
                .data,
            [3.0]
        );
        assert_eq!(
            call(trapz, &[num(0.5), row(&[1.0, 2.0, 3.0])])
                .unwrap()
                .data,
            [2.0]
        );
        assert_eq!(
            call(cumtrapz, &[row(&[1.0, 2.0, 3.0])]).unwrap().data,
            [0.0, 1.5, 4.0]
        );
        // A matrix integrates each column; along dimension 2, each row.
        let m = rows(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            call(trapz, std::slice::from_ref(&m)).unwrap().data,
            [2.0, 3.0]
        );
        let r = call(trapz, &[m.clone(), num(2.0)]).unwrap();
        assert_eq!((r.rows, r.cols, r.data.clone()), (2, 1, vec![1.5, 3.5]));
        assert_eq!(call(trapz, &[m.clone(), num(3.0)]).unwrap().data, [0.0; 4]);
        assert_eq!(
            call(cumtrapz, std::slice::from_ref(&m)).unwrap().data,
            [0.0, 2.0, 0.0, 3.0]
        );
        // The trapezoidal rule is exact for a line, and converges as h^2.
        let x: Vec<f64> = (0..=100).map(|k| k as f64 / 100.0).collect();
        let y: Vec<f64> = x.iter().map(|t| t * t).collect();
        let q = call(trapz, &[row(&x), row(&y)]).unwrap().data[0];
        assert!((q - 1.0 / 3.0).abs() < 2e-5, "{q}");
        assert!(err(trapz, &[row(&[1.0, 2.0]), row(&[1.0, 2.0, 3.0])]).contains("same length"));
    }

    #[test]
    fn diff_takes_an_order_and_a_dimension() {
        let v = row(&[1.0, 4.0, 9.0, 16.0]);
        assert_eq!(
            call(diff, std::slice::from_ref(&v)).unwrap().data,
            [3.0, 5.0, 7.0]
        );
        assert_eq!(call(diff, &[v.clone(), num(2.0)]).unwrap().data, [2.0, 2.0]);
        let e = call(diff, &[v.clone(), num(5.0)]).unwrap();
        assert!(e.is_empty());
        assert_eq!(
            call(diff, &[v.clone(), num(0.0)]).unwrap().data,
            [1.0, 4.0, 9.0, 16.0]
        );
        let m = rows(2, 2, &[1.0, 2.0, 4.0, 8.0]);
        let d = call(diff, std::slice::from_ref(&m)).unwrap();
        assert_eq!((d.rows, d.cols, d.data.clone()), (1, 2, vec![3.0, 6.0]));
        // The second round works along the row the first one left.
        assert_eq!(call(diff, &[m.clone(), num(2.0)]).unwrap().data, [3.0]);
        let d = call(diff, &[m.clone(), num(1.0), num(2.0)]).unwrap();
        assert_eq!((d.rows, d.cols, d.data.clone()), (2, 1, vec![1.0, 4.0]));
        let d = call(diff, &[m, num(2.0), num(1.0)]).unwrap();
        assert_eq!((d.rows, d.cols), (0, 2));
        assert!(err(diff, &[v, num(-1.0)]).contains("non-negative integer"));
        assert_eq!(differences(&[1.0, 2.0], 3), Vec::<f64>::new());
    }

    // ---- statistics --------------------------------------------------

    #[test]
    fn std_and_var_normalise_by_n_minus_1_or_n() {
        let v = row(&[1.0, 2.0, 3.0, 4.0]);
        let s = call(std, std::slice::from_ref(&v)).unwrap().data[0];
        assert!((s - (5.0f64 / 3.0).sqrt()).abs() < 1e-14);
        assert!((call(var, std::slice::from_ref(&v)).unwrap().data[0] - 5.0 / 3.0).abs() < 1e-14);
        assert!((call(var, &[v.clone(), num(1.0)]).unwrap().data[0] - 1.25).abs() < 1e-14);
        assert_eq!(call(var, &[num(7.0)]).unwrap().data, [0.0]);
        assert!(call(var, &[Value::Mat(Matrix::empty())]).unwrap().data[0].is_nan());
        let m = rows(2, 2, &[1.0, 2.0, 3.0, 6.0]);
        assert_eq!(
            call(var, std::slice::from_ref(&m)).unwrap().data,
            [2.0, 8.0]
        );
        let r = call(var, &[m.clone(), Value::Mat(Matrix::empty()), num(2.0)]).unwrap();
        assert_eq!((r.rows, r.cols, r.data.clone()), (2, 1, vec![0.5, 4.5]));
        assert_eq!(call(std, &[m, num(0.0), num(3.0)]).unwrap().data, [0.0; 4]);
        // The two-pass variance does not cancel catastrophically.
        let big: Vec<f64> = [4.0, 7.0, 13.0, 16.0].iter().map(|x| x + 1e9).collect();
        assert!((variance(&big, false) - 30.0).abs() < 1e-6);
        assert!(err(std, &[v, num(2.0)]).contains("Weight"));
    }

    #[test]
    fn median_and_mode() {
        assert_eq!(call(median, &[row(&[3.0, 1.0, 2.0])]).unwrap().data, [2.0]);
        assert_eq!(
            call(median, &[row(&[4.0, 1.0, 3.0, 2.0])]).unwrap().data,
            [2.5]
        );
        assert!(call(median, &[row(&[1.0, f64::NAN])]).unwrap().data[0].is_nan());
        assert!(median_of(&[]).is_nan());
        assert_eq!(median_of(&[f64::MAX, f64::MAX]), f64::MAX);
        let m = rows(3, 2, &[1.0, 9.0, 5.0, 3.0, 2.0, 4.0]);
        assert_eq!(
            call(median, std::slice::from_ref(&m)).unwrap().data,
            [2.0, 4.0]
        );
        assert_eq!(
            call(mode, &[row(&[1.0, 2.0, 2.0, 3.0])]).unwrap().data,
            [2.0]
        );
        // A tie goes to the smallest value; NaN is ignored.
        let out = outputs(
            mode,
            &[row(&[3.0, 1.0, 3.0, 1.0, f64::NAN, f64::NAN, f64::NAN])],
            2,
        )
        .unwrap();
        assert_eq!((out[0].data[0], out[1].data[0]), (1.0, 2.0));
        assert_eq!(mode_of(&[]).1, 0);
        assert!(mode_of(&[f64::NAN]).0.is_nan());
    }

    // ---- number theory -----------------------------------------------

    #[test]
    fn factorial_nchoosek_primes_and_isprime() {
        assert_eq!(
            call(factorial, &[row(&[0.0, 1.0, 5.0, 10.0])])
                .unwrap()
                .data,
            [1.0, 1.0, 120.0, 3628800.0]
        );
        assert!(fact(170.0).is_finite());
        assert_eq!(fact(171.0), f64::INFINITY);
        assert!((fact(20.0) - 2432902008176640000.0).abs() < 1.0);
        assert!(err(factorial, &[num(2.5)]).contains("non-negative integer"));
        assert_eq!(call(nchoosek, &[num(5.0), num(2.0)]).unwrap().data, [10.0]);
        assert_eq!(binomial(52.0, 5.0), 2598960.0);
        assert_eq!(binomial(60.0, 30.0), 118264581564861424.0);
        assert!(err(nchoosek, &[num(3.0), num(5.0)]).contains("between 0 and N"));
        let c = call(nchoosek, &[row(&[1.0, 2.0, 3.0, 4.0]), num(2.0)]).unwrap();
        assert_eq!((c.rows, c.cols), (6, 2));
        // Rows [1 2; 1 3; 1 4; 2 3; 2 4; 3 4], column-major.
        assert_eq!(
            c.data,
            [1.0, 1.0, 1.0, 2.0, 2.0, 3.0, 2.0, 3.0, 4.0, 3.0, 4.0, 4.0]
        );
        let c = call(nchoosek, &[row(&[1.0, 2.0]), num(3.0)]).unwrap();
        assert_eq!((c.rows, c.cols), (0, 3));
        assert!(err(nchoosek, &[row(&[1.0; 60]), num(30.0)]).contains("maximum array size"));
        assert_eq!(
            call(primes, &[num(20.0)]).unwrap().data,
            [2.0, 3.0, 5.0, 7.0, 11.0, 13.0, 17.0, 19.0]
        );
        assert_eq!(call(primes, &[num(1.0)]).unwrap().cols, 0);
        assert_eq!(sieve(100).len(), 25);
        let p = call(isprime, &[row(&[0.0, 1.0, 2.0, 9.0, 97.0, 561.0])]).unwrap();
        assert_eq!(p.class, Class::Logical);
        assert_eq!(p.data, [0.0, 0.0, 1.0, 0.0, 1.0, 0.0]);
        // The largest prime below 2^53, and a strong pseudoprime to base 2.
        assert!(is_prime(9_007_199_254_740_881));
        assert!(!is_prime(2_047));
        assert!(!is_prime(3_215_031_751));
        assert!(err(isprime, &[num(-3.0)]).contains("non-negative integer"));
    }

    #[test]
    fn gcd_and_lcm_are_element_wise() {
        assert_eq!(call(gcd, &[num(12.0), num(18.0)]).unwrap().data, [6.0]);
        assert_eq!(call(lcm, &[num(4.0), num(6.0)]).unwrap().data, [12.0]);
        assert_eq!(
            call(gcd, &[row(&[-12.0, 0.0, 7.0]), num(8.0)])
                .unwrap()
                .data,
            [4.0, 8.0, 1.0]
        );
        assert_eq!(
            call(lcm, &[row(&[0.0, -3.0]), num(4.0)]).unwrap().data,
            [0.0, 12.0]
        );
        assert_eq!(gcd_of(0.0, 0.0), 0.0);
        assert!(err(gcd, &[num(2.5), num(5.0)]).contains("Argument 1"));
        assert!(err(lcm, &[num(2.0), num(f64::NAN)]).contains("Argument 2"));
    }

    // ---- grids and counts --------------------------------------------

    #[test]
    fn logspace_meshgrid_and_histc() {
        assert_eq!(
            call(logspace, &[num(0.0), num(2.0), num(3.0)])
                .unwrap()
                .data,
            [1.0, 10.0, 100.0]
        );
        assert_eq!(
            call(logspace, &[num(-2.0), num(0.0), num(3.0)])
                .unwrap()
                .data,
            [0.01, 0.1, 1.0]
        );
        assert_eq!(call(logspace, &[num(0.0), num(1.0)]).unwrap().numel(), 50);
        let l = call(logspace, &[num(0.0), num(PI), num(3.0)]).unwrap();
        assert_eq!((l.data[0], l.data[2]), (1.0, PI));
        let g = outputs(meshgrid, &[row(&[1.0, 2.0]), row(&[1.0, 2.0, 3.0])], 2).unwrap();
        assert_eq!((g[0].rows, g[0].cols), (3, 2));
        assert_eq!(g[0].data, [1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
        assert_eq!(g[1].data, [1.0, 2.0, 3.0, 1.0, 2.0, 3.0]);
        assert_eq!(
            outputs(meshgrid, &[row(&[1.0, 2.0])], 1).unwrap()[0].rows,
            2
        );
        assert!(err(meshgrid, &[num(1.0), num(1.0), num(1.0)]).contains("N-D"));
        let h = outputs(
            histc,
            &[
                row(&[1.0, 2.0, 2.0, 3.0, 5.0, 0.0, 4.0]),
                row(&[1.0, 2.0, 3.0, 4.0]),
            ],
            2,
        )
        .unwrap();
        assert_eq!((h[0].rows, h[0].cols), (1, 4));
        assert_eq!(h[0].data, [1.0, 2.0, 1.0, 1.0]);
        assert_eq!(h[1].data, [1.0, 2.0, 2.0, 3.0, 0.0, 0.0, 4.0]);
        let h = call(
            histc,
            &[rows(2, 2, &[1.0, 1.0, 2.0, 3.0]), row(&[1.0, 2.0, 3.0])],
        )
        .unwrap();
        assert_eq!(
            (h.rows, h.cols, h.data.clone()),
            (3, 2, vec![1.0, 1.0, 0.0, 1.0, 0.0, 1.0])
        );
        assert!(err(histc, &[num(1.0), row(&[2.0, 1.0])]).contains("non-decreasing"));
        assert_eq!(bin_of(&[1.0, 2.0, 2.0, 3.0], 2.0), Some(2));
    }

    #[test]
    fn map_slices_judges_the_shape_and_refuses_nd() {
        let m = Matrix::new(2, 3, (1..=6).map(f64::from).collect());
        let s = map_slices(&m, 1, 1, |s| vec![s.iter().sum()]).unwrap();
        assert_eq!(s.data, [3.0, 7.0, 11.0]);
        let s = map_slices(&m, 2, 1, |s| vec![s.iter().sum()]).unwrap();
        assert_eq!((s.rows, s.cols, s.data.clone()), (2, 1, vec![9.0, 12.0]));
        assert!(map_slices(&m, 3, 0, |_| vec![]).is_err());
        assert_eq!(first_dim(&Matrix::scalar(1.0)), 1);
        assert_eq!(first_dim(&Matrix::row(vec![1.0, 2.0])), 2);
        assert_eq!(first_dim(&Matrix::empty()), 1);
    }
}
