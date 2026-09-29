//! The set functions (cycle 09): `unique`, `ismember`, `setdiff`,
//! `intersect` and `union`, over arrays of any class and over cells of
//! character vectors.
//!
//! An array's elements are compared as numbers: `NaN` equals nothing, not
//! even itself, and sorts last, as `sort` sorts it, so `unique([NaN NaN])`
//! keeps both. A cell's elements are compared as texts, by UTF-16 code unit
//! as `sort` orders chars, a shorter text before any it is a prefix of. A
//! character vector beside a cell is one text, so `ismember('a', {'a',
//! 'b'})` asks about the word, not its letters; two arrays, chars included,
//! are compared element by element. A cell holding anything but character
//! vectors, or a cell beside a numeric array, is refused.
//!
//! Results are sorted by default and in the order of first occurrence with
//! `'stable'`. A result is a row when its inputs are rows, as MATLAB orients
//! them (`is_row` and the notes on each builtin), and a column otherwise;
//! every index output is a column.

use std::cmp::Ordering;

use super::args::{at_most, need, option};
use super::{Registry, add};
use crate::error;
use crate::interp::{Interp, R};
use crate::value::{CellArray, Class, Matrix, Value};

/// One line per builtin; see the note on `core::register`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "unique", unique, "[C,ia,ic] = unique(A), unique(A,'stable') - the distinct elements of A, sorted; A may be a cell of char.");
    add(r, "ismember", ismember, "[tf,loc] = ismember(A,S) - true where an element of A is in S, and the lowest index in S where it is.");
    add(r, "setdiff", setdiff, "[C,ia] = setdiff(A,B), setdiff(A,B,'stable') - the distinct elements of A that are not in B.");
    add(r, "intersect", intersect, "[C,ia,ib] = intersect(A,B), intersect(A,B,'stable') - the distinct elements in both A and B.");
    add(r, "union", union, "[C,ia,ib] = union(A,B), union(A,B,'stable') - the distinct elements in A or B.");
}

/// One input of a set function.
enum Set {
    /// An array of any class, compared as numbers.
    Num(Matrix),
    /// Texts: the elements of a cell of character vectors, or one character
    /// vector beside a cell. `keys` are their code units, `items` the
    /// values themselves, which the results hand back unchanged.
    Text {
        keys: Vec<Vec<f64>>,
        items: Vec<Value>,
        rows: usize,
        cols: usize,
    },
}

/// A total order over doubles, `NaN` last: `sort`'s.
fn num_cmp(a: f64, b: f64) -> Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
    }
}

fn text_cmp(a: &[f64], b: &[f64]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        match x.total_cmp(y) {
            Ordering::Equal => continue,
            o => return o,
        }
    }
    a.len().cmp(&b.len())
}

impl Set {
    /// Argument `v` of the set function `name`.
    fn of(v: &Value, name: &str) -> R<Set> {
        match v {
            Value::Cell(c) => {
                let mut keys = Vec::with_capacity(c.data.len());
                for item in &c.data {
                    match item {
                        Value::Mat(m) if m.class == Class::Char && m.rows <= 1 => {
                            keys.push(m.data.clone());
                        }
                        _ => return Err(error::set_cell_contents(name)),
                    }
                }
                Ok(Set::Text {
                    keys,
                    items: c.data.clone(),
                    rows: c.rows,
                    cols: c.cols,
                })
            }
            v => Ok(Set::Num(v.mat()?.clone())),
        }
    }

    /// A character vector as one text.
    fn one_text(m: Matrix) -> Set {
        Set::Text {
            keys: vec![m.data.clone()],
            items: vec![Value::Mat(m)],
            rows: 1,
            cols: 1,
        }
    }

    fn len(&self) -> usize {
        match self {
            Set::Num(m) => m.numel(),
            Set::Text { keys, .. } => keys.len(),
        }
    }

    fn dims(&self) -> (usize, usize) {
        match self {
            Set::Num(m) => (m.rows, m.cols),
            Set::Text { rows, cols, .. } => (*rows, *cols),
        }
    }

    /// A row vector, a scalar included.
    fn is_row(&self) -> bool {
        self.dims().0 == 1
    }

    /// The 0x0 empty, which takes the other input's orientation.
    fn is_blank(&self) -> bool {
        self.dims() == (0, 0)
    }

    fn rowish(&self) -> bool {
        self.is_row() || self.is_blank()
    }

    /// Element `i` of `self` against element `j` of `other`, which is of
    /// the same kind.
    fn cmp(&self, i: usize, other: &Set, j: usize) -> Ordering {
        match (self, other) {
            (Set::Num(a), Set::Num(b)) => num_cmp(a.data[i], b.data[j]),
            (Set::Text { keys: a, .. }, Set::Text { keys: b, .. }) => text_cmp(&a[i], &b[j]),
            _ => Ordering::Equal,
        }
    }

    /// Whether the two elements are the same; `NaN` never is.
    fn same(&self, i: usize, other: &Set, j: usize) -> bool {
        match (self, other) {
            (Set::Num(a), Set::Num(b)) => a.data[i] == b.data[j],
            (Set::Text { keys: a, .. }, Set::Text { keys: b, .. }) => a[i] == b[j],
            _ => false,
        }
    }

    /// The positions `0..len`, sorted by value and, among equals, by
    /// position.
    fn sorted(&self) -> Vec<usize> {
        let mut perm: Vec<usize> = (0..self.len()).collect();
        perm.sort_by(|&i, &j| self.cmp(i, self, j));
        perm
    }
}

/// The two inputs of `ismember`, `setdiff`, `intersect` and `union`, made
/// the same kind: a character vector beside a cell becomes one text.
fn pair(a: &Value, b: &Value, name: &str) -> R<(Set, Set)> {
    let as_text = |s: Set| match s {
        Set::Num(m) if m.class == Class::Char && m.rows <= 1 => Ok(Set::one_text(m)),
        Set::Num(_) => Err(error::set_mixed(name)),
        t => Ok(t),
    };
    match (Set::of(a, name)?, Set::of(b, name)?) {
        (x @ Set::Num(_), y @ Set::Num(_)) => Ok((x, y)),
        (x @ Set::Text { .. }, y @ Set::Text { .. }) => Ok((x, y)),
        (x @ Set::Num(_), y) => Ok((as_text(x)?, y)),
        (x, y) => Ok((x, as_text(y)?)),
    }
}

/// `'sorted'` (the default) or `'stable'` among the arguments from `from`
/// on: true for stable.
fn stable_option(a: &[Value], from: usize, name: &str) -> R<bool> {
    let mut stable = false;
    for i in from..a.len() {
        match option(a, i) {
            Some(s) if s.eq_ignore_ascii_case("sorted") => stable = false,
            Some(s) if s.eq_ignore_ascii_case("stable") => stable = true,
            _ => return Err(error::set_option(name)),
        }
    }
    Ok(stable)
}

/// The distinct elements of `s`: the position of each one's first
/// occurrence, sorted by value, and for every element the number of its
/// group in that list.
fn distinct(s: &Set) -> (Vec<usize>, Vec<usize>) {
    let perm = s.sorted();
    let mut firsts: Vec<usize> = Vec::new();
    let mut group = vec![0; s.len()];
    for (k, &i) in perm.iter().enumerate() {
        if k == 0 || !s.same(perm[k - 1], s, i) {
            firsts.push(i);
        }
        group[i] = firsts.len() - 1;
    }
    (firsts, group)
}

/// Where each element of `x` first occurs in `y`, if it does, with `perm`
/// the sorted positions of `y`.
fn locate(x: &Set, i: usize, y: &Set, perm: &[usize]) -> Option<usize> {
    let k = perm.partition_point(|&j| y.cmp(j, x, i) == Ordering::Less);
    perm.get(k).copied().filter(|&j| x.same(i, y, j))
}

/// The result: the elements `sel` picks, each `(0, i)` for element `i` of
/// `x` or `(1, j)` for element `j` of `y`, as a row or a column. An array
/// result keeps the inputs' class when they share one and is a double
/// otherwise; a text result is a cell of the texts as they were given.
fn build(x: &Set, y: &Set, sel: &[(usize, usize)], row: bool) -> Value {
    let n = sel.len();
    let (r, c) = if row { (1, n) } else { (n, 1) };
    match (x, y) {
        (Set::Num(a), Set::Num(b)) => {
            let class = if a.class == b.class {
                a.class
            } else {
                Class::Double
            };
            let data = sel
                .iter()
                .map(|&(s, i)| if s == 0 { a.data[i] } else { b.data[i] })
                .collect();
            Value::Mat(Matrix::new(r, c, data).with_class(class))
        }
        (Set::Text { items: a, .. }, Set::Text { items: b, .. }) => {
            let data = sel
                .iter()
                .map(|&(s, i)| if s == 0 { a[i].clone() } else { b[i].clone() })
                .collect();
            Value::cell(CellArray::new(r, c, data))
        }
        _ => Value::Mat(Matrix::empty()),
    }
}

/// One-based positions, as a column of doubles.
fn index_col(ix: impl Iterator<Item = usize>) -> Value {
    Value::Mat(Matrix::col(ix.map(|i| (i + 1) as f64).collect()))
}

/// `unique(A)` and `[C, ia, ic] = unique(A)`: the distinct elements, sorted
/// (or in the order of first occurrence with `'stable'`), with
/// `C = A(ia)` taking each element's first occurrence and `A = C(ic)`. `C`
/// is a row when `A` is a row vector and a column otherwise, a matrix
/// included.
fn unique(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(a, 1, "unique")?;
    at_most(a, 2, "unique")?;
    let stable = stable_option(a, 1, "unique")?;
    let s = Set::of(&a[0], "unique")?;
    let (mut firsts, mut group) = distinct(&s);
    if stable {
        let mut order: Vec<usize> = (0..firsts.len()).collect();
        order.sort_by_key(|&g| firsts[g]);
        let mut rank = vec![0; order.len()];
        for (k, &g) in order.iter().enumerate() {
            rank[g] = k;
        }
        firsts = order.iter().map(|&g| firsts[g]).collect();
        group = group.iter().map(|&g| rank[g]).collect();
    }
    let sel: Vec<(usize, usize)> = firsts.iter().map(|&i| (0, i)).collect();
    let mut out = vec![build(&s, &s, &sel, s.is_row())];
    if nargout >= 2 {
        out.push(index_col(firsts.into_iter()));
    }
    if nargout >= 3 {
        out.push(index_col(group.into_iter()));
    }
    Ok(out)
}

/// `[tf, loc] = ismember(A, S)`: a logical the shape of `A`, true where the
/// element is in `S`, and the lowest index in `S` where it is, `0` where it
/// is not. A character vector `A` beside a cell `S` is one text, so both
/// are 1x1.
fn ismember(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(a, 2, "ismember")?;
    at_most(a, 2, "ismember")?;
    let (x, y) = pair(&a[0], &a[1], "ismember")?;
    let perm = y.sorted();
    let loc: Vec<f64> = (0..x.len())
        .map(|i| locate(&x, i, &y, &perm).map_or(0.0, |j| (j + 1) as f64))
        .collect();
    let (r, c) = x.dims();
    let tf = Matrix::new(r, c, loc.iter().map(|&l| (l > 0.0) as u8 as f64).collect())
        .with_class(Class::Logical);
    let mut out = vec![Value::Mat(tf)];
    if nargout >= 2 {
        out.push(Value::Mat(Matrix::new(r, c, loc)));
    }
    Ok(out)
}

/// The shared front of the two-input set functions.
fn two(a: &[Value], name: &str) -> R<(Set, Set, bool)> {
    need(a, 2, name)?;
    at_most(a, 3, name)?;
    let stable = stable_option(a, 2, name)?;
    let (x, y) = pair(&a[0], &a[1], name)?;
    Ok((x, y, stable))
}

/// `[C, ia] = setdiff(A, B)`: the distinct elements of `A` not in `B`, with
/// `C = A(ia)`. `C` is a row when `A` is a row vector, or when `A` is `[]`
/// and `B` a row.
fn setdiff(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    let (x, y, stable) = two(a, "setdiff")?;
    let perm = y.sorted();
    let (firsts, _) = distinct(&x);
    let mut keep: Vec<usize> = firsts
        .into_iter()
        .filter(|&i| locate(&x, i, &y, &perm).is_none())
        .collect();
    if stable {
        keep.sort_unstable();
    }
    let sel: Vec<(usize, usize)> = keep.iter().map(|&i| (0, i)).collect();
    let row = x.is_row() || (x.is_blank() && y.is_row());
    let mut out = vec![build(&x, &y, &sel, row)];
    if nargout >= 2 {
        out.push(index_col(keep.into_iter()));
    }
    Ok(out)
}

/// Whether a two-input result is a row: both inputs rows, or `[]`, and not
/// both `[]`.
fn both_rows(x: &Set, y: &Set) -> bool {
    x.rowish() && y.rowish() && !(x.is_blank() && y.is_blank())
}

/// `[C, ia, ib] = intersect(A, B)`: the distinct elements in both, with
/// `C = A(ia)` and `C = B(ib)`, each the first occurrence.
fn intersect(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    let (x, y, stable) = two(a, "intersect")?;
    let perm = y.sorted();
    let (firsts, _) = distinct(&x);
    let mut both: Vec<(usize, usize)> = firsts
        .into_iter()
        .filter_map(|i| locate(&x, i, &y, &perm).map(|j| (i, j)))
        .collect();
    if stable {
        both.sort_unstable();
    }
    let sel: Vec<(usize, usize)> = both.iter().map(|&(i, _)| (0, i)).collect();
    let mut out = vec![build(&x, &y, &sel, both_rows(&x, &y))];
    if nargout >= 2 {
        out.push(index_col(both.iter().map(|&(i, _)| i)));
    }
    if nargout >= 3 {
        out.push(index_col(both.iter().map(|&(_, j)| j)));
    }
    Ok(out)
}

/// `[C, ia, ib] = union(A, B)`: the distinct elements of either, each taken
/// from `A` when it is there and from `B` otherwise, so that `C` is the
/// sorted combination of `A(ia)` and `B(ib)`. With `'stable'`, `A`'s come
/// first in the order they occur there, then `B`'s.
fn union(_: &mut Interp, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    let (x, y, stable) = two(a, "union")?;
    let perm_x = x.sorted();
    let (fa, _) = distinct(&x);
    let (fb, _) = distinct(&y);
    let mut sel: Vec<(usize, usize)> = fa.iter().map(|&i| (0, i)).collect();
    sel.extend(
        fb.iter()
            .filter(|&&j| locate(&y, j, &x, &perm_x).is_none())
            .map(|&j| (1, j)),
    );
    if stable {
        sel.sort_unstable();
    } else {
        let side = |s: usize| if s == 0 { &x } else { &y };
        sel.sort_by(|&(s, i), &(t, j)| side(s).cmp(i, side(t), j));
    }
    let mut out = vec![build(&x, &y, &sel, both_rows(&x, &y))];
    if nargout >= 2 {
        out.push(index_col(sel.iter().filter(|p| p.0 == 0).map(|p| p.1)));
    }
    if nargout >= 3 {
        out.push(index_col(sel.iter().filter(|p| p.0 == 1).map(|p| p.1)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    fn row(v: &[f64]) -> Value {
        Value::Mat(Matrix::row(v.to_vec()))
    }

    fn cellstr(words: &[&str]) -> Value {
        Value::cell(CellArray::row(
            words.iter().map(|w| Value::str(w)).collect(),
        ))
    }

    fn run(f: crate::builtins::BuiltinFn, args: &[Value], nargout: usize) -> R<Vec<Value>> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        f(&mut it, args, nargout)
    }

    fn data(v: &Value) -> Vec<f64> {
        v.mat().unwrap().data.clone()
    }

    fn words(v: &Value) -> Vec<String> {
        match v {
            Value::Cell(c) => c.data.iter().map(|x| x.text().unwrap()).collect(),
            _ => panic!("not a cell"),
        }
    }

    #[test]
    fn unique_sorts_and_gives_both_index_vectors() {
        let out = run(unique, &[row(&[3.0, 1.0, 2.0, 1.0])], 3).unwrap();
        assert_eq!(data(&out[0]), [1.0, 2.0, 3.0]);
        assert_eq!(out[0].dims(), (1, 3));
        assert_eq!(data(&out[1]), [2.0, 3.0, 1.0]);
        assert_eq!(data(&out[2]), [3.0, 1.0, 2.0, 1.0]);
        let out = run(unique, &[row(&[3.0, 1.0, 3.0]), Value::str("stable")], 3).unwrap();
        assert_eq!(data(&out[0]), [3.0, 1.0]);
        assert_eq!(data(&out[2]), [1.0, 2.0, 1.0]);
        // NaN is never equal to itself, and sorts last.
        let n = run(unique, &[row(&[f64::NAN, 1.0, f64::NAN])], 1).unwrap();
        let d = data(&n[0]);
        assert_eq!(d[0], 1.0);
        assert!(d[1].is_nan() && d[2].is_nan());
        // A column or a matrix gives a column; a char stays a char.
        let m = Value::Mat(Matrix::new(2, 2, vec![2.0, 1.0, 2.0, 1.0]));
        assert_eq!(run(unique, &[m], 1).unwrap()[0].dims(), (2, 1));
        let c = run(unique, &[Value::str("hello")], 1).unwrap();
        assert_eq!(c[0].text().unwrap(), "ehlo");
        assert!(run(unique, &[num(1.0), Value::str("rows")], 1).is_err());
    }

    #[test]
    fn unique_of_a_cell_sorts_texts_by_code_unit() {
        let out = run(unique, &[cellstr(&["b", "a", "b", "ab", ""])], 3).unwrap();
        assert_eq!(words(&out[0]), ["", "a", "ab", "b"]);
        assert_eq!(out[0].dims(), (1, 4));
        assert_eq!(data(&out[1]), [5.0, 2.0, 4.0, 1.0]);
        assert_eq!(data(&out[2]), [4.0, 2.0, 4.0, 3.0, 1.0]);
        let upper = run(unique, &[cellstr(&["b", "B", "a"])], 1).unwrap();
        assert_eq!(words(&upper[0]), ["B", "a", "b"]);
        let bad = Value::cell(CellArray::row(vec![num(1.0), Value::str("a")]));
        let e = run(unique, &[bad], 1).unwrap_err().msg;
        assert!(e.contains("character vectors only"), "{e}");
    }

    #[test]
    fn ismember_gives_the_lowest_index() {
        let out = run(
            ismember,
            &[row(&[2.0, 5.0, 1.0]), row(&[1.0, 2.0, 3.0, 2.0])],
            2,
        )
        .unwrap();
        assert_eq!(out[0].mat().unwrap().class, Class::Logical);
        assert_eq!(data(&out[0]), [1.0, 0.0, 1.0]);
        assert_eq!(data(&out[1]), [2.0, 0.0, 1.0]);
        let nan = run(ismember, &[num(f64::NAN), row(&[f64::NAN])], 1).unwrap();
        assert_eq!(data(&nan[0]), [0.0]);
        let out = run(ismember, &[Value::str("a"), cellstr(&["b", "a"])], 2).unwrap();
        assert_eq!((data(&out[0]), data(&out[1])), (vec![1.0], vec![2.0]));
        let out = run(ismember, &[cellstr(&["x", "b"]), cellstr(&["b", "a"])], 2).unwrap();
        assert_eq!(data(&out[1]), [0.0, 1.0]);
        // Two chars are compared letter by letter.
        let out = run(ismember, &[Value::str("abz"), Value::str("cab")], 1).unwrap();
        assert_eq!(data(&out[0]), [1.0, 1.0, 0.0]);
        let e = run(ismember, &[num(1.0), cellstr(&["a"])], 1)
            .unwrap_err()
            .msg;
        assert!(e.contains("can be combined only"), "{e}");
    }

    #[test]
    fn setdiff_intersect_and_union() {
        let a = row(&[5.0, 1.0, 3.0, 3.0, 7.0]);
        let b = row(&[3.0, 4.0, 5.0]);
        let d = run(setdiff, &[a.clone(), b.clone()], 2).unwrap();
        assert_eq!((data(&d[0]), data(&d[1])), (vec![1.0, 7.0], vec![2.0, 5.0]));
        let d = run(setdiff, &[a.clone(), b.clone(), Value::str("stable")], 1).unwrap();
        assert_eq!(data(&d[0]), [1.0, 7.0]);
        let i = run(intersect, &[a.clone(), b.clone()], 3).unwrap();
        assert_eq!(data(&i[0]), [3.0, 5.0]);
        assert_eq!(data(&i[1]), [3.0, 1.0]);
        assert_eq!(data(&i[2]), [1.0, 3.0]);
        let s = run(intersect, &[a.clone(), b.clone(), Value::str("stable")], 1).unwrap();
        assert_eq!(data(&s[0]), [5.0, 3.0]);
        let u = run(union, &[a.clone(), b.clone()], 3).unwrap();
        assert_eq!(data(&u[0]), [1.0, 3.0, 4.0, 5.0, 7.0]);
        assert_eq!(data(&u[1]), [2.0, 3.0, 1.0, 5.0]);
        assert_eq!(data(&u[2]), [2.0]);
        let u = run(union, &[a, b, Value::str("stable")], 1).unwrap();
        assert_eq!(data(&u[0]), [5.0, 1.0, 3.0, 7.0, 4.0]);
        // Orientation: a column in gives a column out.
        let c = Value::Mat(Matrix::col(vec![2.0, 5.0]));
        assert_eq!(
            run(union, &[c.clone(), row(&[3.0])], 1).unwrap()[0].dims(),
            (3, 1)
        );
        assert_eq!(
            run(setdiff, &[row(&[1.0, 2.0]), c], 1).unwrap()[0].dims(),
            (1, 1)
        );
        let e = run(setdiff, &[Value::Mat(Matrix::empty()), row(&[1.0])], 1).unwrap();
        assert_eq!(e[0].dims(), (1, 0));
    }

    #[test]
    fn the_two_input_functions_take_cells_of_char() {
        let d = run(setdiff, &[cellstr(&["a", "b", "c"]), cellstr(&["b"])], 1).unwrap();
        assert_eq!(words(&d[0]), ["a", "c"]);
        let i = run(intersect, &[cellstr(&["a", "b", "c"]), Value::str("b")], 1).unwrap();
        assert_eq!(words(&i[0]), ["b"]);
        let u = run(union, &[cellstr(&["c", "a"]), cellstr(&["b", "a"])], 1).unwrap();
        assert_eq!(words(&u[0]), ["a", "b", "c"]);
        let bad = Value::cell(CellArray::row(vec![Value::cell(CellArray::default())]));
        assert!(run(union, &[bad, cellstr(&["a"])], 1).is_err());
        assert!(run(intersect, &[cellstr(&["a"]), num(2.0)], 1).is_err());
    }
}
