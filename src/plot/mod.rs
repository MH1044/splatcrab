//! Plotting (cycle 12): the builtins `figure`, `gcf`, `close`, `clf`,
//! `subplot`, `plot`, `scatter`, `bar`, `histogram`, `xlabel`, `ylabel`,
//! `title`, `legend`, `grid`, `axis`, `xlim`, `ylim`, `hold`, `saveas` and
//! `print`, over three files:
//!
//! - `figure.rs`: the figure state `Interp` holds, line specs, the tick
//!   rule and the layout of a figure into a scene of drawing items;
//! - `svg.rs`: the SVG writer;
//! - `png.rs`: the rasterizer, its bitmap font and the store-only zlib and
//!   PNG encoder.
//!
//! This file reads the builtins' arguments and changes the state. Nothing
//! here draws until a figure is saved, printed or asked for by
//! `Interp::figure_svg`, so a plotting call costs the copy of its data and
//! no more. A figure number is a double, as `gcf` returns it: there are no
//! graphics objects.
//!
//! No builtin opens a viewer: only the interactive REPL in `main.rs` does,
//! after an entry, for the figures `Interp::take_changed_figures` names.

pub mod figure;
pub mod png;
pub mod svg;

use std::path::Path;

use crate::builtins::args::{at_most, check_shape, need, scalar, string};
use crate::builtins::{Registry, add, none, one_mat};
use crate::error::{self, R};
use crate::interp::Interp;
use crate::value::{Matrix, Value};
use figure::{Axes, Color, Dash, LineSpec, MAX_FIGURE, Place, Series, parse_line_spec};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "figure", figure, "figure, figure(n), n = figure - make a new figure, or figure n, current.");
    add(r, "gcf", gcf, "gcf - the current figure's number, a new figure made if none is open.");
    add(r, "close", close, "close, close(n), close all - close the current figure, figure n, or every figure.");
    add(r, "clf", clf, "clf - clear the current figure.");
    add(r, "subplot", subplot, "subplot(m,n,p) - make the p-th axes of an m-by-n grid current.");
    add(r, "plot", plot, "plot(y), plot(x,y), plot(x,y,spec,...) - lines, one per column of a matrix; spec such as 'r--o'.");
    add(r, "scatter", scatter, "scatter(x,y,sz,c,'filled') - a circle at each point.");
    add(r, "bar", bar, "bar(y), bar(x,y,width) - a bar for each value, grouped for a matrix.");
    add(r, "histogram", histogram, "histogram(x), histogram(x,nbins), histogram(x,edges) - bars of counts.");
    add(r, "xlabel", xlabel, "xlabel(text) - label the x axis.");
    add(r, "ylabel", ylabel, "ylabel(text) - label the y axis.");
    add(r, "title", title, "title(text) - title the axes.");
    add(r, "legend", legend, "legend(labels...), legend(cell), legend off - label the series.");
    add(r, "grid", grid, "grid on, grid off, grid - show, hide or toggle grid lines.");
    add(r, "axis", axis, "axis([x0 x1 y0 y1]), axis tight|auto|equal|square|normal|manual|on|off, v = axis.");
    add(r, "xlim", xlim, "xlim([lo hi]), xlim auto, v = xlim - the x limits.");
    add(r, "ylim", ylim, "ylim([lo hi]), ylim auto, v = ylim - the y limits.");
    add(r, "hold", hold, "hold on, hold off, hold - keep, replace or toggle what the axes holds.");
    add(r, "saveas", saveas, "saveas(fig,name), saveas(fig,name,fmt) - save a figure as svg or png.");
    add(r, "print", print, "print('-dpng','-r150',name), print('-dsvg',name) - save the current figure.");
}

// ---- figures -------------------------------------------------------------

/// A figure number: a positive whole number of 32 bits.
fn figure_number(v: f64) -> R<u32> {
    if (1.0..=MAX_FIGURE).contains(&v) && v.fract() == 0.0 {
        Ok(v as u32)
    } else {
        Err(error::invalid_figure())
    }
}

/// Argument `i` as the number of an open figure.
fn open_figure(it: &Interp, args: &[Value], i: usize, name: &str) -> R<u32> {
    if args.get(i).is_some_and(Value::is_char) {
        return Err(error::invalid_figure());
    }
    let n = figure_number(scalar(args, i, name)?)?;
    if !it.figures.is_open(n) {
        return Err(error::invalid_figure());
    }
    Ok(n)
}

fn number(n: u32) -> R<Vec<Value>> {
    one_mat(Matrix::scalar(n as f64))
}

fn figure(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    at_most(args, 1, "figure")?;
    let n = match args.first() {
        None => it.figures.create(),
        Some(v) => {
            if let Some(t) = v.text() {
                return Err(error::plot_option("figure", &t));
            }
            let n = figure_number(scalar(args, 0, "figure")?)?;
            it.figures.select(n);
            n
        }
    };
    if nargout > 0 { number(n) } else { none() }
}

fn gcf(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 0, "gcf")?;
    let n = it.figures.gcf();
    number(n)
}

fn close(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "close")?;
    match args.first() {
        None => {
            if let Some(n) = it.figures.current() {
                it.figures.close(n);
            }
        }
        Some(v) => {
            if let Some(t) = v.text() {
                if !t.eq_ignore_ascii_case("all") {
                    return Err(error::plot_option("close", &t));
                }
                it.figures.close_all();
            } else {
                let m = v.mat()?;
                // Every number is judged before any figure closes.
                let mut ns = Vec::with_capacity(m.numel());
                for &x in &m.data {
                    let n = figure_number(x)?;
                    if !it.figures.is_open(n) {
                        return Err(error::invalid_figure());
                    }
                    ns.push(n);
                }
                for n in ns {
                    it.figures.close(n);
                }
            }
        }
    }
    none()
}

fn clf(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "clf")?;
    if let Some(v) = args.first() {
        match v.text() {
            Some(t) if t.eq_ignore_ascii_case("reset") => {}
            Some(t) => return Err(error::plot_option("clf", &t)),
            None => return Err(error::arg_not_a_string(1, "clf")),
        }
    }
    it.figures.current_mut().clear();
    none()
}

/// A positive whole number no larger than a figure number, for a subplot
/// grid's size.
fn grid_size(v: f64) -> R<u64> {
    if (1.0..=MAX_FIGURE).contains(&v) && v.fract() == 0.0 {
        Ok(v as u64)
    } else {
        Err(error::bad_subplot())
    }
}

fn subplot(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let whole = |i: usize| -> R<f64> {
        if args[i].is_char() {
            return Err(error::bad_subplot());
        }
        scalar(args, i, "subplot").map_err(|_| error::bad_subplot())
    };
    let (m, n, ps) = match args.len() {
        1 => {
            // subplot(211): three digits.
            let d = whole(0)?;
            if !(111.0..=999.0).contains(&d) || d.fract() != 0.0 {
                return Err(error::bad_subplot());
            }
            let d = d as u64;
            (d / 100, d / 10 % 10, vec![(d % 10) as f64])
        }
        3 => {
            let (m, n) = (grid_size(whole(0)?)?, grid_size(whole(1)?)?);
            if args[2].is_char() {
                return Err(error::bad_subplot());
            }
            (m, n, args[2].mat()?.data.clone())
        }
        _ => return Err(error::bad_subplot()),
    };
    if m == 0 || n == 0 || ps.is_empty() {
        return Err(error::bad_subplot());
    }
    let cells = (m * n) as f64;
    let mut lo = u64::MAX;
    let mut hi = 0;
    for p in ps {
        if !(p >= 1.0 && p <= cells && p.fract() == 0.0) {
            return Err(error::bad_subplot());
        }
        lo = lo.min(p as u64 - 1);
        hi = hi.max(p as u64 - 1);
    }
    let place = Place::cells(m as usize, n as usize, lo as usize, hi as usize);
    it.figures.current_mut().subplot(place);
    none()
}

// ---- data ----------------------------------------------------------------

/// Argument `i` as numeric data, borrowed: a char there is never data.
/// Nothing is copied until the budget has been judged.
fn data<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a Matrix> {
    match args.get(i) {
        Some(v) if v.is_char() => Err(error::plot_data(i + 1, name)),
        Some(v) => v.mat(),
        None => Err(error::not_enough_args(name)),
    }
}

fn is_vector(m: &Matrix) -> bool {
    m.rows <= 1 || m.cols <= 1
}

/// Which elements of a matrix one line takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Take {
    /// `1..=n`, for a `y` given alone.
    Count(usize),
    All,
    Col(usize),
    Row(usize),
}

/// A copy of what `t` takes of `m`.
fn take(m: Option<&Matrix>, t: Take) -> Vec<f64> {
    match (t, m) {
        (Take::Count(n), _) => (1..=n).map(|k| k as f64).collect(),
        (Take::All, Some(m)) => m.data.clone(),
        (Take::Col(c), Some(m)) => m.data[c * m.rows..(c + 1) * m.rows].to_vec(),
        (Take::Row(r), Some(m)) => (0..m.cols).map(|c| m.get(r, c)).collect(),
        (_, None) => Vec::new(),
    }
}

/// Element `k` of what `t` takes of `m`, read in place.
fn take_at(m: Option<&Matrix>, t: Take, k: usize) -> f64 {
    match (t, m) {
        (Take::Count(_), _) => (k + 1) as f64,
        (Take::All, Some(m)) => m.data[k],
        (Take::Col(c), Some(m)) => m.data[c * m.rows + k],
        (Take::Row(r), Some(m)) => m.get(r, k),
        (_, None) => f64::NAN,
    }
}

/// One data group of `plot`: `y`, or `x` and `y`, borrowed from the
/// arguments, and its line spec.
struct Group<'a> {
    x: Option<&'a Matrix>,
    y: &'a Matrix,
    spec: LineSpec,
    /// Its lines, each taking some of `x` and of `y`.
    lines: Lines,
}

/// A line's style: the spec's, else none when it names a marker (markers
/// alone), else solid.
fn line_dash(spec: &LineSpec) -> Option<Dash> {
    match (spec.dash, spec.marker) {
        (Some(d), _) => Some(d),
        (None, Some(_)) => None,
        (None, None) => Some(Dash::Solid),
    }
}

impl Group<'_> {
    /// The points one line has, from what it takes of `y`.
    fn len(&self, ty: Take) -> usize {
        match ty {
            Take::Count(n) => n,
            Take::All => self.y.numel(),
            Take::Col(_) => self.y.rows,
            Take::Row(_) => self.y.cols,
        }
    }

    /// What the group's lines count against the budget, read from the
    /// arguments in place: what [`Series::cost`] gives their copies.
    fn cost(&self) -> usize {
        let drawn = line_dash(&self.spec).is_some();
        self.lines.iter().fold(0usize, |total, (tx, ty)| {
            let n = self.len(ty);
            let runs = if drawn {
                figure::runs((0..n).map(|k| (take_at(self.x, tx, k), take_at(Some(self.y), ty, k))))
            } else {
                0
            };
            total.saturating_add(figure::line_cost(n, runs, drawn, self.spec.marker))
        })
    }
}

/// What every line of a group takes of one matrix: the same whole of it
/// (or `1..=n`), or its `k`th column or row for line `k`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    Count(usize),
    All,
    Col,
    Row,
}

impl Pick {
    fn of(self, k: usize) -> Take {
        match self {
            Pick::Count(n) => Take::Count(n),
            Pick::All => Take::All,
            Pick::Col => Take::Col(k),
            Pick::Row => Take::Row(k),
        }
    }
}

/// The lines of a group: `n` of them, line `k` taking `x.of(k)` and
/// `y.of(k)`. A description rather than a list, so a matrix of millions of
/// columns costs nothing before the budget has been judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Lines {
    n: usize,
    x: Pick,
    y: Pick,
}

impl Lines {
    fn iter(self) -> impl Iterator<Item = (Take, Take)> {
        (0..self.n).map(move |k| (self.x.of(k), self.y.of(k)))
    }
}

/// The lines of one group, MATLAB's rules: a vector `y` alone is one line
/// against `1:n` and a matrix one line per column against `1:rows`; two
/// vectors of one length are one line; a vector against a matrix pairs it
/// with the columns whose length it has, else the rows; two matrices of one
/// size pair their columns.
fn group_lines(x: Option<&Matrix>, y: &Matrix, name: &str) -> R<Lines> {
    let lines = |n, x, y| Ok(Lines { n, x, y });
    let Some(x) = x else {
        if is_vector(y) {
            return lines(1, Pick::Count(y.numel()), Pick::All);
        }
        return lines(y.cols, Pick::Count(y.rows), Pick::Col);
    };
    let mismatch = || Err(error::plot_lengths(name));
    match (is_vector(x), is_vector(y)) {
        (true, true) => {
            if x.numel() != y.numel() {
                return mismatch();
            }
            lines(1, Pick::All, Pick::All)
        }
        (true, false) => {
            let n = x.numel();
            if n == y.rows {
                lines(y.cols, Pick::All, Pick::Col)
            } else if n == y.cols {
                lines(y.rows, Pick::All, Pick::Row)
            } else {
                mismatch()
            }
        }
        (false, true) => {
            let n = y.numel();
            if n == x.rows {
                lines(x.cols, Pick::Col, Pick::All)
            } else if n == x.cols {
                lines(x.rows, Pick::Row, Pick::All)
            } else {
                mismatch()
            }
        }
        (false, false) => {
            if (x.rows, x.cols) != (y.rows, y.cols) {
                return mismatch();
            }
            lines(x.cols, Pick::Col, Pick::Col)
        }
    }
}

/// `plot`'s arguments as groups: data, then optionally more data, then
/// optionally a line spec, repeated.
fn plot_groups(args: &[Value]) -> R<Vec<Group<'_>>> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if let Some(t) = args[i].text() {
            return Err(error::bad_line_spec(&t));
        }
        let first = data(args, i, "plot")?;
        i += 1;
        let (x, y) = match args.get(i) {
            Some(v) if !v.is_char() => {
                i += 1;
                (Some(first), data(args, i - 1, "plot")?)
            }
            _ => (None, first),
        };
        let mut spec = LineSpec::default();
        if let Some(t) = args.get(i).and_then(Value::text) {
            spec = parse_line_spec(&t).ok_or_else(|| error::bad_line_spec(&t))?;
            i += 1;
        }
        let lines = group_lines(x, y, "plot")?;
        groups.push(Group { x, y, spec, lines });
    }
    Ok(groups)
}

fn plot(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "plot")?;
    let groups = plot_groups(args)?;
    let count = groups
        .iter()
        .fold(0usize, |n, g| n.saturating_add(g.cost()));
    it.figures.add_series(count, |ax| {
        let mut out = Vec::new();
        for g in &groups {
            for (tx, ty) in g.lines.iter() {
                let color = g.spec.color.unwrap_or_else(|| ax.next_color());
                out.push(Series::Line {
                    x: take(g.x, tx),
                    y: take(Some(g.y), ty),
                    color,
                    dash: line_dash(&g.spec),
                    marker: g.spec.marker,
                });
            }
        }
        out
    })?;
    none()
}

/// A colour given as a line-spec letter, or as an RGB triple of fractions.
fn color_arg(v: &Value, name: &str) -> R<Color> {
    if let Some(t) = v.text() {
        return match parse_line_spec(&t) {
            Some(LineSpec {
                color: Some(c),
                dash: None,
                marker: None,
            }) => Ok(c),
            _ => Err(error::bad_color(name)),
        };
    }
    let m = v.mat()?;
    if m.numel() == 3 {
        if let Some(c) = Color::from_fractions(m.data[0], m.data[1], m.data[2]) {
            return Ok(c);
        }
    }
    Err(error::bad_color(name))
}

fn scatter(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "scatter")?;
    at_most(args, 5, "scatter")?;
    let x = data(args, 0, "scatter")?;
    let y = data(args, 1, "scatter")?;
    if x.numel() != y.numel() {
        return Err(error::plot_lengths("scatter"));
    }
    let n = x.numel();
    // MATLAB's default area, 36 points squared, for every point.
    let mut sizes: &[f64] = &[36.0];
    let mut color = None;
    let mut filled = false;
    for (i, a) in args.iter().enumerate().skip(2) {
        if let Some(t) = a.text() {
            if t.eq_ignore_ascii_case("filled") {
                filled = true;
            } else {
                color = Some(color_arg(a, "scatter")?);
            }
            continue;
        }
        match i {
            2 => {
                let m = a.mat()?;
                if !m.is_empty() {
                    let ok = (m.numel() == 1 || m.numel() == n)
                        && m.data.iter().all(|&s| s.is_finite() && s > 0.0);
                    if !ok {
                        return Err(error::bad_scatter_size());
                    }
                    sizes = &m.data;
                }
            }
            3 => color = Some(color_arg(a, "scatter")?),
            _ => return Err(error::too_many_args()),
        }
    }
    it.figures.add_series(figure::scatter_cost(n), |ax| {
        vec![Series::Scatter {
            x: x.data.clone(),
            y: y.data.clone(),
            sizes: sizes.to_vec(),
            color: color.unwrap_or_else(|| ax.next_color()),
            filled,
        }]
    })?;
    none()
}

/// The smallest gap between neighbouring bar positions, in the order
/// given, or 1 when there is none: what a bar's width is a fraction of.
fn spacing(x: &[f64]) -> f64 {
    let d = x
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .filter(|d| *d > 0.0 && d.is_finite())
        .fold(f64::INFINITY, f64::min);
    if d.is_finite() { d } else { 1.0 }
}

fn bar(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "bar")?;
    let nums = args.iter().take_while(|v| !v.is_char()).count();
    if nums == 0 {
        return Err(error::plot_data(1, "bar"));
    }
    let mut color = None;
    for (k, a) in args.iter().enumerate().skip(nums) {
        let Some(t) = a.text() else {
            return Err(error::plot_data(k + 1, "bar"));
        };
        if t.eq_ignore_ascii_case("grouped") {
            continue;
        }
        match parse_line_spec(&t) {
            Some(LineSpec {
                color: Some(c),
                dash: None,
                marker: None,
            }) => color = Some(c),
            _ => return Err(error::plot_option("bar", &t)),
        }
    }
    let width_of = |i: usize| -> R<f64> {
        let w = scalar(args, i, "bar")?;
        if w.is_finite() && w > 0.0 {
            Ok(w)
        } else {
            Err(error::bad_bar_width())
        }
    };
    let (x, y, width) = match nums {
        1 => (None, data(args, 0, "bar")?, 0.8),
        2 => {
            let (a, b) = (data(args, 0, "bar")?, data(args, 1, "bar")?);
            if b.is_scalar() && a.numel() > 1 {
                (None, a, width_of(1)?)
            } else {
                (Some(a), b, 0.8)
            }
        }
        3 => (
            Some(data(args, 0, "bar")?),
            data(args, 1, "bar")?,
            width_of(2)?,
        ),
        _ => return Err(error::too_many_args()),
    };
    // A vector is one series; a matrix one series per column, grouped by
    // row.
    let (groups, series) = if is_vector(y) {
        (y.numel(), 1)
    } else {
        (y.rows, y.cols)
    };
    if x.is_some_and(|x| x.numel() != groups) {
        return Err(error::plot_lengths("bar"));
    }
    // The positions are read in place, 1 to n when none are given, whose
    // spacing is 1.
    let at = |g: usize| x.map_or((g + 1) as f64, |x| x.data[g]);
    let group = width * x.map_or(1.0, |x| spacing(&x.data));
    let each = group / series as f64;
    let count = figure::bars_cost(groups).saturating_mul(series);
    it.figures.add_series(count, |ax| {
        (0..series)
            .map(|j| {
                let rects = (0..groups)
                    .map(|g| {
                        let h = if series == 1 { y.data[g] } else { y.get(g, j) };
                        let left = at(g) - group / 2.0 + j as f64 * each;
                        [left, left + each, h]
                    })
                    .collect();
                Series::Bars {
                    rects,
                    color: color.unwrap_or_else(|| ax.next_color()),
                    opacity: 1.0,
                }
            })
            .collect()
    })?;
    none()
}

/// How `histogram` bins its data: a count, or edges borrowed from the
/// arguments.
enum Bins<'a> {
    Count(usize),
    Edges(&'a [f64]),
}

/// `n` equal bins from `lo` to `hi`: the count of each for the finite
/// values of `data`, the last bin closed, and the `n + 1` edges. Computed
/// on halves, so a range wider than the largest double (`[-1e308 1e308]`)
/// keeps finite edges and bins, and any other range gets exactly what
/// `lo + k * (hi - lo) / n` gives, since halving a double is exact (but
/// for the subnormal ones).
fn equal_bins(data: &[f64], lo: f64, hi: f64, n: usize) -> (Vec<f64>, Vec<f64>) {
    let half = (hi * 0.5 - lo * 0.5) / n as f64;
    let mut counts = vec![0.0; n];
    for &v in data.iter().filter(|v| v.is_finite()) {
        let k = (((v * 0.5 - lo * 0.5) / half).floor().max(0.0) as usize).min(n - 1);
        counts[k] += 1.0;
    }
    let edges = (0..=n)
        .map(|k| {
            if k == n {
                hi
            } else {
                (lo * 0.5 + half * k as f64) * 2.0
            }
        })
        .collect();
    (counts, edges)
}

fn histogram(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "histogram")?;
    at_most(args, 2, "histogram")?;
    let x = data(args, 0, "histogram")?;
    let finite = x.data.iter().filter(|v| v.is_finite()).count();
    let bins = match args.get(1) {
        None => {
            // Sturges' rule, a recorded choice: MATLAB's automatic binning
            // is its own.
            Bins::Count(((finite.max(1) as f64).log2() + 1.0).ceil() as usize)
        }
        Some(_) => {
            let m = data(args, 1, "histogram")?;
            if let Some(n) = m.scalar_value() {
                if !(n.is_finite() && n >= 1.0 && n.fract() == 0.0) {
                    return Err(error::bad_bins());
                }
                let (_, n) = check_shape(1.0, n)?;
                Bins::Count(n)
            } else {
                let e = &m.data[..];
                if e.len() < 2 || e.iter().any(|v| v.is_nan()) || e.windows(2).any(|w| w[0] >= w[1])
                {
                    return Err(error::bad_bins());
                }
                Bins::Edges(e)
            }
        }
    };
    let nbins = match &bins {
        Bins::Count(_) if finite == 0 && args.len() < 2 => 0,
        Bins::Count(n) => *n,
        Bins::Edges(e) => e.len() - 1,
    };
    it.figures.add_series(figure::bars_cost(nbins), |ax| {
        let (counts, edges) = match bins {
            Bins::Edges(e) => {
                let mut counts = vec![0.0; nbins];
                let last = e[e.len() - 1];
                for &v in &x.data {
                    if v.is_nan() || v < e[0] || v > last {
                        continue;
                    }
                    let k = if v == last {
                        nbins - 1
                    } else {
                        e.partition_point(|&b| b <= v) - 1
                    };
                    counts[k] += 1.0;
                }
                (counts, e.to_vec())
            }
            Bins::Count(_) if nbins == 0 => (Vec::new(), Vec::new()),
            Bins::Count(_) => {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for &v in x.data.iter().filter(|v| v.is_finite()) {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
                if !lo.is_finite() {
                    (lo, hi) = (0.0, 1.0);
                }
                if lo == hi {
                    (lo, hi) = (lo - 0.5, hi + 0.5);
                }
                equal_bins(&x.data, lo, hi, nbins)
            }
        };
        let rects = counts
            .iter()
            .enumerate()
            .map(|(k, &c)| [edges[k], edges[k + 1], c])
            .collect();
        vec![Series::Bars {
            rects,
            color: ax.next_color(),
            opacity: 0.6,
        }]
    })?;
    none()
}

// ---- decoration ----------------------------------------------------------

/// The current axes of the current figure, both made if need be.
fn gca(it: &mut Interp) -> &mut Axes {
    it.figures.current_mut().gca()
}

fn label(args: &[Value], name: &str) -> R<String> {
    need(args, 1, name)?;
    at_most(args, 1, name)?;
    string(args, 0, name)
}

fn xlabel(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let s = label(args, "xlabel")?;
    gca(it).xlabel = s;
    none()
}

fn ylabel(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let s = label(args, "ylabel")?;
    gca(it).ylabel = s;
    none()
}

fn title(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let s = label(args, "title")?;
    gca(it).title = s;
    none()
}

fn legend(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let labels = match args {
        [] => None,
        [one] => match one {
            Value::Cell(c) => {
                let mut v = Vec::with_capacity(c.data.len());
                for e in &c.data {
                    v.push(
                        e.text()
                            .ok_or_else(|| error::arg_not_a_string(1, "legend"))?,
                    );
                }
                Some(v)
            }
            v => {
                let t = v
                    .text()
                    .ok_or_else(|| error::arg_not_a_string(1, "legend"))?;
                match t.to_ascii_lowercase().as_str() {
                    "off" | "hide" => {
                        gca(it).legend = None;
                        return none();
                    }
                    "show" | "on" if gca(it).legend.is_some() => return none(),
                    "show" | "on" => None,
                    _ => Some(vec![t]),
                }
            }
        },
        _ => {
            let mut v = Vec::with_capacity(args.len());
            for (k, a) in args.iter().enumerate() {
                v.push(
                    a.text()
                        .ok_or_else(|| error::arg_not_a_string(k + 1, "legend"))?,
                );
            }
            Some(v)
        }
    };
    let ax = gca(it);
    // With no labels, each series is named data1, data2, ...
    let labels = labels.unwrap_or_else(|| {
        (1..=ax.series.len())
            .map(|k| format!("data{}", k))
            .collect()
    });
    ax.legend = Some(labels);
    none()
}

/// `on`, `off` or nothing (a toggle), for `grid` and `hold`.
fn on_off(args: &[Value], name: &str) -> R<Option<bool>> {
    at_most(args, 1, name)?;
    let Some(v) = args.first() else {
        return Ok(None);
    };
    let t = v.text().ok_or_else(|| error::arg_not_a_string(1, name))?;
    match t.to_ascii_lowercase().as_str() {
        "on" => Ok(Some(true)),
        "off" => Ok(Some(false)),
        // MATLAB's old spelling of `hold on`.
        "all" if name == "hold" => Ok(Some(true)),
        _ => Err(error::plot_option(name, &t)),
    }
}

fn grid(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let v = on_off(args, "grid")?;
    let ax = gca(it);
    ax.grid = v.unwrap_or(!ax.grid);
    none()
}

fn hold(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let v = on_off(args, "hold")?;
    let ax = gca(it);
    ax.hold = v.unwrap_or(!ax.hold);
    none()
}

/// Limits a user gave: `n / 2` pairs of increasing numbers, an infinite end
/// standing for the automatic one.
fn limit_pairs(v: &Value, n: usize, name: &str) -> R<Vec<(f64, f64)>> {
    let bad = || error::bad_limits(name, n);
    if v.is_char() {
        return Err(bad());
    }
    let m = v.mat()?;
    if m.numel() != n {
        return Err(bad());
    }
    m.data
        .chunks(2)
        .map(|p| {
            if p[0].is_nan()
                || p[1].is_nan()
                || p[0] >= p[1]
                || p[0] == f64::INFINITY
                || p[1] == f64::NEG_INFINITY
            {
                Err(bad())
            } else {
                Ok((p[0], p[1]))
            }
        })
        .collect()
}

fn axis(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    if args.is_empty() {
        let (a, b, c, d) = gca(it).limits();
        return one_mat(Matrix::row(vec![a, b, c, d]));
    }
    for v in args {
        let ax = gca(it);
        let Some(t) = v.text() else {
            let p = limit_pairs(v, 4, "axis")?;
            ax.xlim = Some(p[0]);
            ax.ylim = Some(p[1]);
            continue;
        };
        match t.to_ascii_lowercase().as_str() {
            "auto" => {
                ax.xlim = None;
                ax.ylim = None;
                ax.tight = false;
            }
            "tight" => {
                ax.xlim = None;
                ax.ylim = None;
                ax.tight = true;
            }
            "equal" => ax.equal = true,
            "image" => {
                ax.equal = true;
                ax.tight = true;
            }
            "square" => ax.square = true,
            "normal" => {
                ax.square = false;
                ax.equal = false;
            }
            "manual" => {
                let (a, b, c, d) = ax.limits();
                ax.xlim = Some((a, b));
                ax.ylim = Some((c, d));
            }
            "on" => ax.visible = true,
            "off" => ax.visible = false,
            _ => return Err(error::plot_option("axis", &t)),
        }
    }
    none()
}

/// `xlim` and `ylim`, `y` saying which.
fn lim(it: &mut Interp, args: &[Value], y: bool) -> R<Vec<Value>> {
    let name = if y { "ylim" } else { "xlim" };
    at_most(args, 1, name)?;
    let ax = gca(it);
    let (a, b, c, d) = ax.limits();
    let now = if y { (c, d) } else { (a, b) };
    let Some(v) = args.first() else {
        return one_mat(Matrix::row(vec![now.0, now.1]));
    };
    let set = match v.text() {
        Some(t) if t.eq_ignore_ascii_case("auto") => None,
        Some(t) if t.eq_ignore_ascii_case("manual") => Some(now),
        Some(t) => return Err(error::plot_option(name, &t)),
        None => Some(limit_pairs(v, 2, name)?[0]),
    };
    if y {
        ax.ylim = set;
    } else {
        ax.xlim = set;
    }
    none()
}

fn xlim(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    lim(it, args, false)
}

fn ylim(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    lim(it, args, true)
}

// ---- output --------------------------------------------------------------

/// A file format the plot module writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Svg,
    Png,
}

fn format_named(s: &str) -> Option<Format> {
    match s.to_ascii_lowercase().as_str() {
        "svg" => Some(Format::Svg),
        "png" => Some(Format::Png),
        _ => None,
    }
}

fn extension(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
}

/// The bytes of figure `n` in `format`, at `dpi` for a PNG. The image's
/// pixel size is judged before anything is drawn.
fn figure_bytes(it: &Interp, n: u32, format: Format, dpi: f64) -> R<Vec<u8>> {
    let fig = it.figures.get(n).ok_or_else(error::invalid_figure)?;
    let sc = figure::scene(fig);
    Ok(match format {
        Format::Svg => svg::render(&sc).into_bytes(),
        Format::Png => {
            let (w, h) = png::pixel_size(dpi / 96.0)?;
            png::render(&sc, w, h)
        }
    })
}

fn saveas(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "saveas")?;
    at_most(args, 3, "saveas")?;
    let n = open_figure(it, args, 0, "saveas")?;
    let mut name = string(args, 1, "saveas")?;
    let format = if args.len() == 3 {
        let f = string(args, 2, "saveas")?;
        let format = format_named(&f).ok_or_else(|| error::plot_format("saveas", &f))?;
        if extension(&name).is_none() {
            name = format!("{}.{}", name, f.to_ascii_lowercase());
        }
        format
    } else {
        // A name with no extension, or only its dot, names no format.
        let ext = extension(&name).unwrap_or_default();
        if ext.is_empty() {
            return Err(error::saveas_no_extension(&name));
        }
        format_named(&ext).ok_or_else(|| error::plot_format("saveas", &ext))?
    };
    let bytes = figure_bytes(it, n, format, 96.0)?;
    crate::builtins::io::write_file(it, &name, &bytes)?;
    none()
}

fn print(it: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    let mut fig = None;
    let mut format = None;
    let mut dpi = 96.0;
    let mut name: Option<String> = None;
    for (k, a) in args.iter().enumerate() {
        let Some(t) = a.text() else {
            if k == 0 {
                fig = Some(open_figure(it, args, 0, "print")?);
                continue;
            }
            return Err(error::arg_not_a_string(k + 1, "print"));
        };
        if let Some(dev) = t.strip_prefix("-d") {
            format = Some(format_named(dev).ok_or_else(|| error::plot_format("print", dev))?);
        } else if let Some(r) = t.strip_prefix("-r") {
            let v: f64 = r.trim().parse().map_err(|_| error::bad_resolution(&t))?;
            if !(v.is_finite() && v >= 0.0) {
                return Err(error::bad_resolution(&t));
            }
            // -r0 is the screen's resolution.
            dpi = if v == 0.0 { 96.0 } else { v };
        } else if let Some(f) = t.strip_prefix("-f") {
            let v: f64 = f.trim().parse().map_err(|_| error::invalid_figure())?;
            let n = figure_number(v)?;
            if !it.figures.is_open(n) {
                return Err(error::invalid_figure());
            }
            fig = Some(n);
        } else if t.starts_with('-') || name.is_some() {
            return Err(error::plot_option("print", &t));
        } else {
            name = Some(t);
        }
    }
    let mut name = name.ok_or_else(error::print_needs_file)?;
    let ext = extension(&name);
    let format = match (format, ext.as_deref().and_then(format_named)) {
        (Some(f), _) => f,
        (None, Some(f)) => f,
        (None, None) => Format::Png,
    };
    if ext.is_none() {
        name.push_str(if format == Format::Svg {
            ".svg"
        } else {
            ".png"
        });
    }
    let n = match fig {
        Some(n) => n,
        None => it.figures.gcf(),
    };
    let bytes = figure_bytes(it, n, format, dpi)?;
    crate::builtins::io::write_file(it, &name, &bytes)?;
    none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interp() -> Interp {
        Interp::with_sinks(Box::new(std::io::sink()), Box::new(std::io::sink()))
    }

    /// The in-memory accessor (Scope): a front end reads each open figure's
    /// SVG by number, with no file written.
    #[test]
    fn figure_svg_and_figure_numbers_read_open_figures_in_memory() {
        let mut it = interp();
        assert!(it.figure_numbers().is_empty());
        assert_eq!(it.figure_svg(1), None);
        it.run_command("figure; plot(1:3, [1 4 9]); figure(4); bar([1 2])")
            .unwrap();
        assert_eq!(it.figure_numbers(), [1, 4]);
        let one = it.figure_svg(1).unwrap();
        assert_eq!(one.matches("<polyline").count(), 1);
        assert!(one.contains(">9<"));
        let four = it.figure_svg(4).unwrap();
        assert_eq!(four.matches("<rect class=\"bar\"").count(), 2);
        assert_eq!(it.figure_svg(2), None);
        // The accessor renders what the figure holds now.
        it.run_command("figure(1); hold on; plot(1:2)").unwrap();
        assert_eq!(it.figure_svg(1).unwrap().matches("<polyline").count(), 2);
        assert_eq!(it.take_changed_figures(), [1, 4]);
        assert!(it.take_changed_figures().is_empty());
        it.run_command("close all").unwrap();
        assert!(it.figure_numbers().is_empty());
    }

    #[test]
    fn plot_groups_follow_matlabs_pairing_rules() {
        let m = |r, c| Matrix::new(r, c, (0..r * c).map(|k| k as f64).collect());
        let lines = |x: Option<&Matrix>, y: &Matrix| group_lines(x, y, "plot").map(|l| l.n);
        assert_eq!(lines(None, &m(1, 5)).unwrap(), 1);
        assert_eq!(lines(None, &m(3, 2)).unwrap(), 2);
        assert_eq!(lines(Some(&m(1, 3)), &m(3, 4)).unwrap(), 4);
        assert_eq!(lines(Some(&m(1, 4)), &m(3, 4)).unwrap(), 3);
        assert_eq!(lines(Some(&m(3, 2)), &m(1, 3)).unwrap(), 2);
        assert_eq!(lines(Some(&m(3, 2)), &m(3, 2)).unwrap(), 2);
        assert!(lines(Some(&m(1, 3)), &m(1, 4)).is_err());
        assert!(lines(Some(&m(2, 2)), &m(3, 3)).is_err());
        assert_eq!(take(Some(&m(3, 2)), Take::Col(1)), [3.0, 4.0, 5.0]);
        assert_eq!(take(Some(&m(3, 2)), Take::Row(1)), [1.0, 4.0]);
        assert_eq!(take(None, Take::Count(3)), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn the_builtins_change_the_state_they_name() {
        let mut it = interp();
        it.run_command("x = 1:10; plot(x, x.^2, 'k:', x, x, 'o'); grid on; title('T'); axis tight")
            .unwrap();
        let n = it.figures.current().unwrap();
        let ax = &it.figures.get(n).unwrap().axes[0];
        assert_eq!(ax.series.len(), 2);
        assert!(ax.grid && ax.tight);
        assert_eq!(ax.title, "T");
        assert_eq!(ax.limits(), (1.0, 10.0, 1.0, 100.0));
        it.run_command("xlim([0 20]); v = xlim; w = axis;").unwrap();
        let v = it.vars()["v"].mat().unwrap().data.clone();
        assert_eq!(v, [0.0, 20.0]);
        let w = it.vars()["w"].mat().unwrap().data.clone();
        assert_eq!(w, [0.0, 20.0, 1.0, 100.0]);
        // histogram of six values in three bins counts 2, 1 and 3.
        it.run_command("histogram([1 1 2 3 3 3], 3)").unwrap();
        let ax = &it.figures.get(n).unwrap().axes[0];
        let Series::Bars { rects, .. } = &ax.series[0] else {
            panic!("bars")
        };
        let counts: Vec<f64> = rects.iter().map(|r| r[2]).collect();
        assert_eq!(counts, [2.0, 1.0, 3.0]);
        // Bin edges put an end value in the last bin.
        it.run_command("histogram([0 0.5 1 2], [0 1 2])").unwrap();
        let ax = &it.figures.get(n).unwrap().axes[0];
        let Series::Bars { rects, .. } = &ax.series[0] else {
            panic!("bars")
        };
        assert_eq!(rects.iter().map(|r| r[2]).collect::<Vec<_>>(), [2.0, 2.0]);
        // A grouped bar chart is one series per column.
        it.run_command("bar([1 2; 3 4; 5 6])").unwrap();
        assert_eq!(it.figures.get(n).unwrap().axes[0].series.len(), 2);
        it.run_command("clf").unwrap();
        assert!(it.figures.get(n).unwrap().axes.is_empty());
    }

    #[test]
    fn bad_arguments_are_clean_errors() {
        let mut it = interp();
        let err = |it: &mut Interp, src: &str| it.run_command(src).unwrap_err().msg;
        assert_eq!(
            err(&mut it, "plot(1:3, 'LineWidth')"),
            error::bad_line_spec("LineWidth").msg
        );
        assert_eq!(
            err(&mut it, "plot(1:3, 1:4)"),
            error::plot_lengths("plot").msg
        );
        assert_eq!(err(&mut it, "close(7)"), error::invalid_figure().msg);
        assert_eq!(err(&mut it, "figure(1.5)"), error::invalid_figure().msg);
        assert_eq!(err(&mut it, "subplot(2, 1, 3)"), error::bad_subplot().msg);
        assert_eq!(
            err(&mut it, "xlim([2 1])"),
            error::bad_limits("xlim", 2).msg
        );
        assert_eq!(
            err(&mut it, "axis([0 1 0])"),
            error::bad_limits("axis", 4).msg
        );
        assert_eq!(
            err(&mut it, "hold maybe"),
            error::plot_option("hold", "maybe").msg
        );
        assert_eq!(
            err(&mut it, "saveas(gcf, 'x.jpg')"),
            error::plot_format("saveas", "jpg").msg
        );
        assert_eq!(
            err(&mut it, "print('-djpeg', 'x')"),
            error::plot_format("print", "jpeg").msg
        );
        assert_eq!(
            err(&mut it, "print('-rabc', 'x.png')"),
            error::bad_resolution("-rabc").msg
        );
        assert_eq!(
            err(&mut it, "print('-dpng')"),
            error::print_needs_file().msg
        );
        assert_eq!(err(&mut it, "histogram(1:3, 0)"), error::bad_bins().msg);
        assert_eq!(err(&mut it, "histogram(1:3, [1 1])"), error::bad_bins().msg);
        assert_eq!(
            err(&mut it, "scatter(1:2, 1:2, -1)"),
            error::bad_scatter_size().msg
        );
        assert_eq!(
            err(&mut it, "scatter(1:2, 1:2, 3, 'q')"),
            error::bad_color("scatter").msg
        );
        assert_eq!(err(&mut it, "bar(1:3, 0)"), error::bad_bar_width().msg);
        assert_eq!(err(&mut it, "bar('abc')"), error::plot_data(1, "bar").msg);
        assert!(err(&mut it, "print('-dpng', '-r100000', 'x.png')").starts_with("Requested "));
        assert!(err(&mut it, "histogram(1:3, 1e12)").starts_with("Requested "));
        for name in ["x", "x."] {
            assert_eq!(
                err(&mut it, &format!("saveas(gcf, '{name}')")),
                error::saveas_no_extension(name).msg
            );
        }
    }

    /// The budget is counted from the arguments, read in place, before
    /// anything is copied, and comes to what the copies cost.
    #[test]
    fn the_budget_counted_from_the_arguments_is_what_the_series_cost() {
        let mut it = interp();
        for src in [
            "plot([1 2 NaN 4 5], 'o-')",
            "x = [1 NaN 3]; plot(x, [x; 2*x; x]', ':h', [1 2; 3 4])",
            "plot([1 2 3], [1 2; 3 4; 5 6])",
            "scatter(1:4, [4 3 NaN 1], [10 20 30 40])",
            "bar([1 2; 3 4; 5 6])",
            "histogram([1 2 2 3 Inf], [0 1 2 3])",
            "histogram([NaN NaN])",
        ] {
            it.run_command(&format!("close all; {src}")).unwrap();
            let fig = it.figures.get(1).unwrap();
            let sum: usize = fig.axes[0].series.iter().map(Series::cost).sum();
            assert_eq!(fig.points(), sum, "{src}");
        }
        // A million hexagrams, which a count of one a point admitted, come
        // to about eighteen million: refused, with no figure made.
        it.run_command("close all").unwrap();
        assert_eq!(
            it.run_command("plot(1:1e6, 'h')").unwrap_err().msg,
            error::too_many_points(figure::MAX_POINTS).msg
        );
        assert!(it.figure_numbers().is_empty());
        assert_eq!(
            it.run_command("scatter(1:3e6, 1:3e6)").unwrap_err().msg,
            error::too_many_points(figure::MAX_POINTS).msg
        );
        assert!(it.figure_numbers().is_empty());
    }

    /// A range wider than the largest double used to give an infinite bin
    /// width, `NaN` edges and no bars drawn.
    #[test]
    fn histogram_bins_a_range_past_the_largest_double() {
        let mut it = interp();
        it.run_command("histogram([1e308 -1e308], 2)").unwrap();
        let fig = it.figures.get(1).unwrap();
        let Series::Bars { rects, .. } = &fig.axes[0].series[0] else {
            panic!("bars")
        };
        assert_eq!(rects, &[[-1e308, 0.0, 1.0], [0.0, 1e308, 1.0]]);
        let svg = it.figure_svg(1).unwrap();
        assert_eq!(svg.matches("<rect class=\"bar\"").count(), 2);
        assert!(!svg.contains("NaN"));
        // Any other range gets the edges `lo + k * (hi - lo) / n` gives.
        let (counts, edges) = equal_bins(&[1.0, 1.0, 2.0, 3.0, 3.0, 3.0], 1.0, 3.0, 3);
        assert_eq!(counts, [2.0, 1.0, 3.0]);
        let w = 2.0 / 3.0;
        assert_eq!(edges, [1.0, 1.0 + w, 1.0 + w * 2.0, 3.0]);
        let (counts, edges) = equal_bins(&[-f64::MAX, f64::MAX, 0.0], -f64::MAX, f64::MAX, 4);
        assert_eq!(counts, [1.0, 0.0, 1.0, 1.0]);
        assert!(edges.iter().all(|e| e.is_finite()), "{edges:?}");
        assert!(edges.windows(2).all(|w| w[0] < w[1]), "{edges:?}");
    }
}
