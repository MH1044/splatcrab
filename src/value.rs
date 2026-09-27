//! Runtime values: a column-major double matrix (as in MATLAB) and char strings.

use std::fmt::Write as _;

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
    pub fn into_mat(self) -> Matrix {
        match self {
            Value::Mat(m) => m,
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

    pub fn is_vector(&self) -> bool {
        (self.rows == 1 || self.cols == 1) && !self.is_empty()
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

    /// MATLAB truthiness: non-empty and every element non-zero.
    pub fn is_true(&self) -> bool {
        !self.data.is_empty() && self.data.iter().all(|v| *v != 0.0)
    }

    pub fn map(&self, f: impl Fn(f64) -> f64) -> Matrix {
        Matrix::new(self.rows, self.cols, self.data.iter().map(|v| f(*v)).collect())
    }

    /// Element-wise combination with scalar / row / column broadcasting.
    pub fn zip(&self, o: &Matrix, op: &str, f: impl Fn(f64, f64) -> f64) -> Result<Matrix, String> {
        let dims_err = || {
            format!(
                "Arrays have incompatible sizes for operator '{}' ({}x{} vs {}x{}).",
                op, self.rows, self.cols, o.rows, o.cols
            )
        };
        let rows = broadcast_dim(self.rows, o.rows).ok_or_else(dims_err)?;
        let cols = broadcast_dim(self.cols, o.cols).ok_or_else(dims_err)?;
        let mut data = Vec::with_capacity(rows * cols);
        for c in 0..cols {
            for r in 0..rows {
                let a = self.get(
                    if self.rows == 1 { 0 } else { r },
                    if self.cols == 1 { 0 } else { c },
                );
                let b = o.get(if o.rows == 1 { 0 } else { r }, if o.cols == 1 { 0 } else { c });
                data.push(f(a, b));
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

    pub fn matmul(&self, o: &Matrix) -> Result<Matrix, String> {
        if self.cols != o.rows {
            return Err(format!(
                "Incorrect dimensions for matrix multiplication ({}x{} * {}x{}). \
                 Use '.*' for element-wise multiplication.",
                self.rows, self.cols, o.rows, o.cols
            ));
        }
        let mut out = Matrix::filled(self.rows, o.cols, 0.0);
        for j in 0..o.cols {
            for k in 0..self.cols {
                let b = o.get(k, j);
                if b == 0.0 {
                    continue;
                }
                for i in 0..self.rows {
                    out.data[j * self.rows + i] += self.get(i, k) * b;
                }
            }
        }
        Ok(out)
    }

    /// Solve A * X = B for square A (Gaussian elimination with partial pivoting).
    pub fn solve(&self, b: &Matrix) -> Result<Matrix, String> {
        let n = self.rows;
        if self.rows != self.cols {
            return Err(
                "Only square systems are supported by '\\' and '/' for now (no least squares yet)."
                    .to_string(),
            );
        }
        if b.rows != n {
            return Err(format!(
                "Matrix dimensions must agree for '\\' ({}x{} \\ {}x{}).",
                self.rows, self.cols, b.rows, b.cols
            ));
        }
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
            if a[p][k].abs() < 1e-14 {
                return Err("Matrix is singular to working precision.".to_string());
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

    pub fn inv(&self) -> Result<Matrix, String> {
        if self.rows != self.cols {
            return Err("Matrix must be square to invert.".to_string());
        }
        self.solve(&Matrix::identity(self.rows, self.rows))
    }

    pub fn det(&self) -> Result<f64, String> {
        if self.rows != self.cols {
            return Err("Matrix must be square to compute a determinant.".to_string());
        }
        let n = self.rows;
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
            if a[p][k] == 0.0 {
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

    /// MATLAB-like display body (no name header).
    pub fn format(&self) -> String {
        if self.is_empty() {
            return "     []\n".to_string();
        }
        let mut out = String::new();
        let all_int = self
            .data
            .iter()
            .all(|v| v.fract() == 0.0 && v.abs() < 1e15);
        if all_int {
            let width = self
                .data
                .iter()
                .map(|v| format!("{}", *v as i64).len())
                .max()
                .unwrap_or(1);
            let width = (width + 3).max(6);
            for r in 0..self.rows {
                for c in 0..self.cols {
                    let _ = write!(out, "{:>w$}", self.get(r, c) as i64, w = width);
                }
                out.push('\n');
            }
        } else {
            let max_abs = self
                .data
                .iter()
                .filter(|v| v.is_finite())
                .map(|v| v.abs())
                .fold(0.0_f64, f64::max);
            let sci = max_abs >= 1e5 || (max_abs > 0.0 && max_abs < 1e-3);
            for r in 0..self.rows {
                for c in 0..self.cols {
                    let v = self.get(r, c);
                    let cell = if !v.is_finite() {
                        nonfinite(v)
                    } else if sci {
                        crate::interp::fmt_e(v, 4)
                    } else {
                        format!("{:.4}", v)
                    };
                    let _ = write!(out, "{:>w$}", cell, w = if sci { 13 } else { 10 });
                }
                out.push('\n');
            }
        }
        out
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
