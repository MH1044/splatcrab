//! Cells and structs (cycle 07): their constructors, the field functions,
//! `cellfun` beside `arrayfun`, and the conversions between cells and
//! arrays.

use std::collections::HashMap;
use std::rc::Rc;

use super::args::{at_most, check_shape, need, scalar, shape, string};
use super::{Registry, add, none, one, one_as};
use crate::bail;
use crate::error;
use crate::interp::{Callee, Interp, R, concat_rows, is_field_name, set_fields};
use crate::value::{CellArray, Class, Func, Matrix, StructArray, Value};

/// One line per builtin, as in `core.rs`.
#[rustfmt::skip]
pub fn register(r: &mut Registry) {
    add(r, "cell", cell, "cell(n), cell(r,c), cell(sz) - a cell array of empty matrices.");
    add(r, "struct", strukt, "struct('f1',v1,...) - a struct; cell values make a struct array of their size.");
    add(r, "fieldnames", fieldnames, "fieldnames(s) - the field names of s, as a column cell of char.");
    add(r, "isfield", isfield, "isfield(s,name) - true where name, or each name in a cell, is a field of s.");
    add(r, "rmfield", rmfield, "rmfield(s,name) - s without the field name, or without each field a cell names.");
    add(r, "getfield", getfield, "getfield(s,'f1','f2',...) - the value of s.f1.f2...");
    add(r, "setfield", setfield, "setfield(s,'f1',...,v) - a copy of s with s.f1... set to v, created if absent.");
    add(r, "iscell", iscell, "iscell(A) - true when A is a cell array.");
    add(r, "isstruct", isstruct, "isstruct(A) - true when A is a struct or struct array.");
    add(r, "cellfun", cellfun, "cellfun(f,C,...,'UniformOutput',tf) - call f on the contents of each element of C, ...");
    add(r, "num2cell", num2cell, "num2cell(A) - a cell of A's size holding each element of A.");
    add(r, "cell2mat", cell2mat, "cell2mat(C) - join the arrays a cell holds into one array.");
    add(r, "deal", deal, "[a,b,...] = deal(x) or deal(x1,x2,...) - copy the inputs to the outputs.");
}

/// `cell`, `cell(n)`, `cell(r, c)` or `cell([r c])`: that many `[]`. With
/// no size it is the 0x0 cell `{}`.
fn cell(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    if args.is_empty() {
        return one(Value::cell(CellArray::default()));
    }
    let (r, c) = shape(args, 0, "cell", 2)?;
    let (r, c) = check_shape(r, c)?;
    one(Value::cell(CellArray::blanks(r, c)))
}

/// `struct('f1', v1, 'f2', v2, ...)`, `struct()` (a 1x1 struct with no
/// fields) and `struct(s)` of a struct, which is `s`.
///
/// A value that is a cell makes a struct array of the cell's size, element
/// `k` taking the cell's element `k`: `struct('a', {1, 2})` is 1x2 and
/// `struct('a', {})` is 0x0 with the field `a`. A 1x1 cell is one value
/// for every element, which is how a field holds a cell,
/// `struct('a', {{1, 2}})`. Every cell that is not 1x1 must have one size.
/// Any other value goes into every element as it is. A field named twice
/// takes the later value in the earlier place.
///
/// Names already seen are found through a map, not a scan of the list: the
/// scan made `struct(c{:})` with 100,000 pairs take 81 s (cycle 07's review),
/// the same fault already fixed in `StructArray::field_index`.
fn strukt(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    if args.is_empty() {
        return one(Value::strukt(StructArray::scalar(Vec::new(), Vec::new())));
    }
    if let [v @ Value::Struct(_)] = args {
        return one(v.clone());
    }
    if args.len() % 2 != 0 {
        bail!(error::struct_pairs());
    }
    let mut fields: Vec<String> = Vec::new();
    let mut values: Vec<&Value> = Vec::new();
    let mut place: HashMap<String, usize> = HashMap::new();
    for (k, pair) in args.chunks(2).enumerate() {
        let name = string(args, 2 * k, "struct")?;
        if !is_field_name(&name) {
            bail!(error::invalid_field_name(&name));
        }
        match place.get(&name) {
            Some(&i) => values[i] = &pair[1],
            None => {
                place.insert(name.clone(), fields.len());
                fields.push(name);
                values.push(&pair[1]);
            }
        }
    }
    let mut dims: Option<(usize, usize)> = None;
    for v in &values {
        if let Value::Cell(c) = v {
            if c.numel() != 1 {
                match dims {
                    None => dims = Some((c.rows, c.cols)),
                    Some(d) if d != (c.rows, c.cols) => bail!(error::struct_cell_dims()),
                    Some(_) => {}
                }
            }
        }
    }
    let (rows, cols) = dims.unwrap_or((1, 1));
    let elems = (0..rows * cols)
        .map(|k| {
            values
                .iter()
                .map(|v| match v {
                    Value::Cell(c) if c.numel() == 1 => c.data[0].clone(),
                    Value::Cell(c) => c.data[k].clone(),
                    v => (*v).clone(),
                })
                .collect()
        })
        .collect();
    one(Value::strukt(StructArray::new(rows, cols, fields, elems)))
}

/// Argument `i` as a struct array.
fn struct_arg<'a>(args: &'a [Value], i: usize, name: &str) -> R<&'a Rc<StructArray>> {
    match args.get(i) {
        Some(Value::Struct(s)) => Ok(s),
        Some(_) => Err(error::arg_not_a_struct(i + 1, name)),
        None => Err(error::not_enough_args(name)),
    }
}

/// `fieldnames(s)`: the names in the order the fields were made, as an
/// Nx1 cell of char rows; 0x1 for a struct with no fields.
fn fieldnames(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    at_most(args, 1, "fieldnames")?;
    let s = struct_arg(args, 0, "fieldnames")?;
    let names: Vec<Value> = s.fields.iter().map(|f| Value::str(f)).collect();
    one(Value::cell(CellArray::new(names.len(), 1, names)))
}

/// `isfield(s, name)`: a logical, false for anything that is not a struct.
/// A cell of names gives a logical array of its size, an element that is
/// not text being false; any other second argument is false.
fn isfield(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "isfield")?;
    at_most(args, 2, "isfield")?;
    let has = |v: &Value| match (&args[0], v.text()) {
        (Value::Struct(s), Some(t)) => s.field_index(&t).is_some(),
        _ => false,
    };
    match &args[1] {
        Value::Cell(c) => {
            let data = c.data.iter().map(|v| has(v) as u8 as f64).collect();
            one_as(Matrix::new(c.rows, c.cols, data).with_class(Class::Logical))
        }
        v => one_as(Matrix::from_bool(has(v))),
    }
}

/// The names argument `i` gives: one char, or a cell of them.
fn names(args: &[Value], i: usize, name: &str) -> R<Vec<String>> {
    match args.get(i) {
        Some(Value::Cell(c)) => c
            .data
            .iter()
            .map(|v| v.text().ok_or_else(|| error::arg_not_a_string(i + 1, name)))
            .collect(),
        _ => Ok(vec![string(args, i, name)?]),
    }
}

/// `rmfield(s, name)`: `s` without the field, or without each field a cell
/// names; every one must be there.
fn rmfield(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "rmfield")?;
    at_most(args, 2, "rmfield")?;
    let s = struct_arg(args, 0, "rmfield")?;
    let gone = names(args, 1, "rmfield")?;
    let mut drop = vec![false; s.fields.len()];
    for g in &gone {
        let i = s.field_index(g).ok_or_else(|| error::no_such_field(g))?;
        drop[i] = true;
    }
    let keep: Vec<usize> = (0..s.fields.len()).filter(|&i| !drop[i]).collect();
    one(Value::strukt(StructArray::new(
        s.rows,
        s.cols,
        keep.iter().map(|&i| s.fields[i].clone()).collect(),
        s.elems
            .iter()
            .map(|e| keep.iter().map(|&i| e[i].clone()).collect())
            .collect(),
    )))
}

/// `getfield(s, 'f1', 'f2', ...)`: `s.f1.f2...`, which must be one value
/// at every step, as the expression's would.
fn getfield(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 2, "getfield")?;
    let mut v = args[0].clone();
    for i in 1..args.len() {
        let f = string(args, i, "getfield")?;
        let Value::Struct(s) = &v else {
            bail!(error::dot_indexing_unsupported());
        };
        if s.numel() != 1 {
            bail!(error::cs_list_count(s.numel()));
        }
        let k = s.field_index(&f).ok_or_else(|| error::no_such_field(&f))?;
        let next = s.elems[0][k].clone();
        v = next;
    }
    one(v)
}

/// `setfield(s, 'f1', ..., v)`: a copy of `s` with `s.f1...` set to `v`,
/// every field on the way created if it is not there, as the assignment
/// `s.f1... = v` would. `s` itself is unchanged.
fn setfield(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 3, "setfield")?;
    let last = args.len() - 1;
    let fields = (1..last)
        .map(|i| string(args, i, "setfield"))
        .collect::<R<Vec<String>>>()?;
    let mut out = args[0].clone();
    set_fields(&mut out, &fields, args[last].clone())?;
    one(out)
}

fn iscell(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "iscell")?;
    at_most(args, 1, "iscell")?;
    one_as(Matrix::from_bool(matches!(args[0], Value::Cell(_))))
}

fn isstruct(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "isstruct")?;
    at_most(args, 1, "isstruct")?;
    one_as(Matrix::from_bool(matches!(args[0], Value::Struct(_))))
}

/// `num2cell(A)`: a cell of `A`'s size whose element `k` is element `k` of
/// `A` as a value of its own: a scalar of a matrix, a 1x1 cell of a cell,
/// a 1x1 struct of a struct array.
fn num2cell(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "num2cell")?;
    at_most(args, 1, "num2cell")?;
    let v = &args[0];
    let (r, c) = v.dims();
    let data = (0..r * c).map(|k| v.element(k)).collect();
    one(Value::cell(CellArray::new(r, c, data)))
}

/// `cell2mat(C)`: each row of the cell joined as `[a, b, ...]` would join
/// it, and the rows then stacked, so the arrays must fit together as a
/// bracket's would. Every element must be an array; `{}` gives `[]`.
fn cell2mat(_: &mut Interp, args: &[Value], _: usize) -> R<Vec<Value>> {
    need(args, 1, "cell2mat")?;
    at_most(args, 1, "cell2mat")?;
    let Value::Cell(c) = &args[0] else {
        bail!(error::arg_not_a_cell(1, "cell2mat"));
    };
    if c.data.iter().any(|v| !matches!(v, Value::Mat(_))) {
        bail!(error::cell2mat_contents());
    }
    if c.data.is_empty() {
        return one(Value::Mat(Matrix::empty()));
    }
    let rows = (0..c.rows)
        .map(|r| {
            (0..c.cols)
                .map(|j| c.data[j * c.rows + r].clone())
                .collect()
        })
        .collect();
    one(concat_rows(rows)?)
}

/// `[a, b, ...] = deal(x)` copies `x` to every output, and
/// `deal(x1, x2, ...)` gives one input per output; anything else is the
/// count error. Asked for no output, it is asked for one.
fn deal(_: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    need(args, 1, "deal")?;
    let n = nargout.max(1);
    match args.len() {
        1 => Ok(vec![args[0].clone(); n]),
        k if k == n => Ok(args.to_vec()),
        _ => Err(error::deal_count()),
    }
}

fn cellfun(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
    map_elements(it, args, nargout, "cellfun")
}

/// `arrayfun` (cycle 06) and `cellfun` (cycle 07): `f` called on the
/// elements at each position of the inputs in turn, in column-major order,
/// and asked for as many outputs as the builtin was.
///
/// `arrayfun` hands `f` each element as a value of its own (a scalar of a
/// matrix, a 1x1 cell of a cell, a 1x1 struct of a struct array) and
/// `cellfun` the contents of each element of its cells, which must all be
/// cells. `f` is a function handle, or for `cellfun` a function's name,
/// called as `feval` would call it. Every input must have the first one's
/// size.
///
/// A trailing `'UniformOutput', tf`, the name in any case, chooses the
/// outputs. True, the default: every result must be a scalar, and each
/// output is an array of the inputs' size, of the results' class when
/// they all share one and double otherwise. False: each output is a cell
/// of the inputs' size holding the results as they are.
///
/// Asked for no output, as a statement asks, `f` is asked for none too, and
/// a result it gives anyway becomes the one output. Each call goes through
/// `call_nested`, so a function that calls `cellfun` that calls it stays
/// bounded.
pub(crate) fn map_elements(
    it: &mut Interp,
    args: &[Value],
    nargout: usize,
    name: &str,
) -> R<Vec<Value>> {
    let cells = name == "cellfun";
    need(args, 2, name)?;
    let named: Option<String> = match &args[0] {
        Value::Func(_) => None,
        v if cells && v.is_char() => v.text(),
        _ => bail!(error::arg_not_a_handle(1, name)),
    };
    let handle: Option<Rc<Func>> = match &args[0] {
        Value::Func(f) => Some(f.clone()),
        _ => None,
    };
    // The options start at the first `'UniformOutput'` after the first
    // input; everything before it is an input.
    let is_option = |v: &Value| {
        v.text()
            .is_some_and(|t| t.eq_ignore_ascii_case("UniformOutput"))
    };
    let end = (2..args.len())
        .find(|&i| is_option(&args[i]))
        .unwrap_or(args.len());
    let mut uniform = true;
    let mut i = end;
    while i < args.len() {
        if !is_option(&args[i]) {
            bail!(error::arg_not_a_string(i + 1, name));
        }
        uniform = scalar(args, i + 1, name)? != 0.0;
        i += 2;
    }
    let inputs = &args[1..end];
    if cells {
        if let Some(k) = inputs.iter().position(|v| !matches!(v, Value::Cell(_))) {
            bail!(error::arg_not_a_cell(k + 2, name));
        }
    }
    let (rows, cols) = inputs[0].dims();
    if inputs.iter().any(|v| v.dims() != (rows, cols)) {
        bail!(error::arrayfun_size());
    }
    let outs = nargout.max(1);
    let mut results: Vec<Vec<Value>> = vec![Vec::with_capacity(rows * cols); outs];
    // Whether `f` gives values; a statement's call finds out from the
    // first call.
    let mut gives = nargout > 0;
    for k in 0..rows * cols {
        let elems = inputs
            .iter()
            .map(|v| match v {
                Value::Cell(c) if cells => c.data[k].clone(),
                v => v.element(k),
            })
            .collect();
        let callee = match (&handle, &named) {
            (Some(f), _) => Callee::Handle(f),
            (None, Some(n)) => Callee::Name(n),
            (None, None) => bail!(error::arg_not_a_handle(1, name)),
        };
        let vals = it.call_nested(callee, elems, nargout)?;
        if k == 0 && nargout == 0 {
            gives = !vals.is_empty();
        }
        if !gives {
            continue;
        }
        if vals.len() < outs {
            bail!(error::too_many_outputs());
        }
        for (o, v) in vals.into_iter().take(outs).enumerate() {
            if uniform {
                let ok = v.mat()?.is_scalar();
                if !ok {
                    bail!(error::arrayfun_nonscalar(k + 1, o + 1));
                }
            }
            results[o].push(v);
        }
    }
    if !gives && rows * cols > 0 {
        return none();
    }
    results
        .into_iter()
        .map(|vals| {
            if !uniform {
                return Ok(Value::cell(CellArray::new(rows, cols, vals)));
            }
            let mut class: Option<Class> = None;
            let mut data = Vec::with_capacity(vals.len());
            // A complex result makes the output complex (cycle 10), stored
            // by the flag rule; it is never read by its real part alone.
            let mut im = Vec::with_capacity(vals.len());
            for v in &vals {
                let m = v.mat()?;
                class = match class {
                    None => Some(m.class),
                    Some(c) if c == m.class => Some(c),
                    Some(_) => Some(Class::Double),
                };
                let z = m.c(0);
                data.push(z.re);
                im.push(z.im);
            }
            let class = class.unwrap_or_default();
            let m = Matrix::new(rows, cols, data).with_im(Some(im));
            let m = if m.is_complex() {
                m
            } else {
                m.with_class(class)
            };
            Ok(Value::Mat(m))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::BuiltinFn;

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    fn call(f: BuiltinFn, args: &[Value], nargout: usize) -> R<Vec<Value>> {
        let mut it = Interp::with_output(Box::new(std::io::sink()));
        f(&mut it, args, nargout)
    }

    fn one_of(f: BuiltinFn, args: &[Value]) -> Value {
        call(f, args, 1).unwrap().remove(0)
    }

    fn cell_of(v: &Value) -> &CellArray {
        match v {
            Value::Cell(c) => c,
            v => panic!("not a cell: {}", v.class_name()),
        }
    }

    fn struct_of(v: &Value) -> &StructArray {
        match v {
            Value::Struct(s) => s,
            v => panic!("not a struct: {}", v.class_name()),
        }
    }

    fn row(vals: &[Value]) -> Value {
        Value::cell(CellArray::row(vals.to_vec()))
    }

    #[test]
    fn cell_makes_empties_of_the_size_asked() {
        let c = one_of(cell, &[num(1.0), num(3.0)]);
        assert_eq!(
            (c.dims(), cell_of(&c).data.iter().all(Value::is_blank)),
            ((1, 3), true)
        );
        assert_eq!(one_of(cell, &[num(2.0)]).dims(), (2, 2));
        assert_eq!(one_of(cell, &[]).dims(), (0, 0));
        assert!(call(cell, &[num(1e10)], 1).is_err());
    }

    #[test]
    fn struct_makes_a_struct_or_a_struct_array_from_cells() {
        let s = one_of(
            strukt,
            &[Value::str("a"), num(1.0), Value::str("b"), Value::str("x")],
        );
        let st = struct_of(&s);
        assert_eq!(
            ((st.rows, st.cols), st.fields.clone()),
            ((1, 1), vec!["a".into(), "b".into()])
        );
        // A cell value is one element per cell element; a 1x1 cell is the
        // one value of every element.
        let arr = one_of(
            strukt,
            &[
                Value::str("a"),
                row(&[num(1.0), num(2.0)]),
                Value::str("b"),
                row(&[num(9.0)]),
            ],
        );
        let st = struct_of(&arr);
        assert_eq!((st.rows, st.cols), (1, 2));
        assert!(matches!(&st.elems[1][0], Value::Mat(m) if m.data == [2.0]));
        assert!(matches!(&st.elems[1][1], Value::Mat(m) if m.data == [9.0]));
        let none = one_of(
            strukt,
            &[Value::str("a"), Value::cell(CellArray::default())],
        );
        assert_eq!((none.dims(), struct_of(&none).fields.len()), ((0, 0), 1));
        let holds = one_of(
            strukt,
            &[Value::str("a"), row(&[row(&[num(1.0), num(2.0)])])],
        );
        assert!(matches!(&struct_of(&holds).elems[0][0], Value::Cell(c) if c.numel() == 2));
        assert_eq!(one_of(strukt, &[]).dims(), (1, 1));
        assert_eq!(
            call(strukt, &[Value::str("a")], 1).unwrap_err().msg,
            error::struct_pairs().msg
        );
        assert_eq!(
            call(strukt, &[Value::str("1a"), num(1.0)], 1)
                .unwrap_err()
                .msg,
            error::invalid_field_name("1a").msg
        );
        assert_eq!(
            call(
                strukt,
                &[
                    Value::str("a"),
                    row(&[num(1.0), num(2.0)]),
                    Value::str("b"),
                    row(&[num(1.0), num(2.0), num(3.0)])
                ],
                1
            )
            .unwrap_err()
            .msg,
            error::struct_cell_dims().msg
        );
        // A field named twice keeps its first place and its last value.
        let twice = one_of(
            strukt,
            &[Value::str("a"), num(1.0), Value::str("a"), num(2.0)],
        );
        let st = struct_of(&twice);
        assert_eq!(st.fields.len(), 1);
        assert!(matches!(&st.elems[0][0], Value::Mat(m) if m.data == [2.0]));
    }

    #[test]
    fn the_field_functions() {
        let s = one_of(
            strukt,
            &[Value::str("a"), num(1.0), Value::str("b"), num(2.0)],
        );
        let f = one_of(fieldnames, std::slice::from_ref(&s));
        assert_eq!(f.dims(), (2, 1));
        assert_eq!(cell_of(&f).data[1].text().as_deref(), Some("b"));
        let yes = |args: &[Value]| one_of(isfield, args);
        assert!(
            matches!(yes(&[s.clone(), Value::str("a")]), Value::Mat(m) if m.data == [1.0] && m.class == Class::Logical)
        );
        assert!(matches!(yes(&[num(1.0), Value::str("a")]), Value::Mat(m) if m.data == [0.0]));
        assert!(
            matches!(yes(&[s.clone(), row(&[Value::str("b"), Value::str("z"), num(1.0)])]), Value::Mat(m) if m.data == [1.0, 0.0, 0.0])
        );
        let r = one_of(rmfield, &[s.clone(), Value::str("a")]);
        assert_eq!(struct_of(&r).fields, ["b"]);
        assert_eq!(
            call(rmfield, &[s.clone(), Value::str("zz")], 1)
                .unwrap_err()
                .msg,
            error::no_such_field("zz").msg
        );
        assert!(
            matches!(one_of(getfield, &[s.clone(), Value::str("b")]), Value::Mat(m) if m.data == [2.0])
        );
        let set = one_of(
            setfield,
            &[s.clone(), Value::str("c"), Value::str("d"), num(5.0)],
        );
        let inner = &struct_of(&set).elems[0][2];
        assert!(
            matches!(one_of(getfield, &[inner.clone(), Value::str("d")]), Value::Mat(m) if m.data == [5.0])
        );
        // The argument is unchanged.
        assert_eq!(struct_of(&s).fields.len(), 2);
        assert_eq!(
            call(fieldnames, &[num(1.0)], 1).unwrap_err().msg,
            error::arg_not_a_struct(1, "fieldnames").msg
        );
        let is =
            |f: BuiltinFn, v: Value| matches!(one_of(f, &[v]), Value::Mat(m) if m.data == [1.0]);
        assert!(is(iscell, row(&[])) && !is(iscell, s.clone()));
        assert!(is(isstruct, s.clone()) && !is(isstruct, num(1.0)));
    }

    #[test]
    fn the_conversions_and_deal() {
        let m = Value::Mat(Matrix::new(2, 2, vec![1.0, 3.0, 2.0, 4.0]));
        let c = one_of(num2cell, std::slice::from_ref(&m));
        assert_eq!(c.dims(), (2, 2));
        assert!(matches!(&cell_of(&c).data[1], Value::Mat(x) if x.data == [3.0]));
        let back = one_of(cell2mat, &[c]);
        assert!(matches!(back, Value::Mat(x) if x.rows == 2 && x.data == [1.0, 3.0, 2.0, 4.0]));
        // Rows of arrays of different widths join as a bracket would.
        let parts = Value::cell(CellArray::new(
            2,
            1,
            vec![
                Value::Mat(Matrix::row(vec![1.0, 2.0])),
                Value::Mat(Matrix::row(vec![3.0, 4.0])),
            ],
        ));
        assert!(matches!(one_of(cell2mat, &[parts]), Value::Mat(x) if x.rows == 2 && x.cols == 2));
        assert!(matches!(one_of(cell2mat, &[row(&[])]), Value::Mat(x) if x.data.is_empty()));
        assert_eq!(
            call(cell2mat, &[row(&[row(&[num(1.0)])])], 1)
                .unwrap_err()
                .msg,
            error::cell2mat_contents().msg
        );
        assert_eq!(call(deal, &[num(7.0)], 3).unwrap().len(), 3);
        assert_eq!(call(deal, &[num(1.0), num(2.0)], 2).unwrap().len(), 2);
        assert_eq!(
            call(deal, &[num(1.0), num(2.0)], 3).unwrap_err().msg,
            error::deal_count().msg
        );
        assert_eq!(call(deal, &[num(1.0)], 0).unwrap().len(), 1);
    }

    #[test]
    fn cellfun_and_arrayfun_map_each_element() {
        let numel_of = Value::str("numel");
        let c = row(&[Value::str("ab"), Value::str("cde"), Value::str("")]);
        let r = one_of(cellfun, &[numel_of.clone(), c.clone()]);
        assert!(matches!(r, Value::Mat(m) if m.data == [2.0, 3.0, 0.0] && m.rows == 1));
        let off = [
            Value::str("UniformOutput"),
            Value::Mat(Matrix::from_bool(false)),
        ];
        let mut args = vec![numel_of.clone(), c.clone()];
        args.extend(off.iter().cloned());
        let r = one_of(cellfun, &args);
        assert_eq!((r.class_name(), r.dims()), ("cell", (1, 3)));
        // The option's name in any case.
        args[2] = Value::str("uniformoutput");
        assert_eq!(one_of(cellfun, &args).class_name(), "cell");
        assert_eq!(
            call(cellfun, &[numel_of.clone(), num(1.0)], 1)
                .unwrap_err()
                .msg,
            error::arg_not_a_cell(2, "cellfun").msg
        );
        assert_eq!(
            call(cellfun, &[numel_of.clone(), c.clone(), row(&[num(1.0)])], 1)
                .unwrap_err()
                .msg,
            error::arrayfun_size().msg
        );
        // arrayfun over a matrix, with 'UniformOutput', false.
        let mut args = vec![numel_of, Value::Mat(Matrix::row(vec![1.0, 2.0]))];
        args.extend(off.iter().cloned());
        assert_eq!(
            call(map_elements_arrayfun, &args, 1).unwrap_err().msg,
            error::arg_not_a_handle(1, "arrayfun").msg
        );
    }

    /// `arrayfun`'s entry, for the test above: the same map, by that name.
    fn map_elements_arrayfun(it: &mut Interp, args: &[Value], nargout: usize) -> R<Vec<Value>> {
        map_elements(it, args, nargout, "arrayfun")
    }
}
