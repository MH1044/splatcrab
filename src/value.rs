//! Runtime values: a column-major double matrix (as in MATLAB) and char strings.

use std::fmt::Write as _;

use crate::bail;
use crate::error::{self, R};

#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    /// Column-major: element (r, c) lives at data[c * rows + r].
    pub data: Vec<f64>,
}

#[derive(Clone, Debug)]
pub enum Value {
    Mat(Matrix),
    Str(String),
}

impl Value {
    /// Strings become row vectors of character codes when used numerically.
    ///
    /// The empty string is the exception: `''` is `0x0` in MATLAB, not the
    /// `1x0` a row of no elements would be, so `size('')` is `0 0`. Every
    /// shape query goes through here, which is why the fix lives here and not
    /// in `size`.
    pub fn into_mat(self) -> Matrix {
        match self {
            Value::Mat(m) => m,
            Value::Str(s) if s.is_empty() => Matrix::empty(),
            Value::Str(s) => Matrix::row(s.chars().map(|c| c as u32 as f64).collect()),
        }
    }

    pub fn display(&self, name: &str) -> String {
        match self {
            Value::Str(s) => format!("{} =\n\n    '{}'\n\n", name, s),
            Value::Mat(m) => format!("{} =\n\n{}\n", name, m.format()),
        }
    }
}

fn broadcast_dim(a: usize, b: usize) -> Option<usize> {
    if a == b {
        Some(a)
    } else if a == 1 {
        Some(b)
    } else if b == 1 {
        Some(a)
    } else {
        None
    }
}

impl Matrix {
    pub fn new(rows: usize, cols: usize, data: Vec<f64>) -> Matrix {
        debug_assert_eq!(rows * cols, data.len());
        Matrix { rows, cols, data }
    }

    pub fn scalar(v: f64) -> Matrix {
        Matrix::new(1, 1, vec![v])
    }

    pub fn empty() -> Matrix {
        Matrix::new(0, 0, Vec::new())
    }

    pub fn filled(rows: usize, cols: usize, v: f64) -> Matrix {
        Matrix::new(rows, cols, vec![v; rows * cols])
    }

    pub fn row(data: Vec<f64>) -> Matrix {
        let n = data.len();
        Matrix::new(1, n, data)
    }

    pub fn col(data: Vec<f64>) -> Matrix {
        let n = data.len();
        Matrix::new(n, 1, data)
    }

    pub fn identity(rows: usize, cols: usize) -> Matrix {
        let mut m = Matrix::filled(rows, cols, 0.0);
        for i in 0..rows.min(cols) {
            m.set(i, i, 1.0);
        }
        m
    }

    pub fn numel(&self) -> usize {
        self.data.len()
    }

    pub fn is_scalar(&self) -> bool {
        self.data.len() == 1
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// MATLAB's `isvector`: 1-by-N or N-by-1, where N may be `0`. A 1x0 and a
    /// 0x1 are vectors; a 0x0 is not.
    pub fn is_vector(&self) -> bool {
        self.rows == 1 || self.cols == 1
    }

    pub fn get(&self, r: usize, c: usize) -> f64 {
        self.data[c * self.rows + r]
    }

    pub fn set(&mut self, r: usize, c: usize, v: f64) {
        self.data[c * self.rows + r] = v;
    }

    pub fn scalar_value(&self) -> Option<f64> {
        if self.is_scalar() {
            Some(self.data[0])
        } else {
            None
        }
    }

    /// MATLAB truthiness for `if` and `while`: non-empty and every element
    /// non-zero. `if []` is false, and takes no error with it.
    ///
    /// A `NaN` is neither: MATLAB and Octave both refuse to convert one, and
    /// taking it as true (which `!= 0.0` does) is silent and wrong (QA D5).
    /// That is the only way this fails, so an empty stays false rather than
    /// becoming an error.
    pub fn truth(&self) -> R<bool> {
        if self.data.iter().any(|v| v.is_nan()) {
            bail!(error::nan_to_logical());
        }
        Ok(!self.data.is_empty() && self.data.iter().all(|v| *v != 0.0))
    }

    /// The single value `&&` and `||` branch on.
    ///
    /// Stricter than [`truth`](Matrix::truth): those operators need one
    /// logical value, so an array or an empty is an error rather than "all
    /// non-zero". `[1 1] && 1` used to be `1` and `[] || 1` used to be `1`.
    pub fn logical_scalar(&self) -> R<bool> {
        match self.scalar_value() {
            Some(v) if v.is_nan() => bail!(error::nan_to_logical()),
            Some(v) => Ok(v != 0.0),
            None => bail!(error::logical_scalar_operand()),
        }
    }

    /// One element as a logical, for `&`, `|` and `~`, which convert element
    /// by element and so refuse a `NaN` the same way.
    pub fn logical_element(v: f64) -> R<bool> {
        if v.is_nan() {
            bail!(error::nan_to_logical());
        }
        Ok(v != 0.0)
    }

    pub fn map(&self, f: impl Fn(f64) -> f64) -> Matrix {
        Matrix::new(
            self.rows,
            self.cols,
            self.data.iter().map(|v| f(*v)).collect(),
        )
    }

    /// [`map`](Matrix::map) for an operation that may refuse an element, which
    /// is what `~` needs now that a `NaN` cannot become a logical. The shape
    /// comes from the operand, so there is nothing new to size-check.
    pub fn try_map(&self, f: impl Fn(f64) -> R<f64>) -> R<Matrix> {
        let mut data = Vec::with_capacity(self.numel());
        for v in &self.data {
            data.push(f(*v)?);
        }
        Ok(Matrix::new(self.rows, self.cols, data))
    }

    /// Element-wise combination with scalar / row / column broadcasting.
    pub fn zip(&self, o: &Matrix, op: &str, f: impl Fn(f64, f64) -> f64) -> R<Matrix> {
        self.try_zip(o, op, |a, b| Ok(f(a, b)))
    }

    /// [`zip`](Matrix::zip) for an operation that may refuse an element, which
    /// is what `.^` needs now that a would-be-complex result is an error
    /// rather than a `NaN`. `zip` is this function with an infallible closure.
    ///
    /// The broadcast shape goes through `args::check_shape` before a single
    /// element is allocated. `ones(1e5, 1) + ones(1, 1e5)` asks for 1e10
    /// elements from two 1e5-element operands, and used to abort in the
    /// allocator with exit 134, taking the REPL with it.
    pub fn try_zip(&self, o: &Matrix, op: &str, f: impl Fn(f64, f64) -> R<f64>) -> R<Matrix> {
        let dims_err = || error::operator_dims(op, self.rows, self.cols, o.rows, o.cols);
        let rows = broadcast_dim(self.rows, o.rows).ok_or_else(dims_err)?;
        let cols = broadcast_dim(self.cols, o.cols).ok_or_else(dims_err)?;
        crate::builtins::args::check_shape(rows as f64, cols as f64)?;
        let mut data = Vec::with_capacity(rows * cols);
        for c in 0..cols {
            for r in 0..rows {
                let a = self.get(
                    if self.rows == 1 { 0 } else { r },
                    if self.cols == 1 { 0 } else { c },
                );
                let b = o.get(
                    if o.rows == 1 { 0 } else { r },
                    if o.cols == 1 { 0 } else { c },
                );
                data.push(f(a, b)?);
            }
        }
        Ok(Matrix::new(rows, cols, data))
    }

    pub fn transpose(&self) -> Matrix {
        let mut data = Vec::with_capacity(self.numel());
        for r in 0..self.rows {
            for c in 0..self.cols {
                data.push(self.get(r, c));
            }
        }
        Matrix::new(self.cols, self.rows, data)
    }

    /// Matrix product. The result shape comes from the operands, so it goes
    /// through `args::check_shape` before `Matrix::filled` allocates: the
    /// outer product `ones(1e5, 1) * ones(1, 1e5)` used to abort in the
    /// allocator, and `zeros(2^32, 0) * zeros(0, 2^32)` used to wrap
    /// `rows * cols` to zero, report a 4294967296-square result and then panic
    /// on the next transpose.
    ///
    /// There is deliberately no `if b == 0.0 { continue }` shortcut. Skipping
    /// the multiply meant `Inf * 0` and `NaN * 0` never happened, so
    /// `[Inf 0] * [0; 1]` gave `0` where MATLAB gives `NaN`.
    pub fn matmul(&self, o: &Matrix) -> R<Matrix> {
        if self.cols != o.rows {
            bail!(error::matmul_dims(self.rows, self.cols, o.rows, o.cols));
        }
        crate::builtins::args::check_shape(self.rows as f64, o.cols as f64)?;
        let mut out = Matrix::filled(self.rows, o.cols, 0.0);
        if out.data.is_empty() {
            // Nothing to accumulate into, and `o.cols` alone can be enormous:
            // `zeros(0, 5) * zeros(5, 2^32)` is a legal 0x2^32 result, so the
            // column loop below would spin four billion times for nothing.
            return Ok(out);
        }
        for j in 0..o.cols {
            for k in 0..self.cols {
                let b = o.get(k, j);
                for i in 0..self.rows {
                    out.data[j * self.rows + i] += self.get(i, k) * b;
                }
            }
        }
        Ok(out)
    }

    /// The pivot magnitude at or below which this matrix counts as singular.
    ///
    /// It is relative to the matrix, not absolute: the old fixed `1e-14`
    /// called the diagonal `[1e-15 0; 0 1e-15]` singular although it is
    /// perfectly conditioned, and only its scale was small. `eps * n * ||A||`
    /// is the usual rule, with `||A||` the largest magnitude in `A`.
    ///
    /// `solve` and `det` both use it, which is what makes them agree on what
    /// singular means. Non-finite entries are left out of the norm, so an
    /// `Inf` in the matrix cannot make every pivot look negligible; a pivot
    /// that is itself `Inf` or `NaN` fails the `<=` test and flows through to
    /// the arithmetic, exactly as it did under the fixed threshold.
    ///
    /// The test is `<=` rather than `<` so that the all-zero matrix, whose
    /// norm and tolerance are both `0`, is still singular.
    fn singular_tol(&self) -> f64 {
        let norm = self
            .data
            .iter()
            .filter(|v| v.is_finite())
            .fold(0.0_f64, |acc, v| acc.max(v.abs()));
        f64::EPSILON * self.rows.max(1) as f64 * norm
    }

    /// Solve A * X = B for square A (Gaussian elimination with partial pivoting).
    // Elimination is written with explicit indices on purpose; cycle 08 replaces
    // solve and det with a shared LU factorisation.
    #[allow(clippy::needless_range_loop)]
    pub fn solve(&self, b: &Matrix) -> R<Matrix> {
        let n = self.rows;
        if self.rows != self.cols {
            bail!(error::nonsquare_system());
        }
        if b.rows != n {
            bail!(error::solve_dims(self.rows, self.cols, b.rows, b.cols));
        }
        let tol = self.singular_tol();
        let m = b.cols;
        let mut a: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..n).map(|j| self.get(i, j)).collect())
            .collect();
        let mut x: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..m).map(|j| b.get(i, j)).collect())
            .collect();
        for k in 0..n {
            let p = (k..n)
                .max_by(|&i, &j| {
                    a[i][k]
                        .abs()
                        .partial_cmp(&a[j][k].abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            if a[p][k].abs() <= tol {
                bail!(error::singular());
            }
            a.swap(k, p);
            x.swap(k, p);
            for i in k + 1..n {
                let f = a[i][k] / a[k][k];
                if f == 0.0 {
                    continue;
                }
                for j in k..n {
                    a[i][j] -= f * a[k][j];
                }
                for j in 0..m {
                    x[i][j] -= f * x[k][j];
                }
            }
        }
        for k in (0..n).rev() {
            for j in 0..m {
                let mut s = x[k][j];
                for i in k + 1..n {
                    s -= a[k][i] * x[i][j];
                }
                x[k][j] = s / a[k][k];
            }
        }
        let mut out = Matrix::filled(n, m, 0.0);
        for i in 0..n {
            for j in 0..m {
                out.set(i, j, x[i][j]);
            }
        }
        Ok(out)
    }

    pub fn inv(&self) -> R<Matrix> {
        if self.rows != self.cols {
            bail!(error::nonsquare_inverse());
        }
        self.solve(&Matrix::identity(self.rows, self.rows))
    }

    // Elimination is written with explicit indices on purpose; cycle 08 replaces
    // solve and det with a shared LU factorisation.
    #[allow(clippy::needless_range_loop)]
    pub fn det(&self) -> R<f64> {
        if self.rows != self.cols {
            bail!(error::nonsquare_determinant());
        }
        let n = self.rows;
        let tol = self.singular_tol();
        let mut a: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..n).map(|j| self.get(i, j)).collect())
            .collect();
        let mut det = 1.0;
        for k in 0..n {
            let p = (k..n)
                .max_by(|&i, &j| {
                    a[i][k]
                        .abs()
                        .partial_cmp(&a[j][k].abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            // The same test `solve` makes, so the two agree on what singular
            // means: `det` reports `0` where `solve` refuses to divide.
            if a[p][k].abs() <= tol {
                return Ok(0.0);
            }
            if p != k {
                a.swap(k, p);
                det = -det;
            }
            det *= a[k][k];
            for i in k + 1..n {
                let f = a[i][k] / a[k][k];
                for j in k..n {
                    a[i][j] -= f * a[k][j];
                }
            }
        }
        Ok(det)
    }

    /// One rendered cell per element, column-major like `data`, and the column
    /// width they are all padded to.
    ///
    /// A `NaN` or an `Inf` no longer forces the whole matrix onto the
    /// four-decimal path. MATLAB keeps the integer column format for
    /// `[1 2 NaN]` and prints `     1     2   NaN`; a non-finite value is not
    /// a reason to stop using integer columns, it simply has no digits of its
    /// own.
    ///
    /// Having no digits is also why it does not widen the column: the width
    /// is the widest *number* plus three, so `[NaN Inf -Inf 1]` is four
    /// six-wide columns, `   NaN   Inf  -Inf     1`, and `-Inf` fits in six
    /// without asking for a seventh. A matrix of nothing but non-finite
    /// values falls back to one digit, which is what makes `disp(NaN)` the
    /// `   NaN` MATLAB prints.
    fn cells(&self) -> (Vec<String>, usize) {
        let all_int = self
            .data
            .iter()
            .all(|v| !v.is_finite() || (v.fract() == 0.0 && v.abs() < 1e15));
        if all_int {
            let texts: Vec<String> = self
                .data
                .iter()
                .map(|v| {
                    if v.is_finite() {
                        format!("{}", *v as i64)
                    } else {
                        nonfinite(*v)
                    }
                })
                .collect();
            let digits = self
                .data
                .iter()
                .zip(&texts)
                .filter(|(v, _)| v.is_finite())
                .map(|(_, s)| s.len())
                .max()
                .unwrap_or(1);
            return (texts, (digits + 3).max(6));
        }
        let max_abs = self
            .data
            .iter()
            .filter(|v| v.is_finite())
            .map(|v| v.abs())
            .fold(0.0_f64, f64::max);
        let sci = max_abs >= 1e5 || (max_abs > 0.0 && max_abs < 1e-3);
        let texts: Vec<String> = self
            .data
            .iter()
            .map(|v| {
                if !v.is_finite() {
                    nonfinite(*v)
                } else if sci {
                    crate::interp::fmt_e(*v, 4)
                } else {
                    format!("{:.4}", v)
                }
            })
            .collect();
        (texts, if sci { 13 } else { 10 })
    }

    /// MATLAB-like display body (no name header).
    ///
    /// A matrix too wide for [`TERM_WIDTH`] is split into blocks of whole
    /// columns, each headed by the columns it holds, which is what MATLAB
    /// does; `linspace(1, 2)` used to print about 1300 characters on one line.
    pub fn format(&self) -> String {
        if self.is_empty() {
            return "     []\n".to_string();
        }
        let (texts, width) = self.cells();
        // At least one column per block, however wide a single column is:
        // wrapping every element onto its own line is still better than a
        // block with no columns in it, which would never terminate.
        let per = (TERM_WIDTH / width).max(1);
        let wrapped = self.cols > per;
        let mut out = String::new();
        let mut c0 = 0;
        while c0 < self.cols {
            let c1 = (c0 + per).min(self.cols);
            if wrapped {
                out.push_str(&column_header(c0 + 1, c1));
                // The header, then a blank line, then the block's rows.
                out.push_str("\n\n");
            }
            for r in 0..self.rows {
                for c in c0..c1 {
                    let _ = write!(out, "{:>w$}", texts[c * self.rows + r], w = width);
                }
                out.push('\n');
            }
            c0 = c1;
            if wrapped && c0 < self.cols {
                out.push('\n');
            }
        }
        out
    }
}

/// The width of the display MATLAB assumes, in characters.
///
/// It is a constant rather than a terminal query on purpose: the output of a
/// script must not depend on whether it was run at a prompt or piped into a
/// file, or a golden case would pass on one machine and fail on another.
/// MATLAB's own command window defaults to 80 and keeps using 80 when its
/// output is captured, so 80 is both the compatible answer and the
/// reproducible one.
pub const TERM_WIDTH: usize = 80;

/// `  Columns 1 through 13`, MATLAB's heading for one block of a wide matrix,
/// with its singular and two-column spellings.
fn column_header(first: usize, last: usize) -> String {
    match last - first {
        0 => format!("  Column {}", first),
        1 => format!("  Columns {} and {}", first, last),
        _ => format!("  Columns {} through {}", first, last),
    }
}

pub fn nonfinite(v: f64) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else if v > 0.0 {
        "Inf".to_string()
    } else {
        "-Inf".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close_tol(a: f64, b: f64, tol: f64) {
        assert!(
            (a - b).abs() <= tol * b.abs().max(1.0),
            "{a} vs {b} (tolerance {tol})"
        );
    }

    fn close(a: f64, b: f64) {
        close_tol(a, b, 1e-12);
    }

    /// Builds a matrix from elements given in reading (row-major) order.
    fn rmat(rows: usize, cols: usize, row_major: &[f64]) -> Matrix {
        assert_eq!(rows * cols, row_major.len());
        let mut m = Matrix::filled(rows, cols, 0.0);
        for (i, v) in row_major.iter().enumerate() {
            m.set(i / cols, i % cols, *v);
        }
        m
    }

    fn close_all(got: &Matrix, want: &Matrix) {
        assert_eq!((got.rows, got.cols), (want.rows, want.cols));
        for (g, w) in got.data.iter().zip(&want.data) {
            close(*g, *w);
        }
    }

    // ---- layout ------------------------------------------------------

    #[test]
    fn column_major_layout() {
        let m = Matrix::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // Element (r, c) lives at data[c * rows + r].
        assert_eq!(m.get(0, 0), m.data[0]);
        assert_eq!(m.get(1, 0), m.data[1]);
        assert_eq!(m.get(0, 1), m.data[2]);
        assert_eq!(m.get(1, 1), m.data[3]);
        assert_eq!(m.get(0, 2), m.data[4]);
        assert_eq!(m.get(1, 2), m.data[5]);
        // So the matrix above reads [1 3 5; 2 4 6].
        assert_eq!(m, rmat(2, 3, &[1.0, 3.0, 5.0, 2.0, 4.0, 6.0]));

        let mut z = Matrix::filled(2, 2, 0.0);
        z.set(1, 0, 7.0);
        z.set(0, 1, 9.0);
        assert_eq!(z.data, [0.0, 7.0, 9.0, 0.0]);
    }

    #[test]
    fn transpose_round_trips() {
        let m = Matrix::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let t = m.transpose();
        assert_eq!((t.rows, t.cols), (3, 2));
        assert_eq!(t.get(0, 0), m.get(0, 0));
        assert_eq!(t.get(1, 0), m.get(0, 1));
        assert_eq!(t.get(2, 0), m.get(0, 2));
        assert_eq!(t.get(0, 1), m.get(1, 0));
        assert_eq!(t.get(2, 1), m.get(1, 2));
        assert_eq!(t.transpose(), m);
        assert_eq!(Matrix::empty().transpose(), Matrix::empty());
    }

    #[test]
    fn shape_predicates() {
        assert!(Matrix::empty().is_empty());
        assert!(Matrix::scalar(1.0).is_scalar());
        assert!(Matrix::row(vec![1.0, 2.0]).is_vector());
        assert!(Matrix::col(vec![1.0, 2.0]).is_vector());
        assert!(!rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]).is_vector());
        assert!(!Matrix::empty().is_vector());
        // 1-by-N or N-by-1 with N = 0 is still a vector; 2x0 is not.
        assert!(Matrix::new(1, 0, vec![]).is_vector());
        assert!(Matrix::new(0, 1, vec![]).is_vector());
        assert!(!Matrix::new(2, 0, vec![]).is_vector());
        assert!(Matrix::scalar(1.0).is_vector());
        assert_eq!(Matrix::identity(2, 3).data, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        // MATLAB truthiness: non-empty and every element non-zero.
        assert!(Matrix::row(vec![1.0, 2.0]).truth().unwrap());
        assert!(!Matrix::row(vec![1.0, 0.0]).truth().unwrap());
        assert!(!Matrix::empty().truth().unwrap());
    }

    #[test]
    fn value_conversions() {
        let m = Value::Str("AB".to_string()).into_mat();
        assert_eq!((m.rows, m.cols), (1, 2));
        assert_eq!(m.data, [65.0, 66.0]);
        assert_eq!(
            Value::Str("hi".to_string()).display("s"),
            "s =\n\n    'hi'\n\n"
        );
        assert_eq!(
            Value::Mat(Matrix::scalar(3.0)).display("x"),
            "x =\n\n     3\n\n"
        );
    }

    // ---- zip / broadcasting ------------------------------------------

    #[test]
    fn zip_broadcasts_a_row() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let r = rmat(1, 3, &[10.0, 20.0, 30.0]);
        let s = a.zip(&r, "+", |x, y| x + y).unwrap();
        assert_eq!((s.rows, s.cols), (2, 3));
        assert_eq!(s, rmat(2, 3, &[11.0, 22.0, 33.0, 14.0, 25.0, 36.0]));
        // Broadcasting is symmetric in shape.
        let s2 = r.zip(&a, "+", |x, y| x + y).unwrap();
        assert_eq!(s2, s);
    }

    #[test]
    fn zip_broadcasts_a_column() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let c = rmat(2, 1, &[10.0, 20.0]);
        let s = a.zip(&c, "+", |x, y| x + y).unwrap();
        assert_eq!((s.rows, s.cols), (2, 3));
        assert_eq!(s, rmat(2, 3, &[11.0, 12.0, 13.0, 24.0, 25.0, 26.0]));
    }

    #[test]
    fn zip_broadcasts_scalars_on_either_side() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let s = Matrix::scalar(2.0);
        let l = a.zip(&s, "*", |x, y| x * y).unwrap();
        assert_eq!(l, rmat(2, 3, &[2.0, 4.0, 6.0, 8.0, 10.0, 12.0]));
        let r = s.zip(&a, "-", |x, y| x - y).unwrap();
        assert_eq!((r.rows, r.cols), (2, 3));
        assert_eq!(r, rmat(2, 3, &[1.0, 0.0, -1.0, -2.0, -3.0, -4.0]));
    }

    #[test]
    fn zip_column_by_row_is_an_outer_product() {
        let c = Matrix::col(vec![1.0, 2.0, 3.0]);
        let r = Matrix::row(vec![10.0, 20.0, 30.0]);
        let o = c.zip(&r, "*", |x, y| x * y).unwrap();
        assert_eq!((o.rows, o.cols), (3, 3));
        assert_eq!(
            o,
            rmat(
                3,
                3,
                &[10.0, 20.0, 30.0, 20.0, 40.0, 60.0, 30.0, 60.0, 90.0]
            )
        );
    }

    /// Acceptance test 17, the `zip` half: the broadcast result shape goes
    /// through `check_shape`, so `ones(1e5, 1) + ones(1, 1e5)` is an error
    /// rather than an allocator abort. Asserted here directly, not only
    /// through a script, because the script form used to kill the process.
    #[test]
    fn zip_checks_the_broadcast_result_size_before_allocating() {
        // 20000 squared is 4e8, past the 2^28-element cap, while the two
        // operands together are 40000 elements.
        let col = Matrix::col(vec![1.0; 20_000]);
        let row = Matrix::row(vec![1.0; 20_000]);
        let e = col.zip(&row, "+", |x, y| x + y).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 20000x20000 array exceeds the maximum array size."
        );
        // The same guard on the fallible form.
        assert!(col.try_zip(&row, "+", |x, y| Ok(x + y)).is_err());
        // A shape that fits is untouched, and `zip` is `try_zip` with an
        // infallible closure.
        let small = Matrix::col(vec![1.0, 2.0])
            .try_zip(&Matrix::row(vec![10.0, 20.0]), "*", |x, y| Ok(x * y))
            .unwrap();
        assert_eq!(small.data, [10.0, 20.0, 20.0, 40.0]);
        // A refusal from the closure comes out as the error.
        let e = Matrix::scalar(1.0)
            .try_zip(&Matrix::scalar(2.0), "+", |_, _| {
                Err(crate::error::too_many_outputs())
            })
            .unwrap_err()
            .msg;
        assert_eq!(e, "Too many output arguments.");
    }

    #[test]
    fn zip_rejects_incompatible_sizes() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = rmat(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let e = a.zip(&b, "+", |x, y| x + y).unwrap_err().msg;
        assert!(e.contains("incompatible sizes"), "{e}");
        assert!(e.contains("2x3 vs 3x2"), "{e}");
        assert!(e.contains("'+'"), "{e}");
    }

    // ---- matmul ------------------------------------------------------

    #[test]
    fn matmul_values_and_shape() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = rmat(3, 2, &[7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
        let p = a.matmul(&b).unwrap();
        assert_eq!((p.rows, p.cols), (2, 2));
        assert_eq!(p, rmat(2, 2, &[58.0, 64.0, 139.0, 154.0]));
        // The other order gives the 3x3 product.
        let q = b.matmul(&a).unwrap();
        assert_eq!((q.rows, q.cols), (3, 3));
        assert_eq!(q.get(0, 0), 7.0 * 1.0 + 8.0 * 4.0);
        // Multiplying by the identity is a no-op.
        assert_eq!(a.matmul(&Matrix::identity(3, 3)).unwrap(), a);
        assert_eq!(Matrix::identity(2, 2).matmul(&a).unwrap(), a);
    }

    /// Acceptance test 17, the `matmul` half, and acceptance test 4.
    #[test]
    fn matmul_checks_the_result_size_before_allocating() {
        // Both operands are empty, so the old code allocated nothing and
        // reached `Matrix::filled` with a wrapped `rows * cols`.
        let tall = Matrix::new(100_000, 0, Vec::new());
        let wide = Matrix::new(0, 100_000, Vec::new());
        assert_eq!(
            tall.matmul(&wide).unwrap_err().msg,
            "Requested 100000x100000 array exceeds the maximum array size."
        );
        // The size that used to wrap to zero: 2^32 squared is exactly 2^64.
        let a = Matrix::new(1 << 32, 0, Vec::new());
        let b = Matrix::new(0, 1 << 32, Vec::new());
        assert_eq!(
            a.matmul(&b).unwrap_err().msg,
            "Requested 4294967296x4294967296 array exceeds the maximum array size."
        );
        // A legal empty result is still produced, and promptly.
        let wide_ok = Matrix::new(0, 1 << 20, Vec::new());
        let empty = Matrix::new(0, 0, Vec::new()).matmul(&wide_ok).unwrap();
        assert_eq!((empty.rows, empty.cols), (0, 1 << 20));
    }

    /// The `if b == 0.0 { continue }` shortcut is gone, so a zero factor is
    /// multiplied like any other and `Inf * 0` and `NaN * 0` happen.
    #[test]
    fn matmul_keeps_inf_and_nan_through_a_zero_factor() {
        let inf = Matrix::row(vec![f64::INFINITY, 0.0]);
        let nan = Matrix::row(vec![f64::NAN, 0.0]);
        let pick = Matrix::col(vec![0.0, 1.0]);
        assert!(inf.matmul(&pick).unwrap().data[0].is_nan());
        assert!(nan.matmul(&pick).unwrap().data[0].is_nan());
        // A finite matrix with zeros is unaffected: the shortcut only ever
        // mattered for a non-finite partner.
        let a = rmat(2, 2, &[1.0, 0.0, 0.0, 2.0]);
        assert_eq!(a.matmul(&a).unwrap(), rmat(2, 2, &[1.0, 0.0, 0.0, 4.0]));
    }

    #[test]
    fn matmul_rejects_bad_dimensions() {
        let a = Matrix::row(vec![1.0, 2.0]);
        let b = Matrix::row(vec![3.0, 4.0]);
        let e = a.matmul(&b).unwrap_err().msg;
        assert!(e.contains("Incorrect dimensions"), "{e}");
        assert!(e.contains("1x2 * 1x2"), "{e}");
        // Transposing the right side makes it legal again.
        assert_eq!(a.matmul(&b.transpose()).unwrap(), Matrix::scalar(11.0));
    }

    // ---- solve -------------------------------------------------------

    #[test]
    fn solve_two_by_two() {
        let a = rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        let b = Matrix::col(vec![3.0, 5.0]);
        let x = a.solve(&b).unwrap();
        assert_eq!((x.rows, x.cols), (2, 1));
        close(x.get(0, 0), 0.8);
        close(x.get(1, 0), 1.4);
    }

    #[test]
    fn solve_multiple_right_hand_sides() {
        let a = rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        let b = rmat(2, 2, &[3.0, 1.0, 5.0, 0.0]);
        let x = a.solve(&b).unwrap();
        assert_eq!((x.rows, x.cols), (2, 2));
        close(x.get(0, 0), 0.8);
        close(x.get(1, 0), 1.4);
        close(x.get(0, 1), 0.6);
        close(x.get(1, 1), -0.2);
        // A * X reproduces B.
        close_all(&a.matmul(&x).unwrap(), &b);
    }

    #[test]
    fn solve_uses_partial_pivoting() {
        // A zero in the leading pivot position needs a row swap.
        let a = rmat(2, 2, &[0.0, 1.0, 1.0, 0.0]);
        let x = a.solve(&Matrix::col(vec![1.0, 2.0])).unwrap();
        close(x.get(0, 0), 2.0);
        close(x.get(1, 0), 1.0);

        let a3 = rmat(3, 3, &[0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 0.0]);
        let x3 = a3.solve(&Matrix::col(vec![1.0, 2.0, 3.0])).unwrap();
        close(x3.get(0, 0), 1.0);
        close(x3.get(1, 0), 1.0);
        close(x3.get(2, 0), 1.0);
    }

    #[test]
    fn solve_moderately_ill_conditioned_system() {
        // 4x4 Hilbert matrix, H(i, j) = 1 / (i + j - 1) with 1-based indices.
        let mut h = Matrix::filled(4, 4, 0.0);
        for (i, v) in h.data.iter_mut().enumerate() {
            let (r, c) = (i % 4, i / 4);
            *v = 1.0 / (r + c + 1) as f64;
        }
        let want = Matrix::filled(4, 1, 1.0);
        let b = h.matmul(&want).unwrap();
        let x = h.solve(&b).unwrap();
        assert_eq!((x.rows, x.cols), (4, 1));
        for v in &x.data {
            close_tol(*v, 1.0, 1e-8);
        }
    }

    /// The pivot threshold is relative to the matrix, so a well-conditioned
    /// system is solved whatever its scale. The old fixed `1e-14` called the
    /// first of these singular.
    #[test]
    fn solve_scales_its_pivot_tolerance_with_the_matrix() {
        let tiny = rmat(2, 2, &[1e-15, 0.0, 0.0, 1e-15]);
        let x = tiny.solve(&Matrix::col(vec![1.0, 1.0])).unwrap();
        close_tol(x.get(0, 0), 1e15, 1e-12);
        close_tol(x.get(1, 0), 1e15, 1e-12);
        // The same system scaled up and down is solved just as well.
        for scale in [1e-300, 1e-30, 1.0, 1e30, 1e150] {
            let a = rmat(2, 2, &[scale, 0.0, 0.0, scale]);
            let x = a.solve(&Matrix::col(vec![scale, 2.0 * scale])).unwrap();
            close(x.get(0, 0), 1.0);
            close(x.get(1, 0), 2.0);
        }
        // Scale alone never decides: a matrix that is singular stays singular
        // however small its entries are.
        let singular = rmat(2, 2, &[1e-15, 2e-15, 2e-15, 4e-15]);
        assert!(
            singular
                .solve(&Matrix::col(vec![1.0, 2.0]))
                .unwrap_err()
                .msg
                .contains("singular")
        );
        // An all-zero matrix has a zero norm and so a zero tolerance, which
        // is why the test is `<=` and not `<`.
        let zeros = Matrix::filled(2, 2, 0.0);
        assert!(zeros.solve(&Matrix::col(vec![1.0, 1.0])).is_err());
    }

    /// `det` and `solve` make the same test, which is what "agree on what
    /// singular means" is: `det` reports `0` exactly where `solve` refuses.
    #[test]
    fn det_and_solve_agree_on_singular() {
        let cases = [
            rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]),
            rmat(2, 2, &[1e-15, 2e-15, 2e-15, 4e-15]),
            Matrix::filled(2, 2, 0.0),
            rmat(2, 2, &[1e-15, 0.0, 0.0, 1e-15]),
            rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]),
        ];
        for a in cases {
            let rhs = Matrix::col(vec![1.0, 1.0]);
            let singular_to_det = a.det().unwrap() == 0.0;
            let singular_to_solve = a.solve(&rhs).is_err();
            assert_eq!(singular_to_det, singular_to_solve, "{:?}", a.data);
        }
        // The scaled diagonal has a real determinant now, rather than being
        // written off: 1e-15 * 1e-15.
        close_tol(
            rmat(2, 2, &[1e-15, 0.0, 0.0, 1e-15]).det().unwrap(),
            1e-30,
            1e-12,
        );
    }

    /// A non-finite entry must not drag the norm, and with it the tolerance,
    /// to infinity: every pivot would then be at or below it and every such
    /// matrix would be called singular, which the fixed threshold never did.
    #[test]
    fn a_non_finite_entry_does_not_make_everything_singular() {
        for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let a = rmat(2, 2, &[bad, 0.0, 0.0, 1.0]);
            assert!(a.singular_tol().is_finite(), "{bad}");
            assert_eq!(a.singular_tol(), f64::EPSILON * 2.0);
        }
        // And the system is still solved: the `Inf` pivot is far above the
        // tolerance, so the second row is eliminated and back-substituted as
        // it always was.
        let a = rmat(2, 2, &[f64::INFINITY, 0.0, 0.0, 1.0]);
        let x = a.solve(&Matrix::col(vec![1.0, 1.0])).unwrap();
        assert_eq!(x.get(1, 0), 1.0);
    }

    #[test]
    fn solve_rejects_singular_and_non_square() {
        let s = rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        let e = s.solve(&Matrix::col(vec![1.0, 2.0])).unwrap_err().msg;
        assert!(e.contains("singular"), "{e}");

        let ns = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let e = ns.solve(&Matrix::col(vec![1.0, 2.0])).unwrap_err().msg;
        assert!(e.contains("square"), "{e}");

        // Square, but the right-hand side has the wrong number of rows.
        let a = rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        assert!(a.solve(&Matrix::col(vec![1.0, 2.0, 3.0])).is_err());
    }

    // ---- inv / det ---------------------------------------------------

    #[test]
    fn inv_times_original_is_the_identity() {
        let a = rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0]);
        let ai = a.inv().unwrap();
        let id = Matrix::identity(3, 3);
        close_all(&ai.matmul(&a).unwrap(), &id);
        close_all(&a.matmul(&ai).unwrap(), &id);

        // A 2x2 case with an inverse that is easy to state exactly.
        let b = rmat(2, 2, &[4.0, 7.0, 2.0, 6.0]);
        close_all(&b.inv().unwrap(), &rmat(2, 2, &[0.6, -0.7, -0.2, 0.4]));

        assert!(rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).inv().is_err());
        assert!(rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]).inv().is_err());
    }

    #[test]
    fn det_known_values() {
        close(rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]).det().unwrap(), -2.0);
        assert_eq!(rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]).det().unwrap(), 0.0);
        close(Matrix::identity(1, 1).det().unwrap(), 1.0);
        close(Matrix::identity(4, 4).det().unwrap(), 1.0);
        close(
            rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0])
                .det()
                .unwrap(),
            9.0,
        );
        let e = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .det()
            .unwrap_err()
            .msg;
        assert!(e.contains("square"), "{e}");
    }

    #[test]
    fn det_sign_flips_when_rows_are_swapped() {
        let a = rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let swapped = rmat(2, 2, &[3.0, 4.0, 1.0, 2.0]);
        let da = a.det().unwrap();
        let ds = swapped.det().unwrap();
        close(ds, -da);

        let b = rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0]);
        let b_swapped = rmat(3, 3, &[3.0, 6.0, 1.0, 4.0, 7.0, 2.0, 2.0, 5.0, 3.0]);
        close(b_swapped.det().unwrap(), -b.det().unwrap());
    }

    // ---- format ------------------------------------------------------

    #[test]
    fn format_integer_path() {
        assert_eq!(
            Matrix::row(vec![1.0, 10.0, 100.0]).format(),
            "     1    10   100\n"
        );
        assert_eq!(Matrix::scalar(0.0).format(), "     0\n");
        assert_eq!(Matrix::row(vec![-1.0, 2.0]).format(), "    -1     2\n");
        assert_eq!(
            Matrix::row(vec![-123456.0, 1.0]).format(),
            "   -123456         1\n"
        );
        assert_eq!(
            rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]).format(),
            "     1     2\n     3     4\n"
        );
    }

    #[test]
    fn format_four_decimal_path() {
        assert_eq!(
            Matrix::row(vec![1.5, 2.25]).format(),
            "    1.5000    2.2500\n"
        );
        assert_eq!(Matrix::scalar(-0.5).format(), "   -0.5000\n");
        // 1e-3 itself is on the fixed side of the scientific threshold.
        assert_eq!(Matrix::scalar(0.001).format(), "    0.0010\n");
    }

    #[test]
    fn format_scientific_thresholds() {
        // max |x| >= 1e5 switches the whole matrix to scientific.
        assert_eq!(Matrix::scalar(123456.7).format(), "   1.2346e+05\n");
        assert_eq!(
            Matrix::row(vec![1e5, 0.5]).format(),
            "   1.0000e+05   5.0000e-01\n"
        );
        // ... and so does max |x| < 1e-3.
        assert_eq!(Matrix::scalar(0.00012).format(), "   1.2000e-04\n");
    }

    #[test]
    fn format_nonfinite_and_empty() {
        // Integer columns. There is no finite value to take a digit count
        // from, so the width falls back to one digit plus three.
        assert_eq!(
            Matrix::row(vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY]).format(),
            "   NaN   Inf  -Inf\n"
        );
        assert_eq!(Matrix::empty().format(), "     []\n");
        assert_eq!(Matrix::new(1, 0, Vec::new()).format(), "     []\n");
        assert_eq!(nonfinite(f64::NAN), "NaN");
        assert_eq!(nonfinite(f64::INFINITY), "Inf");
        assert_eq!(nonfinite(f64::NEG_INFINITY), "-Inf");
    }

    /// A `NaN` or an `Inf` no longer drags a row of whole numbers onto the
    /// four-decimal path: `[1 2 NaN]` used to print
    /// `    1.0000    2.0000       NaN`.
    #[test]
    fn a_non_finite_element_keeps_the_integer_columns() {
        assert_eq!(
            Matrix::row(vec![1.0, 2.0, f64::NAN]).format(),
            "     1     2   NaN\n"
        );
        assert_eq!(
            Matrix::row(vec![1.0, f64::INFINITY]).format(),
            "     1   Inf\n"
        );
        // A scalar `NaN` is the same six-wide column, not the ten-wide one.
        assert_eq!(Matrix::scalar(f64::NAN).format(), "   NaN\n");
        // A genuinely fractional neighbour still moves the whole matrix to
        // four decimals, and the non-finite element rides along in it.
        assert_eq!(
            Matrix::row(vec![1.5, f64::NAN]).format(),
            "    1.5000       NaN\n"
        );
        // `-Inf` is four characters wide but has no digits, so it does not
        // widen the column past what the numbers ask for.
        assert_eq!(
            Matrix::row(vec![1.0, f64::NEG_INFINITY]).format(),
            "     1  -Inf\n"
        );
        // A number that does need the room still gets it.
        assert_eq!(
            Matrix::row(vec![-12345.0, f64::NAN]).format(),
            "   -12345      NaN\n"
        );
    }

    /// A matrix too wide for the display is split into blocks of columns, as
    /// MATLAB does; it used to print on one unwrapped line.
    #[test]
    fn a_wide_matrix_wraps_into_column_blocks() {
        // Integer columns are 6 wide, so 13 of them fit in 80 characters.
        let x = Matrix::row((1..=20).map(f64::from).collect::<Vec<f64>>());
        let out = x.format();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "  Columns 1 through 13");
        assert_eq!(lines[1], "");
        assert!(lines[2].starts_with("     1     2"), "{:?}", lines[2]);
        assert!(lines[2].ends_with("    13"), "{:?}", lines[2]);
        assert_eq!(lines[3], "");
        assert_eq!(lines[4], "  Columns 14 through 20");
        assert_eq!(lines[5], "");
        assert!(lines[6].starts_with("    14"), "{:?}", lines[6]);
        assert_eq!(lines.len(), 7);

        // Exactly the width that fits is not wrapped at all, and so carries
        // no heading.
        let fits = Matrix::row(vec![1.0; 13]).format();
        assert_eq!(fits.lines().count(), 1);
        assert!(!fits.contains("Column"));

        // Every row of a block is printed before the next block starts.
        let two = Matrix::filled(2, 14, 7.0).format();
        let lines: Vec<&str> = two.lines().collect();
        assert_eq!(lines[0], "  Columns 1 through 13");
        assert_eq!(lines.len(), 9);
        assert_eq!(lines[5], "  Column 14");

        // MATLAB's singular and two-column spellings of the heading.
        assert_eq!(column_header(3, 3), "  Column 3");
        assert_eq!(column_header(3, 4), "  Columns 3 and 4");
        assert_eq!(column_header(3, 5), "  Columns 3 through 5");
    }

    /// A `NaN` is neither true nor false. It used to convert silently, so
    /// `if NaN` was taken as true (QA D5).
    #[test]
    fn a_nan_cannot_become_a_logical() {
        let nan = Matrix::scalar(f64::NAN);
        for e in [
            nan.truth().unwrap_err(),
            nan.logical_scalar().unwrap_err(),
            Matrix::logical_element(f64::NAN).unwrap_err(),
            // One NaN anywhere is enough, as it is for MATLAB's own `if`.
            Matrix::row(vec![1.0, f64::NAN]).truth().unwrap_err(),
        ] {
            assert_eq!(e.msg, "NaN's cannot be converted to logicals.");
        }
        // Every other value converts as it always did, `Inf` included.
        assert!(Matrix::scalar(f64::INFINITY).logical_scalar().unwrap());
        assert!(!Matrix::scalar(0.0).logical_scalar().unwrap());
        assert!(Matrix::logical_element(-2.0).unwrap());
    }

    /// `&&` and `||` need one value to branch on, so an array or an empty is
    /// an error. `truth`, which `if` uses, still accepts both.
    #[test]
    fn the_short_circuit_operators_need_a_logical_scalar() {
        let want = "Operands to the logical AND (&&) and OR (||) operators \
                    must be convertible to logical scalar values.";
        assert_eq!(
            Matrix::row(vec![1.0, 1.0])
                .logical_scalar()
                .unwrap_err()
                .msg,
            want
        );
        assert_eq!(Matrix::empty().logical_scalar().unwrap_err().msg, want);
        assert_eq!(
            Matrix::new(1, 0, Vec::new())
                .logical_scalar()
                .unwrap_err()
                .msg,
            want
        );
        // `if [1 1]` and `if []` are still legal, and still mean what they did.
        assert!(Matrix::row(vec![1.0, 1.0]).truth().unwrap());
        assert!(!Matrix::empty().truth().unwrap());
    }

    /// `''` is `0x0` in MATLAB, not the `1x0` a row of no characters would
    /// be, which is what made `size('')` report `1 0`.
    #[test]
    fn the_empty_string_is_zero_by_zero() {
        let m = Value::Str(String::new()).into_mat();
        assert_eq!((m.rows, m.cols), (0, 0));
        // A non-empty string is still the row of codes it always was.
        let ab = Value::Str("ab".to_string()).into_mat();
        assert_eq!((ab.rows, ab.cols), (1, 2));
    }
}
