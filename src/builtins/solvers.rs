//! The solvers (cycle 09): `fzero`, `fminsearch`, `integral` and `ode45`,
//! with `odeset` for `ode45`'s options.
//!
//! Each builtin is a thin shell around a pure method here that takes the
//! user's function as a Rust closure, so the unit tests can drive the
//! numerics, and reach every cap, with no interpreter. The shell's closure
//! calls the user's function through `Interp::call_nested` (cycle 05's rule
//! for a builtin that calls back), so every evaluation counts against the
//! nesting budget every frame shares, and a solver calling a solver stays
//! bounded.
//!
//! Every method has a cap that ends in a clean error, never a hang
//! (invariant 6): `fzero`'s search for a sign change and its iterations,
//! `fminsearch`'s iterations and evaluations, `integral`'s subintervals, and
//! `ode45`'s steps and its minimum step. A solver that fails is an error,
//! never a `NaN` handed back as an answer, which is where SplatCrab departs
//! from MATLAB's warnings and exit flags.

use std::f64::consts::SQRT_2;
use std::rc::Rc;

use super::args::{at_most, check_shape, mat, need, option, scalar};
use super::core::eps_at;
use super::math::sum0;
use super::{Registry, add, one, one_mat};
use crate::error;
use crate::interp::{Callee, Interp, R, fmt_g};
use crate::value::{Class, Func, Matrix, StructArray, Value, blank};

const EPS: f64 = f64::EPSILON;

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "fzero", fzero, "x = fzero(f,x0), fzero(f,[a b]), [x,fval,exitflag] = fzero(...) - a zero of a scalar function: a sign change, then Brent's method.");
    add(r, "fminsearch", fminsearch, "x = fminsearch(f,x0), [x,fval,exitflag] = fminsearch(...) - a local minimum by the Nelder-Mead simplex method.");
    add(r, "integral", integral, "integral(f,a,b), integral(f,a,b,'AbsTol',t,'RelTol',t) - adaptive Gauss-Kronrod quadrature; a and b may be infinite.");
    add(r, "ode45", ode45, "[t,y] = ode45(f,tspan,y0), ode45(f,tspan,y0,opts) - a nonstiff ODE by the Dormand-Prince 5(4) pair.");
    add(r, "odeset", odeset, "opts = odeset('Name',value,...) - ode45 options: AbsTol, InitialStep, MaxStep, Refine, RelTol.");
}

// ---- calling back ----------------------------------------------------

/// The function a solver calls: a handle, or a function named by a char,
/// as `feval` takes either.
enum Target {
    Handle(Rc<Func>),
    Name(String),
}

fn target(a: &[Value], name: &str) -> R<Target> {
    match a.first() {
        Some(Value::Func(f)) => Ok(Target::Handle(f.clone())),
        Some(v) if v.is_char() => Ok(Target::Name(v.text().unwrap_or_default())),
        Some(_) => Err(error::arg_not_a_handle(1, name)),
        None => Err(error::not_enough_args(name)),
    }
}

/// One call of the user's function, through `call_nested`, for its first
/// output.
fn call(it: &mut Interp, t: &Target, args: Vec<Value>) -> R<Value> {
    let callee = match t {
        Target::Handle(f) => Callee::Handle(f),
        Target::Name(n) => Callee::Name(n),
    };
    let mut out = it.call_nested(callee, args, 1)?;
    if out.is_empty() {
        return Err(error::too_many_outputs());
    }
    Ok(out.swap_remove(0))
}

fn num(v: f64) -> Value {
    Value::Mat(Matrix::scalar(v))
}

/// A returned value that must be one real number.
fn real_scalar(v: Value, name: &str) -> R<f64> {
    match v {
        Value::Mat(m) if m.is_scalar() && m.class != Class::Char => Ok(m.data[0]),
        _ => Err(error::solver_not_scalar(name)),
    }
}

/// The largest magnitude among `xs`, `0` for none; a `NaN` anywhere is
/// `NaN`.
fn norm_inf(xs: impl Iterator<Item = f64>) -> f64 {
    let mut m: f64 = 0.0;
    for x in xs {
        if x.is_nan() {
            return f64::NAN;
        }
        m = m.max(x.abs());
    }
    m
}

// ---- fzero -----------------------------------------------------------

/// Steps `fzero`'s search for a sign change may take. The step grows by
/// `sqrt(2)` each time, so the search leaves the doubles in about 2,050
/// steps from a start near 1 and about 4,000 from one near `1e-300`; the
/// cap is met only by a start so small that its step underflows to `0`.
pub const FZERO_SEARCH: usize = 4096;
/// Iterations of Brent's method. Each at least halves the bracket every
/// few steps, so a double interval closes in a few hundred at most.
pub const FZERO_ITERATIONS: usize = 1000;

/// What the search for a sign change found.
enum Bracket {
    Root(f64, f64),
    Interval(f64, f64, f64, f64),
}

/// A zero of `f` from `start`: one point, around which an interval with a
/// sign change is searched for as MATLAB's `fzero` searches (steps of
/// `x0/50`, or `1/50` from `0`, growing by `sqrt(2)`, alternately below and
/// above), or two points that already bracket one. Then Brent's method, to
/// the tolerance `2 eps |x| + eps/2`. Returns the zero and `f` there.
pub fn fzero_solve(
    f: &mut dyn FnMut(f64) -> R<f64>,
    start: &[f64],
    search_cap: usize,
    iter_cap: usize,
) -> R<(f64, f64)> {
    let found = if let [a, b] = *start {
        let (fa, fb) = (f(a)?, f(b)?);
        if fa == 0.0 {
            Bracket::Root(a, fa)
        } else if fb == 0.0 {
            Bracket::Root(b, fb)
        } else if !fa.is_finite() || !fb.is_finite() {
            return Err(error::solver_nonfinite("fzero"));
        } else if (fa > 0.0) == (fb > 0.0) {
            return Err(error::fzero_endpoints());
        } else {
            Bracket::Interval(a, fa, b, fb)
        }
    } else {
        let x = start[0];
        let fx = f(x)?;
        if fx == 0.0 {
            Bracket::Root(x, fx)
        } else if !fx.is_finite() {
            return Err(error::solver_nonfinite("fzero"));
        } else {
            search(f, x, fx, search_cap)?
        }
    };
    match found {
        Bracket::Root(x, fx) => Ok((x, fx)),
        Bracket::Interval(a, fa, b, fb) => brent(f, a, fa, b, fb, iter_cap),
    }
}

/// The search for a sign change around `x`. It ends without one when a
/// point or a value stops being finite, which a growing step always
/// reaches, or at `cap` steps.
fn search(f: &mut dyn FnMut(f64) -> R<f64>, x: f64, fx: f64, cap: usize) -> R<Bracket> {
    let mut dx = if x != 0.0 { x.abs() / 50.0 } else { 1.0 / 50.0 };
    let (mut b, mut fb) = (x, fx);
    for _ in 0..cap {
        dx *= SQRT_2;
        let a = x - dx;
        let fa = if a.is_finite() { f(a)? } else { f64::NAN };
        if fa == 0.0 {
            return Ok(Bracket::Root(a, fa));
        }
        if !fa.is_finite() {
            break;
        }
        if (fa > 0.0) != (fb > 0.0) {
            return Ok(Bracket::Interval(a, fa, b, fb));
        }
        b = x + dx;
        fb = if b.is_finite() { f(b)? } else { f64::NAN };
        if fb == 0.0 {
            return Ok(Bracket::Root(b, fb));
        }
        if !fb.is_finite() {
            break;
        }
        if (fa > 0.0) != (fb > 0.0) {
            return Ok(Bracket::Interval(a, fa, b, fb));
        }
    }
    Err(error::fzero_no_sign_change())
}

/// Brent's method (the `zeroin` of Brent's 1973 book, which MATLAB's
/// `fzero` also follows) on an interval whose ends have values of opposite
/// signs: inverse quadratic or secant steps where they make progress and
/// bisection where they do not.
fn brent(
    f: &mut dyn FnMut(f64) -> R<f64>,
    mut a: f64,
    mut fa: f64,
    mut b: f64,
    mut fb: f64,
    cap: usize,
) -> R<(f64, f64)> {
    let tolx = EPS;
    let (mut c, mut fc) = (a, fa);
    let mut d = b - a;
    let mut e = d;
    for _ in 0..cap {
        if (fb > 0.0) == (fc > 0.0) {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let tol = 2.0 * EPS * b.abs() + 0.5 * tolx;
        let m = 0.5 * (c - b);
        if m.abs() <= tol || fb == 0.0 {
            return Ok((b, fb));
        }
        if e.abs() < tol || fa.abs() <= fb.abs() {
            d = m;
            e = m;
        } else {
            let s = fb / fa;
            let (mut p, mut q);
            if a == c {
                p = 2.0 * m * s;
                q = 1.0 - s;
            } else {
                let q0 = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * m * q0 * (q0 - r) - (b - a) * (r - 1.0));
                q = (q0 - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }
            if 2.0 * p < 3.0 * m * q - (tol * q).abs() && p < (0.5 * e * q).abs() {
                e = d;
                d = p / q;
            } else {
                d = m;
                e = m;
            }
        }
        a = b;
        fa = fb;
        b += if d.abs() > tol {
            d
        } else if m > 0.0 {
            tol
        } else {
            -tol
        };
        fb = f(b)?;
        if fb.is_nan() {
            return Err(error::solver_nonfinite("fzero"));
        }
    }
    Err(error::no_convergence("fzero"))
}

/// `fzero(f, x0)` and `fzero(f, [a b])`; `[x, fval, exitflag]`, the flag
/// always `1`, since every failure is an error. An options argument is not
/// provided.
fn fzero(it: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 2, "fzero")?;
    need(a, 2, "fzero")?;
    let t = target(a, "fzero")?;
    let x0 = mat(a, 1, "fzero")?;
    if x0.numel() != 1 && x0.numel() != 2 {
        return Err(error::fzero_start());
    }
    let mut f = |x: f64| real_scalar(call(it, &t, vec![num(x)])?, "fzero");
    let (x, fx) = fzero_solve(&mut f, &x0.data, FZERO_SEARCH, FZERO_ITERATIONS)?;
    Ok([x, fx, 1.0]
        .into_iter()
        .take(nargout.max(1))
        .map(num)
        .collect())
}

// ---- fminsearch ------------------------------------------------------

/// `fminsearch`'s tolerances on the simplex's size and on its values,
/// MATLAB's defaults.
pub const NM_TOL: f64 = 1e-4;

/// The Nelder-Mead simplex method as MATLAB's `fminsearch` runs it
/// (Lagarias et al. 1998): the first simplex is `x0` and `x0` with each
/// coordinate in turn 5% larger (`0.00025` where it is `0`); reflection 1,
/// expansion 2, contraction and shrink 1/2; it stops when every vertex is
/// within `tolx` of the best and every value within `tolf` of its value.
/// Past `max_iter` iterations or `max_evals` evaluations it is an error.
/// Returns the best vertex and its value.
pub fn nelder_mead(
    f: &mut dyn FnMut(&[f64]) -> R<f64>,
    x0: &[f64],
    max_iter: usize,
    max_evals: usize,
    tolx: f64,
    tolf: f64,
) -> R<(Vec<f64>, f64)> {
    let n = x0.len();
    // The simplex is (n + 1) x n values, quadratic in the operand: judge it
    // before building it or calling `f`, so `zeros(1, 1e5)` is the size
    // error and not an 80 GB allocation (cycle 09's review).
    check_shape((n + 1) as f64, n as f64)?;
    let mut v: Vec<Vec<f64>> = vec![x0.to_vec()];
    let mut fv = vec![f(x0)?];
    for j in 0..n {
        let mut y = x0.to_vec();
        y[j] = if y[j] != 0.0 { 1.05 * y[j] } else { 0.00025 };
        fv.push(f(&y)?);
        v.push(y);
    }
    if n == 0 {
        return Ok((Vec::new(), fv[0]));
    }
    order(&mut v, &mut fv);
    let (mut evals, mut iters) = (n + 1, 1);
    let comb = |a: f64, x: &[f64], b: f64, y: &[f64]| -> Vec<f64> {
        x.iter().zip(y).map(|(p, q)| a * p + b * q).collect()
    };
    loop {
        let spread_f = fv[1..]
            .iter()
            .fold(0.0_f64, |m, x| m.max((fv[0] - x).abs()));
        let spread_x = v[1..].iter().fold(0.0_f64, |m, w| {
            w.iter()
                .zip(&v[0])
                .fold(m, |m, (p, q)| m.max((p - q).abs()))
        });
        let big = v[0].iter().fold(0.0_f64, |m, x| m.max(*x));
        if spread_f <= tolf.max(10.0 * eps_at(fv[0])) && spread_x <= tolx.max(10.0 * eps_at(big)) {
            return Ok((v[0].clone(), fv[0]));
        }
        if evals >= max_evals || iters >= max_iter {
            return Err(error::no_convergence("fminsearch"));
        }
        let mut xbar = vec![0.0; n];
        for w in &v[..n] {
            for (s, x) in xbar.iter_mut().zip(w) {
                *s += x / n as f64;
            }
        }
        let worst = v[n].clone();
        let xr = comb(2.0, &xbar, -1.0, &worst);
        let fxr = f(&xr)?;
        evals += 1;
        if fxr < fv[0] {
            let xe = comb(3.0, &xbar, -2.0, &worst);
            let fxe = f(&xe)?;
            evals += 1;
            if fxe < fxr {
                (v[n], fv[n]) = (xe, fxe);
            } else {
                (v[n], fv[n]) = (xr, fxr);
            }
        } else if fxr < fv[n - 1] {
            (v[n], fv[n]) = (xr, fxr);
        } else {
            let (xc, fxc, ok) = if fxr < fv[n] {
                let xc = comb(1.5, &xbar, -0.5, &worst);
                let fxc = f(&xc)?;
                (xc, fxc, fxc <= fxr)
            } else {
                let xcc = comb(0.5, &xbar, 0.5, &worst);
                let fxcc = f(&xcc)?;
                (xcc, fxcc, fxcc < fv[n])
            };
            evals += 1;
            if ok {
                (v[n], fv[n]) = (xc, fxc);
            } else {
                for j in 1..=n {
                    v[j] = comb(0.5, &v[0].clone(), 0.5, &v[j]);
                    fv[j] = f(&v[j])?;
                }
                evals += n;
            }
        }
        order(&mut v, &mut fv);
        iters += 1;
    }
}

/// The simplex sorted by value, stably, `NaN` last.
fn order(v: &mut Vec<Vec<f64>>, fv: &mut Vec<f64>) {
    let mut perm: Vec<usize> = (0..fv.len()).collect();
    perm.sort_by(|&i, &j| {
        let (a, b) = (fv[i], fv[j]);
        match (a.is_nan(), b.is_nan()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
        }
    });
    *v = perm.iter().map(|&k| v[k].clone()).collect();
    *fv = perm.iter().map(|&k| fv[k]).collect();
}

/// `fminsearch(f, x0)`; `[x, fval, exitflag]`, the flag always `1`. `x` has
/// the shape of `x0`, and so has every point `f` is called with. The caps
/// are MATLAB's defaults, `200 * numel(x0)` iterations and evaluations. An
/// options argument is not provided.
fn fminsearch(it: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(a, 2, "fminsearch")?;
    need(a, 2, "fminsearch")?;
    let t = target(a, "fminsearch")?;
    let x0 = mat(a, 1, "fminsearch")?;
    let (r, c) = (x0.rows, x0.cols);
    let mut f = |x: &[f64]| {
        let arg = Value::Mat(Matrix::new(r, c, x.to_vec()));
        real_scalar(call(it, &t, vec![arg])?, "fminsearch")
    };
    let cap = 200 * x0.numel().max(1);
    let (x, fx) = nelder_mead(&mut f, &x0.data, cap, cap, NM_TOL, NM_TOL)?;
    let mut out = vec![Value::Mat(Matrix::new(r, c, x))];
    if nargout >= 2 {
        out.push(num(fx));
    }
    if nargout >= 3 {
        out.push(num(1.0));
    }
    Ok(out)
}

// ---- integral --------------------------------------------------------

/// Subintervals `integral` may hold before it gives up, MATLAB's
/// `MaxIntervalCount`.
pub const INTEGRAL_INTERVALS: usize = 650;

/// The Gauss-Kronrod 7-15 pair: the Kronrod nodes on `[0, 1)` from the
/// outermost in (the Gauss nodes are every other one, from the second),
/// then the Kronrod and Gauss weights, the centre last. QUADPACK's values.
const XGK: [f64; 7] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_4,
    0.586_087_235_467_691_1,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_5,
];
const WGK: [f64; 8] = [
    0.022_935_322_010_529_22,
    0.063_092_092_629_978_55,
    0.104_790_010_322_250_2,
    0.140_653_259_715_525_9,
    0.169_004_726_639_267_9,
    0.190_350_578_064_785_4,
    0.204_432_940_075_298_9,
    0.209_482_141_084_727_8,
];
const WG: [f64; 4] = [
    0.129_484_966_168_869_7,
    0.279_705_391_489_276_7,
    0.381_830_050_505_118_9,
    0.417_959_183_673_469_4,
];

/// The integrand `integral` works on: the values at a set of points.
pub type Integrand<'a> = dyn FnMut(&[f64]) -> R<Vec<f64>> + 'a;

/// The Kronrod estimate over `[l, r]` and its error, `|K15 - G7|`, from one
/// evaluation at the fifteen nodes.
fn gk15(g: &mut Integrand, l: f64, r: f64) -> R<(f64, f64)> {
    let (c, h) = (0.5 * (l + r), 0.5 * (r - l));
    let mut nodes = Vec::with_capacity(15);
    nodes.extend(XGK.iter().map(|x| c - h * x));
    nodes.push(c);
    nodes.extend(XGK.iter().map(|x| c + h * x));
    let y = g(&nodes)?;
    let (below, centre, above) = (&y[..7], y[7], &y[8..]);
    let mut k = vec![WGK[7] * centre];
    let mut gauss = vec![WG[3] * centre];
    for i in 0..7 {
        let pair = below[i] + above[i];
        k.push(WGK[i] * pair);
        if i % 2 == 1 {
            gauss.push(WG[i / 2] * pair);
        }
    }
    let (k, gauss) = (h * sum0(&k), h * sum0(&gauss));
    Ok((k, (k - gauss).abs()))
}

/// Whether every Kronrod node of `[l, r]` falls strictly inside it once
/// rounded. A subinterval a few ulps wide has nodes that round onto an end
/// point, and at the end of an infinite limit's map that point is the pole,
/// where `x` is infinite; such a subinterval is too narrow to halve. The
/// outermost nodes are enough to judge, since rounding is monotonic.
fn nodes_inside(l: f64, r: f64) -> bool {
    let (c, h) = (0.5 * (l + r), 0.5 * (r - l));
    l < c - h * XGK[0] && c + h * XGK[0] < r
}

/// The integral of `f` from `a` to `b` by global adaptive Gauss-Kronrod
/// quadrature: ten equal subintervals to start, then the one with the
/// largest error estimate is halved until the estimates sum to at most
/// `max(abstol, reltol * |Q|)`. More than `max_intervals` subintervals, or
/// one too narrow to halve, is an error: the integral is divergent or too
/// singular.
///
/// An infinite limit is mapped onto a finite interval first:
/// `x = a + t / (1 - t)` on `[0, 1)` for `[a, Inf)`, `x = b - t / (1 - t)`
/// for `(-Inf, b]`, and `x = t / (1 - t^2)` on `(-1, 1)` for the whole
/// line, each with its derivative as a weight. No node is ever an end
/// point, so neither the map's pole nor an integrable singularity at a
/// limit is evaluated. `b < a` is the negated integral from `b` to `a`;
/// a `NaN` limit gives `NaN`.
pub fn quad(
    f: &mut Integrand,
    a: f64,
    b: f64,
    abstol: f64,
    reltol: f64,
    max_intervals: usize,
) -> R<f64> {
    if a.is_nan() || b.is_nan() {
        return Ok(f64::NAN);
    }
    if a == b {
        return Ok(0.0);
    }
    if a > b {
        return Ok(-quad(f, b, a, abstol, reltol, max_intervals)?);
    }
    let map = |t: f64| -> (f64, f64) {
        match (a.is_finite(), b.is_finite()) {
            (true, true) => (t, 1.0),
            (true, false) => (a + t / (1.0 - t), 1.0 / ((1.0 - t) * (1.0 - t))),
            (false, true) => (b - t / (1.0 - t), 1.0 / ((1.0 - t) * (1.0 - t))),
            (false, false) => {
                let d = 1.0 - t * t;
                (t / d, (1.0 + t * t) / (d * d))
            }
        }
    };
    let (ta, tb) = match (a.is_finite(), b.is_finite()) {
        (true, true) => (a, b),
        (false, false) => (-1.0, 1.0),
        _ => (0.0, 1.0),
    };
    let mut g = |ts: &[f64]| -> R<Vec<f64>> {
        let (xs, ws): (Vec<f64>, Vec<f64>) = ts.iter().map(|&t| map(t)).unzip();
        let ys = f(&xs)?;
        let out: Vec<f64> = ys.iter().zip(&ws).map(|(y, w)| y * w).collect();
        if out.iter().any(|v| !v.is_finite()) {
            return Err(error::solver_nonfinite("integral"));
        }
        Ok(out)
    };
    const START: usize = 10;
    let mut parts: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(START);
    let width = (tb - ta) / START as f64;
    for k in 0..START {
        let l = ta + width * k as f64;
        let r = if k + 1 == START { tb } else { l + width };
        let (q, e) = gk15(&mut g, l, r)?;
        parts.push((l, r, q, e));
    }
    loop {
        let qs: Vec<f64> = parts.iter().map(|p| p.2).collect();
        let es: Vec<f64> = parts.iter().map(|p| p.3).collect();
        let (q, e) = (sum0(&qs), sum0(&es));
        if e <= abstol.max(reltol * q.abs()) {
            return Ok(q);
        }
        if parts.len() >= max_intervals {
            return Err(error::integral_limit(max_intervals));
        }
        let worst = (0..parts.len())
            .max_by(|&i, &j| parts[i].3.total_cmp(&parts[j].3))
            .unwrap_or(0);
        let (l, r, _, _) = parts.swap_remove(worst);
        let mid = 0.5 * (l + r);
        if !(nodes_inside(l, mid) && nodes_inside(mid, r)) {
            return Err(error::integral_limit(max_intervals));
        }
        let (q1, e1) = gk15(&mut g, l, mid)?;
        let (q2, e2) = gk15(&mut g, mid, r)?;
        parts.push((l, mid, q1, e1));
        parts.push((mid, r, q2, e2));
    }
}

/// A tolerance option's value: a real scalar, not negative.
fn tolerance(v: &Value, opt: &str, name: &str) -> R<f64> {
    match v {
        Value::Mat(m) if m.class != Class::Char && m.is_scalar() && m.data[0] >= 0.0 => {
            Ok(m.data[0])
        }
        _ => Err(error::option_value(opt, name)),
    }
}

/// `integral(f, a, b)` with `'AbsTol'` (default `1e-10`) and `'RelTol'`
/// (`1e-6`) as name-value pairs, MATLAB's defaults. `f` is called with a
/// row of points and must return a value at each, so it must be written
/// with element-wise operators; `'ArrayValued'` and `'Waypoints'` are not
/// provided.
fn integral(it: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    need(a, 3, "integral")?;
    let t = target(a, "integral")?;
    let lo = scalar(a, 1, "integral")?;
    let hi = scalar(a, 2, "integral")?;
    let (mut abstol, mut reltol) = (1e-10, 1e-6);
    if (a.len() - 3) % 2 != 0 {
        return Err(error::option_pairs("integral"));
    }
    for i in (3..a.len()).step_by(2) {
        let opt = option(a, i).ok_or_else(|| error::option_pairs("integral"))?;
        match opt.to_ascii_lowercase().as_str() {
            "abstol" => abstol = tolerance(&a[i + 1], &opt, "integral")?,
            "reltol" => reltol = tolerance(&a[i + 1], &opt, "integral")?,
            _ => return Err(error::unrecognized_option(&opt, "integral")),
        }
    }
    let mut f = |xs: &[f64]| -> R<Vec<f64>> {
        let v = call(it, &t, vec![Value::Mat(Matrix::row(xs.to_vec()))])?;
        match v {
            Value::Mat(m) if m.numel() == xs.len() && m.class != Class::Char => Ok(m.data),
            _ => Err(error::integral_not_elementwise()),
        }
    };
    one_mat(Matrix::scalar(quad(
        &mut f,
        lo,
        hi,
        abstol,
        reltol,
        INTEGRAL_INTERVALS,
    )?))
}

// ---- ode45 -----------------------------------------------------------

/// Step attempts `ode45` may make, accepted and rejected together.
pub const ODE_STEPS: usize = 50_000;

/// `ode45`'s options, from `odeset`; the defaults are MATLAB's.
#[derive(Clone, Debug)]
pub struct OdeOptions {
    pub rtol: f64,
    /// One tolerance, or one per component.
    pub atol: Vec<f64>,
    pub max_step: Option<f64>,
    pub initial_step: Option<f64>,
    /// Output points per step when `tspan` has two elements.
    pub refine: usize,
}

impl Default for OdeOptions {
    fn default() -> OdeOptions {
        OdeOptions {
            rtol: 1e-3,
            atol: vec![1e-6],
            max_step: None,
            initial_step: None,
            refine: 4,
        }
    }
}

/// The Dormand-Prince 5(4) pair, as MATLAB's `ode45` tabulates it: the
/// nodes, the stage coefficients row by row, the fifth-order weights, and
/// the error weights (fifth order less fourth).
const C: [f64; 6] = [0.2, 0.3, 0.8, 8.0 / 9.0, 1.0, 1.0];
const A: [[f64; 5]; 5] = [
    [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0],
    [
        19372.0 / 6561.0,
        -25360.0 / 2187.0,
        64448.0 / 6561.0,
        -212.0 / 729.0,
        0.0,
    ],
    [
        9017.0 / 3168.0,
        -355.0 / 33.0,
        46732.0 / 5247.0,
        49.0 / 176.0,
        -5103.0 / 18656.0,
    ],
];
const B: [f64; 6] = [
    35.0 / 384.0,
    0.0,
    500.0 / 1113.0,
    125.0 / 192.0,
    -2187.0 / 6784.0,
    11.0 / 84.0,
];
const E: [f64; 7] = [
    71.0 / 57600.0,
    0.0,
    -71.0 / 16695.0,
    71.0 / 1920.0,
    -17253.0 / 339200.0,
    22.0 / 525.0,
    -1.0 / 40.0,
];
/// The continuous extension (Shampine's, MATLAB's `ntrp45`): the weight of
/// stage `j` at the fraction `s` of a step is `BI[j] . [s s^2 s^3 s^4]`.
const BI: [[f64; 4]; 7] = [
    [1.0, -183.0 / 64.0, 37.0 / 12.0, -145.0 / 128.0],
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 1500.0 / 371.0, -1000.0 / 159.0, 1000.0 / 371.0],
    [0.0, -125.0 / 32.0, 125.0 / 12.0, -375.0 / 64.0],
    [0.0, 9477.0 / 3392.0, -729.0 / 106.0, 25515.0 / 6784.0],
    [0.0, -11.0 / 7.0, 11.0 / 3.0, -55.0 / 28.0],
    [0.0, 3.0 / 2.0, -4.0, 5.0 / 2.0],
];

/// The solution at the fraction `s` of the step of size `h` from `y`, whose
/// stages are `k`.
fn ntrp45(y: &[f64], h: f64, k: &[Vec<f64>], s: f64) -> Vec<f64> {
    let pw = [s, s * s, s * s * s, s * s * s * s];
    let w: Vec<f64> = BI
        .iter()
        .map(|row| row.iter().zip(&pw).map(|(a, b)| a * b).sum())
        .collect();
    (0..y.len())
        .map(|i| y[i] + h * (0..7).map(|j| w[j] * k[j][i]).sum::<f64>())
        .collect()
}

/// The ODE integrand: the derivative at a time and a state.
pub type Derivative<'a> = dyn FnMut(f64, &[f64]) -> R<Vec<f64>> + 'a;

/// `y' = f(t, y)` from `y0` over `tspan` by the Dormand-Prince pair with
/// the step control of MATLAB's `ode45`: the error is the largest
/// component of the error estimate scaled by `max(|y|, |ynew|, AbsTol /
/// RelTol)`, times the step; a step is accepted when that is at most
/// `RelTol`, and the next step is chosen from it, at most five times
/// larger and after a failure at least ten times smaller, never above
/// `MaxStep` (a tenth of the span by default) nor below `16 eps(t)`.
///
/// With two times in `tspan` the output is every step's end and, with
/// `refine` above 1, `refine - 1` points inside each step from the
/// continuous extension; with more, exactly the times asked for. The first
/// output point is `tspan(1)` itself and the last `tspan(end)`.
///
/// Returns the times and the states, the states one after another in one
/// vector. A step that fails at the minimum size, or more than `max_steps`
/// attempts, is an error. A `NaN` in the error estimate counts as a failure,
/// so a solution that blows up ends in the minimum-step error.
pub fn dopri(
    f: &mut Derivative,
    tspan: &[f64],
    y0: &[f64],
    o: &OdeOptions,
    max_steps: usize,
) -> R<(Vec<f64>, Vec<f64>)> {
    let n = y0.len();
    let (t0, tfinal) = (tspan[0], tspan[tspan.len() - 1]);
    let tdir = (tfinal - t0).signum();
    let span = (tfinal - t0).abs();
    let rtol = o.rtol.max(100.0 * EPS);
    let threshold: Vec<f64> = (0..n)
        .map(|i| o.atol[if o.atol.len() == 1 { 0 } else { i }] / rtol)
        .collect();
    let hmax = o.max_step.unwrap_or(0.1 * span).min(span);
    let hmin = |t: f64| 16.0 * eps_at(t);
    let (mut t, mut y) = (t0, y0.to_vec());
    let mut f0 = f(t, &y)?;
    let mut absh = match o.initial_step {
        Some(h) => h.min(hmax).max(hmin(t)),
        None => {
            let rh = norm_inf((0..n).map(|i| f0[i] / y[i].abs().max(threshold[i])))
                / (0.8 * rtol.powf(0.2));
            let mut absh = hmax;
            if absh * rh > 1.0 {
                absh = 1.0 / rh;
            }
            absh.max(hmin(t))
        }
    };
    let refine = if tspan.len() > 2 { 1 } else { o.refine.max(1) };
    let mut tout = vec![t0];
    let mut yout = y.clone();
    let mut next = 1;
    let mut attempts = 0;
    let mut k: Vec<Vec<f64>> = vec![vec![0.0; n]; 7];
    loop {
        let hmin_t = hmin(t);
        absh = absh.min(hmax).max(hmin_t);
        let mut h = tdir * absh;
        let mut done = false;
        if 1.1 * absh >= (tfinal - t).abs() {
            h = tfinal - t;
            absh = h.abs();
            done = true;
        }
        let mut nofailed = true;
        let (tnew, ynew, err) = loop {
            attempts += 1;
            if attempts > max_steps {
                return Err(error::ode_step_limit(max_steps));
            }
            k[0] = f0.clone();
            for s in 1..6 {
                let ys: Vec<f64> = (0..n)
                    .map(|i| y[i] + h * (0..s).map(|j| A[s - 1][j] * k[j][i]).sum::<f64>())
                    .collect();
                k[s] = f(t + C[s - 1] * h, &ys)?;
            }
            let tn = if done { tfinal } else { t + h };
            let yn: Vec<f64> = (0..n)
                .map(|i| y[i] + h * (0..6).map(|j| B[j] * k[j][i]).sum::<f64>())
                .collect();
            k[6] = f(tn, &yn)?;
            let scaled = norm_inf((0..n).map(|i| {
                let est: f64 = (0..7).map(|j| E[j] * k[j][i]).sum();
                est / y[i].abs().max(yn[i].abs()).max(threshold[i])
            }));
            let err = absh * scaled;
            if err <= rtol {
                break (tn, yn, err);
            }
            if absh <= hmin_t {
                return Err(error::ode_min_step(&fmt_g(t, 5)));
            }
            absh = if nofailed {
                nofailed = false;
                hmin_t.max(absh * 0.1_f64.max(0.8 * (rtol / err).powf(0.2)))
            } else {
                hmin_t.max(0.5 * absh)
            };
            h = tdir * absh;
            done = false;
        };
        // The output is judged as the matrix it becomes before it grows,
        // so no `Refine` can make it allocate without bound.
        check_shape(tout.len() as f64 + refine as f64, n as f64)?;
        if tspan.len() == 2 {
            for j in 1..refine {
                let s = j as f64 / refine as f64;
                tout.push(t + s * h);
                yout.extend(ntrp45(&y, h, &k, s));
            }
            tout.push(tnew);
            yout.extend_from_slice(&ynew);
        } else {
            while next < tspan.len() && tdir * (tnew - tspan[next]) >= 0.0 {
                let tq = tspan[next];
                tout.push(tq);
                if tq == tnew {
                    yout.extend_from_slice(&ynew);
                } else {
                    yout.extend(ntrp45(&y, h, &k, (tq - t) / h));
                }
                next += 1;
            }
        }
        if done {
            return Ok((tout, yout));
        }
        if nofailed {
            let temp = 1.25 * (err / rtol).powf(0.2);
            absh = if temp > 0.2 { absh / temp } else { 5.0 * absh };
        }
        t = tnew;
        y = ynew;
        f0 = k[6].clone();
    }
}

/// The options `odeset` knows, in the order its struct holds them.
const ODE_OPTIONS: [&str; 5] = ["AbsTol", "InitialStep", "MaxStep", "Refine", "RelTol"];

/// One option's value, judged: `None` for `[]`, which means the default.
/// Every option is positive and finite; `AbsTol` may have one element per
/// component, which `ode45` checks against `y0`; `Refine` is an integer.
fn ode_value(opt: &str, v: &Value, name: &str) -> R<Option<Vec<f64>>> {
    if v.is_blank() {
        return Ok(None);
    }
    let bad = || error::option_value(opt, name);
    let m = match v {
        Value::Mat(m) if m.class != Class::Char && !m.is_empty() => m,
        _ => return Err(bad()),
    };
    if m.data.iter().any(|x| !x.is_finite() || *x <= 0.0) {
        return Err(bad());
    }
    if opt != "AbsTol" && !m.is_scalar() {
        return Err(bad());
    }
    if opt == "Refine" && m.data[0].fract() != 0.0 {
        return Err(bad());
    }
    if opt == "AbsTol" && !m.is_vector() {
        return Err(bad());
    }
    Ok(Some(m.data.clone()))
}

/// `odeset('Name', value, ...)` and `odeset(old, 'Name', value, ...)`: a
/// struct with a field for each of the five options `ode45` reads, `[]` for
/// the ones not set. A name is matched in any case and stored in the
/// canonical one; a name `ode45` does not read is refused. `odeset()` with
/// no arguments returns every field empty rather than printing the list.
fn odeset(_: &mut Interp, a: &[Value], _: usize) -> R<Vec<Value>> {
    let mut values: Vec<Value> = ODE_OPTIONS.iter().map(|_| blank()).collect();
    let mut i = 0;
    if let Some(Value::Struct(s)) = a.first() {
        if s.numel() == 1 {
            for (f, v) in s.fields.iter().zip(&s.elems[0]) {
                if let Some(k) = ODE_OPTIONS.iter().position(|o| o.eq_ignore_ascii_case(f)) {
                    values[k] = v.clone();
                }
            }
        }
        i = 1;
    }
    if (a.len() - i) % 2 != 0 {
        return Err(error::option_pairs("odeset"));
    }
    while i < a.len() {
        let opt = option(a, i).ok_or_else(|| error::option_pairs("odeset"))?;
        let k = ODE_OPTIONS
            .iter()
            .position(|o| o.eq_ignore_ascii_case(&opt))
            .ok_or_else(|| error::unrecognized_option(&opt, "odeset"))?;
        ode_value(ODE_OPTIONS[k], &a[i + 1], "odeset")?;
        values[k] = a[i + 1].clone();
        i += 2;
    }
    let fields = ODE_OPTIONS.iter().map(|s| s.to_string()).collect();
    one(Value::strukt(StructArray::scalar(fields, values)))
}

/// The options in an `odeset` struct, or any struct: fields are matched in
/// any case, and fields `ode45` does not read are ignored.
fn ode_options(v: Option<&Value>, n: usize) -> R<OdeOptions> {
    let mut o = OdeOptions::default();
    let s = match v {
        None => return Ok(o),
        Some(v) if v.is_blank() => return Ok(o),
        Some(Value::Struct(s)) if s.numel() == 1 => s,
        Some(_) => return Err(error::arg_not_a_struct(4, "ode45")),
    };
    for (field, value) in s.fields.iter().zip(&s.elems[0]) {
        let Some(k) = ODE_OPTIONS
            .iter()
            .position(|o| o.eq_ignore_ascii_case(field))
        else {
            continue;
        };
        let opt = ODE_OPTIONS[k];
        let Some(x) = ode_value(opt, value, "ode45")? else {
            continue;
        };
        match opt {
            "AbsTol" if x.len() == 1 || x.len() == n => o.atol = x,
            "AbsTol" => return Err(error::option_value(opt, "ode45")),
            "InitialStep" => o.initial_step = Some(x[0]),
            "MaxStep" => o.max_step = Some(x[0]),
            "Refine" => o.refine = x[0].min(usize::MAX as f64) as usize,
            _ => o.rtol = x[0],
        }
    }
    Ok(o)
}

/// `[t, y] = ode45(f, tspan, y0)` and `ode45(f, tspan, y0, opts)`: `t` a
/// column of times and `y` one row per time, one column per component. `f`
/// is called as `f(t, y)` with `y` a column and must return a vector of
/// the same length. Asked for one output, `ode45` returns a struct with
/// `solver`, `x` (the times, a row) and `y` (one column per time), the
/// fields of MATLAB's solution struct that this cycle fills.
fn ode45(it: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(a, 3, "ode45")?;
    at_most(a, 4, "ode45")?;
    let t = target(a, "ode45")?;
    let tspan = mat(a, 1, "ode45")?;
    let ts = &tspan.data;
    let monotonic = ts.windows(2).all(|w| w[1] > w[0]) || ts.windows(2).all(|w| w[1] < w[0]);
    if ts.len() < 2 || !tspan.is_vector() || ts.iter().any(|x| !x.is_finite()) || !monotonic {
        return Err(error::ode_tspan());
    }
    let y0 = mat(a, 2, "ode45")?;
    let n = y0.numel();
    let opts = ode_options(a.get(3), n)?;
    let mut f = |tt: f64, y: &[f64]| -> R<Vec<f64>> {
        let v = call(it, &t, vec![num(tt), Value::Mat(Matrix::col(y.to_vec()))])?;
        match v {
            Value::Mat(m) if m.numel() == n && m.class != Class::Char => Ok(m.data),
            _ => Err(error::ode_value_length()),
        }
    };
    let (times, states) = dopri(&mut f, ts, &y0.data, &opts, ODE_STEPS)?;
    let m = times.len();
    let (rows, cols) = check_shape(m as f64, n as f64)?;
    let mut ymat = Matrix::filled(rows, cols, 0.0);
    for i in 0..rows {
        for j in 0..cols {
            ymat.set(i, j, states[i * cols + j]);
        }
    }
    if nargout >= 2 {
        return Ok(vec![Value::Mat(Matrix::col(times)), Value::Mat(ymat)]);
    }
    let fields = ["solver", "x", "y"].map(String::from).to_vec();
    let values = vec![
        Value::str("ode45"),
        Value::Mat(Matrix::row(times)),
        Value::Mat(ymat.transpose()),
    ];
    one(Value::strukt(StructArray::scalar(fields, values)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- fzero -------------------------------------------------------

    #[test]
    fn fzero_finds_a_zero_from_a_point_or_a_bracket() {
        let mut f = |x: f64| Ok(x * x - 2.0);
        let (x, fx) = fzero_solve(&mut f, &[1.0], FZERO_SEARCH, FZERO_ITERATIONS).unwrap();
        assert!((x - SQRT_2).abs() < 4.0 * EPS, "{x}");
        assert!(fx.abs() < 1e-14);
        let mut g = |x: f64| Ok(x.cos() - x);
        let (x, _) = fzero_solve(&mut g, &[0.0, 1.0], FZERO_SEARCH, FZERO_ITERATIONS).unwrap();
        assert!((x - 0.739_085_133_215_160_6).abs() < 1e-15, "{x}");
        // A zero at the start, or at an end of the bracket, is returned as is.
        let mut h = |x: f64| Ok(x - 3.0);
        assert_eq!(fzero_solve(&mut h, &[3.0], 10, 10).unwrap().0, 3.0);
        assert_eq!(fzero_solve(&mut h, &[0.0, 3.0], 10, 10).unwrap().0, 3.0);
        // Far from the start: the search grows its step geometrically.
        let mut far = |x: f64| Ok(x - 1e6);
        let (x, _) = fzero_solve(&mut far, &[0.0], FZERO_SEARCH, FZERO_ITERATIONS).unwrap();
        assert!((x - 1e6).abs() < 1e-8, "{x}");
        // A root at 0 is found to an absolute tolerance.
        let mut odd = |x: f64| Ok(x.powi(3) + x);
        let (x, _) = fzero_solve(&mut odd, &[1.0], FZERO_SEARCH, FZERO_ITERATIONS).unwrap();
        assert!(x.abs() < 1e-12, "{x}");
    }

    #[test]
    fn fzero_ends_every_failure_in_an_error() {
        let mut none = |x: f64| Ok(x * x + 1.0);
        let e = fzero_solve(&mut none, &[0.0], FZERO_SEARCH, FZERO_ITERATIONS).unwrap_err();
        assert!(e.msg.contains("no sign change"), "{}", e.msg);
        // The search stops at overflow, well inside the cap.
        let mut evals = 0;
        let mut counted = |x: f64| {
            evals += 1;
            Ok(x * x + 1.0)
        };
        assert!(fzero_solve(&mut counted, &[0.0], FZERO_SEARCH, 10).is_err());
        assert!(evals < 2 * FZERO_SEARCH, "{evals}");
        // The search cap itself.
        let e = fzero_solve(&mut none, &[0.0], 3, 10).unwrap_err();
        assert!(e.msg.contains("no sign change"));
        let e = fzero_solve(&mut none, &[-1.0, 1.0], 10, 10).unwrap_err();
        assert!(e.msg.contains("must differ in sign"));
        let mut nan = |_: f64| Ok(f64::NAN);
        assert!(
            fzero_solve(&mut nan, &[1.0], 10, 10)
                .unwrap_err()
                .msg
                .contains("NaN or Inf")
        );
        // The iteration cap.
        let mut slow = |x: f64| Ok(x - 0.123_456_789);
        let e = fzero_solve(&mut slow, &[0.0, 1.0], 10, 1).unwrap_err();
        assert!(e.msg.contains("did not converge"), "{}", e.msg);
        // An error from the function passes through unchanged.
        let mut failing = |_: f64| -> R<f64> { Err(error::too_many_outputs()) };
        assert_eq!(
            fzero_solve(&mut failing, &[1.0], 10, 10).unwrap_err(),
            error::too_many_outputs()
        );
    }

    // ---- fminsearch --------------------------------------------------

    #[test]
    fn nelder_mead_converges_on_a_quadratic_and_on_rosenbrock() {
        let mut q = |x: &[f64]| Ok((x[0] - 1.0).powi(2) + (x[1] - 2.0).powi(2));
        let (x, fx) = nelder_mead(&mut q, &[0.0, 0.0], 400, 400, NM_TOL, NM_TOL).unwrap();
        assert!(
            ((x[0] - 1.0).powi(2) + (x[1] - 2.0).powi(2)).sqrt() < 1e-3,
            "{x:?}"
        );
        assert!(fx < 1e-6);
        let mut banana =
            |x: &[f64]| Ok(100.0 * (x[1] - x[0] * x[0]).powi(2) + (1.0 - x[0]).powi(2));
        let (x, _) = nelder_mead(&mut banana, &[-1.2, 1.0], 400, 400, NM_TOL, NM_TOL).unwrap();
        assert!(
            (x[0] - 1.0).abs() < 1e-3 && (x[1] - 1.0).abs() < 1e-3,
            "{x:?}"
        );
        // One dimension.
        let mut p = |x: &[f64]| Ok((x[0] - 3.0).powi(2));
        let (x, _) = nelder_mead(&mut p, &[1.0], 200, 200, NM_TOL, NM_TOL).unwrap();
        assert!((x[0] - 3.0).abs() < 1e-3, "{x:?}");
    }

    #[test]
    fn nelder_mead_stops_at_its_caps() {
        let mut q = |x: &[f64]| Ok((x[0] - 1.0).powi(2) + (x[1] - 2.0).powi(2));
        let e = nelder_mead(&mut q, &[0.0, 0.0], 5, 400, NM_TOL, NM_TOL).unwrap_err();
        assert!(e.msg.contains("'fminsearch' did not converge"), "{}", e.msg);
        let e = nelder_mead(&mut q, &[0.0, 0.0], 400, 10, NM_TOL, NM_TOL).unwrap_err();
        assert!(e.msg.contains("'fminsearch' did not converge"));
        // An unbounded function runs into the cap rather than forever.
        let mut down = |x: &[f64]| Ok(-x[0]);
        assert!(nelder_mead(&mut down, &[0.0], 200, 200, NM_TOL, NM_TOL).is_err());
    }

    // ---- integral ----------------------------------------------------

    fn integrate(g: fn(f64) -> f64, a: f64, b: f64) -> R<f64> {
        let mut f = |xs: &[f64]| Ok(xs.iter().map(|&x| g(x)).collect());
        quad(&mut f, a, b, 1e-10, 1e-6, INTEGRAL_INTERVALS)
    }

    #[test]
    fn integral_meets_its_tolerance_on_known_integrals() {
        let pi = std::f64::consts::PI;
        assert!((integrate(|x| x * x, 0.0, 1.0).unwrap() - 1.0 / 3.0).abs() < 1e-14);
        assert!((integrate(f64::sin, 0.0, pi).unwrap() - 2.0).abs() < 1e-9);
        assert!(
            (integrate(|x| (-x * x).exp(), f64::NEG_INFINITY, f64::INFINITY).unwrap() - pi.sqrt())
                .abs()
                < 1e-8
        );
        assert!((integrate(|x| (-x).exp(), 0.0, f64::INFINITY).unwrap() - 1.0).abs() < 1e-8);
        assert!((integrate(|x| x.exp(), f64::NEG_INFINITY, 0.0).unwrap() - 1.0).abs() < 1e-8);
        assert!(
            (integrate(|x| 1.0 / (1.0 + x * x), f64::NEG_INFINITY, f64::INFINITY).unwrap() - pi)
                .abs()
                < 1e-7
        );
        // An integrable singularity at a limit is never evaluated.
        assert!((integrate(|x| 1.0 / x.sqrt(), 0.0, 1.0).unwrap() - 2.0).abs() < 1e-5);
        // Reversed limits negate; equal limits give zero; NaN gives NaN.
        assert!((integrate(|x| x, 1.0, 0.0).unwrap() + 0.5).abs() < 1e-14);
        assert_eq!(integrate(|x| x, 2.0, 2.0).unwrap(), 0.0);
        assert!(integrate(|x| x, f64::NAN, 1.0).unwrap().is_nan());
    }

    #[test]
    fn integral_of_a_divergent_integrand_ends_at_its_cap() {
        let e = integrate(|x| 1.0 / x, 0.0, 1.0).unwrap_err();
        assert!(e.msg.contains("limit of 650 subintervals"), "{}", e.msg);
        let mut calls = 0;
        let mut f = |xs: &[f64]| {
            calls += 1;
            Ok(xs.iter().map(|x| 1.0 / x).collect())
        };
        assert!(quad(&mut f, 0.0, 1.0, 1e-10, 1e-6, 20).is_err());
        assert!(calls <= 10 + 2 * 20, "{calls}");
        let e = integrate(|x| if x > 0.5 { f64::NAN } else { x }, 0.0, 1.0).unwrap_err();
        assert!(e.msg.contains("NaN or Inf"));
        // A divergent integral over an infinite range halves towards the
        // map's pole until the nodes would round onto it: the subinterval
        // error, never the NaN-or-Inf one for a function that returned 1.
        for (a, b) in [
            (0.0, f64::INFINITY),
            (f64::NEG_INFINITY, 0.0),
            (f64::NEG_INFINITY, f64::INFINITY),
        ] {
            let e = integrate(|_| 1.0, a, b).unwrap_err();
            assert!(e.msg.contains("limit of 650 subintervals"), "{}", e.msg);
            let e = integrate(f64::cos, a, b).unwrap_err();
            assert!(e.msg.contains("limit of 650 subintervals"), "{}", e.msg);
        }
        assert!(nodes_inside(0.0, 1.0));
        assert!(!nodes_inside(1.0, 1.0 + f64::EPSILON));
    }

    // ---- ode45 -------------------------------------------------------

    #[test]
    fn the_interpolant_ends_at_the_fifth_order_solution() {
        for (j, row) in BI.iter().enumerate() {
            let at_one: f64 = row.iter().sum();
            let b = if j < 6 { B[j] } else { 0.0 };
            assert!((at_one - b).abs() < 1e-15, "stage {j}");
        }
        // The stage coefficients of each row sum to its node.
        for (s, row) in A.iter().enumerate() {
            assert!((row.iter().sum::<f64>() - C[s]).abs() < 1e-14, "row {s}");
        }
        assert!((B.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        assert!(E.iter().sum::<f64>().abs() < 1e-15);
    }

    fn solve(
        f: &mut Derivative,
        tspan: &[f64],
        y0: &[f64],
        o: &OdeOptions,
    ) -> R<(Vec<f64>, Vec<Vec<f64>>)> {
        let (t, flat) = dopri(f, tspan, y0, o, ODE_STEPS)?;
        let n = y0.len();
        Ok((t, flat.chunks(n).map(<[f64]>::to_vec).collect()))
    }

    #[test]
    fn ode45_solves_decay_and_the_harmonic_oscillator() {
        let o = OdeOptions::default();
        let mut decay = |_: f64, y: &[f64]| Ok(vec![-y[0]]);
        let (t, y) = solve(&mut decay, &[0.0, 1.0], &[1.0], &o).unwrap();
        assert_eq!((t[0], *t.last().unwrap()), (0.0, 1.0));
        assert!((y.last().unwrap()[0] - (-1.0f64).exp()).abs() < 1e-3);
        // Refine 4: three interpolated points per step, then its end.
        assert_eq!((t.len() - 1) % 4, 0);
        assert!(t.windows(2).all(|w| w[1] > w[0]));
        for (ti, yi) in t.iter().zip(&y) {
            assert!((yi[0] - (-ti).exp()).abs() < 1e-3, "{ti}");
        }
        let pi = std::f64::consts::PI;
        let mut osc = |_: f64, y: &[f64]| Ok(vec![y[1], -y[0]]);
        let (_, y) = solve(&mut osc, &[0.0, pi], &[0.0, 1.0], &o).unwrap();
        assert!((y.last().unwrap()[1] + 1.0).abs() < 1e-3);
        // Tight tolerances are met far more closely.
        let tight = OdeOptions {
            rtol: 1e-10,
            atol: vec![1e-12],
            ..OdeOptions::default()
        };
        let (_, y) = solve(&mut osc, &[0.0, pi], &[0.0, 1.0], &tight).unwrap();
        assert!((y.last().unwrap()[1] + 1.0).abs() < 1e-8);
        assert!(y.last().unwrap()[0].abs() < 1e-8);
        // Backwards in time, and output at the times asked for.
        let (t, y) = solve(&mut decay, &[1.0, 0.5, 0.0], &[(-1.0f64).exp()], &tight).unwrap();
        assert_eq!(t, [1.0, 0.5, 0.0]);
        assert!((y[1][0] - (-0.5f64).exp()).abs() < 1e-8);
        assert!((y[2][0] - 1.0).abs() < 1e-8);
    }

    #[test]
    fn ode45_ends_every_failure_in_an_error() {
        let o = OdeOptions::default();
        // y' = y^2 from 1 blows up at t = 1.
        let mut blowup = |_: f64, y: &[f64]| Ok(vec![y[0] * y[0]]);
        let e = solve(&mut blowup, &[0.0, 2.0], &[1.0], &o).unwrap_err();
        assert!(
            e.msg.contains("smallest allowed") || e.msg.contains("steps"),
            "{}",
            e.msg
        );
        // The step cap.
        let mut decay = |_: f64, y: &[f64]| Ok(vec![-y[0]]);
        let e = dopri(&mut decay, &[0.0, 100.0], &[1.0], &o, 5).unwrap_err();
        assert!(e.msg.contains("limit of 5 steps"), "{}", e.msg);
        let mut nan = |_: f64, _: &[f64]| Ok(vec![f64::NAN]);
        let e = solve(&mut nan, &[0.0, 1.0], &[1.0], &o).unwrap_err();
        assert!(e.msg.contains("smallest allowed"), "{}", e.msg);
    }

    #[test]
    fn norm_inf_propagates_nan() {
        assert_eq!(norm_inf([1.0, -3.0, 2.0].into_iter()), 3.0);
        assert!(norm_inf([1.0, f64::NAN].into_iter()).is_nan());
        assert_eq!(norm_inf(std::iter::empty()), 0.0);
    }
}
