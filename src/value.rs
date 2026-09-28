//! Runtime values: a column-major matrix of doubles carrying a class tag.
//!
//! MATLAB's `double`, `logical` and `char` are one storage type here, an
//! `f64` per element, told apart by [`Class`]. A logical element is `0` or
//! `1`; a char element is one UTF-16 code unit, `0` to `65535`, as in MATLAB,
//! so `length('😀')` is `2`.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::bail;
use crate::error::{self, R};
use crate::interp::Unit;
use crate::parser::{AnonFn, Function};

/// The class of an array. Storage is the same for all three; the tag decides
/// how the array displays, what `class` says, and how an operation classes
/// its result. Only the constructors of results decide it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Class {
    #[default]
    Double,
    Logical,
    Char,
}

impl Class {
    /// The name `class` returns.
    pub fn name(self) -> &'static str {
        match self {
            Class::Double => "double",
            Class::Logical => "logical",
            Class::Char => "char",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    /// Column-major: element (r, c) lives at data[c * rows + r].
    pub data: Vec<f64>,
    /// How the elements are to be read. Every constructor below makes a
    /// `Double`; a result of another class says so with [`Matrix::with_class`]
    /// or [`Matrix::to_class`].
    pub class: Class,
}

/// A value the interpreter can hold. Cycle 02 removed `Str`: text is a
/// `Char` matrix. The containers joined in cycle 07.
#[derive(Clone, Debug)]
pub enum Value {
    Mat(Matrix),
    /// The `MException` a `catch e` binds (cycle 04): the error that was
    /// caught, whole, so that `rethrow(e)` raises it again unchanged, its
    /// line included. `e.message` and `e.identifier` read its two texts and
    /// `e.stack` (cycle 07) its trace as a struct array. It is a value of
    /// its own, not an array, so every array operation refuses it with
    /// [`error::not_an_array`].
    Exception(error::MError),
    /// A function handle (cycle 06), shared: reading the variable that
    /// holds one copies a pointer, not its captured workspace.
    Func(Rc<Func>),
    /// A cell array (cycle 07). Shared and copied on write, like a handle:
    /// reading a variable clones its value, and `c = {c}` in a loop must
    /// not copy the whole nest each time, or it would be quadratic and the
    /// copy itself would recurse once per level.
    Cell(Rc<CellArray>),
    /// A struct or struct array (cycle 07), shared and copied on write for
    /// the same reasons as a cell.
    Struct(Rc<StructArray>),
}

/// The class name an `MException` reports.
pub const EXCEPTION_CLASS: &str = "MException";

/// The class name a function handle reports.
pub const FUNC_CLASS: &str = "function_handle";

/// The class name a cell array reports.
pub const CELL_CLASS: &str = "cell";

/// The class name a struct array reports.
pub const STRUCT_CLASS: &str = "struct";

/// A cell array (cycle 07): `rows x cols` values of any kind, stored
/// column-major exactly as a [`Matrix`] stores its elements, so linear
/// indexing, `c(:)` and growth follow the same rules and reuse the same
/// index resolution in `interp.rs`. A new element is the 0x0 double `[]`.
#[derive(Clone, Debug, Default)]
pub struct CellArray {
    pub rows: usize,
    pub cols: usize,
    /// Column-major: element (r, c) lives at data[c * rows + r].
    pub data: Vec<Value>,
}

/// A struct array (cycle 07): `rows x cols` elements that all have the same
/// fields, in the order they were first assigned. A 1x1 struct array is
/// what MATLAB calls a struct. Elements are stored column-major, each as
/// one value per field in the order of `fields`.
#[derive(Clone, Debug, Default)]
pub struct StructArray {
    pub rows: usize,
    pub cols: usize,
    pub fields: Vec<String>,
    /// Column-major; `elems[k][f]` is field `fields[f]` of element `k`.
    pub elems: Vec<Vec<Value>>,
    /// Where each name is in `fields`, built on the first lookup once there
    /// are [`INDEXED_FIELDS`] fields or more, so that adding fields one at a
    /// time in a loop is not quadratic. Only [`StructArray::new`],
    /// [`StructArray::scalar`] and [`StructArray::ensure_field`] make or add
    /// fields, and the last keeps the index up to date.
    index: OnceCell<HashMap<String, usize>>,
}

/// The field count from which [`StructArray::field_index`] looks a name up
/// in a hash index rather than walking the names.
const INDEXED_FIELDS: usize = 32;

/// The value a new element of a cell, a new field or a new struct element
/// holds: the 0x0 double `[]`.
pub fn blank() -> Value {
    Value::Mat(Matrix::empty())
}

impl CellArray {
    pub fn new(rows: usize, cols: usize, data: Vec<Value>) -> CellArray {
        debug_assert_eq!(rows * cols, data.len());
        CellArray { rows, cols, data }
    }

    /// A 1-row cell of `data`; no values is the 0x0 cell `{}`.
    pub fn row(data: Vec<Value>) -> CellArray {
        match data.len() {
            0 => CellArray::default(),
            n => CellArray::new(1, n, data),
        }
    }

    pub fn numel(&self) -> usize {
        self.data.len()
    }

    /// `rows x cols` of `[]`, which is what `cell(r, c)` makes.
    pub fn blanks(rows: usize, cols: usize) -> CellArray {
        CellArray::new(rows, cols, (0..rows * cols).map(|_| blank()).collect())
    }
}

impl StructArray {
    /// A 1x1 struct with these fields and values.
    pub fn scalar(fields: Vec<String>, values: Vec<Value>) -> StructArray {
        debug_assert_eq!(fields.len(), values.len());
        StructArray::new(1, 1, fields, vec![values])
    }

    /// A `rows x cols` struct array of `elems`, column-major, each holding
    /// one value per field.
    pub fn new(
        rows: usize,
        cols: usize,
        fields: Vec<String>,
        elems: Vec<Vec<Value>>,
    ) -> StructArray {
        debug_assert_eq!(rows * cols, elems.len());
        StructArray {
            rows,
            cols,
            fields,
            elems,
            index: OnceCell::new(),
        }
    }

    /// Where `name` is among the fields: the first place it has.
    pub fn field_index(&self, name: &str) -> Option<usize> {
        if self.fields.len() < INDEXED_FIELDS {
            return self.fields.iter().position(|f| f == name);
        }
        let index = self.index.get_or_init(|| {
            let mut m = HashMap::with_capacity(self.fields.len());
            for (i, f) in self.fields.iter().enumerate() {
                m.entry(f.clone()).or_insert(i);
            }
            m
        });
        index.get(name).copied()
    }

    pub fn numel(&self) -> usize {
        self.elems.len()
    }

    /// Element `k` as a 1x1 struct of its own.
    pub fn element(&self, k: usize) -> StructArray {
        StructArray::scalar(self.fields.clone(), self.elems[k].clone())
    }

    /// Adds a field holding `[]` in every element, if it is not there yet,
    /// and returns where it is.
    pub fn ensure_field(&mut self, name: &str) -> usize {
        if let Some(f) = self.field_index(name) {
            return f;
        }
        let f = self.fields.len();
        self.fields.push(name.to_string());
        if let Some(index) = self.index.get_mut() {
            index.insert(name.to_string(), f);
        }
        for e in &mut self.elems {
            e.push(blank());
        }
        f
    }

    /// The same fields as `other`, in any order.
    pub fn same_fields(&self, other: &StructArray) -> bool {
        self.fields.len() == other.fields.len()
            && self.fields.iter().all(|f| other.field_index(f).is_some())
    }

    /// Element `k` of `other` with its values in this array's field order;
    /// the caller has checked [`same_fields`](StructArray::same_fields).
    pub fn reordered(&self, other: &StructArray, k: usize) -> Vec<Value> {
        self.fields
            .iter()
            .map(|f| match other.field_index(f) {
                Some(i) => other.elems[k][i].clone(),
                None => blank(),
            })
            .collect()
    }
}

/// True for a value that holds other values, and so could start a chain
/// that must be freed without recursion.
fn holds_values(v: &Value) -> bool {
    matches!(v, Value::Cell(_) | Value::Struct(_) | Value::Func(_))
}

/// Frees values with a worklist rather than by recursion (cycles 06 and 07).
///
/// A cell can hold a cell, a struct a struct, a handle can capture either
/// and either can hold a handle: `c = {c}` or `h = @() c; c = {h}` 500,000
/// times builds a chain that the default drop freed one stack frame per
/// link, overflowing the stack (exit 134). Here each container this is the
/// last owner of is opened and what it holds is moved onto the worklist
/// before it is dropped, so no drop ever recurses more than one level. A
/// container still shared elsewhere is only released: its count goes down
/// and nothing else happens.
fn free(mut pending: Vec<Value>) {
    while let Some(v) = pending.pop() {
        match v {
            Value::Cell(rc) => {
                if let Ok(mut c) = Rc::try_unwrap(rc) {
                    pending.extend(c.data.drain(..).filter(holds_values));
                }
            }
            Value::Struct(rc) => {
                if let Ok(mut s) = Rc::try_unwrap(rc) {
                    for e in s.elems.drain(..) {
                        pending.extend(e.into_iter().filter(holds_values));
                    }
                }
            }
            Value::Func(rc) => {
                if let Ok(Func::Anon { captured, .. }) = Rc::try_unwrap(rc).as_mut() {
                    pending.extend(captured.drain(..).map(|(_, v)| v).filter(holds_values));
                }
            }
            // A matrix or an `MException` holds no values.
            _ => {}
        }
        // Whatever was opened above now holds nothing that nests, so the
        // drop at the end of this iteration does not recurse.
    }
}

impl Drop for CellArray {
    fn drop(&mut self) {
        if self.data.iter().any(holds_values) {
            free(std::mem::take(&mut self.data));
        }
    }
}

impl Drop for StructArray {
    fn drop(&mut self) {
        if self.elems.iter().flatten().any(holds_values) {
            free(
                std::mem::take(&mut self.elems)
                    .into_iter()
                    .flatten()
                    .collect(),
            );
        }
    }
}

/// A function handle: what `@name` and `@(x) body` evaluate to (cycle 06).
///
/// Like an `MException` it is one object and not an array, so every array
/// operation refuses it through [`Value::mat`]; calling it is the one thing
/// it offers.
#[derive(Debug)]
pub enum Func {
    /// `@name`. `local` is the local function the name resolved to where the
    /// handle was made, by invariant 4's order (the running file's local
    /// functions, then the script's), with the file it belongs to: such a
    /// handle keeps calling that function wherever it is called from. With
    /// no local function of the name, it is resolved when called, against
    /// the path and then the builtins, never against the local functions
    /// of wherever it has been passed to.
    Named {
        name: String,
        local: Option<(Rc<Unit>, Rc<Function>)>,
    },
    /// `@(params) body`: the definition, the variables it captured when it
    /// was made (a snapshot of each name the body reads that was a variable
    /// then), and the file it was made in, whose local functions its body
    /// calls first.
    Anon {
        def: Rc<AnonFn>,
        captured: Vec<(String, Value)>,
        unit: Rc<Unit>,
    },
}

/// Frees a chain of handles with the worklist of [`free`] rather than by
/// recursion.
///
/// An anonymous function holds the variables it captured, and one of them
/// can be a handle holding its own captures, and so on: `for k = 1:500000,
/// h = @() h() + 1; end` builds a chain half a million deep, and the default
/// drop freed it one stack frame per link and overflowed the stack (exit 134,
/// cycle 06's review). Cycle 07 extended the worklist to cells and structs,
/// which can hold handles and be captured by them.
impl Drop for Func {
    fn drop(&mut self) {
        if let Func::Anon { captured, .. } = self {
            if captured.iter().any(|(_, v)| holds_values(v)) {
                free(captured.drain(..).map(|(_, v)| v).collect());
            }
        }
    }
}

impl Func {
    /// What `func2str` returns: the name of a named handle, and the
    /// rendered text of an anonymous one, `@(x)x+1`.
    pub fn text(&self) -> String {
        match self {
            Func::Named { name, .. } => name.clone(),
            Func::Anon { def, .. } => def.text(),
        }
    }

    /// The handle as its display shows it: `@sin`, or `@(x)x+1`.
    pub fn shown(&self) -> String {
        match self {
            Func::Named { name, .. } => format!("@{}", name),
            Func::Anon { def, .. } => def.text(),
        }
    }
}

impl Value {
    /// The matrix this value is, or the refusal for a value that is not one.
    pub fn into_mat(self) -> R<Matrix> {
        match self {
            Value::Mat(m) => Ok(m),
            v => Err(error::not_an_array(v.class_name())),
        }
    }

    /// The matrix this value is, borrowed; see [`Value::into_mat`].
    pub fn mat(&self) -> R<&Matrix> {
        match self {
            Value::Mat(m) => Ok(m),
            v => Err(error::not_an_array(v.class_name())),
        }
    }

    /// A char row holding `s` as UTF-16 code units; see [`Matrix::char_row`].
    pub fn str(s: &str) -> Value {
        Value::Mat(Matrix::char_row(s))
    }

    pub fn is_char(&self) -> bool {
        matches!(self, Value::Mat(m) if m.class == Class::Char)
    }

    /// The text of a char value, and `None` for any other class.
    pub fn text(&self) -> Option<String> {
        match self {
            Value::Mat(m) if m.class == Class::Char => Some(m.text()),
            _ => None,
        }
    }

    /// What `class` returns.
    pub fn class_name(&self) -> &'static str {
        match self {
            Value::Mat(m) => m.class.name(),
            Value::Exception(_) => EXCEPTION_CLASS,
            Value::Func(_) => FUNC_CLASS,
            Value::Cell(_) => CELL_CLASS,
            Value::Struct(_) => STRUCT_CLASS,
        }
    }

    /// Rows and columns; an `MException` and a function handle are each
    /// one object, 1x1.
    pub fn dims(&self) -> (usize, usize) {
        match self {
            Value::Mat(m) => (m.rows, m.cols),
            Value::Exception(_) | Value::Func(_) => (1, 1),
            Value::Cell(c) => (c.rows, c.cols),
            Value::Struct(s) => (s.rows, s.cols),
        }
    }

    /// How many elements: `rows * cols` of [`dims`](Value::dims).
    pub fn numel(&self) -> usize {
        let (r, c) = self.dims();
        r * c
    }

    /// True for the 0x0 double `[]`, which an assignment or a
    /// concatenation treats as "nothing here yet": `x = []; x.a = 1` makes
    /// a struct and `[[] {1}]` is a cell.
    pub fn is_blank(&self) -> bool {
        matches!(self, Value::Mat(m) if m.class == Class::Double && m.rows == 0 && m.cols == 0)
    }

    /// Element `k`, linear and zero-based, as a value of its own: a scalar
    /// of a matrix's class, a 1x1 cell of a cell, a 1x1 struct of a struct
    /// array, and the value itself for a handle or an `MException`. What
    /// `arrayfun` hands its function.
    pub fn element(&self, k: usize) -> Value {
        match self {
            Value::Mat(m) => Value::Mat(Matrix::scalar(m.data[k]).with_class(m.class)),
            Value::Cell(c) => Value::Cell(Rc::new(CellArray::new(1, 1, vec![c.data[k].clone()]))),
            Value::Struct(s) => Value::Struct(Rc::new(s.element(k))),
            v => v.clone(),
        }
    }

    /// A cell value.
    pub fn cell(c: CellArray) -> Value {
        Value::Cell(Rc::new(c))
    }

    /// A struct value.
    pub fn strukt(s: StructArray) -> Value {
        Value::Struct(Rc::new(s))
    }

    /// `name =`, a blank line, the display body and a closing blank line.
    /// A struct's first line is `name = `, with the space after the `=`
    /// that the spec records for MATLAB's struct display (cycle 07).
    pub fn display(&self, name: &str) -> String {
        let eq = if matches!(self, Value::Struct(_)) {
            "= "
        } else {
            "="
        };
        format!("{} {}\n\n{}\n", name, eq, self.display_body())
    }

    /// What follows `x =` and its blank line.
    ///
    /// An `MException` shows SplatCrab's own one-line form rather than
    /// MATLAB's property listing: `  MException: boom`, or
    /// `  MException (a:b): boom` when it has an identifier.
    ///
    /// A function handle shows MATLAB's header and then the handle,
    /// indented four: `  function_handle with value:`, a blank line,
    /// `    @(x)x+1` (cycle 06).
    pub fn display_body(&self) -> String {
        match self {
            Value::Mat(m) => m.display_body(),
            Value::Exception(e) => exception_line(e),
            Value::Func(f) => format!("  {} with value:\n\n    {}\n", FUNC_CLASS, f.shown()),
            Value::Cell(c) => cell_body(c),
            Value::Struct(s) => struct_body(s),
        }
    }

    /// What `disp` prints: a matrix's [`Matrix::disp_text`], for an
    /// `MException` the same line its named display shows, and for a
    /// function handle the handle as its display shows it, unindented:
    /// `@(x)x+1`, which is its `func2str` text, and `@sin`. A cell prints
    /// its rows without the header, and a struct its field lines.
    pub fn disp_text(&self) -> String {
        match self {
            Value::Mat(m) => m.disp_text(),
            Value::Exception(e) => exception_line(e),
            Value::Func(f) => format!("{}\n", f.shown()),
            Value::Cell(c) => cell_rows(c),
            Value::Struct(s) if s.numel() == 1 => field_lines(s),
            Value::Struct(s) => struct_body(s),
        }
    }
}

/// `r×c`, the size as every container display writes it.
fn size_text(r: usize, c: usize) -> String {
    format!("{r}×{c}")
}

/// A named display's body for a cell (cycle 07): `  1×2 cell array`, a
/// blank line and the rows, or `  0×0 empty cell array` for an empty.
fn cell_body(c: &CellArray) -> String {
    if c.data.is_empty() {
        return format!("  {} empty cell array\n", size_text(c.rows, c.cols));
    }
    format!(
        "  {} cell array\n\n{}",
        size_text(c.rows, c.cols),
        cell_rows(c)
    )
}

/// One element of a cell as its display shows it, and the character
/// position where the padding that aligns its column goes.
///
/// Each element is summarised on one line and never expanded, which is the
/// rule that keeps the display of a deeply nested cell bounded: a cell
/// inside a cell is `{1×2 cell}`, whatever it holds. A numeric or logical
/// scalar is `{[1]}`, padded inside the brackets so numbers right-align;
/// a 1-row char is `{'ab'}`; a handle is `{@(x)x+1}`; everything else is
/// its size and class, `{1×2 double}`, `{0×0 char}`, `{1×1 struct}`, and
/// is padded before its closing brace.
fn cell_element(v: &Value) -> (String, usize) {
    let text = match v {
        Value::Mat(m) if m.is_scalar() && m.class != Class::Char => {
            let t = format!("{{[{}]}}", m.format().trim());
            return (t, 2);
        }
        Value::Mat(m) if m.class == Class::Char && m.rows == 1 => {
            format!("{{'{}'}}", m.row_text(0))
        }
        Value::Func(f) => format!("{{{}}}", f.shown()),
        v => {
            let (r, c) = v.dims();
            format!("{{{} {}}}", size_text(r, c), v.class_name())
        }
    };
    let at = text.chars().count() - 1;
    (text, at)
}

/// `text` widened to `width` characters by spaces inserted at `at`.
fn pad_at(text: &str, at: usize, width: usize) -> String {
    let n = text.chars().count();
    if n >= width {
        return text.to_string();
    }
    let split = text.char_indices().nth(at).map_or(text.len(), |(i, _)| i);
    format!(
        "{}{}{}",
        &text[..split],
        " ".repeat(width - n),
        &text[split..]
    )
}

/// What `disp` prints for a cell: each row indented four, its columns four
/// apart and each column as wide as its widest element. A cell too wide for
/// [`TERM_WIDTH`] is split into blocks of whole columns under the headings
/// a wide matrix has, `  Columns 1 through 3`.
fn cell_rows(c: &CellArray) -> String {
    if c.data.is_empty() {
        return String::new();
    }
    let texts: Vec<(String, usize)> = c.data.iter().map(cell_element).collect();
    let widths: Vec<usize> = (0..c.cols)
        .map(|j| {
            (0..c.rows)
                .map(|i| texts[j * c.rows + i].0.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    // Blocks of whole columns, at least one each.
    let mut blocks = Vec::new();
    let mut start = 0;
    while start < c.cols {
        let mut end = start + 1;
        let mut used = 4 + widths[start];
        while end < c.cols && used + 4 + widths[end] <= TERM_WIDTH {
            used += 4 + widths[end];
            end += 1;
        }
        blocks.push((start, end));
        start = end;
    }
    let wrapped = blocks.len() > 1;
    let mut out = String::new();
    for (b, &(c0, c1)) in blocks.iter().enumerate() {
        if wrapped {
            out.push_str(&column_header(c0 + 1, c1));
            out.push_str("\n\n");
        }
        for i in 0..c.rows {
            let cells: Vec<String> = (c0..c1)
                .map(|j| {
                    let (t, at) = &texts[j * c.rows + i];
                    pad_at(t, *at, widths[j])
                })
                .collect();
            let _ = writeln!(out, "    {}", cells.join("    "));
        }
        if wrapped && b + 1 < blocks.len() {
            out.push('\n');
        }
    }
    out
}

/// A named display's body for a struct array (cycle 07). A 1x1 struct is
/// `  struct with fields:`, a blank line and one line per field; any other
/// size lists the field names alone, `  1×2 struct array with fields:`.
fn struct_body(s: &StructArray) -> String {
    let n = s.numel();
    if n == 1 {
        if s.fields.is_empty() {
            return "  struct with no fields.\n".to_string();
        }
        return format!("  struct with fields:\n\n{}", field_lines(s));
    }
    let what = if n == 0 {
        format!("{} empty struct array", size_text(s.rows, s.cols))
    } else {
        format!("{} struct array", size_text(s.rows, s.cols))
    };
    if s.fields.is_empty() {
        return format!("  {what} with no fields.\n");
    }
    let names: String = s.fields.iter().map(|f| format!("    {f}\n")).collect();
    format!("  {what} with fields:\n\n{names}")
}

/// The field lines of a 1x1 struct: each name right-aligned to the longest,
/// so the longest is indented four, then `: ` and the value summarised on
/// one line by [`field_summary`].
fn field_lines(s: &StructArray) -> String {
    let w = s
        .fields
        .iter()
        .map(|f| f.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (f, v) in s.fields.iter().zip(&s.elems[0]) {
        let head = format!("    {:>w$}: ", f, w = w);
        let room = TERM_WIDTH.saturating_sub(head.chars().count());
        let _ = writeln!(out, "{}{}", head, field_summary(v, room));
    }
    out
}

/// One value on one line, as a struct's field line shows it. Never expanded
/// past one level, so a deeply nested struct displays in bounded time and
/// stack. A numeric or logical scalar is its number, a row of them `[1 2 3]`
/// when that fits in `room` characters, a 1-row char `'hi'`, `[]` the 0x0
/// double, a handle its text; anything else is its size and class, `[2×2
/// double]`, `{1×2 cell}` for a cell and `[1×1 struct]` for a struct.
fn field_summary(v: &Value, room: usize) -> String {
    let sized = |v: &Value| {
        let (r, c) = v.dims();
        format!("[{} {}]", size_text(r, c), v.class_name())
    };
    match v {
        Value::Mat(m) if m.class == Class::Char => {
            if m.rows == 0 && m.cols == 0 {
                "''".to_string()
            } else if m.rows == 1 {
                format!("'{}'", m.row_text(0))
            } else {
                sized(v)
            }
        }
        Value::Mat(m) if m.is_empty() && m.rows == 0 && m.cols == 0 => "[]".to_string(),
        Value::Mat(m) if m.is_scalar() => m.format().trim().to_string(),
        Value::Mat(m) if m.rows == 1 && !m.is_empty() => {
            let (texts, _, scale) = m.cells();
            let inline = format!("[{}]", texts.join(" "));
            if scale.is_none() && inline.chars().count() <= room {
                inline
            } else {
                sized(v)
            }
        }
        Value::Func(f) => f.shown(),
        Value::Cell(c) => format!("{{{} cell}}", size_text(c.rows, c.cols)),
        v => sized(v),
    }
}

/// The one line an `MException` displays as.
fn exception_line(e: &error::MError) -> String {
    if e.identifier().is_empty() {
        format!("  {}: {}\n", EXCEPTION_CLASS, e.msg)
    } else {
        format!("  {} ({}): {}\n", EXCEPTION_CLASS, e.identifier(), e.msg)
    }
}

/// A value as the UTF-16 code unit a char array stores for it: a `NaN` is
/// `0`, anything out of range is clamped to it, and a fraction is rounded.
pub fn code_unit(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.round().clamp(0.0, 65535.0)
    }
}

/// Decodes code units back to text. An unpaired surrogate becomes U+FFFD,
/// which is what "lossy" means here; a valid pair becomes its one character.
pub fn decode_units(units: impl Iterator<Item = f64>) -> String {
    let units: Vec<u16> = units.map(|v| code_unit(v) as u16).collect();
    String::from_utf16_lossy(&units)
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
        Matrix {
            rows,
            cols,
            data,
            class: Class::Double,
        }
    }

    /// This matrix with its class tag set, leaving the elements alone. The
    /// caller vouches that they are valid for `class`; [`to_class`] is the
    /// converting form.
    ///
    /// [`to_class`]: Matrix::to_class
    pub fn with_class(mut self, class: Class) -> Matrix {
        self.class = class;
        self
    }

    /// A logical scalar.
    pub fn from_bool(b: bool) -> Matrix {
        Matrix::scalar(b as u8 as f64).with_class(Class::Logical)
    }

    /// `s` as a 1-row char of UTF-16 code units. `''` is `0x0`, as in MATLAB,
    /// not the `1x0` a row of no units would be, so `size('')` is `0 0`.
    pub fn char_row(s: &str) -> Matrix {
        let units: Vec<f64> = s.encode_utf16().map(f64::from).collect();
        let m = if units.is_empty() {
            Matrix::empty()
        } else {
            Matrix::row(units)
        };
        m.with_class(Class::Char)
    }

    /// This matrix converted to `class`: a logical is every element tested
    /// against zero, which refuses a `NaN` exactly as `if NaN` does; a char is
    /// every element as a [`code_unit`]; a double keeps the values.
    pub fn to_class(self, class: Class) -> R<Matrix> {
        let data = match class {
            Class::Double => self.data,
            Class::Logical if self.class == Class::Logical => self.data,
            Class::Logical => {
                let mut out = Vec::with_capacity(self.data.len());
                for v in &self.data {
                    out.push(Matrix::logical_element(*v)? as u8 as f64);
                }
                out
            }
            Class::Char => self.data.into_iter().map(code_unit).collect(),
        };
        Ok(Matrix {
            rows: self.rows,
            cols: self.cols,
            data,
            class,
        })
    }

    pub fn is_char(&self) -> bool {
        self.class == Class::Char
    }

    /// Every element, in column-major order, decoded as UTF-16. This is how
    /// `fprintf('%s', A)` reads a char matrix, and it is the text of a row.
    pub fn text(&self) -> String {
        decode_units(self.data.iter().copied())
    }

    /// Row `r` of a char matrix as text.
    pub fn row_text(&self, r: usize) -> String {
        decode_units((0..self.cols).map(|c| self.get(r, c)))
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

    /// MATLAB's `isvector`: 1-by-N or N-by-1, where N may be `0`. A 1x0 and a
    /// 0x1 are vectors; a 0x0 is not.
    pub fn is_vector(&self) -> bool {
        self.rows == 1 || self.cols == 1
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

    /// MATLAB truthiness for `if` and `while`: non-empty and every element
    /// non-zero. `if []` is false, and takes no error with it.
    ///
    /// A `NaN` is neither: MATLAB and Octave both refuse to convert one, and
    /// taking it as true (which `!= 0.0` does) is silent and wrong (QA D5).
    /// That is the only way this fails, so an empty stays false rather than
    /// becoming an error.
    pub fn truth(&self) -> R<bool> {
        if self.data.iter().any(|v| v.is_nan()) {
            bail!(error::nan_to_logical());
        }
        Ok(!self.data.is_empty() && self.data.iter().all(|v| *v != 0.0))
    }

    /// The single value `&&` and `||` branch on.
    ///
    /// Stricter than [`truth`](Matrix::truth): those operators need one
    /// logical value, so an array or an empty is an error rather than "all
    /// non-zero". `[1 1] && 1` used to be `1` and `[] || 1` used to be `1`.
    pub fn logical_scalar(&self) -> R<bool> {
        match self.scalar_value() {
            Some(v) if v.is_nan() => bail!(error::nan_to_logical()),
            Some(v) => Ok(v != 0.0),
            None => bail!(error::logical_scalar_operand()),
        }
    }

    /// One element as a logical, for `&`, `|` and `~`, which convert element
    /// by element and so refuse a `NaN` the same way.
    pub fn logical_element(v: f64) -> R<bool> {
        if v.is_nan() {
            bail!(error::nan_to_logical());
        }
        Ok(v != 0.0)
    }

    pub fn map(&self, f: impl Fn(f64) -> f64) -> Matrix {
        Matrix::new(
            self.rows,
            self.cols,
            self.data.iter().map(|v| f(*v)).collect(),
        )
    }

    /// [`map`](Matrix::map) for an operation that may refuse an element, which
    /// is what `~` needs now that a `NaN` cannot become a logical. The shape
    /// comes from the operand, so there is nothing new to size-check.
    pub fn try_map(&self, f: impl Fn(f64) -> R<f64>) -> R<Matrix> {
        let mut data = Vec::with_capacity(self.numel());
        for v in &self.data {
            data.push(f(*v)?);
        }
        Ok(Matrix::new(self.rows, self.cols, data))
    }

    /// Element-wise combination with scalar / row / column broadcasting.
    pub fn zip(&self, o: &Matrix, op: &str, f: impl Fn(f64, f64) -> f64) -> R<Matrix> {
        self.try_zip(o, op, |a, b| Ok(f(a, b)))
    }

    /// [`zip`](Matrix::zip) for an operation that may refuse an element, which
    /// is what `.^` needs now that a would-be-complex result is an error
    /// rather than a `NaN`. `zip` is this function with an infallible closure.
    ///
    /// The broadcast shape goes through `args::check_shape` before a single
    /// element is allocated. `ones(1e5, 1) + ones(1, 1e5)` asks for 1e10
    /// elements from two 1e5-element operands, and used to abort in the
    /// allocator with exit 134, taking the REPL with it.
    pub fn try_zip(&self, o: &Matrix, op: &str, f: impl Fn(f64, f64) -> R<f64>) -> R<Matrix> {
        let dims_err = || error::operator_dims(op, self.rows, self.cols, o.rows, o.cols);
        let rows = broadcast_dim(self.rows, o.rows).ok_or_else(dims_err)?;
        let cols = broadcast_dim(self.cols, o.cols).ok_or_else(dims_err)?;
        crate::builtins::args::check_shape(rows as f64, cols as f64)?;
        let mut data = Vec::with_capacity(rows * cols);
        for c in 0..cols {
            for r in 0..rows {
                let a = self.get(
                    if self.rows == 1 { 0 } else { r },
                    if self.cols == 1 { 0 } else { c },
                );
                let b = o.get(
                    if o.rows == 1 { 0 } else { r },
                    if o.cols == 1 { 0 } else { c },
                );
                data.push(f(a, b)?);
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
        // Rearrangement keeps the class: `'ab'.'` is a char column.
        Matrix::new(self.cols, self.rows, data).with_class(self.class)
    }

    /// Matrix product. The result shape comes from the operands, so it goes
    /// through `args::check_shape` before `Matrix::filled` allocates: the
    /// outer product `ones(1e5, 1) * ones(1, 1e5)` used to abort in the
    /// allocator, and `zeros(2^32, 0) * zeros(0, 2^32)` used to wrap
    /// `rows * cols` to zero, report a 4294967296-square result and then panic
    /// on the next transpose.
    ///
    /// There is deliberately no `if b == 0.0 { continue }` shortcut. Skipping
    /// the multiply meant `Inf * 0` and `NaN * 0` never happened, so
    /// `[Inf 0] * [0; 1]` gave `0` where MATLAB gives `NaN`.
    pub fn matmul(&self, o: &Matrix) -> R<Matrix> {
        if self.cols != o.rows {
            bail!(error::matmul_dims(self.rows, self.cols, o.rows, o.cols));
        }
        crate::builtins::args::check_shape(self.rows as f64, o.cols as f64)?;
        let mut out = Matrix::filled(self.rows, o.cols, 0.0);
        if out.data.is_empty() {
            // Nothing to accumulate into, and `o.cols` alone can be enormous:
            // `zeros(0, 5) * zeros(5, 2^32)` is a legal 0x2^32 result, so the
            // column loop below would spin four billion times for nothing.
            return Ok(out);
        }
        for j in 0..o.cols {
            for k in 0..self.cols {
                let b = o.get(k, j);
                for i in 0..self.rows {
                    out.data[j * self.rows + i] += self.get(i, k) * b;
                }
            }
        }
        Ok(out)
    }

    /// The pivot magnitude at or below which this matrix counts as singular.
    ///
    /// It is relative to the matrix, not absolute: the old fixed `1e-14`
    /// called the diagonal `[1e-15 0; 0 1e-15]` singular although it is
    /// perfectly conditioned, and only its scale was small. `eps * n * ||A||`
    /// is the usual rule, with `||A||` the largest magnitude in `A`.
    ///
    /// `solve` and `det` both use it, which is what makes them agree on what
    /// singular means. Non-finite entries are left out of the norm, so an
    /// `Inf` in the matrix cannot make every pivot look negligible; a pivot
    /// that is itself `Inf` or `NaN` fails the `<=` test and flows through to
    /// the arithmetic, exactly as it did under the fixed threshold.
    ///
    /// The test is `<=` rather than `<` so that the all-zero matrix, whose
    /// norm and tolerance are both `0`, is still singular.
    fn singular_tol(&self) -> f64 {
        let norm = self
            .data
            .iter()
            .filter(|v| v.is_finite())
            .fold(0.0_f64, |acc, v| acc.max(v.abs()));
        f64::EPSILON * self.rows.max(1) as f64 * norm
    }

    /// Solve A * X = B for square A (Gaussian elimination with partial pivoting).
    // Elimination is written with explicit indices on purpose; cycle 08 replaces
    // solve and det with a shared LU factorisation.
    #[allow(clippy::needless_range_loop)]
    pub fn solve(&self, b: &Matrix) -> R<Matrix> {
        let n = self.rows;
        if self.rows != self.cols {
            bail!(error::nonsquare_system());
        }
        if b.rows != n {
            bail!(error::solve_dims(self.rows, self.cols, b.rows, b.cols));
        }
        let tol = self.singular_tol();
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
            if a[p][k].abs() <= tol {
                bail!(error::singular());
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

    pub fn inv(&self) -> R<Matrix> {
        if self.rows != self.cols {
            bail!(error::nonsquare_inverse());
        }
        self.solve(&Matrix::identity(self.rows, self.rows))
    }

    // Elimination is written with explicit indices on purpose; cycle 08 replaces
    // solve and det with a shared LU factorisation.
    #[allow(clippy::needless_range_loop)]
    pub fn det(&self) -> R<f64> {
        if self.rows != self.cols {
            bail!(error::nonsquare_determinant());
        }
        let n = self.rows;
        let tol = self.singular_tol();
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
            // The same test `solve` makes, so the two agree on what singular
            // means: `det` reports `0` where `solve` refuses to divide.
            if a[p][k].abs() <= tol {
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

    /// One rendered cell per element, column-major like `data`, the column
    /// width they are all padded to, and the common scale factor line that
    /// heads the display when there is one.
    ///
    /// A logical is `0` or `1` in four-wide columns, whatever its size, so
    /// `disp(3 > 1)` is `   1` (QA D38). A double takes one of three layouts:
    ///
    /// - **Integers.** Every finite element whole and below `1e9` in
    ///   magnitude. Up to 999 the column is the widest number plus three, at
    ///   least six, as it always was; from 1000 on it is twelve, which is what
    ///   `x = 1000` (`        1000`) and `x = [1 1000]` record.
    /// - **Fixed point.** Four decimals in ten-wide columns, when the largest
    ///   magnitude `M` has `floor(log10(M))` between -2 and 2, so from `0.01`
    ///   up to below `1000`. An exact zero prints as a bare `0`.
    /// - **Outside that range**, a scalar is short `e` format
    ///   (`   1.2345e+03`) and an array is the fixed-point layout of
    ///   `A / 10^k`, headed once by `   1.0e+0k *`, with `k = floor(log10(M))`.
    ///
    /// A `NaN` or an `Inf` has no digits of its own, so it neither changes the
    /// layout nor widens a column: `[1 2 NaN]` is `     1     2   NaN`, and a
    /// matrix of nothing but non-finite values is six-wide integer columns.
    fn cells(&self) -> (Vec<String>, usize, Option<String>) {
        if self.class == Class::Logical {
            let texts = self
                .data
                .iter()
                .map(|v| if *v != 0.0 { "1" } else { "0" }.to_string())
                .collect();
            return (texts, 4, None);
        }
        let finite = || self.data.iter().copied().filter(|v| v.is_finite());
        let max_abs = finite().map(f64::abs).fold(0.0_f64, f64::max);
        if finite().all(|v| v.fract() == 0.0) && max_abs < INT_LIMIT {
            let texts: Vec<String> = self
                .data
                .iter()
                .map(|v| {
                    if v.is_finite() {
                        format!("{}", *v as i64)
                    } else {
                        nonfinite(*v)
                    }
                })
                .collect();
            let digits = self
                .data
                .iter()
                .zip(&texts)
                .filter(|(v, _)| v.is_finite())
                .map(|(_, s)| s.len())
                .max()
                .unwrap_or(1);
            let width = if max_abs < 1000.0 {
                (digits + 3).max(6)
            } else {
                (digits + 2).max(12)
            };
            return (texts, width, None);
        }
        let exp = max_abs.log10().floor() as i32;
        if (-2..=2).contains(&exp) {
            return (self.fixed_texts(1.0), 10, None);
        }
        if self.is_scalar() {
            let v = self.data[0];
            return (vec![crate::interp::fmt_e(v, 4)], 13, None);
        }
        let scale: f64 = format!("1e{exp}").parse().unwrap_or(1.0);
        let header = format!("   1.0e{:+03} *\n\n", exp);
        (self.fixed_texts(scale), 10, Some(header))
    }

    /// Every element divided by `scale` in four decimals, with an exact zero
    /// as a bare `0` and a non-finite value as its name.
    fn fixed_texts(&self, scale: f64) -> Vec<String> {
        self.data
            .iter()
            .map(|v| {
                if !v.is_finite() {
                    nonfinite(*v)
                } else if *v == 0.0 {
                    "0".to_string()
                } else {
                    format!("{:.4}", v / scale)
                }
            })
            .collect()
    }

    /// The numeric display body, with no name and no class header: what
    /// `disp` prints for a non-empty double or logical.
    ///
    /// A matrix too wide for [`TERM_WIDTH`] is split into blocks of whole
    /// columns, each headed by the columns it holds, which is what MATLAB
    /// does; `linspace(1, 2)` used to print about 1300 characters on one line.
    /// A common scale factor is printed once, above the first block.
    pub fn format(&self) -> String {
        if self.is_empty() {
            return "     []\n".to_string();
        }
        let (texts, width, scale) = self.cells();
        // At least one column per block, however wide a single column is:
        // wrapping every element onto its own line is still better than a
        // block with no columns in it, which would never terminate.
        let per = (TERM_WIDTH / width).max(1);
        let wrapped = self.cols > per;
        let mut out = scale.unwrap_or_default();
        let mut c0 = 0;
        while c0 < self.cols {
            let c1 = (c0 + per).min(self.cols);
            if wrapped {
                out.push_str(&column_header(c0 + 1, c1));
                // The header, then a blank line, then the block's rows.
                out.push_str("\n\n");
            }
            for r in 0..self.rows {
                for c in c0..c1 {
                    let _ = write!(out, "{:>w$}", texts[c * self.rows + r], w = width);
                }
                out.push('\n');
            }
            c0 = c1;
            if wrapped && c0 < self.cols {
                out.push('\n');
            }
        }
        out
    }

    /// What `disp` prints. A char is bare text, one line per row, and `''`
    /// is still one empty line; any other empty prints nothing at all.
    pub fn disp_text(&self) -> String {
        if self.class == Class::Char {
            if self.rows == 0 {
                return "\n".to_string();
            }
            return (0..self.rows)
                .map(|r| format!("{}\n", self.row_text(r)))
                .collect();
        }
        if self.is_empty() {
            return String::new();
        }
        self.format()
    }

    /// What follows `x =` and its blank line in a named display.
    ///
    /// A logical carries a `  logical` or `  1×3 logical array` header and a
    /// char of more than one row a `  2×3 char array` header, each followed by
    /// a blank line. A 1-row char is its text quoted, as MATLAB has shown it
    /// since R2018a. An empty says what it is, `  0×3 empty double matrix`,
    /// except the 0x0 double, which is still `     []`.
    pub fn display_body(&self) -> String {
        let (r, c) = (self.rows, self.cols);
        if self.is_empty() {
            return match self.class {
                Class::Char => format!("  {r}×{c} empty char array\n"),
                Class::Logical => format!("  {r}×{c} empty logical array\n"),
                Class::Double if r == 0 && c == 0 => "     []\n".to_string(),
                Class::Double if r == 1 => format!("  1×{c} empty double row vector\n"),
                Class::Double if c == 1 => format!("  {r}×1 empty double column vector\n"),
                Class::Double => format!("  {r}×{c} empty double matrix\n"),
            };
        }
        match self.class {
            Class::Char => {
                let header = if r == 1 {
                    String::new()
                } else {
                    format!("  {r}×{c} char array\n\n")
                };
                let rows: String = (0..r)
                    .map(|k| format!("    '{}'\n", self.row_text(k)))
                    .collect();
                header + &rows
            }
            Class::Logical if self.is_scalar() => format!("  logical\n\n{}", self.format()),
            Class::Logical => format!("  {r}×{c} logical array\n\n{}", self.format()),
            Class::Double => self.format(),
        }
    }
}

/// The magnitude from which a whole number no longer displays as one. A
/// matrix reaching it takes the scale factor, and a scalar the `e` format:
/// `x = 1e10` is `   1.0000e+10`.
const INT_LIMIT: f64 = 1e9;

/// The width of the display MATLAB assumes, in characters.
///
/// It is a constant rather than a terminal query on purpose: the output of a
/// script must not depend on whether it was run at a prompt or piped into a
/// file, or a golden case would pass on one machine and fail on another.
/// MATLAB's own command window defaults to 80 and keeps using 80 when its
/// output is captured, so 80 is both the compatible answer and the
/// reproducible one.
pub const TERM_WIDTH: usize = 80;

/// `  Columns 1 through 13`, MATLAB's heading for one block of a wide matrix,
/// with its singular and two-column spellings.
fn column_header(first: usize, last: usize) -> String {
    match last - first {
        0 => format!("  Column {}", first),
        1 => format!("  Columns {} and {}", first, last),
        _ => format!("  Columns {} through {}", first, last),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn close_tol(a: f64, b: f64, tol: f64) {
        assert!(
            (a - b).abs() <= tol * b.abs().max(1.0),
            "{a} vs {b} (tolerance {tol})"
        );
    }

    fn close(a: f64, b: f64) {
        close_tol(a, b, 1e-12);
    }

    /// Builds a matrix from elements given in reading (row-major) order.
    fn rmat(rows: usize, cols: usize, row_major: &[f64]) -> Matrix {
        assert_eq!(rows * cols, row_major.len());
        let mut m = Matrix::filled(rows, cols, 0.0);
        for (i, v) in row_major.iter().enumerate() {
            m.set(i / cols, i % cols, *v);
        }
        m
    }

    fn close_all(got: &Matrix, want: &Matrix) {
        assert_eq!((got.rows, got.cols), (want.rows, want.cols));
        for (g, w) in got.data.iter().zip(&want.data) {
            close(*g, *w);
        }
    }

    // ---- layout ------------------------------------------------------

    #[test]
    fn column_major_layout() {
        let m = Matrix::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // Element (r, c) lives at data[c * rows + r].
        assert_eq!(m.get(0, 0), m.data[0]);
        assert_eq!(m.get(1, 0), m.data[1]);
        assert_eq!(m.get(0, 1), m.data[2]);
        assert_eq!(m.get(1, 1), m.data[3]);
        assert_eq!(m.get(0, 2), m.data[4]);
        assert_eq!(m.get(1, 2), m.data[5]);
        // So the matrix above reads [1 3 5; 2 4 6].
        assert_eq!(m, rmat(2, 3, &[1.0, 3.0, 5.0, 2.0, 4.0, 6.0]));

        let mut z = Matrix::filled(2, 2, 0.0);
        z.set(1, 0, 7.0);
        z.set(0, 1, 9.0);
        assert_eq!(z.data, [0.0, 7.0, 9.0, 0.0]);
    }

    #[test]
    fn transpose_round_trips() {
        let m = Matrix::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let t = m.transpose();
        assert_eq!((t.rows, t.cols), (3, 2));
        assert_eq!(t.get(0, 0), m.get(0, 0));
        assert_eq!(t.get(1, 0), m.get(0, 1));
        assert_eq!(t.get(2, 0), m.get(0, 2));
        assert_eq!(t.get(0, 1), m.get(1, 0));
        assert_eq!(t.get(2, 1), m.get(1, 2));
        assert_eq!(t.transpose(), m);
        assert_eq!(Matrix::empty().transpose(), Matrix::empty());
    }

    #[test]
    fn shape_predicates() {
        assert!(Matrix::empty().is_empty());
        assert!(Matrix::scalar(1.0).is_scalar());
        assert!(Matrix::row(vec![1.0, 2.0]).is_vector());
        assert!(Matrix::col(vec![1.0, 2.0]).is_vector());
        assert!(!rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]).is_vector());
        assert!(!Matrix::empty().is_vector());
        // 1-by-N or N-by-1 with N = 0 is still a vector; 2x0 is not.
        assert!(Matrix::new(1, 0, vec![]).is_vector());
        assert!(Matrix::new(0, 1, vec![]).is_vector());
        assert!(!Matrix::new(2, 0, vec![]).is_vector());
        assert!(Matrix::scalar(1.0).is_vector());
        assert_eq!(Matrix::identity(2, 3).data, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        // MATLAB truthiness: non-empty and every element non-zero.
        assert!(Matrix::row(vec![1.0, 2.0]).truth().unwrap());
        assert!(!Matrix::row(vec![1.0, 0.0]).truth().unwrap());
        assert!(!Matrix::empty().truth().unwrap());
    }

    #[test]
    fn value_conversions() {
        let m = Value::str("AB").into_mat().unwrap();
        assert_eq!((m.rows, m.cols), (1, 2));
        assert_eq!(m.data, [65.0, 66.0]);
        assert_eq!(m.class, Class::Char);
        assert_eq!(Value::str("hi").display("s"), "s =\n\n    'hi'\n\n");
        assert_eq!(Value::str("hi").text().as_deref(), Some("hi"));
        assert_eq!(Value::Mat(Matrix::scalar(3.0)).text(), None);
        assert_eq!(
            Value::Mat(Matrix::scalar(3.0)).display("x"),
            "x =\n\n     3\n\n"
        );
    }

    // ---- zip / broadcasting ------------------------------------------

    #[test]
    fn zip_broadcasts_a_row() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let r = rmat(1, 3, &[10.0, 20.0, 30.0]);
        let s = a.zip(&r, "+", |x, y| x + y).unwrap();
        assert_eq!((s.rows, s.cols), (2, 3));
        assert_eq!(s, rmat(2, 3, &[11.0, 22.0, 33.0, 14.0, 25.0, 36.0]));
        // Broadcasting is symmetric in shape.
        let s2 = r.zip(&a, "+", |x, y| x + y).unwrap();
        assert_eq!(s2, s);
    }

    #[test]
    fn zip_broadcasts_a_column() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let c = rmat(2, 1, &[10.0, 20.0]);
        let s = a.zip(&c, "+", |x, y| x + y).unwrap();
        assert_eq!((s.rows, s.cols), (2, 3));
        assert_eq!(s, rmat(2, 3, &[11.0, 12.0, 13.0, 24.0, 25.0, 26.0]));
    }

    #[test]
    fn zip_broadcasts_scalars_on_either_side() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let s = Matrix::scalar(2.0);
        let l = a.zip(&s, "*", |x, y| x * y).unwrap();
        assert_eq!(l, rmat(2, 3, &[2.0, 4.0, 6.0, 8.0, 10.0, 12.0]));
        let r = s.zip(&a, "-", |x, y| x - y).unwrap();
        assert_eq!((r.rows, r.cols), (2, 3));
        assert_eq!(r, rmat(2, 3, &[1.0, 0.0, -1.0, -2.0, -3.0, -4.0]));
    }

    #[test]
    fn zip_column_by_row_is_an_outer_product() {
        let c = Matrix::col(vec![1.0, 2.0, 3.0]);
        let r = Matrix::row(vec![10.0, 20.0, 30.0]);
        let o = c.zip(&r, "*", |x, y| x * y).unwrap();
        assert_eq!((o.rows, o.cols), (3, 3));
        assert_eq!(
            o,
            rmat(
                3,
                3,
                &[10.0, 20.0, 30.0, 20.0, 40.0, 60.0, 30.0, 60.0, 90.0]
            )
        );
    }

    /// Acceptance test 17, the `zip` half: the broadcast result shape goes
    /// through `check_shape`, so `ones(1e5, 1) + ones(1, 1e5)` is an error
    /// rather than an allocator abort. Asserted here directly, not only
    /// through a script, because the script form used to kill the process.
    #[test]
    fn zip_checks_the_broadcast_result_size_before_allocating() {
        // 20000 squared is 4e8, past the 2^28-element cap, while the two
        // operands together are 40000 elements.
        let col = Matrix::col(vec![1.0; 20_000]);
        let row = Matrix::row(vec![1.0; 20_000]);
        let e = col.zip(&row, "+", |x, y| x + y).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 20000x20000 array exceeds the maximum array size."
        );
        // The same guard on the fallible form.
        assert!(col.try_zip(&row, "+", |x, y| Ok(x + y)).is_err());
        // A shape that fits is untouched, and `zip` is `try_zip` with an
        // infallible closure.
        let small = Matrix::col(vec![1.0, 2.0])
            .try_zip(&Matrix::row(vec![10.0, 20.0]), "*", |x, y| Ok(x * y))
            .unwrap();
        assert_eq!(small.data, [10.0, 20.0, 20.0, 40.0]);
        // A refusal from the closure comes out as the error.
        let e = Matrix::scalar(1.0)
            .try_zip(&Matrix::scalar(2.0), "+", |_, _| {
                Err(crate::error::too_many_outputs())
            })
            .unwrap_err()
            .msg;
        assert_eq!(e, "Too many output arguments.");
    }

    #[test]
    fn zip_rejects_incompatible_sizes() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = rmat(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let e = a.zip(&b, "+", |x, y| x + y).unwrap_err().msg;
        assert!(e.contains("incompatible sizes"), "{e}");
        assert!(e.contains("2x3 vs 3x2"), "{e}");
        assert!(e.contains("'+'"), "{e}");
    }

    // ---- matmul ------------------------------------------------------

    #[test]
    fn matmul_values_and_shape() {
        let a = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = rmat(3, 2, &[7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
        let p = a.matmul(&b).unwrap();
        assert_eq!((p.rows, p.cols), (2, 2));
        assert_eq!(p, rmat(2, 2, &[58.0, 64.0, 139.0, 154.0]));
        // The other order gives the 3x3 product.
        let q = b.matmul(&a).unwrap();
        assert_eq!((q.rows, q.cols), (3, 3));
        assert_eq!(q.get(0, 0), 7.0 * 1.0 + 8.0 * 4.0);
        // Multiplying by the identity is a no-op.
        assert_eq!(a.matmul(&Matrix::identity(3, 3)).unwrap(), a);
        assert_eq!(Matrix::identity(2, 2).matmul(&a).unwrap(), a);
    }

    /// Acceptance test 17, the `matmul` half, and acceptance test 4.
    #[test]
    fn matmul_checks_the_result_size_before_allocating() {
        // Both operands are empty, so the old code allocated nothing and
        // reached `Matrix::filled` with a wrapped `rows * cols`.
        let tall = Matrix::new(100_000, 0, Vec::new());
        let wide = Matrix::new(0, 100_000, Vec::new());
        assert_eq!(
            tall.matmul(&wide).unwrap_err().msg,
            "Requested 100000x100000 array exceeds the maximum array size."
        );
        // The size that used to wrap to zero: 2^32 squared is exactly 2^64.
        let a = Matrix::new(1 << 32, 0, Vec::new());
        let b = Matrix::new(0, 1 << 32, Vec::new());
        assert_eq!(
            a.matmul(&b).unwrap_err().msg,
            "Requested 4294967296x4294967296 array exceeds the maximum array size."
        );
        // A legal empty result is still produced, and promptly.
        let wide_ok = Matrix::new(0, 1 << 20, Vec::new());
        let empty = Matrix::new(0, 0, Vec::new()).matmul(&wide_ok).unwrap();
        assert_eq!((empty.rows, empty.cols), (0, 1 << 20));
    }

    /// The `if b == 0.0 { continue }` shortcut is gone, so a zero factor is
    /// multiplied like any other and `Inf * 0` and `NaN * 0` happen.
    #[test]
    fn matmul_keeps_inf_and_nan_through_a_zero_factor() {
        let inf = Matrix::row(vec![f64::INFINITY, 0.0]);
        let nan = Matrix::row(vec![f64::NAN, 0.0]);
        let pick = Matrix::col(vec![0.0, 1.0]);
        assert!(inf.matmul(&pick).unwrap().data[0].is_nan());
        assert!(nan.matmul(&pick).unwrap().data[0].is_nan());
        // A finite matrix with zeros is unaffected: the shortcut only ever
        // mattered for a non-finite partner.
        let a = rmat(2, 2, &[1.0, 0.0, 0.0, 2.0]);
        assert_eq!(a.matmul(&a).unwrap(), rmat(2, 2, &[1.0, 0.0, 0.0, 4.0]));
    }

    #[test]
    fn matmul_rejects_bad_dimensions() {
        let a = Matrix::row(vec![1.0, 2.0]);
        let b = Matrix::row(vec![3.0, 4.0]);
        let e = a.matmul(&b).unwrap_err().msg;
        assert!(e.contains("Incorrect dimensions"), "{e}");
        assert!(e.contains("1x2 * 1x2"), "{e}");
        // Transposing the right side makes it legal again.
        assert_eq!(a.matmul(&b.transpose()).unwrap(), Matrix::scalar(11.0));
    }

    // ---- solve -------------------------------------------------------

    #[test]
    fn solve_two_by_two() {
        let a = rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        let b = Matrix::col(vec![3.0, 5.0]);
        let x = a.solve(&b).unwrap();
        assert_eq!((x.rows, x.cols), (2, 1));
        close(x.get(0, 0), 0.8);
        close(x.get(1, 0), 1.4);
    }

    #[test]
    fn solve_multiple_right_hand_sides() {
        let a = rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        let b = rmat(2, 2, &[3.0, 1.0, 5.0, 0.0]);
        let x = a.solve(&b).unwrap();
        assert_eq!((x.rows, x.cols), (2, 2));
        close(x.get(0, 0), 0.8);
        close(x.get(1, 0), 1.4);
        close(x.get(0, 1), 0.6);
        close(x.get(1, 1), -0.2);
        // A * X reproduces B.
        close_all(&a.matmul(&x).unwrap(), &b);
    }

    #[test]
    fn solve_uses_partial_pivoting() {
        // A zero in the leading pivot position needs a row swap.
        let a = rmat(2, 2, &[0.0, 1.0, 1.0, 0.0]);
        let x = a.solve(&Matrix::col(vec![1.0, 2.0])).unwrap();
        close(x.get(0, 0), 2.0);
        close(x.get(1, 0), 1.0);

        let a3 = rmat(3, 3, &[0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 0.0]);
        let x3 = a3.solve(&Matrix::col(vec![1.0, 2.0, 3.0])).unwrap();
        close(x3.get(0, 0), 1.0);
        close(x3.get(1, 0), 1.0);
        close(x3.get(2, 0), 1.0);
    }

    #[test]
    fn solve_moderately_ill_conditioned_system() {
        // 4x4 Hilbert matrix, H(i, j) = 1 / (i + j - 1) with 1-based indices.
        let mut h = Matrix::filled(4, 4, 0.0);
        for (i, v) in h.data.iter_mut().enumerate() {
            let (r, c) = (i % 4, i / 4);
            *v = 1.0 / (r + c + 1) as f64;
        }
        let want = Matrix::filled(4, 1, 1.0);
        let b = h.matmul(&want).unwrap();
        let x = h.solve(&b).unwrap();
        assert_eq!((x.rows, x.cols), (4, 1));
        for v in &x.data {
            close_tol(*v, 1.0, 1e-8);
        }
    }

    /// The pivot threshold is relative to the matrix, so a well-conditioned
    /// system is solved whatever its scale. The old fixed `1e-14` called the
    /// first of these singular.
    #[test]
    fn solve_scales_its_pivot_tolerance_with_the_matrix() {
        let tiny = rmat(2, 2, &[1e-15, 0.0, 0.0, 1e-15]);
        let x = tiny.solve(&Matrix::col(vec![1.0, 1.0])).unwrap();
        close_tol(x.get(0, 0), 1e15, 1e-12);
        close_tol(x.get(1, 0), 1e15, 1e-12);
        // The same system scaled up and down is solved just as well.
        for scale in [1e-300, 1e-30, 1.0, 1e30, 1e150] {
            let a = rmat(2, 2, &[scale, 0.0, 0.0, scale]);
            let x = a.solve(&Matrix::col(vec![scale, 2.0 * scale])).unwrap();
            close(x.get(0, 0), 1.0);
            close(x.get(1, 0), 2.0);
        }
        // Scale alone never decides: a matrix that is singular stays singular
        // however small its entries are.
        let singular = rmat(2, 2, &[1e-15, 2e-15, 2e-15, 4e-15]);
        assert!(
            singular
                .solve(&Matrix::col(vec![1.0, 2.0]))
                .unwrap_err()
                .msg
                .contains("singular")
        );
        // An all-zero matrix has a zero norm and so a zero tolerance, which
        // is why the test is `<=` and not `<`.
        let zeros = Matrix::filled(2, 2, 0.0);
        assert!(zeros.solve(&Matrix::col(vec![1.0, 1.0])).is_err());
    }

    /// `det` and `solve` make the same test, which is what "agree on what
    /// singular means" is: `det` reports `0` exactly where `solve` refuses.
    #[test]
    fn det_and_solve_agree_on_singular() {
        let cases = [
            rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]),
            rmat(2, 2, &[1e-15, 2e-15, 2e-15, 4e-15]),
            Matrix::filled(2, 2, 0.0),
            rmat(2, 2, &[1e-15, 0.0, 0.0, 1e-15]),
            rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]),
        ];
        for a in cases {
            let rhs = Matrix::col(vec![1.0, 1.0]);
            let singular_to_det = a.det().unwrap() == 0.0;
            let singular_to_solve = a.solve(&rhs).is_err();
            assert_eq!(singular_to_det, singular_to_solve, "{:?}", a.data);
        }
        // The scaled diagonal has a real determinant now, rather than being
        // written off: 1e-15 * 1e-15.
        close_tol(
            rmat(2, 2, &[1e-15, 0.0, 0.0, 1e-15]).det().unwrap(),
            1e-30,
            1e-12,
        );
    }

    /// A non-finite entry must not drag the norm, and with it the tolerance,
    /// to infinity: every pivot would then be at or below it and every such
    /// matrix would be called singular, which the fixed threshold never did.
    #[test]
    fn a_non_finite_entry_does_not_make_everything_singular() {
        for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let a = rmat(2, 2, &[bad, 0.0, 0.0, 1.0]);
            assert!(a.singular_tol().is_finite(), "{bad}");
            assert_eq!(a.singular_tol(), f64::EPSILON * 2.0);
        }
        // And the system is still solved: the `Inf` pivot is far above the
        // tolerance, so the second row is eliminated and back-substituted as
        // it always was.
        let a = rmat(2, 2, &[f64::INFINITY, 0.0, 0.0, 1.0]);
        let x = a.solve(&Matrix::col(vec![1.0, 1.0])).unwrap();
        assert_eq!(x.get(1, 0), 1.0);
    }

    #[test]
    fn solve_rejects_singular_and_non_square() {
        let s = rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        let e = s.solve(&Matrix::col(vec![1.0, 2.0])).unwrap_err().msg;
        assert!(e.contains("singular"), "{e}");

        let ns = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let e = ns.solve(&Matrix::col(vec![1.0, 2.0])).unwrap_err().msg;
        assert!(e.contains("square"), "{e}");

        // Square, but the right-hand side has the wrong number of rows.
        let a = rmat(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        assert!(a.solve(&Matrix::col(vec![1.0, 2.0, 3.0])).is_err());
    }

    // ---- inv / det ---------------------------------------------------

    #[test]
    fn inv_times_original_is_the_identity() {
        let a = rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0]);
        let ai = a.inv().unwrap();
        let id = Matrix::identity(3, 3);
        close_all(&ai.matmul(&a).unwrap(), &id);
        close_all(&a.matmul(&ai).unwrap(), &id);

        // A 2x2 case with an inverse that is easy to state exactly.
        let b = rmat(2, 2, &[4.0, 7.0, 2.0, 6.0]);
        close_all(&b.inv().unwrap(), &rmat(2, 2, &[0.6, -0.7, -0.2, 0.4]));

        assert!(rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).inv().is_err());
        assert!(rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]).inv().is_err());
    }

    #[test]
    fn det_known_values() {
        close(rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]).det().unwrap(), -2.0);
        assert_eq!(rmat(2, 2, &[1.0, 2.0, 2.0, 4.0]).det().unwrap(), 0.0);
        close(Matrix::identity(1, 1).det().unwrap(), 1.0);
        close(Matrix::identity(4, 4).det().unwrap(), 1.0);
        close(
            rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0])
                .det()
                .unwrap(),
            9.0,
        );
        let e = rmat(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .det()
            .unwrap_err()
            .msg;
        assert!(e.contains("square"), "{e}");
    }

    #[test]
    fn det_sign_flips_when_rows_are_swapped() {
        let a = rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let swapped = rmat(2, 2, &[3.0, 4.0, 1.0, 2.0]);
        let da = a.det().unwrap();
        let ds = swapped.det().unwrap();
        close(ds, -da);

        let b = rmat(3, 3, &[4.0, 7.0, 2.0, 3.0, 6.0, 1.0, 2.0, 5.0, 3.0]);
        let b_swapped = rmat(3, 3, &[3.0, 6.0, 1.0, 4.0, 7.0, 2.0, 2.0, 5.0, 3.0]);
        close(b_swapped.det().unwrap(), -b.det().unwrap());
    }

    // ---- format ------------------------------------------------------

    #[test]
    fn format_integer_path() {
        assert_eq!(
            Matrix::row(vec![1.0, 10.0, 100.0]).format(),
            "     1    10   100\n"
        );
        assert_eq!(Matrix::scalar(0.0).format(), "     0\n");
        assert_eq!(Matrix::scalar(-0.0).format(), "     0\n");
        assert_eq!(Matrix::row(vec![-1.0, 2.0]).format(), "    -1     2\n");
        assert_eq!(
            rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]).format(),
            "     1     2\n     3     4\n"
        );
    }

    /// Acceptance test 10: from 1000 on, an integer column is twelve wide.
    #[test]
    fn integers_of_1000_and_above_take_twelve_wide_columns() {
        assert_eq!(Matrix::scalar(1000.0).format(), "        1000\n");
        assert_eq!(
            Matrix::row(vec![1.0, 1000.0]).format(),
            "           1        1000\n"
        );
        assert_eq!(
            Matrix::row(vec![-123456.0, 1.0]).format(),
            "     -123456           1\n"
        );
        // 999 is still the six-wide column it always was.
        assert_eq!(Matrix::row(vec![1.0, 999.0]).format(), "     1   999\n");
        // Six columns of twelve fit in 80; the seventh wraps.
        let wide = Matrix::row(vec![1000.0; 7]).format();
        assert!(wide.starts_with("  Columns 1 through 6\n\n"), "{wide}");
        // From 1e9 a whole number is no longer shown as one.
        assert_eq!(Matrix::scalar(1e10).format(), "   1.0000e+10\n");
    }

    #[test]
    fn format_four_decimal_path() {
        assert_eq!(
            Matrix::row(vec![1.5, 2.25]).format(),
            "    1.5000    2.2500\n"
        );
        assert_eq!(Matrix::scalar(-0.5).format(), "   -0.5000\n");
        assert_eq!(Matrix::scalar(999.5).format(), "  999.5000\n");
        assert_eq!(Matrix::scalar(0.05).format(), "    0.0500\n");
        // A value a rounding error away from an integer is not one: this is
        // MATLAB's det([1 2; 3 4]), which displays as `-2.0000`.
        let near = -2.0 - 4.0 * f64::EPSILON / 2.0;
        assert_eq!(Matrix::scalar(near).format(), "   -2.0000\n");
    }

    /// Acceptance test 10: an exact zero in a fixed-point row is a bare `0`.
    #[test]
    fn an_exact_zero_in_a_fixed_point_row_is_bare() {
        assert_eq!(
            Matrix::row(vec![0.0, 1.5]).format(),
            "         0    1.5000\n"
        );
        // A value that only rounds to zero keeps its decimals.
        assert_eq!(
            Matrix::row(vec![0.00001, 1.5]).format(),
            "    0.0000    1.5000\n"
        );
    }

    /// QA D20: a scalar outside the fixed-point range is short `e` format.
    #[test]
    fn a_scalar_outside_the_fixed_point_range_is_e_format() {
        assert_eq!(Matrix::scalar(1234.5).format(), "   1.2345e+03\n");
        assert_eq!(Matrix::scalar(12345.6).format(), "   1.2346e+04\n");
        assert_eq!(Matrix::scalar(0.001).format(), "   1.0000e-03\n");
        assert_eq!(Matrix::scalar(123456.7).format(), "   1.2346e+05\n");
        assert_eq!(Matrix::scalar(0.00012).format(), "   1.2000e-04\n");
        assert_eq!(Matrix::scalar(-1234.5).format(), "  -1.2345e+03\n");
    }

    /// Acceptance test 10: an array outside the range is scaled by a common
    /// factor, printed once above it.
    #[test]
    fn an_array_outside_the_fixed_point_range_takes_a_scale_factor() {
        assert_eq!(
            Matrix::row(vec![1.5, 1000.5]).format(),
            "   1.0e+03 *\n\n    0.0015    1.0005\n"
        );
        assert_eq!(
            Matrix::row(vec![0.001, 0.002]).format(),
            "   1.0e-03 *\n\n    1.0000    2.0000\n"
        );
        assert_eq!(
            Matrix::row(vec![1e5, 0.5]).format(),
            "   1.0e+05 *\n\n    1.0000    0.0000\n"
        );
        // A zero stays bare under the factor, and NaN keeps its name.
        assert_eq!(
            Matrix::row(vec![0.0, 1500.5, f64::NAN]).format(),
            "   1.0e+03 *\n\n         0    1.5005       NaN\n"
        );
        // A whole number too large for integer columns scales too.
        assert_eq!(
            Matrix::row(vec![1.0, 1e10]).format(),
            "   1.0e+10 *\n\n    0.0000    1.0000\n"
        );
        // A wide matrix prints the factor once, above the first block.
        let wide = Matrix::row((1..=9).map(|k| k as f64 * 1000.5).collect()).format();
        assert!(
            wide.starts_with("   1.0e+03 *\n\n  Columns 1 through 8\n\n"),
            "{wide}"
        );
        assert_eq!(wide.matches("1.0e+03").count(), 1);
        assert!(wide.contains("  Column 9\n"), "{wide}");
    }

    #[test]
    fn format_nonfinite_and_empty() {
        // Integer columns. There is no finite value to take a digit count
        // from, so the width falls back to one digit plus three.
        assert_eq!(
            Matrix::row(vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY]).format(),
            "   NaN   Inf  -Inf\n"
        );
        assert_eq!(Matrix::empty().format(), "     []\n");
        assert_eq!(nonfinite(f64::NAN), "NaN");
        assert_eq!(nonfinite(f64::INFINITY), "Inf");
        assert_eq!(nonfinite(f64::NEG_INFINITY), "-Inf");
    }

    /// A `NaN` or an `Inf` no longer drags a row of whole numbers onto the
    /// four-decimal path: `[1 2 NaN]` used to print
    /// `    1.0000    2.0000       NaN`.
    #[test]
    fn a_non_finite_element_keeps_the_integer_columns() {
        assert_eq!(
            Matrix::row(vec![1.0, 2.0, f64::NAN]).format(),
            "     1     2   NaN\n"
        );
        assert_eq!(
            Matrix::row(vec![1.0, f64::INFINITY]).format(),
            "     1   Inf\n"
        );
        // A scalar `NaN` is the same six-wide column, not the ten-wide one.
        assert_eq!(Matrix::scalar(f64::NAN).format(), "   NaN\n");
        // A genuinely fractional neighbour still moves the whole matrix to
        // four decimals, and the non-finite element rides along in it.
        assert_eq!(
            Matrix::row(vec![1.5, f64::NAN]).format(),
            "    1.5000       NaN\n"
        );
        // `-Inf` is four characters wide but has no digits, so it does not
        // widen the column past what the numbers ask for.
        assert_eq!(
            Matrix::row(vec![1.0, f64::NEG_INFINITY]).format(),
            "     1  -Inf\n"
        );
        // A number that does need the room still gets it.
        assert_eq!(
            Matrix::row(vec![-12345.0, f64::NAN]).format(),
            "      -12345         NaN\n"
        );
    }

    // ---- classes and their display -----------------------------------

    fn logical(rows: usize, cols: usize, row_major: &[f64]) -> Matrix {
        rmat(rows, cols, row_major).with_class(Class::Logical)
    }

    /// Acceptance tests 1 and 2 (QA D38): four-wide logical columns, and
    /// the `logical` headers of a named display.
    #[test]
    fn a_logical_displays_in_four_wide_columns_under_its_header() {
        let t = Matrix::from_bool(true);
        assert_eq!(t.format(), "   1\n");
        assert_eq!(t.disp_text(), "   1\n");
        assert_eq!(Value::Mat(t).display("x"), "x =\n\n  logical\n\n   1\n\n");
        let m = logical(1, 3, &[0.0, 1.0, 1.0]);
        assert_eq!(m.disp_text(), "   0   1   1\n");
        assert_eq!(
            Value::Mat(m).display("x"),
            "x =\n\n  1×3 logical array\n\n   0   1   1\n\n"
        );
        // Twenty four-wide columns fit in 80.
        let wide = Matrix::filled(1, 21, 1.0)
            .with_class(Class::Logical)
            .format();
        assert!(wide.starts_with("  Columns 1 through 20\n\n"), "{wide}");
    }

    /// Acceptance test 14: a 1-row char is quoted, a multi-row char has a
    /// header and one quoted row per line; `disp` is bare either way.
    #[test]
    fn a_char_displays_quoted_and_disp_is_bare() {
        let s = Matrix::char_row("abc");
        assert_eq!(Value::Mat(s.clone()).display("s"), "s =\n\n    'abc'\n\n");
        assert_eq!(s.disp_text(), "abc\n");
        let c = rmat(2, 2, &[97.0, 98.0, 99.0, 100.0]).with_class(Class::Char);
        assert_eq!(
            Value::Mat(c.clone()).display("c"),
            "c =\n\n  2×2 char array\n\n    'ab'\n    'cd'\n\n"
        );
        assert_eq!(c.disp_text(), "ab\ncd\n");
        // A char column is one character per row.
        let col = s.transpose();
        assert_eq!(col.class, Class::Char);
        assert_eq!(col.disp_text(), "a\nb\nc\n");
        // `disp('')` is still one empty line.
        assert_eq!(Matrix::char_row("").disp_text(), "\n");
    }

    /// Acceptance tests 9 and 19: an empty says what it is, except the 0x0
    /// double, which is still `[]`.
    #[test]
    fn an_empty_displays_its_class_and_shape() {
        let body = |m: Matrix| m.display_body();
        assert_eq!(body(Matrix::empty()), "     []\n");
        assert_eq!(
            body(Matrix::new(0, 3, vec![])),
            "  0×3 empty double matrix\n"
        );
        assert_eq!(
            body(Matrix::new(1, 0, vec![])),
            "  1×0 empty double row vector\n"
        );
        assert_eq!(
            body(Matrix::new(0, 1, vec![])),
            "  0×1 empty double column vector\n"
        );
        assert_eq!(body(Matrix::char_row("")), "  0×0 empty char array\n");
        assert_eq!(
            body(Matrix::new(0, 3, vec![]).with_class(Class::Logical)),
            "  0×3 empty logical array\n"
        );
        assert_eq!(
            Value::Mat(Matrix::new(0, 3, vec![])).display("x"),
            "x =\n\n  0×3 empty double matrix\n\n"
        );
        // `disp` of any empty that is not a char prints nothing.
        assert_eq!(Matrix::new(0, 3, vec![]).disp_text(), "");
        assert_eq!(
            Matrix::new(0, 3, vec![])
                .with_class(Class::Logical)
                .disp_text(),
            ""
        );
    }

    /// The class tag is a property of the array, and conversion is explicit.
    #[test]
    fn to_class_converts_and_with_class_only_retags() {
        let m = Matrix::row(vec![2.0, 0.0, -1.0]);
        let l = m.clone().to_class(Class::Logical).unwrap();
        assert_eq!(l.class, Class::Logical);
        assert_eq!(l.data, [1.0, 0.0, 1.0]);
        assert_eq!(
            Matrix::scalar(f64::NAN)
                .to_class(Class::Logical)
                .unwrap_err()
                .msg,
            "NaN's cannot be converted to logicals."
        );
        let c = Matrix::row(vec![72.0, 105.4, -3.0, 70000.0, f64::NAN])
            .to_class(Class::Char)
            .unwrap();
        assert_eq!(c.data, [72.0, 105.0, 0.0, 65535.0, 0.0]);
        let d = Matrix::char_row("A").to_class(Class::Double).unwrap();
        assert_eq!((d.class, d.data.as_slice()), (Class::Double, &[65.0][..]));
        // Every numeric constructor is a double; `with_class` changes only
        // the tag.
        assert_eq!(Matrix::scalar(1.0).class, Class::Double);
        assert_eq!(m.with_class(Class::Char).data, [2.0, 0.0, -1.0]);
        // Arithmetic helpers build doubles from any class.
        let s = Matrix::char_row("ab");
        assert_eq!(s.map(|x| x + 1.0).class, Class::Double);
        assert_eq!(s.zip(&s, "+", |x, y| x + y).unwrap().class, Class::Double);
        assert_eq!(Class::Logical.name(), "logical");
    }

    /// QA D37: a char element is a UTF-16 code unit, so a character outside
    /// the Basic Multilingual Plane is two elements, and output puts the
    /// pair back together.
    #[test]
    fn char_storage_is_utf16_code_units() {
        let emoji = Matrix::char_row("😀");
        assert_eq!((emoji.rows, emoji.cols), (1, 2));
        assert_eq!(emoji.data, [55357.0, 56832.0]);
        assert_eq!(emoji.text(), "😀");
        assert_eq!(emoji.disp_text(), "😀\n");
        let e = Matrix::char_row("é");
        assert_eq!(e.data, [233.0]);
        let mixed = "a😀é\u{10FFFF}z";
        assert_eq!(Matrix::char_row(mixed).text(), mixed);
        assert_eq!(Matrix::char_row(mixed).numel(), 7);
        // A lone surrogate cannot be decoded and becomes U+FFFD.
        assert_eq!(decode_units([55357.0].into_iter()), "\u{FFFD}");
        assert_eq!(
            decode_units([56832.0, 55357.0].into_iter()),
            "\u{FFFD}\u{FFFD}"
        );
    }

    /// A matrix too wide for the display is split into blocks of columns, as
    /// MATLAB does; it used to print on one unwrapped line.
    #[test]
    fn a_wide_matrix_wraps_into_column_blocks() {
        // Integer columns are 6 wide, so 13 of them fit in 80 characters.
        let x = Matrix::row((1..=20).map(f64::from).collect::<Vec<f64>>());
        let out = x.format();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "  Columns 1 through 13");
        assert_eq!(lines[1], "");
        assert!(lines[2].starts_with("     1     2"), "{:?}", lines[2]);
        assert!(lines[2].ends_with("    13"), "{:?}", lines[2]);
        assert_eq!(lines[3], "");
        assert_eq!(lines[4], "  Columns 14 through 20");
        assert_eq!(lines[5], "");
        assert!(lines[6].starts_with("    14"), "{:?}", lines[6]);
        assert_eq!(lines.len(), 7);

        // Exactly the width that fits is not wrapped at all, and so carries
        // no heading.
        let fits = Matrix::row(vec![1.0; 13]).format();
        assert_eq!(fits.lines().count(), 1);
        assert!(!fits.contains("Column"));

        // Every row of a block is printed before the next block starts.
        let two = Matrix::filled(2, 14, 7.0).format();
        let lines: Vec<&str> = two.lines().collect();
        assert_eq!(lines[0], "  Columns 1 through 13");
        assert_eq!(lines.len(), 9);
        assert_eq!(lines[5], "  Column 14");

        // MATLAB's singular and two-column spellings of the heading.
        assert_eq!(column_header(3, 3), "  Column 3");
        assert_eq!(column_header(3, 4), "  Columns 3 and 4");
        assert_eq!(column_header(3, 5), "  Columns 3 through 5");
    }

    /// A `NaN` is neither true nor false. It used to convert silently, so
    /// `if NaN` was taken as true (QA D5).
    #[test]
    fn a_nan_cannot_become_a_logical() {
        let nan = Matrix::scalar(f64::NAN);
        for e in [
            nan.truth().unwrap_err(),
            nan.logical_scalar().unwrap_err(),
            Matrix::logical_element(f64::NAN).unwrap_err(),
            // One NaN anywhere is enough, as it is for MATLAB's own `if`.
            Matrix::row(vec![1.0, f64::NAN]).truth().unwrap_err(),
        ] {
            assert_eq!(e.msg, "NaN's cannot be converted to logicals.");
        }
        // Every other value converts as it always did, `Inf` included.
        assert!(Matrix::scalar(f64::INFINITY).logical_scalar().unwrap());
        assert!(!Matrix::scalar(0.0).logical_scalar().unwrap());
        assert!(Matrix::logical_element(-2.0).unwrap());
    }

    /// `&&` and `||` need one value to branch on, so an array or an empty is
    /// an error. `truth`, which `if` uses, still accepts both.
    #[test]
    fn the_short_circuit_operators_need_a_logical_scalar() {
        let want = "Operands to the logical AND (&&) and OR (||) operators \
                    must be convertible to logical scalar values.";
        assert_eq!(
            Matrix::row(vec![1.0, 1.0])
                .logical_scalar()
                .unwrap_err()
                .msg,
            want
        );
        assert_eq!(Matrix::empty().logical_scalar().unwrap_err().msg, want);
        assert_eq!(
            Matrix::new(1, 0, Vec::new())
                .logical_scalar()
                .unwrap_err()
                .msg,
            want
        );
        // `if [1 1]` and `if []` are still legal, and still mean what they did.
        assert!(Matrix::row(vec![1.0, 1.0]).truth().unwrap());
        assert!(!Matrix::empty().truth().unwrap());
    }

    /// `''` is `0x0` in MATLAB, not the `1x0` a row of no characters would
    /// be, which is what made `size('')` report `1 0`.
    #[test]
    fn the_empty_string_is_zero_by_zero() {
        let m = Value::str("").into_mat().unwrap();
        assert_eq!((m.rows, m.cols, m.class), (0, 0, Class::Char));
        // A non-empty string is still the row of codes it always was.
        let ab = Value::str("ab").into_mat().unwrap();
        assert_eq!((ab.rows, ab.cols), (1, 2));
    }

    // ---- cells and structs (cycle 07) --------------------------------

    fn num(v: f64) -> Value {
        Value::Mat(Matrix::scalar(v))
    }

    #[test]
    fn a_cell_displays_its_header_and_one_summary_per_element() {
        let c = Value::cell(CellArray::row(vec![num(1.0), Value::str("ab")]));
        assert_eq!(
            c.display("c"),
            "c =\n\n  1×2 cell array\n\n    {[1]}    {'ab'}\n\n"
        );
        assert_eq!(c.disp_text(), "    {[1]}    {'ab'}\n");
        let empty = Value::cell(CellArray::default());
        assert_eq!(empty.display("x"), "x =\n\n  0×0 empty cell array\n\n");
        assert_eq!(empty.disp_text(), "");
        assert_eq!(
            Value::cell(CellArray::new(1, 0, vec![])).display_body(),
            "  1×0 empty cell array\n"
        );
    }

    #[test]
    fn a_cell_column_is_as_wide_as_its_widest_element() {
        // Numbers are padded inside their brackets, so they right-align;
        // anything else is padded before its closing brace.
        let c = Value::cell(CellArray::new(
            2,
            2,
            vec![
                num(1.0),
                Value::Mat(Matrix::row(vec![1.0, 2.0, 3.0])),
                Value::str("ab"),
                Value::cell(CellArray::row(vec![num(2.0)])),
            ],
        ));
        assert_eq!(
            c.disp_text(),
            "    {[       1]}    {'ab'    }\n    {1×3 double}    {1×1 cell}\n"
        );
        let wide = Value::cell(CellArray::row((0..30).map(|k| num(k as f64)).collect()));
        let text = wide.disp_text();
        assert!(text.starts_with("  Columns 1 through "), "{text}");
        assert!(
            text.lines().all(|l| l.chars().count() <= TERM_WIDTH),
            "{text}"
        );
    }

    #[test]
    fn a_nested_cell_is_summarised_not_expanded() {
        let mut v = Value::cell(CellArray::default());
        for _ in 0..1000 {
            v = Value::cell(CellArray::row(vec![v]));
        }
        assert_eq!(v.disp_text(), "    {1×1 cell}\n");
    }

    #[test]
    fn a_struct_displays_its_fields_aligned() {
        let s = Value::strukt(StructArray::scalar(
            vec!["a".into(), "long".into(), "c".into()],
            vec![
                num(1.0),
                Value::str("hi"),
                Value::Mat(Matrix::row(vec![1.0, 2.0])),
            ],
        ));
        assert_eq!(
            s.display("s"),
            "s = \n\n  struct with fields:\n\n       a: 1\n    long: 'hi'\n       c: [1 2]\n\n"
        );
        assert_eq!(
            s.disp_text(),
            "       a: 1\n    long: 'hi'\n       c: [1 2]\n"
        );
        let nested = Value::strukt(StructArray::scalar(
            vec!["in".into(), "m".into(), "e".into(), "c".into()],
            vec![
                s.clone(),
                Value::Mat(Matrix::filled(2, 2, 0.0)),
                blank(),
                Value::cell(CellArray::blanks(1, 2)),
            ],
        ));
        assert_eq!(
            nested.disp_text(),
            "    in: [1×1 struct]\n     m: [2×2 double]\n     e: []\n     c: {1×2 cell}\n"
        );
        let none = Value::strukt(StructArray::scalar(vec![], vec![]));
        assert_eq!(none.display_body(), "  struct with no fields.\n");
    }

    #[test]
    fn a_struct_array_lists_its_field_names() {
        let s = Value::strukt(StructArray::new(
            1,
            2,
            vec!["name".into()],
            vec![vec![Value::str("A")], vec![Value::str("B")]],
        ));
        assert_eq!(
            s.display_body(),
            "  1×2 struct array with fields:\n\n    name\n"
        );
        let empty = Value::strukt(StructArray::new(0, 1, vec!["file".into()], vec![]));
        assert_eq!(
            empty.display_body(),
            "  0×1 empty struct array with fields:\n\n    file\n"
        );
    }

    #[test]
    fn every_value_answers_class_dims_and_element() {
        let c = Value::cell(CellArray::blanks(2, 3));
        assert_eq!((c.class_name(), c.dims(), c.numel()), ("cell", (2, 3), 6));
        let e = c.element(4);
        assert!(matches!(&e, Value::Cell(x) if x.numel() == 1));
        let s = Value::strukt(StructArray::scalar(vec!["a".into()], vec![num(1.0)]));
        assert_eq!((s.class_name(), s.dims()), ("struct", (1, 1)));
        assert!(blank().is_blank());
        assert!(!Value::str("").is_blank());
    }

    #[test]
    fn struct_fields_are_added_in_every_element_and_reordered_on_demand() {
        let mut s = StructArray::new(1, 2, vec!["a".into()], vec![vec![num(1.0)], vec![num(2.0)]]);
        assert_eq!(s.ensure_field("b"), 1);
        assert_eq!(s.ensure_field("a"), 0);
        assert!(s.elems.iter().all(|e| e.len() == 2 && e[1].is_blank()));
        let t = StructArray::scalar(vec!["b".into(), "a".into()], vec![num(8.0), num(9.0)]);
        assert!(s.same_fields(&t));
        let vals = s.reordered(&t, 0);
        assert!(matches!(&vals[0], Value::Mat(m) if m.data == [9.0]));
    }

    /// Past `INDEXED_FIELDS` a lookup goes through the hash index, which
    /// must agree with the names at every step: built on the first lookup,
    /// kept up to date by `ensure_field`, first place for a name given twice,
    /// and carried by a clone.
    #[test]
    fn the_field_index_agrees_with_the_names() {
        let names: Vec<String> = (0..INDEXED_FIELDS + 5).map(|k| format!("f{k}")).collect();
        let mut s = StructArray::scalar(vec![], vec![]);
        for (k, n) in names.iter().enumerate() {
            assert_eq!(s.ensure_field(n), k);
            assert_eq!(s.field_index(n), Some(k));
            assert_eq!(s.field_index("f0"), Some(0));
        }
        assert_eq!(s.field_index("missing"), None);
        assert_eq!(s.ensure_field("f3"), 3);
        let mut t = s.clone();
        assert_eq!(t.ensure_field("late"), names.len());
        assert_eq!(
            (t.field_index("late"), s.field_index("late")),
            (Some(names.len()), None)
        );
        let mut twice = names.clone();
        twice.push("f1".into());
        let vals = vec![blank(); twice.len()];
        let d = StructArray::scalar(twice, vals);
        assert_eq!(d.field_index("f1"), Some(1));
    }

    /// Chains of every kind, freed on this test's own 2 MB thread: the
    /// default drop would recurse once per link and overflow it.
    #[test]
    fn deep_chains_are_freed_without_recursion() {
        let depth = 200_000;
        let mut v = Value::cell(CellArray::default());
        for _ in 0..depth {
            v = Value::cell(CellArray::row(vec![v]));
        }
        drop(v);
        let mut v = blank();
        for _ in 0..depth {
            v = Value::strukt(StructArray::scalar(vec!["a".into()], vec![v]));
        }
        drop(v);
        // Cell, handle, struct, in turn.
        let def = Rc::new(AnonFn::new(vec![], crate::parser::Expr::Num(1.0)));
        let mut v = blank();
        for k in 0..depth {
            v = match k % 3 {
                0 => Value::cell(CellArray::row(vec![v])),
                1 => Value::Func(Rc::new(Func::Anon {
                    def: def.clone(),
                    captured: vec![("c".into(), v)],
                    unit: Rc::new(Unit::default()),
                })),
                _ => Value::strukt(StructArray::scalar(vec!["h".into()], vec![v])),
            };
        }
        // A chain still shared is only released: freeing the other owner
        // later must still work.
        let other = v.clone();
        drop(v);
        drop(other);
    }
}
