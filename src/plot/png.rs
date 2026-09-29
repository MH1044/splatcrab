//! PNG output (cycle 12): a rasterizer of the figure's own, a bitmap font,
//! and a store-only zlib encoder, with no crate and no font file.
//!
//! **The rasterizer** draws a [`Scene`] into an RGB buffer at a scale of
//! `dpi / 96` pixels per figure pixel: rectangles filled (with their
//! opacity) and outlined, lines and polylines stroked with their dash
//! pattern carried along the line, circles filled or outlined, marker
//! outlines, and text in the bitmap font, all clipped to the group they sit
//! in. There is no anti-aliasing. Every segment is clipped to its clip box
//! before it is walked, so each costs at most the box's diagonal whatever
//! its coordinates, and a figure costs time linear in its points. A line
//! wider than 1.5 pixels is the union of squares of its width stepped
//! along it, drawn one run across the line a pixel along it, so its cost
//! is the pixels it covers.
//!
//! **The font** is 5x7 pixels in a 6x8 cell, one glyph for each printable
//! ASCII character and a hollow box for any other, scaled by a whole number
//! for large text or a high resolution. Bold is drawn twice, one pixel
//! apart. A vertical text reads upwards.
//!
//! **The encoder** writes the signature, `IHDR` (8-bit RGB, no interlace),
//! one `IDAT` and `IEND`, each chunk with its CRC-32. The image data is a
//! zlib stream of stored (uncompressed) deflate blocks of at most 65,535
//! bytes, each scanline led by filter type 0, closed by its Adler-32.

use super::figure::{Anchor, Color, Dash, HEIGHT, Item, Rect, Scene, Shape, WIDTH};
use crate::builtins::args::check_shape;
use crate::error::R;

/// The pixel size of a figure drawn at `scale` pixels per figure pixel,
/// judged by `check_shape` before anything is allocated: `-r100000` is
/// refused here.
pub fn pixel_size(scale: f64) -> R<(usize, usize)> {
    let w = (WIDTH * scale).round().max(1.0);
    let h = (HEIGHT * scale).round().max(1.0);
    let (rows, cols) = check_shape(h, w)?;
    Ok((cols, rows))
}

/// The PNG file of `scene` drawn `w` by `h` pixels, a size from
/// [`pixel_size`].
pub fn render(scene: &Scene, w: usize, h: usize) -> Vec<u8> {
    let mut c = Canvas::new(w, h, w as f64 / scene.width);
    c.draw(scene);
    encode(w, h, &c.rgb)
}

// ---- the font ------------------------------------------------------------

/// Rows of each glyph, top first, bit 4 the leftmost column, for the
/// characters from space (32) to tilde (126) in order.
#[rustfmt::skip]
const FONT: [[u8; 7]; 95] = [
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // space
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100], // !
    [0b01010, 0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000], // "
    [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010], // #
    [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100], // $
    [0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011], // %
    [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101], // &
    [0b01100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000], // '
    [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010], // (
    [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000], // )
    [0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000], // *
    [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000], // +
    [0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000], // ,
    [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000], // -
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100], // .
    [0b00000, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000], // /
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111], // 2
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
    [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000], // :
    [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b00100, 0b01000], // ;
    [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010], // <
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000], // =
    [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000], // >
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100], // ?
    [0b01110, 0b10001, 0b00001, 0b01101, 0b10101, 0b10101, 0b01110], // @
    [0b01110, 0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001], // A
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110], // B
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110], // C
    [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100], // D
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111], // E
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000], // F
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111], // G
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // H
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // I
    [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100], // J
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001], // K
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111], // L
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001], // M
    [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001], // N
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // O
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000], // P
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101], // Q
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001], // R
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110], // S
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // T
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // U
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100], // V
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010], // W
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001], // X
    [0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100], // Y
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111], // Z
    [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110], // [
    [0b00000, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00000], // backslash
    [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110], // ]
    [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000], // ^
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111], // _
    [0b01000, 0b00100, 0b00010, 0b00000, 0b00000, 0b00000, 0b00000], // `
    [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111], // a
    [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110], // b
    [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110], // c
    [0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111], // d
    [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110], // e
    [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000], // f
    [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110], // g
    [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001], // h
    [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110], // i
    [0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100], // j
    [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010], // k
    [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // l
    [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001], // m
    [0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001], // n
    [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110], // o
    [0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000], // p
    [0b00000, 0b00000, 0b01101, 0b10011, 0b01111, 0b00001, 0b00001], // q
    [0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000], // r
    [0b00000, 0b00000, 0b01110, 0b10000, 0b01110, 0b00001, 0b11110], // s
    [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110], // t
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101], // u
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100], // v
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010], // w
    [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001], // x
    [0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110], // y
    [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111], // z
    [0b00010, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00010], // {
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // |
    [0b01000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01000], // }
    [0b00000, 0b00000, 0b01000, 0b10101, 0b00010, 0b00000, 0b00000], // ~
];

/// The glyph for a character without one of its own.
const BOX: [u8; 7] = [
    0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
];

/// The rows of `c`'s glyph.
pub fn glyph(c: char) -> &'static [u8; 7] {
    match c as u32 {
        k @ 32..=126 => &FONT[(k - 32) as usize],
        _ => &BOX,
    }
}

// ---- the rasterizer --------------------------------------------------------

/// A half-open box of whole pixels, `x0..x1` by `y0..y1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Clip {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

/// An RGB image being drawn, with its stack of clip boxes.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
    /// Canvas pixels per figure pixel.
    scale: f64,
    clips: Vec<Clip>,
}

impl Canvas {
    /// A white canvas.
    pub fn new(w: usize, h: usize, scale: f64) -> Canvas {
        Canvas {
            w,
            h,
            rgb: vec![255; w * h * 3],
            scale,
            clips: vec![Clip {
                x0: 0,
                y0: 0,
                x1: w as i64,
                y1: h as i64,
            }],
        }
    }

    fn clip(&self) -> Clip {
        *self.clips.last().expect("the whole canvas is never popped")
    }

    /// The colour at `(x, y)`, for tests.
    pub fn at(&self, x: usize, y: usize) -> Color {
        let k = 3 * (y * self.w + x);
        Color(self.rgb[k], self.rgb[k + 1], self.rgb[k + 2])
    }

    fn put(&mut self, x: i64, y: i64, c: Color, alpha: f64) {
        let b = self.clip();
        if x < b.x0 || x >= b.x1 || y < b.y0 || y >= b.y1 {
            return;
        }
        let k = 3 * (y as usize * self.w + x as usize);
        if alpha >= 1.0 {
            self.rgb[k] = c.0;
            self.rgb[k + 1] = c.1;
            self.rgb[k + 2] = c.2;
        } else {
            let mix =
                |old: u8, new: u8| (old as f64 * (1.0 - alpha) + new as f64 * alpha).round() as u8;
            self.rgb[k] = mix(self.rgb[k], c.0);
            self.rgb[k + 1] = mix(self.rgb[k + 1], c.1);
            self.rgb[k + 2] = mix(self.rgb[k + 2], c.2);
        }
    }

    /// Fills every pixel whose centre lies in `x0..x1` by `y0..y1`, in
    /// canvas pixels.
    fn fill(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, c: Color, alpha: f64) {
        let b = self.clip();
        let lo = |v: f64, min: i64| ((v - 0.5).ceil() as i64).max(min);
        let hi = |v: f64, max: i64| ((v - 0.5).ceil() as i64).min(max);
        let (xa, xb) = (lo(x0, b.x0), hi(x1, b.x1));
        let (ya, yb) = (lo(y0, b.y0), hi(y1, b.y1));
        for y in ya..yb {
            for x in xa..xb {
                self.put(x, y, c, alpha);
            }
        }
    }

    /// The part of the segment inside the clip box, grown by `pad`, as the
    /// parameters `t0..t1` of `p0 + t (p1 - p0)`; `None` when it misses
    /// (Liang and Barsky's clip).
    fn clip_segment(&self, p0: (f64, f64), p1: (f64, f64), pad: f64) -> Option<(f64, f64)> {
        let b = self.clip();
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let mut t = (0.0f64, 1.0f64);
        let edges = [
            (-dx, p0.0 - (b.x0 as f64 - pad)),
            (dx, (b.x1 as f64 + pad) - p0.0),
            (-dy, p0.1 - (b.y0 as f64 - pad)),
            (dy, (b.y1 as f64 + pad) - p0.1),
        ];
        for (p, q) in edges {
            if p == 0.0 {
                if q < 0.0 {
                    return None;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    t.0 = t.0.max(r);
                } else {
                    t.1 = t.1.min(r);
                }
            }
        }
        (t.0 <= t.1).then_some(t)
    }

    /// A solid segment, walked one pixel at a time along its longer side:
    /// the `n + 1` steps `p0 + (k / n) (p1 - p0)`, each a pixel when the
    /// line is at most 1.5 pixels wide, else a square of side `width`
    /// centred on it, drawn by [`Canvas::thick`].
    fn solid(&mut self, p0: (f64, f64), p1: (f64, f64), c: Color, width: f64) {
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let n = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
        let step = |k: usize| {
            let t = k as f64 / n as f64;
            (p0.0 + t * dx, p0.1 + t * dy)
        };
        if width <= 1.5 {
            for k in 0..=n {
                let (x, y) = step(k);
                self.put(x.floor() as i64, y.floor() as i64, c, 1.0);
            }
        } else if dx.abs() >= dy.abs() {
            self.thick(n, step, false, c, width);
        } else {
            let swapped = |k: usize| {
                let (x, y) = step(k);
                (y, x)
            };
            self.thick(n, swapped, true, c, width);
        }
    }

    /// The union of the squares of side `width` centred on the steps
    /// `step(0..=n)`, each covering the pixels whose centres lie within
    /// half the width of it as [`Canvas::fill`] counts them, drawn as one
    /// run across the line for each pixel along it: the same pixels as
    /// filling every square, at a cost of the pixels covered rather than
    /// the steps times the square's area. The steps are `(along, across)`
    /// with `along` the longer side, x unless `swap`; consecutive steps are
    /// at most a pixel apart on both, so the squares covering a pixel
    /// along are consecutive steps, and their runs across join into one.
    fn thick(
        &mut self,
        n: usize,
        step: impl Fn(usize) -> (f64, f64),
        swap: bool,
        c: Color,
        width: f64,
    ) {
        let r = width / 2.0;
        // The pixels `[lo, hi)` a square centred on `v` covers on one axis.
        let span = |v: f64| (((v - r) - 0.5).ceil() as i64, ((v + r) - 0.5).ceil() as i64);
        // Step `j`'s spans along and across, the steps taken in the order
        // `along` increases, so both ends of the spans along only grow.
        let back = step(n).0 < step(0).0;
        let spans = |j: usize| {
            let (a, b) = step(if back { n - j } else { j });
            (span(a), span(b))
        };
        let b = self.clip();
        let (along, across) = if swap {
            ((b.y0, b.y1), (b.x0, b.x1))
        } else {
            ((b.x0, b.x1), (b.y0, b.y1))
        };
        // The steps covering pixel `i` along are `lo..=hi`, their spans
        // kept, and the span of the step after `hi`.
        let (mut lo, mut hi) = (0, 0);
        let (mut lo_s, mut hi_s) = (spans(0), spans(0));
        let mut next = (n > 0).then(|| spans(1));
        let first = lo_s.0.0.max(along.0);
        let last = spans(n).0.1.min(along.1);
        for i in first..last {
            while let Some(s) = next.filter(|s| s.0.0 <= i) {
                hi += 1;
                hi_s = s;
                next = (hi < n).then(|| spans(hi + 1));
            }
            while lo_s.0.1 <= i && lo < n {
                lo += 1;
                lo_s = spans(lo);
            }
            if lo > hi || lo_s.0.1 <= i {
                continue;
            }
            let from = lo_s.1.0.min(hi_s.1.0).max(across.0);
            let to = lo_s.1.1.max(hi_s.1.1).min(across.1);
            self.run(i, from, to, swap, c);
        }
    }

    /// Pixels `from..to` across pixel `i` along, inside the clip box: a
    /// column of rows when `along` is x, a stretch of a row when `swap`.
    fn run(&mut self, i: i64, from: i64, to: i64, swap: bool, c: Color) {
        if from >= to {
            return;
        }
        let (i, from, to) = (i as usize, from as usize, to as usize);
        let rgb = [c.0, c.1, c.2];
        if swap {
            let row = &mut self.rgb[3 * (i * self.w + from)..3 * (i * self.w + to)];
            for px in row.chunks_exact_mut(3) {
                px.copy_from_slice(&rgb);
            }
        } else {
            for y in from..to {
                let k = 3 * (y * self.w + i);
                self.rgb[k..k + 3].copy_from_slice(&rgb);
            }
        }
    }

    /// A segment with the dash `pattern`, `phase` being how far along the
    /// pattern the segment starts; returns the phase at its end, so a
    /// polyline's dashes run on across its vertices.
    fn segment(
        &mut self,
        p0: (f64, f64),
        p1: (f64, f64),
        c: Color,
        width: f64,
        pattern: &[f64],
        phase: f64,
    ) -> f64 {
        if !(p0.0.is_finite() && p0.1.is_finite() && p1.0.is_finite() && p1.1.is_finite()) {
            return phase;
        }
        let len = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
        let period: f64 = pattern.iter().sum();
        let end_phase = if period > 0.0 {
            (phase + len) % period
        } else {
            0.0
        };
        let Some((t0, t1)) = self.clip_segment(p0, p1, width) else {
            return end_phase;
        };
        let at = |t: f64| (p0.0 + t * (p1.0 - p0.0), p0.1 + t * (p1.1 - p0.1));
        if period <= 0.0 || len == 0.0 {
            self.solid(at(t0), at(t1), c, width);
            return end_phase;
        }
        // Walk the pattern over the clipped part only.
        let (s0, s1) = (t0 * len, t1 * len);
        let mut pos = s0;
        let mut into = (phase + s0) % period;
        let mut piece = 0;
        while into >= pattern[piece] {
            into -= pattern[piece];
            piece = (piece + 1) % pattern.len();
        }
        while pos < s1 {
            let step = (pattern[piece] - into).min(s1 - pos);
            if piece % 2 == 0 {
                self.solid(at(pos / len), at((pos + step) / len), c, width);
            }
            pos += step;
            into = 0.0;
            piece = (piece + 1) % pattern.len();
        }
        end_phase
    }

    fn line(&mut self, p0: (f64, f64), p1: (f64, f64), c: Color, width: f64, dash: Dash) {
        let pattern = self.pattern(dash);
        self.segment(p0, p1, c, width, &pattern, 0.0);
    }

    fn pattern(&self, dash: Dash) -> Vec<f64> {
        dash.pattern().iter().map(|v| v * self.scale).collect()
    }

    fn circle(&mut self, cx: f64, cy: f64, r: f64, stroke: Color, fill: Option<Color>, width: f64) {
        let b = self.clip();
        if cx + r < b.x0 as f64
            || cx - r > b.x1 as f64
            || cy + r < b.y0 as f64
            || cy - r > b.y1 as f64
        {
            return;
        }
        if let Some(f) = fill {
            let (ya, yb) = (
                ((cy - r) - 0.5).ceil() as i64,
                ((cy + r) - 0.5).ceil() as i64,
            );
            for y in ya.max(b.y0)..yb.min(b.y1) {
                let dy = y as f64 + 0.5 - cy;
                let half = (r * r - dy * dy).max(0.0).sqrt();
                self.fill(cx - half, y as f64, cx + half, y as f64 + 1.0, f, 1.0);
            }
        }
        let n = ((2.0 * std::f64::consts::PI * r).ceil() as usize).max(8);
        let pt = |k: usize| {
            let t = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            (cx + r * t.cos(), cy + r * t.sin())
        };
        for k in 0..n {
            self.solid(pt(k), pt(k + 1), stroke, width);
        }
    }

    /// Text in the bitmap font, `(x, y)` its anchor and vertical middle.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        x: f64,
        y: f64,
        anchor: Anchor,
        k: i64,
        bold: bool,
        vertical: bool,
        s: &str,
    ) {
        let n = s.chars().count() as f64;
        let len = n * 6.0 * k as f64 - k as f64;
        let start = match anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => -len / 2.0,
            Anchor::End => -len,
        };
        let kf = k as f64;
        let top = -3.5 * kf;
        let c = super::figure::AXIS_COLOR;
        let passes = if bold { 2 } else { 1 };
        for (i, ch) in s.chars().enumerate() {
            let rows = glyph(ch);
            let along = start + (i as f64) * 6.0 * kf;
            // A character wholly off the canvas costs nothing more.
            let (bx, by, bw, bh) = if vertical {
                (x + top, y - along - 7.0 * kf, 8.0 * kf, 7.0 * kf)
            } else {
                (x + along, y + top, 7.0 * kf, 8.0 * kf)
            };
            if bx > self.w as f64 || by > self.h as f64 || bx + bw < 0.0 || by + bh < 0.0 {
                continue;
            }
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0b10000 >> col) == 0 {
                        continue;
                    }
                    for p in 0..passes {
                        let (u, v) = ((col + p) as f64 * k as f64, row as f64 * k as f64);
                        let (px, py) = if vertical {
                            (x + top + v, y - along - u - k as f64)
                        } else {
                            (x + along + u, y + top + v)
                        };
                        self.fill(px, py, px + k as f64, py + k as f64, c, 1.0);
                    }
                }
            }
        }
    }

    /// Draws every item of `scene`.
    pub fn draw(&mut self, scene: &Scene) {
        let s = self.scale;
        let p = |x: f64, y: f64| (x * s, y * s);
        let thin = s.max(1.0);
        for item in &scene.items {
            match item {
                Item::Begin { clip, .. } => {
                    let mut b = self.clip();
                    if let Some(r) = clip {
                        let (x0, y0) = p(r.x, r.y);
                        let (x1, y1) = p(r.x + r.w, r.y + r.h);
                        b = Clip {
                            x0: b.x0.max((x0 - 0.5).ceil() as i64),
                            y0: b.y0.max((y0 - 0.5).ceil() as i64),
                            x1: b.x1.min((x1 + 0.5).floor() as i64),
                            y1: b.y1.min((y1 + 0.5).floor() as i64),
                        };
                    }
                    self.clips.push(b);
                }
                Item::End => {
                    if self.clips.len() > 1 {
                        self.clips.pop();
                    }
                }
                Item::Rect {
                    r,
                    fill,
                    opacity,
                    stroke,
                    ..
                } => self.rect(r, *fill, *opacity, *stroke),
                Item::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke,
                    dash,
                    ..
                } => self.line(p(*x1, *y1), p(*x2, *y2), *stroke, thin, *dash),
                Item::Polyline {
                    points,
                    stroke,
                    dash,
                } => {
                    let pattern = self.pattern(*dash);
                    let mut phase = 0.0;
                    for w in points.windows(2) {
                        phase = self.segment(
                            p(w[0].0, w[0].1),
                            p(w[1].0, w[1].1),
                            *stroke,
                            thin,
                            &pattern,
                            phase,
                        );
                    }
                }
                Item::Marker {
                    shape,
                    stroke,
                    fill,
                    ..
                } => match shape {
                    Shape::Circle { cx, cy, r } => {
                        let (x, y) = p(*cx, *cy);
                        self.circle(x, y, r * s, *stroke, *fill, thin);
                    }
                    Shape::Polygon(pts) => {
                        for k in 0..pts.len() {
                            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                            self.line(p(a.0, a.1), p(b.0, b.1), *stroke, thin, Dash::Solid);
                        }
                    }
                    Shape::Strokes(lines) => {
                        for l in lines {
                            self.line(p(l[0], l[1]), p(l[2], l[3]), *stroke, thin, Dash::Solid);
                        }
                    }
                },
                Item::Text {
                    x,
                    y,
                    anchor,
                    size,
                    bold,
                    vertical,
                    text,
                    ..
                } => {
                    let k = ((size * s / 10.0).round() as i64).max(1);
                    let (x, y) = p(*x, *y);
                    self.text(x, y, *anchor, k, *bold, *vertical, text);
                }
            }
        }
    }

    fn rect(&mut self, r: &Rect, fill: Option<Color>, opacity: f64, stroke: Option<Color>) {
        let s = self.scale;
        let (x0, y0, x1, y1) = (r.x * s, r.y * s, (r.x + r.w) * s, (r.y + r.h) * s);
        if let Some(f) = fill {
            self.fill(x0, y0, x1, y1, f, opacity);
        }
        if let Some(c) = stroke {
            let w = (0.5 * s).max(1.0);
            for (a, b) in [
                ((x0, y0), (x1, y0)),
                ((x1, y0), (x1, y1)),
                ((x1, y1), (x0, y1)),
                ((x0, y1), (x0, y0)),
            ] {
                self.segment(a, b, c, w, &[], 0.0);
            }
        }
    }
}

// ---- the encoder -----------------------------------------------------------

/// The PNG file signature.
pub const SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// CRC-32 (ISO 3309, the polynomial 0xEDB88320 reflected), as PNG's chunks
/// carry it, from a table built at compile time.
const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
};

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c = CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32, as a zlib stream ends with it.
pub fn adler32(bytes: &[u8]) -> u32 {
    let mut a = Adler::default();
    a.update(bytes);
    a.value()
}

struct Adler {
    a: u32,
    b: u32,
}

impl Default for Adler {
    fn default() -> Adler {
        Adler { a: 1, b: 0 }
    }
}

impl Adler {
    fn update(&mut self, bytes: &[u8]) {
        // 5552 bytes is the most that can be summed before `b` could
        // overflow 32 bits, so the modulus is taken once per chunk.
        for chunk in bytes.chunks(5552) {
            for &x in chunk {
                self.a += x as u32;
                self.b += self.a;
            }
            self.a %= 65521;
            self.b %= 65521;
        }
    }

    fn value(&self) -> u32 {
        (self.b << 16) | self.a
    }
}

/// The largest stored deflate block.
const BLOCK: usize = 65535;

/// Writes a zlib stream of stored blocks into `out` as bytes are pushed.
struct Stored<'a> {
    out: &'a mut Vec<u8>,
    block: Vec<u8>,
    adler: Adler,
}

impl<'a> Stored<'a> {
    fn new(out: &'a mut Vec<u8>) -> Stored<'a> {
        // CMF 0x78: deflate, a 32K window; FLG 0x01 makes the pair a
        // multiple of 31, with no dictionary and the fastest level.
        out.extend_from_slice(&[0x78, 0x01]);
        Stored {
            out,
            block: Vec::with_capacity(BLOCK),
            adler: Adler::default(),
        }
    }

    fn push(&mut self, mut bytes: &[u8]) {
        self.adler.update(bytes);
        while !bytes.is_empty() {
            let room = BLOCK - self.block.len();
            let (now, rest) = bytes.split_at(room.min(bytes.len()));
            self.block.extend_from_slice(now);
            bytes = rest;
            if self.block.len() == BLOCK {
                self.flush(false);
            }
        }
    }

    fn flush(&mut self, last: bool) {
        let n = self.block.len() as u16;
        self.out.push(last as u8);
        self.out.extend_from_slice(&n.to_le_bytes());
        self.out.extend_from_slice(&(!n).to_le_bytes());
        self.out.extend_from_slice(&self.block);
        self.block.clear();
    }

    /// The last block, which may be empty, and the checksum.
    fn finish(mut self) {
        self.flush(true);
        let a = self.adler.value();
        self.out.extend_from_slice(&a.to_be_bytes());
    }
}

/// The zlib stream of `raw` in stored blocks.
pub fn zlib_store(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / BLOCK * 5 + 11);
    let mut z = Stored::new(&mut out);
    z.push(raw);
    z.finish();
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// The PNG file of a `w` by `h` RGB image, rows top first.
pub fn encode(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
    let raw = h * (1 + 3 * w);
    let mut out = Vec::with_capacity(raw + raw / BLOCK * 5 + 128);
    out.extend_from_slice(&SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    // 8 bits a channel, colour type 2 (RGB), deflate, filter method 0,
    // no interlace.
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    // The IDAT chunk is written in place: its length is known once the
    // stream is.
    let at = out.len();
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(b"IDAT");
    {
        let mut z = Stored::new(&mut out);
        for row in rgb.chunks(3 * w.max(1)).take(h) {
            z.push(&[0]);
            z.push(row);
        }
        z.finish();
    }
    let len = (out.len() - at - 8) as u32;
    out[at..at + 4].copy_from_slice(&len.to_be_bytes());
    let crc = crc32(&out[at + 4..]);
    out.extend_from_slice(&crc.to_be_bytes());
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::figure::{BLACK, Figure, Series, scene};

    /// A minimal PNG reader for what [`encode`] writes: every chunk's CRC
    /// checked, the stored blocks unpacked, the Adler-32 checked.
    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(png[..8], SIGNATURE);
        let mut k = 8;
        let (mut w, mut h, mut idat) = (0, 0, Vec::new());
        let mut kinds = Vec::new();
        while k < png.len() {
            let len = u32::from_be_bytes(png[k..k + 4].try_into().unwrap()) as usize;
            let body = &png[k + 4..k + 8 + len];
            let crc = u32::from_be_bytes(png[k + 8 + len..k + 12 + len].try_into().unwrap());
            assert_eq!(crc32(body), crc, "chunk CRC");
            let kind = &body[..4];
            let data = &body[4..];
            kinds.push(String::from_utf8(kind.to_vec()).unwrap());
            match kind {
                b"IHDR" => {
                    w = u32::from_be_bytes(data[..4].try_into().unwrap());
                    h = u32::from_be_bytes(data[4..8].try_into().unwrap());
                    assert_eq!(data[8..], [8, 2, 0, 0, 0]);
                }
                b"IDAT" => idat.extend_from_slice(data),
                _ => {}
            }
            k += 12 + len;
        }
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        (w, h, inflate_stored(&idat))
    }

    fn inflate_stored(z: &[u8]) -> Vec<u8> {
        assert_eq!(z[..2], [0x78, 0x01]);
        assert_eq!(u16::from_be_bytes([z[0], z[1]]) % 31, 0);
        let mut k = 2;
        let mut raw = Vec::new();
        loop {
            let head = z[k];
            assert_eq!(head & 0b110, 0, "a stored block");
            let n = u16::from_le_bytes([z[k + 1], z[k + 2]]);
            let nn = u16::from_le_bytes([z[k + 3], z[k + 4]]);
            assert_eq!(n, !nn);
            raw.extend_from_slice(&z[k + 5..k + 5 + n as usize]);
            k += 5 + n as usize;
            if head & 1 == 1 {
                break;
            }
        }
        let adler = u32::from_be_bytes(z[k..k + 4].try_into().unwrap());
        assert_eq!(adler, adler32(&raw));
        assert_eq!(k + 4, z.len());
        raw
    }

    #[test]
    fn the_checksums_match_their_published_values() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        // Past one chunk of 5552 the modulus still holds.
        let big = vec![255u8; 100_000];
        let (mut a, mut b) = (1u64, 0u64);
        for &x in &big {
            a = (a + x as u64) % 65521;
            b = (b + a) % 65521;
        }
        assert_eq!(adler32(&big), ((b << 16) | a) as u32);
    }

    #[test]
    fn the_zlib_stream_is_stored_blocks_of_at_most_65535_bytes() {
        for n in [0usize, 1, 65535, 65536, 200_000] {
            let raw: Vec<u8> = (0..n).map(|k| (k * 7 % 251) as u8).collect();
            let z = zlib_store(&raw);
            assert_eq!(inflate_stored(&z), raw, "{n}");
            // Every full block, then a last one with the rest, which may be
            // empty.
            let blocks = n / BLOCK + 1;
            assert_eq!(z.len(), 2 + 5 * blocks + n + 4, "{n}");
        }
    }

    #[test]
    fn an_encoded_image_decodes_to_its_filtered_rows() {
        let (w, h) = (3, 2);
        let rgb: Vec<u8> = (0..18).collect();
        let png = encode(w, h, &rgb);
        let (dw, dh, raw) = decode(&png);
        assert_eq!((dw, dh), (3, 2));
        let mut want = vec![0];
        want.extend_from_slice(&rgb[..9]);
        want.push(0);
        want.extend_from_slice(&rgb[9..]);
        assert_eq!(raw, want);
    }

    #[test]
    fn the_rasterizer_fills_strokes_clips_and_writes_text() {
        let mut c = Canvas::new(10, 10, 1.0);
        let red = Color(255, 0, 0);
        c.fill(2.0, 2.0, 4.0, 4.0, red, 1.0);
        assert_eq!(c.at(2, 2), red);
        assert_eq!(c.at(3, 3), red);
        assert_eq!(c.at(4, 4), Color(255, 255, 255));
        // Half opacity over white.
        c.fill(6.0, 6.0, 7.0, 7.0, Color(0, 0, 0), 0.5);
        assert_eq!(c.at(6, 6), Color(128, 128, 128));
        // A horizontal line, and a clipped one far outside the canvas that
        // costs only its visible part.
        c.line((0.0, 8.5), (9.9, 8.5), BLACK, 1.0, Dash::Solid);
        assert_eq!(c.at(5, 8), BLACK);
        c.line((-1e6, 0.5), (1e6, 0.5), BLACK, 1.0, Dash::Solid);
        assert_eq!(c.at(9, 0), BLACK);
        // A dashed line leaves gaps.
        let mut d = Canvas::new(20, 1, 1.0);
        d.line((0.0, 0.5), (19.9, 0.5), BLACK, 1.0, Dash::Dashed);
        let dark = (0..20).filter(|&x| d.at(x, 0) == BLACK).count();
        assert!(dark > 8 && dark < 18, "{dark}");
        // Text: a glyph's set bits, at scale 1.
        let mut t = Canvas::new(20, 12, 1.0);
        t.text(1.0, 6.0, Anchor::Start, 1, false, false, "I");
        let inked = (0..20)
            .flat_map(|x| (0..12).map(move |y| (x, y)))
            .filter(|&(x, y)| t.at(x, y) != Color(255, 255, 255))
            .count();
        let bits: u32 = glyph('I').iter().map(|r| r.count_ones()).sum();
        assert_eq!(inked, bits as usize);
        assert_eq!(glyph('\u{e9}'), &BOX);
    }

    /// How a segment wider than 1.5 pixels was drawn before `thick`: a
    /// whole square of its width at every step, at a cost of the steps
    /// times the square's area.
    fn stamped(c: &mut Canvas, p0: (f64, f64), p1: (f64, f64), col: Color, width: f64) {
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let n = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
        let r = width / 2.0;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let (x, y) = (p0.0 + t * dx, p0.1 + t * dy);
            c.fill(x - r, y - r, x + r, y + r, col, 1.0);
        }
    }

    /// A run across the line a pixel along it covers exactly the pixels the
    /// stamped squares did, in every direction, at any width past 1.5 and
    /// inside a clip box.
    #[test]
    fn a_wide_segment_covers_the_pixels_its_stepped_squares_did() {
        let mut segs = vec![
            ((3.0, 3.0), (30.0, 3.0)),
            ((30.0, 3.0), (3.0, 3.0)),
            ((5.5, 2.0), (5.5, 35.0)),
            ((5.5, 35.0), (5.5, 2.0)),
            ((0.5, 0.5), (10.5, 10.5)),
            ((10.5, 10.5), (0.5, 0.5)),
            ((2.25, 30.75), (37.1, 4.3)),
            ((12.0, 12.0), (12.0, 12.0)),
            ((-6.0, 20.0), (47.0, 26.5)),
            ((20.0, -3.0), (17.0, 44.0)),
        ];
        // And pseudo-random ones, from a fixed linear congruential sequence.
        let mut s = 12345u64;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 33) as f64 / (1u64 << 31) as f64 * 50.0 - 5.0
        };
        for _ in 0..300 {
            segs.push(((next(), next()), (next(), next())));
        }
        let boxes = [
            None,
            Some(Clip {
                x0: 6,
                y0: 9,
                x1: 31,
                y1: 27,
            }),
        ];
        let mut inked = 0;
        for (k, &(p0, p1)) in segs.iter().enumerate() {
            let width = [1.6, 2.0, 2.5, 3.0, 4.0, 7.3, 10.0][k % 7];
            for clip in boxes {
                let mut old = Canvas::new(40, 40, 1.0);
                let mut new = Canvas::new(40, 40, 1.0);
                if let Some(b) = clip {
                    old.clips.push(b);
                    new.clips.push(b);
                }
                stamped(&mut old, p0, p1, BLACK, width);
                new.solid(p0, p1, BLACK, width);
                assert!(old.rgb == new.rgb, "{p0:?} {p1:?} width {width} {clip:?}");
                inked += old.rgb.contains(&0) as usize;
            }
        }
        // Nearly every case drew something to compare.
        assert!(inked > segs.len() * 3 / 2, "{inked}");
    }

    #[test]
    fn a_figure_rasterizes_at_its_resolution_and_the_size_is_judged() {
        let mut fig = Figure::default();
        fig.add_series(3, |_| {
            vec![Series::Bars {
                rects: vec![[0.6, 1.4, 1.0], [1.6, 2.4, 2.0], [2.6, 3.4, 3.0]],
                color: Color(0, 114, 189),
                opacity: 1.0,
            }]
        })
        .unwrap();
        let sc = scene(&fig);
        let (w, h) = pixel_size(1.0).unwrap();
        assert_eq!((w, h), (560, 420));
        let png = render(&sc, w, h);
        let (dw, dh, raw) = decode(&png);
        assert_eq!((dw, dh), (560, 420));
        assert_eq!(raw.len(), 420 * (1 + 3 * 560));
        // Some pixel is the bars' colour.
        assert!(raw.windows(3).any(|p| p == [0, 114, 189]));
        assert_eq!(pixel_size(2.0).unwrap(), (1120, 840));
        let e = pixel_size(100000.0 / 96.0).unwrap_err().msg;
        assert!(e.starts_with("Requested 437500x583333 array"), "{e}");
    }
}
