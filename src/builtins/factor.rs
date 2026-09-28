//! Matrix factorisations (cycle 08): LU with partial pivoting, Householder
//! QR (plain, and column-pivoted for least squares), Cholesky, the cyclic
//! Jacobi method for a symmetric eigenproblem, Hessenberg reduction with the
//! shifted double QR iteration for a general real one, and the one-sided
//! Jacobi SVD.
//!
//! Everything here is pure numerics: matrices in, matrices out, no `Interp`
//! and no warnings, which the callers decide on from what is returned. Three
//! rules hold throughout:
//!
//! - **Column-major.** Every `Matrix` is read and written through
//!   `get`/`set` or `data[c * rows + r]`; the general eigensolver works on a
//!   row-per-`Vec` copy internally because its source (the Wilkinson and
//!   Reinsch `orthes` and `hqr2`, as JAMA transcribes them) is written that
//!   way, and converts back at the end.
//! - **Shapes are judged first.** A result whose shape is computed from the
//!   operands' (the `m`-by-`m` `Q` of a tall `qr`, the `n`-by-`k` solution
//!   of a system with no rows) goes through `check_shape` before anything is
//!   allocated.
//! - **Every iteration has a cap** (invariant 6). The Jacobi sweeps, the
//!   one-sided Jacobi SVD and the QR iteration each stop with
//!   `no_convergence` past theirs, and `eig` and `svd` refuse a non-finite
//!   input before iterating at all. The caps are parameters so the unit tests
//!   can reach the error.

use super::args::check_shape;
use crate::error;
use crate::interp::R;
use crate::value::Matrix;

const EPS: f64 = f64::EPSILON;

/// Sweeps the symmetric Jacobi method may take. Convergence is quadratic,
/// and ten sweeps is already rare.
pub const JACOBI_SWEEPS: usize = 100;
/// Sweeps the one-sided Jacobi SVD may take.
pub const SVD_SWEEPS: usize = 100;

/// QR iterations the general eigensolver may spend on one eigenvalue (or
/// pair) before deflating: LAPACK's `dlahqr` allows `30 * max(10, n)`.
pub fn qr_iterations(n: usize) -> usize {
    30 * n.max(10)
}

/// A zero matrix whose shape was computed rather than asked for, judged
/// before it is allocated.
pub fn zeros(rows: usize, cols: usize) -> R<Matrix> {
    let (r, c) = check_shape(rows as f64, cols as f64)?;
    Ok(Matrix::filled(r, c, 0.0))
}

pub fn eye(n: usize) -> R<Matrix> {
    let mut m = zeros(n, n)?;
    for i in 0..n {
        m.set(i, i, 1.0);
    }
    Ok(m)
}

/// A double working copy, whatever the class of `a`.
fn work(a: &Matrix) -> Matrix {
    Matrix::new(a.rows, a.cols, a.data.clone())
}

pub fn all_finite(a: &Matrix) -> bool {
    a.data.iter().all(|v| v.is_finite())
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |acc, x| acc.max(x.abs()))
}

/// The 2-norm of `v`, scaled by its largest magnitude so that it neither
/// overflows nor underflows. A `NaN` anywhere is `NaN`.
fn norm2(v: &[f64]) -> f64 {
    if v.iter().any(|x| x.is_nan()) {
        return f64::NAN;
    }
    let s = max_abs(v);
    if s == 0.0 || !s.is_finite() {
        return s;
    }
    s * v.iter().map(|x| (x / s) * (x / s)).sum::<f64>().sqrt()
}

/// Column `j` of `m` as a slice.
fn col(m: &Matrix, j: usize) -> &[f64] {
    &m.data[j * m.rows..(j + 1) * m.rows]
}

/// `m` with its columns taken in the order `order`.
fn permute_cols(m: &Matrix, order: &[usize]) -> Matrix {
    let mut out = Matrix::filled(m.rows, order.len(), 0.0);
    for (to, &from) in order.iter().enumerate() {
        out.data[to * m.rows..(to + 1) * m.rows].copy_from_slice(col(m, from));
    }
    out
}

/// Columns `from..to` of `m`.
pub fn cols_range(m: &Matrix, from: usize, to: usize) -> Matrix {
    Matrix::new(
        m.rows,
        to - from,
        m.data[from * m.rows..to * m.rows].to_vec(),
    )
}

// ---- LU --------------------------------------------------------------

/// `P * A = L * U` by Gaussian elimination with partial pivoting, the one
/// factorisation `det`, `inv`, a square `\` and `/`, `A^-n` and `lu` share.
///
/// `lu` holds LAPACK's packed form: the multipliers of `L` below the
/// diagonal (its unit diagonal implied) and `U` on and above it.
/// `perm[i]` is the row of `A` that is row `i` of `P * A`.
#[derive(Debug)]
pub struct Lu {
    pub lu: Matrix,
    pub perm: Vec<usize>,
    /// `+1` or `-1`, the parity of the row swaps.
    pub sign: f64,
    /// Some pivot was at or below `Matrix::singular_tol`, the test `det` and
    /// the solve have made since cycle 01d, with `<=` so that the all-zero
    /// matrix, whose tolerance is `0`, is singular. What both callers mean
    /// by singular.
    pub singular: bool,
    /// Some pivot was exactly zero, so the elimination could not divide by
    /// it (LAPACK's `info > 0`). `inv` then returns `Inf` everywhere.
    pub zero_pivot: bool,
}

/// Factors any `m`-by-`n` matrix. The pivot is the first element of largest
/// magnitude in its column, as BLAS `idamax` chooses it; a `NaN` never
/// displaces a number. A zero pivot column is left as it is, as LAPACK's
/// `dgetf2` leaves it.
///
/// The arithmetic is the one `det` and the solve used before cycle 08:
/// a multiplier is `a(i, k) / a(k, k)` and an update is
/// `a(i, j) - l * a(k, j)`, in the same order, so every value they gave
/// before this rewrite they give again. `det([1 2; 3 4])` in particular is
/// still exactly `-2` (see the Design notes of the cycle 08 spec).
pub fn lu(a: &Matrix) -> Lu {
    let (m, n) = (a.rows, a.cols);
    let tol = a.singular_tol();
    let mut w = work(a);
    let mut perm: Vec<usize> = (0..m).collect();
    let (mut sign, mut singular, mut zero_pivot) = (1.0, false, false);
    for k in 0..m.min(n) {
        let mut p = k;
        let mut best = w.get(k, k).abs();
        for i in k + 1..m {
            let v = w.get(i, k).abs();
            if v > best {
                best = v;
                p = i;
            }
        }
        if best <= tol {
            singular = true;
        }
        if best == 0.0 {
            zero_pivot = true;
            continue;
        }
        if p != k {
            for j in 0..n {
                w.data.swap(j * m + k, j * m + p);
            }
            perm.swap(k, p);
            sign = -sign;
        }
        let pivot = w.get(k, k);
        for i in k + 1..m {
            let l = w.get(i, k) / pivot;
            w.set(i, k, l);
        }
        for j in k + 1..n {
            let u = w.get(k, j);
            for i in k + 1..m {
                let v = w.get(i, j) - w.get(i, k) * u;
                w.set(i, j, v);
            }
        }
    }
    Lu {
        lu: w,
        perm,
        sign,
        singular,
        zero_pivot,
    }
}

impl Lu {
    /// The determinant of a square factor: the sign of the permutation times
    /// the pivots, multiplied in order, or exactly `0` when a pivot was
    /// singular. That is the rule `det` has had since cycle 01d, and it is
    /// what makes `det` and `\` agree.
    pub fn det(&self) -> f64 {
        if self.singular {
            return 0.0;
        }
        let mut d = self.sign;
        for k in 0..self.lu.rows {
            d *= self.lu.get(k, k);
        }
        d
    }

    /// `X` with `A * X = B` for a square factor, by forward and back
    /// substitution. Nothing is refused: a zero pivot divides, and the
    /// `Inf` or `NaN` it makes is the answer, as MATLAB's is.
    #[allow(clippy::needless_range_loop)]
    pub fn solve(&self, b: &Matrix) -> R<Matrix> {
        let n = self.lu.rows;
        let mut x = zeros(n, b.cols)?;
        for j in 0..b.cols {
            let c = &mut x.data[j * n..(j + 1) * n];
            for (i, v) in c.iter_mut().enumerate() {
                *v = b.get(self.perm[i], j);
            }
            for k in 0..n {
                let xk = c[k];
                for i in k + 1..n {
                    c[i] -= self.lu.get(i, k) * xk;
                }
            }
            for k in (0..n).rev() {
                let mut s = c[k];
                for i in k + 1..n {
                    s -= self.lu.get(k, i) * c[i];
                }
                c[k] = s / self.lu.get(k, k);
            }
        }
        Ok(x)
    }

    /// The inverse of a square factor: `Inf` everywhere when a pivot was
    /// exactly zero, as MATLAB and Octave give for `inv([1 2; 2 4])` and
    /// `inv(0)` (QA D26), and `A \ I` otherwise.
    pub fn inverse(&self) -> R<Matrix> {
        let n = self.lu.rows;
        if self.zero_pivot {
            let (r, c) = check_shape(n as f64, n as f64)?;
            return Ok(Matrix::filled(r, c, f64::INFINITY));
        }
        self.solve(&eye(n)?)
    }

    /// `L` (`m`-by-`min(m, n)`, unit lower triangular), `U`
    /// (`min(m, n)`-by-`n`, upper triangular) and `P` (`m`-by-`m`), with
    /// `P * A = L * U`.
    pub fn factors(&self) -> R<(Matrix, Matrix, Matrix)> {
        let (m, n) = (self.lu.rows, self.lu.cols);
        let k = m.min(n);
        let mut l = zeros(m, k)?;
        let mut u = zeros(k, n)?;
        let mut p = zeros(m, m)?;
        for j in 0..k {
            l.set(j, j, 1.0);
            for i in j + 1..m {
                l.set(i, j, self.lu.get(i, j));
            }
        }
        for j in 0..n {
            for i in 0..k.min(j + 1) {
                u.set(i, j, self.lu.get(i, j));
            }
        }
        for (i, &r) in self.perm.iter().enumerate() {
            p.set(i, r, 1.0);
        }
        Ok((l, u, p))
    }
}

// ---- QR --------------------------------------------------------------

/// A Householder reflector `H = I - 2 v v' / (v' v)` that maps `x` to
/// `beta * e1`, with `beta = -sign(x(1)) * norm(x)` as LAPACK's `dlarfg`
/// chooses it. `None` when there is nothing below the first element to
/// annihilate, where `dlarfg` takes `H = I`. The vector is built from
/// `x / max|x|`, so the reflector neither overflows nor underflows.
fn reflector(x: &[f64]) -> Option<(Vec<f64>, f64, f64)> {
    if x[1..].iter().all(|&v| v == 0.0) {
        return None;
    }
    let s = max_abs(x);
    let mut v: Vec<f64> = x.iter().map(|e| e / s).collect();
    let norm = v.iter().map(|e| e * e).sum::<f64>().sqrt();
    let beta = if v[0] >= 0.0 { -norm } else { norm };
    v[0] -= beta;
    let vtv = v.iter().map(|e| e * e).sum::<f64>();
    Some((v, vtv, beta * s))
}

/// Applies the reflector `(v, vtv)` to rows `k..` of every column of `m`
/// from `from` on.
fn reflect_cols(m: &mut Matrix, v: &[f64], vtv: f64, k: usize, from: usize) {
    let rows = m.rows;
    for j in from..m.cols {
        let c = &mut m.data[j * rows + k..(j + 1) * rows];
        let d: f64 = c.iter().zip(v).map(|(a, b)| a * b).sum();
        let f = 2.0 * d / vtv;
        for (a, b) in c.iter_mut().zip(v) {
            *a -= f * b;
        }
    }
}

/// Householder QR, `A = Q * R` with `Q` `m`-by-`m` orthogonal and `R`
/// `m`-by-`n` upper triangular; `Q` only when asked for. `R`'s diagonal
/// takes LAPACK's signs, so it may be negative, and everything below it is
/// an exact `+0`. There is no iteration; a `NaN` or an `Inf` in `A` spreads
/// through the arithmetic to a `NaN` result.
pub fn qr(a: &Matrix, want_q: bool) -> R<(Option<Matrix>, Matrix)> {
    let (m, n) = (a.rows, a.cols);
    let mut r = work(a);
    let mut q = if want_q { Some(eye(m)?) } else { None };
    for k in 0..m.min(n) {
        let x = r.data[k * m + k..(k + 1) * m].to_vec();
        if let Some((v, vtv, beta)) = reflector(&x) {
            reflect_cols(&mut r, &v, vtv, k, k + 1);
            r.set(k, k, beta);
            if let Some(q) = q.as_mut() {
                // Q = Q * H, row by row.
                for i in 0..m {
                    let d: f64 = (k..m).map(|c| q.get(i, c) * v[c - k]).sum();
                    let f = 2.0 * d / vtv;
                    for c in k..m {
                        let e = q.get(i, c) - f * v[c - k];
                        q.set(i, c, e);
                    }
                }
            }
        }
        for i in k + 1..m {
            r.set(i, k, 0.0);
        }
    }
    Ok((q, r))
}

/// The least-squares solution of `A * X = B` for any `A`, by Householder QR
/// with column pivoting: `A * P = Q * R`, the numerical rank `r` is the
/// number of leading diagonal elements of `R` above
/// `max(m, n) * eps * |R(1, 1)|`, and `X` is the basic solution, the one
/// with at most `r` non-zero rows, which is the one MATLAB's `\` documents
/// for a rank-deficient system. Returns `X` (`n`-by-`k`) and `r`.
///
/// An `A` with a `NaN` or an `Inf` has no meaningful rank; the answer is
/// then `NaN` everywhere and the rank is reported as full, so no warning is
/// raised over it.
#[allow(clippy::needless_range_loop)]
pub fn lstsq(a: &Matrix, b: &Matrix) -> R<(Matrix, usize)> {
    let (m, n, k) = (a.rows, a.cols, b.cols);
    let steps = m.min(n);
    let mut x = zeros(n, k)?;
    if !all_finite(a) {
        x.data.iter_mut().for_each(|v| *v = f64::NAN);
        return Ok((x, steps));
    }
    let mut r = work(a);
    let mut c = work(b);
    let mut perm: Vec<usize> = (0..n).collect();
    for j in 0..steps {
        let mut p = j;
        let mut best = norm2(&r.data[j * m + j..(j + 1) * m]);
        for q in j + 1..n {
            let v = norm2(&r.data[q * m + j..(q + 1) * m]);
            if v > best {
                best = v;
                p = q;
            }
        }
        if p != j {
            for i in 0..m {
                r.data.swap(j * m + i, p * m + i);
            }
            perm.swap(j, p);
        }
        let xj = r.data[j * m + j..(j + 1) * m].to_vec();
        if let Some((v, vtv, beta)) = reflector(&xj) {
            reflect_cols(&mut r, &v, vtv, j, j + 1);
            reflect_cols(&mut c, &v, vtv, j, 0);
            r.set(j, j, beta);
        }
        for i in j + 1..m {
            r.set(i, j, 0.0);
        }
    }
    let tol = if steps > 0 {
        m.max(n) as f64 * EPS * r.get(0, 0).abs()
    } else {
        0.0
    };
    let mut rank = 0;
    while rank < steps && r.get(rank, rank).abs() > tol {
        rank += 1;
    }
    let mut y = vec![0.0; rank];
    for j in 0..k {
        for i in (0..rank).rev() {
            let mut s = c.get(i, j);
            for t in i + 1..rank {
                s -= r.get(i, t) * y[t];
            }
            y[i] = s / r.get(i, i);
        }
        for i in 0..rank {
            x.set(perm[i], j, y[i]);
        }
    }
    Ok((x, rank))
}

// ---- Cholesky --------------------------------------------------------

/// The upper triangular `R` with `R' * R = A`, read from the diagonal and
/// upper triangle of `A` alone, as MATLAB's `chol` reads it. When a pivot
/// is not positive (a `NaN` included, since `NaN > 0` is false) the answer
/// is `Err(j)` with `j` the zero-based column that failed, together with
/// the leading `j`-by-`j` block computed so far, which is what
/// `[R, p] = chol(A)` returns.
pub fn chol(a: &Matrix) -> R<(Matrix, Option<usize>)> {
    let n = a.rows;
    let mut r = zeros(n, n)?;
    for j in 0..n {
        let mut s = a.get(j, j);
        for k in 0..j {
            s -= r.get(k, j) * r.get(k, j);
        }
        if s.is_nan() || s <= 0.0 {
            let mut lead = zeros(j, j)?;
            for c in 0..j {
                for i in 0..=c {
                    lead.set(i, c, r.get(i, c));
                }
            }
            return Ok((lead, Some(j)));
        }
        let d = s.sqrt();
        r.set(j, j, d);
        for i in j + 1..n {
            let mut t = a.get(j, i);
            for k in 0..j {
                t -= r.get(k, j) * r.get(k, i);
            }
            r.set(j, i, t / d);
        }
    }
    Ok((r, None))
}

// ---- eigenvalues -----------------------------------------------------

/// Whether `a` is exactly symmetric, which is how MATLAB's `eig` chooses
/// its symmetric solver.
pub fn is_symmetric(a: &Matrix) -> bool {
    a.rows == a.cols && (0..a.rows).all(|i| (0..i).all(|j| a.get(i, j) == a.get(j, i)))
}

/// Eigenvalues in ascending order and orthonormal eigenvectors of a
/// symmetric matrix, by the cyclic Jacobi method on `A / max|A|`.
///
/// A rotation is skipped when its off-diagonal element is negligible
/// against the two diagonal ones it couples (the Demmel-Veselic test,
/// `|a_pq| <= eps * sqrt(|a_pp| * |a_qq|)`) or against the whole matrix
/// (`|a_pq| <= eps^2 * ||A||_F`, for a pair of zero diagonals), and the
/// method has converged when a sweep skips every rotation. The caller has
/// refused a non-finite input already.
pub fn eig_sym(a: &Matrix, max_sweeps: usize) -> R<(Vec<f64>, Matrix)> {
    let n = a.rows;
    let scale = max_abs(&a.data);
    let mut w = work(a);
    let mut v = eye(n)?;
    if scale > 0.0 {
        w.data.iter_mut().for_each(|x| *x /= scale);
    }
    let floor = EPS * EPS * norm2(&w.data);
    let mut converged = false;
    for _ in 0..max_sweeps {
        let mut rotated = false;
        for p in 0..n {
            for q in p + 1..n {
                let apq = w.get(p, q);
                let (app, aqq) = (w.get(p, p), w.get(q, q));
                if apq.abs() <= floor || apq.abs() <= EPS * app.abs().sqrt() * aqq.abs().sqrt() {
                    continue;
                }
                rotated = true;
                let theta = (aqq - app) / (2.0 * apq);
                let sgn = if theta >= 0.0 { 1.0 } else { -1.0 };
                let t = sgn / (theta.abs() + theta.hypot(1.0));
                let c = 1.0 / t.hypot(1.0);
                let s = t * c;
                rotate_cols(&mut w, p, q, c, s);
                for k in 0..n {
                    let (x, y) = (w.get(p, k), w.get(q, k));
                    w.set(p, k, c * x - s * y);
                    w.set(q, k, s * x + c * y);
                }
                w.set(p, q, 0.0);
                w.set(q, p, 0.0);
                rotate_cols(&mut v, p, q, c, s);
            }
        }
        if !rotated {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(error::no_convergence("eig"));
    }
    let vals: Vec<f64> = (0..n).map(|i| w.get(i, i) * scale).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| vals[i].total_cmp(&vals[j]));
    let sorted = order.iter().map(|&i| vals[i]).collect();
    Ok((sorted, permute_cols(&v, &order)))
}

/// Columns `p` and `q` of `m` become `c * p - s * q` and `s * p + c * q`.
fn rotate_cols(m: &mut Matrix, p: usize, q: usize, c: f64, s: f64) {
    let rows = m.rows;
    for i in 0..rows {
        let (x, y) = (m.data[p * rows + i], m.data[q * rows + i]);
        m.data[p * rows + i] = c * x - s * y;
        m.data[q * rows + i] = s * x + c * y;
    }
}

/// Eigenvalues and unit eigenvectors of a general real matrix: Householder
/// reduction to Hessenberg form, then the shifted double QR iteration with
/// back-substitution for the vectors (EISPACK's `orthes` and `hqr2`, in
/// JAMA's transcription), on `A / max|A|`. The eigenvalues come in the
/// order the iteration deflates them, which, as for MATLAB's
/// non-symmetric `eig`, is no particular order.
///
/// A complex pair is refused with the cycle-10 refusal as soon as it is
/// found, never returned as a wrong real answer. `max_iter` bounds the
/// iterations spent on any one eigenvalue; past it the answer is
/// `no_convergence`. The caller has refused a non-finite input already.
pub fn eig_general(a: &Matrix, max_iter: usize) -> R<(Vec<f64>, Matrix)> {
    let nn = a.rows;
    let scale = max_abs(&a.data);
    let s = if scale > 0.0 { scale } else { 1.0 };
    let mut h: Vec<Vec<f64>> = (0..nn)
        .map(|i| (0..nn).map(|j| a.get(i, j) / s).collect())
        .collect();
    let mut v: Vec<Vec<f64>> = (0..nn)
        .map(|i| (0..nn).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    orthes(&mut h, &mut v);
    let d = hqr2(&mut h, &mut v, max_iter)?;
    let mut vecs = zeros(nn, nn)?;
    for (i, row) in v.iter().enumerate() {
        for (j, x) in row.iter().enumerate() {
            vecs.set(i, j, *x);
        }
    }
    // Unit 2-norm columns, as MATLAB normalises them.
    for j in 0..nn {
        let len = norm2(col(&vecs, j));
        if len > 0.0 {
            vecs.data[j * nn..(j + 1) * nn]
                .iter_mut()
                .for_each(|x| *x /= len);
        }
    }
    Ok((d.iter().map(|x| x * s).collect(), vecs))
}

/// Reduction to upper Hessenberg form by orthogonal similarity
/// transformations, accumulating them into `v`.
#[allow(clippy::needless_range_loop)]
fn orthes(h: &mut [Vec<f64>], v: &mut [Vec<f64>]) {
    let n = h.len();
    if n < 3 {
        return;
    }
    let high = n - 1;
    let mut ort = vec![0.0; n];
    for m in 1..high {
        let scale: f64 = (m..=high).map(|i| h[i][m - 1].abs()).sum();
        if scale == 0.0 {
            continue;
        }
        let mut hh = 0.0;
        for i in (m..=high).rev() {
            ort[i] = h[i][m - 1] / scale;
            hh += ort[i] * ort[i];
        }
        let mut g = hh.sqrt();
        if ort[m] > 0.0 {
            g = -g;
        }
        hh -= ort[m] * g;
        ort[m] -= g;
        for j in m..n {
            let mut f = 0.0;
            for i in (m..=high).rev() {
                f += ort[i] * h[i][j];
            }
            f /= hh;
            for i in m..=high {
                h[i][j] -= f * ort[i];
            }
        }
        for i in 0..=high {
            let mut f = 0.0;
            for j in (m..=high).rev() {
                f += ort[j] * h[i][j];
            }
            f /= hh;
            for j in m..=high {
                h[i][j] -= f * ort[j];
            }
        }
        ort[m] *= scale;
        h[m][m - 1] = scale * g;
    }
    for m in (1..high).rev() {
        if h[m][m - 1] != 0.0 {
            for i in m + 1..=high {
                ort[i] = h[i][m - 1];
            }
            for j in m..=high {
                let mut g = 0.0;
                for i in m..=high {
                    g += ort[i] * v[i][j];
                }
                // Double division avoids a possible underflow.
                g = (g / ort[m]) / h[m][m - 1];
                for i in m..=high {
                    v[i][j] += g * ort[i];
                }
            }
        }
    }
}

/// The shifted double QR iteration on a Hessenberg `h`, accumulating into
/// `v`, then back-substitution for the eigenvectors. Real eigenvalues only:
/// a complex pair is the complex refusal.
#[allow(clippy::needless_range_loop)]
fn hqr2(h: &mut [Vec<f64>], v: &mut [Vec<f64>], max_iter: usize) -> R<Vec<f64>> {
    let nn = h.len();
    let mut d = vec![0.0; nn];
    if nn == 0 {
        return Ok(d);
    }
    let high = nn - 1;
    let mut exshift = 0.0;
    let (mut p, mut q, mut r, mut s, mut z);
    let (mut w, mut x, mut y);
    let mut norm = 0.0;
    for i in 0..nn {
        for j in i.saturating_sub(1)..nn {
            norm += h[i][j].abs();
        }
    }
    let mut iter = 0usize;
    // `n` counts down to -1, so it is signed; `nu` is its value as an index.
    let mut n = nn as isize - 1;
    while n >= 0 {
        let nu = n as usize;
        // Look for a single small sub-diagonal element.
        let mut l = nu;
        while l > 0 {
            s = h[l - 1][l - 1].abs() + h[l][l].abs();
            if s == 0.0 {
                s = norm;
            }
            // An exact zero always deflates: JAMA's `<` alone never does
            // for the zero matrix, whose norm, and so `s`, is 0.
            if h[l][l - 1] == 0.0 || h[l][l - 1].abs() < EPS * s {
                break;
            }
            l -= 1;
        }
        if l == nu {
            // One root found.
            h[nu][nu] += exshift;
            d[nu] = h[nu][nu];
            n -= 1;
            iter = 0;
        } else if l + 1 == nu {
            // Two roots found.
            w = h[nu][nu - 1] * h[nu - 1][nu];
            p = (h[nu - 1][nu - 1] - h[nu][nu]) / 2.0;
            q = p * p + w;
            z = q.abs().sqrt();
            h[nu][nu] += exshift;
            h[nu - 1][nu - 1] += exshift;
            x = h[nu][nu];
            if q < 0.0 {
                return Err(error::complex_eigenvalues());
            }
            z = if p >= 0.0 { p + z } else { p - z };
            d[nu - 1] = x + z;
            d[nu] = d[nu - 1];
            if z != 0.0 {
                d[nu] = x - w / z;
            }
            x = h[nu][nu - 1];
            s = x.abs() + z.abs();
            p = x / s;
            q = z / s;
            r = (p * p + q * q).sqrt();
            p /= r;
            q /= r;
            for j in nu - 1..nn {
                z = h[nu - 1][j];
                h[nu - 1][j] = q * z + p * h[nu][j];
                h[nu][j] = q * h[nu][j] - p * z;
            }
            for i in 0..=nu {
                z = h[i][nu - 1];
                h[i][nu - 1] = q * z + p * h[i][nu];
                h[i][nu] = q * h[i][nu] - p * z;
            }
            for i in 0..=high {
                z = v[i][nu - 1];
                v[i][nu - 1] = q * z + p * v[i][nu];
                v[i][nu] = q * v[i][nu] - p * z;
            }
            n -= 2;
            iter = 0;
        } else {
            // No convergence yet: form the shift.
            x = h[nu][nu];
            y = h[nu - 1][nu - 1];
            w = h[nu][nu - 1] * h[nu - 1][nu];
            // Wilkinson's original ad hoc shift.
            if iter == 10 {
                exshift += x;
                for i in 0..=nu {
                    h[i][i] -= x;
                }
                s = h[nu][nu - 1].abs() + h[nu - 1][nu - 2].abs();
                x = 0.75 * s;
                y = x;
                w = -0.4375 * s * s;
            }
            // The later ad hoc shift JAMA records as MATLAB's.
            if iter == 30 {
                s = (y - x) / 2.0;
                s = s * s + w;
                if s > 0.0 {
                    s = s.sqrt();
                    if y < x {
                        s = -s;
                    }
                    s = x - w / ((y - x) / 2.0 + s);
                    for i in 0..=nu {
                        h[i][i] -= s;
                    }
                    exshift += s;
                    x = 0.964;
                    y = x;
                    w = x;
                }
            }
            iter += 1;
            if iter > max_iter {
                return Err(error::no_convergence("eig"));
            }
            // Look for two consecutive small sub-diagonal elements.
            let mut m = nu - 2;
            loop {
                z = h[m][m];
                r = x - z;
                s = y - z;
                p = (r * s - w) / h[m + 1][m] + h[m][m + 1];
                q = h[m + 1][m + 1] - z - r - s;
                r = h[m + 2][m + 1];
                s = p.abs() + q.abs() + r.abs();
                p /= s;
                q /= s;
                r /= s;
                if m == l {
                    break;
                }
                if h[m][m - 1].abs() * (q.abs() + r.abs())
                    < EPS * (p.abs() * (h[m - 1][m - 1].abs() + z.abs() + h[m + 1][m + 1].abs()))
                {
                    break;
                }
                m -= 1;
            }
            for i in m + 2..=nu {
                h[i][i - 2] = 0.0;
                if i > m + 2 {
                    h[i][i - 3] = 0.0;
                }
            }
            // A double QR step on rows l..=nu and columns m..=nu.
            for k in m..nu {
                let notlast = k != nu - 1;
                if k != m {
                    p = h[k][k - 1];
                    q = h[k + 1][k - 1];
                    r = if notlast { h[k + 2][k - 1] } else { 0.0 };
                    x = p.abs() + q.abs() + r.abs();
                    if x == 0.0 {
                        continue;
                    }
                    p /= x;
                    q /= x;
                    r /= x;
                }
                s = (p * p + q * q + r * r).sqrt();
                if p < 0.0 {
                    s = -s;
                }
                if s != 0.0 {
                    if k != m {
                        h[k][k - 1] = -s * x;
                    } else if l != m {
                        h[k][k - 1] = -h[k][k - 1];
                    }
                    p += s;
                    x = p / s;
                    y = q / s;
                    z = r / s;
                    q /= p;
                    r /= p;
                    for j in k..nn {
                        p = h[k][j] + q * h[k + 1][j];
                        if notlast {
                            p += r * h[k + 2][j];
                            h[k + 2][j] -= p * z;
                        }
                        h[k][j] -= p * x;
                        h[k + 1][j] -= p * y;
                    }
                    for i in 0..=nu.min(k + 3) {
                        p = x * h[i][k] + y * h[i][k + 1];
                        if notlast {
                            p += z * h[i][k + 2];
                            h[i][k + 2] -= p * r;
                        }
                        h[i][k] -= p;
                        h[i][k + 1] -= p * q;
                    }
                    for i in 0..=high {
                        p = x * v[i][k] + y * v[i][k + 1];
                        if notlast {
                            p += z * v[i][k + 2];
                            v[i][k + 2] -= p * r;
                        }
                        v[i][k] -= p;
                        v[i][k + 1] -= p * q;
                    }
                }
            }
        }
    }
    if norm == 0.0 {
        // The zero matrix: every vector is an eigenvector, and `v` is `I`.
        return Ok(d);
    }
    // Back-substitute for the vectors of the upper triangular form.
    for nb in (0..nn).rev() {
        let p = d[nb];
        let mut l = nb;
        h[nb][nb] = 1.0;
        for i in (0..nb).rev() {
            let w = h[i][i] - p;
            let mut r = 0.0;
            for j in l..=nb {
                r += h[i][j] * h[j][nb];
            }
            l = i;
            h[i][nb] = if w != 0.0 { -r / w } else { -r / (EPS * norm) };
            // Overflow control.
            let t = h[i][nb].abs();
            if (EPS * t) * t > 1.0 {
                for j in i..=nb {
                    h[j][nb] /= t;
                }
            }
        }
    }
    // Back-transform to the vectors of the original matrix.
    for j in (0..nn).rev() {
        for i in 0..nn {
            let mut z = 0.0;
            for k in 0..=j {
                z += v[i][k] * h[k][j];
            }
            v[i][j] = z;
        }
    }
    Ok(d)
}

// ---- SVD -------------------------------------------------------------

/// Which singular vectors an SVD is asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vectors {
    /// The values alone: `svd(A)`, `rank`, `cond`, the matrix 2-norm.
    None,
    /// `U` `m`-by-`k` and `V` `n`-by-`k` with `k = min(m, n)`, a zero
    /// column where a singular value is zero: `pinv` and `orth`, which read
    /// only the columns of the non-zero values, and never need the
    /// `m`-by-`m` `U` of a long column.
    Thin,
    /// `U` `m`-by-`m` and `V` `n`-by-`n`, both orthogonal: `[U, S, V] =
    /// svd(A)` and `null`.
    Full,
}

/// A singular value decomposition, `A = U * S * V'`, singular values in
/// descending order, and the vectors that were asked for.
#[derive(Debug)]
pub struct Svd {
    pub s: Vec<f64>,
    pub u: Option<Matrix>,
    pub v: Option<Matrix>,
}

/// The SVD by one-sided Jacobi (Hestenes): columns of `A / max|A|` are
/// rotated in pairs until every pair is orthogonal to within
/// `m * eps` relative to their norms, with `V` accumulating the rotations;
/// the singular values are then the column norms. A wide matrix is done
/// through its transpose. At most `max_sweeps` sweeps, then
/// `no_convergence`; the caller has refused a non-finite input already.
pub fn svd(a: &Matrix, want: Vectors, max_sweeps: usize) -> R<Svd> {
    let wide = a.rows < a.cols;
    let (u, s, v) = if wide {
        jacobi_tall(&a.transpose(), max_sweeps)?
    } else {
        jacobi_tall(a, max_sweeps)?
    };
    // For a wide matrix the roles swap: A' = U1 S V1' gives A = V1 S U1'.
    let (u, v) = if wide { (v, u) } else { (u, v) };
    let (u, v) = match want {
        Vectors::None => (None, None),
        Vectors::Thin => (Some(u), Some(v)),
        Vectors::Full if wide => (Some(u), Some(complete(&v, &s)?)),
        Vectors::Full => (Some(complete(&u, &s)?), Some(v)),
    };
    Ok(Svd { s, u, v })
}

/// One-sided Jacobi on a tall (or square) `a`: the normalised columns of
/// `A * V` (a zero column where the singular value is zero), the singular
/// values, sorted descending, and `V`.
fn jacobi_tall(a: &Matrix, max_sweeps: usize) -> R<(Matrix, Vec<f64>, Matrix)> {
    let (m, n) = (a.rows, a.cols);
    let scale = max_abs(&a.data);
    let mut u = work(a);
    let mut v = eye(n)?;
    if scale > 0.0 {
        u.data.iter_mut().for_each(|x| *x /= scale);
    }
    let tol = m.max(1) as f64 * EPS;
    // A column whose norm has fallen to `eps^2 * ||A||_F` is roundoff left
    // over from a column the rotations have already cancelled. When `A` has
    // a zero row, that residue lies in the span of the other columns, so no
    // rotation can make it orthogonal to them: it only shrinks, a factor of
    // about `eps` a sweep, until its squared norm underflows to `0` and the
    // relative test above can never pass again (`svd([1 2 3; 4 5 7; 0 0 0])`
    // reached the cap this way). Such a column is set to exactly zero, so
    // its singular value is `0`, `U` gets a zero column there as for any
    // zero value, and `A * V - U * S` moves by less than `eps^2 * ||A||_F`.
    let floor = EPS * EPS * norm2(&u.data);
    let mut converged = false;
    for _ in 0..max_sweeps {
        let mut rotated = false;
        for p in 0..n {
            for q in p + 1..n {
                let (cp, cq) = (col(&u, p), col(&u, q));
                let alpha: f64 = cp.iter().map(|x| x * x).sum();
                let beta: f64 = cq.iter().map(|x| x * x).sum();
                let gamma: f64 = cp.iter().zip(cq).map(|(x, y)| x * y).sum();
                if gamma == 0.0 || gamma.abs() <= tol * alpha.sqrt() * beta.sqrt() {
                    continue;
                }
                let (np, nq) = (alpha.sqrt(), beta.sqrt());
                if np <= floor || nq <= floor {
                    let j = if np <= nq { p } else { q };
                    u.data[j * m..(j + 1) * m].fill(0.0);
                    rotated = true;
                    continue;
                }
                rotated = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let sgn = if zeta >= 0.0 { 1.0 } else { -1.0 };
                let t = sgn / (zeta.abs() + zeta.hypot(1.0));
                let c = 1.0 / t.hypot(1.0);
                let s = t * c;
                rotate_cols(&mut u, p, q, c, s);
                rotate_cols(&mut v, p, q, c, s);
            }
        }
        if !rotated {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(error::no_convergence("svd"));
    }
    let mut s: Vec<f64> = (0..n).map(|j| norm2(col(&u, j))).collect();
    for (j, sj) in s.iter_mut().enumerate() {
        if *sj > 0.0 {
            let len = *sj;
            u.data[j * m..(j + 1) * m]
                .iter_mut()
                .for_each(|x| *x /= len);
        }
        *sj *= scale;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| s[j].total_cmp(&s[i]));
    let sorted: Vec<f64> = order.iter().map(|&i| s[i]).collect();
    Ok((permute_cols(&u, &order), sorted, permute_cols(&v, &order)))
}

/// A full `m`-by-`m` orthogonal matrix whose leading columns are the
/// columns of `u` that belong to a non-zero singular value, completed with
/// an orthonormal basis of their complement: the trailing columns of the
/// `Q` of a Householder QR of those leading columns.
fn complete(u: &Matrix, s: &[f64]) -> R<Matrix> {
    let m = u.rows;
    let r = s.iter().take_while(|&&x| x > 0.0).count();
    let lead = cols_range(u, 0, r);
    let (q, _) = qr(&lead, true)?;
    let mut full = q.expect("qr was asked for Q");
    full.data[..m * r].copy_from_slice(&lead.data);
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rmat(rows: usize, cols: usize, row_major: &[f64]) -> Matrix {
        let mut m = Matrix::filled(rows, cols, 0.0);
        for (i, v) in row_major.iter().enumerate() {
            m.set(i / cols, i % cols, *v);
        }
        m
    }

    fn mul(a: &Matrix, b: &Matrix) -> Matrix {
        a.matmul(b).unwrap()
    }

    /// The largest absolute difference between two matrices of one shape.
    fn diff(a: &Matrix, b: &Matrix) -> f64 {
        assert_eq!((a.rows, a.cols), (b.rows, b.cols));
        a.data
            .iter()
            .zip(&b.data)
            .fold(0.0, |acc, (x, y)| f64::max(acc, (x - y).abs()))
    }

    /// `||Q' * Q - I||`, elementwise.
    fn orthogonality(q: &Matrix) -> f64 {
        diff(&mul(&q.transpose(), q), &Matrix::identity(q.cols, q.cols))
    }

    fn diag(v: &[f64], rows: usize, cols: usize) -> Matrix {
        let mut m = Matrix::filled(rows, cols, 0.0);
        for (i, x) in v.iter().enumerate() {
            m.set(i, i, *x);
        }
        m
    }

    /// A deterministic pseudo-random matrix with entries in [-1, 1).
    fn noise(rows: usize, cols: usize, seed: u64) -> Matrix {
        let mut state = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut m = Matrix::filled(rows, cols, 0.0);
        for v in m.data.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *v = ((state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0;
        }
        m
    }

    fn samples() -> Vec<Matrix> {
        vec![
            rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]),
            rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0]),
            rmat(3, 2, &[1.0, 1.0, 1.0, 2.0, 1.0, 3.0]),
            rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            rmat(3, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]),
            noise(5, 5, 1),
            noise(7, 4, 2),
            noise(4, 7, 3),
            Matrix::filled(3, 3, 0.0),
            Matrix::filled(1, 1, 5.0),
        ]
    }

    // ---- LU ----------------------------------------------------------

    #[test]
    fn lu_reconstructs_p_times_a() {
        for a in samples() {
            let f = lu(&a);
            let (l, u, p) = f.factors().unwrap();
            let scale = max_abs(&a.data).max(1.0);
            assert!(diff(&mul(&p, &a), &mul(&l, &u)) < 1e-12 * scale, "{a:?}");
            // L is unit lower triangular, U upper triangular.
            for j in 0..l.cols {
                assert_eq!(l.get(j, j), 1.0);
                for i in 0..j {
                    assert_eq!(l.get(i, j), 0.0);
                }
            }
            for j in 0..u.cols {
                for i in j + 1..u.rows {
                    assert_eq!(u.get(i, j), 0.0);
                }
            }
            // The multipliers are bounded by one: the pivoting is partial.
            assert!(l.data.iter().all(|x| x.abs() <= 1.0));
        }
    }

    #[test]
    fn lu_pivots_on_the_first_largest_element() {
        // |1| and |-1| tie; idamax takes the first, so no swap.
        let f = lu(&rmat(2, 2, &[1.0, 2.0, -1.0, 3.0]));
        assert_eq!(f.perm, [0, 1]);
        assert_eq!(f.sign, 1.0);
        let f = lu(&rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]));
        assert_eq!(f.perm, [1, 0]);
        assert_eq!(f.sign, -1.0);
    }

    /// The cycle-02 verify-first bullet: this LU keeps the old operation
    /// order, and the old order lands exactly on -2.
    #[test]
    fn det_of_one_two_three_four_is_still_exactly_minus_two() {
        assert_eq!(lu(&rmat(2, 2, &[1.0, 2.0, 3.0, 4.0])).det(), -2.0);
        assert_eq!(lu(&rmat(2, 2, &[4.0, -2.0, 1.0, 1.0])).det(), 6.0);
        assert_eq!(lu(&Matrix::empty()).det(), 1.0);
    }

    #[test]
    fn a_zero_pivot_is_flagged_and_inverts_to_inf() {
        let f = lu(&rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]));
        assert!(f.singular && f.zero_pivot);
        assert_eq!(f.det(), 0.0);
        let inv = f.inverse().unwrap();
        assert!(inv.data.iter().all(|&x| x == f64::INFINITY));
        let f = lu(&Matrix::scalar(0.0));
        assert!(f.zero_pivot);
        assert_eq!(f.inverse().unwrap().data, [f64::INFINITY]);
        // A well-conditioned matrix is neither.
        let f = lu(&rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]));
        assert!(!f.singular && !f.zero_pivot);
    }

    #[test]
    fn lu_solve_and_inverse_agree_with_the_matrix() {
        let a = noise(6, 6, 9);
        let f = lu(&a);
        let b = noise(6, 2, 10);
        let x = f.solve(&b).unwrap();
        assert!(diff(&mul(&a, &x), &b) < 1e-10);
        let inv = f.inverse().unwrap();
        assert!(diff(&mul(&a, &inv), &Matrix::identity(6, 6)) < 1e-10);
    }

    #[test]
    fn a_nan_in_lu_spreads_without_hanging() {
        let f = lu(&rmat(2, 2, &[f64::NAN, 1.0, 1.0, 1.0]));
        let (l, u, _) = f.factors().unwrap();
        assert!(l.data.iter().chain(&u.data).any(|x| x.is_nan()));
    }

    // ---- QR ----------------------------------------------------------

    #[test]
    fn qr_reconstructs_and_q_is_orthogonal() {
        for a in samples() {
            let (q, r) = qr(&a, true).unwrap();
            let q = q.unwrap();
            assert_eq!((q.rows, q.cols), (a.rows, a.rows));
            assert_eq!((r.rows, r.cols), (a.rows, a.cols));
            assert!(orthogonality(&q) < 1e-13, "{a:?}");
            let scale = max_abs(&a.data).max(1.0);
            assert!(diff(&mul(&q, &r), &a) < 1e-12 * scale, "{a:?}");
            for j in 0..r.cols {
                for i in j + 1..r.rows {
                    let x = r.get(i, j);
                    assert!(x == 0.0 && x.is_sign_positive());
                }
            }
        }
    }

    #[test]
    fn qr_of_one_two_three_four() {
        let (_, r) = qr(&rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]), false).unwrap();
        assert!((r.get(0, 0).abs() - 10f64.sqrt()).abs() < 1e-14);
        // An upper triangular input is its own R, with Q = I, as LAPACK has it.
        let t = rmat(2, 2, &[2.0, 1.0, 0.0, 3.0]);
        let (q, r) = qr(&t, true).unwrap();
        assert_eq!(r, t);
        assert_eq!(q.unwrap(), Matrix::identity(2, 2));
    }

    #[test]
    fn qr_neither_overflows_nor_underflows() {
        for scale in [1e-300, 1e300] {
            let a = rmat(2, 2, &[1.0 * scale, 2.0 * scale, 3.0 * scale, 4.0 * scale]);
            let (q, r) = qr(&a, true).unwrap();
            assert!((r.get(0, 0).abs() / scale - 10f64.sqrt()).abs() < 1e-13);
            assert!(orthogonality(&q.unwrap()) < 1e-13);
        }
    }

    #[test]
    fn least_squares_fits_a_line() {
        let a = rmat(3, 2, &[1.0, 1.0, 1.0, 2.0, 1.0, 3.0]);
        let (x, rank) = lstsq(&a, &Matrix::col(vec![1.0, 2.0, 2.0])).unwrap();
        assert_eq!(rank, 2);
        assert!((x.get(0, 0) - 2.0 / 3.0).abs() < 1e-14);
        assert!((x.get(1, 0) - 0.5).abs() < 1e-14);
        // The residual is orthogonal to the columns: A' (A x - b) = 0.
        let r = mul(&a, &x);
        let res = Matrix::col(vec![r.data[0] - 1.0, r.data[1] - 2.0, r.data[2] - 2.0]);
        assert!(
            mul(&a.transpose(), &res)
                .data
                .iter()
                .all(|v| v.abs() < 1e-13)
        );
    }

    #[test]
    fn least_squares_of_a_wide_system_is_a_basic_solution() {
        // x + 2y + 3z = 6 has a basic solution with one non-zero: the
        // pivoted column is the largest, z.
        let a = rmat(1, 3, &[1.0, 2.0, 3.0]);
        let (x, rank) = lstsq(&a, &Matrix::scalar(6.0)).unwrap();
        assert_eq!(rank, 1);
        assert_eq!(x.data.iter().filter(|v| **v != 0.0).count(), 1);
        assert!((x.get(2, 0) - 2.0).abs() < 1e-14);
        // A full-rank wide system is solved exactly.
        let a = noise(3, 5, 4);
        let b = noise(3, 2, 5);
        let (x, rank) = lstsq(&a, &b).unwrap();
        assert_eq!(rank, 3);
        assert!(diff(&mul(&a, &x), &b) < 1e-12);
    }

    #[test]
    fn least_squares_reports_the_rank_of_a_deficient_system() {
        let a = rmat(3, 2, &[1.0, 2.0, 2.0, 4.0, 3.0, 6.0]);
        let (x, rank) = lstsq(&a, &Matrix::col(vec![1.0, 2.0, 3.0])).unwrap();
        assert_eq!(rank, 1);
        assert!(diff(&mul(&a, &x), &Matrix::col(vec![1.0, 2.0, 3.0])) < 1e-13);
        let (x, rank) = lstsq(&Matrix::filled(3, 2, 0.0), &Matrix::col(vec![1.0; 3])).unwrap();
        assert_eq!(rank, 0);
        assert_eq!(x.data, [0.0, 0.0]);
        let (x, _) = lstsq(
            &rmat(3, 2, &[f64::NAN, 1.0, 1.0, 1.0, 1.0, 1.0]),
            &Matrix::col(vec![1.0; 3]),
        )
        .unwrap();
        assert!(x.data.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn least_squares_judges_the_result_shape() {
        // No rows, and an n-by-k answer far too large to allocate.
        let a = Matrix::new(0, 100_000, vec![]);
        let b = Matrix::new(0, 100_000, vec![]);
        assert!(lstsq(&a, &b).is_err());
    }

    // ---- Cholesky ----------------------------------------------------

    #[test]
    fn chol_reconstructs_a_positive_definite_matrix() {
        let r = chol(&rmat(2, 2, &[4.0, 2.0, 2.0, 3.0])).unwrap();
        assert!(r.1.is_none());
        let r = r.0;
        assert_eq!(r.get(0, 0), 2.0);
        assert_eq!(r.get(0, 1), 1.0);
        assert!((r.get(1, 1) - 2f64.sqrt()).abs() < 1e-15);
        assert!(r.get(1, 0) == 0.0 && r.get(1, 0).is_sign_positive());
        let b = noise(6, 6, 7);
        let spd = mul(&b.transpose(), &b);
        let (r, fail) = chol(&spd).unwrap();
        assert!(fail.is_none());
        assert!(diff(&mul(&r.transpose(), &r), &spd) < 1e-12);
    }

    #[test]
    fn chol_names_the_column_that_failed() {
        let (lead, fail) = chol(&rmat(2, 2, &[1.0, 2.0, 2.0, 1.0])).unwrap();
        assert_eq!(fail, Some(1));
        assert_eq!(lead.data, [1.0]);
        let (lead, fail) = chol(&rmat(2, 2, &[-1.0, 0.0, 0.0, 1.0])).unwrap();
        assert_eq!((fail, lead.rows), (Some(0), 0));
        // NaN > 0 is false, so a NaN is not positive definite.
        let (_, fail) = chol(&rmat(2, 2, &[f64::NAN, 0.0, 0.0, 1.0])).unwrap();
        assert_eq!(fail, Some(0));
    }

    // ---- eigenvalues -------------------------------------------------

    fn check_eig(a: &Matrix, vals: &[f64], v: &Matrix, tol: f64) {
        let av = mul(a, v);
        let vd = mul(v, &diag(vals, vals.len(), vals.len()));
        let scale = max_abs(&a.data).max(1.0);
        assert!(diff(&av, &vd) < tol * scale, "{a:?}: {vals:?}");
        for j in 0..v.cols {
            assert!((norm2(col(v, j)) - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn jacobi_diagonalises_a_symmetric_matrix() {
        let (vals, v) = eig_sym(&rmat(2, 2, &[2.0, 1.0, 1.0, 2.0]), JACOBI_SWEEPS).unwrap();
        assert!((vals[0] - 1.0).abs() < 1e-14 && (vals[1] - 3.0).abs() < 1e-14);
        assert!(orthogonality(&v) < 1e-14);
        for seed in 0..6 {
            let b = noise(6, 6, seed);
            let s = b.transpose().zip(&b, "+", |x, y| x + y).unwrap();
            let (vals, v) = eig_sym(&s, JACOBI_SWEEPS).unwrap();
            check_eig(&s, &vals, &v, 1e-12);
            assert!(orthogonality(&v) < 1e-13);
            assert!(vals.windows(2).all(|w| w[0] <= w[1]), "ascending");
        }
        // Rank-deficient and zero-diagonal matrices converge too.
        for a in [
            Matrix::filled(4, 4, 1.0),
            rmat(2, 2, &[0.0, 1.0, 1.0, 0.0]),
            Matrix::filled(3, 3, 0.0),
        ] {
            let (vals, v) = eig_sym(&a, JACOBI_SWEEPS).unwrap();
            check_eig(&a, &vals, &v, 1e-13);
        }
    }

    #[test]
    fn jacobi_stops_at_its_cap() {
        let a = rmat(2, 2, &[2.0, 1.0, 1.0, 2.0]);
        let e = eig_sym(&a, 0).unwrap_err().msg;
        assert_eq!(e, "'eig' did not converge within its iteration limit.");
        // A diagonal matrix needs no rotation, so even a cap of one sweep
        // is enough.
        assert!(eig_sym(&rmat(2, 2, &[2.0, 0.0, 0.0, 3.0]), 1).is_ok());
    }

    #[test]
    fn the_qr_iteration_finds_real_eigenvalues() {
        let (mut vals, v) = eig_general(&rmat(2, 2, &[4.0, 1.0, 2.0, 3.0]), 300).unwrap();
        check_eig(&rmat(2, 2, &[4.0, 1.0, 2.0, 3.0]), &vals, &v, 1e-13);
        vals.sort_by(f64::total_cmp);
        assert!((vals[0] - 2.0).abs() < 1e-14 && (vals[1] - 5.0).abs() < 1e-14);
        // A non-symmetric matrix with known real eigenvalues 1, 2, 3, 4:
        // S * diag * S^-1 with a well-conditioned S.
        let s = rmat(
            4,
            4,
            &[
                2.0, 1.0, 0.0, 0.0, 1.0, 3.0, 1.0, 0.0, 0.0, 1.0, 4.0, 1.0, 0.5, 0.0, 1.0, 5.0,
            ],
        );
        let a = mul(
            &mul(&s, &diag(&[1.0, 2.0, 3.0, 4.0], 4, 4)),
            &lu(&s).inverse().unwrap(),
        );
        let (mut vals, v) = eig_general(&a, qr_iterations(4)).unwrap();
        check_eig(&a, &vals, &v, 1e-10);
        vals.sort_by(f64::total_cmp);
        for (got, want) in vals.iter().zip([1.0, 2.0, 3.0, 4.0]) {
            assert!((got - want).abs() < 1e-10, "{vals:?}");
        }
        // Triangular and zero matrices deflate at once.
        let t = rmat(3, 3, &[1.0, 5.0, 7.0, 0.0, 2.0, 3.0, 0.0, 0.0, 3.0]);
        let (vals, v) = eig_general(&t, qr_iterations(3)).unwrap();
        check_eig(&t, &vals, &v, 1e-13);
        let (vals, _) = eig_general(&Matrix::filled(3, 3, 0.0), qr_iterations(3)).unwrap();
        assert_eq!(vals, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_complex_pair_is_refused() {
        let e = eig_general(&rmat(2, 2, &[0.0, -1.0, 1.0, 0.0]), 300)
            .unwrap_err()
            .msg;
        assert!(e.starts_with("Complex results are not supported."), "{e}");
        // A 3x3 rotation about an axis has one real and two complex.
        let r = rmat(3, 3, &[0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 2.0]);
        assert!(eig_general(&r, 300).is_err());
    }

    #[test]
    fn the_qr_iteration_stops_at_its_cap() {
        // A 3x3 non-symmetric matrix that needs at least one iteration.
        let a = rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0]);
        let e = eig_general(&a, 0).unwrap_err().msg;
        assert_eq!(e, "'eig' did not converge within its iteration limit.");
        assert!(eig_general(&a, qr_iterations(3)).is_ok());
    }

    // ---- SVD ---------------------------------------------------------

    fn check_svd(a: &Matrix) {
        let f = svd(a, Vectors::Full, SVD_SWEEPS).unwrap();
        let (u, v) = (f.u.unwrap(), f.v.unwrap());
        let (m, n) = (a.rows, a.cols);
        assert_eq!((u.rows, u.cols, v.rows, v.cols), (m, m, n, n));
        assert_eq!(f.s.len(), m.min(n));
        assert!(orthogonality(&u) < 1e-13, "U of {a:?}");
        assert!(orthogonality(&v) < 1e-13, "V of {a:?}");
        let usv = mul(&mul(&u, &diag(&f.s, m, n)), &v.transpose());
        let scale = max_abs(&a.data).max(1.0);
        assert!(diff(&usv, a) < 1e-12 * scale, "{a:?}");
        assert!(f.s.windows(2).all(|w| w[0] >= w[1]), "descending");
        assert!(f.s.iter().all(|&x| x >= 0.0));
    }

    #[test]
    fn thin_factors_reconstruct_too() {
        for a in samples() {
            let f = svd(&a, Vectors::Thin, SVD_SWEEPS).unwrap();
            let (u, v) = (f.u.unwrap(), f.v.unwrap());
            let k = a.rows.min(a.cols);
            assert_eq!((u.rows, v.rows), (a.rows, a.cols));
            assert_eq!(u.cols.min(v.cols), k);
            let su = cols_range(&u, 0, k);
            let sv = cols_range(&v, 0, k);
            let usv = mul(&mul(&su, &diag(&f.s, k, k)), &sv.transpose());
            let scale = max_abs(&a.data).max(1.0);
            assert!(diff(&usv, &a) < 1e-12 * scale, "{a:?}");
        }
        // A long column's thin U is the column itself, normalised.
        let tall = Matrix::filled(100_000, 1, 1.0);
        let f = svd(&tall, Vectors::Thin, SVD_SWEEPS).unwrap();
        assert_eq!(f.u.unwrap().cols, 1);
    }

    #[test]
    fn svd_reconstructs_and_its_factors_are_orthogonal() {
        for a in samples() {
            check_svd(&a);
        }
        check_svd(&Matrix::col(vec![3.0, 4.0]));
        check_svd(&Matrix::row(vec![3.0, 4.0]));
        check_svd(&Matrix::filled(3, 2, 1.0));
        check_svd(&Matrix::new(0, 3, vec![]));
        check_svd(&Matrix::new(2, 0, vec![]));
        let f = svd(
            &rmat(2, 2, &[3.0, 0.0, 0.0, 4.0]),
            Vectors::None,
            SVD_SWEEPS,
        )
        .unwrap();
        assert_eq!(f.s, [4.0, 3.0]);
    }

    #[test]
    fn svd_of_a_rank_one_matrix_has_a_negligible_second_value() {
        let f = svd(
            &rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]),
            Vectors::None,
            SVD_SWEEPS,
        )
        .unwrap();
        assert!((f.s[0] - 5.0).abs() < 1e-14);
        assert!(f.s[1] < 2.0 * 5.0 * EPS, "{:?}", f.s);
    }

    #[test]
    fn svd_of_a_matrix_with_a_zero_row_converges() {
        // The cancelled column's residue lies in the span of the others and
        // only shrinks; before the floor it underflowed and hit the cap.
        let a = rmat(3, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 7.0, 0.0, 0.0, 0.0]);
        check_svd(&a);
        let f = svd(&a, Vectors::None, SVD_SWEEPS).unwrap();
        assert!(f.s[2] < 1e-14, "{:?}", f.s);
        for (seed, n) in [(1, 3), (2, 8), (3, 40)] {
            let mut a = noise(n, n, seed);
            for j in 0..n {
                a.set(0, j, 0.0);
            }
            check_svd(&a);
            check_svd(&a.transpose());
        }
    }

    #[test]
    fn svd_neither_overflows_nor_underflows() {
        for scale in [1e-300, 1e300] {
            let a = rmat(2, 2, &[3.0 * scale, 0.0, 0.0, 4.0 * scale]);
            let f = svd(&a, Vectors::None, SVD_SWEEPS).unwrap();
            assert!((f.s[0] / scale - 4.0).abs() < 1e-13);
            assert!((f.s[1] / scale - 3.0).abs() < 1e-13);
        }
    }

    #[test]
    fn svd_stops_at_its_cap() {
        let e = svd(&rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]), Vectors::None, 0)
            .unwrap_err()
            .msg;
        assert_eq!(e, "'svd' did not converge within its iteration limit.");
    }

    #[test]
    fn svd_judges_the_shape_of_u_before_allocating_it() {
        // A 100000x1 column has a 100000x100000 U, past the element limit.
        let tall = Matrix::filled(100_000, 1, 1.0);
        assert!(svd(&tall, Vectors::None, SVD_SWEEPS).is_ok());
        assert!(svd(&tall, Vectors::Full, SVD_SWEEPS).is_err());
    }
}
