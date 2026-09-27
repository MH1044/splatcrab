//! Linear algebra, array rearrangement, and search and sort.

use std::cmp::Ordering;

use super::args::{
    at_most, check_shape, dim, fmt_dim, mat, need, option, shape, size_list, trailing_ones,
};
use super::math::{reduce, sum0};
use super::{Registry, add, one_mat};
use crate::error;
use crate::interp::{Interp, R};
use crate::value::{Matrix, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    // ---- linear algebra ----------------------------------------------
    add(r, "transpose", transpose, "transpose(A) - the non-conjugate transpose of A.");
    add(r, "inv", inv, "inv(A) - the inverse of a square matrix.");
    add(r, "det", det, "det(A) - the determinant of a square matrix.");
    add(r, "trace", trace, "trace(A) - the sum of the diagonal of a square matrix.");
    add(r, "diag", diag, "diag(v,k) puts v on the k-th diagonal; diag(A,k) takes the k-th diagonal.");
    add(r, "norm", norm, "norm(v), norm(v,p) - the p-norm of a vector: 1, 2, p > 0, Inf, -Inf or 'fro'.");
    add(r, "dot", dot, "dot(A,B), dot(A,B,dim) - scalar products of vectors, or of matching columns.");

    // ---- rearrangement -----------------------------------------------
    add(r, "reshape", reshape, "reshape(A,r,c), reshape(A,sz), reshape(A,r,[]) - the elements of A in a new shape.");
    add(r, "repmat", repmat, "repmat(A,n), repmat(A,r,c), repmat(A,sz) - tile A into a block matrix.");
    add(r, "fliplr", fliplr, "fliplr(A) - reverse the order of the columns.");
    add(r, "flipud", flipud, "flipud(A) - reverse the order of the rows.");

    // ---- search and sort ---------------------------------------------
    add(r, "find", find, "find(A), find(A,n), find(A,n,'last') - indices of the non-zero elements.");
    add(r, "sort", sort, "sort(v), sort(v,dim), sort(v,'descend') - a sorted vector, NaN at the high end.");
}

// ---- linear algebra --------------------------------------------------

fn transpose(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "transpose")?;
    one_mat(mat(a, 0, "transpose")?.transpose())
}

fn inv(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "inv")?;
    one_mat(mat(a, 0, "inv")?.inv()?)
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
    one_mat(Matrix::scalar((0..m.rows).map(|i| m.get(i, i)).sum()))
}

/// `diag(v, k)` places `v` on the k-th diagonal of a square matrix of order
/// `numel(v) + abs(k)`, and `diag(A, k)` returns the k-th diagonal of `A` as
/// a column. `k > 0` is above the main diagonal. A `k` past the matrix gives
/// a 0x1, which is Octave's answer and MATLAB's shape for an empty diagonal.
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
        one_mat(out)
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
        one_mat(Matrix::col(
            (0..len).map(|i| m.get(r0 + i, c0 + i)).collect(),
        ))
    }
}

/// The order of a vector norm.
#[derive(Clone, Copy, Debug, PartialEq)]
enum NormType {
    /// A positive, finite `p`; `2` is the default and `'fro'`.
    P(f64),
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
            Ok(NormType::P(2.0))
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
    let p = if a.len() >= 2 {
        norm_type(a)?
    } else {
        NormType::P(2.0)
    };
    if !m.is_vector() && !m.is_empty() {
        return Err(error::norm_vectors_only());
    }
    one_mat(Matrix::scalar(vector_norm(&m.data, p)))
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
        NormType::P(p) => {
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
    one_mat(reduce(&products, d, sum0))
}

// ---- rearrangement ---------------------------------------------------

/// `reshape(A, r, c, ...)`, `reshape(A, sz)`, and one `[]` placeholder among
/// the sizes for the one that makes the count come out. Trailing sizes of `1`
/// are dropped; any other third size is the N-D error.
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
    let dims: Vec<f64> = list.into_iter().map(|d| d.unwrap_or(fill)).collect();
    let (r, c) = trailing_ones(&dims)?;
    let (r, c) = check_shape(r, c)?;
    if r * c != total {
        return Err(error::reshape_numel(total, r, c));
    }
    one_mat(Matrix::new(r, c, m.data))
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
    let mut out = Matrix::filled(rows, cols, 0.0);
    for j in 0..out.cols {
        for i in 0..out.rows {
            out.set(i, j, m.get(i % m.rows.max(1), j % m.cols.max(1)));
        }
    }
    one_mat(out)
}

fn fliplr(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "fliplr")?;
    let m = mat(a, 0, "fliplr")?;
    let mut out = m.clone();
    for c in 0..m.cols {
        for r in 0..m.rows {
            out.set(r, m.cols - 1 - c, m.get(r, c));
        }
    }
    one_mat(out)
}

fn flipud(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "flipud")?;
    let m = mat(a, 0, "flipud")?;
    let mut out = m.clone();
    for c in 0..m.cols {
        for r in 0..m.rows {
            out.set(m.rows - 1 - r, c, m.get(r, c));
        }
    }
    one_mat(out)
}

// ---- search and sort -------------------------------------------------

/// `find(X)`, `find(X, n)`, `find(X, n, 'first')` and `find(X, n, 'last')`.
/// The last `n` indices stay in ascending order, as MATLAB returns them, and
/// the result is a row for a row vector and a column otherwise.
fn find(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
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
    one_mat(if m.rows == 1 {
        Matrix::row(idx)
    } else {
        Matrix::col(idx)
    })
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

/// `sort(v)`, `sort(v, direction)`, `sort(v, dim)` and
/// `sort(v, dim, direction)` for a vector. Along a dimension the vector does
/// not extend in, every slice has one element and nothing moves.
///
/// Descending is `sort_cmp` reversed rather than the ascending result
/// reversed: that keeps it stable, as the MATLAB page requires "regardless of
/// sorting direction", and puts `NaN` first, the documented placement.
fn sort(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
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
    if !m.is_vector() && !m.is_empty() {
        return Err(error::sort_vectors_only());
    }
    let along = d.unwrap_or(if m.rows == 1 { 2 } else { 1 });
    let extent = match along {
        1 => m.rows,
        2 => m.cols,
        _ => 1,
    };
    let mut out = m;
    if extent == out.numel() {
        if descend {
            out.data.sort_by(|x, y| sort_cmp(y, x));
        } else {
            out.data.sort_by(sort_cmp);
        }
    }
    one_mat(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(f: crate::builtins::BuiltinFn, args: &[Value]) -> R<Matrix> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        Ok(f(&mut it, args, 1)?[0].clone().into_mat())
    }

    fn row(v: &[f64]) -> Value {
        Value::Mat(Matrix::row(v.to_vec()))
    }

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
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
        Value::Str(s.to_string())
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
        // A matrix is still refused, whatever the options.
        assert!(call(sort, &[mat(2, 2, &[1.0, 2.0, 3.0, 4.0]), text("descend")]).is_err());
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
        let e = call(norm, &[mat(2, 2, &[1.0, 2.0, 3.0, 4.0]), num(1.0)]).unwrap_err();
        assert_eq!(e.msg, "'norm' currently supports vectors only.");
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
        assert_eq!(
            call(reshape, &[v.clone(), num(3.0), num(2.0), num(2.0)])
                .unwrap_err()
                .msg,
            "N-D arrays are not supported."
        );
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
}
