//! The SVG writer (cycle 12): a [`Scene`] as the text of an SVG document.
//!
//! The vocabulary is small and fixed, and is what the golden cases read
//! back with `strfind`:
//!
//! - each axes is a `<g class="axes">` group; nothing else carries that
//!   class;
//! - each run of a data line is one `<polyline class="line" ...>`, with
//!   `stroke="#rrggbb"` and, for a dashed, dotted or dash-dot style, a
//!   `stroke-dasharray`; nothing else is a polyline;
//! - each scatter point, and each `o` or `.` marker, is a `<circle>`; a
//!   legend's circle sample is an `<ellipse>`, so it is never counted as
//!   data;
//! - each bar of `bar` and `histogram` is a `<rect class="bar" ...>`;
//! - each text is one `<text>` element whose content is the text alone,
//!   escaped, so a label `t` appears as `>t<`.
//!
//! The data of each axes sits in a group clipped to its plot box. Numbers
//! are written with at most two decimals.

use std::fmt::Write;

use super::figure::{Anchor, Color, Dash, Item, Rect, Scene, Shape};

/// A number as the SVG writes it: two decimals at most, trailing zeros
/// dropped, never `-0`.
fn num(out: &mut String, v: f64) {
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        let _ = write!(out, "{}", r as i64);
    } else {
        let s = format!("{:.2}", r);
        out.push_str(s.trim_end_matches('0'));
    }
}

fn attr(out: &mut String, name: &str, v: f64) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    num(out, v);
    out.push('"');
}

fn color_attr(out: &mut String, name: &str, c: Option<Color>) {
    let _ = write!(
        out,
        " {}=\"{}\"",
        name,
        c.map_or_else(|| "none".to_string(), Color::hex)
    );
}

fn dash_attr(out: &mut String, d: Dash) {
    let p = d.pattern();
    if p.is_empty() {
        return;
    }
    out.push_str(" stroke-dasharray=\"");
    for (k, v) in p.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        num(out, *v);
    }
    out.push('"');
}

/// Text escaped for element content and attribute values.
fn escape(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // XML 1.0 forbids most control characters outright, and the
            // two noncharacters U+FFFE and U+FFFF.
            c if ((c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r'))
                || matches!(c, '\u{fffe}' | '\u{ffff}') =>
            {
                out.push('\u{fffd}')
            }
            c => out.push(c),
        }
    }
}

fn rect_attrs(out: &mut String, r: &Rect) {
    attr(out, "x", r.x);
    attr(out, "y", r.y);
    attr(out, "width", r.w.max(0.0));
    attr(out, "height", r.h.max(0.0));
}

fn points(out: &mut String, pts: &[(f64, f64)]) {
    out.push_str(" points=\"");
    for (k, (x, y)) in pts.iter().enumerate() {
        if k > 0 {
            out.push(' ');
        }
        num(out, *x);
        out.push(',');
        num(out, *y);
    }
    out.push('"');
}

/// About what `item` writes, a little over for typical coordinates: a
/// coordinate pair about 14 bytes, a marker's tag about 70, a bar 138 with
/// its opacity. Reserving the sum up front writes a large figure without
/// growing its text by doubling, which would leave up to half of it unused
/// and briefly hold the old and the new copies.
fn estimate(item: &Item) -> usize {
    match item {
        Item::Begin { .. } => 176,
        Item::End => 8,
        Item::Rect { .. } => 144,
        Item::Line { .. } => 112,
        Item::Polyline { points, .. } => 112 + 16 * points.len(),
        Item::Marker { shape, .. } => match shape {
            Shape::Circle { .. } => 96,
            Shape::Polygon(p) => 72 + 16 * p.len(),
            Shape::Strokes(s) => 64 + 32 * s.len(),
        },
        Item::Text { text, .. } => 128 + 2 * text.len(),
    }
}

/// The whole document.
pub fn render(scene: &Scene) -> String {
    let size = scene
        .items
        .iter()
        .fold(512usize, |n, i| n.saturating_add(estimate(i)));
    let mut out = String::with_capacity(size);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\"");
    attr(&mut out, "width", scene.width);
    attr(&mut out, "height", scene.height);
    out.push_str(" viewBox=\"0 0 ");
    num(&mut out, scene.width);
    out.push(' ');
    num(&mut out, scene.height);
    out.push_str("\" font-family=\"Helvetica, Arial, sans-serif\">\n");
    let mut clips = 0;
    for item in &scene.items {
        item_svg(&mut out, item, &mut clips);
    }
    out.push_str("</svg>\n");
    out
}

fn item_svg(out: &mut String, item: &Item, clips: &mut usize) {
    match item {
        Item::Begin { class, clip } => {
            if let Some(r) = clip {
                *clips += 1;
                let _ = write!(out, "<clipPath id=\"clip{}\"><rect", clips);
                rect_attrs(out, r);
                out.push_str("/></clipPath>\n");
                let _ = writeln!(
                    out,
                    "<g class=\"{}\" clip-path=\"url(#clip{})\">",
                    class, clips
                );
            } else {
                let _ = writeln!(out, "<g class=\"{}\">", class);
            }
        }
        Item::End => out.push_str("</g>\n"),
        Item::Rect {
            class,
            r,
            fill,
            opacity,
            stroke,
        } => {
            let _ = write!(out, "<rect class=\"{}\"", class);
            rect_attrs(out, r);
            color_attr(out, "fill", *fill);
            if *opacity < 1.0 {
                attr(out, "fill-opacity", *opacity);
            }
            color_attr(out, "stroke", *stroke);
            if stroke.is_some() {
                out.push_str(" stroke-width=\"0.5\"");
            }
            out.push_str("/>\n");
        }
        Item::Line {
            class,
            x1,
            y1,
            x2,
            y2,
            stroke,
            dash,
        } => {
            let _ = write!(out, "<line class=\"{}\"", class);
            attr(out, "x1", *x1);
            attr(out, "y1", *y1);
            attr(out, "x2", *x2);
            attr(out, "y2", *y2);
            color_attr(out, "stroke", Some(*stroke));
            dash_attr(out, *dash);
            out.push_str("/>\n");
        }
        Item::Polyline {
            points: pts,
            stroke,
            dash,
        } => {
            out.push_str("<polyline class=\"line\" fill=\"none\"");
            color_attr(out, "stroke", Some(*stroke));
            out.push_str(" stroke-width=\"1\"");
            dash_attr(out, *dash);
            points(out, pts);
            out.push_str("/>\n");
        }
        Item::Marker {
            shape,
            stroke,
            fill,
            sample,
        } => {
            match shape {
                Shape::Circle { cx, cy, r } => {
                    if *sample {
                        out.push_str("<ellipse class=\"sample\"");
                        attr(out, "cx", *cx);
                        attr(out, "cy", *cy);
                        attr(out, "rx", *r);
                        attr(out, "ry", *r);
                    } else {
                        out.push_str("<circle class=\"marker\"");
                        attr(out, "cx", *cx);
                        attr(out, "cy", *cy);
                        attr(out, "r", *r);
                    }
                }
                Shape::Polygon(pts) => {
                    out.push_str("<polygon class=\"marker\"");
                    points(out, pts);
                }
                Shape::Strokes(lines) => {
                    out.push_str("<path class=\"marker\" d=\"");
                    for l in lines {
                        out.push('M');
                        num(out, l[0]);
                        out.push(',');
                        num(out, l[1]);
                        out.push('L');
                        num(out, l[2]);
                        out.push(',');
                        num(out, l[3]);
                    }
                    out.push('"');
                }
            }
            color_attr(out, "fill", *fill);
            color_attr(out, "stroke", Some(*stroke));
            out.push_str("/>\n");
        }
        Item::Text {
            class,
            x,
            y,
            anchor,
            size,
            bold,
            vertical,
            text,
        } => {
            let _ = write!(out, "<text class=\"{}\"", class);
            attr(out, "x", *x);
            // `y` is the text's middle; the baseline sits a third of the
            // size below it.
            attr(out, "y", y + size * 0.35);
            attr(out, "font-size", *size);
            let anchor = match anchor {
                Anchor::Start => "start",
                Anchor::Middle => "middle",
                Anchor::End => "end",
            };
            let _ = write!(out, " text-anchor=\"{}\"", anchor);
            if *bold {
                out.push_str(" font-weight=\"bold\"");
            }
            if *vertical {
                out.push_str(" transform=\"rotate(-90 ");
                num(out, *x);
                out.push(' ');
                num(out, *y);
                out.push('"');
            }
            out.push_str(" fill=\"#262626\">");
            escape(out, text);
            out.push_str("</text>\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::figure::{COLOR_ORDER, Figure, Place, Series, parse_line_spec, scene};

    fn count(s: &str, pat: &str) -> usize {
        s.matches(pat).count()
    }

    fn line(x: Vec<f64>, y: Vec<f64>, spec: &str) -> Series {
        let spec = parse_line_spec(spec).unwrap();
        Series::Line {
            x,
            y,
            color: spec.color.unwrap_or(COLOR_ORDER[0]),
            dash: spec.dash.or(if spec.marker.is_some() {
                None
            } else {
                Some(Dash::Solid)
            }),
            marker: spec.marker,
        }
    }

    fn svg_of(fig: &Figure) -> String {
        render(&scene(fig))
    }

    #[test]
    fn numbers_have_two_decimals_at_most_and_no_negative_zero() {
        let s = |v| {
            let mut o = String::new();
            num(&mut o, v);
            o
        };
        assert_eq!(s(1.0), "1");
        assert_eq!(s(1.5), "1.5");
        assert_eq!(s(2.345678), "2.35");
        assert_eq!(s(-0.001), "0");
        assert_eq!(s(-12.1), "-12.1");
    }

    #[test]
    fn a_line_plot_is_one_polyline_in_one_axes_with_its_ticks_as_text() {
        let mut fig = Figure::default();
        fig.add_series(3, |_| {
            vec![line(vec![1.0, 2.0, 3.0], vec![1.0, 4.0, 9.0], "")]
        })
        .unwrap();
        let s = svg_of(&fig);
        assert!(s.starts_with("<?xml"), "{s}");
        assert!(s.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(s.trim_end().ends_with("</svg>"));
        assert_eq!(count(&s, "<polyline"), 1);
        assert_eq!(count(&s, "class=\"axes\""), 1);
        assert!(s.contains(">9<"), "{s}");
        assert!(s.contains(">2.5<"), "{s}");
        assert!(s.contains("stroke=\"#0072bd\""));
        assert!(!s.contains("stroke-dasharray"));
        // Every group closes.
        assert_eq!(count(&s, "<g "), count(&s, "</g>"));
    }

    #[test]
    fn a_line_spec_sets_the_stroke_the_dashes_and_the_markers() {
        let mut fig = Figure::default();
        fig.add_series(3, |_| {
            vec![line(vec![1.0, 2.0, 3.0], vec![1.0, 2.0, 3.0], "r--")]
        })
        .unwrap();
        let s = svg_of(&fig);
        assert!(s.contains("stroke=\"#ff0000\""));
        assert!(s.contains("stroke-dasharray=\"6,4\""));
        fig.add_series(3, |_| {
            vec![line(vec![1.0, 2.0, 3.0], vec![1.0, 2.0, 3.0], "o")]
        })
        .unwrap();
        let s = svg_of(&fig);
        // Markers alone: no line, one circle a point.
        assert_eq!((count(&s, "<polyline"), count(&s, "<circle")), (0, 3));
        fig.add_series(2, |_| vec![line(vec![1.0, 2.0], vec![1.0, 2.0], "s:")])
            .unwrap();
        let s = svg_of(&fig);
        assert_eq!(count(&s, "<polygon class=\"marker\""), 2);
        assert!(s.contains("stroke-dasharray=\"1.5,3\""));
    }

    #[test]
    fn scatter_is_circles_and_bars_are_bar_rects() {
        let mut fig = Figure::default();
        fig.add_series(3, |_| {
            vec![Series::Scatter {
                x: vec![1.0, 2.0, 3.0],
                y: vec![3.0, 1.0, 2.0],
                sizes: vec![36.0],
                color: COLOR_ORDER[0],
                filled: false,
            }]
        })
        .unwrap();
        fig.gca().legend = Some(vec!["pts".into()]);
        let s = svg_of(&fig);
        // The legend's sample is an ellipse, so the data's circles count.
        assert_eq!(count(&s, "<circle"), 3);
        assert_eq!(count(&s, "<ellipse"), 1);
        assert!(s.contains(">pts<"));
        fig.add_series(2, |_| {
            vec![Series::Bars {
                rects: vec![[0.6, 1.4, 1.0], [1.6, 2.4, -2.0]],
                color: COLOR_ORDER[0],
                opacity: 0.6,
            }]
        })
        .unwrap();
        let s = svg_of(&fig);
        assert_eq!(count(&s, "<rect class=\"bar\""), 2);
        assert!(s.contains("fill-opacity=\"0.6\""));
        assert!(s.contains(">-2<"), "{s}");
    }

    #[test]
    fn labels_are_texts_of_their_own_and_escaped() {
        let mut fig = Figure::default();
        fig.add_series(2, |_| vec![line(vec![1.0, 2.0], vec![1.0, 2.0], "k")])
            .unwrap();
        let ax = fig.gca();
        ax.xlabel = "t".into();
        ax.ylabel = "y".into();
        ax.title = "a<b & \"c\"".into();
        ax.legend = Some(vec!["a".into(), "extra".into()]);
        ax.grid = true;
        let s = svg_of(&fig);
        for want in [">t<", ">y<", ">a<", ">a&lt;b &amp; &quot;c&quot;<"] {
            assert!(s.contains(want), "{want}: {s}");
        }
        // A label with no series to name is not drawn.
        assert!(!s.contains(">extra<"));
        assert!(s.contains("class=\"grid\""));
        assert!(s.contains("rotate(-90"));
        assert!(s.contains("stroke=\"#000000\""));
    }

    #[test]
    fn subplots_are_axes_groups_of_their_own() {
        let mut fig = Figure::default();
        for k in 0..2 {
            fig.subplot(Place::cells(2, 1, k, k));
            fig.add_series(2, |_| vec![line(vec![1.0, 2.0], vec![1.0, 2.0], "")])
                .unwrap();
        }
        let s = svg_of(&fig);
        assert_eq!(count(&s, "class=\"axes\""), 2);
        assert_eq!(count(&s, "<clipPath"), 2);
        // axis off hides the ticks but keeps the data.
        fig.gca().visible = false;
        let s = svg_of(&fig);
        assert_eq!(count(&s, "<polyline"), 2);
        assert_eq!(count(&s, "class=\"frame\""), 1);
    }

    #[test]
    fn characters_xml_forbids_are_replaced() {
        let mut o = String::new();
        escape(&mut o, "a\u{0}b\u{1b}c\td\u{fffe}\u{ffff}e&'");
        assert_eq!(o, "a\u{fffd}b\u{fffd}c\td\u{fffd}\u{fffd}e&amp;'");
    }
}
