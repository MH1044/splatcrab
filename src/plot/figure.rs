//! Figure state and layout (cycle 12).
//!
//! [`Figures`] is what `Interp` holds: every open figure by number, the
//! order they were last made current in (the last is the current figure),
//! and the numbers changed since the REPL last asked. A [`Figure`] holds its
//! [`Axes`], one per subplot, and the index of the current one; an `Axes`
//! holds its [`Series`] and everything `hold`, `grid`, `axis`, `xlim`,
//! `ylim`, `title`, `xlabel`, `ylabel` and `legend` set.
//!
//! [`scene`] lays a figure out once into a [`Scene`], a flat list of drawing
//! [`Item`]s in pixels of the 560x420 figure, y downwards. The SVG writer
//! (`svg.rs`) serialises it and the rasterizer (`png.rs`) draws it, so the
//! two formats cannot disagree about where anything is.
//!
//! **Bounded work.** Every series is a copy of arrays the user already
//! holds, and a figure holds at most [`MAX_POINTS`] points, each thing it
//! draws weighted by what it writes (see [`MAX_POINTS`]), judged from the
//! arguments before a plotting call copies or changes anything. The layout
//! is one pass over the points; the ticks are at most eleven per axis
//! whatever the limits are.
//!
//! **The tick rule.** The step is the smallest of 1, 2 and 5 times a power
//! of ten that splits the axis into at most `n` intervals, where `n` is the
//! axes' width over 50 pixels (height over 30 for y), clamped to 2..=10.
//! Automatic limits then widen outwards to the nearest multiples of that
//! step, so both ends carry a tick; `axis tight` and manual limits keep the
//! limits and tick every multiple of the step inside them. A tick is
//! labelled from its integer multiple and the decimal exponent of the step,
//! never from a float's digits, so `9` is `9` and `0.3` is `0.3`; a label
//! longer than ten characters is written in `%g` form instead.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{self, R};
use crate::interp::fmt_g;

/// The figure's size in pixels: MATLAB's default figure position.
pub const WIDTH: f64 = 560.0;
pub const HEIGHT: f64 = 420.0;

/// The most points one figure holds; a plotting call that would pass it is
/// refused before it copies or changes anything.
///
/// A point is what a line's vertex costs: its `x` and `y` in the series, its
/// pair in the scene and about 14 bytes of SVG (`123.45,67.89 `). Everything
/// else counts its SVG bytes over 16, rounded up, measured on typical
/// coordinates: a marker its outline's points (a circle its centre) plus
/// [`TAG`], so `o` counts 6 (84 bytes) and a hexagram 17 (226); a scatter
/// circle [`SCATTER`], a bar [`BAR`], and each run of a line [`RUN`] for its
/// `<polyline>` element. Each series counts [`LABEL`], weighed by memory
/// rather than SVG, and one sample of each thing it draws, for the legend
/// row it may be given. So no figure the budget admits writes much more
/// than 16 bytes a point, 256 MiB, and memory follows: a unit of any kind
/// takes no more than a line's vertex, about 48 bytes with the SVG. At the
/// largest size each kind admits, the release build writes 167 to 235 MiB
/// and peaks at 444 to 771 MB, the plain line the heaviest.
pub const MAX_POINTS: usize = 1 << 24;

/// What a marker's element costs beyond its outline's points: its tag and
/// colours, about 70 bytes of SVG.
pub const TAG: usize = 5;

/// A run of a line beyond its vertices: a `<polyline>` element's own text,
/// 80 bytes, 109 with a dash array.
pub const RUN: usize = 7;

/// A scatter point: a `<circle>`, 84 to 92 bytes, its centre plus [`TAG`].
pub const SCATTER: usize = 1 + TAG;

/// A bar: a `<rect class="bar">`, 122 bytes, 138 with a histogram's
/// opacity; its four numbers plus [`TAG`].
pub const BAR: usize = 4 + TAG;

/// A series' record and its legend text, about 112 bytes of SVG but some
/// 300 of memory: the record, its label and the text's scene item. Its
/// legend samples count as what they show.
pub const LABEL: usize = 9;

/// The largest figure number: MATLAB's figure numbers are 32-bit integers.
pub const MAX_FIGURE: f64 = 2_147_483_647.0;

/// Pixel coordinates are clamped to this, so a point far outside the limits
/// neither writes a 300-digit number into the SVG nor costs the rasterizer
/// more than its clipped part.
const COORD_LIMIT: f64 = 1e6;

/// The largest marker radius, in figure pixels.
pub const MAX_MARKER_RADIUS: f64 = 50.0;

/// A plot marker's radius, in pixels: MATLAB's default `MarkerSize` of 6
/// points is 8 pixels across.
const MARKER_RADIUS: f64 = 4.0;

/// A colour, 8 bits a channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    /// `#rrggbb`, lower case, which is how the SVG writes every colour.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// The colour of a line-spec letter.
    pub fn from_letter(c: char) -> Option<Color> {
        Some(match c {
            'r' => Color(255, 0, 0),
            'g' => Color(0, 255, 0),
            'b' => Color(0, 0, 255),
            'c' => Color(0, 255, 255),
            'm' => Color(255, 0, 255),
            'y' => Color(255, 255, 0),
            'k' => Color(0, 0, 0),
            'w' => Color(255, 255, 255),
            _ => return None,
        })
    }

    /// An RGB triple of fractions in `[0, 1]`.
    pub fn from_fractions(r: f64, g: f64, b: f64) -> Option<Color> {
        let ch = |v: f64| (0.0..=1.0).contains(&v).then(|| (v * 255.0).round() as u8);
        Some(Color(ch(r)?, ch(g)?, ch(b)?))
    }
}

pub const WHITE: Color = Color(255, 255, 255);
pub const BLACK: Color = Color(0, 0, 0);
/// MATLAB's axis and text colour, `[0.15 0.15 0.15]`.
pub const AXIS_COLOR: Color = Color(38, 38, 38);
/// Grid lines: the axis colour at MATLAB's grid alpha of 0.15 over white.
pub const GRID_COLOR: Color = Color(223, 223, 223);

/// MATLAB's default colour order since R2014b, which successive series take
/// in turn.
pub const COLOR_ORDER: [Color; 7] = [
    Color(0, 114, 189),
    Color(217, 83, 25),
    Color(237, 177, 32),
    Color(126, 47, 142),
    Color(119, 172, 48),
    Color(77, 190, 238),
    Color(162, 20, 47),
];

/// A line style from a line spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dash {
    /// `-`
    Solid,
    /// `--`
    Dashed,
    /// `:`
    Dotted,
    /// `-.`
    DashDot,
}

impl Dash {
    /// The on and off lengths in pixels, empty for a solid line: what the
    /// SVG's `stroke-dasharray` says and the rasterizer walks.
    pub fn pattern(self) -> &'static [f64] {
        match self {
            Dash::Solid => &[],
            Dash::Dashed => &[6.0, 4.0],
            Dash::Dotted => &[1.5, 3.0],
            Dash::DashDot => &[6.0, 3.0, 1.5, 3.0],
        }
    }
}

/// A marker from a line spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Circle,
    Plus,
    Star,
    Point,
    Cross,
    Square,
    Diamond,
    Up,
    Down,
    Right,
    Left,
    Pentagram,
    Hexagram,
}

impl Marker {
    fn from_char(c: char) -> Option<Marker> {
        Some(match c {
            'o' => Marker::Circle,
            '+' => Marker::Plus,
            '*' => Marker::Star,
            '.' => Marker::Point,
            'x' => Marker::Cross,
            's' => Marker::Square,
            'd' => Marker::Diamond,
            '^' => Marker::Up,
            'v' => Marker::Down,
            '>' => Marker::Right,
            '<' => Marker::Left,
            'p' => Marker::Pentagram,
            'h' => Marker::Hexagram,
            _ => return None,
        })
    }

    /// What one of these markers counts against [`MAX_POINTS`]: the points
    /// of its outline, a circle's centre or a stroke's two ends, plus
    /// [`TAG`]; from 6 for `o` to 17 for a hexagram.
    pub fn weight(self) -> usize {
        TAG + match marker_shape(self, 0.0, 0.0, 1.0) {
            Shape::Circle { .. } => 1,
            Shape::Polygon(p) => p.len(),
            Shape::Strokes(s) => 2 * s.len(),
        }
    }
}

/// What a line spec such as `'r--o'` asks for; each part is `None` when the
/// spec does not name it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineSpec {
    pub color: Option<Color>,
    pub dash: Option<Dash>,
    pub marker: Option<Marker>,
}

/// Parses a MATLAB line spec: at most one colour letter (`rgbcmykw`), one
/// line style (`-`, `--`, `:`, `-.`) and one marker (`o+*.xsd^v><ph`), in
/// any order. `None` for anything else, a part given twice included. The
/// spelled-out names (`'square'`, `'none'`) are not read.
pub fn parse_line_spec(s: &str) -> Option<LineSpec> {
    let chars: Vec<char> = s.chars().collect();
    let mut spec = LineSpec::default();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let clash = match c {
            '-' => {
                let d = match chars.get(i + 1) {
                    Some('-') => Dash::Dashed,
                    Some('.') => Dash::DashDot,
                    _ => Dash::Solid,
                };
                if d != Dash::Solid {
                    i += 1;
                }
                spec.dash.replace(d).is_some()
            }
            ':' => spec.dash.replace(Dash::Dotted).is_some(),
            _ => {
                if let Some(col) = Color::from_letter(c) {
                    spec.color.replace(col).is_some()
                } else {
                    spec.marker.replace(Marker::from_char(c)?).is_some()
                }
            }
        };
        if clash {
            return None;
        }
        i += 1;
    }
    Some(spec)
}

/// One thing drawn in an axes.
#[derive(Clone, Debug, PartialEq)]
pub enum Series {
    /// `plot`: the points in order, a line through them unless `dash` is
    /// `None`, and a marker at each when `marker` is set. A non-finite
    /// coordinate is a gap.
    Line {
        x: Vec<f64>,
        y: Vec<f64>,
        color: Color,
        dash: Option<Dash>,
        marker: Option<Marker>,
    },
    /// `scatter`: a circle at each point, `sizes` holding one area in
    /// points squared for all of them or one each.
    Scatter {
        x: Vec<f64>,
        y: Vec<f64>,
        sizes: Vec<f64>,
        color: Color,
        filled: bool,
    },
    /// `bar` and `histogram`: rectangles `[left, right, height]` standing
    /// on zero.
    Bars {
        rects: Vec<[f64; 3]>,
        color: Color,
        opacity: f64,
    },
}

/// The runs of a line, the stretches of finite points each drawn as one
/// `<polyline>`, from its points in order.
pub fn runs(points: impl Iterator<Item = (f64, f64)>) -> usize {
    let mut runs = 0;
    let mut inside = false;
    for (a, b) in points {
        let finite = a.is_finite() && b.is_finite();
        if finite && !inside {
            runs += 1;
        }
        inside = finite;
    }
    runs
}

/// What a line of `n` points counts against [`MAX_POINTS`]: a point each,
/// a [`RUN`] for each of its `runs` and one for its legend sample when the
/// line is drawn, a marker's weight for each point and one for its sample,
/// and a [`LABEL`]. A marker is counted at every point, finite or not, so
/// only the runs need a look at the data.
pub fn line_cost(n: usize, runs: usize, drawn: bool, marker: Option<Marker>) -> usize {
    let line = if drawn {
        RUN.saturating_mul(runs.saturating_add(1))
    } else {
        0
    };
    let marks = marker.map_or(0, |m| m.weight().saturating_mul(n.saturating_add(1)));
    n.saturating_add(line)
        .saturating_add(marks)
        .saturating_add(LABEL)
}

/// What `n` scatter points count: [`SCATTER`] each and one for the legend
/// sample, and a [`LABEL`].
pub fn scatter_cost(n: usize) -> usize {
    SCATTER
        .saturating_mul(n.saturating_add(1))
        .saturating_add(LABEL)
}

/// What a series of `n` bars counts: [`BAR`] each and one for the legend
/// sample, and a [`LABEL`].
pub fn bars_cost(n: usize) -> usize {
    BAR.saturating_mul(n.saturating_add(1))
        .saturating_add(LABEL)
}

impl Series {
    /// What the series counts against [`MAX_POINTS`]; a plotting call
    /// counts the same from its arguments before it copies them.
    pub fn cost(&self) -> usize {
        match self {
            Series::Line {
                x, y, dash, marker, ..
            } => {
                let drawn = dash.is_some();
                let runs = if drawn {
                    runs(x.iter().copied().zip(y.iter().copied()))
                } else {
                    0
                };
                line_cost(x.len(), runs, drawn, *marker)
            }
            Series::Scatter { x, .. } => scatter_cost(x.len()),
            Series::Bars { rects, .. } => bars_cost(rects.len()),
        }
    }

    /// The finite extent of the series along x and y.
    fn extent(&self, xs: &mut Span, ys: &mut Span) {
        match self {
            Series::Line { x, y, .. } | Series::Scatter { x, y, .. } => {
                for (&a, &b) in x.iter().zip(y) {
                    if a.is_finite() && b.is_finite() {
                        xs.add(a);
                        ys.add(b);
                    }
                }
            }
            Series::Bars { rects, .. } => {
                for r in rects {
                    if r.iter().all(|v| v.is_finite()) {
                        xs.add(r[0]);
                        xs.add(r[1]);
                        ys.add(0.0);
                        ys.add(r[2]);
                    }
                }
            }
        }
    }
}

/// A running minimum and maximum of finite values.
#[derive(Clone, Copy, Debug, Default)]
struct Span {
    lo: f64,
    hi: f64,
    any: bool,
}

impl Span {
    fn add(&mut self, v: f64) {
        if !v.is_finite() {
            return;
        }
        if self.any {
            self.lo = self.lo.min(v);
            self.hi = self.hi.max(v);
        } else {
            *self = Span {
                lo: v,
                hi: v,
                any: true,
            };
        }
    }
}

/// Where an axes sits: a rectangle in fractions of the figure, y downwards.
/// The whole figure for a plain axes, a cell (or a span of cells) of the
/// grid for a subplot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Place {
    pub const WHOLE: Place = Place {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };

    /// Cells `first..=last` (zero-based, numbered along the rows from the
    /// top left, as `subplot` numbers them) of a `rows` by `cols` grid,
    /// as the smallest rectangle holding both.
    pub fn cells(rows: usize, cols: usize, first: usize, last: usize) -> Place {
        let (r0, c0) = (first / cols, first % cols);
        let (r1, c1) = (last / cols, last % cols);
        let (ra, rb) = (r0.min(r1) as f64, (r0.max(r1) + 1) as f64);
        let (ca, cb) = (c0.min(c1) as f64, (c0.max(c1) + 1) as f64);
        Place {
            x0: ca / cols as f64,
            y0: ra / rows as f64,
            x1: cb / cols as f64,
            y1: rb / rows as f64,
        }
    }

    fn same(&self, o: &Place) -> bool {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        close(self.x0, o.x0) && close(self.y0, o.y0) && close(self.x1, o.x1) && close(self.y1, o.y1)
    }

    fn overlaps(&self, o: &Place) -> bool {
        let w = self.x1.min(o.x1) - self.x0.max(o.x0);
        let h = self.y1.min(o.y1) - self.y0.max(o.y0);
        w > 1e-9 && h > 1e-9
    }
}

/// One axes: its place, what it draws and how.
#[derive(Clone, Debug, PartialEq)]
pub struct Axes {
    pub place: Place,
    pub series: Vec<Series>,
    pub hold: bool,
    pub title: String,
    pub xlabel: String,
    pub ylabel: String,
    /// The legend's labels, `None` when there is no legend.
    pub legend: Option<Vec<String>>,
    pub grid: bool,
    /// Manual limits; an infinite end is automatic, as in MATLAB.
    pub xlim: Option<(f64, f64)>,
    pub ylim: Option<(f64, f64)>,
    /// `axis off` hides the box, the ticks and the axis labels.
    pub visible: bool,
    /// `axis tight`: automatic limits are the data's own.
    pub tight: bool,
    /// `axis equal`: one unit is as long along x as along y.
    pub equal: bool,
    /// `axis square`: the plot box is square.
    pub square: bool,
    /// The next colour of [`COLOR_ORDER`] an automatic series takes.
    next_color: usize,
    points: usize,
}

impl Axes {
    pub fn new(place: Place) -> Axes {
        Axes {
            place,
            series: Vec::new(),
            hold: false,
            title: String::new(),
            xlabel: String::new(),
            ylabel: String::new(),
            legend: None,
            grid: false,
            xlim: None,
            ylim: None,
            visible: true,
            tight: false,
            equal: false,
            square: false,
            next_color: 0,
            points: 0,
        }
    }

    /// What a plotting call does to an axes whose hold is off: MATLAB's
    /// `NextPlot` `'replace'`, which clears it and resets everything but
    /// its place.
    fn reset(&mut self) {
        *self = Axes::new(self.place);
    }

    /// The next colour of the colour order, for a series that names none.
    pub fn next_color(&mut self) -> Color {
        let c = COLOR_ORDER[self.next_color % COLOR_ORDER.len()];
        self.next_color = (self.next_color + 1) % COLOR_ORDER.len();
        c
    }

    /// The limits this axes shows, `(xlo, xhi, ylo, yhi)`: what `axis`,
    /// `xlim` and `ylim` return.
    pub fn limits(&self) -> (f64, f64, f64, f64) {
        let r = inner_rect(self);
        let (x, y) = axis_limits(self, r);
        (x.lo, x.hi, y.lo, y.hi)
    }
}

/// One figure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Figure {
    pub axes: Vec<Axes>,
    pub current: Option<usize>,
    points: usize,
}

impl Figure {
    /// The points the figure holds; see [`MAX_POINTS`].
    pub fn points(&self) -> usize {
        self.points
    }

    fn recount(&mut self) {
        self.points = self.axes.iter().map(|a| a.points).sum();
    }

    /// The figure's points once a plotting call adds `count` to its current
    /// axes: all it holds under hold, else all but that axes' own, which
    /// the call replaces. Refused past [`MAX_POINTS`]; nothing is changed
    /// either way.
    fn judge(&self, count: usize) -> R<usize> {
        let kept = match self.current {
            Some(k) if k < self.axes.len() && !self.axes[k].hold => {
                self.points - self.axes[k].points
            }
            // Under hold, or when a new whole-figure axes will be made.
            _ => self.points,
        };
        kept.checked_add(count)
            .filter(|&n| n <= MAX_POINTS)
            .ok_or_else(|| error::too_many_points(MAX_POINTS))
    }

    /// The current axes' index, making a whole-figure axes first when there
    /// is none, as MATLAB's `gca` does.
    fn gca_index(&mut self) -> usize {
        match self.current {
            Some(k) if k < self.axes.len() => k,
            _ => {
                self.axes.push(Axes::new(Place::WHOLE));
                let k = self.axes.len() - 1;
                self.current = Some(k);
                k
            }
        }
    }

    /// The current axes, made if there is none.
    pub fn gca(&mut self) -> &mut Axes {
        let k = self.gca_index();
        &mut self.axes[k]
    }

    /// `clf`: every axes goes.
    pub fn clear(&mut self) {
        *self = Figure::default();
    }

    /// `subplot`: the axes at `place` becomes current, made if there is
    /// none. As in MATLAB, a new one deletes every axes it overlaps.
    pub fn subplot(&mut self, place: Place) {
        if let Some(k) = self.axes.iter().position(|a| a.place.same(&place)) {
            self.current = Some(k);
            return;
        }
        self.axes.retain(|a| !a.place.overlaps(&place));
        self.axes.push(Axes::new(place));
        self.current = Some(self.axes.len() - 1);
        self.recount();
    }

    /// Adds series costing `count` points (see [`Series::cost`]) to the
    /// current axes, cleared first unless its hold is on. `make` builds
    /// them, given the axes so it can take colours from its order; it runs
    /// only once the budget has been judged, so a refusal copies nothing,
    /// makes no axes and changes nothing.
    pub fn add_series(
        &mut self,
        count: usize,
        make: impl FnOnce(&mut Axes) -> Vec<Series>,
    ) -> R<()> {
        let total = self.judge(count)?;
        let k = self.gca_index();
        let ax = &mut self.axes[k];
        if !ax.hold {
            ax.reset();
        }
        ax.points += count;
        let made = make(ax);
        ax.series.extend(made);
        self.points = total;
        Ok(())
    }
}

/// Every open figure; see the module comment.
#[derive(Debug, Default)]
pub struct Figures {
    figs: BTreeMap<u32, Figure>,
    /// Figure numbers, the most recently current last.
    order: Vec<u32>,
    /// Figures changed since [`Figures::take_changed`] last ran.
    changed: BTreeSet<u32>,
}

impl Figures {
    /// The current figure's number, if any figure is open.
    pub fn current(&self) -> Option<u32> {
        self.order.last().copied()
    }

    /// `figure`: a new figure with the lowest unused number, made current.
    pub fn create(&mut self) -> u32 {
        let mut n = 1;
        for &k in self.figs.keys() {
            if k == n {
                n += 1;
            } else if k > n {
                break;
            }
        }
        self.select(n);
        n
    }

    /// `figure(n)`: figure `n` becomes current, made if it is not open.
    pub fn select(&mut self, n: u32) {
        self.figs.entry(n).or_default();
        self.order.retain(|&k| k != n);
        self.order.push(n);
        self.changed.insert(n);
    }

    /// Closes figure `n`; false when it is not open.
    pub fn close(&mut self, n: u32) -> bool {
        let open = self.figs.remove(&n).is_some();
        self.order.retain(|&k| k != n);
        self.changed.remove(&n);
        open
    }

    /// `close all`.
    pub fn close_all(&mut self) {
        *self = Figures::default();
    }

    /// `gcf`: the current figure's number, a new figure made if none is
    /// open.
    pub fn gcf(&mut self) -> u32 {
        match self.current() {
            Some(n) => n,
            None => self.create(),
        }
    }

    /// The current figure, made if none is open, marked changed: what
    /// every builtin that draws goes through.
    pub fn current_mut(&mut self) -> &mut Figure {
        let n = self.gcf();
        self.changed.insert(n);
        self.figs.entry(n).or_default()
    }

    /// [`Figure::add_series`] on the current figure, the budget judged
    /// first: a refusal makes no figure and marks none changed.
    pub fn add_series(
        &mut self,
        count: usize,
        make: impl FnOnce(&mut Axes) -> Vec<Series>,
    ) -> R<()> {
        match self.current().and_then(|n| self.figs.get(&n)) {
            Some(f) => f.judge(count)?,
            None => Figure::default().judge(count)?,
        };
        self.current_mut().add_series(count, make)
    }

    pub fn is_open(&self, n: u32) -> bool {
        self.figs.contains_key(&n)
    }

    pub fn get(&self, n: u32) -> Option<&Figure> {
        self.figs.get(&n)
    }

    /// Every open figure's number, ascending.
    pub fn numbers(&self) -> Vec<u32> {
        self.figs.keys().copied().collect()
    }

    /// The open figures changed since the last call, ascending; what the
    /// REPL hands to the viewer.
    pub fn take_changed(&mut self) -> Vec<u32> {
        let changed = std::mem::take(&mut self.changed);
        changed.into_iter().filter(|n| self.is_open(*n)).collect()
    }
}

// ---- the scene ---------------------------------------------------------

/// A rectangle in figure pixels, y downwards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Where a text's `x` is: its start, middle or end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// A marker's outline, in pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    /// A closed outline.
    Polygon(Vec<(f64, f64)>),
    /// Separate strokes, `[x1, y1, x2, y2]` each.
    Strokes(Vec<[f64; 4]>),
}

/// One thing to draw. A text's `y` is its vertical middle, so the SVG
/// writer and the bitmap font agree on where it sits.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Opens a group, clipped to `clip` when set; closed by the next
    /// unmatched [`Item::End`].
    Begin {
        class: &'static str,
        clip: Option<Rect>,
    },
    End,
    Rect {
        class: &'static str,
        r: Rect,
        fill: Option<Color>,
        opacity: f64,
        stroke: Option<Color>,
    },
    Line {
        class: &'static str,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke: Color,
        dash: Dash,
    },
    /// A data line: the one item written as `<polyline>`.
    Polyline {
        points: Vec<(f64, f64)>,
        stroke: Color,
        dash: Dash,
    },
    /// A marker. `sample` marks a legend's, which the SVG writes so that it
    /// is never counted with the data's circles.
    Marker {
        shape: Shape,
        stroke: Color,
        fill: Option<Color>,
        sample: bool,
    },
    Text {
        class: &'static str,
        x: f64,
        y: f64,
        anchor: Anchor,
        size: f64,
        bold: bool,
        /// Rotated a quarter turn anticlockwise, reading upwards.
        vertical: bool,
        text: String,
    },
}

/// A figure laid out: its size and its items, in drawing order.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub items: Vec<Item>,
}

/// Lays `fig` out; see the module comment.
pub fn scene(fig: &Figure) -> Scene {
    let mut items = vec![Item::Rect {
        class: "figure",
        r: Rect {
            x: 0.0,
            y: 0.0,
            w: WIDTH,
            h: HEIGHT,
        },
        fill: Some(WHITE),
        opacity: 1.0,
        stroke: None,
    }];
    for ax in &fig.axes {
        axes_items(ax, &mut items);
    }
    Scene {
        width: WIDTH,
        height: HEIGHT,
        items,
    }
}

/// The plot box of an axes in figure pixels: MATLAB's default axes position
/// `[0.13 0.11 0.775 0.815]` for the whole figure, and within a subplot's
/// cell the same margins, each at most a fixed share of the cell, so a
/// small cell keeps a plot box. `axis square` shrinks it to a square about
/// its centre.
fn inner_rect(ax: &Axes) -> Rect {
    let p = ax.place;
    let (cx, cy) = (p.x0 * WIDTH, p.y0 * HEIGHT);
    let (cw, ch) = ((p.x1 - p.x0) * WIDTH, (p.y1 - p.y0) * HEIGHT);
    let left = (0.13 * WIDTH).min(0.25 * cw);
    let right = (0.095 * WIDTH).min(0.1 * cw);
    let bottom = (0.11 * HEIGHT).min(0.3 * ch);
    let top = (0.075 * HEIGHT).min(0.2 * ch);
    let mut r = Rect {
        x: cx + left,
        y: cy + top,
        w: (cw - left - right).max(1.0),
        h: (ch - top - bottom).max(1.0),
    };
    if ax.square {
        let s = r.w.min(r.h);
        r = Rect {
            x: r.x + (r.w - s) / 2.0,
            y: r.y + (r.h - s) / 2.0,
            w: s,
            h: s,
        };
    }
    r
}

/// A tick step: `m` (1, 2 or 5) times ten to the `e`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Step {
    m: i64,
    e: i32,
}

impl Step {
    /// The value of the `n`th multiple, computed so that a decimal step
    /// lands on its decimal: `3 * 0.1` is `3 / 10`, which is `0.3`.
    fn value(self, n: i64) -> f64 {
        let q = (n * self.m) as f64;
        if self.e >= 0 {
            q * 10f64.powi(self.e)
        } else {
            q / 10f64.powi(-self.e)
        }
    }

    fn size(self) -> f64 {
        self.value(1)
    }

    /// The label of the `n`th multiple, from its digits; see the module
    /// comment.
    fn label(self, n: i64) -> String {
        let q = n * self.m;
        if q == 0 {
            return "0".to_string();
        }
        let digits = q.unsigned_abs().to_string();
        let body = if self.e >= 0 {
            let e = self.e as usize;
            if digits.len() + e > 10 {
                return fmt_g(self.value(n), 6);
            }
            digits + &"0".repeat(e)
        } else {
            let k = (-self.e) as usize;
            if k > 10 {
                return fmt_g(self.value(n), 6);
            }
            let padded = if digits.len() <= k {
                "0".repeat(k + 1 - digits.len()) + &digits
            } else {
                digits
            };
            let (int, frac) = padded.split_at(padded.len() - k);
            let frac = frac.trim_end_matches('0');
            if frac.is_empty() {
                int.to_string()
            } else {
                format!("{}.{}", int, frac)
            }
        };
        if body.len() > 10 {
            return fmt_g(self.value(n), 6);
        }
        if q < 0 { format!("-{}", body) } else { body }
    }

    /// The multiples of the step from `lo` to `hi`, outwards when `out`.
    fn range(self, lo: f64, hi: f64, out: bool) -> (i64, i64) {
        let s = self.size();
        let (a, b) = (lo / s, hi / s);
        if out {
            ((a + 1e-9).floor() as i64, (b - 1e-9).ceil() as i64)
        } else {
            ((a - 1e-9).ceil() as i64, (b + 1e-9).floor() as i64)
        }
    }
}

/// The smallest step of the tick rule for `lo..hi` in at most `max`
/// intervals, or `None` when the range is empty, not finite, or too far
/// from zero for its multiples to count exactly.
fn step_for(lo: f64, hi: f64, max: usize, out: bool) -> Option<Step> {
    let range = hi - lo;
    if !(range.is_finite() && range > 0.0) {
        return None;
    }
    let e0 = (range / max as f64).log10().floor() as i32;
    for e in e0..e0 + 3 {
        if !(-300..=300).contains(&e) {
            return None;
        }
        for m in [1, 2, 5] {
            let st = Step { m, e };
            let s = st.size();
            if (lo / s).abs() > 1e15 || (hi / s).abs() > 1e15 {
                return None;
            }
            let (a, b) = st.range(lo, hi, out);
            if b - a <= max as i64 {
                return Some(st);
            }
        }
    }
    None
}

/// One axis's limits and ticks.
#[derive(Clone, Debug, PartialEq)]
struct Limits {
    lo: f64,
    hi: f64,
    ticks: Vec<(f64, String)>,
}

/// The limits of one axis; see the module comment. `manual` are the
/// user's, an infinite end automatic; `span` is the data's.
fn one_axis(manual: Option<(f64, f64)>, span: Span, tight: bool, max: usize) -> Limits {
    let (mut dlo, mut dhi) = if span.any {
        (span.lo, span.hi)
    } else {
        (0.0, 1.0)
    };
    if dlo == dhi {
        // Clamped, so a value at the largest double keeps finite limits.
        let d = 1f64.max(dlo.abs() * 1e-9);
        dlo = (dlo - d).max(-f64::MAX);
        dhi = (dhi + d).min(f64::MAX);
    }
    let auto = step_for(dlo, dhi, max, true);
    let (mut lo, mut hi) = match auto {
        Some(st) if !tight => {
            let (a, b) = st.range(dlo, dhi, true);
            // Widening past the largest double keeps the data's end.
            let (l, h) = (st.value(a), st.value(b));
            (
                if l.is_finite() { l } else { dlo },
                if h.is_finite() { h } else { dhi },
            )
        }
        _ => (dlo, dhi),
    };
    let mut fixed = tight;
    if let Some((a, b)) = manual {
        if a.is_finite() {
            lo = a;
            fixed = true;
        }
        if b.is_finite() {
            hi = b;
            fixed = true;
        }
        if lo >= hi {
            // One end fixed past the other, automatic, end: the automatic
            // end moves past it by the data's range, or by a sliver of the
            // fixed end when that range is lost in its magnitude, and stays
            // finite, so the limits always increase.
            let d = (dhi - dlo)
                .min(f64::MAX)
                .max(lo.abs().max(hi.abs()) * 1e-9)
                .max(f64::MIN_POSITIVE);
            if a.is_finite() {
                hi = (lo + d).min(f64::MAX);
                if hi <= lo {
                    lo = hi - d;
                }
            } else {
                lo = (hi - d).max(-f64::MAX);
                if lo >= hi {
                    hi = lo + d;
                }
            }
        }
    }
    Limits {
        lo,
        hi,
        ticks: ticks(lo, hi, max, !fixed, auto),
    }
}

/// The ticks from `lo` to `hi`: every multiple of the automatic step when
/// the limits were widened to it, else of the step for these limits; the
/// two ends alone when no step can be found.
fn ticks(lo: f64, hi: f64, max: usize, widened: bool, auto: Option<Step>) -> Vec<(f64, String)> {
    let step = if widened {
        auto
    } else {
        step_for(lo, hi, max, false)
    };
    match step {
        Some(st) => {
            let (a, b) = st.range(lo, hi, false);
            (a..=b)
                .take(64)
                .map(|n| (st.value(n), st.label(n)))
                .collect()
        }
        None => vec![(lo, fmt_g(lo, 6)), (hi, fmt_g(hi, 6))],
    }
}

/// Both axes' limits for the plot box `r`, `axis equal` applied.
fn axis_limits(ax: &Axes, r: Rect) -> (Limits, Limits) {
    let (mut xs, mut ys) = (Span::default(), Span::default());
    for s in &ax.series {
        s.extent(&mut xs, &mut ys);
    }
    let xmax = ((r.w / 50.0) as usize).clamp(2, 10);
    let ymax = ((r.h / 30.0) as usize).clamp(2, 10);
    let mut x = one_axis(ax.xlim, xs, ax.tight, xmax);
    let mut y = one_axis(ax.ylim, ys, ax.tight, ymax);
    if ax.equal {
        let ux = (x.hi - x.lo) / r.w;
        let uy = (y.hi - y.lo) / r.h;
        if ux.is_finite() && uy.is_finite() && ux > 0.0 && uy > 0.0 {
            // The centre on halves, as `Map::frac` computes, and the wider
            // limits kept only while they are finite: near the largest
            // double the axis stays as it was rather than overflowing.
            let widen = |l: &mut Limits, unit: f64, len: f64, max: usize| {
                let (c, half) = (l.lo * 0.5 + l.hi * 0.5, unit * len / 2.0);
                let (lo, hi) = (c - half, c + half);
                if lo.is_finite() && hi.is_finite() && lo < hi {
                    *l = Limits {
                        lo,
                        hi,
                        ticks: ticks(lo, hi, max, false, None),
                    };
                }
            };
            if ux > uy {
                widen(&mut y, ux, r.h, ymax);
            } else if uy > ux {
                widen(&mut x, uy, r.w, xmax);
            }
        }
    }
    (x, y)
}

/// From data coordinates to figure pixels.
struct Map {
    r: Rect,
    x: (f64, f64),
    y: (f64, f64),
}

impl Map {
    /// Where `v` falls between `lo` and `hi`, as a fraction, computed on
    /// halves so that limits near the largest double do not overflow.
    fn frac(v: f64, (lo, hi): (f64, f64)) -> f64 {
        (v * 0.5 - lo * 0.5) / (hi * 0.5 - lo * 0.5)
    }

    fn px(&self, v: f64) -> f64 {
        clamp(self.r.x + Map::frac(v, self.x) * self.r.w)
    }

    fn py(&self, v: f64) -> f64 {
        clamp(self.r.y + (1.0 - Map::frac(v, self.y)) * self.r.h)
    }
}

fn clamp(v: f64) -> f64 {
    if v.is_nan() {
        return v;
    }
    v.clamp(-COORD_LIMIT, COORD_LIMIT)
}

/// A marker's outline at `(cx, cy)` with radius `r`.
pub fn marker_shape(m: Marker, cx: f64, cy: f64, r: f64) -> Shape {
    let poly = |pts: &[(f64, f64)]| {
        Shape::Polygon(pts.iter().map(|&(a, b)| (cx + a * r, cy + b * r)).collect())
    };
    let star = |points: usize, inner: f64| {
        let n = 2 * points;
        Shape::Polygon(
            (0..n)
                .map(|k| {
                    let t = std::f64::consts::PI * (k as f64) / points as f64
                        - std::f64::consts::FRAC_PI_2;
                    let rad = if k % 2 == 0 { r } else { r * inner };
                    (cx + rad * t.cos(), cy + rad * t.sin())
                })
                .collect(),
        )
    };
    let plus = [cx - r, cy, cx + r, cy];
    let vert = [cx, cy - r, cx, cy + r];
    let d = r * std::f64::consts::FRAC_1_SQRT_2;
    let diag1 = [cx - d, cy - d, cx + d, cy + d];
    let diag2 = [cx - d, cy + d, cx + d, cy - d];
    match m {
        Marker::Circle => Shape::Circle { cx, cy, r },
        Marker::Point => Shape::Circle {
            cx,
            cy,
            r: (r / 3.0).max(1.5),
        },
        Marker::Plus => Shape::Strokes(vec![plus, vert]),
        Marker::Cross => Shape::Strokes(vec![diag1, diag2]),
        Marker::Star => Shape::Strokes(vec![plus, vert, diag1, diag2]),
        Marker::Square => poly(&[(-0.9, -0.9), (0.9, -0.9), (0.9, 0.9), (-0.9, 0.9)]),
        Marker::Diamond => poly(&[(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)]),
        Marker::Up => poly(&[(0.0, -1.0), (1.0, 0.8), (-1.0, 0.8)]),
        Marker::Down => poly(&[(0.0, 1.0), (1.0, -0.8), (-1.0, -0.8)]),
        Marker::Right => poly(&[(1.0, 0.0), (-0.8, 1.0), (-0.8, -1.0)]),
        Marker::Left => poly(&[(-1.0, 0.0), (0.8, 1.0), (0.8, -1.0)]),
        Marker::Pentagram => star(5, 0.4),
        Marker::Hexagram => star(6, 0.55),
    }
}

/// A scatter point's radius in pixels for an area of `size` points
/// squared: its diameter is `sqrt(size)` points, at 96 pixels an inch.
pub fn scatter_radius(size: f64) -> f64 {
    (size.sqrt() * (4.0 / 3.0) / 2.0).min(MAX_MARKER_RADIUS)
}

fn text(class: &'static str, x: f64, y: f64, anchor: Anchor, size: f64, s: &str) -> Item {
    Item::Text {
        class,
        x,
        y,
        anchor,
        size,
        bold: false,
        vertical: false,
        text: s.to_string(),
    }
}

/// Everything one axes draws, in a `class="axes"` group.
fn axes_items(ax: &Axes, out: &mut Vec<Item>) {
    let r = inner_rect(ax);
    let (xl, yl) = axis_limits(ax, r);
    let map = Map {
        r,
        x: (xl.lo, xl.hi),
        y: (yl.lo, yl.hi),
    };
    let (left, top, right, bottom) = (r.x, r.y, r.x + r.w, r.y + r.h);
    out.push(Item::Begin {
        class: "axes",
        clip: None,
    });
    if ax.visible {
        out.push(Item::Rect {
            class: "box",
            r,
            fill: Some(WHITE),
            opacity: 1.0,
            stroke: None,
        });
        if ax.grid {
            for (v, _) in &xl.ticks {
                let x = map.px(*v);
                out.push(Item::Line {
                    class: "grid",
                    x1: x,
                    y1: top,
                    x2: x,
                    y2: bottom,
                    stroke: GRID_COLOR,
                    dash: Dash::Solid,
                });
            }
            for (v, _) in &yl.ticks {
                let y = map.py(*v);
                out.push(Item::Line {
                    class: "grid",
                    x1: left,
                    y1: y,
                    x2: right,
                    y2: y,
                    stroke: GRID_COLOR,
                    dash: Dash::Solid,
                });
            }
        }
    }
    out.push(Item::Begin {
        class: "data",
        clip: Some(r),
    });
    for s in &ax.series {
        series_items(s, &map, out);
    }
    out.push(Item::End);
    if ax.visible {
        out.push(Item::Rect {
            class: "frame",
            r,
            fill: None,
            opacity: 1.0,
            stroke: Some(AXIS_COLOR),
        });
        let tick = |x1, y1, x2, y2| Item::Line {
            class: "tick",
            x1,
            y1,
            x2,
            y2,
            stroke: AXIS_COLOR,
            dash: Dash::Solid,
        };
        for (v, label) in &xl.ticks {
            let x = map.px(*v);
            out.push(tick(x, bottom, x, bottom - 5.0));
            out.push(tick(x, top, x, top + 5.0));
            out.push(text("tick", x, bottom + 11.0, Anchor::Middle, 10.0, label));
        }
        let mut widest = 0;
        for (v, label) in &yl.ticks {
            let y = map.py(*v);
            out.push(tick(left, y, left + 5.0, y));
            out.push(tick(right, y, right - 5.0, y));
            out.push(text("tick", left - 5.0, y, Anchor::End, 10.0, label));
            widest = widest.max(label.chars().count());
        }
        if !ax.xlabel.is_empty() {
            out.push(text(
                "xlabel",
                left + r.w / 2.0,
                bottom + 27.0,
                Anchor::Middle,
                11.0,
                &ax.xlabel,
            ));
        }
        if !ax.ylabel.is_empty() {
            out.push(Item::Text {
                class: "ylabel",
                x: left - 16.0 - 6.0 * widest as f64,
                y: top + r.h / 2.0,
                anchor: Anchor::Middle,
                size: 11.0,
                bold: false,
                vertical: true,
                text: ax.ylabel.clone(),
            });
        }
    }
    if !ax.title.is_empty() {
        out.push(Item::Text {
            class: "title",
            x: left + r.w / 2.0,
            y: top - 11.0,
            anchor: Anchor::Middle,
            size: 11.0,
            bold: true,
            vertical: false,
            text: ax.title.clone(),
        });
    }
    if let Some(labels) = &ax.legend {
        legend_items(ax, r, labels, out);
    }
    out.push(Item::End);
}

/// One series' items. A line becomes one polyline per run of finite
/// points, so a line with no gap is exactly one `<polyline>`.
fn series_items(s: &Series, map: &Map, out: &mut Vec<Item>) {
    match s {
        Series::Line {
            x,
            y,
            color,
            dash,
            marker,
        } => {
            if let Some(d) = dash {
                let mut run = Vec::new();
                for (&a, &b) in x.iter().zip(y) {
                    if a.is_finite() && b.is_finite() {
                        run.push((map.px(a), map.py(b)));
                    } else if !run.is_empty() {
                        out.push(Item::Polyline {
                            points: std::mem::take(&mut run),
                            stroke: *color,
                            dash: *d,
                        });
                    }
                }
                if !run.is_empty() {
                    out.push(Item::Polyline {
                        points: run,
                        stroke: *color,
                        dash: *d,
                    });
                }
            }
            if let Some(m) = marker {
                for (&a, &b) in x.iter().zip(y) {
                    if a.is_finite() && b.is_finite() {
                        out.push(Item::Marker {
                            shape: marker_shape(*m, map.px(a), map.py(b), MARKER_RADIUS),
                            stroke: *color,
                            fill: (*m == Marker::Point).then_some(*color),
                            sample: false,
                        });
                    }
                }
            }
        }
        Series::Scatter {
            x,
            y,
            sizes,
            color,
            filled,
        } => {
            for (k, (&a, &b)) in x.iter().zip(y).enumerate() {
                let size = if sizes.len() == 1 { sizes[0] } else { sizes[k] };
                if a.is_finite() && b.is_finite() {
                    out.push(Item::Marker {
                        shape: Shape::Circle {
                            cx: map.px(a),
                            cy: map.py(b),
                            r: scatter_radius(size),
                        },
                        stroke: *color,
                        fill: filled.then_some(*color),
                        sample: false,
                    });
                }
            }
        }
        Series::Bars {
            rects,
            color,
            opacity,
        } => {
            for b in rects {
                if !b.iter().all(|v| v.is_finite()) {
                    continue;
                }
                let (x0, x1) = (map.px(b[0]), map.px(b[1]));
                let (y0, y1) = (map.py(0.0), map.py(b[2]));
                out.push(Item::Rect {
                    class: "bar",
                    r: Rect {
                        x: x0.min(x1),
                        y: y0.min(y1),
                        w: (x1 - x0).abs(),
                        h: (y1 - y0).abs(),
                    },
                    fill: Some(*color),
                    opacity: *opacity,
                    stroke: Some(BLACK),
                });
            }
        }
    }
}

/// The legend, in the plot box's top right corner: one row per label that
/// has a series to name, each with a sample of it.
fn legend_items(ax: &Axes, r: Rect, labels: &[String], out: &mut Vec<Item>) {
    let n = labels.len().min(ax.series.len());
    if n == 0 {
        return;
    }
    let widest = labels[..n]
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0);
    let w = 42.0 + 6.0 * widest as f64;
    let h = 16.0 * n as f64 + 6.0;
    let (x, y) = (r.x + r.w - w - 8.0, r.y + 8.0);
    out.push(Item::Begin {
        class: "legend",
        clip: None,
    });
    out.push(Item::Rect {
        class: "legend",
        r: Rect { x, y, w, h },
        fill: Some(WHITE),
        opacity: 1.0,
        stroke: Some(AXIS_COLOR),
    });
    for (k, (s, label)) in ax.series.iter().zip(labels).take(n).enumerate() {
        let yk = y + 11.0 + 16.0 * k as f64;
        match s {
            Series::Line {
                color,
                dash,
                marker,
                ..
            } => {
                if let Some(d) = dash {
                    out.push(Item::Line {
                        class: "sample",
                        x1: x + 6.0,
                        y1: yk,
                        x2: x + 32.0,
                        y2: yk,
                        stroke: *color,
                        dash: *d,
                    });
                }
                if let Some(m) = marker {
                    out.push(Item::Marker {
                        shape: marker_shape(*m, x + 19.0, yk, MARKER_RADIUS),
                        stroke: *color,
                        fill: (*m == Marker::Point).then_some(*color),
                        sample: true,
                    });
                }
            }
            Series::Scatter { color, filled, .. } => out.push(Item::Marker {
                shape: Shape::Circle {
                    cx: x + 19.0,
                    cy: yk,
                    r: MARKER_RADIUS,
                },
                stroke: *color,
                fill: filled.then_some(*color),
                sample: true,
            }),
            Series::Bars { color, opacity, .. } => out.push(Item::Rect {
                class: "sample",
                r: Rect {
                    x: x + 10.0,
                    y: yk - 5.0,
                    w: 18.0,
                    h: 10.0,
                },
                fill: Some(*color),
                opacity: *opacity,
                stroke: Some(BLACK),
            }),
        }
        out.push(text("legend", x + 38.0, yk, Anchor::Start, 10.0, label));
    }
    out.push(Item::End);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(lo: f64, hi: f64, max: usize) -> (f64, f64, Vec<String>) {
        let span = Span { lo, hi, any: true };
        let l = one_axis(None, span, false, max);
        (l.lo, l.hi, l.ticks.into_iter().map(|t| t.1).collect())
    }

    #[test]
    fn line_specs_parse_each_part_once_in_any_order() {
        let s = parse_line_spec("r--").unwrap();
        assert_eq!(s.color, Some(Color(255, 0, 0)));
        assert_eq!(s.dash, Some(Dash::Dashed));
        assert_eq!(s.marker, None);
        let s = parse_line_spec("o-.k").unwrap();
        assert_eq!(s.marker, Some(Marker::Circle));
        assert_eq!(s.dash, Some(Dash::DashDot));
        assert_eq!(s.color, Some(BLACK));
        assert_eq!(parse_line_spec(":").unwrap().dash, Some(Dash::Dotted));
        assert_eq!(parse_line_spec("-").unwrap().dash, Some(Dash::Solid));
        // A '.' after '-' is the dash-dot style; before it, a marker.
        let s = parse_line_spec(".-").unwrap();
        assert_eq!((s.marker, s.dash), (Some(Marker::Point), Some(Dash::Solid)));
        assert_eq!(parse_line_spec("").unwrap(), LineSpec::default());
        for bad in ["rr", "--:", "q", "LineWidth", "r -", "oo"] {
            assert_eq!(parse_line_spec(bad), None, "{bad}");
        }
    }

    #[test]
    fn ticks_follow_the_rule_and_label_their_digits() {
        // [1, 9] in ten intervals: every integer, 9 included.
        let (lo, hi, t) = labels(1.0, 9.0, 10);
        assert_eq!((lo, hi), (1.0, 9.0));
        assert_eq!(t, ["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
        // [1, 3] in eight: halves.
        let (_, _, t) = labels(1.0, 3.0, 8);
        assert_eq!(t, ["1", "1.5", "2", "2.5", "3"]);
        // Automatic limits widen to the step.
        let (lo, hi, t) = labels(1.0, 1e5, 8);
        assert_eq!((lo, hi), (0.0, 1e5));
        assert_eq!(t[1], "20000");
        // Decimal steps land on their decimals.
        let (_, _, t) = labels(0.0, 0.3, 3);
        assert_eq!(t, ["0", "0.1", "0.2", "0.3"]);
        let (_, _, t) = labels(-2.0, 2.0, 4);
        assert_eq!(t, ["-2", "-1", "0", "1", "2"]);
        // A long label is written in %g form.
        let (_, _, t) = labels(0.0, 3e12, 3);
        assert_eq!(t[1], "1e+12");
        // A single value gets a range around it.
        let (lo, hi, _) = labels(5.0, 5.0, 10);
        assert!(lo < 5.0 && hi > 5.0);
        // An overflowing range falls back to its two ends.
        let (lo, hi, t) = labels(-1e308, 1e308, 10);
        assert_eq!((lo, hi, t.len()), (-1e308, 1e308, 2));
    }

    #[test]
    fn manual_limits_keep_their_ends_and_tick_inside() {
        let span = Span {
            lo: 1.0,
            hi: 3.0,
            any: true,
        };
        let l = one_axis(Some((0.0, 4.0)), span, false, 8);
        assert_eq!((l.lo, l.hi), (0.0, 4.0));
        assert_eq!(l.ticks.first().unwrap().0, 0.0);
        assert_eq!(l.ticks.last().unwrap().0, 4.0);
        // An infinite end is automatic.
        let l = one_axis(Some((f64::NEG_INFINITY, 10.0)), span, false, 8);
        assert_eq!((l.lo, l.hi), (1.0, 10.0));
        // Tight limits are the data's.
        let l = one_axis(None, span, true, 8);
        assert_eq!((l.lo, l.hi), (1.0, 3.0));
    }

    #[test]
    fn figures_number_from_one_and_reuse_the_lowest_free() {
        let mut f = Figures::default();
        assert_eq!(f.current(), None);
        assert_eq!(f.gcf(), 1);
        assert_eq!(f.create(), 2);
        f.select(5);
        assert_eq!(f.current(), Some(5));
        assert!(f.close(1));
        assert!(!f.close(1));
        assert_eq!(f.create(), 1);
        assert_eq!(f.numbers(), [1, 2, 5]);
        // Closing the current figure makes the one current before it
        // current again.
        f.close(1);
        assert_eq!(f.current(), Some(5));
        assert_eq!(f.take_changed(), [2, 5]);
        assert!(f.take_changed().is_empty());
        f.close_all();
        assert!(f.numbers().is_empty());
        assert_eq!(f.create(), 1);
    }

    #[test]
    fn subplots_select_their_own_place_and_delete_what_they_overlap() {
        let mut fig = Figure::default();
        fig.gca();
        assert_eq!(fig.axes.len(), 1);
        fig.subplot(Place::cells(2, 1, 0, 0));
        // The whole-figure axes overlapped the new cell and went.
        assert_eq!(fig.axes.len(), 1);
        fig.subplot(Place::cells(2, 1, 1, 1));
        assert_eq!(fig.axes.len(), 2);
        fig.subplot(Place::cells(2, 1, 0, 0));
        assert_eq!((fig.axes.len(), fig.current), (2, Some(0)));
        // subplot(1, 1, 1) is the whole figure.
        assert_eq!(Place::cells(1, 1, 0, 0), Place::WHOLE);
        let p = Place::cells(2, 3, 1, 4);
        assert_eq!((p.x0, p.x1, p.y0, p.y1), (1.0 / 3.0, 2.0 / 3.0, 0.0, 1.0));
    }

    #[test]
    fn hold_off_replaces_and_the_budget_is_judged_first() {
        let mut fig = Figure::default();
        let line = |n: usize| Series::Line {
            x: vec![0.0; n],
            y: vec![0.0; n],
            color: BLACK,
            dash: Some(Dash::Solid),
            marker: None,
        };
        fig.add_series(3, |_| vec![line(3)]).unwrap();
        fig.gca().title = "t".into();
        fig.add_series(4, |_| vec![line(4)]).unwrap();
        assert_eq!((fig.points(), fig.gca().series.len()), (4, 1));
        assert_eq!(fig.gca().title, "");
        fig.gca().hold = true;
        fig.add_series(2, |_| vec![line(2)]).unwrap();
        assert_eq!((fig.points(), fig.gca().series.len()), (6, 2));
        let e = fig.add_series(MAX_POINTS, |_| unreachable!()).unwrap_err();
        assert!(e.msg.contains("16777216"), "{}", e.msg);
        assert_eq!(fig.points(), 6);
        // Colours follow the order and carry on under hold.
        let a = fig.gca();
        assert_eq!(a.next_color(), COLOR_ORDER[0]);
        assert_eq!(a.next_color(), COLOR_ORDER[1]);
    }

    /// The budget's weights (see `MAX_POINTS`): whatever a series draws,
    /// its SVG is at most 16 bytes a point it counts, so the largest figure
    /// the budget admits writes about 256 MiB of SVG, not the gigabytes a
    /// count of one a marker, a circle or a bar allowed.
    #[test]
    fn every_kind_writes_at_most_16_bytes_of_svg_a_point_it_counts() {
        let weights: Vec<usize> = "o.+x*sd^v><ph"
            .chars()
            .map(|c| {
                parse_line_spec(&c.to_string())
                    .unwrap()
                    .marker
                    .unwrap()
                    .weight()
            })
            .collect();
        assert_eq!(weights, [6, 6, 9, 9, 13, 9, 9, 8, 8, 8, 8, 15, 17]);
        assert_eq!((SCATTER, BAR, RUN, LABEL), (6, 9, 7, 9));
        let n = 2000;
        let xs: Vec<f64> = (0..n).map(|k| 1.0 + k as f64 / n as f64).collect();
        let ys: Vec<f64> = xs.iter().map(|x| x + 0.123).collect();
        let gaps: Vec<f64> = ys
            .iter()
            .enumerate()
            .map(|(k, &y)| if k % 2 == 1 { f64::NAN } else { y })
            .collect();
        let line = |y: &[f64], dash, marker| Series::Line {
            x: xs.clone(),
            y: y.to_vec(),
            color: COLOR_ORDER[0],
            dash,
            marker,
        };
        let mut kinds = vec![
            line(&ys, Some(Dash::Solid), None),
            // A run of one point each: a `<polyline>` element per point.
            line(&gaps, Some(Dash::DashDot), None),
            Series::Scatter {
                x: xs.clone(),
                y: ys.clone(),
                sizes: vec![2500.0],
                color: COLOR_ORDER[1],
                filled: true,
            },
            Series::Bars {
                rects: xs.iter().map(|&x| [x, x + 0.0004, x]).collect(),
                color: COLOR_ORDER[2],
                opacity: 0.6,
            },
        ];
        for c in "o.+x*sd^v><ph".chars() {
            let m = parse_line_spec(&format!("{c}-.")).unwrap().marker;
            kinds.push(line(&ys, Some(Dash::DashDot), m));
        }
        let svg_len = |fig: &Figure| crate::plot::svg::render(&scene(fig)).len();
        for s in kinds {
            let cost = s.cost();
            // Limits fixed, so the empty figure has the same ticks.
            let mut empty = Figure::default();
            let ax = empty.gca();
            (ax.xlim, ax.ylim) = (Some((0.5, 3.5)), Some((0.5, 3.5)));
            let base = svg_len(&empty);
            let mut fig = empty.clone();
            fig.gca().hold = true;
            fig.add_series(cost, |_| vec![s.clone()]).unwrap();
            fig.gca().legend = Some(vec!["data1234567".into()]);
            let bytes = svg_len(&fig) - base;
            assert!(bytes <= 16 * cost, "{bytes} bytes for {cost}: {s:?}");
            // And not far under it, or the weight would waste the budget.
            assert!(bytes * 2 > 16 * cost, "{bytes} bytes for {cost}");
            assert_eq!(fig.points(), cost);
        }
    }

    #[test]
    fn a_refused_plot_makes_no_figure_and_leaves_the_old_one_as_it_was() {
        let mut f = Figures::default();
        let e = f
            .add_series(MAX_POINTS + 1, |_| unreachable!())
            .unwrap_err();
        assert!(e.msg.contains("16777216"), "{}", e.msg);
        assert!(f.numbers().is_empty() && f.take_changed().is_empty());
        f.add_series(10, |_| Vec::new()).unwrap();
        f.take_changed();
        // With hold off, the axes' own points are replaced, so a call may
        // take the whole budget; under hold they are kept.
        f.add_series(MAX_POINTS, |_| Vec::new()).unwrap();
        f.current_mut().gca().hold = true;
        f.take_changed();
        assert!(f.add_series(1, |_| unreachable!()).is_err());
        assert!(f.take_changed().is_empty());
        let n = f.current().unwrap();
        assert_eq!(f.get(n).unwrap().points(), MAX_POINTS);
        // The line costs of the rule: a point each, a run and a legend
        // sample, a label.
        assert_eq!(line_cost(3, 1, true, None), 3 + 2 * RUN + LABEL);
        assert_eq!(
            line_cost(3, 0, false, Some(Marker::Hexagram)),
            3 + 17 * 4 + LABEL
        );
        assert_eq!(
            runs(
                [1.0, f64::NAN, 2.0, 3.0, f64::INFINITY]
                    .iter()
                    .map(|&v| (1.0, v))
            ),
            2
        );
        assert_eq!(runs(std::iter::empty()), 0);
    }

    #[test]
    fn a_line_with_no_gap_is_one_polyline_and_a_gap_splits_it() {
        let mut fig = Figure::default();
        let mk = |y: Vec<f64>| Series::Line {
            x: (1..=y.len()).map(|k| k as f64).collect(),
            y,
            color: BLACK,
            dash: Some(Dash::Solid),
            marker: Some(Marker::Circle),
        };
        fig.add_series(3, |_| vec![mk(vec![1.0, 4.0, 9.0])])
            .unwrap();
        let polys = |s: &Scene| {
            s.items
                .iter()
                .filter(|i| matches!(i, Item::Polyline { .. }))
                .count()
        };
        let marks = |s: &Scene| {
            s.items
                .iter()
                .filter(|i| matches!(i, Item::Marker { .. }))
                .count()
        };
        let s = scene(&fig);
        assert_eq!((polys(&s), marks(&s)), (1, 3));
        fig.add_series(3, |_| vec![mk(vec![1.0, f64::NAN, 9.0])])
            .unwrap();
        let s = scene(&fig);
        assert_eq!((polys(&s), marks(&s)), (2, 2));
        // Far-away data is clamped, not written out in full.
        fig.add_series(2, |_| vec![mk(vec![1.0, 1e300])]).unwrap();
        fig.gca().ylim = Some((0.0, 2.0));
        for i in scene(&fig).items {
            if let Item::Polyline { points, .. } = i {
                assert!(points.iter().all(|p| p.1.abs() <= COORD_LIMIT));
            }
        }
    }

    /// Every coordinate a scene holds is a number, whatever the data and
    /// the limits: no `NaN` from limits that overflowed or collapsed.
    fn assert_finite(fig: &Figure) {
        for i in scene(fig).items {
            let ok = match &i {
                Item::Line { x1, y1, x2, y2, .. } => [x1, y1, x2, y2].iter().all(|v| v.is_finite()),
                Item::Polyline { points, .. } => {
                    points.iter().all(|p| p.0.is_finite() && p.1.is_finite())
                }
                Item::Text { x, y, .. } => x.is_finite() && y.is_finite(),
                Item::Rect { r, .. } => [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()),
                _ => true,
            };
            assert!(ok, "{i:?}");
        }
    }

    #[test]
    fn limits_stay_finite_and_increasing_at_the_ends_of_the_doubles() {
        let mk = |x: Vec<f64>, y: Vec<f64>| Series::Line {
            x,
            y,
            color: BLACK,
            dash: Some(Dash::Solid),
            marker: None,
        };
        let limits = |fig: &mut Figure| {
            let (a, b, c, d) = fig.gca().limits();
            assert!(
                [a, b, c, d].iter().all(|v| v.is_finite()),
                "{a} {b} {c} {d}"
            );
            assert!(a < b && c < d, "{a} {b} {c} {d}");
        };
        // A constant at the largest double: widening it used to overflow.
        let mut fig = Figure::default();
        fig.add_series(2, |_| vec![mk(vec![1.0, 2.0], vec![f64::MAX, f64::MAX])])
            .unwrap();
        limits(&mut fig);
        assert_finite(&fig);
        // A manual end past the data by more than the data's range can
        // hold: the automatic end used to come out equal to it.
        for (lo, hi) in [
            (1e300, f64::INFINITY),
            (f64::MAX, f64::INFINITY),
            (f64::NEG_INFINITY, -f64::MAX),
        ] {
            let mut fig = Figure::default();
            fig.add_series(3, |_| vec![mk(vec![1.0, 2.0, 3.0], vec![1.0, 2.0, 3.0])])
                .unwrap();
            fig.gca().xlim = Some((lo, hi));
            limits(&mut fig);
            assert_finite(&fig);
        }
        // axis equal next to the largest double keeps finite limits.
        let mut fig = Figure::default();
        fig.add_series(2, |_| vec![mk(vec![0.0, f64::MAX], vec![0.0, 1.0])])
            .unwrap();
        fig.gca().equal = true;
        limits(&mut fig);
        assert_finite(&fig);
    }
}
