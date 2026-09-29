//! The environment a name can come from, as seen from outside the evaluator.
//!
//! [`completions`] answers the protocol's `completions` operation: every name
//! a user could type next, from the three places a name resolves: the
//! workspace variables, the `.m` files on the path (cycle 13) and the
//! builtin registry. The terminal's tab completion (cycle 13) calls the
//! same function, so the two can never offer different names.
//!
//! [`preview`] is the value text the desktop's workspace pane shows beside
//! each variable (cycle U2), which `workspace` with `"preview": true`
//! answers: SplatCrab's own one-line summary of a value, by the rules of
//! `docs/modules/U2-ui-desktop.md`, costing time in proportion to what it
//! shows rather than to the value's size.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::builtins::Registry;
use crate::value::{Class, Format, Matrix, Value, code_unit, with_format};

/// The longest preview, in Unicode scalar values; a longer one is cut to
/// this many and followed by `…`.
pub const PREVIEW_CHARS: usize = 80;

/// The most elements a numeric or logical array may have for its preview
/// to write them out; a larger one is shown by its size and class.
pub const PREVIEW_ELEMENTS: usize = 10;

/// The workspace pane's text for `v`, displayed in `format`, by the first
/// rule that applies:
///
/// 1. a numeric or logical array, real or complex, that is not empty and
///    has at most [`PREVIEW_ELEMENTS`] elements: each element as `disp`
///    prints it alone, as the 1x1 value indexing gives (so a zero imaginary
///    part is dropped), without its leading and trailing spaces; one alone,
///    more in brackets with `,` between the elements of a row and `;`
///    between rows: `3`, `[1,2;3,4]`, `[1.0000 + 2.0000i,3]`;
/// 2. a char array of at most one row: its text quoted as a char literal
///    writes it, each `'` doubled: `'it''s'`, `''`;
/// 3. a function handle: its text, `@(x)x+1` or `@sin`;
/// 4. anything else: its size and class, `2×3 double`, `1×1 struct`, with
///    `complex ` before `double` for complex storage.
///
/// A preview longer than [`PREVIEW_CHARS`] is cut to that many and followed
/// by `…`. Rule 2 decodes only the code units the cut can keep, so a long
/// char row costs no more than a short one, and rule 3 renders a handle's
/// text only as far as one character past the cut, so a long handle costs
/// no more than a short one either.
pub fn preview(v: &Value, format: Format) -> String {
    match v {
        Value::Mat(m)
            if m.class != Class::Char && !m.is_empty() && m.numel() <= PREVIEW_ELEMENTS =>
        {
            cut(elements(m, format))
        }
        Value::Mat(m) if m.class == Class::Char && m.rows <= 1 => {
            // One row, or none: `data` is the row in order.
            quoted(m.data.iter().map(|&u| code_unit(u) as u16))
        }
        // Rendered only as far as the cut needs to know it is cut.
        Value::Func(f) => cut(f.shown_up_to(PREVIEW_CHARS + 1)),
        v => {
            let (rows, cols) = v.dims();
            let complex = matches!(v, Value::Mat(m) if m.is_complex());
            let prefix = if complex { "complex " } else { "" };
            cut(format!("{rows}×{cols} {prefix}{}", v.class_name()))
        }
    }
}

/// Rule 1: the elements of a small numeric or logical array.
fn elements(m: &Matrix, format: Format) -> String {
    let one = |k: usize| {
        let text = with_format(format, || m.element(k).disp_text());
        text.trim_matches([' ', '\n']).to_string()
    };
    if m.numel() == 1 {
        return one(0);
    }
    let rows: Vec<String> = (0..m.rows)
        .map(|r| {
            (0..m.cols)
                .map(|c| one(c * m.rows + r))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    format!("[{}]", rows.join(";"))
}

/// Rule 2: UTF-16 code units as a quoted char literal, decoded only until
/// the text is known to be longer than the cut keeps. An unpaired
/// surrogate is U+FFFD, as everywhere else text is decoded.
fn quoted(units: impl Iterator<Item = u16>) -> String {
    let mut out = String::from("'");
    let mut n = 1;
    for c in char::decode_utf16(units) {
        match c.unwrap_or(char::REPLACEMENT_CHARACTER) {
            '\'' => {
                out.push_str("''");
                n += 2;
            }
            c => {
                out.push(c);
                n += 1;
            }
        }
        // Past the cut already, and the closing quote is still to come:
        // nothing further can be shown.
        if n > PREVIEW_CHARS {
            return cut(out);
        }
    }
    out.push('\'');
    cut(out)
}

/// `text` cut to its first [`PREVIEW_CHARS`] scalar values and `…` when it
/// is longer than that, and unchanged otherwise.
fn cut(text: String) -> String {
    match text.char_indices().nth(PREVIEW_CHARS) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    }
}

/// Every variable, function file on the path and builtin whose name starts
/// with `prefix`, sorted by byte order and without duplicates: a variable
/// or a file that shadows a builtin is one name, not two. An empty prefix
/// lists everything.
///
/// A function file is a file in one of the `path` folders whose name is a
/// MATLAB identifier followed by `.m`, listed without the `.m`, as a call
/// names it. A folder that cannot be read contributes nothing.
pub fn completions(
    prefix: &str,
    vars: &HashMap<String, Value>,
    registry: &Registry,
    path: &[PathBuf],
) -> Vec<String> {
    let mut names: Vec<String> = vars
        .keys()
        .map(String::as_str)
        .chain(registry.keys().copied())
        .filter(|n| n.starts_with(prefix))
        .map(str::to_string)
        .collect();
    for dir in path {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let file = entry.file_name();
            let Some(stem) = file.to_str().and_then(|f| f.strip_suffix(".m")) else {
                continue;
            };
            let is_file = entry.file_type().is_ok_and(|t| !t.is_dir());
            if is_file && stem.starts_with(prefix) && crate::interp::is_identifier(stem) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::registry;
    use crate::value::Matrix;

    fn vars(names: &[&str]) -> HashMap<String, Value> {
        names
            .iter()
            .map(|n| (n.to_string(), Value::Mat(Matrix::scalar(1.0))))
            .collect()
    }

    #[test]
    fn builtins_alone() {
        let r = registry();
        assert_eq!(completions("dis", &vars(&[]), &r, &[]), ["disp"]);
        assert_eq!(
            completions("zzz", &vars(&[]), &r, &[]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn variables_and_builtins_are_merged_in_order() {
        let r = registry();
        let got = completions("dis", &vars(&["display_count", "dia"]), &r, &[]);
        assert_eq!(got, ["disp", "display_count"]);
        // Cycle 12's `xlabel` and `xlim` sort among the variables.
        let got = completions("x", &vars(&["x2", "x", "x10"]), &r, &[]);
        assert_eq!(got, ["x", "x10", "x2", "xlabel", "xlim"]);
    }

    #[test]
    fn a_variable_shadowing_a_builtin_appears_once() {
        let r = registry();
        let got = completions("disp", &vars(&["disp"]), &r, &[]);
        assert_eq!(got, ["disp"]);
    }

    #[test]
    fn an_empty_prefix_lists_every_name_once_sorted() {
        let r = registry();
        let got = completions("", &vars(&["sum", "aaa"]), &r, &[]);
        assert_eq!(got.len(), r.len() + 1);
        assert!(got.windows(2).all(|w| w[0] < w[1]));
        // Byte order, as `who` sorts: capitals first.
        assert_eq!(got[..3], ["Inf", "NaN", "aaa"]);
    }

    /// Cycle 13: the `.m` files of every path folder, without the `.m`,
    /// merged with the rest; a file that is not a function name, a folder
    /// and a file shadowing a builtin are handled.
    #[test]
    fn path_files_are_listed_by_their_function_names() {
        let dir = std::env::temp_dir().join(format!("splatcrab-env-{}", std::process::id()));
        let other = dir.join("more");
        std::fs::create_dir_all(&other).unwrap();
        for f in ["mytool.m", "myother.m", "disp.m", "my-bad.m", "mydata.txt"] {
            std::fs::write(dir.join(f), "x = 1;\n").unwrap();
        }
        std::fs::create_dir_all(dir.join("mydir.m")).unwrap();
        std::fs::write(other.join("myfar.m"), "x = 1;\n").unwrap();
        let r = registry();
        let path = [dir.clone(), other.clone(), dir.join("missing")];
        let got = completions("my", &vars(&["myvar"]), &r, &path);
        assert_eq!(got, ["myfar", "myother", "mytool", "myvar"]);
        assert_eq!(completions("disp", &vars(&[]), &r, &path), ["disp"]);
        // No path: exactly the names the protocol offered before.
        assert_eq!(completions("my", &vars(&[]), &r, &[]), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_prefix_is_case_sensitive() {
        let r = registry();
        assert_eq!(completions("D", &vars(&["Data"]), &r, &[]), ["Data"]);
    }

    /// Runs `code` in a fresh session and previews each named variable.
    fn previews(code: &str, names: &[&str]) -> Vec<String> {
        let mut it =
            crate::interp::Interp::with_sinks(Box::new(std::io::sink()), Box::new(std::io::sink()));
        it.run_command(code).expect("the code runs");
        names
            .iter()
            .map(|n| preview(&it.vars()[*n], it.format))
            .collect()
    }

    /// Rule 1: each element as `disp` prints it alone, trimmed; one alone,
    /// more in brackets, `,` within a row and `;` between rows; a complex
    /// element with a zero imaginary part shown real, as indexing gives it.
    #[test]
    fn preview_rule_one_writes_small_arrays_out() {
        let got = previews(
            "x = 3; v = [1 2 3]; m = [1 2; 3 4]; t = true; z = 1+2i; zz = [1+2i 3]; \
             p = -2.5; lg = [true false]; nn = [NaN -Inf]; col = [1; 2]; \
             v10 = 1:10; zc = complex(1, 0); w = [1+2i 3; 4 5; 6 7];",
            &[
                "x", "v", "m", "t", "z", "zz", "p", "lg", "nn", "col", "v10", "zc", "w",
            ],
        );
        assert_eq!(
            got,
            [
                "3",
                "[1,2,3]",
                "[1,2;3,4]",
                "1",
                "1.0000 + 2.0000i",
                "[1.0000 + 2.0000i,3]",
                "-2.5000",
                "[1,0]",
                "[NaN,-Inf]",
                "[1;2]",
                "[1,2,3,4,5,6,7,8,9,10]",
                "1",
                "[1.0000 + 2.0000i,3;4,5;6,7]",
            ]
        );
        // One more than ten elements is rule 4, real or complex.
        let got = previews(
            "big = 1:11; zb = [1:11] * 1i; e = []; el = true(0, 3);",
            &["big", "zb", "e", "el"],
        );
        assert_eq!(
            got,
            [
                "1×11 double",
                "1×11 complex double",
                "0×0 double",
                "0×3 logical",
            ]
        );
    }

    /// The display format is the session's: `format long` writes fifteen
    /// decimals.
    #[test]
    fn preview_rule_one_follows_the_format() {
        let pi = Value::Mat(Matrix::scalar(std::f64::consts::PI));
        assert_eq!(preview(&pi, Format::Short), "3.1416");
        assert_eq!(preview(&pi, Format::Long), "3.141592653589793");
    }

    /// Rule 2: a char array of at most one row, quoted with each `'`
    /// doubled; several rows are rule 4.
    #[test]
    fn preview_rule_two_quotes_one_row_of_text() {
        let got = previews(
            "s = 'it''s'; q = ''; e5 = char(zeros(0, 5)); u = 'αβ😀'; ch = ['ab'; 'cd']; \
             only = '''';",
            &["s", "q", "e5", "u", "ch", "only"],
        );
        assert_eq!(got, ["'it''s'", "''", "''", "'αβ😀'", "2×2 char", "''''"]);
        // An unpaired surrogate is U+FFFD, as everywhere text is decoded.
        let lone = Value::Mat(Matrix::row(vec![97.0, 55296.0, 98.0]).with_class(Class::Char));
        assert_eq!(preview(&lone, Format::Short), "'a\u{FFFD}b'");
    }

    /// Rule 3 and rule 4's other kinds.
    #[test]
    fn preview_rules_three_and_four() {
        let got = previews(
            "f = @(x) x+1; g = @sin; c = {1}; st.a = 1; s2 = struct('a', {1, 2}); \
             try, error('a:b', 'boom'), catch ex, end",
            &["f", "g", "c", "st", "s2", "ex"],
        );
        assert_eq!(
            got,
            [
                "@(x)x+1",
                "@sin",
                "1×1 cell",
                "1×1 struct",
                "1×2 struct",
                "1×1 MException",
            ]
        );
    }

    /// The cut: more than 80 scalar values keep the first 80 and gain `…`;
    /// exactly 80 are left whole. Counted in scalar values, not bytes.
    #[test]
    fn a_long_preview_is_cut_at_eighty() {
        let got = previews(
            "w = repmat('a', 1, 100); at = repmat('b', 1, 78); over = repmat('c', 1, 79); \
             wide = repmat('é', 1, 100); quotes = repmat('''', 1, 50);",
            &["w", "at", "over", "wide", "quotes"],
        );
        assert_eq!(got[0], format!("'{}…", "a".repeat(79)));
        assert_eq!(got[1], format!("'{}'", "b".repeat(78)), "80 exactly");
        assert_eq!(got[2], format!("'{}…", "c".repeat(79)), "81 is cut");
        assert_eq!(got[3], format!("'{}…", "é".repeat(79)));
        assert_eq!(got[4], format!("'{}…", "'".repeat(79)));
        for p in &got {
            assert!(p.chars().count() <= PREVIEW_CHARS + 1, "{p}");
        }
        // A handle's text is cut the same way.
        let long = previews(&format!("f = @() {};", "1+".repeat(60) + "1"), &["f"]);
        assert_eq!(long[0].chars().count(), PREVIEW_CHARS + 1);
        assert!(long[0].starts_with("@()1+1+") && long[0].ends_with('…'));
        assert_eq!(cut("x".repeat(80)), "x".repeat(80));
        assert_eq!(cut(String::new()), "");
    }

    /// Rule 2 costs what it shows: a 2^20-element char row is previewed by
    /// decoding a handful of its code units, never the whole row.
    #[test]
    fn a_long_char_row_is_never_decoded_whole() {
        let n = 1 << 20;
        let pulled = std::cell::Cell::new(0usize);
        let units =
            std::iter::repeat_n(u16::from(b'a'), n).inspect(|_| pulled.set(pulled.get() + 1));
        let got = quoted(units);
        assert_eq!(got, format!("'{}…", "a".repeat(79)));
        assert!(pulled.get() <= PREVIEW_CHARS + 2, "pulled {}", pulled.get());
        // Surrogate pairs take two units a character, still bounded.
        let pair = [0xD83D_u16, 0xDE00];
        let pulled = std::cell::Cell::new(0usize);
        let units = pair
            .iter()
            .copied()
            .cycle()
            .take(2 * n)
            .inspect(|_| pulled.set(pulled.get() + 1));
        let got = quoted(units);
        assert_eq!(got, format!("'{}…", "😀".repeat(79)));
        assert!(
            pulled.get() <= 2 * PREVIEW_CHARS + 2,
            "pulled {}",
            pulled.get()
        );
        // And through `preview`, on the value itself.
        let big = Value::Mat(Matrix::row(vec![98.0; n]).with_class(Class::Char));
        assert_eq!(
            preview(&big, Format::Short),
            format!("'{}…", "b".repeat(79))
        );
    }

    /// Rule 3 costs what it shows too (cycle U2's review): a handle whose
    /// body is a bracket of a million elements is previewed by rendering
    /// its first 81 characters, a node or so a character, never its whole
    /// text, which used to take most of a second for every preview. The
    /// preview is still the text's first 80 characters and `…`, and
    /// `func2str`'s text is still rendered whole.
    #[test]
    fn a_long_handle_is_never_rendered_whole() {
        use crate::parser::{AnonFn, Expr, RENDERED};
        use std::rc::Rc;
        let n = 1_000_000;
        let body = Expr::Matrix(vec![vec![Expr::Num(1.0); n]]);
        let f = Rc::new(crate::value::Func::Anon {
            def: Rc::new(AnonFn::new(vec![], body)),
            captured: vec![],
            unit: Rc::new(crate::interp::Unit::default()),
        });
        let rendered = || RENDERED.with(|c| c.get());
        let before = rendered();
        let got = preview(&Value::Func(f.clone()), Format::Short);
        let visited = rendered() - before;
        assert_eq!(got, format!("@()[{}…", "1,".repeat(38)));
        assert_eq!(got.chars().count(), PREVIEW_CHARS + 1);
        assert!(visited <= PREVIEW_CHARS + 2, "visited {visited} nodes");
        // The whole text: every element, the bracket closed.
        let before = rendered();
        let whole = f.text();
        assert_eq!(rendered() - before, n + 1);
        assert_eq!(whole.len(), 2 * n + 4);
        assert!(whole.starts_with(&got[..got.len() - '…'.len_utf8()]));
        assert!(whole.ends_with(",1]"));
        // A handle's text of exactly 80 characters is left whole, and one
        // of 81 is cut, with a budget as without.
        for (len, cut_off) in [(80, false), (81, true)] {
            let text = format!("@(){}", "x".repeat(len - 3));
            let h = crate::value::Func::Anon {
                def: Rc::new(AnonFn::new(vec![], Expr::Ident("x".repeat(len - 3)))),
                captured: vec![],
                unit: Rc::new(crate::interp::Unit::default()),
            };
            assert_eq!(h.shown(), text);
            let expected = if cut_off {
                format!("{}…", &text[..PREVIEW_CHARS])
            } else {
                text
            };
            assert_eq!(preview(&Value::Func(Rc::new(h)), Format::Short), expected);
        }
    }
}
