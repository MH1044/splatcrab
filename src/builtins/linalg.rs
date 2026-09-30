//! Linear algebra, array rearrangement, and search and sort.

use std::cmp::Ordering;

use super::args::{
    at_most, check_dims, check_shape, dim, fmt_dim, mat, need, option, shape, size_list,
};
use super::complex::C;
use super::core::eps_at;
use super::factor::{self, JACOBI_SWEEPS, SVD_SWEEPS, Vectors, qr_iterations};
use super::math::{reduce, sum0};
use super::{Registry, add, one_as, one_mat};
use crate::error;
use crate::interp::{Interp, R};
use crate::value::{Class, Matrix, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    // ---- linear algebra ----------------------------------------------
    add(r, "transpose", transpose, "transpose(A) - the non-conjugate transpose of A.");
    add(r, "inv", inv, "inv(A) - the inverse of a square matrix.");
    add(r, "det", det, "det(A) - the determinant of a square matrix.");
    add(r, "trace", trace, "trace(A) - the sum of the diagonal of a square matrix.");
    add(r, "diag", diag, "diag(v,k) puts v on the k-th diagonal; diag(A,k) takes the k-th diagonal.");
    add(r, "norm", norm, "norm(v,p), norm(A,p) - a vector p-norm (1, 2, p > 0, Inf, -Inf, 'fro'), or a matrix norm (1, 2, Inf, 'fro').");
    add(r, "dot", dot, "dot(A,B), dot(A,B,dim) - scalar products of vectors, or of matching columns.");
    add(r, "lu", lu, "[L,U,P] = lu(A), [L,U] = lu(A), Y = lu(A) - LU factorisation with partial pivoting, P*A = L*U.");
    add(r, "qr", qr, "[Q,R] = qr(A), R = qr(A) - Householder QR factorisation, A = Q*R.");
    add(r, "chol", chol, "R = chol(A), [R,p] = chol(A) - Cholesky factor, R'*R = A, of a positive definite matrix.");
    add(r, "eig", eig, "e = eig(A), [V,D] = eig(A) - real eigenvalues and eigenvectors, A*V = V*D.");
    add(r, "svd", svd, "s = svd(A), [U,S,V] = svd(A) - singular value decomposition, A = U*S*V'.");
    add(r, "rank", rank, "rank(A), rank(A,tol) - the number of singular values above the tolerance.");
    add(r, "pinv", pinv, "pinv(A), pinv(A,tol) - the Moore-Penrose pseudoinverse.");
    add(r, "null", null, "null(A) - an orthonormal basis of the null space of A.");
    add(r, "orth", orth, "orth(A) - an orthonormal basis of the range of A.");
    add(r, "cond", cond, "cond(A) - the 2-norm condition number, the ratio of the largest singular value to the smallest.");
    add(r, "kron", kron, "kron(A,B) - the Kronecker tensor product.");
    add(r, "cross", cross, "cross(A,B) - the cross product of 3-element vectors, or along the first dimension of length 3.");
    add(r, "triu", triu, "triu(A), triu(A,k) - the upper triangle of A, on and above the k-th diagonal.");
    add(r, "tril", tril, "tril(A), tril(A,k) - the lower triangle of A, on and below the k-th diagonal.");
    add(r, "magic", magic, "magic(n) - an n-by-n magic square, MATLAB's construction.");

    // ---- rearrangement -----------------------------------------------
    add(r, "reshape", reshape, "reshape(A,r,c), reshape(A,sz), reshape(A,r,[]) - the elements of A in a new shape.");
    add(r, "repmat", repmat, "repmat(A,n), repmat(A,r,c), repmat(A,sz) - tile A into a block matrix.");
    add(r, "fliplr", fliplr, "fliplr(A) - reverse the order of the columns.");
    add(r, "flipud", flipud, "flipud(A) - reverse the order of the rows.");

    // ---- search and sort ---------------------------------------------
    add(r, "find", find, "find(A), find(A,n), find(A,n,'last'), [r,c,v] = find(...) - indices of the non-zero elements.");
    add(r, "sort", sort, "sort(A), sort(A,dim), sort(A,'descend'), [s,i] = sort(...) - sorted vectors, columns or rows, NaN at the high end.");
}

// ---- linear algebra --------------------------------------------------

fn transpose(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "transpose")?;
    one_as(mat(a, 0, "transpose")?.transpose())
}

/// `inv(A)`, with the singular warning and an `Inf` result for a singular
/// matrix, as MATLAB gives them (QA D26; an error before cycle 08).
fn inv(it: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "inv")?;
    let (x, w) = mat(a, 0, "inv")?.inv()?;
    it.warn(w)?;
    one_mat(x)
}

fn det(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "det")?;
    one_mat(Matrix::scalar(mat(a, 0, "det")?.det()?))
}

fn trace(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "trace")?;
    let m = mat(a, 0, "trace")?;
    if m.rows != m.cols {
        return Err(error::nonsquare_trace());
    }
    // Through `math::sum0`, not Rust's `.sum()`, whose empty sum is `-0.0`:
    // `fprintf('%.4f', trace([]))` used to print `-0.0000`. Cycle 01c routed
    // `sum`, `mean`, `norm` and `dot` this way and missed `trace`.
    let diagonal: Vec<f64> = (0..m.rows).map(|i| m.get(i, i)).collect();
    one_mat(Matrix::scalar(sum0(&diagonal)))
}

/// `diag(v, k)` places `v` on the k-th diagonal of a square matrix of order
/// `numel(v) + abs(k)`, and `diag(A, k)` returns the k-th diagonal of `A` as
/// a column. `k > 0` is above the main diagonal. A `k` past the matrix gives
/// a 0x1, which is Octave's answer and MATLAB's shape for an empty diagonal.
///
/// `diag([])` is the exception: it is `0x0` in MATLAB, not the `0x1` that
/// rule would give, because a `0x0` has no diagonal to orient. The argument
/// is still validated first, so `diag([], 'x')` is the offset error.
fn diag(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "diag")?;
    let m = mat(a, 0, "diag")?;
    let k = if a.len() >= 2 {
        match (option(a, 1), mat(a, 1, "diag")?.scalar_value()) {
            (None, Some(k)) if k.fract() == 0.0 => k,
            _ => return Err(error::diag_offset()),
        }
    } else {
        0.0
    };
    // A char keeps its class, as in MATLAB (cycle 11): `diag('ab')` is a
    // 2x2 char, its zeros the character with code 0. Every other class is a
    // double, as before.
    let keep = |d: Matrix| {
        if m.class == Class::Char {
            one_as(d.with_class(Class::Char))
        } else {
            one_mat(d)
        }
    };
    if m.rows == 0 && m.cols == 0 {
        return keep(Matrix::empty());
    }
    if m.is_vector() {
        let n = m.numel();
        let (order, _) = check_shape(n as f64 + k.abs(), n as f64 + k.abs())?;
        // `k` is now known to fit, because the order does.
        let (dr, dc) = if k >= 0.0 {
            (0, k as usize)
        } else {
            (-k as usize, 0)
        };
        let mut out = Matrix::filled(order, order, 0.0);
        for (i, v) in m.data.iter().enumerate() {
            out.set(i + dr, i + dc, *v);
        }
        keep(out)
    } else {
        // Row and column of the diagonal's first element. They stay `f64`
        // until they are known to lie inside the matrix, so `diag(A, 1e300)`
        // is an honest 0x1 rather than a saturated cast.
        let (r0, c0) = if k >= 0.0 { (0.0, k) } else { (-k, 0.0) };
        let inside = r0 < m.rows as f64 && c0 < m.cols as f64;
        let (r0, c0) = if inside {
            (r0 as usize, c0 as usize)
        } else {
            (0, 0)
        };
        let len = if inside {
            (m.rows - r0).min(m.cols - c0)
        } else {
            0
        };
        keep(Matrix::col(
            (0..len).map(|i| m.get(r0 + i, c0 + i)).collect(),
        ))
    }
}

/// The order of a norm.
#[derive(Clone, Copy, Debug, PartialEq)]
enum NormType {
    /// A positive, finite `p`; `2` is the default.
    P(f64),
    /// `'fro'`: the 2-norm of a vector, and the square root of the sum of
    /// squares of a matrix, which is not its 2-norm.
    Fro,
    Inf,
    NegInf,
}

/// `norm(v, p)`: `p` is a positive real scalar, `Inf` or `-Inf`, or one of
/// the strings `'fro'` (the 2-norm, for a vector) and `'inf'`. The MATLAB page
/// does not say what `p = 0` or a negative finite `p` does, so both are
/// refused rather than guessed.
fn norm_type(a: &[Value]) -> R<NormType> {
    if let Some(s) = option(a, 1) {
        return if s.eq_ignore_ascii_case("fro") {
            Ok(NormType::Fro)
        } else if s.eq_ignore_ascii_case("inf") {
            Ok(NormType::Inf)
        } else {
            Err(error::norm_type())
        };
    }
    match mat(a, 1, "norm")?.scalar_value() {
        Some(p) if p == f64::INFINITY => Ok(NormType::Inf),
        Some(p) if p == f64::NEG_INFINITY => Ok(NormType::NegInf),
        Some(p) if p > 0.0 && p.is_finite() => Ok(NormType::P(p)),
        _ => Err(error::norm_type()),
    }
}

fn norm(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "norm")?;
    let m = mat(a, 0, "norm")?;
    let is_matrix = !m.is_vector() && !m.is_empty();
    // An order the vector rules refuse is refused for a matrix in the
    // matrix's own words: the vector text offers `-Inf` and any positive
    // real, which a matrix does not take (cycle 08's review).
    let p = if a.len() >= 2 {
        match norm_type(a) {
            Ok(p) => p,
            Err(_) if is_matrix => return Err(error::matrix_norm_type()),
            Err(e) => return Err(e),
        }
    } else {
        NormType::P(2.0)
    };
    if is_matrix {
        return one_mat(Matrix::scalar(matrix_norm(&m, p)?));
    }
    one_mat(Matrix::scalar(vector_norm(&m.data, p)))
}

/// `norm(A, p)` of a matrix (cycle 08; vectors only before it): the largest
/// column sum of magnitudes for `1`, the largest row sum for `Inf`, the
/// root of the sum of squares for `'fro'` and the largest singular value
/// for `2`. Any other order is refused, as MATLAB offers no other matrix
/// norm. A `NaN` anywhere makes every one of them `NaN`; an `Inf` makes the
/// 2-norm `Inf` without an SVD, which would refuse it.
fn matrix_norm(m: &Matrix, p: NormType) -> R<f64> {
    let nan = m.data.iter().any(|x| x.is_nan());
    let sums = |outer: usize, inner: usize, at: &dyn Fn(usize, usize) -> f64| {
        (0..outer)
            .map(|o| sum0(&(0..inner).map(|i| at(o, i).abs()).collect::<Vec<f64>>()))
            .fold(0.0, f64::max)
    };
    let v = match p {
        NormType::P(1.0) => sums(m.cols, m.rows, &|c, r| m.get(r, c)),
        NormType::Inf => sums(m.rows, m.cols, &|r, c| m.get(r, c)),
        NormType::Fro => vector_norm(&m.data, NormType::P(2.0)),
        NormType::P(2.0) => {
            if nan {
                f64::NAN
            } else if m.data.iter().any(|x| x.is_infinite()) {
                f64::INFINITY
            } else {
                singular_values(m, "norm")?.first().copied().unwrap_or(0.0)
            }
        }
        _ => return Err(error::matrix_norm_type()),
    };
    Ok(if nan { f64::NAN } else { v })
}

/// A vector norm that neither overflows nor underflows. With `s` the largest
/// magnitude, the p-norm is `s * (sum((abs(v) / s).^p))^(1/p)`, so
/// `norm([1e200 1e200])` is `1.4142e+200` rather than `Inf`. When `s` is not
/// a finite non-zero number it is the answer itself: `0` for an all-zero or
/// empty vector ("The norm of an empty matrix is zero"), `Inf` when an
/// element is infinite. A `NaN` anywhere is `NaN`.
fn vector_norm(v: &[f64], p: NormType) -> f64 {
    if v.iter().any(|x| x.is_nan()) {
        return f64::NAN;
    }
    let abs = v.iter().map(|x| x.abs());
    match p {
        NormType::Inf => abs.fold(0.0, f64::max),
        NormType::NegInf => {
            if v.is_empty() {
                0.0
            } else {
                abs.fold(f64::INFINITY, f64::min)
            }
        }
        NormType::P(1.0) => abs.fold(0.0, |acc, x| acc + x),
        NormType::P(_) | NormType::Fro => {
            let p = if let NormType::P(p) = p { p } else { 2.0 };
            let s = abs.clone().fold(0.0, f64::max);
            if s == 0.0 || !s.is_finite() {
                return s;
            }
            let total = if p == 2.0 {
                abs.fold(0.0, |acc, x| acc + (x / s) * (x / s))
            } else {
                abs.fold(0.0, |acc, x| acc + (x / s).powf(p))
            };
            if p == 2.0 {
                s * total.sqrt()
            } else {
                s * total.powf(1.0 / p)
            }
        }
    }
}

/// `dot(A, B)` and `dot(A, B, dim)`. Two vectors of the same length, in any
/// orientation, give their scalar product. Otherwise `A` and `B` must be the
/// same size, and the answer is `sum(A .* B)` along the first non-singleton
/// dimension or along `dim`. With `dim`, even two vectors must match in size.
fn dot(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "dot")?;
    need(a, 2, "dot")?;
    let x = mat(a, 0, "dot")?;
    let y = mat(a, 1, "dot")?;
    let d = if a.len() >= 3 {
        Some(dim(a, 2, "dot")?)
    } else {
        None
    };
    if d.is_none() && x.is_vector() && y.is_vector() && x.numel() == y.numel() {
        let products: Vec<f64> = x.data.iter().zip(&y.data).map(|(p, q)| p * q).collect();
        return one_mat(Matrix::scalar(sum0(&products)));
    }
    if (x.rows, x.cols) != (y.rows, y.cols) {
        return Err(error::dot_size_mismatch());
    }
    let products = x.zip(&y, "dot", |p, q| p * q)?;
    one_mat(reduce(&products, d, sum0)?)
}

// ---- factorisations (cycle 08) --------------------------------------

/// `Y = lu(A)`, `[L, U] = lu(A)` and `[L, U, P] = lu(A)`, for any
/// `m`-by-`n` `A`: `P * A = L * U`. With two outputs `L` is `P' * L`, the
/// permuted lower triangle, so that `A = L * U`; with one, `Y` is LAPACK's
/// packed form, the multipliers below the diagonal and `U` on and above it.
/// `lu` never warns; a singular `A` has a zero on `U`'s diagonal. A `NaN`
/// spreads through the arithmetic, and there is no iteration to hang.
fn lu(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 1, "lu")?;
    let f = factor::lu(&mat(a, 0, "lu")?);
    if nargout < 2 {
        return one_mat(f.lu);
    }
    let (l, u, p) = f.factors()?;
    if nargout == 2 {
        // Row i of L is row perm[i] of P' * L. Written out rather than
        // multiplied, so that a NaN multiplier stays in its own row.
        let mut pl = l.clone();
        for (i, &r) in f.perm.iter().enumerate() {
            for j in 0..l.cols {
                pl.set(r, j, l.get(i, j));
            }
        }
        return Ok(vec![Value::Mat(pl), Value::Mat(u)]);
    }
    Ok(vec![Value::Mat(l), Value::Mat(u), Value::Mat(p)])
}

/// `R = qr(A)` and `[Q, R] = qr(A)`, the full factorisation: `Q` is
/// `m`-by-`m`. `R`'s diagonal may be negative, as LAPACK's is. A `NaN` or
/// `Inf` spreads to a `NaN` result.
fn qr(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 1, "qr")?;
    let (q, r) = factor::qr(&mat(a, 0, "qr")?, nargout >= 2)?;
    match q {
        Some(q) => Ok(vec![Value::Mat(q), Value::Mat(r)]),
        None => one_mat(r),
    }
}

/// `R = chol(A)` and `[R, p] = chol(A)`. The first form refuses a matrix
/// that is not positive definite, a `NaN` on the diagonal included; the
/// second returns the one-based column `p` that failed, with `R` the
/// leading `(p-1)`-by-`(p-1)` factor, and `p = 0` on success.
fn chol(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 1, "chol")?;
    let m = mat(a, 0, "chol")?;
    if m.rows != m.cols {
        return Err(error::nonsquare_for("chol"));
    }
    let (r, fail) = factor::chol(&m)?;
    if nargout >= 2 {
        let p = fail.map_or(0.0, |j| (j + 1) as f64);
        return Ok(vec![Value::Mat(r), Value::Mat(Matrix::scalar(p))]);
    }
    match fail {
        None => one_mat(r),
        Some(_) => Err(error::not_positive_definite()),
    }
}

/// `e = eig(A)` and `[V, D] = eig(A)`. An exactly symmetric `A` goes to the
/// Jacobi method and its eigenvalues are ascending; any other to the
/// Hessenberg QR iteration, in the order it finds them, a complex pair as
/// two complex values with complex vectors (cycle 10; a refusal before it).
/// A `NaN` or `Inf` is refused before any iteration starts, and a complex
/// `A` by the registry's gate.
fn eig(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 1, "eig")?;
    let m = mat(a, 0, "eig")?;
    if m.rows != m.cols {
        return Err(error::nonsquare_for("eig"));
    }
    if !factor::all_finite(&m) {
        return Err(error::nonfinite_input("eig"));
    }
    let (vals, vecs) = if factor::is_symmetric(&m) {
        let (vals, vecs) = factor::eig_sym(&m, JACOBI_SWEEPS)?;
        (vals.into_iter().map(C::real).collect(), vecs)
    } else {
        factor::eig_general(&m, qr_iterations(m.rows))?
    };
    let n = vals.len();
    if nargout < 2 {
        return one_mat(Matrix::from_c(n, 1, vals));
    }
    let re: Vec<f64> = vals.iter().map(|z| z.re).collect();
    let im: Vec<f64> = vals.iter().map(|z| z.im).collect();
    let d = diagonal(&re, n, n)?.with_im(Some(diagonal(&im, n, n)?.data));
    Ok(vec![Value::Mat(vecs), Value::Mat(d)])
}

/// An `rows`-by-`cols` matrix with `v` down its diagonal.
fn diagonal(v: &[f64], rows: usize, cols: usize) -> R<Matrix> {
    let mut d = factor::zeros(rows, cols)?;
    for (i, x) in v.iter().enumerate() {
        d.set(i, i, *x);
    }
    Ok(d)
}

/// The SVD every builtin that needs one shares: a `NaN` or `Inf` is refused
/// in the name of the builtin the user called, before any sweep starts.
fn checked_svd(m: &Matrix, want: Vectors, name: &str) -> R<factor::Svd> {
    if !factor::all_finite(m) {
        return Err(error::nonfinite_input(name));
    }
    factor::svd(m, want, SVD_SWEEPS)
}

fn singular_values(m: &Matrix, name: &str) -> R<Vec<f64>> {
    Ok(checked_svd(m, Vectors::None, name)?.s)
}

/// `s = svd(A)`, a column of the `min(m, n)` singular values in descending
/// order, and `[U, S, V] = svd(A)`, the full decomposition.
fn svd(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 1, "svd")?;
    let m = mat(a, 0, "svd")?;
    if nargout < 2 {
        return one_mat(Matrix::col(singular_values(&m, "svd")?));
    }
    let f = checked_svd(&m, Vectors::Full, "svd")?;
    let s = diagonal(&f.s, m.rows, m.cols)?;
    let (u, v) = (f.u.expect("full vectors"), f.v.expect("full vectors"));
    Ok(vec![Value::Mat(u), Value::Mat(s), Value::Mat(v)])
}

/// The optional tolerance argument of `rank` and `pinv`.
fn tolerance(a: &[Value], i: usize, name: &str) -> R<Option<f64>> {
    if a.len() <= i {
        return Ok(None);
    }
    match (option(a, i), mat(a, i, name)?.scalar_value()) {
        (None, Some(t)) => Ok(Some(t)),
        _ => Err(error::tolerance_arg(name)),
    }
}

/// The tolerance `rank`, `null` and `orth` share, `max(m, n) * eps(s(1))`,
/// the default the MATLAB `rank` page gives. Sharing it is what makes
/// `rank(A) + size(null(A), 2)` equal `size(A, 2)`.
fn rank_tol(m: &Matrix, s: &[f64]) -> f64 {
    m.rows.max(m.cols) as f64 * eps_at(s.first().copied().unwrap_or(0.0))
}

fn rank(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "rank")?;
    let m = mat(a, 0, "rank")?;
    let tol = tolerance(a, 1, "rank")?;
    let s = singular_values(&m, "rank")?;
    let tol = tol.unwrap_or_else(|| rank_tol(&m, &s));
    one_mat(Matrix::scalar(s.iter().filter(|&&x| x > tol).count() as f64))
}

/// `pinv(A)` and `pinv(A, tol)`: `V * diag(1 ./ s) * U'` over the singular
/// values above `tol`, by default `max(m, n) * s(1) * eps`, the MATLAB
/// `pinv` page's default. The result is `n`-by-`m`.
fn pinv(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "pinv")?;
    let m = mat(a, 0, "pinv")?;
    let tol = tolerance(a, 1, "pinv")?;
    let f = checked_svd(&m, Vectors::Thin, "pinv")?;
    let smax = f.s.first().copied().unwrap_or(0.0);
    let tol = tol.unwrap_or(m.rows.max(m.cols) as f64 * smax * f64::EPSILON);
    let (u, v) = (f.u.expect("thin vectors"), f.v.expect("thin vectors"));
    let mut x = factor::zeros(m.cols, m.rows)?;
    for (k, &sk) in f.s.iter().enumerate().filter(|(_, s)| **s > tol) {
        for j in 0..m.rows {
            let ujk = u.get(j, k) / sk;
            for i in 0..m.cols {
                let e = x.get(i, j) + v.get(i, k) * ujk;
                x.set(i, j, e);
            }
        }
    }
    one_mat(x)
}

/// `null(A)`: the columns of the full `V` past the rank, an orthonormal
/// basis of the null space, `n`-by-`(n - r)`.
fn null(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "null")?;
    let m = mat(a, 0, "null")?;
    let f = checked_svd(&m, Vectors::Full, "null")?;
    let tol = rank_tol(&m, &f.s);
    let r = f.s.iter().filter(|&&x| x > tol).count();
    let v = f.v.expect("full vectors");
    one_mat(factor::cols_range(&v, r, v.cols))
}

/// `orth(A)`: the columns of `U` that belong to the singular values above
/// the rank tolerance, an orthonormal basis of the range, `m`-by-`r`.
fn orth(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "orth")?;
    let m = mat(a, 0, "orth")?;
    let f = checked_svd(&m, Vectors::Thin, "orth")?;
    let tol = rank_tol(&m, &f.s);
    let r = f.s.iter().filter(|&&x| x > tol).count();
    one_mat(factor::cols_range(&f.u.expect("thin vectors"), 0, r))
}

/// `cond(A)`, the 2-norm condition number: `s(1) / s(end)`, `Inf` for a
/// singular matrix (a zero singular value, the zero matrix included) and
/// `0` for an empty one.
fn cond(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "cond")?;
    let m = mat(a, 0, "cond")?;
    let s = singular_values(&m, "cond")?;
    let c = match (s.first(), s.last()) {
        (Some(_), Some(&0.0)) => f64::INFINITY,
        (Some(&hi), Some(&lo)) => hi / lo,
        _ => 0.0,
    };
    one_mat(Matrix::scalar(c))
}

// ---- constructions (cycle 08) ----------------------------------------

/// `kron(A, B)`: the block matrix of `A(i, j) * B`, `(ma*mb)`-by-`(na*nb)`,
/// its shape judged before it is allocated.
fn kron(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "kron")?;
    need(a, 2, "kron")?;
    let (x, y) = (mat(a, 0, "kron")?, mat(a, 1, "kron")?);
    let (rows, cols) = check_shape(x.rows as f64 * y.rows as f64, x.cols as f64 * y.cols as f64)?;
    let mut out = Matrix::filled(rows, cols, 0.0);
    if out.data.is_empty() {
        // No block to fill, and a factor's other dimension alone can be
        // enormous (cycle 13b).
        return one_mat(out);
    }
    for j in 0..x.cols {
        for i in 0..x.rows {
            let v = x.get(i, j);
            for l in 0..y.cols {
                for k in 0..y.rows {
                    out.set(i * y.rows + k, j * y.cols + l, v * y.get(k, l));
                }
            }
        }
    }
    one_mat(out)
}

/// `cross(A, B)` of two arrays of one size: along the first dimension of
/// length 3, so a 3-element row or column gives the same shape back, and a
/// 3-by-n matrix is taken column by column.
fn cross(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "cross")?;
    need(a, 2, "cross")?;
    let (x, y) = (mat(a, 0, "cross")?, mat(a, 1, "cross")?);
    if (x.rows, x.cols) != (y.rows, y.cols) {
        return Err(error::ab_size_mismatch("cross"));
    }
    // The linear indices of each triple.
    let r = x.rows;
    let triples: Vec<[usize; 3]> = if r == 3 {
        (0..x.cols).map(|j| [3 * j, 3 * j + 1, 3 * j + 2]).collect()
    } else if x.cols == 3 {
        (0..r).map(|i| [i, i + r, i + 2 * r]).collect()
    } else {
        return Err(error::cross_length());
    };
    let mut out = Matrix::filled(x.rows, x.cols, 0.0);
    for [i, j, k] in triples {
        let (p, q) = (&x.data, &y.data);
        out.data[i] = p[j] * q[k] - p[k] * q[j];
        out.data[j] = p[k] * q[i] - p[i] * q[k];
        out.data[k] = p[i] * q[j] - p[j] * q[i];
    }
    one_mat(out)
}

/// The `k` of `triu(A, k)` and `tril(A, k)`: an integer scalar, `0` when
/// absent, refused as `diag` refuses its offset.
fn tri_offset(a: &[Value], name: &str) -> R<f64> {
    if a.len() < 2 {
        return Ok(0.0);
    }
    match (option(a, 1), mat(a, 1, name)?.scalar_value()) {
        (None, Some(k)) if k.fract() == 0.0 => Ok(k),
        _ => Err(error::diag_offset()),
    }
}

/// `triu` and `tril`: `A` with the elements on the wrong side of the k-th
/// diagonal zeroed, its class kept, as a rearrangement keeps it.
fn triangle(a: &[Value], name: &str, keep: fn(f64, f64) -> bool) -> R<Vec<Value>> {
    at_most(a, 2, name)?;
    let mut m = mat(a, 0, name)?;
    let k = tri_offset(a, name)?;
    // Nothing to zero in an empty matrix, however many columns (cycle 13b).
    let cols = if m.data.is_empty() { 0 } else { m.cols };
    for j in 0..cols {
        for i in 0..m.rows {
            if !keep(j as f64 - i as f64, k) {
                m.set(i, j, 0.0);
            }
        }
    }
    one_as(m)
}

fn triu(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    triangle(a, "triu", |d, k| d >= k)
}

fn tril(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    triangle(a, "tril", |d, k| d <= k)
}

/// `magic(n)`: MATLAB's construction, the siamese method for odd `n`, the
/// complement pattern for a multiple of 4, and the LUX-style quadrant swap
/// for the rest. `n` is floored, and below 1 the answer is `[]`.
fn magic(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "magic")?;
    need(a, 1, "magic")?;
    let n = match (option(a, 0), mat(a, 0, "magic")?.scalar_value()) {
        (None, Some(n)) if !n.is_nan() => n.floor(),
        _ => return Err(error::magic_order()),
    };
    if n < 1.0 {
        return one_mat(Matrix::empty());
    }
    let (n, _) = check_shape(n, n)?;
    one_mat(magic_square(n))
}

fn magic_square(n: usize) -> Matrix {
    let mut m = Matrix::filled(n, n, 0.0);
    if n % 2 == 1 {
        // M = n * mod(I + J - (n + 3) / 2, n) + mod(I + 2J - 2, n) + 1, with
        // one-based I and J; n is added before subtracting so nothing wraps.
        for i in 1..=n {
            for j in 1..=n {
                let a = (i + j + n - (n + 3) / 2) % n;
                let b = (i + 2 * j - 2) % n;
                m.set(i - 1, j - 1, (n * a + b + 1) as f64);
            }
        }
    } else if n % 4 == 0 {
        // 1:n^2 row by row, complemented where fix(mod(I, 4) / 2) equals
        // fix(mod(J, 4) / 2).
        for i in 1..=n {
            for j in 1..=n {
                let v = ((i - 1) * n + j) as f64;
                let flip = (i % 4) / 2 == (j % 4) / 2;
                m.set(i - 1, j - 1, if flip { (n * n + 1) as f64 - v } else { v });
            }
        }
    } else {
        let p = n / 2;
        let q = magic_square(p);
        let pp = (p * p) as f64;
        for j in 0..p {
            for i in 0..p {
                let v = q.get(i, j);
                m.set(i, j, v);
                m.set(i, j + p, v + 2.0 * pp);
                m.set(i + p, j, v + 3.0 * pp);
                m.set(i + p, j + p, v + pp);
            }
        }
        if n == 2 {
            return m;
        }
        let swap = |m: &mut Matrix, i: usize, j: usize| {
            let t = m.get(i, j);
            m.set(i, j, m.get(i + p, j));
            m.set(i + p, j, t);
        };
        // Columns 1..k and n-k+2..n (one-based) swap their halves ...
        let k = (n - 2) / 4;
        let cols: Vec<usize> = (0..k).chain(n - k + 1..n).collect();
        for &j in &cols {
            for i in 0..p {
                swap(&mut m, i, j);
            }
        }
        // ... and row k+1 swaps with row k+1+p in columns 1 and k+1.
        for j in [0, k] {
            swap(&mut m, k, j);
        }
    }
    m
}

// ---- rearrangement ---------------------------------------------------

/// `reshape(A, r, c, ...)`, `reshape(A, sz)`, and one `[]` placeholder among
/// the sizes, anywhere, for the one that makes the count come out. Since
/// cycle 14 it takes any number of sizes and an N-D `A`: `reshape(1:24, 2,
/// 3, 4)` is 2x3x4 and `reshape(A, 6, [])` of it 6x4. Trailing sizes of `1`
/// are dropped, and the shape is judged by `check_dims` before the count.
fn reshape(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 2, "reshape")?;
    let m = mat(a, 0, "reshape")?;
    // A single size is a size vector of at least two elements. A scalar is
    // not `n` by `n` here, as it is for `zeros`: MATLAB refuses it.
    if let [_, Value::Mat(sz)] = a {
        if sz.is_scalar() || sz.is_empty() {
            return Err(error::not_enough_args("reshape"));
        }
    }
    let list = size_list(a, 1, "reshape", true)?;
    let unknown = list.iter().filter(|d| d.is_none()).count();
    if unknown > 1 {
        return Err(error::reshape_two_unknowns());
    }
    let total = m.numel();
    let known: f64 = list.iter().flatten().product();
    let fill = if unknown == 0 {
        0.0
    } else if known == 0.0 && total == 0 {
        // Any size would do; Octave makes it 0.
        0.0
    } else if known == 0.0 || total as f64 % known != 0.0 {
        return Err(error::reshape_not_divisible(&fmt_dim(known), total));
    } else {
        total as f64 / known
    };
    let mut dims: Vec<f64> = list.into_iter().map(|d| d.unwrap_or(fill)).collect();
    while dims.len() > 2 && dims.last() == Some(&1.0) {
        dims.pop();
    }
    let dims = check_dims(&dims)?;
    // Saturating, since the sizes of an empty shape may multiply past
    // `usize` before they reach the 0 (`reshape([], 2^40, 2^40, 0)`).
    if crate::value::dims_product(&dims) != total {
        return Err(error::reshape_numel(total, &dims));
    }
    // Rearrangement keeps the class, here and in the next three.
    one_as(Matrix::from_dims(&dims, m.data).with_class(m.class))
}

/// `repmat(A, n)`, `repmat(A, r, c, ...)` and `repmat(A, sz)`, through the
/// same size parser as the constructors.
fn repmat(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 2, "repmat")?;
    let m = mat(a, 0, "repmat")?;
    let (r, c) = shape(a, 1, "repmat", usize::MAX)?;
    // Judge the shape that was actually asked for. Checking `m.rows * r` on
    // its own reported "Requested 1x10000000000" for
    // `repmat([1 2], 1e10, 1e10)`, naming an intermediate instead of the
    // 10000000000x20000000000 array requested.
    let (rows, cols) = check_shape(m.rows as f64 * r, m.cols as f64 * c)?;
    let mut out = Matrix::filled(rows, cols, 0.0).with_class(m.class);
    // An empty result has no columns worth visiting (cycle 13b).
    let out_cols = if out.data.is_empty() { 0 } else { out.cols };
    for j in 0..out_cols {
        for i in 0..out.rows {
            out.set(i, j, m.get(i % m.rows.max(1), j % m.cols.max(1)));
        }
    }
    one_as(out)
}

fn fliplr(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "fliplr")?;
    let m = mat(a, 0, "fliplr")?;
    let mut out = m.clone();
    // An empty matrix is its own flip (cycle 13b).
    let cols = if m.data.is_empty() { 0 } else { m.cols };
    for c in 0..cols {
        for r in 0..m.rows {
            out.set(r, m.cols - 1 - c, m.get(r, c));
        }
    }
    one_as(out)
}

fn flipud(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "flipud")?;
    let m = mat(a, 0, "flipud")?;
    let mut out = m.clone();
    // An empty matrix is its own flip (cycle 13b).
    let cols = if m.data.is_empty() { 0 } else { m.cols };
    for c in 0..cols {
        for r in 0..m.rows {
            out.set(m.rows - 1 - r, c, m.get(r, c));
        }
    }
    one_as(out)
}

// ---- search and sort -------------------------------------------------

/// `find(X)`, `find(X, n)`, `find(X, n, 'first')` and `find(X, n, 'last')`.
/// The last `n` indices stay in ascending order, as MATLAB returns them, and
/// the result is a row for a row vector and a column otherwise.
///
/// `find([])` is `0x0` rather than the `0x1` "otherwise" would give: a `0x0`
/// input has no orientation to keep, which MATLAB reflects in the result.
/// `find(zeros(1, 0))` is still the `1x0` its row shape asks for.
///
/// Asked for two outputs, `find` gives the row and column subscripts of the
/// same elements instead of their linear indices, and asked for three, their
/// values as well, in the argument's class. All of them take the shape the
/// one output would have had.
fn find(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 3, "find")?;
    let m = mat(a, 0, "find")?;
    let mut idx: Vec<f64> = m
        .data
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != 0.0)
        .map(|(i, _)| (i + 1) as f64)
        .collect();
    if a.len() >= 2 {
        let n = match (option(a, 1), mat(a, 1, "find")?.scalar_value()) {
            (None, Some(n)) if n >= 1.0 && n.fract() == 0.0 => n,
            _ => return Err(error::find_count()),
        };
        let last = match option(a, 2) {
            None if a.len() < 3 => false,
            Some(d) if d.eq_ignore_ascii_case("first") => false,
            Some(d) if d.eq_ignore_ascii_case("last") => true,
            _ => return Err(error::find_direction()),
        };
        // `n` is at least 1, and saturates harmlessly past the index count.
        let n = n.min(idx.len() as f64) as usize;
        if last {
            idx.drain(..idx.len() - n);
        } else {
            idx.truncate(n);
        }
    }
    let shape = |data: Vec<f64>| {
        if m.rows == 0 && m.cols == 0 {
            Matrix::empty()
        } else if m.rows == 1 {
            Matrix::row(data)
        } else {
            Matrix::col(data)
        }
    };
    if nargout < 2 {
        return one_mat(shape(idx));
    }
    // `idx` is one-based and column-major: element `k` is at row
    // `(k - 1) % rows` and column `(k - 1) / rows`, both zero-based.
    let at = |k: f64| k as usize - 1;
    let rows = shape(idx.iter().map(|&k| (at(k) % m.rows + 1) as f64).collect());
    let cols = shape(idx.iter().map(|&k| (at(k) / m.rows + 1) as f64).collect());
    let mut out = vec![Value::Mat(rows), Value::Mat(cols)];
    if nargout >= 3 {
        let vals = shape(idx.iter().map(|&k| m.data[at(k)]).collect());
        out.push(Value::Mat(vals.with_class(m.class)));
    }
    Ok(out)
}

/// A total order over doubles for sorting: NaN compares greater than every
/// number and equal to itself, so it lands at the end. `partial_cmp` returns
/// `None` for a NaN pair, and mapping that to `Equal` is *not* a total order,
/// which is why the old comparator misplaced the finite elements too.
fn sort_cmp(a: &f64, b: &f64) -> Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
    }
}

/// `sort(A)`, `sort(A, direction)`, `sort(A, dim)` and
/// `sort(A, dim, direction)`. Since cycle 09 `A` may be a matrix: each
/// column is sorted on its own by default (the first dimension that is not
/// a singleton, so a row vector sorts along its length), and each row with
/// `dim` 2. Along a dimension past the second every slice has one element
/// and nothing moves.
///
/// Descending is `sort_cmp` reversed rather than the ascending result
/// reversed: that keeps it stable, as the MATLAB page requires "regardless of
/// sorting direction", and puts `NaN` first, the documented placement.
///
/// Asked for two outputs, the second is the permutation within each slice,
/// a double of the same shape: for a matrix sorted along its columns,
/// `s(:, j) = A(i(:, j), j)`. The sort is of the positions, keyed by value,
/// so the order the values take and the order the indices record are one
/// and the same. Along a dimension past the second every index is `1`.
fn sort(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 3, "sort")?;
    let m = mat(a, 0, "sort")?;
    // The second argument is a direction when it is a char, and a dimension
    // otherwise; the third can only be a direction.
    let (d, dir) = match a.len() {
        1 => (None, None),
        2 if option(a, 1).is_some() => (None, Some(1)),
        2 => (Some(dim(a, 1, "sort")?), None),
        _ => (Some(dim(a, 1, "sort")?), Some(2)),
    };
    let descend = match dir.map(|i| option(a, i)) {
        None => false,
        Some(Some(s)) if s.eq_ignore_ascii_case("ascend") => false,
        Some(Some(s)) if s.eq_ignore_ascii_case("descend") => true,
        Some(_) => return Err(error::sort_direction()),
    };
    let along = d.unwrap_or(if m.rows == 1 { 2 } else { 1 });
    let (rows, cols) = (m.rows, m.cols);
    // Each slice as its linear positions: a column is `rows` consecutive
    // elements, a row is every `rows`-th.
    let slices: Vec<Vec<usize>> = match along {
        // An empty matrix has no slices worth sorting, however long its
        // other dimension (cycle 13b).
        _ if rows * cols == 0 => Vec::new(),
        1 => (0..cols)
            .map(|c| (c * rows..(c + 1) * rows).collect())
            .collect(),
        2 => (0..rows)
            .map(|r| (0..cols).map(|c| c * rows + r).collect())
            .collect(),
        _ => Vec::new(),
    };
    let mut out = m;
    let mut index = vec![1.0; out.numel()];
    let data = out.data.clone();
    for pos in &slices {
        let mut perm: Vec<usize> = (0..pos.len()).collect();
        if descend {
            perm.sort_by(|&x, &y| sort_cmp(&data[pos[y]], &data[pos[x]]));
        } else {
            perm.sort_by(|&x, &y| sort_cmp(&data[pos[x]], &data[pos[y]]));
        }
        for (k, &p) in perm.iter().enumerate() {
            out.data[pos[k]] = data[pos[p]];
            index[pos[k]] = (p + 1) as f64;
        }
    }
    let index = Matrix::new(rows, cols, index);
    // A sorted char is a char: `sort('cab')` is `'abc'`.
    if nargout < 2 {
        return one_as(out);
    }
    Ok(vec![Value::Mat(out), Value::Mat(index)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(f: crate::builtins::BuiltinFn, args: &[Value]) -> R<Matrix> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        Ok(f(&mut it, args, 1)?[0].clone().into_mat().unwrap())
    }

    fn row(v: &[f64]) -> Value {
        Value::Mat(Matrix::row(v.to_vec()))
    }

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    /// `trace` goes through `math::sum0`, whose empty sum is `+0`. Rust's own
    /// `.sum()` starts from `-0.0`, so `fprintf('%.4f', trace([]))` printed
    /// `-0.0000` where MATLAB gives `0`. Cycle 01c fixed `sum`, `mean`,
    /// `norm` and `dot` this way and missed this one.
    #[test]
    fn trace_of_an_empty_is_positive_zero() {
        let t = call(trace, &[Value::Mat(Matrix::empty())]).unwrap();
        assert_eq!(t.data, [0.0]);
        assert!(t.data[0].is_sign_positive(), "trace([]) is -0");
        // The ordinary answers are unchanged.
        let a = Value::Mat(Matrix::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]));
        assert_eq!(call(trace, &[a]).unwrap().data, [5.0]);
        assert_eq!(
            call(trace, &[Value::Mat(Matrix::identity(3, 3))])
                .unwrap()
                .data,
            [3.0]
        );
        // A non-square matrix is still refused.
        assert!(call(trace, &[row(&[1.0, 2.0])]).is_err());
    }

    #[test]
    fn sort_puts_nan_last_without_disturbing_the_finite_values() {
        let got = call(sort, &[row(&[5.0, 4.0, f64::NAN, 2.0, 1.0])]).unwrap();
        assert_eq!(got.data[..4], [1.0, 2.0, 4.0, 5.0]);
        assert!(got.data[4].is_nan());
        // Several NaNs, and NaN at either end of the input.
        let got = call(sort, &[row(&[f64::NAN, 3.0, f64::NAN, 1.0])]).unwrap();
        assert_eq!(got.data[..2], [1.0, 3.0]);
        assert!(got.data[2].is_nan() && got.data[3].is_nan());
        // The shape and orientation are kept.
        assert_eq!((got.rows, got.cols), (1, 4));
        let all_nan = call(sort, &[row(&[f64::NAN, f64::NAN])]).unwrap();
        assert!(all_nan.data.iter().all(|v| v.is_nan()));
        // Infinities are ordinary values and sort before NaN.
        let got = call(sort, &[row(&[f64::INFINITY, 1.0, f64::NEG_INFINITY])]).unwrap();
        assert_eq!(got.data, [f64::NEG_INFINITY, 1.0, f64::INFINITY]);
    }

    #[test]
    fn sort_cmp_is_a_total_order() {
        let xs = [f64::NAN, 1.0, -1.0, f64::INFINITY, 0.0];
        for a in xs {
            assert_eq!(sort_cmp(&a, &a), Ordering::Equal);
            for b in xs {
                assert_eq!(sort_cmp(&a, &b), sort_cmp(&b, &a).reverse());
            }
        }
    }

    #[test]
    fn rearrangement_keeps_column_major_order() {
        // reshape fills column by column, so 1:6 becomes [1 3 5; 2 4 6].
        let got = call(
            reshape,
            &[row(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), num(2.0), num(3.0)],
        )
        .unwrap();
        assert_eq!((got.rows, got.cols), (2, 3));
        assert_eq!(got.get(0, 1), 3.0);
        assert_eq!(got.data, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let got = call(fliplr, &[row(&[1.0, 2.0, 3.0])]).unwrap();
        assert_eq!(got.data, [3.0, 2.0, 1.0]);
        let got = call(repmat, &[row(&[1.0, 2.0]), num(2.0), num(2.0)]).unwrap();
        assert_eq!((got.rows, got.cols), (2, 4));
    }

    #[test]
    fn a_size_that_would_overflow_is_an_error_not_a_panic() {
        let e = call(reshape, &[num(1.0), num(1e10), num(1e10)])
            .unwrap_err()
            .msg;
        assert!(e.contains("10000000000x10000000000"), "{e}");
        // repmat names the array it was asked for, not an intermediate: a
        // 1x2 tiled 1e10 by 1e10 is 10000000000x20000000000.
        let e = call(repmat, &[row(&[1.0, 2.0]), num(1e10), num(1e10)])
            .unwrap_err()
            .msg;
        assert!(e.contains("10000000000x20000000000"), "{e}");
        // A huge count with an empty source is still an empty array, as in
        // MATLAB, because the product is zero.
        let got = call(repmat, &[Value::Mat(Matrix::new(1, 0, vec![])), num(1e6)]).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn arity_is_checked_at_both_ends() {
        assert_eq!(
            call(inv, &[num(1.0), num(2.0)]).unwrap_err().msg,
            "Too many input arguments."
        );
        assert!(call(dot, &[row(&[1.0, 2.0])]).is_err());
        assert!(call(reshape, &[num(1.0), num(1.0)]).is_err());
        let v = row(&[2.0, 1.0]);
        assert!(call(sort, &[v.clone(), num(2.0), text("ascend"), num(1.0)]).is_err());
        assert!(call(find, &[v.clone(), num(1.0), text("first"), num(1.0)]).is_err());
        assert!(call(norm, &[v.clone(), num(1.0), num(1.0)]).is_err());
        assert!(call(diag, &[v.clone(), num(1.0), num(1.0)]).is_err());
        assert!(call(dot, &[v.clone(), v.clone(), num(1.0), num(1.0)]).is_err());
    }

    fn text(s: &str) -> Value {
        Value::str(s)
    }

    fn col(v: &[f64]) -> Value {
        Value::Mat(Matrix::col(v.to_vec()))
    }

    fn mat(rows: usize, cols: usize, row_major: &[f64]) -> Value {
        let mut m = Matrix::filled(rows, cols, 0.0);
        for (i, v) in row_major.iter().enumerate() {
            m.set(i / cols, i % cols, *v);
        }
        Value::Mat(m)
    }

    // ---- sort --------------------------------------------------------

    #[test]
    fn sort_takes_a_direction_and_a_dimension() {
        let v = row(&[3.0, 1.0, 2.0]);
        assert_eq!(
            call(sort, &[v.clone(), text("descend")]).unwrap().data,
            [3.0, 2.0, 1.0]
        );
        assert_eq!(
            call(sort, &[v.clone(), text("ascend")]).unwrap().data,
            [1.0, 2.0, 3.0]
        );
        let d = call(sort, &[v.clone(), num(2.0), text("descend")]).unwrap();
        assert_eq!(d.data, [3.0, 2.0, 1.0]);
        // Along a dimension the vector does not extend in, nothing moves.
        assert_eq!(
            call(sort, &[v.clone(), num(1.0)]).unwrap().data,
            [3.0, 1.0, 2.0]
        );
        assert_eq!(
            call(sort, &[v.clone(), num(3.0)]).unwrap().data,
            [3.0, 1.0, 2.0]
        );
        let c = call(sort, &[col(&[3.0, 1.0, 2.0]), text("descend")]).unwrap();
        assert_eq!((c.rows, c.cols), (3, 1));
        assert_eq!(c.data, [3.0, 2.0, 1.0]);
        assert_eq!(
            call(sort, &[col(&[3.0, 1.0]), num(2.0)]).unwrap().data,
            [3.0, 1.0]
        );
        let dir = "Sort direction for 'sort' must be 'ascend' or 'descend'.";
        assert_eq!(call(sort, &[v.clone(), text("up")]).unwrap_err().msg, dir);
        assert_eq!(
            call(sort, &[v.clone(), num(1.0), num(2.0)])
                .unwrap_err()
                .msg,
            dir
        );
        assert_eq!(
            call(sort, &[v.clone(), num(1.0), text("up")])
                .unwrap_err()
                .msg,
            dir
        );
        // A char in the dimension position of the three-argument form is a
        // bad dimension, never a character code.
        let e = call(sort, &[v, text("x"), text("ascend")]).unwrap_err().msg;
        assert!(e.contains("Dimension argument"), "{e}");
    }

    /// Cycle 09: a matrix sorts each column by default, each row along
    /// dimension 2, and the permutation is per slice.
    #[test]
    fn sort_of_a_matrix_sorts_each_slice() {
        // [3 1; 2 4], column-major [3 2 1 4].
        let a = mat(2, 2, &[3.0, 1.0, 2.0, 4.0]);
        assert_eq!(
            call(sort, std::slice::from_ref(&a)).unwrap().data,
            [2.0, 3.0, 1.0, 4.0]
        );
        assert_eq!(
            call(sort, &[a.clone(), num(2.0)]).unwrap().data,
            [1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(
            call(sort, &[a.clone(), text("descend")]).unwrap().data,
            [3.0, 2.0, 4.0, 1.0]
        );
        assert_eq!(
            call(sort, &[a.clone(), num(2.0), text("descend")])
                .unwrap()
                .data,
            [3.0, 4.0, 1.0, 2.0]
        );
        assert_eq!(
            call(sort, &[a.clone(), num(3.0)]).unwrap().data,
            [3.0, 2.0, 1.0, 4.0]
        );
        let out = outputs(sort, std::slice::from_ref(&a), 2);
        assert_eq!(out[1].data, [2.0, 1.0, 1.0, 2.0]);
        let out = outputs(sort, &[a, num(2.0)], 2);
        assert_eq!(out[1].data, [2.0, 1.0, 1.0, 2.0]);
        // NaN goes last in each column ascending, first descending, and the
        // sort is stable within a column.
        let n = mat(3, 2, &[f64::NAN, 5.0, 1.0, f64::NAN, 1.0, 5.0]);
        let up = call(sort, std::slice::from_ref(&n)).unwrap();
        assert_eq!(up.data[..2], [1.0, 1.0]);
        assert!(up.data[2].is_nan());
        assert_eq!(up.data[3..5], [5.0, 5.0]);
        let down = call(sort, &[n, text("descend")]).unwrap();
        assert!(down.data[0].is_nan());
        assert_eq!(down.data[1..3], [1.0, 1.0]);
        // A char matrix stays a char; an empty stays its shape.
        let c = call(sort, &[text("ba")]).unwrap();
        assert!(c.is_char());
        let e = call(sort, &[Value::Mat(Matrix::new(0, 3, vec![]))]).unwrap();
        assert_eq!((e.rows, e.cols), (0, 3));
    }

    #[test]
    fn sort_descending_puts_nan_first() {
        let got = call(sort, &[row(&[3.0, f64::NAN, 1.0, 2.0]), text("descend")]).unwrap();
        assert!(got.data[0].is_nan());
        assert_eq!(got.data[1..], [3.0, 2.0, 1.0]);
        let got = call(
            sort,
            &[
                row(&[f64::NEG_INFINITY, f64::NAN, f64::INFINITY]),
                text("descend"),
            ],
        )
        .unwrap();
        assert!(got.data[0].is_nan());
        assert_eq!(got.data[1..], [f64::INFINITY, f64::NEG_INFINITY]);
    }

    #[test]
    fn sort_is_stable_in_both_directions() {
        // -0 and +0 compare equal, so their sign bits show whether equal
        // elements keep their input order. The MATLAB page: stable
        // "regardless of sorting direction". Reversing an ascending sort
        // would get the descending case wrong.
        let signs =
            |m: &Matrix| -> Vec<bool> { m.data.iter().map(|x| x.is_sign_negative()).collect() };
        let v = row(&[-0.0, 0.0, -0.0, 1.0]);
        let down = call(sort, &[v.clone(), text("descend")]).unwrap();
        assert_eq!(down.data[0], 1.0);
        assert_eq!(signs(&down)[1..], [true, false, true]);
        let up = call(sort, &[v, text("ascend")]).unwrap();
        assert_eq!(up.data[3], 1.0);
        assert_eq!(signs(&up)[..3], [true, false, true]);
        let v = row(&[0.0, f64::NAN, -0.0]);
        let down = call(sort, &[v, text("descend")]).unwrap();
        assert!(down.data[0].is_nan());
        assert_eq!(signs(&down)[1..], [false, true]);
    }

    // ---- find --------------------------------------------------------

    #[test]
    fn find_takes_a_count_and_an_end() {
        let x = row(&[0.0, 1.0, 1.0, 1.0, 0.0, 1.0]);
        assert_eq!(call(find, &[x.clone(), num(2.0)]).unwrap().data, [2.0, 3.0]);
        let last = call(find, &[x.clone(), num(2.0), text("last")]).unwrap();
        assert_eq!(last.data, [4.0, 6.0]);
        assert_eq!((last.rows, last.cols), (1, 2));
        let first = call(find, &[x.clone(), num(2.0), text("first")]).unwrap();
        assert_eq!(first.data, [2.0, 3.0]);
        // More than there are is all of them; a column stays a column.
        let c = call(find, &[col(&[0.0, 1.0, 1.0]), num(5.0)]).unwrap();
        assert_eq!((c.rows, c.cols), (2, 1));
        assert_eq!(c.data, [2.0, 3.0]);
        let c = call(find, &[col(&[0.0, 1.0, 1.0]), num(5.0), text("last")]).unwrap();
        assert_eq!(c.data, [2.0, 3.0]);
        let one = call(find, &[col(&[0.0, 1.0, 1.0]), num(1.0)]).unwrap();
        assert_eq!((one.rows, one.cols), (1, 1));
        let none = call(find, &[row(&[0.0, 0.0]), num(1.0)]).unwrap();
        assert_eq!((none.rows, none.cols), (1, 0));
        let count = "Number of elements for 'find' must be a positive integer scalar.";
        for bad in [0.0, -1.0, 1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                call(find, &[x.clone(), num(bad)]).unwrap_err().msg,
                count,
                "{bad}"
            );
        }
        assert_eq!(call(find, &[x.clone(), text("a")]).unwrap_err().msg, count);
        let dir = "Search direction for 'find' must be 'first' or 'last'.";
        let e = call(find, &[x.clone(), num(1.0), text("middle")])
            .unwrap_err()
            .msg;
        assert_eq!(e, dir);
        assert_eq!(call(find, &[x, num(1.0), num(1.0)]).unwrap_err().msg, dir);
    }

    // ---- norm --------------------------------------------------------

    /// The textbook p-norm, with no scaling.
    fn naive(v: &[f64], p: f64) -> f64 {
        v.iter().map(|x| x.abs().powf(p)).sum::<f64>().powf(1.0 / p)
    }

    #[test]
    fn norm_takes_every_vector_order() {
        let v = row(&[3.0, -4.0]);
        let n = |p: Value| call(norm, &[v.clone(), p]).unwrap().data[0];
        assert_eq!(n(num(1.0)), 7.0);
        assert!((n(num(2.0)) - 5.0).abs() < 1e-12);
        assert_eq!(n(num(f64::INFINITY)), 4.0);
        assert_eq!(n(num(f64::NEG_INFINITY)), 3.0);
        assert!((n(num(3.0)) - 91f64.cbrt()).abs() < 1e-12);
        assert!((n(num(0.5)) - naive(&[3.0, 4.0], 0.5)).abs() < 1e-9);
        assert!((n(text("fro")) - 5.0).abs() < 1e-12);
        assert_eq!(n(text("inf")), 4.0);
        assert_eq!(n(text("Inf")), 4.0);
        let bad = "Norm type for 'norm' must be a positive real scalar, Inf, -Inf or 'fro'.";
        for p in [
            num(0.0),
            num(-1.0),
            num(f64::NAN),
            text("abc"),
            row(&[1.0, 2.0]),
        ] {
            assert_eq!(call(norm, &[v.clone(), p]).unwrap_err().msg, bad);
        }
        // A matrix has its own norms since cycle 08: [1 2; 3 4] has column
        // sums 4 and 6.
        let a = mat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(call(norm, &[a.clone(), num(1.0)]).unwrap().data, [6.0]);
    }

    #[test]
    fn the_scaled_norm_agrees_with_the_naive_one_where_both_are_finite() {
        let cases: [&[f64]; 4] = [
            &[3.0, -4.0],
            &[1.0, 2.0, 3.0, 4.0, 5.0],
            &[0.001, -0.5, 7.25, 0.0],
            &[1e10, 3e9, -2e10],
        ];
        for v in cases {
            for p in [1.0, 1.5, 2.0, 3.0, 7.0] {
                let got = vector_norm(v, NormType::P(p));
                let want = naive(v, p);
                assert!(
                    (got - want).abs() <= 1e-12 * want,
                    "{v:?} p={p}: {got} vs {want}"
                );
            }
        }
    }

    #[test]
    fn the_scaled_norm_neither_overflows_nor_underflows() {
        let r2 = std::f64::consts::SQRT_2;
        let big = vector_norm(&[1e200, 1e200], NormType::P(2.0));
        assert!((big / 1e200 - r2).abs() < 1e-12, "{big}");
        let small = vector_norm(&[1e-200, 1e-200], NormType::P(2.0));
        assert!((small / 1e-200 - r2).abs() < 1e-12, "{small}");
        let cube = vector_norm(&[1e200, 1e200], NormType::P(3.0));
        assert!((cube / 1e200 - 2f64.cbrt()).abs() < 1e-12, "{cube}");
        // The naive formula really does fail here, which is the point.
        assert!(naive(&[1e200, 1e200], 2.0).is_infinite());
        assert_eq!(naive(&[1e-200, 1e-200], 2.0), 0.0);
        // Where the largest magnitude is not a finite non-zero number, it is
        // the answer; and an empty sum is +0.
        for p in [
            NormType::P(1.0),
            NormType::P(2.0),
            NormType::P(3.0),
            NormType::Inf,
        ] {
            let z = vector_norm(&[], p);
            assert!(z == 0.0 && z.is_sign_positive(), "{p:?}");
            assert_eq!(vector_norm(&[0.0, 0.0], p), 0.0);
            assert_eq!(vector_norm(&[1.0, f64::INFINITY], p), f64::INFINITY);
            assert!(vector_norm(&[f64::INFINITY, f64::NAN], p).is_nan());
        }
        assert_eq!(vector_norm(&[], NormType::NegInf), 0.0);
        assert!(vector_norm(&[f64::NAN, 1.0], NormType::NegInf).is_nan());
    }

    // ---- diag --------------------------------------------------------

    #[test]
    fn diag_places_a_vector_on_the_kth_diagonal() {
        let up = call(diag, &[row(&[1.0, 2.0]), num(1.0)]).unwrap();
        assert_eq!((up.rows, up.cols), (3, 3));
        assert_eq!((up.get(0, 1), up.get(1, 2)), (1.0, 2.0));
        assert_eq!(up.data.iter().sum::<f64>(), 3.0);
        let down = call(diag, &[col(&[1.0, 2.0]), num(-1.0)]).unwrap();
        assert_eq!((down.get(1, 0), down.get(2, 1)), (1.0, 2.0));
        let s = call(diag, &[num(5.0), num(1.0)]).unwrap();
        assert_eq!(s.data, [0.0, 0.0, 5.0, 0.0]);
        // An empty vector is a vector: diag of it is an empty square matrix,
        // or a square of zeros once there is an offset.
        let e = call(diag, &[Value::Mat(Matrix::new(1, 0, vec![]))]).unwrap();
        assert_eq!((e.rows, e.cols), (0, 0));
        let e = call(diag, &[Value::Mat(Matrix::new(1, 0, vec![])), num(1.0)]).unwrap();
        assert_eq!(e.data, [0.0]);
        let e = call(diag, &[row(&[1.0, 2.0]), num(1e300)]).unwrap_err().msg;
        assert!(e.contains("1e+300x1e+300"), "{e}");
    }

    #[test]
    fn diag_reads_the_kth_diagonal_of_a_matrix() {
        let a = mat(3, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        let d = |k: f64| call(diag, &[a.clone(), num(k)]).unwrap();
        assert_eq!(d(0.0).data, [1.0, 5.0, 9.0]);
        assert_eq!(d(1.0).data, [2.0, 6.0]);
        assert_eq!(d(-2.0).data, [7.0]);
        assert_eq!((d(1.0).rows, d(1.0).cols), (2, 1));
        // Past the matrix, either way, is a 0x1.
        for k in [3.0, -3.0, 1e300, -1e300] {
            let e = d(k);
            assert_eq!((e.rows, e.cols), (0, 1), "{k}");
        }
        let wide = mat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(call(diag, &[wide.clone(), num(-1.0)]).unwrap().data, [4.0]);
        assert_eq!(call(diag, &[wide, num(2.0)]).unwrap().data, [3.0]);
        let bad = "K-th diagonal input must be an integer scalar.";
        for k in [
            num(1.5),
            num(f64::NAN),
            num(f64::INFINITY),
            text("a"),
            row(&[1.0, 2.0]),
        ] {
            assert_eq!(call(diag, &[a.clone(), k]).unwrap_err().msg, bad);
        }
    }

    // ---- dot ---------------------------------------------------------

    #[test]
    fn dot_of_matrices_is_column_wise() {
        let a = mat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            call(dot, &[a.clone(), a.clone()]).unwrap(),
            Matrix::row(vec![10.0, 20.0])
        );
        let by_row = call(dot, &[a.clone(), a.clone(), num(2.0)]).unwrap();
        assert_eq!(by_row, Matrix::col(vec![5.0, 25.0]));
        // Vectors may differ in orientation without a dimension ...
        let v = call(dot, &[row(&[1.0, 2.0, 3.0]), col(&[4.0, 5.0, 6.0])]).unwrap();
        assert_eq!(v, Matrix::scalar(32.0));
        // ... but with one they must match, and the dimension is honoured.
        assert_eq!(
            call(dot, &[row(&[1.0, 2.0]), row(&[3.0, 4.0]), num(1.0)]).unwrap(),
            Matrix::row(vec![3.0, 8.0])
        );
        assert_eq!(
            call(dot, &[row(&[1.0, 2.0]), row(&[3.0, 4.0]), num(2.0)]).unwrap(),
            Matrix::scalar(11.0)
        );
        let size = "A and B must be the same size for 'dot'.";
        let e = call(dot, &[row(&[1.0, 2.0]), col(&[3.0, 4.0]), num(1.0)]).unwrap_err();
        assert_eq!(e.msg, size);
        let e = call(
            dot,
            &[
                Value::Mat(Matrix::filled(2, 3, 1.0)),
                Value::Mat(Matrix::filled(3, 2, 1.0)),
            ],
        )
        .unwrap_err();
        assert_eq!(e.msg, size);
        assert_eq!(
            call(dot, &[row(&[1.0, 2.0]), row(&[1.0, 2.0, 3.0])])
                .unwrap_err()
                .msg,
            size
        );
        // The empty dot product is +0.
        let e = call(
            dot,
            &[Value::Mat(Matrix::empty()), Value::Mat(Matrix::empty())],
        )
        .unwrap();
        assert!(e.data[0] == 0.0 && e.data[0].is_sign_positive());
        assert!(call(dot, &[a.clone(), a, text("x")]).is_err());
    }

    // ---- reshape and repmat ------------------------------------------

    fn shape_of(f: crate::builtins::BuiltinFn, args: &[Value]) -> (usize, usize) {
        let m = call(f, args).unwrap();
        (m.rows, m.cols)
    }

    #[test]
    fn reshape_takes_a_size_vector_and_one_placeholder() {
        let v = row(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let empty = Value::Mat(Matrix::empty());
        let got = call(reshape, &[v.clone(), row(&[3.0, 2.0])]).unwrap();
        assert_eq!((got.rows, got.cols), (3, 2));
        assert_eq!(got.data, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(
            shape_of(reshape, &[v.clone(), empty.clone(), num(2.0)]),
            (3, 2)
        );
        assert_eq!(
            shape_of(reshape, &[v.clone(), num(2.0), empty.clone()]),
            (2, 3)
        );
        assert_eq!(
            shape_of(reshape, &[v.clone(), num(3.0), num(2.0), num(1.0)]),
            (3, 2)
        );
        assert_eq!(
            shape_of(reshape, &[v.clone(), num(3.0), num(2.0), empty.clone()]),
            (3, 2)
        );
        assert_eq!(
            shape_of(reshape, &[v.clone(), row(&[3.0, 2.0, 1.0])]),
            (3, 2)
        );
        let five = row(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(
            call(reshape, &[five, empty.clone(), num(2.0)])
                .unwrap_err()
                .msg,
            "Product of known dimensions, 2, not divisible into total number of elements, 5."
        );
        assert_eq!(
            call(reshape, &[v.clone(), empty.clone(), empty.clone()])
                .unwrap_err()
                .msg,
            "Size can only have one unknown dimension."
        );
        // Cycle 14: to and from N-D, a placeholder anywhere, and the count
        // message naming every size.
        assert_eq!(
            call(reshape, &[v.clone(), num(3.0), num(2.0), num(2.0)])
                .unwrap_err()
                .msg,
            "To reshape the number of elements must not change (6 vs 3x2x2)."
        );
        let nd = call(reshape, &[v.clone(), num(1.0), num(2.0), num(3.0)]).unwrap();
        assert_eq!(
            (nd.dims(), nd.data.clone()),
            (vec![1, 2, 3], v.mat().unwrap().data.clone())
        );
        let back = call(reshape, &[Value::Mat(nd.clone()), empty.clone(), num(2.0)]).unwrap();
        assert_eq!(back.dims(), [3, 2]);
        // An empty shape whose sizes multiply past `usize` before the 0 is
        // counted as the 0 it is, never overflowing.
        let huge = 2f64.powi(40);
        let e = call(reshape, &[empty.clone(), num(huge), num(huge), num(0.0)]).unwrap();
        assert_eq!(e.dims(), [1 << 40, 1 << 40, 0]);
        let mid = call(
            reshape,
            &[Value::Mat(nd), num(1.0), empty.clone(), num(2.0)],
        )
        .unwrap();
        assert_eq!(mid.dims(), [1, 3, 2]);
        // A single size must be a vector of at least two, not n by n.
        assert!(call(reshape, &[v.clone(), num(6.0)]).is_err());
        assert!(call(reshape, &[v.clone(), empty.clone()]).is_err());
        let e = call(reshape, &[v.clone(), col(&[3.0, 2.0])])
            .unwrap_err()
            .msg;
        assert_eq!(e, "Size vector for 'reshape' must be a row vector.");
        // The count still has to match.
        let e = call(reshape, &[v, num(2.0), num(2.0)]).unwrap_err().msg;
        assert!(e.contains("6 vs 2x2"), "{e}");
        // An empty with a zero known size: Octave's 0.
        let z = Value::Mat(Matrix::filled(0, 3, 0.0));
        assert_eq!(shape_of(reshape, &[z, empty, num(0.0)]), (0, 0));
    }

    #[test]
    fn repmat_takes_a_size_vector_and_trailing_ones() {
        let v = row(&[1.0, 2.0]);
        let got = call(repmat, &[v.clone(), row(&[2.0, 2.0])]).unwrap();
        assert_eq!((got.rows, got.cols), (2, 4));
        assert_eq!(got.data, [1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0]);
        assert_eq!(shape_of(repmat, &[num(1.0), row(&[2.0, 3.0])]), (2, 3));
        assert_eq!(
            shape_of(repmat, &[num(1.0), num(2.0), num(3.0), num(1.0)]),
            (2, 3)
        );
        assert_eq!(shape_of(repmat, &[v.clone(), num(2.0)]), (2, 4));
        assert_eq!(
            call(repmat, &[v, num(2.0), num(3.0), num(2.0)])
                .unwrap_err()
                .msg,
            "N-D arrays are not supported."
        );
    }

    // ---- more than one output (cycle 03) ------------------------------

    use crate::value::Class;

    fn outputs(f: crate::builtins::BuiltinFn, args: &[Value], nargout: usize) -> Vec<Matrix> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        f(&mut it, args, nargout)
            .unwrap()
            .into_iter()
            .map(|v| v.into_mat().unwrap())
            .collect()
    }

    #[test]
    fn sort_gives_the_permutation_as_a_second_output() {
        let out = outputs(sort, &[row(&[3.0, 1.0, 2.0])], 2);
        assert_eq!(out[0].data, [1.0, 2.0, 3.0]);
        assert_eq!(out[1].data, [2.0, 3.0, 1.0]);
        assert_eq!(
            (out[1].rows, out[1].cols, out[1].class),
            (1, 3, Class::Double)
        );
        // Stable in both directions, with NaN at the high end.
        let v = row(&[2.0, f64::NAN, 1.0, 2.0]);
        let out = outputs(sort, std::slice::from_ref(&v), 2);
        assert_eq!(out[1].data, [3.0, 1.0, 4.0, 2.0]);
        let out = outputs(sort, &[v, text("descend")], 2);
        assert_eq!(out[1].data, [2.0, 1.0, 4.0, 3.0]);
        // A column keeps its shape; a dimension it does not extend in moves
        // nothing and gives ones.
        let out = outputs(sort, &[col(&[5.0, 4.0])], 2);
        assert_eq!(
            (out[1].rows, out[1].cols, out[1].data.clone()),
            (2, 1, vec![2.0, 1.0])
        );
        let out = outputs(sort, &[row(&[5.0, 4.0]), num(1.0)], 2);
        assert_eq!(
            (out[0].data.clone(), out[1].data.clone()),
            (vec![5.0, 4.0], vec![1.0, 1.0])
        );
        // One output is the value alone, unchanged.
        assert_eq!(outputs(sort, &[row(&[2.0, 1.0])], 1).len(), 1);
        // A char sorts to a char; its permutation is a double.
        let out = outputs(sort, &[text("cab")], 2);
        assert_eq!((out[0].class, out[1].class), (Class::Char, Class::Double));
        assert_eq!(out[1].data, [2.0, 3.0, 1.0]);
    }

    #[test]
    fn find_gives_subscripts_and_values_when_asked() {
        // [0 7; 5 0]: the non-zeros are (2,1) = 5 and (1,2) = 7.
        let a = mat(2, 2, &[0.0, 7.0, 5.0, 0.0]);
        let out = outputs(find, std::slice::from_ref(&a), 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].data, [2.0, 1.0]);
        assert_eq!(out[1].data, [1.0, 2.0]);
        assert_eq!(out[2].data, [5.0, 7.0]);
        assert_eq!((out[0].rows, out[0].cols), (2, 1));
        assert_eq!(outputs(find, std::slice::from_ref(&a), 2).len(), 2);
        // A row gives rows, and the count and direction still apply.
        let r = row(&[0.0, 3.0, 0.0, 4.0]);
        let out = outputs(find, &[r, num(1.0), text("last")], 2);
        assert_eq!(
            (out[0].data.clone(), out[1].data.clone()),
            (vec![1.0], vec![4.0])
        );
        assert_eq!((out[1].rows, out[1].cols), (1, 1));
        // The values keep the argument's class; a 0x0 gives three 0x0s.
        let t = Value::Mat(Matrix::row(vec![1.0, 0.0]).with_class(Class::Logical));
        assert_eq!(outputs(find, &[t], 3)[2].class, Class::Logical);
        let e = outputs(find, &[Value::Mat(Matrix::empty())], 3);
        assert!(e.iter().all(|m| (m.rows, m.cols) == (0, 0)));
    }

    // ---- cycle 08: factorisations and constructions ------------------

    fn err(f: crate::builtins::BuiltinFn, args: &[Value], nargout: usize) -> String {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        f(&mut it, args, nargout).unwrap_err().msg
    }

    fn near(a: &Matrix, b: &Matrix, tol: f64) -> bool {
        (a.rows, a.cols) == (b.rows, b.cols)
            && a.data
                .iter()
                .zip(&b.data)
                .all(|(x, y)| (x - y).abs() <= tol)
    }

    #[test]
    fn magic_squares_have_equal_sums_and_matlabs_layout() {
        let m3 = call(magic, &[num(3.0)]).unwrap();
        assert_eq!(
            m3,
            Matrix::new(3, 3, vec![8.0, 3.0, 4.0, 1.0, 5.0, 9.0, 6.0, 7.0, 2.0])
        );
        let m4 = call(magic, &[num(4.0)]).unwrap();
        assert_eq!(m4.data[..4], [16.0, 5.0, 9.0, 4.0]);
        for n in 1..=12usize {
            let m = magic_square(n);
            let want = (n * (n * n + 1) / 2) as f64;
            let mut seen: Vec<f64> = m.data.clone();
            seen.sort_by(f64::total_cmp);
            let all: Vec<f64> = (1..=n * n).map(|k| k as f64).collect();
            assert_eq!(seen, all, "magic({n}) holds 1..n^2");
            if n == 2 {
                continue; // no 2x2 magic square exists
            }
            for i in 0..n {
                let r: f64 = (0..n).map(|j| m.get(i, j)).sum();
                let c: f64 = (0..n).map(|j| m.get(j, i)).sum();
                assert_eq!((r, c), (want, want), "row and column {i} of {n}");
            }
            assert_eq!((0..n).map(|i| m.get(i, i)).sum::<f64>(), want);
            assert_eq!((0..n).map(|i| m.get(i, n - 1 - i)).sum::<f64>(), want);
        }
        assert!(call(magic, &[num(0.0)]).unwrap().is_empty());
        assert_eq!(call(magic, &[num(3.7)]).unwrap(), m3);
        let bad = "Order for 'magic' must be a real scalar.";
        assert_eq!(err(magic, &[num(f64::NAN)], 1), bad);
        assert_eq!(err(magic, &[text("a")], 1), bad);
        assert!(err(magic, &[num(1e10)], 1).contains("10000000000x10000000000"));
    }

    #[test]
    fn kron_cross_triu_and_tril() {
        let k = call(kron, &[row(&[1.0, 2.0]), col(&[1.0, 1.0])]).unwrap();
        assert_eq!(k, Matrix::new(2, 2, vec![1.0, 1.0, 2.0, 2.0]));
        let a = mat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let k = call(kron, &[a, Value::Mat(Matrix::identity(2, 2))]).unwrap();
        assert_eq!((k.rows, k.cols), (4, 4));
        assert_eq!((k.get(0, 2), k.get(3, 1), k.get(1, 0)), (2.0, 3.0, 0.0));
        // Two 1e5-element vectors ask for 1e10 elements: judged first.
        let tall = Value::Mat(Matrix::filled(100_000, 1, 1.0));
        let wide = Value::Mat(Matrix::filled(1, 100_000, 1.0));
        assert!(err(kron, &[tall, wide], 1).contains("100000x100000"));

        let c = call(cross, &[row(&[1.0, 0.0, 0.0]), row(&[0.0, 1.0, 0.0])]).unwrap();
        assert_eq!(c, Matrix::row(vec![0.0, 0.0, 1.0]));
        let c = call(cross, &[col(&[1.0, 2.0, 3.0]), col(&[4.0, 5.0, 6.0])]).unwrap();
        assert_eq!(c, Matrix::col(vec![-3.0, 6.0, -3.0]));
        // A 3-by-2 is taken column by column.
        let a = mat(3, 2, &[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let b = mat(3, 2, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        let c = call(cross, &[a, b]).unwrap();
        assert_eq!(c.data, [0.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
        assert_eq!(
            err(cross, &[row(&[1.0, 2.0, 3.0]), col(&[1.0, 2.0, 3.0])], 1),
            "A and B must be the same size for 'cross'."
        );
        assert_eq!(
            err(cross, &[row(&[1.0, 2.0]), row(&[1.0, 2.0])], 1),
            "A and B must have a dimension of length 3 for 'cross'."
        );

        let a = mat(3, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        let one = std::slice::from_ref(&a);
        assert_eq!(
            call(triu, one).unwrap().data,
            [1.0, 0.0, 0.0, 2.0, 5.0, 0.0, 3.0, 6.0, 9.0]
        );
        assert_eq!(
            call(tril, one).unwrap().data,
            [1.0, 4.0, 7.0, 0.0, 5.0, 8.0, 0.0, 0.0, 9.0]
        );
        assert_eq!(
            call(triu, &[a.clone(), num(1.0)]).unwrap().data,
            [0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 3.0, 6.0, 0.0]
        );
        assert_eq!(
            call(tril, &[a.clone(), num(-1.0)]).unwrap().data,
            [0.0, 4.0, 7.0, 0.0, 0.0, 8.0, 0.0, 0.0, 0.0]
        );
        let all = call(triu, &[a.clone(), num(-1e300)]).unwrap();
        assert_eq!(all, call(tril, &[a.clone(), num(1e300)]).unwrap());
        // The class is kept, as a rearrangement keeps it.
        let t = Value::Mat(Matrix::filled(2, 2, 1.0).with_class(Class::Logical));
        assert_eq!(call(triu, &[t]).unwrap().class, Class::Logical);
        assert_eq!(
            err(triu, &[a, num(0.5)], 1),
            "K-th diagonal input must be an integer scalar."
        );
    }

    #[test]
    fn matrix_norms() {
        let a = mat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let n = |p: Option<Value>| {
            let mut args = vec![a.clone()];
            args.extend(p);
            call(norm, &args).unwrap().data[0]
        };
        assert!((n(None) - 5.464985704219043).abs() < 1e-12);
        assert!((n(Some(num(2.0))) - n(None)).abs() < 1e-15);
        assert!((n(Some(text("fro"))) - 30f64.sqrt()).abs() < 1e-12);
        assert_eq!(n(Some(num(1.0))), 6.0);
        assert_eq!(n(Some(num(f64::INFINITY))), 7.0);
        assert_eq!(n(Some(text("inf"))), 7.0);
        let bad = "Matrix norm type for 'norm' must be 1, 2, Inf or 'fro'.";
        for p in [num(3.0), num(f64::NEG_INFINITY), num(0.5)] {
            assert_eq!(err(norm, &[a.clone(), p], 1), bad);
        }
        // NaN makes every matrix norm NaN; Inf makes the 2-norm Inf.
        let nan = mat(2, 2, &[f64::NAN, 1.0, 1.0, 1.0]);
        for p in [num(1.0), num(2.0), num(f64::INFINITY), text("fro")] {
            assert!(call(norm, &[nan.clone(), p]).unwrap().data[0].is_nan());
        }
        let inf = mat(2, 2, &[f64::INFINITY, 1.0, 1.0, 1.0]);
        assert_eq!(call(norm, &[inf]).unwrap().data[0], f64::INFINITY);
        // A vector's 'fro' is still its 2-norm.
        let v = call(norm, &[row(&[3.0, 4.0]), text("fro")]).unwrap();
        assert_eq!(v.data[0], 5.0);
    }

    #[test]
    fn rank_pinv_null_orth_and_cond() {
        let s = mat(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        let one = std::slice::from_ref(&s);
        assert_eq!(call(rank, one).unwrap().data, [1.0]);
        let eye3 = Value::Mat(Matrix::identity(3, 3));
        assert_eq!(call(rank, std::slice::from_ref(&eye3)).unwrap().data, [3.0]);
        assert_eq!(
            call(rank, &[Value::Mat(Matrix::empty())]).unwrap().data,
            [0.0]
        );
        let z2 = Value::Mat(Matrix::filled(2, 2, 0.0));
        assert_eq!(call(rank, std::slice::from_ref(&z2)).unwrap().data, [0.0]);
        assert_eq!(call(rank, &[s.clone(), num(10.0)]).unwrap().data, [0.0]);
        let p = call(pinv, one).unwrap();
        let want = Matrix::new(2, 2, vec![0.04, 0.08, 0.08, 0.16]);
        assert!(near(&p, &want, 1e-14), "{p:?}");
        // pinv of a full-rank tall matrix is a left inverse.
        let t = mat(3, 2, &[1.0, 1.0, 1.0, 2.0, 1.0, 3.0]);
        let pt = call(pinv, std::slice::from_ref(&t)).unwrap();
        assert_eq!((pt.rows, pt.cols), (2, 3));
        let tm = t.into_mat().unwrap();
        let left = pt.matmul(&tm).unwrap();
        assert!(near(&left, &Matrix::identity(2, 2), 1e-13));
        // A long column needs no 100000x100000 U.
        let long = Value::Mat(Matrix::filled(100_000, 1, 1.0));
        let pl = call(pinv, &[long]).unwrap();
        assert_eq!((pl.rows, pl.cols), (1, 100_000));
        assert!((pl.data[0] - 1e-5).abs() < 1e-18);

        let z = call(null, one).unwrap();
        assert_eq!((z.rows, z.cols), (2, 1));
        let sm = s.clone().into_mat().unwrap();
        assert!(sm.matmul(&z).unwrap().data.iter().all(|v| v.abs() < 1e-14));
        assert!((z.data[0].hypot(z.data[1]) - 1.0).abs() < 1e-14);
        let z = call(null, &[Value::Mat(Matrix::identity(2, 2))]).unwrap();
        assert_eq!((z.rows, z.cols), (2, 0));
        let o = call(orth, one).unwrap();
        assert_eq!((o.rows, o.cols), (2, 1));
        assert!((o.data[0] / o.data[1] - 0.5).abs() < 1e-14, "{o:?}");

        let c = call(cond, &[mat(2, 2, &[1.0, 2.0, 3.0, 4.0])])
            .unwrap()
            .data[0];
        assert!((c - 14.933034373659268).abs() < 1e-10);
        assert_eq!(call(cond, &[z2]).unwrap().data, [f64::INFINITY]);
        assert_eq!(
            call(cond, &[Value::Mat(Matrix::empty())]).unwrap().data,
            [0.0]
        );
        assert_eq!(call(cond, &[eye3]).unwrap().data, [1.0]);

        let named: [(crate::builtins::BuiltinFn, &str); 6] = [
            (rank, "rank"),
            (pinv, "pinv"),
            (null, "null"),
            (orth, "orth"),
            (cond, "cond"),
            (svd, "svd"),
        ];
        for (f, name) in named {
            let e = err(f, &[mat(2, 2, &[f64::NAN, 0.0, 0.0, 1.0])], 1);
            assert_eq!(e, format!("Input to '{name}' must not contain NaN or Inf."));
            let e = err(f, &[mat(2, 2, &[f64::INFINITY, 0.0, 0.0, 1.0])], 1);
            assert!(e.contains("NaN or Inf"));
        }
        assert_eq!(
            err(rank, &[s, text("x")], 1),
            "Tolerance for 'rank' must be a real scalar."
        );
    }

    #[test]
    fn eig_chol_lu_qr_and_svd_through_the_builtins() {
        let e = call(eig, &[mat(2, 2, &[2.0, 1.0, 1.0, 2.0])]).unwrap();
        assert_eq!((e.rows, e.cols), (2, 1));
        assert!((e.data[0] - 1.0).abs() < 1e-14 && (e.data[1] - 3.0).abs() < 1e-14);
        let vd = outputs(eig, &[mat(2, 2, &[2.0, 0.0, 0.0, 3.0])], 2);
        assert_eq!(vd[1], Matrix::new(2, 2, vec![2.0, 0.0, 0.0, 3.0]));
        assert_eq!(vd[0], Matrix::identity(2, 2));
        // A complex pair is a pair of complex values since cycle 10, and
        // `[V, D]` gives complex vectors with `A * V = V * D`.
        let e = call(eig, &[mat(2, 2, &[0.0, -1.0, 1.0, 0.0])]).unwrap();
        assert_eq!(e.data, [0.0, 0.0]);
        assert_eq!(e.im.as_deref(), Some(&[1.0, -1.0][..]));
        let a = mat(2, 2, &[0.0, -1.0, 1.0, 0.0]).into_mat().unwrap();
        let vd = outputs(eig, &[Value::Mat(a.clone())], 2);
        let (av, vd) = (a.matmul(&vd[0]).unwrap(), vd[0].matmul(&vd[1]).unwrap());
        for k in 0..4 {
            assert!((av.c(k) - vd.c(k)).abs() < 1e-14);
        }
        assert_eq!(
            err(eig, &[mat(2, 2, &[f64::NAN, 1.0, 1.0, 1.0])], 1),
            "Input to 'eig' must not contain NaN or Inf."
        );
        assert_eq!(
            err(eig, &[row(&[1.0, 2.0])], 1),
            "Matrix must be square for 'eig'."
        );
        let e = call(eig, &[Value::Mat(Matrix::empty())]).unwrap();
        assert_eq!((e.rows, e.cols), (0, 1));

        let r = call(chol, &[mat(2, 2, &[4.0, 2.0, 2.0, 3.0])]).unwrap();
        assert_eq!(r.data[..3], [2.0, 0.0, 1.0]);
        let pd = "Matrix must be positive definite.";
        assert_eq!(err(chol, &[mat(2, 2, &[1.0, 2.0, 2.0, 1.0])], 1), pd);
        assert_eq!(err(chol, &[mat(2, 2, &[f64::NAN, 0.0, 0.0, 1.0])], 1), pd);
        assert_eq!(
            err(chol, &[row(&[1.0, 2.0])], 1),
            "Matrix must be square for 'chol'."
        );
        let rp = outputs(chol, &[mat(2, 2, &[1.0, 2.0, 2.0, 1.0])], 2);
        assert_eq!(
            (rp[0].data.clone(), rp[1].data.clone()),
            (vec![1.0], vec![2.0])
        );
        let rp = outputs(chol, &[mat(2, 2, &[4.0, 2.0, 2.0, 3.0])], 2);
        assert_eq!(rp[1].data, [0.0]);

        let a = mat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let one = std::slice::from_ref(&a);
        let am = a.clone().into_mat().unwrap();
        let lup = outputs(lu, one, 3);
        assert!((lup[0].get(1, 0) - 1.0 / 3.0).abs() < 1e-15);
        let pa = lup[2].matmul(&am).unwrap();
        assert!(near(&pa, &lup[0].matmul(&lup[1]).unwrap(), 1e-14));
        let lu2 = outputs(lu, one, 2);
        assert!(near(&lu2[0].matmul(&lu2[1]).unwrap(), &am, 1e-14));
        let y = call(lu, one).unwrap();
        assert_eq!(y.get(0, 0), 3.0);

        let qr2 = outputs(qr, one, 2);
        assert!(near(&qr2[0].matmul(&qr2[1]).unwrap(), &am, 1e-14));
        let r = call(qr, one).unwrap();
        assert!((r.get(0, 0).abs() - 10f64.sqrt()).abs() < 1e-14);

        let s = call(svd, &[mat(2, 2, &[3.0, 0.0, 0.0, 4.0])]).unwrap();
        assert_eq!((s.rows, s.cols, s.data.clone()), (2, 1, vec![4.0, 3.0]));
        let usv = outputs(svd, &[mat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])], 3);
        assert_eq!((usv[1].rows, usv[1].cols), (2, 3));
        let back = usv[0]
            .matmul(&usv[1])
            .unwrap()
            .matmul(&usv[2].transpose())
            .unwrap();
        let want = Matrix::new(2, 3, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
        assert!(near(&back, &want, 1e-13));
    }

    /// A sink the test can read back after the interpreter has written to it.
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

    #[test]
    fn inv_warns_through_the_error_sink_and_returns_inf() {
        let buf = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        it.err = Box::new(Shared(buf.clone()));
        let x = inv(&mut it, &[mat(2, 2, &[1.0, 2.0, 2.0, 4.0])], 1).unwrap();
        let x = x[0].clone().into_mat().unwrap();
        assert!(x.data.iter().all(|&v| v == f64::INFINITY));
        assert_eq!(
            String::from_utf8(buf.borrow().clone()).unwrap(),
            "Warning: Matrix is singular to working precision.\n"
        );
        let x = inv(&mut it, &[num(0.0)], 1).unwrap();
        assert_eq!(x[0].clone().into_mat().unwrap().data, [f64::INFINITY]);
        // A regular matrix does not warn.
        buf.borrow_mut().clear();
        inv(&mut it, &[mat(2, 2, &[2.0, 1.0, 1.0, 3.0])], 1).unwrap();
        assert!(buf.borrow().is_empty());
    }
}
