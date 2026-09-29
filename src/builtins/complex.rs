//! Complex numbers (cycle 10): the scalar type [`C`] every complex kernel
//! computes with, the builtins that exist for them (`real`, `imag`, `conj`,
//! `angle`, `isreal`, `complex`, `i`, `j`), and `fft` and `ifft`.
//!
//! A complex array is a [`Matrix`] whose `im` holds the imaginary parts,
//! column-major like `data`; `im` is `None` for real storage. The flag rule
//! is MathWorks' (see `docs/modules/10-complex.md`): every operation drops an
//! imaginary part that is zero throughout, so `1i * 0` is real, and
//! `complex(a, b)` is the one way to keep one.
//!
//! **Branch cuts.** `sqrt`, `log`, `log2`, `log10` and a non-integer power
//! have their cut on the negative real axis, and `asin` and `acos` theirs on
//! the real axis outside `[-1, 1]`. A point on a cut, whatever the sign of
//! its zero imaginary part, is taken from above: `sqrt(-4)` is `2i`,
//! `log(-1)` is `pi*i`, `asin(2)` is `pi/2 - 1.3170i` and `acos(2)` is
//! `1.3170i`. SplatCrab never distinguishes `+0` from `-0` in an imaginary
//! part, which keeps every answer independent of how the zero was reached.

use std::ops::{Add, Div, Mul, Neg, Sub};

use super::args::{at_most, mat, need};
use super::{Registry, add, one_as, one_mat};
use crate::error::R;
use crate::value::{Matrix, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "i", |_, a, _| unit(a, "i"), "i - the imaginary unit, unless a variable of the name exists.");
    add(r, "j", |_, a, _| unit(a, "j"), "j - the imaginary unit, unless a variable of the name exists.");
    add(r, "real", |_, a, _| part(a, "real", true), "real(Z) - real part.");
    add(r, "imag", |_, a, _| part(a, "imag", false), "imag(Z) - imaginary part.");
    add(r, "conj", |_, a, _| conj(a), "conj(Z) - complex conjugate.");
    add(r, "angle", |_, a, _| angle(a), "angle(Z) - phase angle in radians, in [-pi, pi].");
    add(r, "isreal", |_, a, _| isreal(a), "isreal(A) - true unless A has complex storage.");
    add(r, "complex", |_, a, _| complex(a), "complex(a, b) - a + b*i with complex storage, even where b is zero.");
    add(r, "fft", |_, a, _| fourier(a, "fft", false), "fft(X) - discrete Fourier transform of each column (a row: of the row).");
    add(r, "ifft", |_, a, _| fourier(a, "ifft", true), "ifft(X) - inverse discrete Fourier transform.");
}

/// A complex scalar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const fn new(re: f64, im: f64) -> C {
        C { re, im }
    }

    pub const fn real(re: f64) -> C {
        C { re, im: 0.0 }
    }

    pub const I: C = C::new(0.0, 1.0);

    /// True when the imaginary part is zero, of either sign.
    pub fn is_real(self) -> bool {
        self.im == 0.0
    }

    pub fn conj(self) -> C {
        C::new(self.re, -self.im)
    }

    pub fn abs(self) -> f64 {
        if self.im == 0.0 {
            self.re.abs()
        } else {
            self.re.hypot(self.im)
        }
    }

    /// The phase angle in `[-pi, pi]`, with a zero imaginary part read as
    /// `+0`: `arg(-1)` is `pi`, never `-pi`. See the module's note.
    pub fn arg(self) -> f64 {
        let im = if self.im == 0.0 { 0.0 } else { self.im };
        im.atan2(self.re)
    }

    pub fn exp(self) -> C {
        if self.im == 0.0 {
            return C::real(self.re.exp());
        }
        let m = self.re.exp();
        C::new(m * self.im.cos(), m * self.im.sin())
    }

    /// The principal logarithm; `ln(-1)` is `pi*i`.
    pub fn ln(self) -> C {
        if self.im == 0.0 && (self.re >= 0.0 || self.re.is_nan()) {
            return C::real(self.re.ln());
        }
        C::new(self.abs().ln(), self.arg())
    }

    /// `log2(z)`: `f64::log2` on the real domain, so `log2(8)` is exactly
    /// `3`, and `ln(z) / ln(2)` elsewhere.
    pub fn log2(self) -> C {
        if self.im == 0.0 && (self.re >= 0.0 || self.re.is_nan()) {
            return C::real(self.re.log2());
        }
        let w = self.ln();
        C::new(w.re / std::f64::consts::LN_2, w.im / std::f64::consts::LN_2)
    }

    /// `log10(z)`, as [`log2`](C::log2) is.
    pub fn log10(self) -> C {
        if self.im == 0.0 && (self.re >= 0.0 || self.re.is_nan()) {
            return C::real(self.re.log10());
        }
        let w = self.ln();
        C::new(
            w.re / std::f64::consts::LN_10,
            w.im / std::f64::consts::LN_10,
        )
    }

    /// The principal square root, with a non-negative real part.
    pub fn sqrt(self) -> C {
        if self.im == 0.0 {
            return if self.re < 0.0 {
                C::new(0.0, (-self.re).sqrt())
            } else {
                C::real(self.re.sqrt())
            };
        }
        // t = sqrt((|re| + |z|) / 2), which never cancels.
        let t = ((self.re.abs() + self.abs()) / 2.0).sqrt();
        if self.re >= 0.0 {
            C::new(t, self.im / (2.0 * t))
        } else {
            C::new(self.im.abs() / (2.0 * t), t.copysign(self.im))
        }
    }

    /// On the imaginary axis `sin(yi)` is exactly `sinh(y) i`, so a large
    /// `y` gives `0 + Infi` rather than a `NaN` from `0 * Inf`.
    pub fn sin(self) -> C {
        if self.im == 0.0 {
            return C::real(self.re.sin());
        }
        if self.re == 0.0 {
            return C::new(self.re, self.im.sinh());
        }
        C::new(
            self.re.sin() * self.im.cosh(),
            self.re.cos() * self.im.sinh(),
        )
    }

    /// On the imaginary axis `cos(yi)` is exactly the real `cosh(y)`.
    pub fn cos(self) -> C {
        if self.im == 0.0 {
            return C::real(self.re.cos());
        }
        if self.re == 0.0 {
            return C::real(self.im.cosh());
        }
        C::new(
            self.re.cos() * self.im.cosh(),
            -(self.re.sin() * self.im.sinh()),
        )
    }

    /// `asin(z) = -i * ln(i*z + sqrt(1 - z^2))`, real on `[-1, 1]`.
    pub fn asin(self) -> C {
        if self.im == 0.0 && (self.re.abs() <= 1.0 || self.re.is_nan()) {
            return C::real(self.re.asin());
        }
        let w = (C::I * self + (C::real(1.0) - self * self).sqrt()).ln();
        C::new(w.im, -w.re)
    }

    /// `acos(z) = -i * ln(z + i * sqrt(1 - z^2))`, real on `[-1, 1]`.
    pub fn acos(self) -> C {
        if self.im == 0.0 && (self.re.abs() <= 1.0 || self.re.is_nan()) {
            return C::real(self.re.acos());
        }
        let w = (self + C::I * (C::real(1.0) - self * self).sqrt()).ln();
        C::new(w.im, -w.re)
    }

    /// `self` to an integer power by repeated squaring, so a Gaussian
    /// integer stays exact: `(1i)^2` is exactly `-1`, and so real.
    pub fn powi(self, n: i64) -> C {
        let mut e = n.unsigned_abs();
        let mut base = self;
        let mut acc = C::real(1.0);
        while e > 0 {
            if e & 1 == 1 {
                acc = acc * base;
            }
            e >>= 1;
            if e > 0 {
                base = base * base;
            }
        }
        if n < 0 { C::real(1.0) / acc } else { acc }
    }

    /// `self ^ p`, MATLAB's scalar power. Real operands give the real
    /// `powf` wherever that is real (a non-negative base, an integer or an
    /// infinite exponent, a `NaN`), so real arithmetic is unchanged; an
    /// integer exponent of a complex base is repeated squaring; anything
    /// else is `exp(p * ln(self))` on the principal branch, so
    /// `(-8)^(1/3)` is `1 + 1.7321i`.
    pub fn pow(self, p: C) -> C {
        if self.im == 0.0 && p.im == 0.0 {
            let (x, y) = (self.re, p.re);
            if x >= 0.0 || x.is_nan() || !y.is_finite() || y.fract() == 0.0 {
                return C::real(x.powf(y));
            }
        }
        if p.im == 0.0 && p.re.fract() == 0.0 && p.re.abs() <= 9.007_199_254_740_992e15 {
            return self.powi(p.re as i64);
        }
        if self.re == 0.0 && self.im == 0.0 {
            return if p.re > 0.0 {
                C::real(0.0)
            } else {
                C::new(f64::NAN, f64::NAN)
            };
        }
        (p * self.ln()).exp()
    }
}

impl Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
}

impl Neg for C {
    type Output = C;
    fn neg(self) -> C {
        C::new(-self.re, -self.im)
    }
}

/// A factor with a zero imaginary part scales the other componentwise, so a
/// real factor never manufactures a `NaN` from `0 * Inf` in a part the
/// product does not need: `2 * (Inf + 1i)` is `Inf + 2i`.
impl Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        if self.im == 0.0 {
            C::new(self.re * o.re, self.re * o.im)
        } else if o.im == 0.0 {
            C::new(self.re * o.re, self.im * o.re)
        } else {
            C::new(
                self.re * o.re - self.im * o.im,
                self.re * o.im + self.im * o.re,
            )
        }
    }
}

/// A divisor with a zero imaginary part divides componentwise, so
/// `(1 + 2i) / 0` is `Inf + Infi`; any other divisor goes through Smith's
/// scaling, which neither overflows nor underflows on the way.
impl Div for C {
    type Output = C;
    fn div(self, o: C) -> C {
        if o.im == 0.0 {
            return C::new(self.re / o.re, self.im / o.re);
        }
        if o.re.abs() >= o.im.abs() {
            let r = o.im / o.re;
            let d = o.re + r * o.im;
            C::new((self.re + r * self.im) / d, (self.im - r * self.re) / d)
        } else {
            let r = o.re / o.im;
            let d = o.im + r * o.re;
            C::new((r * self.re + self.im) / d, (r * self.im - self.re) / d)
        }
    }
}

// ---- the builtins ----------------------------------------------------

/// `i` and `j`: the imaginary unit. A variable of the name shadows it, by
/// invariant 4's order, and `clear i` brings it back.
fn unit(args: &[Value], name: &str) -> R<Vec<Value>> {
    at_most(args, 0, name)?;
    one_mat(Matrix::from_c(1, 1, vec![C::I]))
}

/// `real(Z)` and `imag(Z)`: a real array of `Z`'s shape. Of a real array,
/// `imag` is zeros.
fn part(args: &[Value], name: &str, re: bool) -> R<Vec<Value>> {
    need(args, 1, name)?;
    at_most(args, 1, name)?;
    let m = mat(args, 0, name)?;
    let data = if re {
        m.data.clone()
    } else {
        m.im.clone().unwrap_or_else(|| vec![0.0; m.numel()])
    };
    one_mat(Matrix::new(m.rows, m.cols, data))
}

fn conj(args: &[Value]) -> R<Vec<Value>> {
    need(args, 1, "conj")?;
    at_most(args, 1, "conj")?;
    one_mat(mat(args, 0, "conj")?.conj())
}

fn angle(args: &[Value]) -> R<Vec<Value>> {
    need(args, 1, "angle")?;
    at_most(args, 1, "angle")?;
    let m = mat(args, 0, "angle")?;
    let data = (0..m.numel()).map(|k| m.c(k).arg()).collect();
    one_mat(Matrix::new(m.rows, m.cols, data))
}

/// `isreal(A)`: false exactly when `A` has complex storage, so
/// `isreal(complex(1, 0))` is false and `isreal(1i * 0)` true. Every value
/// that is not a numeric array is real, as a char and a logical are.
fn isreal(args: &[Value]) -> R<Vec<Value>> {
    need(args, 1, "isreal")?;
    at_most(args, 1, "isreal")?;
    let real = match &args[0] {
        Value::Mat(m) => !m.is_complex(),
        _ => true,
    };
    one_as(Matrix::from_bool(real))
}

/// `complex(a)` and `complex(a, b)`: `a + b*i` with complex storage kept
/// even where every `b` is zero, which is what makes it the one way to
/// hold a complex zero. `a` and `b` broadcast as an operator's operands do.
/// Both must be real: `complex` is not among the builtins that take a
/// complex argument, so the registry's gate refuses one before this runs.
fn complex(args: &[Value]) -> R<Vec<Value>> {
    need(args, 1, "complex")?;
    at_most(args, 2, "complex")?;
    let a = mat(args, 0, "complex")?;
    let b = match args.get(1) {
        Some(_) => mat(args, 1, "complex")?,
        None => Matrix::filled(a.rows, a.cols, 0.0),
    };
    let re = a.zip(&b, "complex", |x, _| x)?;
    let im = a.zip(&b, "complex", |_, y| y)?;
    one_mat(Matrix::complex_parts(re.rows, re.cols, re.data, im.data))
}

/// `fft(X)` and `ifft(X)`: the transform of a row, or of each column of
/// any other array, the same shape as `X`. A char is its code units, as
/// for every numeric builtin.
fn fourier(args: &[Value], name: &str, inverse: bool) -> R<Vec<Value>> {
    need(args, 1, name)?;
    at_most(args, 1, name)?;
    let m = mat(args, 0, name)?;
    let (n, count, along_row) = if m.rows == 1 {
        (m.cols, 1, true)
    } else {
        (m.rows, m.cols, false)
    };
    // No transform to run on an empty matrix, however many columns it has
    // (cycle 13b).
    let count = if m.numel() == 0 { 0 } else { count };
    let mut re = vec![0.0; m.numel()];
    let mut im = vec![0.0; m.numel()];
    for c in 0..count {
        // A row's elements are consecutive, as a column's are.
        let at = |k: usize| if along_row { k } else { c * n + k };
        let mut x: Vec<C> = (0..n).map(|k| m.c(at(k))).collect();
        dft(&mut x, inverse);
        for (k, z) in x.into_iter().enumerate() {
            re[at(k)] = z.re;
            im[at(k)] = z.im;
        }
    }
    one_mat(Matrix::new(m.rows, m.cols, re).with_im(Some(im)))
}

// ---- the transform ---------------------------------------------------

/// The discrete Fourier transform of `x` in place,
/// `X(k) = sum_j x(j) * exp(-2*pi*i*j*k/n)`, or with `inverse` the inverse,
/// `x(j) = (1/n) * sum_k X(k) * exp(+2*pi*i*j*k/n)`.
///
/// Every length is `O(n log n)`: a power of two by the iterative radix-2
/// Cooley-Tukey transform, any other length by Bluestein's algorithm, which
/// writes the transform as a convolution with a chirp and does that
/// convolution with power-of-two transforms of at least `2n - 1` points.
pub fn dft(x: &mut [C], inverse: bool) {
    let n = x.len();
    if n <= 1 {
        return;
    }
    if n.is_power_of_two() {
        radix2(x, inverse);
    } else {
        bluestein(x, inverse);
    }
    if inverse {
        let s = 1.0 / n as f64;
        for z in x.iter_mut() {
            *z = C::new(z.re * s, z.im * s);
        }
    }
}

/// `exp(sign * 2*pi*i * k / n)` with the angle reduced exactly first, so
/// the twiddles at the quarter turns are exact.
fn twiddle(k: u128, n: u128, sign: f64) -> C {
    let k = k % n;
    let t = sign * 2.0 * std::f64::consts::PI * (k as f64 / n as f64);
    // The quarter turns exactly, so a transform of integers keeps its
    // exact zeros where it can.
    if 4 * k % n == 0 {
        return match 4 * k / n {
            0 => C::real(1.0),
            1 => C::new(0.0, sign),
            2 => C::real(-1.0),
            _ => C::new(0.0, -sign),
        };
    }
    C::new(t.cos(), t.sin())
}

/// The unscaled radix-2 transform of a power-of-two length, iterative:
/// a bit-reversal permutation, then `log2(n)` passes of butterflies.
fn radix2(x: &mut [C], inverse: bool) {
    let n = x.len();
    let sign = if inverse { 1.0 } else { -1.0 };
    let bits = n.trailing_zeros();
    for k in 0..n {
        let r = k.reverse_bits() >> (usize::BITS - bits);
        if r > k {
            x.swap(k, r);
        }
    }
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let w: Vec<C> = (0..half)
            .map(|k| twiddle(k as u128, len as u128, sign))
            .collect();
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let a = x[start + k];
                let b = x[start + k + half] * w[k];
                x[start + k] = a + b;
                x[start + k + half] = a - b;
            }
        }
        len *= 2;
    }
}

/// Bluestein's algorithm for any length `n`. Since `j*k = (j^2 + k^2 -
/// (k-j)^2) / 2`, with the chirp `w(k) = exp(sign * pi*i * k^2 / n)` the
/// transform is `X(k) = w(k) * sum_j (x(j) * w(j)) * conj(w(k - j))`, a
/// convolution, done by radix-2 transforms of a length `m >= 2n - 1`.
fn bluestein(x: &mut [C], inverse: bool) {
    let n = x.len();
    let sign = if inverse { 1.0 } else { -1.0 };
    let m = (2 * n - 1).next_power_of_two();
    // w(k) = exp(sign * 2*pi*i * k^2 / 2n), with k^2 reduced modulo 2n
    // exactly, in integers.
    let nn = 2 * n as u128;
    let chirp: Vec<C> = (0..n)
        .map(|k| twiddle((k as u128 * k as u128) % nn, nn, sign))
        .collect();
    let mut a = vec![C::real(0.0); m];
    for k in 0..n {
        a[k] = x[k] * chirp[k];
    }
    let mut b = vec![C::real(0.0); m];
    b[0] = chirp[0].conj();
    for k in 1..n {
        b[k] = chirp[k].conj();
        b[m - k] = chirp[k].conj();
    }
    radix2(&mut a, false);
    radix2(&mut b, false);
    for k in 0..m {
        a[k] = a[k] * b[k];
    }
    radix2(&mut a, true);
    let s = 1.0 / m as f64;
    for k in 0..n {
        x[k] = C::new(a[k].re * s, a[k].im * s) * chirp[k];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: C, b: C, tol: f64) -> bool {
        (a.re - b.re).abs() <= tol * b.re.abs().max(1.0)
            && (a.im - b.im).abs() <= tol * b.im.abs().max(1.0)
    }

    fn assert_close(a: C, b: C, tol: f64) {
        assert!(close(a, b, tol), "{a:?} vs {b:?} (tolerance {tol})");
    }

    #[test]
    fn arithmetic() {
        let a = C::new(3.0, 4.0);
        let b = C::new(5.0, -4.0);
        assert_eq!(a + b, C::new(8.0, 0.0));
        assert_eq!(a - b, C::new(-2.0, 8.0));
        assert_eq!(a * b, C::new(31.0, 8.0));
        assert_close(a / b, C::new(-1.0 / 41.0, 32.0 / 41.0), 1e-15);
        assert_close((a / b) * b, a, 1e-14);
        assert_eq!(a.abs(), 5.0);
        assert_eq!(a.conj(), C::new(3.0, -4.0));
        assert_eq!(-a, C::new(-3.0, -4.0));
        // Smith's scaling on both branches.
        assert_close(
            C::real(1.0) / C::new(1e-300, 1e300),
            C::new(0.0, -1e-300),
            1e-12,
        );
        assert_close(
            C::new(2.0, 1.0) / C::new(1.0, 3.0),
            C::new(0.5, -0.5),
            1e-15,
        );
        // A real divisor divides componentwise.
        let q = C::new(1.0, 2.0) / C::real(0.0);
        assert_eq!((q.re, q.im), (f64::INFINITY, f64::INFINITY));
        // A real factor scales componentwise: no NaN from 0 * Inf.
        let p = C::real(2.0) * C::new(f64::INFINITY, 1.0);
        assert_eq!(p, C::new(f64::INFINITY, 2.0));
    }

    #[test]
    fn functions_and_their_branch_cuts() {
        let pi = std::f64::consts::PI;
        assert_eq!(C::real(-4.0).sqrt(), C::new(0.0, 2.0));
        assert_eq!(C::new(-4.0, -0.0).sqrt(), C::new(0.0, 2.0));
        assert_close(C::new(3.0, 4.0).sqrt(), C::new(2.0, 1.0), 1e-15);
        assert_close(C::new(-3.0, -4.0).sqrt(), C::new(1.0, -2.0), 1e-15);
        assert_close(C::real(-1.0).ln(), C::new(0.0, pi), 1e-15);
        assert_close(C::new(-1.0, -0.0).ln(), C::new(0.0, pi), 1e-15);
        assert_close(C::new(0.0, 1.0).ln(), C::new(0.0, pi / 2.0), 1e-15);
        assert_eq!(C::real(-1.0).arg(), pi);
        assert_eq!(C::new(-1.0, -0.0).arg(), pi);
        assert_close(C::new(0.0, pi).exp(), C::real(-1.0), 1e-15);
        assert_close(C::new(1.0, 2.0).exp().ln(), C::new(1.0, 2.0), 1e-15);
        // sin and cos against exp.
        let z = C::new(0.7, -1.3);
        let e = (C::I * z).exp();
        let f = (-(C::I * z)).exp();
        assert_close(z.sin(), (e - f) / C::new(0.0, 2.0), 1e-14);
        assert_close(z.cos(), (e + f) / C::real(2.0), 1e-14);
        // On the imaginary axis no NaN comes from 0 * Inf once sinh and
        // cosh overflow.
        assert_eq!(C::new(0.0, 1000.0).sin(), C::new(0.0, f64::INFINITY));
        assert_eq!(C::new(0.0, -1000.0).cos(), C::real(f64::INFINITY));
        assert_close(C::new(0.0, 1.5).sin(), C::new(0.0, 1.5f64.sinh()), 1e-15);
        // asin and acos off the interval, and their round trips.
        let acosh2 = (2.0f64 + 3.0f64.sqrt()).ln();
        assert_close(C::real(2.0).asin(), C::new(pi / 2.0, -acosh2), 1e-14);
        assert_close(C::real(2.0).acos(), C::new(0.0, acosh2), 1e-14);
        assert_close(C::real(-2.0).acos(), C::new(pi, -acosh2), 1e-14);
        for x in [2.0, -2.0, 5.0, -7.5] {
            assert_close(C::real(x).asin().sin(), C::real(x), 1e-13);
            assert_close(C::real(x).acos().cos(), C::real(x), 1e-13);
        }
        let w = C::new(0.3, 0.8);
        assert_close(w.asin().sin(), w, 1e-14);
        assert_close(w.acos().cos(), w, 1e-14);
        // On the interval they stay real.
        assert_eq!(C::real(0.5).asin(), C::real(0.5f64.asin()));
    }

    #[test]
    fn powers() {
        // A negative base to a fractional power: the principal branch.
        let z = C::real(-8.0).pow(C::real(1.0 / 3.0));
        assert_close(z, C::new(1.0, 3.0f64.sqrt()), 1e-14);
        assert_close(C::real(-4.0).pow(C::real(0.5)), C::new(0.0, 2.0), 1e-15);
        // Real powers that are real stay exactly the real powf.
        assert_eq!(C::real(-2.0).pow(C::real(3.0)), C::real(-8.0));
        assert_eq!(C::real(2.0).pow(C::real(0.5)), C::real(2f64.sqrt()));
        assert_eq!(
            C::real(-2.0).pow(C::real(f64::INFINITY)),
            C::real(f64::INFINITY)
        );
        // An integer power of a complex base is exact where it can be.
        assert_eq!(C::I.pow(C::real(2.0)), C::new(-1.0, 0.0));
        assert_eq!(C::new(1.0, 2.0).pow(C::real(2.0)), C::new(-3.0, 4.0));
        assert_close(
            C::new(1.0, 1.0).pow(C::real(-2.0)),
            C::new(0.0, -0.5),
            1e-15,
        );
        // A complex exponent.
        assert_close(
            C::real(2.0).pow(C::new(3.0, pi_over_ln2())),
            C::real(-8.0),
            1e-13,
        );
        assert_close(
            C::I.pow(C::I),
            C::real((-std::f64::consts::PI / 2.0).exp()),
            1e-14,
        );
        assert_eq!(C::real(0.0).pow(C::new(1.0, 1.0)), C::real(0.0));
    }

    fn pi_over_ln2() -> f64 {
        std::f64::consts::PI / std::f64::consts::LN_2
    }

    /// The transform by its definition, `O(n^2)`, for comparison.
    fn direct(x: &[C], inverse: bool) -> Vec<C> {
        let n = x.len();
        let sign = if inverse { 1.0 } else { -1.0 };
        (0..n)
            .map(|k| {
                let mut s = C::real(0.0);
                for (j, &v) in x.iter().enumerate() {
                    let t = sign * 2.0 * std::f64::consts::PI * ((j * k) % n) as f64 / n as f64;
                    s = s + v * C::new(t.cos(), t.sin());
                }
                if inverse {
                    C::new(s.re / n as f64, s.im / n as f64)
                } else {
                    s
                }
            })
            .collect()
    }

    #[test]
    fn fft_matches_the_direct_transform_for_every_length() {
        for n in 1..=40usize {
            let x: Vec<C> = (0..n)
                .map(|k| C::new((k as f64 * 0.37).sin() + 1.0, (k as f64 * 1.3).cos()))
                .collect();
            for inverse in [false, true] {
                let want = direct(&x, inverse);
                let mut got = x.clone();
                dft(&mut got, inverse);
                for (g, w) in got.iter().zip(&want) {
                    assert_close(*g, *w, 1e-11 * n as f64);
                }
            }
            // The round trip.
            let mut y = x.clone();
            dft(&mut y, false);
            dft(&mut y, true);
            for (g, w) in y.iter().zip(&x) {
                assert_close(*g, *w, 1e-12 * n as f64);
            }
        }
    }

    #[test]
    fn fft_of_an_impulse_and_of_a_ramp() {
        let mut x = vec![C::real(1.0), C::real(0.0), C::real(0.0), C::real(0.0)];
        dft(&mut x, false);
        assert!(x.iter().all(|z| *z == C::real(1.0)));
        let mut y: Vec<C> = [1.0, 2.0, 3.0, 4.0].iter().map(|&v| C::real(v)).collect();
        dft(&mut y, false);
        assert_eq!(
            y,
            [
                C::real(10.0),
                C::new(-2.0, 2.0),
                C::real(-2.0),
                C::new(-2.0, -2.0)
            ]
        );
    }

    /// A long prime length goes through Bluestein in `O(n log n)`; the
    /// direct transform would be 10^10 operations and never finish here.
    #[test]
    fn a_long_prime_length_is_fast() {
        let n = 100_003;
        let mut x: Vec<C> = (0..n).map(|k| C::real((k % 7) as f64)).collect();
        let sum: f64 = x.iter().map(|z| z.re).sum();
        dft(&mut x, false);
        assert!((x[0].re - sum).abs() <= 1e-9 * sum);
        assert!(x[0].im.abs() <= 1e-6);
        dft(&mut x, true);
        assert!((x[5].re - 5.0).abs() < 1e-8);
    }

    #[test]
    fn the_builtins_follow_the_flag_rule() {
        let v = |m: Matrix| Value::Mat(m);
        let z = Matrix::complex_parts(1, 1, vec![1.0], vec![0.0]);
        let out = isreal(&[v(z.clone())]).unwrap()[0]
            .clone()
            .into_mat()
            .unwrap();
        assert_eq!(out.data, [0.0]);
        let r = part(&[v(z)], "imag", false).unwrap()[0]
            .clone()
            .into_mat()
            .unwrap();
        assert!(!r.is_complex());
        assert_eq!(r.data, [0.0]);
        let c = complex(&[v(Matrix::scalar(1.0)), v(Matrix::row(vec![0.0, 2.0]))]).unwrap()[0]
            .clone()
            .into_mat()
            .unwrap();
        assert_eq!((c.rows, c.cols), (1, 2));
        assert_eq!(c.im.as_deref(), Some(&[0.0, 2.0][..]));
    }
}
