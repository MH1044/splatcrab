//! Linear algebra, array rearrangement, and search and sort.

use std::cmp::Ordering;

use super::args::{at_most, check_size, mat, need, size_arg};
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
    add(r, "diag", diag, "diag(v) makes a diagonal matrix; diag(A) takes the diagonal.");
    add(r, "norm", norm, "norm(v) - the Euclidean length of a vector.");
    add(r, "dot", dot, "dot(a,b) - the scalar product of two equal-length vectors.");

    // ---- rearrangement -----------------------------------------------
    add(r, "reshape", reshape, "reshape(A,r,c) - the elements of A in a new shape.");
    add(r, "repmat", repmat, "repmat(A,r,c) - tile A into an r-by-c block matrix.");
    add(r, "fliplr", fliplr, "fliplr(A) - reverse the order of the columns.");
    add(r, "flipud", flipud, "flipud(A) - reverse the order of the rows.");

    // ---- search and sort ---------------------------------------------
    add(r, "find", find, "find(A) - the indices of the non-zero elements.");
    add(r, "sort", sort, "sort(v) - a vector in ascending order, NaN last.");
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

fn diag(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "diag")?;
    let m = mat(a, 0, "diag")?;
    if m.is_vector() {
        let n = m.numel();
        check_size(n, n)?;
        let mut out = Matrix::filled(n, n, 0.0);
        for (i, v) in m.data.iter().enumerate() {
            out.set(i, i, *v);
        }
        one_mat(out)
    } else {
        one_mat(Matrix::col(
            (0..m.rows.min(m.cols)).map(|i| m.get(i, i)).collect(),
        ))
    }
}

fn norm(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "norm")?;
    let m = mat(a, 0, "norm")?;
    if !m.is_vector() && !m.is_empty() {
        return Err(error::norm_vectors_only());
    }
    one_mat(Matrix::scalar(
        m.data.iter().map(|v| v * v).sum::<f64>().sqrt(),
    ))
}

fn dot(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 2, "dot")?;
    need(a, 2, "dot")?;
    let x = mat(a, 0, "dot")?;
    let y = mat(a, 1, "dot")?;
    if x.numel() != y.numel() {
        return Err(error::dot_length_mismatch());
    }
    one_mat(Matrix::scalar(
        x.data.iter().zip(&y.data).map(|(p, q)| p * q).sum(),
    ))
}

// ---- rearrangement ---------------------------------------------------

fn reshape(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "reshape")?;
    need(a, 3, "reshape")?;
    let m = mat(a, 0, "reshape")?;
    let r = size_arg(a, 1, "reshape")?;
    let c = size_arg(a, 2, "reshape")?;
    if check_size(r, c)? != m.numel() {
        return Err(error::reshape_numel(m.numel(), r, c));
    }
    one_mat(Matrix::new(r, c, m.data))
}

fn repmat(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 3, "repmat")?;
    need(a, 2, "repmat")?;
    let m = mat(a, 0, "repmat")?;
    let r = size_arg(a, 1, "repmat")?;
    let c = if a.len() >= 3 {
        size_arg(a, 2, "repmat")?
    } else {
        r
    };
    // Saturate the two factors and let `check_size` judge the shape that was
    // actually asked for. Checking `m.rows * r` on its own reported
    // "Requested 1x10000000000" for `repmat([1 2], 1e10, 1e10)`, naming an
    // intermediate instead of the 10000000000x20000000000 array requested.
    let rows = m.rows.saturating_mul(r);
    let cols = m.cols.saturating_mul(c);
    check_size(rows, cols)?;
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

fn find(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "find")?;
    let m = mat(a, 0, "find")?;
    let idx: Vec<f64> = m
        .data
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != 0.0)
        .map(|(i, _)| (i + 1) as f64)
        .collect();
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

fn sort(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(a, 1, "sort")?;
    let m = mat(a, 0, "sort")?;
    if !m.is_vector() && !m.is_empty() {
        return Err(error::sort_vectors_only());
    }
    let mut out = m.clone();
    out.data.sort_by(sort_cmp);
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
        assert!(call(sort, &[row(&[2.0, 1.0]), num(1.0)]).is_err());
    }
}
