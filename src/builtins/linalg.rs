//! Linear algebra, array rearrangement, search and sort, and the index
//! conversions `sub2ind` and `ind2sub`.

use std::cmp::Ordering;

use super::args::{
    at_most, check_dims, check_shape, dim, fmt_dim, mat, need, option, shape_dims, size_list,
};
use super::complex::C;
use super::core::eps_at;
use super::factor::{self, JACOBI_SWEEPS, SVD_SWEEPS, Vectors, qr_iterations};
use super::math::{default_dim, reduce, sum0};
use super::{Registry, add, one, one_as, one_mat};
use crate::error;
use crate::interp::{Interp, R};
use crate::value::{Class, Matrix, Value, transpose_cell, transpose_struct};

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
    add(r, "repmat", repmat, "repmat(A,n), repmat(A,r,c,...), repmat(A,sz) - tile A along every dimension given.");
    add(r, "squeeze", squeeze, "squeeze(A) - A with its dimensions of length 1 removed; a 2-D array is returned as it is.");
    add(r, "permute", permute, "permute(A,dimorder) - rearrange the dimensions of A: dimension i of the result is dimension dimorder(i) of A.");
    add(r, "cat", cat, "cat(dim,A1,A2,...) - concatenate arrays along dimension dim.");
    add(r, "horzcat", horzcat, "horzcat(A1,A2,...) - concatenate arrays horizontally, as cat(2,A1,A2,...).");
    add(r, "vertcat", vertcat, "vertcat(A1,A2,...) - concatenate arrays vertically, as cat(1,A1,A2,...).");
    add(r, "ipermute", ipermute, "ipermute(B,dimorder) - the inverse of permute: the array A for which permute(A,dimorder) is B.");
    add(r, "fliplr", fliplr, "fliplr(A) - reverse the order of the columns.");
    add(r, "flipud", flipud, "flipud(A) - reverse the order of the rows.");
    add(r, "flip", flip, "flip(A), flip(A,dim) - reverse the order of the elements along the first dimension whose size is not 1, or along dim.");
    add(r, "circshift", circshift, "circshift(A,K), circshift(A,K,dim) - shift the elements circularly by K positions; a vector K shifts dimension i by K(i).");

    // ---- search and sort ---------------------------------------------
    add(r, "find", find, "find(A), find(A,n), find(A,n,'last'), [r,c,v] = find(...) - indices of the non-zero elements.");
    add(r, "sort", sort, "sort(A), sort(A,dim), sort(A,'descend'), [s,i] = sort(...) - sorted vectors, columns or rows, NaN at the high end.");

    // ---- index conversion --------------------------------------------
    add(r, "sub2ind", sub2ind, "ind = sub2ind(sz,I1,I2,...) - the linear indices of the subscripts I1, I2, ... into an array of size sz.");
    add(r, "ind2sub", ind2sub, "[I1,I2,...] = ind2sub(sz,ind) - the subscripts of the linear indices ind into an array of size sz, one per output.");
}

// ---- linear algebra --------------------------------------------------

/// `transpose(A)`, "an alternate way to execute `A.'`" (the MathWorks
/// `transpose` page): a matrix's plain transpose, and since cycle 15 a cell
/// or a struct array with the row and column of every element
/// interchanged, each element unchanged.
fn transpose(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "transpose")?;
    match a.first() {
        Some(Value::Cell(c)) => one(Value::cell(transpose_cell(c.clone()))),
        Some(Value::Struct(s)) => one(Value::strukt(transpose_struct(s.clone()))),
        _ => one_as(mat(a, 0, "transpose")?.transpose()),
    }
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
/// same size parser as the constructors. Since cycle 14b any number of
/// counts, separate or in a size vector, and an N-D `A`: the result's size
/// along each dimension is `A`'s times its count, a dimension past either
/// being 1, and trailing sizes of 1 are dropped, as the constructors drop
/// them, so `repmat(A, 1, 1, 2)` of a 2x3x4 is 2x3x8 and `repmat(1, 2, 3,
/// 1)` is 2x3. The whole shape is judged by `check_dims` before anything
/// is allocated, naming every size of the array asked for, never an
/// intermediate: `repmat([1 2], 1e10, 1e10)` is `Requested
/// 10000000000x20000000000 array exceeds the maximum array size.`
fn repmat(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 2, "repmat")?;
    let m = mat(a, 0, "repmat")?;
    let counts = shape_dims(a, 1, "repmat")?;
    let dims = m.dims();
    let reach = counts.len().max(dims.len());
    let size = |k: usize| dims.get(k).copied().unwrap_or(1);
    let count = |k: usize| counts.get(k).copied().unwrap_or(1.0);
    let mut asked: Vec<f64> = (0..reach).map(|k| size(k) as f64 * count(k)).collect();
    while asked.len() > 2 && asked.last() == Some(&1.0) {
        asked.pop();
    }
    let out_dims = check_dims(&asked)?;
    // An empty result has nothing to tile, however large its sizes (cycle
    // 13b).
    if crate::value::dims_product(&out_dims) == 0 {
        return one_as(Matrix::from_dims(&out_dims, Vec::new()).with_class(m.class));
    }
    // Every count is at least 1 here, so each step's array is no larger
    // than the result, and a count of 1 costs nothing.
    let mut data = m.data;
    // The shape part way through, the dimensions below `k` already tiled.
    let mut cur: Vec<usize> = (0..reach).map(size).collect();
    for k in 0..reach {
        let c = count(k) as usize;
        if c > 1 {
            data = tile(&data, &cur, k + 1, c);
            cur[k] *= c;
        }
    }
    one_as(Matrix::from_dims(&out_dims, data).with_class(m.class))
}

/// `c` copies of an array of shape `dims` joined along dimension `d`: for
/// each of the `after` blocks of the array seen as `[before, n, after]`
/// (`value::along_dim`), its contiguous run of `before * n` elements `c`
/// times over, which is `cat(d, A, A, ...)`.
fn tile(data: &[f64], dims: &[usize], d: usize, c: usize) -> Vec<f64> {
    let (before, n, after) = crate::value::along_dim(dims, d);
    let run = before * n;
    let mut out = Vec::with_capacity(data.len() * c);
    for a in 0..after {
        let block = &data[a * run..(a + 1) * run];
        for _ in 0..c {
            out.extend_from_slice(block);
        }
    }
    out
}

/// `squeeze(A)`, by the MathWorks page (cycle 14b): "B = squeeze(A)
/// returns an array with the same elements as the input array A, but with
/// dimensions of length 1 removed", and "If A is a row vector, column
/// vector, scalar, or an array with no dimensions of length 1, then
/// squeeze returns the input A", as it does every 2-D array. A result with
/// one dimension left is a column, as "a 1-by-1-by-3 array" becomes "a
/// 3-by-1 column vector". No element moves, so class and complex storage
/// are kept; a cell or a struct is refused as by every numeric builtin.
fn squeeze(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 1, "squeeze")?;
    at_most(a, 1, "squeeze")?;
    let mut m = mat(a, 0, "squeeze")?;
    if m.is_nd() {
        let kept: Vec<usize> = m.dims().into_iter().filter(|&d| d != 1).collect();
        // One dimension left is normalised to a column.
        m.set_dims(&kept);
    }
    one_as(m)
}

/// `permute(A, dimorder)`, by the MathWorks page (cycle 14b): "the ith
/// dimension of the output array is the dimension dimorder(i) from the
/// input array", `dimorder` a row of positive integers holding each of 1
/// to n exactly once, with n at least `ndims(A)`, a dimension past
/// `ndims(A)` being of size 1. Anything else is `permute's dimension order
/// must hold each of 1 to n once, with n at least ndims(A).` Class and
/// complex storage are kept, and `permute(M, [2 1])` of a matrix is its
/// plain transpose.
fn permute(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 2, "permute")?;
    at_most(a, 2, "permute")?;
    let m = mat(a, 0, "permute")?;
    let order = permute_order(&a[1], m.ndims(), "permute")?;
    one_as(permuted(&m, &order))
}

/// `ipermute(B, dimorder)` (cycle 14c), by the MathWorks page: the array
/// `A` for which `permute(A, dimorder)` is `B`, "the `i`th dimension of the
/// input array becomes the dimension `dimorder(i)` in the output array".
/// `dimorder` is judged exactly as `permute` judges it, the refusal naming
/// `ipermute`; the result is `permute` of `B` by the inverse order, so the
/// class and complex storage are kept as `permute` keeps them.
fn ipermute(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 2, "ipermute")?;
    at_most(a, 2, "ipermute")?;
    let m = mat(a, 0, "ipermute")?;
    let order = permute_order(&a[1], m.ndims(), "ipermute")?;
    one_as(permuted(&m, &inverse_order(&order)))
}

/// The inverse of a zero-based order holding each of `0..n` once: where
/// `order[i]` is `j`, the inverse's `j` is `i`.
fn inverse_order(order: &[usize]) -> Vec<usize> {
    let mut inv = vec![0; order.len()];
    for (i, &j) in order.iter().enumerate() {
        inv[j] = i;
    }
    inv
}

/// The zero-based order `permute` (and `ipermute`, named by `name`) reads
/// from `v`: a real row of positive integers holding each of 1 to n exactly
/// once, with n at least `ndims`. A column, a char (whatever its codes), a
/// repeat, a gap and a row too short are all the one refusal, judged before
/// anything the order's length would size.
fn permute_order(v: &Value, ndims: usize, name: &str) -> R<Vec<usize>> {
    let Value::Mat(o) = v else {
        return Err(error::dimension_order(name));
    };
    if o.class == Class::Char || o.is_complex() || o.is_nd() || o.rows != 1 || o.cols < ndims {
        return Err(error::dimension_order(name));
    }
    let n = o.cols;
    let mut seen = vec![false; n];
    let mut order = Vec::with_capacity(n);
    for &x in &o.data {
        if !(x >= 1.0 && x <= n as f64 && x.fract() == 0.0) {
            return Err(error::dimension_order(name));
        }
        let k = x as usize - 1;
        if seen[k] {
            return Err(error::dimension_order(name));
        }
        seen[k] = true;
        order.push(k);
    }
    Ok(order)
}

/// `m` permuted by `order`, zero-based and holding each of `0..n` once,
/// `n` at least `m.ndims()`. Dimension `k` of the result is dimension
/// `order[k]` of `m`, so result element `(j1, ..., jn)` is the element of
/// `m` whose coordinate along dimension `order[k]` is `jk`: the result is
/// written in its own column-major order while a counter walks `m`, moving
/// by the stride of dimension `order[k]` along the result's `k`-th. The
/// counter carries only through the dimensions past 1, as broadcasting's
/// does, so a run of singletons costs nothing per step; an empty array is
/// never walked.
pub fn permuted(m: &Matrix, order: &[usize]) -> Matrix {
    let dims = m.dims();
    let size = |j: usize| dims.get(j).copied().unwrap_or(1);
    let mut stride = Vec::with_capacity(order.len());
    let mut acc = 1usize;
    for j in 0..order.len() {
        stride.push(acc);
        acc = acc.saturating_mul(size(j));
    }
    let out_dims: Vec<usize> = order.iter().map(|&j| size(j)).collect();
    let numel = m.numel();
    let live: Vec<(usize, usize)> = order
        .iter()
        .filter(|&&j| size(j) > 1)
        .map(|&j| (size(j), stride[j]))
        .collect();
    let place = |src: &[f64]| -> Vec<f64> {
        let mut out = Vec::with_capacity(numel);
        if numel == 0 {
            return out;
        }
        let mut coord = vec![0usize; live.len()];
        let mut at = 0usize;
        for _ in 0..numel {
            out.push(src[at]);
            for (c, &(d, step)) in coord.iter_mut().zip(&live) {
                *c += 1;
                at += step;
                if *c < d {
                    break;
                }
                at -= step * d;
                *c = 0;
            }
        }
        out
    };
    let mut out = Matrix::from_dims(&out_dims, place(&m.data)).with_class(m.class);
    // The storage as it was: `complex(1, 0)` permuted is still complex.
    out.im = m.im.as_deref().map(place);
    out
}

/// `cat(dim, A1, ..., An)` (cycle 14b): the arrays joined along `dim`, a
/// positive integer that may be past every array's `ndims`, so `cat(3, A,
/// B)` of two matrices makes pages. It is the kernel a bracket with an
/// N-D operand uses, `interp::concat`, which holds the agreement rule, the
/// empty rule, the class and complex rules and the bound on `dim`.
/// `cat(dim)` with no arrays is `[]`, and with one array is that array.
/// Every argument after `dim` must be an array.
fn cat(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 1, "cat")?;
    let d = dim(a, 0, "cat")?;
    let mats = (1..a.len())
        .map(|i| mat(a, i, "cat"))
        .collect::<R<Vec<Matrix>>>()?;
    one_as(crate::interp::concat(d, mats)?)
}

/// `horzcat(A1, ..., An)` (cycle 14c), which is `cat(2, A1, ..., An)`: the
/// MathWorks page's rules, an empty array beside a nonempty one omitted and
/// the empty the sizes give when every input is empty, are `cat`'s, so it
/// runs `cat`'s kernel, `interp::concat`, and not the brackets' 2-D rule:
/// `horzcat(zeros(1, 0), zeros(1, 0))` is 1x0 where `[zeros(1, 0),
/// zeros(1, 0)]` is 0x0. With no argument it is `[]`; every argument must
/// be an array, as `cat`'s must.
fn horzcat(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    joined(a, 2, "horzcat")
}

/// `vertcat(A1, ..., An)` (cycle 14c), which is `cat(1, A1, ..., An)`; see
/// [`horzcat`].
fn vertcat(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    joined(a, 1, "vertcat")
}

/// Every argument joined along `d` through `cat`'s kernel.
fn joined(a: &[Value], d: usize, name: &str) -> R<Vec<Value>> {
    let mats = (0..a.len())
        .map(|i| mat(a, i, name))
        .collect::<R<Vec<Matrix>>>()?;
    one_as(crate::interp::concat(d, mats)?)
}

/// `m` with the order of its elements reversed along dimension `d`,
/// one-based, the class and storage kept (cycle 14c): each of the `before *
/// after` slices of the three-number view `[before, n, after]`
/// ([`crate::value::along_dim`]) reversed on its own, so a matrix along 2
/// has its columns reversed and along 1 its rows, and an N-D array each
/// page alike. Along a dimension of size 1, one past `ndims` included, and
/// for an empty array nothing moves, and nothing is walked.
fn flipped(mut m: Matrix, d: usize) -> Matrix {
    let (before, n, _) = crate::value::along_dim(&m.dims(), d);
    if n < 2 || m.data.is_empty() {
        return m;
    }
    let reverse = |v: &mut [f64]| {
        // Each block of `before * n` elements holds `before` slices, element
        // `k` of slice `b` at `b + before * k`; swapping the `k`-th and the
        // `(n - 1 - k)`-th runs of `before` reverses every slice at once.
        for block in v.chunks_exact_mut(before * n) {
            for k in 0..n / 2 {
                let (lo, hi) = block.split_at_mut((n - 1 - k) * before);
                lo[k * before..(k + 1) * before].swap_with_slice(&mut hi[..before]);
            }
        }
    };
    reverse(&mut m.data);
    if let Some(im) = m.im.as_mut() {
        reverse(im);
    }
    m
}

/// `fliplr(A)`: the order along dimension 2 reversed. Since cycle 14c an
/// N-D array too, by the MathWorks page, "`fliplr` operates on the planes
/// formed by the first and second dimensions", each page flipped on its
/// own. The class is kept; an empty array is its own flip (cycle 13b).
fn fliplr(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "fliplr")?;
    one_as(flipped(mat(a, 0, "fliplr")?, 2))
}

/// `flipud(A)`: the order along dimension 1 reversed, each page on its own
/// since cycle 14c ("The operation flips the elements on each page
/// independently").
fn flipud(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "flipud")?;
    one_as(flipped(mat(a, 0, "flipud")?, 1))
}

/// `flip(A)` and `flip(A, dim)` (cycle 14c), by the MathWorks page: "the
/// order of the elements reversed" along the first dimension whose size is
/// not 1 ([`default_dim`]), so a vector along its length and a matrix in
/// each column, or along `dim`, a positive integer read as the reductions
/// read one. Along a dimension past `ndims(A)` it is `A`. The class is
/// kept; a complex argument is refused by the registry's gate, as for
/// `fliplr`, and a cell or a struct as by every numeric builtin.
fn flip(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 1, "flip")?;
    at_most(a, 2, "flip")?;
    let m = mat(a, 0, "flip")?;
    let d = if a.len() >= 2 {
        dim(a, 1, "flip")?
    } else {
        default_dim(&m.dims())
    };
    one_as(flipped(m, d))
}

/// `circshift(A, K)` and `circshift(A, K, dim)` (cycle 14c), by the
/// MathWorks page: "Positive `K` shifts toward the end of the dimension and
/// negative `K` shifts toward the beginning", wrapping around. An integer
/// `K` shifts along the first dimension whose size is not 1, and element
/// `i` of a vector `K` shifts dimension `i`, one past `ndims(A)` being of
/// size 1, where nothing moves; with `dim`, `K` is one integer shifting
/// along `dim`. `K` is judged first, a nonempty real row or column of
/// integers, then `dim`, read as the reductions read one, and then `K`'s
/// count beside it.
///
/// A shift is taken modulo its dimension's size, exactly in `f64`
/// (`rem_euclid` of two integers), so any integer a double holds costs one
/// pass over the array per dimension that moves, and a dimension of size 0
/// moves nothing; an empty array is returned as it is, never walked. The
/// class is kept; a complex argument is refused by the registry's gate and
/// a cell or a struct as by every numeric builtin.
fn circshift(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 2, "circshift")?;
    at_most(a, 3, "circshift")?;
    let mut m = mat(a, 0, "circshift")?;
    let k = shift_amounts(&a[1])?;
    let along = if a.len() >= 3 {
        let d = dim(a, 2, "circshift")?;
        if k.len() != 1 {
            return Err(error::circshift_shift_with_dim());
        }
        Some(d)
    } else {
        None
    };
    if m.data.is_empty() {
        return one_as(m);
    }
    let dims = m.dims();
    // Each moving dimension, one-based, with its shift in `0..size`. A
    // shift past `ndims` shifts a dimension of size 1, which moves nothing,
    // so it is passed over without a list of sizes reaching it.
    let shifts: Vec<(usize, usize)> = match along {
        Some(d) => vec![(d, k[0])],
        None if k.len() == 1 => vec![(default_dim(&dims), k[0])],
        None => k.iter().enumerate().map(|(i, &s)| (i + 1, s)).collect(),
    }
    .into_iter()
    .filter_map(|(d, s)| {
        let n = *dims.get(d - 1)?;
        if n < 2 {
            return None;
        }
        let s = s.rem_euclid(n as f64) as usize;
        (s != 0).then_some((d, s))
    })
    .collect();
    for (d, s) in shifts {
        let (before, n, _) = crate::value::along_dim(&dims, d);
        // Each block of `before * n` elements holds `before` slices along
        // `d`; rotating the block right by `s * before` moves every slice's
        // element `k` to `(k + s) mod n`.
        for block in m.data.chunks_exact_mut(before * n) {
            block.rotate_right(s * before);
        }
        if let Some(im) = m.im.as_mut() {
            for block in im.chunks_exact_mut(before * n) {
                block.rotate_right(s * before);
            }
        }
    }
    one_as(m)
}

/// The shifts of `circshift`'s `K`: a nonempty real row or column of
/// integers, each still an `f64` so a shift past `usize` is taken modulo
/// its dimension's size exactly. A char, a cell, a matrix, an N-D array, a
/// `NaN`, an infinity and a fraction are all the one refusal.
fn shift_amounts(v: &Value) -> R<Vec<f64>> {
    let Value::Mat(k) = v else {
        return Err(error::circshift_shift());
    };
    let vector = !k.is_nd() && (k.rows == 1 || k.cols == 1);
    if k.class == Class::Char || k.data.is_empty() || !vector {
        return Err(error::circshift_shift());
    }
    if k.data.iter().any(|x| !x.is_finite() || x.fract() != 0.0) {
        return Err(error::circshift_shift());
    }
    Ok(k.data.clone())
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
///
/// Since cycle 14c an N-D `X` too, by the MathWorks page: `find` "returns a
/// column vector of the linear indices", 0x1 when no element is nonzero,
/// and with two outputs "`col` is a linear index over the `N-1` trailing
/// dimensions of `X`", so `X(row(i), col(i))` is the `i`th nonzero element,
/// which column-major storage gives as the linear index divided by the
/// rows. By the page's convention "`k` is an empty matrix `[]` when `X` is
/// ... a scalar zero", `find(0)` and `find(false)` are 0x0 for every
/// output, where they were 1x0.
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
    // A 2-D 0x0, and a scalar zero by the page's convention, give `[]`.
    let blank = (m.rows == 0 && m.cols == 0 && !m.is_nd()) || (m.is_scalar() && idx.is_empty());
    let shape = |data: Vec<f64>| {
        if blank {
            Matrix::empty()
        } else if m.is_nd() {
            Matrix::col(data)
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
///
/// Since cycle 14c any array, by the MathWorks page: along the first
/// dimension whose size is not 1 ([`default_dim`]), or along `dim`, each
/// slice of the three-number view `[before, n, after]`
/// ([`crate::value::along_dim`]) sorted on its own, a matrix along 1 or 2
/// being its columns or its rows as before; "`sort` returns `A` if `dim` is
/// greater than `ndims(A)`", with every index `1`. A dimension of size 1
/// and an empty array are never walked.
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
    let dims = m.dims();
    let along = d.unwrap_or_else(|| default_dim(&dims));
    // Each slice is the `n` elements `b + before * (k + n * a)`: a column of
    // a matrix is `rows` consecutive elements (`before` 1), a row every
    // `rows`-th (`after` 1). Past `ndims`, and along a dimension of size 1,
    // `n` is 1 and nothing moves; an empty array has no slices worth
    // sorting, however long its other dimensions (cycle 13b).
    let (before, n, after) = crate::value::along_dim(&dims, along);
    let mut out = m;
    let mut index = vec![1.0; out.numel()];
    if n > 1 && !out.data.is_empty() {
        let data = out.data.clone();
        let mut pos = Vec::with_capacity(n);
        for a in 0..after {
            for b in 0..before {
                pos.clear();
                pos.extend((0..n).map(|k| b + before * (k + n * a)));
                let mut perm: Vec<usize> = (0..n).collect();
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
        }
    }
    let index = Matrix::from_dims(&dims, index);
    // A sorted char is a char: `sort('cab')` is `'abc'`.
    if nargout < 2 {
        return one_as(out);
    }
    Ok(vec![Value::Mat(out), Value::Mat(index)])
}

// ---- index conversion (cycle 14c) ------------------------------------

/// The size argument of `sub2ind` and `ind2sub`, named by `name`: a
/// nonempty real row or column of positive integers, each kept as the
/// `f64` it was given, so a size past `usize` is never saturated. A char, a
/// cell, a matrix, an N-D array, a zero, a fraction, a `NaN` and an
/// infinity are the one refusal.
fn index_sizes(v: &Value, name: &str) -> R<Vec<f64>> {
    let Value::Mat(sz) = v else {
        return Err(error::index_size_vector(name));
    };
    let vector = !sz.is_nd() && (sz.rows == 1 || sz.cols == 1);
    let positive = |x: f64| x >= 1.0 && x.is_finite() && x.fract() == 0.0;
    if sz.class == Class::Char
        || sz.data.is_empty()
        || !vector
        || !sz.data.iter().all(|&x| positive(x))
    {
        return Err(error::index_size_vector(name));
    }
    Ok(sz.data.clone())
}

/// `sub2ind(sz, I1, ..., In)` (cycle 14c), by the MathWorks page's
/// relation to indexing, "`A(ind(k)) = A(I1(k),…,In(k))`": the linear index
/// of each position in an array of size `sz`, as a double the size of the
/// subscripts that are not scalars (1x1 when all are), which must agree.
/// Subscript `p` runs over size `p`, a size past the end of `sz` being 1,
/// and the last over the product of the sizes from its own on, as an index
/// with fewer subscripts than dimensions folds them; a subscript that is
/// not a positive integer within its size is refused.
///
/// The sizes and strides are `f64`, so a size past `usize` never
/// saturates, and each scalar subscript is judged and weighed once, not
/// once per position: the time is linear in the arguments.
fn sub2ind(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 2, "sub2ind")?;
    let sz = index_sizes(&a[0], "sub2ind")?;
    let subs = (1..a.len())
        .map(|i| mat(a, i, "sub2ind"))
        .collect::<R<Vec<Matrix>>>()?;
    let mut shape: Option<Vec<usize>> = None;
    for s in subs.iter().filter(|s| !s.is_scalar()) {
        let d = s.dims();
        match &shape {
            None => shape = Some(d),
            Some(first) if *first != d => return Err(error::sub2ind_subscript_sizes()),
            Some(_) => {}
        }
    }
    let shape = shape.unwrap_or_else(|| vec![1, 1]);
    let n = subs.len();
    // The size subscript `p` runs over, the last the fold of the rest.
    let size = |p: usize| -> f64 {
        if p + 1 < n {
            sz.get(p).copied().unwrap_or(1.0)
        } else {
            sz.get(p..).map_or(1.0, |rest| rest.iter().product())
        }
    };
    let sizes: Vec<f64> = (0..n).map(size).collect();
    let mut strides = Vec::with_capacity(n);
    let mut acc = 1.0;
    for &d in &sizes {
        strides.push(acc);
        acc *= d;
    }
    let within = |x: f64, p: usize| x >= 1.0 && x <= sizes[p] && x.fract() == 0.0;
    // A subscript of 1 adds nothing, and is never weighed: a stride past
    // what a double holds is `Inf`, and `0 * Inf` would make the index
    // `NaN`, so `sub2ind([1e300 1e300 2], 1, 1, 1)` is 1.
    let offset = |x: f64, p: usize| if x > 1.0 { (x - 1.0) * strides[p] } else { 0.0 };
    let mut base = 1.0;
    for (p, s) in subs.iter().enumerate().filter(|(_, s)| s.is_scalar()) {
        let x = s.data[0];
        if !within(x, p) {
            return Err(error::sub2ind_out_of_range());
        }
        base += offset(x, p);
    }
    let mut out = vec![base; crate::value::dims_product(&shape)];
    for (p, s) in subs.iter().enumerate().filter(|(_, s)| !s.is_scalar()) {
        for (o, &x) in out.iter_mut().zip(&s.data) {
            if !within(x, p) {
                return Err(error::sub2ind_out_of_range());
            }
            *o += offset(x, p);
        }
    }
    one_mat(Matrix::from_dims(&shape, out))
}

/// `[I1, ..., Ik] = ind2sub(sz, ind)` (cycle 14c), by the MathWorks page:
/// `k` doubles each the size of `ind`, `k` the outputs asked for and 1 when
/// none is, reading `sz` as `k` dimensions, with 1s appended up to `k` or
/// the sizes from the `k`th on folded into one. The last of the `k`
/// dimensions has no bound, as the page's example shows (`[row, col] =
/// ind2sub([3 1], [9 11 13 14])` gives `col` as `3 4 5 5`), so the folded
/// size is never needed and any positive integer converts: one output is
/// `ind` itself. An index that is not a positive integer is refused.
///
/// Each subscript is the remainder of the index left so far by its size,
/// and what is left the quotient, exactly in `f64` for any index below
/// 2^53; the time is that of the outputs, `k` times `numel(ind)`.
///
/// A program can ask for as many outputs as the targets it writes, or
/// builds and evaluates, so the `k` outputs are judged together before any
/// is written, as one result: their elements, `numel(ind)` by `k`, and
/// their lists of sizes, `ndims(ind)` by `k`, each held to what one array
/// may hold. A hundred thousand outputs of a hundred thousand indices are
/// refused at once, `Requested 100000x100000 array exceeds the maximum
/// array size.`, rather than written.
fn ind2sub(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(a, 2, "ind2sub")?;
    at_most(a, 2, "ind2sub")?;
    let sz = index_sizes(&a[0], "ind2sub")?;
    let ind = mat(a, 1, "ind2sub")?;
    let positive = |x: f64| x >= 1.0 && x.is_finite() && x.fract() == 0.0;
    if !ind.data.iter().all(|&x| positive(x)) {
        return Err(error::ind2sub_index());
    }
    let dims = ind.dims();
    let k = nargout.max(1);
    check_shape(ind.numel() as f64, k as f64)?;
    check_shape(dims.len() as f64, k as f64)?;
    // Zero-based, what is left of each index once the subscripts before
    // have been taken out.
    let mut left: Vec<f64> = ind.data.iter().map(|&x| x - 1.0).collect();
    let mut outs = Vec::with_capacity(k);
    for p in 0..k - 1 {
        let d = sz.get(p).copied().unwrap_or(1.0);
        let sub: Vec<f64> = left
            .iter_mut()
            .map(|r| {
                let m = *r % d;
                *r = (*r - m) / d;
                m + 1.0
            })
            .collect();
        outs.push(Value::Mat(Matrix::from_dims(&dims, sub)));
    }
    let last = left.into_iter().map(|r| r + 1.0).collect();
    outs.push(Value::Mat(Matrix::from_dims(&dims, last)));
    Ok(outs)
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
        // A third count other than 1 tiles pages since cycle 14b.
        let got = call(repmat, &[v, num(2.0), num(3.0), num(2.0)]).unwrap();
        assert_eq!(got.dims(), [2, 6, 2]);
    }

    /// `A(i, j, k)` of `reshape(1:24, 2, 3, 4)` is `i + 2*(j-1) + 6*(k-1)`,
    /// the spec's running example.
    fn nd24() -> Matrix {
        Matrix::from_dims(&[2, 3, 4], (1..=24).map(f64::from).collect())
    }

    fn nd(dims: &[usize]) -> Value {
        let n = crate::value::dims_product(dims);
        Value::Mat(Matrix::from_dims(dims, (1..=n).map(|k| k as f64).collect()))
    }

    /// Cycle 14b: `repmat` of any number of counts and of an N-D array,
    /// every size judged before anything is allocated.
    #[test]
    fn repmat_tiles_every_dimension_and_judges_the_shape_first() {
        let a = Value::Mat(nd24());
        let got = call(repmat, &[a.clone(), num(1.0), num(1.0), num(2.0)]).unwrap();
        assert_eq!(got.dims(), [2, 3, 8]);
        // The second copy of the pages follows the first.
        assert_eq!(got.data[..24], got.data[24..]);
        assert_eq!(got.data[..24], nd24().data[..]);
        let got = call(repmat, &[row(&[1.0, 2.0]), row(&[2.0, 1.0, 3.0])]).unwrap();
        assert_eq!(got.dims(), [2, 2, 3]);
        assert_eq!(got.data[..4], [1.0, 1.0, 2.0, 2.0]);
        let got = call(repmat, &[a.clone(), num(2.0), num(1.0)]).unwrap();
        assert_eq!(got.dims(), [4, 3, 4]);
        // Each column of the page is doubled down its rows.
        assert_eq!(got.data[..8], [1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 3.0, 4.0]);
        let got = call(repmat, &[row(&[1.0, 2.0]), num(1.0), num(1.0), num(2.0)]).unwrap();
        assert_eq!(
            (got.dims(), got.data),
            (vec![1, 2, 2], vec![1.0, 2.0, 1.0, 2.0])
        );
        // Trailing counts of 1 are dropped, and a count of 0 is empty.
        let got = call(repmat, &[a.clone(), row(&[1.0, 1.0, 1.0, 1.0])]).unwrap();
        assert_eq!(got, nd24());
        let got = call(repmat, &[a.clone(), num(1.0), num(1.0), num(0.0)]).unwrap();
        assert_eq!(got.dims(), [2, 3, 0]);
        // The class is kept.
        let t =
            Value::Mat(Matrix::from_dims(&[1, 1, 2], vec![1.0, 0.0]).with_class(Class::Logical));
        assert_eq!(call(repmat, &[t, num(2.0)]).unwrap().class, Class::Logical);
        // Judged whole before anything is allocated, every size named.
        let t = std::time::Instant::now();
        let e = call(repmat, &[num(1.0), num(1e5), num(1e5), num(1e5)])
            .unwrap_err()
            .msg;
        assert_eq!(
            e,
            "Requested 100000x100000x100000 array exceeds the maximum array size."
        );
        let e = call(repmat, &[a, num(1e5), num(1e5)]).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 200000x300000x4 array exceeds the maximum array size."
        );
        // An empty result costs nothing, however large its other sizes.
        let empty = Value::Mat(Matrix::new(0, 1, Vec::new()));
        let got = call(repmat, &[empty, num(1.0), num(1e15), num(3.0)]).unwrap();
        assert_eq!(got.dims(), [0, 1_000_000_000_000_000, 3]);
        assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
    }

    /// Cycle 14b: `squeeze` by the MathWorks page's rules.
    #[test]
    fn squeeze_removes_the_dimensions_of_length_one() {
        let dims = |d: &[usize]| call(squeeze, &[nd(d)]).unwrap().dims();
        assert_eq!(dims(&[1, 1, 3]), [3, 1]);
        assert_eq!(dims(&[2, 1, 3]), [2, 3]);
        assert_eq!(dims(&[1, 3, 1, 2]), [3, 2]);
        assert_eq!(dims(&[1, 1, 1, 4]), [4, 1]);
        assert_eq!(dims(&[1, 0, 3]), [0, 3]);
        // A 2-D array is returned as it is, a row included.
        assert_eq!(dims(&[2, 3]), [2, 3]);
        assert_eq!(dims(&[1, 5]), [1, 5]);
        assert_eq!(dims(&[1, 1]), [1, 1]);
        // No element moves, and class and complex storage are kept.
        let m = call(squeeze, &[nd(&[1, 3, 1, 2])]).unwrap();
        assert_eq!(m.data, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let mut z = Matrix::from_dims(&[1, 1, 2], vec![1.0, 2.0]);
        z.im = Some(vec![0.0, 0.0]);
        let s = call(squeeze, &[Value::Mat(z)]).unwrap();
        assert_eq!((s.dims(), s.im), (vec![2, 1], Some(vec![0.0, 0.0])));
        let c = Matrix::from_dims(&[1, 1, 2], vec![97.0, 98.0]).with_class(Class::Char);
        assert_eq!(call(squeeze, &[Value::Mat(c)]).unwrap().class, Class::Char);
        // A cell is refused as every numeric builtin refuses one.
        let cell = Value::cell(crate::value::CellArray::new(1, 1, vec![num(1.0)]));
        assert!(call(squeeze, &[cell]).is_err());
    }

    /// Cycle 14b: `permute`'s index arithmetic, "the ith dimension of the
    /// output array is the dimension dimorder(i) from the input array".
    #[test]
    fn permute_moves_each_element_to_its_permuted_subscript() {
        let a = nd24();
        let p = call(permute, &[Value::Mat(a.clone()), row(&[3.0, 1.0, 2.0])]).unwrap();
        assert_eq!(p.dims(), [4, 2, 3]);
        // P(k, i, j) is A(i, j, k), for every element.
        for i in 0..2 {
            for j in 0..3 {
                for k in 0..4 {
                    let at_a = i + 2 * (j + 3 * k);
                    let at_p = k + 4 * (i + 2 * j);
                    assert_eq!(p.data[at_p], a.data[at_a], "({i}, {j}, {k})");
                }
            }
        }
        assert_eq!(p.data[3 + 4 * (1 + 2 * 2)], 24.0);
        assert_eq!(p.data[4 * 2], 3.0);
        // Of a matrix, [2 1] is the plain transpose, imaginary signs kept.
        let z = Matrix::complex_parts(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![1.0; 6]);
        let t = call(permute, &[Value::Mat(z.clone()), row(&[2.0, 1.0])]).unwrap();
        assert_eq!(t, z.transpose());
        // A dimension past ndims is of size 1; the identity order is A.
        let m = call(permute, &[nd(&[2, 3]), row(&[3.0, 1.0, 2.0])]).unwrap();
        assert_eq!(m.dims(), [1, 2, 3]);
        let same = call(
            permute,
            &[Value::Mat(a.clone()), row(&[1.0, 2.0, 3.0, 4.0])],
        )
        .unwrap();
        assert_eq!(same, a);
        let c = call(permute, &[Value::str("ab"), row(&[2.0, 1.0])]).unwrap();
        assert_eq!((c.dims(), c.class), (vec![2, 1], Class::Char));
        // An empty array is never walked, whatever its sizes.
        let e = Matrix::from_dims(&[0, 1 << 40, 3], Vec::new());
        let got = call(permute, &[Value::Mat(e), row(&[3.0, 2.0, 1.0])]).unwrap();
        assert_eq!(got.dims(), [3, 1 << 40, 0]);
        // A run of singletons costs nothing per step.
        let wide = Matrix::from_dims(&[2, 1, 1, 1, 3], vec![1.0; 6]);
        let order: Vec<f64> = [5.0, 4.0, 3.0, 2.0, 1.0].to_vec();
        let got = call(permute, &[Value::Mat(wide), row(&order)]).unwrap();
        assert_eq!(got.dims(), [3, 1, 1, 1, 2]);
    }

    #[test]
    fn permute_refuses_every_other_order() {
        let msg =
            "permute's dimension order must hold each of 1 to n once, with n at least ndims(A).";
        let a = Value::Mat(nd24());
        for bad in [
            row(&[1.0, 2.0]),
            row(&[1.0, 1.0, 2.0]),
            row(&[1.0, 2.0, 4.0]),
            row(&[0.0, 1.0, 2.0]),
            row(&[1.0, 2.0, 3.5]),
            row(&[1.0, 2.0, f64::NAN]),
            Value::Mat(Matrix::col(vec![1.0, 2.0, 3.0])),
            Value::Mat(Matrix::empty()),
            Value::str("abc"),
            // A char is refused whatever its codes, 2, 1 and 3 included.
            Value::Mat(Matrix::row(vec![2.0, 1.0, 3.0]).with_class(Class::Char)),
            Value::cell(crate::value::CellArray::new(1, 1, vec![num(1.0)])),
            Value::Mat(Matrix::complex_parts(
                1,
                3,
                vec![1.0, 2.0, 3.0],
                vec![1.0, 0.0, 0.0],
            )),
        ] {
            let e = call(permute, &[a.clone(), bad.clone()]).unwrap_err().msg;
            assert_eq!(e, msg, "{bad:?}");
        }
        // A matrix needs two, and one is too few.
        assert_eq!(call(permute, &[num(1.0), num(1.0)]).unwrap_err().msg, msg);
        assert!(call(permute, &[a]).is_err());
    }

    fn cat_of(d: f64, args: &[Value]) -> R<Matrix> {
        let mut all = vec![num(d)];
        all.extend_from_slice(args);
        call(cat, &all)
    }

    /// Cycle 14b: `cat`'s agreement rule and its empty rule, which are the
    /// brackets' own.
    #[test]
    fn cat_joins_along_any_dimension_when_the_others_agree() {
        let a = Value::Mat(Matrix::new(2, 2, vec![1.0, 3.0, 2.0, 4.0]));
        let b = Value::Mat(Matrix::new(2, 2, vec![5.0, 7.0, 6.0, 8.0]));
        let p = cat_of(3.0, &[a.clone(), b.clone()]).unwrap();
        assert_eq!(p.dims(), [2, 2, 2]);
        assert_eq!(p.data, [1.0, 3.0, 2.0, 4.0, 5.0, 7.0, 6.0, 8.0]);
        let x = Value::Mat(nd24());
        let v = cat_of(1.0, &[x.clone(), x.clone()]).unwrap();
        assert_eq!(v.dims(), [4, 3, 4]);
        // Each column of each page is the column of A twice.
        assert_eq!(v.data[..8], [1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 3.0, 4.0]);
        let h = cat_of(2.0, &[nd(&[2, 2]), nd(&[2, 3])]).unwrap();
        assert_eq!(h.dims(), [2, 5]);
        assert_eq!(
            cat_of(4.0, &[num(1.0), num(2.0)]).unwrap().dims(),
            [1, 1, 1, 2]
        );
        // Every other dimension must agree, past ndims being 1.
        let mismatch = "Dimensions of arrays being concatenated are not consistent.";
        assert_eq!(
            cat_of(3.0, &[nd(&[2, 2]), nd(&[2, 3])]).unwrap_err().msg,
            mismatch
        );
        assert_eq!(
            cat_of(1.0, &[x.clone(), nd(&[2, 3])]).unwrap_err().msg,
            mismatch
        );
        assert_eq!(
            cat_of(2.0, &[nd(&[2, 3, 2]), nd(&[2, 3, 3])])
                .unwrap_err()
                .msg,
            mismatch
        );
        // An empty beside a nonempty array is left out; when every array is
        // empty, the empty their sizes give, and `[]` when nothing is left.
        assert_eq!(
            cat_of(3.0, &[a.clone(), Value::Mat(Matrix::empty())])
                .unwrap()
                .dims(),
            [2, 2]
        );
        assert_eq!(
            cat_of(1.0, &[nd(&[0, 5]), a.clone()]).unwrap().dims(),
            [2, 2]
        );
        let e = Value::Mat(Matrix::new(2, 0, Vec::new()));
        assert_eq!(
            cat_of(3.0, &[e.clone(), e.clone()]).unwrap().dims(),
            [2, 0, 2]
        );
        assert_eq!(
            cat_of(2.0, &[e.clone(), Value::Mat(Matrix::empty())])
                .unwrap()
                .dims(),
            [2, 0]
        );
        assert_eq!(cat_of(3.0, &[]).unwrap(), Matrix::empty());
        assert_eq!(
            cat_of(2.0, &[Value::Mat(Matrix::empty())]).unwrap(),
            Matrix::empty()
        );
        // One array is that array.
        assert_eq!(cat_of(7.0, std::slice::from_ref(&x)).unwrap(), nd24());
        // The class by the bracket rule, complex if any array is.
        let c = cat_of(3.0, &[Value::str("ab"), Value::str("cd")]).unwrap();
        assert_eq!((c.class, c.dims()), (Class::Char, vec![1, 2, 2]));
        let t = Value::Mat(Matrix::from_bool(true));
        assert_eq!(
            cat_of(3.0, &[t.clone(), t.clone()]).unwrap().class,
            Class::Logical
        );
        assert_eq!(cat_of(3.0, &[t, num(2.0)]).unwrap().class, Class::Double);
        let z = Value::Mat(Matrix::complex_parts(1, 1, vec![1.0], vec![2.0]));
        let j = cat_of(3.0, &[num(5.0), z]).unwrap();
        assert_eq!((j.data, j.im), (vec![5.0, 1.0], Some(vec![0.0, 2.0])));
        // A dimension past every array's makes that many dimensions, so it
        // is judged against the cap on dimensions before any list of sizes
        // is made; at the cap the list is made.
        let cap = "Arrays have at most 1048576 dimensions.";
        for d in [2f64.powi(20) + 1.0, 2f64.powi(21), 1e10, 1e300] {
            let e = cat_of(d, &[num(1.0), num(2.0)]).unwrap_err().msg;
            assert_eq!(e, cap, "{d}");
        }
        let at = cat_of(2f64.powi(20), &[num(1.0), num(2.0)]).unwrap();
        assert_eq!((at.ndims(), at.data), (1 << 20, vec![1.0, 2.0]));
        // Joining nothing along it makes no dimension, so one array is
        // itself.
        assert_eq!(cat_of(1e10, &[num(1.0)]).unwrap(), Matrix::scalar(1.0));
        // The dimension is a positive integer.
        assert!(cat_of(0.0, &[num(1.0)]).is_err());
        assert!(cat_of(1.5, &[num(1.0)]).is_err());
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

    // ---- cycle 14c: the slice functions, the new rearrangements and the
    // index conversions ------------------------------------------------

    /// A builtin of any file, from the registry.
    fn builtin(name: &str) -> crate::builtins::BuiltinFn {
        crate::builtins::registry()[name].f
    }

    /// Every output of the builtin `name`, asked for `nargout`.
    fn run(name: &str, args: &[Value], nargout: usize) -> Vec<Matrix> {
        outputs(builtin(name), args, nargout)
    }

    /// The zero-based order that brings dimension `d` (one-based) of an
    /// array of `nd` dimensions to the front, the others after it in turn.
    fn to_front(nd: usize, d: usize) -> Vec<usize> {
        let mut o = vec![d - 1];
        o.extend((0..nd).filter(|&j| j != d - 1));
        o
    }

    /// Each slice function along `d`, one-based, with its arguments laid
    /// out as the builtin takes them and its outputs: `sort` and `mode`
    /// give two.
    fn along(name: &str, a: &Matrix, d: usize) -> Vec<Matrix> {
        let (x, dn) = (Value::Mat(a.clone()), num(d as f64));
        match name {
            "sort" | "mode" => run(name, &[x, dn], 2),
            "flip" | "median" => run(name, &[x, dn], 1),
            "circshift" | "diff" => run(name, &[x, num(1.0), dn], 1),
            "std" => run(name, &[x, num(0.0), dn], 1),
            "var" => run(name, &[x, num(1.0), dn], 1),
            _ => unreachable!("{name}"),
        }
    }

    const SLICE_FUNCTIONS: [&str; 8] = [
        "sort",
        "mode",
        "flip",
        "median",
        "circshift",
        "diff",
        "std",
        "var",
    ];

    /// Cycle 14c, acceptance test 16: every slice function works along one
    /// dimension through the three-number view `[before, n, after]`. A
    /// matrix along 1 or 2 reads its columns or its rows in the order they
    /// were always read, so each answer is the function of that column or
    /// row alone; an N-D array along any dimension gives what the same
    /// function gives along the columns of the array with that dimension
    /// brought to the front, which reads each slice contiguously.
    #[test]
    fn slices_along_any_dimension_read_a_matrix_as_today() {
        use std::cell::RefCell;
        // The slices `map_slices` hands out, in the order it hands them out.
        let seen: RefCell<Vec<Vec<f64>>> = RefCell::new(Vec::new());
        let record = |s: &[f64]| {
            seen.borrow_mut().push(s.to_vec());
            s.to_vec()
        };
        let slices = crate::builtins::numerics::map_slices;
        // [1 3 5; 2 4 6]: its columns, then its rows, each in order.
        let m = Matrix::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(slices(&m, 1, 2, record).unwrap(), m);
        assert_eq!(
            seen.take(),
            [vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]]
        );
        assert_eq!(slices(&m, 2, 3, record).unwrap(), m);
        assert_eq!(seen.take(), [vec![1.0, 3.0, 5.0], vec![2.0, 4.0, 6.0]]);
        // Past `ndims` each element is a slice of one, in storage order.
        assert_eq!(slices(&m, 3, 1, record).unwrap(), m);
        assert_eq!(seen.take().concat(), m.data);
        // An N-D array: slice `(b, a)` is the elements `b + before * (k +
        // n * a)`, `a` the outer loop.
        let a = nd24();
        for d in 1..=4 {
            let (before, n, after) = crate::value::along_dim(&a.dims(), d);
            assert_eq!(slices(&a, d, n, record).unwrap(), a, "{d}");
            let mut want: Vec<Vec<f64>> = Vec::new();
            for x in 0..after {
                for b in 0..before {
                    want.push((0..n).map(|k| a.data[b + before * (k + n * x)]).collect());
                }
            }
            assert_eq!(seen.take(), want, "{d}");
        }
        // Every slice function on a matrix: along 1 each column of the
        // answer is the function of that column alone, along 2 each row of
        // that row alone. Values repeat, so `mode` and a stable `sort` have
        // ties to settle.
        let data: Vec<f64> = (0..12).map(|k| ((k * 7) % 5) as f64 - 1.5).collect();
        let m = Matrix::new(3, 4, data);
        for name in SLICE_FUNCTIONS {
            let by_cols = along(name, &m, 1);
            let by_rows = along(name, &m, 2);
            for c in 0..4 {
                let col = Matrix::col((0..3).map(|r| m.get(r, c)).collect());
                for (o, whole) in along(name, &col, 1).iter().zip(&by_cols) {
                    let part: Vec<f64> = (0..whole.rows).map(|r| whole.get(r, c)).collect();
                    assert_eq!(part, o.data, "{name} column {c}");
                }
            }
            for r in 0..3 {
                let row = Matrix::row((0..4).map(|c| m.get(r, c)).collect());
                for (o, whole) in along(name, &row, 2).iter().zip(&by_rows) {
                    let part: Vec<f64> = (0..whole.cols).map(|c| whole.get(r, c)).collect();
                    assert_eq!(part, o.data, "{name} row {r}");
                }
            }
        }
        // Every slice function on an N-D array along each of its
        // dimensions, against the columns of the array permuted to bring
        // that dimension to the front and permuted back.
        let data: Vec<f64> = (0..120).map(|k| ((k * 7) % 9) as f64).collect();
        let x = Matrix::from_dims(&[2, 3, 4, 5], data);
        for name in SLICE_FUNCTIONS {
            for d in 1..=4 {
                let order = to_front(4, d);
                let front = permuted(&x, &order);
                let want: Vec<Matrix> = along(name, &front, 1)
                    .iter()
                    .map(|r| permuted(r, &inverse_order(&order)))
                    .collect();
                assert_eq!(along(name, &x, d), want, "{name} along {d}");
            }
        }
    }

    /// Cycle 14c: `[row, col] = find(X)` of an N-D `X` gives `col` as the
    /// linear index over every dimension past the first, so `X(row(i),
    /// col(i))` is the `i`th nonzero element, and every output is a column.
    #[test]
    fn find_gives_the_trailing_dimensions_as_one_column_index() {
        let a = nd24();
        let big = Value::Mat(a.map(|v| if v > 14.0 { v } else { 0.0 }));
        let out = outputs(find, std::slice::from_ref(&big), 3);
        assert!(out.iter().all(|m| m.dims() == [10, 1]));
        for i in 0..10 {
            let (r, c, v) = (out[0].data[i], out[1].data[i], out[2].data[i]);
            // Element (r, c) of the 2x12 fold is the element found.
            let at = (r as usize - 1) + 2 * (c as usize - 1);
            assert_eq!((a.data[at], v), (v, 15.0 + i as f64));
        }
        assert_eq!(out[1].data.last(), Some(&12.0));
        // One output is the linear indices, a column even of a 1x3x2.
        let k = call(find, &[nd(&[1, 3, 2])]).unwrap();
        assert_eq!(
            (k.dims(), k.data),
            (vec![6, 1], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
        );
        let none = call(find, &[Value::Mat(Matrix::filled_dims(&[2, 2, 2], 0.0))]).unwrap();
        assert_eq!(none.dims(), [0, 1]);
        let none = call(find, &[Value::Mat(Matrix::from_dims(&[0, 0, 3], vec![]))]).unwrap();
        assert_eq!(none.dims(), [0, 1]);
        // The count and the direction as for a matrix.
        let x = Value::Mat(a.clone());
        assert_eq!(
            call(find, &[x.clone(), num(2.0), text("last")])
                .unwrap()
                .data,
            [23.0, 24.0]
        );
        assert_eq!(call(find, &[x, num(1.0)]).unwrap().dims(), [1, 1]);
        // A scalar zero is `[]` for every output, by the page's convention;
        // every other 2-D answer is as it was.
        for zero in [num(0.0), Value::Mat(Matrix::from_bool(false)), num(-0.0)] {
            for n in 1..=3 {
                let out = outputs(find, std::slice::from_ref(&zero), n);
                assert!(out.iter().all(|m| m.dims() == [0, 0]), "{n}");
            }
            let out = outputs(find, &[zero, num(1.0)], 1);
            assert_eq!(out[0].dims(), [0, 0]);
        }
        assert_eq!(call(find, &[num(5.0)]).unwrap().dims(), [1, 1]);
        assert_eq!(call(find, &[num(f64::NAN)]).unwrap().dims(), [1, 1]);
        assert_eq!(call(find, &[row(&[0.0, 0.0, 0.0])]).unwrap().dims(), [1, 0]);
        assert_eq!(call(find, &[col(&[0.0, 0.0])]).unwrap().dims(), [0, 1]);
        assert_eq!(call(find, &[mat(2, 3, &[0.0; 6])]).unwrap().dims(), [0, 1]);
        assert_eq!(
            call(find, &[Value::Mat(Matrix::empty())]).unwrap().dims(),
            [0, 0]
        );
    }

    /// Cycle 14c: `flip` along the first dimension that is not 1 or along
    /// `dim`, `fliplr` and `flipud` page by page, each keeping the class.
    #[test]
    fn flip_reverses_along_one_dimension() {
        let a = nd24();
        let x = Value::Mat(a.clone());
        // A(i, j, k) of the result is A(i, j, 5 - k).
        let f = call(flip, &[x.clone(), num(3.0)]).unwrap();
        for i in 0..2 {
            for j in 0..3 {
                for k in 0..4 {
                    let at = |k: usize| i + 2 * (j + 3 * k);
                    assert_eq!(f.data[at(k)], a.data[at(3 - k)]);
                }
            }
        }
        assert_eq!(
            call(flip, std::slice::from_ref(&x)).unwrap(),
            call(flipud, std::slice::from_ref(&x)).unwrap()
        );
        assert_eq!(
            call(flip, &[x.clone(), num(2.0)]).unwrap(),
            call(fliplr, std::slice::from_ref(&x)).unwrap()
        );
        // Page 2 of fliplr(A) is [11 9 7; 12 10 8], page 4 of flipud(A) is
        // [20 22 24; 19 21 23].
        let lr = call(fliplr, std::slice::from_ref(&x)).unwrap();
        assert_eq!(lr.page(1).data, [11.0, 12.0, 9.0, 10.0, 7.0, 8.0]);
        let ud = call(flipud, std::slice::from_ref(&x)).unwrap();
        assert_eq!(ud.page(3).data, [20.0, 19.0, 22.0, 21.0, 24.0, 23.0]);
        // Past `ndims`, along a dimension of size 1 and of an empty array,
        // nothing moves.
        assert_eq!(call(flip, &[x.clone(), num(5.0)]).unwrap(), a);
        assert_eq!(call(flip, &[x.clone(), num(1e300)]).unwrap(), a);
        assert_eq!(
            call(flip, &[row(&[1.0, 2.0]), num(1.0)]).unwrap().data,
            [1.0, 2.0]
        );
        let e = Matrix::from_dims(&[0, 1 << 40, 3], Vec::new());
        assert_eq!(call(flip, &[Value::Mat(e.clone()), num(2.0)]).unwrap(), e);
        // Vectors along their length, a matrix down its columns, the class
        // kept.
        assert_eq!(
            call(flip, &[row(&[1.0, 2.0, 3.0])]).unwrap().data,
            [3.0, 2.0, 1.0]
        );
        assert_eq!(
            call(flip, &[col(&[1.0, 2.0, 3.0])]).unwrap().data,
            [3.0, 2.0, 1.0]
        );
        let m = call(flip, &[mat(2, 2, &[1.0, 2.0, 3.0, 4.0])]).unwrap();
        assert_eq!(m.data, [3.0, 1.0, 4.0, 2.0]);
        let c = call(flip, &[text("abc")]).unwrap();
        assert_eq!((c.class, c.data), (Class::Char, vec![99.0, 98.0, 97.0]));
        let t =
            Value::Mat(Matrix::from_dims(&[1, 1, 2], vec![1.0, 0.0]).with_class(Class::Logical));
        let t = call(flip, &[t]).unwrap();
        assert_eq!((t.class, t.data), (Class::Logical, vec![0.0, 1.0]));
        // The refusals: a cell as every numeric builtin, a bad dimension.
        let cell = Value::cell(crate::value::CellArray::new(1, 1, vec![num(1.0)]));
        assert!(call(flip, &[cell]).is_err());
        assert!(call(flip, &[x.clone(), num(0.0)]).is_err());
        assert!(call(flip, &[x, num(1.0), num(1.0)]).is_err());
    }

    /// Cycle 14c: `circshift` takes each shift modulo its dimension's
    /// size, one pass whatever the shift, a vector shift moving each
    /// dimension, one past `ndims` moving nothing.
    #[test]
    fn circshift_wraps_modulo_the_size() {
        let v = row(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let shift = |k: Value| call(circshift, &[v.clone(), k]).unwrap().data;
        assert_eq!(shift(num(2.0)), [4.0, 5.0, 1.0, 2.0, 3.0]);
        assert_eq!(shift(num(-1.0)), [2.0, 3.0, 4.0, 5.0, 1.0]);
        assert_eq!(shift(num(7.0)), [4.0, 5.0, 1.0, 2.0, 3.0]);
        assert_eq!(shift(num(-7.0)), [3.0, 4.0, 5.0, 1.0, 2.0]);
        // A huge shift, exactly modulo the size: 2^60 is 1 more than a
        // multiple of 5, and 1e15 a multiple of 5.
        let t = std::time::Instant::now();
        assert_eq!(shift(num(2f64.powi(60))), [5.0, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!(shift(num(-(2f64.powi(60)))), [2.0, 3.0, 4.0, 5.0, 1.0]);
        assert_eq!(shift(num(1e15)), [1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(shift(num(1e300)).len(), 5);
        assert!(t.elapsed().as_secs() < 5);
        // A matrix: down the columns by default, along `dim`, or a vector.
        let m = mat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let c = |args: &[Value]| call(circshift, args).unwrap().data;
        assert_eq!(c(&[m.clone(), num(1.0)]), [3.0, 1.0, 4.0, 2.0]);
        assert_eq!(c(&[m.clone(), num(1.0), num(2.0)]), [2.0, 4.0, 1.0, 3.0]);
        assert_eq!(c(&[m.clone(), row(&[1.0, 1.0])]), [4.0, 2.0, 3.0, 1.0]);
        assert_eq!(c(&[m.clone(), col(&[1.0, 1.0])]), [4.0, 2.0, 3.0, 1.0]);
        // An N-D array: element (i, j, k) moves to (i, j, k + 1 mod 4); a
        // whole turn, a shift of a dimension past `ndims` and `dim` past it
        // move nothing.
        let a = nd24();
        let x = Value::Mat(a.clone());
        let s = call(circshift, &[x.clone(), num(1.0), num(3.0)]).unwrap();
        for p in 0..24 {
            let (ij, k) = (p % 6, p / 6);
            assert_eq!(s.data[ij + 6 * ((k + 1) % 4)], a.data[p]);
        }
        assert_eq!(
            call(circshift, &[x.clone(), row(&[0.0, 0.0, 4.0])]).unwrap(),
            a
        );
        assert_eq!(
            call(circshift, &[x.clone(), row(&[1.0, 0.0, 0.0, 5.0])]).unwrap(),
            call(circshift, &[x.clone(), num(1.0)]).unwrap()
        );
        assert_eq!(
            call(circshift, &[x.clone(), num(3.0), num(9.0)]).unwrap(),
            a
        );
        // Shifts of 0 along both dimensions, and of 2 to 999 past them.
        let many: Vec<f64> = (0..1000)
            .map(|k| if k < 2 { 0.0 } else { f64::from(k) })
            .collect();
        assert_eq!(
            call(circshift, &[row(&[1.0, 2.0]), row(&many)])
                .unwrap()
                .data,
            [1.0, 2.0]
        );
        // An empty array is returned as it is, whatever the shift.
        let e = Matrix::from_dims(&[0, 1 << 40, 3], Vec::new());
        assert_eq!(
            call(circshift, &[Value::Mat(e.clone()), row(&[1.0, 5.0, 7.0])]).unwrap(),
            e
        );
        // The class is kept.
        let t = call(circshift, &[text("abc"), num(1.0)]).unwrap();
        assert_eq!((t.class, t.data), (Class::Char, vec![99.0, 97.0, 98.0]));
        // The shift's refusals, and the dimension's.
        let shape = "circshift's shift must be an integer or a vector of integers.";
        for bad in [
            num(1.5),
            num(f64::NAN),
            num(f64::INFINITY),
            Value::Mat(Matrix::empty()),
            mat(2, 2, &[1.0; 4]),
            Value::Mat(Matrix::from_dims(&[1, 1, 2], vec![1.0, 1.0])),
            text("a"),
            Value::cell(crate::value::CellArray::new(1, 1, vec![num(1.0)])),
        ] {
            assert_eq!(
                call(circshift, &[v.clone(), bad.clone()]).unwrap_err().msg,
                shape,
                "{bad:?}"
            );
        }
        assert_eq!(
            call(circshift, &[v.clone(), row(&[1.0, 1.0]), num(2.0)])
                .unwrap_err()
                .msg,
            "circshift's shift must be one integer when a dimension is given."
        );
        assert_eq!(
            call(circshift, &[v.clone(), num(1.5), num(2.0)])
                .unwrap_err()
                .msg,
            shape
        );
        assert!(
            call(circshift, &[v.clone(), num(1.0), num(0.0)])
                .unwrap_err()
                .msg
                .contains("Dimension")
        );
        assert!(call(circshift, std::slice::from_ref(&v)).is_err());
    }

    /// Cycle 14c: `ipermute(permute(A, o), o)` is `A`, and so is
    /// `permute(ipermute(A, o), o)`, over every order of four dimensions;
    /// the order is judged as `permute` judges it, in `ipermute`'s name.
    #[test]
    fn ipermute_inverts_permute() {
        let x = Matrix::from_dims(&[2, 3, 4, 5], (0..120).map(f64::from).collect());
        let mut orders = Vec::new();
        for a in 0..4 {
            for b in (0..4).filter(|&b| b != a) {
                for c in (0..4).filter(|&c| c != a && c != b) {
                    let d = 6 - a - b - c;
                    orders.push([a, b, c, d]);
                }
            }
        }
        assert_eq!(orders.len(), 24);
        for o in orders {
            let one: Vec<f64> = o.iter().map(|&j| (j + 1) as f64).collect();
            let p = call(permute, &[Value::Mat(x.clone()), row(&one)]).unwrap();
            let back = call(ipermute, &[Value::Mat(p), row(&one)]).unwrap();
            assert_eq!(back, x, "{o:?}");
            let i = call(ipermute, &[Value::Mat(x.clone()), row(&one)]).unwrap();
            let again = call(permute, &[Value::Mat(i.clone()), row(&one)]).unwrap();
            assert_eq!(again, x, "{o:?}");
            // Dimension `o(i)` of the result is dimension `i` of the argument.
            for (k, &j) in o.iter().enumerate() {
                assert_eq!(i.dims()[j], x.dims()[k], "{o:?}");
            }
        }
        // A dimension past `ndims` is of size 1; class and complex storage
        // are kept.
        let s = call(
            ipermute,
            &[
                Value::Mat(Matrix::filled_dims(&[4, 2, 3], 1.0)),
                row(&[3.0, 1.0, 2.0]),
            ],
        )
        .unwrap();
        assert_eq!(s.dims(), [2, 3, 4]);
        let z = Matrix::complex_parts(2, 3, vec![1.0; 6], vec![0.0; 6]);
        let t = call(ipermute, &[Value::Mat(z.clone()), row(&[2.0, 1.0])]).unwrap();
        assert_eq!((t.dims(), t.im.is_some()), (vec![3, 2], true));
        let c = call(ipermute, &[text("ab"), row(&[2.0, 1.0])]).unwrap();
        assert_eq!(c.class, Class::Char);
        // Every order `permute` refuses, refused in `ipermute`'s name.
        let msg =
            "ipermute's dimension order must hold each of 1 to n once, with n at least ndims(A).";
        let a = Value::Mat(nd24());
        for bad in [
            row(&[1.0, 2.0]),
            row(&[1.0, 1.0, 2.0]),
            row(&[1.0, 2.0, 4.0]),
            col(&[1.0, 2.0, 3.0]),
            text("abc"),
        ] {
            assert_eq!(
                call(ipermute, &[a.clone(), bad.clone()]).unwrap_err().msg,
                msg,
                "{bad:?}"
            );
        }
        // And `permute`'s own text is as it was.
        assert_eq!(
            call(permute, &[a, row(&[1.0, 2.0])]).unwrap_err().msg,
            "permute's dimension order must hold each of 1 to n once, with n at least ndims(A)."
        );
    }

    /// Cycle 14c: `horzcat(...)` is `cat(2, ...)` and `vertcat(...)` is
    /// `cat(1, ...)`, answer for answer and refusal for refusal, where a
    /// bracket of 2-D operands keeps its own rule for empties.
    #[test]
    fn horzcat_and_vertcat_are_cat() {
        let e10 = Value::Mat(Matrix::new(1, 0, Vec::new()));
        let e01 = Value::Mat(Matrix::new(0, 1, Vec::new()));
        let blank = Value::Mat(Matrix::empty());
        let z = Value::Mat(Matrix::complex_parts(1, 1, vec![1.0], vec![2.0]));
        let cases: Vec<Vec<Value>> = vec![
            vec![],
            vec![num(1.0)],
            vec![row(&[1.0, 2.0]), num(3.0)],
            vec![row(&[1.0, 2.0]), row(&[3.0, 4.0])],
            vec![col(&[1.0, 2.0]), col(&[3.0, 4.0])],
            vec![Value::Mat(nd24()), Value::Mat(nd24())],
            vec![e10.clone(), e10.clone()],
            vec![e01.clone(), e01.clone()],
            vec![e10.clone(), e01.clone()],
            vec![col(&[1.0, 2.0]), blank.clone()],
            vec![blank.clone(), blank.clone()],
            vec![text("ab"), text("cd")],
            vec![text("a"), num(66.0)],
            vec![Value::Mat(Matrix::from_bool(true)), num(2.0)],
            vec![num(5.0), z],
            vec![row(&[1.0, 2.0]), row(&[1.0, 2.0, 3.0])],
            vec![Value::Mat(nd24()), mat(2, 3, &[0.0; 6])],
        ];
        for args in &cases {
            for (f, d) in [(horzcat as crate::builtins::BuiltinFn, 2.0), (vertcat, 1.0)] {
                let mut with = vec![num(d)];
                with.extend_from_slice(args);
                let want = call(cat, &with).map_err(|e| e.msg);
                assert_eq!(call(f, args).map_err(|e| e.msg), want, "{d} {args:?}");
            }
        }
        // The spec's pins: cat's empty rule, where the bracket's gives 0x0.
        assert_eq!(call(horzcat, &[e10.clone(), e10]).unwrap().dims(), [1, 0]);
        assert_eq!(
            call(vertcat, &[col(&[1.0, 2.0]), blank]).unwrap().dims(),
            [2, 1]
        );
        assert_eq!(call(horzcat, &[]).unwrap(), Matrix::empty());
        // A cell is refused as `cat` refuses one.
        let cell = Value::cell(crate::value::CellArray::new(1, 1, vec![num(1.0)]));
        assert_eq!(
            call(horzcat, &[cell.clone(), cell]).unwrap_err().msg,
            "This operation is not supported for a value of class 'cell'."
        );
    }

    /// Cycle 14c: `sub2ind` against the index pipeline over every subscript
    /// of a 2x3x4 array, with as many subscripts as dimensions, fewer (the
    /// last folding the rest) and more (each past them 1).
    #[test]
    fn sub2ind_agrees_with_indexing() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        it.run("A = reshape(1:24, 2, 3, 4);").unwrap();
        let s2i = |args: &[Value]| call(sub2ind, args);
        let sz = row(&[2.0, 3.0, 4.0]);
        for i in 1..=2 {
            for j in 1..=3 {
                for k in 1..=4 {
                    let ind =
                        s2i(&[sz.clone(), num(i as f64), num(j as f64), num(k as f64)]).unwrap();
                    let src = format!("assert(A({i}, {j}, {k}) == A({}));", ind.data[0]);
                    assert!(it.run(&src).is_ok(), "{src}");
                    let more = s2i(&[
                        sz.clone(),
                        num(i as f64),
                        num(j as f64),
                        num(k as f64),
                        num(1.0),
                    ])
                    .unwrap();
                    assert_eq!(more.data, ind.data);
                    assert!(
                        s2i(&[
                            sz.clone(),
                            num(i as f64),
                            num(j as f64),
                            num(k as f64),
                            num(2.0)
                        ])
                        .is_err()
                    );
                }
                // Two subscripts: the second over the 12 columns of the fold.
                for jk in 1..=12 {
                    let ind = s2i(&[sz.clone(), num(i as f64), num(jk as f64)]).unwrap();
                    let src = format!("assert(A({i}, {jk}) == A({}));", ind.data[0]);
                    assert!(it.run(&src).is_ok(), "{src}");
                }
            }
        }
        // One subscript is the index itself, within the whole size.
        for n in 1..=24 {
            assert_eq!(s2i(&[sz.clone(), num(n as f64)]).unwrap().data, [n as f64]);
        }
        // Past each dimension, the fold for the last, and not an integer.
        let range = "sub2ind's subscripts must be positive integers within the size.";
        for bad in [
            vec![num(3.0), num(1.0), num(1.0)],
            vec![num(1.0), num(4.0), num(1.0)],
            vec![num(1.0), num(1.0), num(5.0)],
            vec![num(1.0), num(13.0)],
            vec![num(25.0)],
            vec![num(0.0), num(1.0)],
            vec![num(1.5), num(1.0)],
            vec![num(f64::NAN), num(1.0)],
        ] {
            let mut args = vec![sz.clone()];
            args.extend(bad);
            assert_eq!(s2i(&args).unwrap_err().msg, range);
        }
        // Arrays and scalars mix; the result has the arrays' shape.
        let got = s2i(&[row(&[3.0, 4.0]), col(&[1.0, 2.0]), num(3.0)]).unwrap();
        assert_eq!((got.dims(), got.data), (vec![2, 1], vec![7.0, 8.0]));
        let pages = Value::Mat(Matrix::from_dims(&[1, 1, 2], vec![1.0, 2.0]));
        let got = s2i(&[sz.clone(), num(2.0), num(1.0), pages]).unwrap();
        assert_eq!((got.dims(), got.data), (vec![1, 1, 2], vec![2.0, 8.0]));
        assert_eq!(
            s2i(&[sz.clone(), row(&[1.0, 2.0]), row(&[1.0, 2.0, 3.0])])
                .unwrap_err()
                .msg,
            "sub2ind's subscripts must have the same size, or be scalars."
        );
        // The size: a real row or column of positive integers.
        let size = "sub2ind's size must be a vector of positive integers.";
        for bad in [
            row(&[2.0, 0.0]),
            row(&[2.0, 1.5]),
            row(&[2.0, f64::INFINITY]),
            mat(2, 2, &[1.0; 4]),
            Value::Mat(Matrix::empty()),
            text("ab"),
        ] {
            assert_eq!(
                s2i(&[bad.clone(), num(1.0)]).unwrap_err().msg,
                size,
                "{bad:?}"
            );
        }
        assert_eq!(
            s2i(&[col(&[2.0, 3.0]), num(2.0), num(3.0)]).unwrap().data,
            [6.0]
        );
        // Linear in the arguments: a hundred thousand scalar subscripts of
        // 1 and a subscript array of a million elements, each scalar judged
        // once rather than once per element.
        let t = std::time::Instant::now();
        let mut args = vec![row(&[2.0])];
        args.extend((0..100_000).map(|_| num(1.0)));
        args.push(row(&vec![1.0; 1_000_000]));
        assert_eq!(s2i(&args).unwrap().numel(), 1_000_000);
        assert!(t.elapsed().as_secs() < 10, "{:?}", t.elapsed());
        // A stride past what a double holds is `Inf`; a subscript of 1 adds
        // nothing to the index rather than `0 * Inf`, a scalar or an array.
        let wide = row(&[1e300, 1e300, 2.0]);
        let ones = [num(1.0), num(1.0), num(1.0)];
        let mut args = vec![wide.clone()];
        args.extend(ones.iter().cloned());
        assert_eq!(s2i(&args).unwrap().data, [1.0]);
        let got = s2i(&[wide, row(&[1.0, 2.0]), num(1.0), row(&[1.0, 1.0])]).unwrap();
        assert_eq!(got.data, [1.0, 2.0]);
        let mut args = vec![row(&vec![2.0; 2000])];
        args.extend((0..2000).map(|p| num(if p == 1 { 2.0 } else { 1.0 })));
        assert_eq!(s2i(&args).unwrap().data, [3.0]);
    }

    /// Cycle 14c: `ind2sub` with one output to more than the dimensions,
    /// each set of subscripts indexing the element the index does, the
    /// sizes from the `k`th on folded and the last dimension unbounded.
    #[test]
    fn ind2sub_folds_and_leaves_the_last_dimension_unbounded() {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        it.run("A = reshape(1:24, 2, 3, 4);").unwrap();
        let sz = row(&[2.0, 3.0, 4.0]);
        let every = Value::Mat(Matrix::row((1..=24).map(f64::from).collect()));
        for k in 1..=5 {
            let subs = run("ind2sub", &[sz.clone(), every.clone()], k);
            assert_eq!(subs.len(), k);
            for n in 0..24 {
                let at: Vec<String> = subs.iter().map(|s| s.data[n].to_string()).collect();
                let src = format!("assert(A({}) == {});", at.join(", "), n + 1);
                assert!(it.run(&src).is_ok(), "{src}");
            }
        }
        // One output is the index itself; two read [2 3 4] as 2x12.
        let one = run("ind2sub", &[sz.clone(), every.clone()], 1);
        assert_eq!(one[0].data, (1..=24).map(f64::from).collect::<Vec<_>>());
        let two = run("ind2sub", &[sz.clone(), num(24.0)], 2);
        assert_eq!((two[0].data[0], two[1].data[0]), (2.0, 12.0));
        // The last dimension has no bound: the page's [3 1] example, and an
        // index past the size.
        let rc = run(
            "ind2sub",
            &[row(&[3.0, 1.0]), row(&[9.0, 11.0, 13.0, 14.0])],
            2,
        );
        assert_eq!(
            (rc[0].data.clone(), rc[1].data.clone()),
            (vec![3.0, 2.0, 1.0, 2.0], vec![3.0, 4.0, 5.0, 5.0])
        );
        let past = run("ind2sub", &[row(&[2.0, 3.0]), num(7.0)], 3);
        assert_eq!(
            past.iter().map(|m| m.data[0]).collect::<Vec<_>>(),
            [1.0, 1.0, 2.0]
        );
        let huge = run("ind2sub", &[row(&[2.0, 3.0]), num(2f64.powi(52))], 2);
        assert_eq!((huge[0].data[0], huge[1].data[0]), (2.0, 2f64.powi(51)));
        // Each output has the index's shape, an N-D one included.
        let pages = Value::Mat(Matrix::from_dims(&[1, 1, 2], vec![5.0, 6.0]));
        let got = run("ind2sub", &[row(&[2.0, 3.0]), pages], 2);
        assert!(got.iter().all(|m| m.dims() == [1, 1, 2]));
        // The refusals.
        let index = "ind2sub's indices must be positive integers.";
        for bad in [0.0, -1.0, 1.5, f64::NAN, f64::INFINITY] {
            let mut i = Interp::with_output(Box::new(std::io::sink()));
            let e = builtin("ind2sub")(&mut i, &[sz.clone(), num(bad)], 1)
                .unwrap_err()
                .msg;
            assert_eq!(e, index, "{bad}");
        }
        let mut i = Interp::with_output(Box::new(std::io::sink()));
        let e = builtin("ind2sub")(&mut i, &[row(&[2.0, -3.0]), num(1.0)], 1)
            .unwrap_err()
            .msg;
        assert_eq!(e, "ind2sub's size must be a vector of positive integers.");
        // The outputs are judged together before any is written, their
        // elements and their lists of sizes: a hundred thousand outputs of a
        // hundred thousand indices, or a thousand of an index of 2^20
        // dimensions, are refused at once.
        let t = std::time::Instant::now();
        let many = Value::Mat(Matrix::row((1..=100_000).map(f64::from).collect()));
        let e = builtin("ind2sub")(&mut i, &[sz.clone(), many], 100_000)
            .unwrap_err()
            .msg;
        assert_eq!(
            e,
            "Requested 100000x100000 array exceeds the maximum array size."
        );
        let mut dims = vec![1; (1 << 20) - 1];
        dims.push(2);
        let deep = Value::Mat(Matrix::from_dims(&dims, vec![1.0, 2.0]));
        let e = builtin("ind2sub")(&mut i, &[sz.clone(), deep.clone()], 1 << 10)
            .unwrap_err()
            .msg;
        assert_eq!(
            e,
            "Requested 1048576x1024 array exceeds the maximum array size."
        );
        assert!(t.elapsed().as_secs() < 2, "{:?}", t.elapsed());
        // Within the bound every output is written, once.
        let got = builtin("ind2sub")(&mut i, &[sz.clone(), deep], 2).unwrap();
        assert!(got.iter().all(|v| v.mat().unwrap().ndims() == 1 << 20));
        let got = builtin("ind2sub")(&mut i, &[sz.clone(), num(5.0)], 100_000).unwrap();
        assert_eq!(got.len(), 100_000);
    }
}
