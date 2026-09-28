//! Tree-walking interpreter.

use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Instant;

use crate::bail;
use crate::builtins::math::powf_real;
use crate::builtins::{self, Registry};
use crate::error;
use crate::lexer::scan;
use crate::parser::{BinOp, Expr, Located, MAX_DEPTH, Parser, Stmt};
use crate::value::{Class, Matrix, Value, nonfinite};

/// Every fallible path in the interpreter returns this. It lives in
/// `error.rs`; the re-export is what let cycle 01b swap `String` for `MError`
/// without touching a single `use crate::interp::R` in `builtins/`.
pub use crate::error::R;

pub struct Interp {
    pub vars: HashMap<String, Value>,
    /// Value of `end` for the index argument currently being evaluated.
    end_stack: Vec<usize>,
    rng: u64,
    /// The builtin library, built once here and never changed afterwards.
    builtins: Registry,
    /// When this interpreter started, the origin for every tic/toc reading.
    start: Instant,
    /// Nanoseconds since `start` at the last bare `tic`, and `None` before
    /// the first one: a bare `toc` then has nothing to measure from.
    pub(crate) tic_mark: Option<f64>,
    /// How deeply nested the statement or expression being run is. The
    /// evaluator recurses once per level just as the parser does, so it uses
    /// the parser's `MAX_DEPTH`: anything the parser accepted, this can walk.
    depth: usize,
    /// How many `for` or `while` loops are running. `break` and `continue`
    /// outside every one of them are errors rather than a silent end to the
    /// script (QA D8).
    loop_depth: usize,
    /// Everything the interpreter prints goes here. Nothing in this crate
    /// outside `main.rs` may use `print!`, so tests can capture output.
    pub out: Box<dyn Write>,
}

enum Flow {
    Normal,
    Break,
    Continue,
}

/// One index argument after evaluation.
enum Sel {
    All,
    /// 0-based indices plus the shape of the index expression.
    List(Vec<usize>, usize, usize),
}

impl Default for Interp {
    fn default() -> Self {
        Self::new()
    }
}

impl Interp {
    pub fn new() -> Self {
        Self::with_output(Box::new(io::stdout()))
    }

    /// Builds an interpreter that writes everything to `out`.
    pub fn with_output(out: Box<dyn Write>) -> Self {
        Interp {
            vars: HashMap::new(),
            end_stack: Vec::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            builtins: builtins::registry(),
            start: Instant::now(),
            tic_mark: None,
            depth: 0,
            loop_depth: 0,
            out,
        }
    }

    /// Counts one more level of evaluation, refusing anything past the
    /// parser's [`MAX_DEPTH`]. The mirror of `Parser::deepen`: the two
    /// recursions run to the same depth on the same tree, so they share the
    /// one limit and the one message.
    fn deepen(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            bail!(error::nesting_too_deep(MAX_DEPTH));
        }
        Ok(())
    }

    /// The single place interpreter output leaves the evaluator.
    pub(crate) fn emit(&mut self, s: &str) -> R<()> {
        self.out.write_all(s.as_bytes()).map_err(error::output)
    }

    pub fn run(&mut self, src: &str) -> R<()> {
        // An entry that failed part-way left its counters raised, and the
        // REPL hands the same interpreter the next line. Without this, one
        // over-deep expression would make every later statement too deep and
        // a `break` left mid-loop would make a later top-level one legal.
        self.depth = 0;
        self.loop_depth = 0;
        let lexed = scan(src)?;
        let stmts = Parser::with_lines(lexed).parse_program()?;
        self.exec_block(&stmts)?;
        Ok(())
    }

    // ---- statements --------------------------------------------------

    /// Runs a block, tagging anything that fails with the line of the
    /// statement it came out of.
    ///
    /// `MError::at` keeps the first line it is given, so the innermost block
    /// wins: an error raised inside a `for` body reports the body's line,
    /// not the line of the `for` that is unwinding around it.
    fn exec_block(&mut self, stmts: &[Located]) -> R<Flow> {
        for s in stmts {
            // A block inside a block recurses through here, so this is where
            // statement nesting is counted, matching `Parser::parse_block`.
            self.deepen()?;
            let flow = self.exec(&s.stmt).map_err(|e| e.at(s.line));
            self.depth -= 1;
            match flow? {
                Flow::Normal => {}
                f => return Ok(f),
            }
        }
        Ok(Flow::Normal)
    }

    fn exec(&mut self, stmt: &Stmt) -> R<Flow> {
        match stmt {
            Stmt::Expr(e, show) => {
                // A statement asks for no values, so a builtin that produces
                // none (disp, fprintf, tic, ...) is legal here and returns an
                // empty Vec. Nothing else calls a builtin in this cycle.
                let result = match e {
                    Expr::Ident(n) if !self.vars.contains_key(n) => {
                        self.call_builtin(n, vec![], 0)?
                    }
                    Expr::Index(n, args) if !self.vars.contains_key(n) => {
                        let a = self.eval_args(args)?;
                        self.call_builtin(n, a, 0)?
                    }
                    _ => vec![self.eval(e)?],
                };
                if let Some(v) = result.into_iter().next() {
                    let name = match e {
                        Expr::Ident(n) if self.vars.contains_key(n) => n.clone(),
                        _ => "ans".to_string(),
                    };
                    if name == "ans" {
                        self.vars.insert(name.clone(), v.clone());
                    }
                    if *show {
                        self.emit(&v.display(&name))?;
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Assign(name, e, show) => {
                let v = self.eval(e)?;
                if *show {
                    self.emit(&v.display(name))?;
                }
                self.vars.insert(name.clone(), v);
                Ok(Flow::Normal)
            }
            Stmt::IndexAssign(name, args, e, show) => {
                let rhs = self.eval(e)?;
                self.assign_index(name, args, rhs)?;
                if *show {
                    let shown = self.vars[name].display(name);
                    self.emit(&shown)?;
                }
                Ok(Flow::Normal)
            }
            Stmt::If(arms, otherwise) => {
                for a in arms {
                    // The arm's own line, so an error in an `elseif`
                    // condition names the `elseif` and not the `if` that
                    // opened the statement (QA D27). `MError::at` keeps the
                    // first line it is given, so this wins over the `if`'s.
                    let cond = self
                        .eval_mat(&a.cond)
                        .and_then(|m| m.truth())
                        .map_err(|e| e.at(a.line))?;
                    if cond {
                        return self.exec_block(&a.body);
                    }
                }
                if let Some(body) = otherwise {
                    return self.exec_block(body);
                }
                Ok(Flow::Normal)
            }
            Stmt::For(name, e, body) => {
                let m = self.eval_mat(e)?;
                // A loop that does not run still assigns its variable, as
                // MATLAB does: after `k = 7; for k = []; end`, `k` is the
                // empty, not `7`, and a name that did not exist comes into
                // existence. The value is the empty that the next column
                // would have been, which gives `[]` a 0x0 and `1:0` a 1x0,
                // the shapes Octave assigns. MATLAB's exact shape is
                // unsettled, so no golden case asserts it.
                if m.cols == 0 {
                    let empty = Matrix::new(m.rows, 0, Vec::new()).with_class(m.class);
                    self.vars.insert(name.clone(), Value::Mat(empty));
                }
                self.loop_depth += 1;
                let flow = self.run_for(name, &m, body);
                self.loop_depth -= 1;
                flow?;
                Ok(Flow::Normal)
            }
            Stmt::While(cond, body) => {
                self.loop_depth += 1;
                let flow = self.run_while(cond, body);
                self.loop_depth -= 1;
                flow?;
                Ok(Flow::Normal)
            }
            // MATLAB errors on a `break` with no loop around it; Octave 8.4
            // gives a parse error. It is raised here rather than in the
            // parser so that everything the script printed first is still
            // printed, which is what the script's own output shows.
            Stmt::Break if self.loop_depth == 0 => Err(error::break_outside_loop()),
            Stmt::Continue if self.loop_depth == 0 => Err(error::continue_outside_loop()),
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
        }
    }

    /// The iterations of a `for`, split out so the caller can put
    /// `loop_depth` back however the body ends: normally, on a `break`, or on
    /// an error unwinding through it. A `loop_depth` left raised would make a
    /// later top-level `break` legal.
    fn run_for(&mut self, name: &str, m: &Matrix, body: &[Located]) -> R<()> {
        for c in 0..m.cols {
            let column: Vec<f64> = (0..m.rows).map(|r| m.get(r, c)).collect();
            // Each column keeps the class, so `for k = 'abc'` iterates chars.
            let v = if m.rows == 1 {
                Matrix::scalar(column[0])
            } else {
                Matrix::col(column)
            }
            .with_class(m.class);
            self.vars.insert(name.to_string(), Value::Mat(v));
            if let Flow::Break = self.exec_block(body)? {
                break;
            }
        }
        Ok(())
    }

    /// The iterations of a `while`; see [`run_for`](Interp::run_for).
    fn run_while(&mut self, cond: &Expr, body: &[Located]) -> R<()> {
        while self.eval_mat(cond)?.truth()? {
            if let Flow::Break = self.exec_block(body)? {
                break;
            }
        }
        Ok(())
    }

    // ---- expressions -------------------------------------------------

    /// Evaluates one expression, counting its nesting against [`MAX_DEPTH`].
    fn eval(&mut self, e: &Expr) -> R<Value> {
        self.deepen()?;
        let v = self.eval_node(e);
        self.depth -= 1;
        v
    }

    fn eval_node(&mut self, e: &Expr) -> R<Value> {
        match e {
            Expr::Num(v) => Ok(Value::Mat(Matrix::scalar(*v))),
            // A string literal is a 1-row char of UTF-16 code units.
            Expr::Str(s) => Ok(Value::str(s)),
            Expr::Ident(n) => {
                if let Some(v) = self.vars.get(n) {
                    return Ok(v.clone());
                }
                self.call_for_value(n, vec![])
            }
            Expr::End => self
                .end_stack
                .last()
                .map(|n| Value::Mat(Matrix::scalar(*n as f64)))
                .ok_or_else(error::end_outside_index),
            Expr::Colon => Err(error::colon_outside_index()),
            Expr::Index(n, args) => self.index(n, args),
            Expr::Matrix(rows) => self.build_matrix(rows),
            // Both signs are arithmetic, so both give a double: `+'a'` is 97.
            Expr::Neg(a) => Ok(Value::Mat(self.eval_mat(a)?.map(|x| -x))),
            Expr::Pos(a) => Ok(Value::Mat(self.eval_mat(a)?.map(|x| x))),
            // `~` converts each element to a logical first, so `~NaN` is the
            // same refusal as `if NaN` rather than the `0` it used to give.
            Expr::Not(a) => Ok(Value::Mat(
                self.eval_mat(a)?
                    .try_map(|x| Ok(!Matrix::logical_element(x)? as u8 as f64))?
                    .with_class(Class::Logical),
            )),
            Expr::Transpose(a) => Ok(Value::Mat(self.eval_mat(a)?.transpose())),
            Expr::Range(a, step, b) => {
                let a = self.eval_scalar(a, "range start")?;
                let b = self.eval_scalar(b, "range end")?;
                let s = match step {
                    Some(s) => self.eval_scalar(s, "range step")?,
                    None => 1.0,
                };
                Ok(Value::Mat(range(a, s, b)?))
            }
            Expr::Binary(op, a, b) => self.binary(*op, a, b),
        }
    }

    fn eval_mat(&mut self, e: &Expr) -> R<Matrix> {
        Ok(self.eval(e)?.into_mat())
    }

    fn eval_scalar(&mut self, e: &Expr, what: &str) -> R<f64> {
        self.eval_mat(e)?
            .scalar_value()
            .ok_or_else(|| error::not_a_scalar(what))
    }

    fn eval_args(&mut self, args: &[Expr]) -> R<Vec<Value>> {
        args.iter().map(|a| self.eval(a)).collect()
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> R<Value> {
        // Short-circuit logical operators. Each operand must be convertible to
        // one logical value, so `[1 1] && 1` and `[] || 1` are errors rather
        // than `1`; the right-hand side is still not evaluated when the left
        // already decides the answer, so `0 && undefined_fn()` is `0`.
        match op {
            BinOp::AndAnd => {
                let l = self.eval_mat(a)?.logical_scalar()?;
                let v = l && self.eval_mat(b)?.logical_scalar()?;
                return Ok(Value::Mat(Matrix::from_bool(v)));
            }
            BinOp::OrOr => {
                let l = self.eval_mat(a)?.logical_scalar()?;
                let v = l || self.eval_mat(b)?.logical_scalar()?;
                return Ok(Value::Mat(Matrix::from_bool(v)));
            }
            _ => {}
        }
        let a = self.eval_mat(a)?;
        let b = self.eval_mat(b)?;
        let bool_op =
            |f: fn(f64, f64) -> bool| move |x: f64, y: f64| if f(x, y) { 1.0 } else { 0.0 };
        let r = match op {
            BinOp::Add => a.zip(&b, "+", |x, y| x + y)?,
            BinOp::Sub => a.zip(&b, "-", |x, y| x - y)?,
            BinOp::EMul => a.zip(&b, ".*", |x, y| x * y)?,
            BinOp::EDiv => a.zip(&b, "./", |x, y| x / y)?,
            // `a.\b` divides the other way round, element by element.
            BinOp::ELDiv => a.zip(&b, ".\\", |x, y| y / x)?,
            BinOp::EPow => a.try_zip(&b, ".^", powf_real)?,
            BinOp::Mul => {
                if a.is_scalar() || b.is_scalar() {
                    a.zip(&b, "*", |x, y| x * y)?
                } else {
                    a.matmul(&b)?
                }
            }
            BinOp::Div => {
                if b.is_scalar() {
                    a.zip(&b, "/", |x, y| x / y)?
                } else {
                    // a / b  ==  (b' \ a')'
                    b.transpose().solve(&a.transpose())?.transpose()
                }
            }
            BinOp::LDiv => {
                if a.is_scalar() {
                    a.zip(&b, "\\", |x, y| y / x)?
                } else {
                    a.solve(&b)?
                }
            }
            BinOp::Pow => {
                if a.is_scalar() && b.is_scalar() {
                    Matrix::scalar(powf_real(a.data[0], b.data[0])?)
                } else if let Some(p) = b.scalar_value() {
                    matrix_power(&a, p)?
                } else {
                    bail!(error::matrix_exponent());
                }
            }
            BinOp::Eq => a.zip(&b, "==", bool_op(|x, y| x == y))?,
            BinOp::Ne => a.zip(&b, "~=", bool_op(|x, y| x != y))?,
            BinOp::Lt => a.zip(&b, "<", bool_op(|x, y| x < y))?,
            BinOp::Le => a.zip(&b, "<=", bool_op(|x, y| x <= y))?,
            BinOp::Gt => a.zip(&b, ">", bool_op(|x, y| x > y))?,
            BinOp::Ge => a.zip(&b, ">=", bool_op(|x, y| x >= y))?,
            // Element-wise `&` and `|` convert each pair to logicals first,
            // so `NaN & 1` is a refusal rather than `1`.
            // Both elements are converted before either is looked at, since
            // the conversion is what can fail: `1 | NaN` is a refusal even
            // though a short-circuit would never have read the `NaN`.
            BinOp::And => a.try_zip(&b, "&", |x, y| {
                let (x, y) = (Matrix::logical_element(x)?, Matrix::logical_element(y)?);
                Ok((x && y) as u8 as f64)
            })?,
            BinOp::Or => a.try_zip(&b, "|", |x, y| {
                let (x, y) = (Matrix::logical_element(x)?, Matrix::logical_element(y)?);
                Ok((x || y) as u8 as f64)
            })?,
            BinOp::AndAnd | BinOp::OrOr => unreachable!(),
        };
        // Arithmetic is always a double, whatever its operands were:
        // `true + true` is `2` and `'a' + 1` is `98`. Comparisons and the
        // element-wise logical operators are logical.
        let class = match op {
            BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Le
            | BinOp::Gt
            | BinOp::Ge
            | BinOp::And
            | BinOp::Or => Class::Logical,
            _ => Class::Double,
        };
        Ok(Value::Mat(r.with_class(class)))
    }

    fn build_matrix(&mut self, rows: &[Vec<Expr>]) -> R<Value> {
        let mut row_vals = Vec::with_capacity(rows.len());
        for row in rows {
            let mut elems = Vec::with_capacity(row.len());
            for e in row {
                elems.push(self.eval(e)?);
            }
            row_vals.push(hcat(elems)?);
        }
        vcat(row_vals)
    }

    // ---- indexing ----------------------------------------------------

    fn index(&mut self, name: &str, args: &[Expr]) -> R<Value> {
        let var = match self.vars.get(name) {
            Some(v) => v.clone(),
            None => {
                let a = self.eval_args(args)?;
                return self.call_for_value(name, a);
            }
        };
        let m = var.into_mat();
        let sel = self.eval_index_args(&m, args)?;
        // Indexing keeps the class, so `s(2)` of a char is a char and `s(:)`
        // is a char column (QA D17).
        Ok(Value::Mat(index_read(&m, &sel)?))
    }

    fn eval_index_args(&mut self, m: &Matrix, args: &[Expr]) -> R<Vec<Sel>> {
        if args.is_empty() || args.len() > 2 {
            bail!(error::indexing_rank());
        }
        let mut out = Vec::with_capacity(args.len());
        for (k, a) in args.iter().enumerate() {
            if matches!(a, Expr::Colon) {
                out.push(Sel::All);
                continue;
            }
            let end_val = if args.len() == 1 {
                m.numel()
            } else if k == 0 {
                m.rows
            } else {
                m.cols
            };
            self.end_stack.push(end_val);
            let v = self.eval_mat(a);
            self.end_stack.pop();
            let v = v?;
            // A mask such as `x > 0` must never be read as a list of
            // positions, which made `x(x > 0)` give `5 5 5` (QA D6). Cycle 03
            // replaces this refusal with logical indexing itself.
            if v.class == Class::Logical {
                bail!(error::logical_indexing_unsupported());
            }
            let mut idx = Vec::with_capacity(v.numel());
            for x in &v.data {
                if x.fract() != 0.0 || *x < 1.0 {
                    bail!(error::index_not_positive_integer(k + 1));
                }
                idx.push(*x as usize - 1);
            }
            out.push(Sel::List(idx, v.rows, v.cols));
        }
        Ok(out)
    }

    fn assign_index(&mut self, name: &str, args: &[Expr], rhs: Value) -> R<()> {
        let rhs = rhs.into_mat();
        if rhs.is_empty() {
            bail!(error::deletion_unsupported());
        }
        // The left-hand side keeps its class, so `s(1) = 'X'` of a char stays
        // a char and `y(2) = 'a'` of a double stores `97`. A variable that
        // does not exist yet, or is the 0x0 double `[]`, takes the class of
        // what is assigned into it, which is how `s = []; s(1) = 'a'` builds
        // a char.
        let mut m = match self.vars.get(name) {
            Some(v) => v.clone().into_mat(),
            None => Matrix::empty(),
        };
        if m.class == Class::Double && m.rows == 0 && m.cols == 0 {
            m.class = rhs.class;
        }
        let rhs = rhs.to_class(m.class)?;
        let sel = self.eval_index_args(&m, args)?;
        if sel.len() == 1 {
            let idx: Vec<usize> = match &sel[0] {
                Sel::All => (0..m.numel()).collect(),
                Sel::List(i, _, _) => i.clone(),
            };
            let need = idx.iter().copied().max().map_or(0, |x| x + 1);
            if need > m.numel() {
                crate::builtins::args::check_size(1, need)?;
                if m.is_empty() {
                    m = Matrix::filled(1, need, 0.0).with_class(m.class);
                } else if m.rows == 1 {
                    m.data.resize(need, 0.0);
                    m.cols = need;
                } else if m.cols == 1 {
                    m.data.resize(need, 0.0);
                    m.rows = need;
                } else {
                    bail!(error::ambiguous_growth());
                }
            }
            if !rhs.is_scalar() && rhs.numel() != idx.len() {
                bail!(error::assignment_size(idx.len(), rhs.numel()));
            }
            for (n, &k) in idx.iter().enumerate() {
                m.data[k] = if rhs.is_scalar() {
                    rhs.data[0]
                } else {
                    rhs.data[n]
                };
            }
        } else {
            let rows: Vec<usize> = match &sel[0] {
                Sel::All => (0..if m.rows == 0 { rhs.rows } else { m.rows }).collect(),
                Sel::List(i, _, _) => i.clone(),
            };
            let cols: Vec<usize> = match &sel[1] {
                Sel::All => (0..if m.cols == 0 { rhs.cols } else { m.cols }).collect(),
                Sel::List(i, _, _) => i.clone(),
            };
            let nr = rows.iter().max().map_or(m.rows, |x| (x + 1).max(m.rows));
            let nc = cols.iter().max().map_or(m.cols, |x| (x + 1).max(m.cols));
            if nr != m.rows || nc != m.cols {
                crate::builtins::args::check_size(nr, nc)?;
                let mut grown = Matrix::filled(nr, nc, 0.0).with_class(m.class);
                for c in 0..m.cols {
                    for r in 0..m.rows {
                        grown.set(r, c, m.get(r, c));
                    }
                }
                m = grown;
            }
            let count = rows.len() * cols.len();
            if !rhs.is_scalar() && rhs.numel() != count {
                bail!(error::assignment_size(count, rhs.numel()));
            }
            let mut n = 0;
            for &c in &cols {
                for &r in &rows {
                    let v = if rhs.is_scalar() {
                        rhs.data[0]
                    } else {
                        rhs.data[n]
                    };
                    m.set(r, c, v);
                    n += 1;
                }
            }
        }
        self.vars.insert(name.to_string(), Value::Mat(m));
        Ok(())
    }

    // ---- builtins ----------------------------------------------------

    pub(crate) fn next_rand(&mut self) -> f64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (r >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Nanoseconds since this interpreter started: the clock behind tic/toc.
    pub(crate) fn clock_nanos(&self) -> f64 {
        self.start.elapsed().as_nanos() as f64
    }

    /// Calls the builtin `name`, asking it for `nargout` values. An empty
    /// result means it produced none.
    ///
    /// The function pointer is copied out of the registry before the call:
    /// `self.builtins.get` borrows `self` immutably while the builtin wants
    /// `&mut Interp`, and `BuiltinFn` being a plain `fn` makes the copy free.
    fn call_builtin(&mut self, name: &str, args: Vec<Value>, nargout: usize) -> R<Vec<Value>> {
        match self.builtins.get(name).map(|e| e.f) {
            Some(f) => f(self, &args, nargout),
            None => Err(error::undefined(name)),
        }
    }

    /// The single value an expression wants from a builtin.
    fn call_for_value(&mut self, name: &str, args: Vec<Value>) -> R<Value> {
        self.call_builtin(name, args, 1)?
            .into_iter()
            .next()
            .ok_or_else(error::too_many_outputs)
    }
}

// ---- helpers ---------------------------------------------------------

/// `a:b` and `a:s:b`. The `:` operator never reaches a builtin, so this is the
/// one remaining place a user-supplied number becomes an allocation length
/// without passing through a builtin. It uses `args::check_shape` rather than
/// a second policy, so `x = 1:1e15` gets the same limit and the same message
/// as `zeros(1e10)` instead of aborting in the allocator.
///
/// An infinite end point is no longer an early empty. `0:Inf` and `-Inf:1:0`
/// both have the step count `Inf`, which is what `check_shape` already refuses
/// for `zeros(1, Inf)`, so they report `1xInf` instead of quietly giving a
/// `1x0` and exit 0; MATLAB and Octave both refuse them. A `NaN` still gives
/// the empty: `1:NaN` stays as it was, and stays recorded in Known bugs,
/// because Octave gives a `1x1` `NaN` there and MATLAB is unverified.
///
/// An infinite *step* is no longer an early empty either. Cycle 01d refused
/// the end points and deliberately left the step alone; this is that
/// remainder. `1:Inf:5` follows the documented count `fix((k - j) / i)`,
/// which is `fix(4 / Inf)` and so `0`: a count of `0` is one element, the
/// start, so the answer is the `1x1` `1` rather than a `1x0`.
fn range(a: f64, s: f64, b: f64) -> R<Matrix> {
    // How many times the step fits between the end points; the vector has one
    // more element than that. `1:NaN` and `Inf:Inf` both make it `NaN`, which
    // is neither negative nor a count.
    let steps = (b - a) / s;
    // Whether the range is asked to run against its own step, which is what
    // makes `5:1` and `1:-1:5` empty. It is tested on the signs rather than
    // on `steps < 0.0`, because an infinite step divides the gap down to a
    // *signed zero*: `5:Inf:1` has `steps` of `-0.0`, which is not less than
    // zero, and would otherwise have produced the element `5`.
    let wrong_way = b != a && (b > a) != (s > 0.0);
    if s == 0.0 || steps.is_nan() || wrong_way {
        return Ok(Matrix::new(1, 0, Vec::new()));
    }
    // The count stays in `f64` until `check_shape` has judged it, so a count
    // past `usize` is named as asked: `0:1e-300:1e300` overflows `f64` itself
    // and reports `1xInf`, where it used to report the `usize::MAX` clamp.
    let n = (steps + 1e-10).floor();
    let (_, count) = crate::builtins::args::check_shape(1.0, n + 1.0)?;
    // MATLAB computes the upper half of a colon from the right-hand end point
    // rather than adding the step `n` times. That is what makes `0:0.1:0.3`
    // end exactly on `0.3` and `-1:0.01:1` symmetric: element `k` is
    // `a + k*s` and element `n - k` is `right - k*s`, so the pair sums to
    // `a + right` exactly, whatever the roundoff in `k*s`.
    //
    // The right-hand end point is `b` itself only when the range lands on it.
    // `n >= steps` says the floor above discarded no partial step (the `1e-10`
    // fuzz counts as landing), so `0:0.1:0.35` keeps `a + 3s` as its last
    // element rather than jumping to `0.35`.
    let right = if n >= steps { b } else { a + n * s };
    let n = n as usize;
    let data = (0..count)
        .map(|k| {
            // The first element is the start itself, spelled out rather than
            // computed as `a + 0 * s`: with an infinite step that product is
            // `NaN`, so `1:Inf:5` would have been `NaN` instead of `1`.
            if k == 0 {
                a
            } else if 2 * k <= n {
                a + k as f64 * s
            } else {
                right - (n - k) as f64 * s
            }
        })
        .collect();
    Ok(Matrix::row(data))
}

fn matrix_power(a: &Matrix, p: f64) -> R<Matrix> {
    if a.rows != a.cols {
        bail!(error::nonsquare_power());
    }
    if p.fract() != 0.0 {
        bail!(error::fractional_matrix_power());
    }
    let base = if p < 0.0 { a.inv()? } else { a.clone() };
    let mut n = p.abs() as u64;
    let mut result = Matrix::identity(a.rows, a.rows);
    let mut sq = base;
    while n > 0 {
        if n & 1 == 1 {
            result = result.matmul(&sq)?;
        }
        n >>= 1;
        if n > 0 {
            sq = sq.matmul(&sq)?;
        }
    }
    Ok(result)
}

/// `m(sel)`, keeping `m`'s class.
fn index_read(m: &Matrix, sel: &[Sel]) -> R<Matrix> {
    Ok(index_values(m, sel)?.with_class(m.class))
}

fn index_values(m: &Matrix, sel: &[Sel]) -> R<Matrix> {
    if sel.len() == 1 {
        match &sel[0] {
            Sel::All => Ok(Matrix::col(m.data.clone())),
            Sel::List(idx, ir, ic) => {
                let mut data = Vec::with_capacity(idx.len());
                for &k in idx {
                    if k >= m.numel() {
                        bail!(error::index_exceeds_numel(m.numel()));
                    }
                    data.push(m.data[k]);
                }
                // Vector indexed by a vector keeps the source orientation.
                let (rows, cols) = if m.is_vector() && (*ir == 1 || *ic == 1) {
                    if m.rows == 1 {
                        (1, idx.len())
                    } else {
                        (idx.len(), 1)
                    }
                } else {
                    (*ir, *ic)
                };
                Ok(Matrix::new(rows, cols, data))
            }
        }
    } else {
        let rows: Vec<usize> = match &sel[0] {
            Sel::All => (0..m.rows).collect(),
            Sel::List(i, _, _) => i.clone(),
        };
        let cols: Vec<usize> = match &sel[1] {
            Sel::All => (0..m.cols).collect(),
            Sel::List(i, _, _) => i.clone(),
        };
        if rows.iter().any(|r| *r >= m.rows) {
            bail!(error::index_exceeds_bound(1, m.rows));
        }
        if cols.iter().any(|c| *c >= m.cols) {
            bail!(error::index_exceeds_bound(2, m.cols));
        }
        // A two-subscript read sizes its result from the subscripts, not from
        // the array: `A(ones(1, 1e5), ones(1, 1e5))` asks for 1e10 elements
        // out of a 2x2 `A`, and used to abort in the allocator on the
        // `with_capacity` below. The bounds tests come first, so an
        // out-of-range subscript is still reported as one.
        crate::builtins::args::check_shape(rows.len() as f64, cols.len() as f64)?;
        let mut data = Vec::with_capacity(rows.len() * cols.len());
        for &c in &cols {
            for &r in &rows {
                data.push(m.get(r, c));
            }
        }
        Ok(Matrix::new(rows.len(), cols.len(), data))
    }
}

/// The class of a concatenation: `Char` if any operand is a char, else
/// `Logical` if every operand is a logical, else `Double`.
///
/// A 0x0 double takes no part in the vote, so `[[] 'abc']` is a char and
/// `s = []; s = [s 'abc']` builds one (QA D17). It is what `[]` is, and it
/// contributes no elements. Only when every operand is one does the vote
/// fall back to all of them, which makes `[[] []]` the double it always was.
fn concat_class(mats: &[Matrix]) -> Class {
    let is_blank = |m: &&Matrix| m.class == Class::Double && m.rows == 0 && m.cols == 0;
    let voters: Vec<&Matrix> = if mats.iter().all(|m| is_blank(&m)) {
        mats.iter().collect()
    } else {
        mats.iter().filter(|m| !is_blank(m)).collect()
    };
    if voters.iter().any(|m| m.class == Class::Char) {
        Class::Char
    } else if !voters.is_empty() && voters.iter().all(|m| m.class == Class::Logical) {
        Class::Logical
    } else {
        Class::Double
    }
}

fn hcat(vals: Vec<Value>) -> R<Value> {
    let all: Vec<Matrix> = vals.into_iter().map(|v| v.into_mat()).collect();
    let class = concat_class(&all);
    let mats: Vec<Matrix> = all.into_iter().filter(|m| !m.is_empty()).collect();
    if mats.is_empty() {
        return Ok(Value::Mat(Matrix::empty().with_class(class)));
    }
    let rows = mats[0].rows;
    if mats.iter().any(|m| m.rows != rows) {
        bail!(error::concat_dims());
    }
    let mut data = Vec::new();
    let mut cols = 0;
    for m in &mats {
        data.extend_from_slice(&m.data);
        cols += m.cols;
    }
    // A number joining a char becomes the character with that code: `['a'
    // 66]` is `'aB'`.
    Ok(Value::Mat(Matrix::new(rows, cols, data).to_class(class)?))
}

fn vcat(vals: Vec<Value>) -> R<Value> {
    if vals.len() == 1 {
        return Ok(vals.into_iter().next().unwrap());
    }
    let all: Vec<Matrix> = vals.into_iter().map(|v| v.into_mat()).collect();
    let class = concat_class(&all);
    let mats: Vec<Matrix> = all.into_iter().filter(|m| !m.is_empty()).collect();
    if mats.is_empty() {
        return Ok(Value::Mat(Matrix::empty().with_class(class)));
    }
    let cols = mats[0].cols;
    if mats.iter().any(|m| m.cols != cols) {
        bail!(error::concat_dims());
    }
    let rows: usize = mats.iter().map(|m| m.rows).sum();
    let mut out = Matrix::filled(rows, cols, 0.0);
    let mut r0 = 0;
    for m in &mats {
        for c in 0..cols {
            for r in 0..m.rows {
                out.set(r0 + r, c, m.get(r, c));
            }
        }
        r0 += m.rows;
    }
    Ok(Value::Mat(out.to_class(class)?))
}

// ---- number formatting -----------------------------------------------

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// C-style `%e` with a signed two-digit exponent.
pub fn fmt_e(v: f64, prec: usize) -> String {
    if !v.is_finite() {
        return nonfinite(v);
    }
    let s = format!("{:.*e}", prec, v);
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{}e{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
}

/// C-style `%g`.
pub fn fmt_g(v: f64, prec: usize) -> String {
    if !v.is_finite() {
        return nonfinite(v);
    }
    if v == 0.0 {
        return "0".to_string();
    }
    let p = prec.max(1);
    let s = format!("{:.*e}", p - 1, v);
    let (m, e) = s.split_once('e').unwrap();
    let exp: i32 = e.parse().unwrap();
    if exp < -4 || exp >= p as i32 {
        format!(
            "{}e{}{:02}",
            trim_zeros(m),
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        )
    } else {
        let dec = (p as i32 - 1 - exp).max(0) as usize;
        trim_zeros(&format!("{:.*}", dec, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Shared(Rc<RefCell<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Runs `src` in a fresh interpreter; returns (result, captured stdout).
    fn run(src: &str) -> (R<()>, String) {
        let buf = Rc::new(RefCell::new(Vec::new()));
        let mut it = Interp::with_output(Box::new(Shared(buf.clone())));
        let r = it.run(src);
        let s = String::from_utf8(buf.borrow().clone()).unwrap();
        (r, s)
    }

    /// Runs `src`, requiring success, and returns what it printed.
    fn ok_out(src: &str) -> String {
        let (r, s) = run(src);
        match r {
            Ok(()) => s,
            Err(e) => panic!("{src:?} failed: {e}"),
        }
    }

    /// Runs `src`, requiring failure, and returns the error message.
    fn err_msg(src: &str) -> String {
        err(src).msg
    }

    /// Runs `src`, requiring failure, and returns the error itself.
    fn err(src: &str) -> crate::error::MError {
        let (r, s) = run(src);
        match r {
            Ok(()) => panic!("{src:?} unexpectedly succeeded, printing {s:?}"),
            Err(e) => e,
        }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-12 * b.abs().max(1.0), "{a} vs {b}");
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

    // ---- display and `ans` -------------------------------------------

    #[test]
    fn assignment_echoes_unless_suppressed() {
        assert_eq!(ok_out("x = 3"), "x =\n\n     3\n\n");
        assert_eq!(ok_out("x = 3;"), "");
        assert_eq!(ok_out("s = 'hi'"), "s =\n\n    'hi'\n\n");
        assert_eq!(ok_out("x = 3;\ny = x + 1"), "y =\n\n     4\n\n");
    }

    #[test]
    fn ans_is_set_by_expressions_but_not_by_assignments() {
        assert_eq!(ok_out("1 + 1;\nans"), "ans =\n\n     2\n\n");
        assert_eq!(ok_out("2 * 3"), "ans =\n\n     6\n\n");
        // An assignment leaves `ans` untouched, so it is still undefined here.
        let e = err_msg("x = 5;\nans");
        assert!(e.contains("Unrecognized function or variable 'ans'"), "{e}");
        // Naming an existing variable echoes under its own name, not `ans`.
        assert_eq!(ok_out("x = 5;\nx"), "x =\n\n     5\n\n");
    }

    #[test]
    fn a_variable_shadows_a_builtin() {
        // `sum(1)` indexes the variable instead of calling the builtin,
        // which would have returned 1 rather than 3.
        assert_eq!(ok_out("sum = 3;\nsum(1)"), "ans =\n\n     3\n\n");
        assert_eq!(ok_out("sum = [4 5 6];\ndisp(sum(2))"), "     5\n");
        // Without the variable the builtin is reachable as usual.
        assert_eq!(ok_out("disp(sum([1 2 3]))"), "     6\n");
    }

    // ---- operators ---------------------------------------------------

    #[test]
    fn elementwise_left_divide_divides_the_other_way_round() {
        assert_eq!(ok_out("disp(2.\\8)"), "     4\n");
        assert_eq!(ok_out("disp(2 .\\ 8)"), "     4\n");
        assert_eq!(ok_out("a = [4 8]; disp(a.\\[8 8])"), "     2     1\n");
        assert_eq!(ok_out("disp([2 4].\\[8 8])"), "     4     2\n");
        // It broadcasts like every other elementwise operator, and is the
        // mirror of `./`.
        assert_eq!(ok_out("disp([1 2].\\4)"), "     4     2\n");
        assert_eq!(ok_out("disp(4./[1 2])"), "     4     2\n");
        // A lone backslash still solves a system rather than dividing.
        assert_eq!(ok_out("disp([2 0; 0 4]\\[2; 4])"), "     1\n     1\n");
    }

    #[test]
    fn a_continuation_inside_brackets_separates_elements() {
        assert_eq!(ok_out("x = [1 ...\n-2]; disp(numel(x))"), "     2\n");
        assert_eq!(ok_out("x = [1 ...\n-2]; disp(x(2))"), "    -2\n");
        assert_eq!(ok_out("x = [1 ...\n -2]; disp(numel(x))"), "     2\n");
        // The other side of the whitespace rule is untouched.
        assert_eq!(ok_out("x = [1 - 2]; disp(numel(x))"), "     1\n");
        // A continuation straight after a digit now lexes.
        assert_eq!(ok_out("a = 1...\n+ 2; disp(a)"), "     3\n");
    }

    // ---- indexed assignment ------------------------------------------

    #[test]
    fn indexed_assignment_grows_a_row() {
        assert_eq!(ok_out("z = [];\nz(3) = 1"), "z =\n\n     0     0     1\n\n");
        assert_eq!(
            ok_out("z = [1 2];\nz(4) = 9"),
            "z =\n\n     1     2     0     9\n\n"
        );
    }

    #[test]
    fn indexed_assignment_grows_a_column() {
        assert_eq!(
            ok_out("z = [1;2];\nz(4) = 9"),
            "z =\n\n     1\n     2\n     0\n     9\n\n"
        );
    }

    #[test]
    fn indexed_assignment_grows_in_two_dimensions() {
        assert_eq!(
            ok_out("A = [1 2; 3 4];\nA(3,3) = 1"),
            "A =\n\n     1     2     0\n     3     4     0\n     0     0     1\n\n"
        );
        assert_eq!(
            ok_out("B = [];\nB(2,2) = 7"),
            "B =\n\n     0     0\n     0     7\n\n"
        );
    }

    #[test]
    fn indexed_assignment_errors() {
        // A 2-D array cannot grow through a single linear index.
        let e = err_msg("A = [1 2; 3 4];\nA(5) = 1");
        assert!(
            e.contains("Attempt to grow array along ambiguous dimension"),
            "{e}"
        );
        // Right-hand side of the wrong size.
        let e = err_msg("A = [1 2 3];\nA(1:2) = [1 2 3]");
        assert!(
            e.contains("left side has 2 elements and the right side has 3"),
            "{e}"
        );
        let e = err_msg("A = [1 2; 3 4];\nA(1,:) = [1 2 3]");
        assert!(
            e.contains("left side has 2 elements and the right side has 3"),
            "{e}"
        );
        // Indices must be positive integers.
        assert!(err_msg("A = [1 2 3];\nA(0) = 1").contains("positive integers"));
    }

    // ---- control flow ------------------------------------------------

    #[test]
    fn for_iterates_over_columns() {
        assert_eq!(
            ok_out("A = [1 2; 3 4];\nfor c = A\ndisp(c)\nend"),
            "     1\n     3\n     2\n     4\n"
        );
        // A row vector therefore yields scalars.
        assert_eq!(ok_out("for k = [7 8]\ndisp(k)\nend"), "     7\n     8\n");
        // A column vector is a single 1-column iteration.
        assert_eq!(ok_out("for k = [7; 8]\ndisp(k)\nend"), "     7\n     8\n");
    }

    #[test]
    fn for_over_an_empty_matrix_runs_zero_times() {
        assert_eq!(
            ok_out("n = 0;\nfor k = []\nn = 1;\nend\ndisp(n)"),
            "     0\n"
        );
        // ... but it still assigns its loop variable, as MATLAB does. The
        // variable used to be left undefined, and a name that already held a
        // value kept it.
        assert_eq!(ok_out("for k = []\nend\ndisp(isempty(k))"), "   1\n");
        assert_eq!(
            ok_out("k = 7;\nfor k = []\nend\ndisp(isempty(k))"),
            "   1\n"
        );
        // The empty assigned is the column the loop would have taken next,
        // which is Octave's shape for both of these. MATLAB's own shape is
        // unsettled, so no golden case asserts it.
        assert_eq!(
            ok_out("for k = []\nend\nfprintf('%d %d\\n', size(k))"),
            "0 0\n"
        );
        assert_eq!(
            ok_out("for k = 1:0\nend\nfprintf('%d %d\\n', size(k))"),
            "1 0\n"
        );
        // A loop that does run still leaves the last column behind.
        assert_eq!(ok_out("for k = [1 2 3]\nend\ndisp(k)"), "     3\n");
    }

    #[test]
    fn while_with_break_and_continue() {
        let src = "i = 0;\ns = 0;\n\
                   while 1\n\
                   i = i + 1;\n\
                   if i > 5\n\
                   break\n\
                   end\n\
                   if mod(i, 2) == 0\n\
                   continue\n\
                   end\n\
                   s = s + i;\n\
                   end\n\
                   disp(s)\n\
                   disp(i)";
        // 1 + 3 + 5 = 9, and the loop leaves i at 6.
        assert_eq!(ok_out(src), "     9\n     6\n");
        // A while whose condition is false from the start runs zero times.
        assert_eq!(ok_out("n = 0;\nwhile 0\nn = 1;\nend\ndisp(n)"), "     0\n");
    }

    #[test]
    fn logical_operators_short_circuit() {
        // The right-hand side must not be evaluated at all.
        assert_eq!(ok_out("x = 0 && undefined_fn();\ndisp(x)"), "   0\n");
        assert_eq!(ok_out("y = 1 || undefined_fn();\ndisp(y)"), "   1\n");
        // ... and it really would have failed.
        assert!(err_msg("disp(undefined_fn())").contains("Unrecognized function or variable"));
        assert_eq!(ok_out("disp(1 && 1)"), "   1\n");
        assert_eq!(ok_out("disp(0 || 0)"), "   0\n");
        assert_eq!(ok_out("disp(1 && 0)"), "   0\n");
    }

    // ---- error line numbers ------------------------------------------

    #[test]
    fn a_runtime_error_reports_the_line_it_was_raised_on() {
        assert_eq!(err("y + 1").line, Some(1));
        assert_eq!(err("x = 1;\n[1 2] * [3 4]").line, Some(2));
        assert_eq!(err("x = 1;\ny = 2;\n\nz = undefined_name").line, Some(4));
        // Output before the error is still produced.
        let (r, out) = run("disp(1)\ndisp(2)\ny + 1");
        assert_eq!(out, "     1\n     2\n");
        let e = r.unwrap_err();
        assert_eq!(e.line, Some(3));
        assert_eq!(e.msg, "Unrecognized function or variable 'y'.");
        assert_eq!(
            e.to_string(),
            "Line 3: Unrecognized function or variable 'y'."
        );
    }

    #[test]
    fn an_error_in_a_block_body_reports_the_body_line() {
        // Not line 2, where the `for` is, and not line 1.
        assert_eq!(
            err("disp(1)\nfor k = 1:2\n[1 2] * [3 4];\nend").line,
            Some(3)
        );
        assert_eq!(err("if 1\n\nundefined_name;\nend").line, Some(3));
        assert_eq!(err("while 1\nundefined_name;\nend").line, Some(2));
        // An error in the loop's own range expression belongs to the `for`.
        assert_eq!(err("disp(1)\nfor k = undefined_name\nend").line, Some(2));
    }

    #[test]
    fn a_parse_or_lex_error_reports_its_line_too() {
        assert_eq!(err("x = 5;\ny = x + ;").line, Some(2));
        assert_eq!(err("x = 5;\ny = 'abc").line, Some(2));
    }

    // ---- pure helpers ------------------------------------------------

    #[test]
    fn fmt_e_uses_a_signed_two_digit_exponent() {
        assert_eq!(fmt_e(12345.678, 4), "1.2346e+04");
        assert_eq!(fmt_e(0.0, 4), "0.0000e+00");
        assert_eq!(fmt_e(-0.5, 2), "-5.00e-01");
        assert_eq!(fmt_e(1.0, 0), "1e+00");
        assert_eq!(fmt_e(f64::NAN, 4), "NaN");
        assert_eq!(fmt_e(f64::INFINITY, 4), "Inf");
        assert_eq!(fmt_e(f64::NEG_INFINITY, 4), "-Inf");
    }

    #[test]
    fn fmt_g_switches_between_fixed_and_scientific() {
        assert_eq!(fmt_g(0.0001, 6), "0.0001");
        assert_eq!(fmt_g(0.00001, 6), "1e-05");
        assert_eq!(fmt_g(1e6, 6), "1e+06");
        assert_eq!(fmt_g(123456.0, 6), "123456");
        // Trailing zeros are trimmed in both branches.
        assert_eq!(fmt_g(1.5, 6), "1.5");
        assert_eq!(fmt_g(100.0, 6), "100");
        assert_eq!(fmt_g(1.25e-7, 6), "1.25e-07");
        assert_eq!(fmt_g(0.0, 6), "0");
        assert_eq!(fmt_g(-2.5, 6), "-2.5");
        assert_eq!(fmt_g(f64::NEG_INFINITY, 6), "-Inf");
    }

    // ---- builtins through the registry -------------------------------
    //
    // The builtins themselves are tested in `src/builtins/`; what these
    // check is the wiring: name resolution, nargout, and the empty return.

    #[test]
    fn a_builtin_that_returns_nothing_is_legal_only_as_a_statement() {
        // As a statement it prints and the empty return is ignored.
        assert_eq!(ok_out("disp(1)"), "     1\n");
        // In an expression the same call is an error, after the output.
        let (r, out) = run("x = disp(3)");
        assert_eq!(out, "     3\n");
        assert_eq!(r.unwrap_err().msg, "Too many output arguments.");
        assert_eq!(err_msg("disp(1) + 1"), "Too many output arguments.");
        assert_eq!(err_msg("x = clc"), "Too many output arguments.");
    }

    #[test]
    fn extra_arguments_are_rejected() {
        assert_eq!(err_msg("sum(1, 2, 3)"), "Too many input arguments.");
        assert_eq!(err_msg("abs(1, 2)"), "Too many input arguments.");
        assert_eq!(err_msg("disp('a', 'b')"), "Too many input arguments.");
    }

    #[test]
    fn an_unknown_name_is_undefined_with_or_without_arguments() {
        assert!(err_msg("nope").contains("Unrecognized function or variable 'nope'"));
        assert!(err_msg("nope(1)").contains("Unrecognized function or variable 'nope'"));
    }

    #[test]
    fn clear_removes_only_the_named_variable() {
        assert_eq!(ok_out("a = 1; b = 2; clear('a'); disp(b)"), "     2\n");
        let e = err_msg("a = 1; b = 2; clear('a'); a");
        assert!(e.contains("Unrecognized function or variable 'a'"), "{e}");
        // With no argument it still clears the whole workspace.
        let e = err_msg("a = 1; b = 2; clear; b");
        assert!(e.contains("Unrecognized function or variable 'b'"), "{e}");
    }

    #[test]
    fn tic_and_toc_see_nargout() {
        assert_eq!(ok_out("t = tic; disp(toc(t) >= 0)"), "   1\n");
        assert_eq!(ok_out("tic; disp(toc >= 0)"), "   1\n");
        // As a statement, tic prints nothing and toc reports the time.
        assert_eq!(ok_out("tic;"), "");
        let out = ok_out("tic\ntoc");
        assert!(out.starts_with("Elapsed time is "), "{out}");
        assert!(out.ends_with(" seconds.\n"), "{out}");
    }

    #[test]
    fn fprintf_cycles_the_format_over_the_data() {
        assert_eq!(ok_out("fprintf('%d %d\\n', 1:4)"), "1 2\n3 4\n");
        assert_eq!(ok_out("disp(sprintf('%d-%s', 4, 'x'))"), "4-x\n");
    }

    #[test]
    fn range_endpoints_and_emptiness() {
        let r = range(1.0, 0.1, 2.0).unwrap();
        assert_eq!((r.rows, r.cols), (1, 11));
        assert_eq!(r.numel(), 11);
        assert_eq!(r.data[0], 1.0);
        assert_eq!(r.data[10], 2.0);
        close(r.data[5], 1.5);

        let e = range(3.0, 1.0, 1.0).unwrap();
        assert!(e.is_empty());
        assert_eq!((e.rows, e.cols), (1, 0));

        assert_eq!(
            range(1.0, 1.0, 5.0).unwrap().data,
            [1.0, 2.0, 3.0, 4.0, 5.0]
        );
        assert_eq!(range(3.0, -1.0, 1.0).unwrap().data, [3.0, 2.0, 1.0]);
        assert!(range(1.0, 0.0, 5.0).unwrap().is_empty());
        assert!(range(f64::NAN, 1.0, 5.0).unwrap().is_empty());
    }

    /// The colon lands exactly on its end point, and is symmetric about its
    /// middle, because the upper half is computed from the right-hand end
    /// point rather than by adding the step over and over.
    #[test]
    fn the_colon_hits_its_end_point_exactly() {
        let x = range(0.0, 0.1, 0.3).unwrap();
        assert_eq!(x.numel(), 4);
        assert_eq!(x.data[0], 0.0);
        assert_eq!(*x.data.last().unwrap(), 0.3);

        let y = range(-1.0, 0.01, 1.0).unwrap();
        assert_eq!(y.numel(), 201);
        assert_eq!(*y.data.last().unwrap(), 1.0);
        for k in 0..y.numel() {
            assert_eq!(y.data[k] + y.data[y.numel() - 1 - k], 0.0, "element {k}");
        }

        // An end point the range does not land on is not jumped to: the last
        // element of 0:0.1:0.35 is 0.3-ish, never 0.35.
        let short = range(0.0, 0.1, 0.35).unwrap();
        assert_eq!(short.numel(), 4);
        assert!(*short.data.last().unwrap() < 0.35);
        close(*short.data.last().unwrap(), 0.3);

        // Integer ranges and descending ranges keep their exact values.
        assert_eq!(
            range(1.0, 1.0, 5.0).unwrap().data,
            [1.0, 2.0, 3.0, 4.0, 5.0]
        );
        assert_eq!(
            range(10.0, -2.0, 2.0).unwrap().data,
            [10.0, 8.0, 6.0, 4.0, 2.0]
        );
        // A single-element range, where there is no upper half at all.
        assert_eq!(range(5.0, 1.0, 5.0).unwrap().data, [5.0]);
        assert_eq!(range(0.0, 0.1, 0.05).unwrap().data, [0.0]);
    }

    /// An infinite end point is refused rather than quietly giving a `1x0`;
    /// a `NaN` one still gives the empty, and stays in Known bugs.
    #[test]
    fn a_non_finite_range_end_point_is_refused_but_nan_is_not() {
        let msg = "Requested 1xInf array exceeds the maximum array size.";
        assert_eq!(range(0.0, 1.0, f64::INFINITY).unwrap_err().msg, msg);
        assert_eq!(range(f64::NEG_INFINITY, 1.0, 0.0).unwrap_err().msg, msg);
        assert_eq!(range(0.0, -1.0, f64::NEG_INFINITY).unwrap_err().msg, msg);
        assert_eq!(err_msg("x = 0:Inf;"), msg);
        assert_eq!(err_msg("x = -Inf:1:0;"), msg);
        // A range that runs the wrong way is empty whatever its end points,
        // which is the answer it always gave.
        assert!(range(f64::INFINITY, 1.0, 0.0).unwrap().is_empty());
        // NaN anywhere is the empty, unchanged and still recorded.
        assert!(range(1.0, 1.0, f64::NAN).unwrap().is_empty());
        assert!(range(f64::NAN, 1.0, 1.0).unwrap().is_empty());
        assert!(range(1.0, f64::NAN, 5.0).unwrap().is_empty());
        // Inf:Inf has no count either: the difference is NaN.
        assert!(range(f64::INFINITY, 1.0, f64::INFINITY).unwrap().is_empty());
    }

    /// An infinite *step* follows the documented count `fix((k - j) / i)`,
    /// which is `0` for `1:Inf:5` and so one element, the start. It used to
    /// return the empty on the way in, which made `size(1:Inf:5)` `1 0`.
    #[test]
    fn an_infinite_range_step_gives_the_start_alone() {
        let r = range(1.0, f64::INFINITY, 5.0).unwrap();
        assert_eq!((r.rows, r.cols), (1, 1));
        // Not `NaN`: the first element is the start itself, never `a + 0 * s`.
        assert_eq!(r.data, [1.0]);
        assert_eq!(ok_out("disp(size(1:Inf:5))"), "     1     1\n");
        assert_eq!(ok_out("disp(1:Inf:5)"), "     1\n");
        // Descending with a negative infinite step is the mirror of it.
        assert_eq!(range(5.0, f64::NEG_INFINITY, 1.0).unwrap().data, [5.0]);
        // A range that runs against its step is still empty, which an
        // infinite step must not change: the division gives a *signed zero*
        // there, which is not less than zero.
        assert!(range(5.0, f64::INFINITY, 1.0).unwrap().is_empty());
        assert!(range(1.0, f64::NEG_INFINITY, 5.0).unwrap().is_empty());
        // The finite direction rule is untouched.
        assert!(range(5.0, 1.0, 1.0).unwrap().is_empty());
        assert!(range(1.0, -1.0, 5.0).unwrap().is_empty());
        // Equal end points are one element whatever the step.
        assert_eq!(range(5.0, f64::INFINITY, 5.0).unwrap().data, [5.0]);
    }

    /// Acceptance test 17, the `index_read` half: a two-subscript read sizes
    /// its result from the subscripts, so the guard belongs there and not on
    /// the array being read.
    #[test]
    fn a_two_subscript_read_checks_its_result_size() {
        let a = rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        // 20000 squared is past the 2^28-element cap; every subscript is 1,
        // so the 2x2 source is never the problem.
        let big = || Sel::List(vec![0; 20_000], 1, 20_000);
        let e = index_read(&a, &[big(), big()]).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 20000x20000 array exceeds the maximum array size."
        );
        // An out-of-range subscript is still reported as one: the bounds
        // tests come before the size guard.
        let out = Sel::List(vec![5; 20_000], 1, 20_000);
        let e = index_read(&a, &[out, big()]).unwrap_err().msg;
        assert!(e.contains("exceeds array bounds"), "{e}");
        // A result that fits is unaffected.
        let ok = index_read(&a, &[Sel::List(vec![0], 1, 1), Sel::All]).unwrap();
        assert_eq!(ok.data, [1.0, 2.0]);
    }

    /// Acceptance test 14, through both spellings of the power operator.
    #[test]
    fn a_would_be_complex_result_is_an_error_not_a_nan() {
        for src in [
            "sqrt(-4)",
            "log(-1)",
            "log2(-8)",
            "log10(-10)",
            "asin(2)",
            "acos(-2)",
            "(-8)^(1/3)",
            "(-8).^(1/3)",
            "power(-2, 0.5)",
            "x = [1 -4]; sqrt(x)",
            "[-8 1].^(1/3)",
        ] {
            let e = err_msg(src);
            assert!(
                e.starts_with("Complex results are not supported."),
                "{src}: {e}"
            );
        }
        // The real neighbours of each of those still compute.
        assert_eq!(ok_out("disp(sqrt(4))"), "     2\n");
        assert_eq!(ok_out("disp(log(1))"), "     0\n");
        assert_eq!(ok_out("disp(asin(0))"), "     0\n");
        assert_eq!(ok_out("disp((-8)^2)"), "    64\n");
        assert_eq!(ok_out("disp((-8)^(1/1))"), "    -8\n");
        assert_eq!(ok_out("fprintf('%g\\n', (-2)^Inf)"), "Inf\n");
        // A NaN argument is in the real domain of all of them.
        assert_eq!(ok_out("fprintf('%g\\n', sqrt(NaN))"), "NaN\n");
        assert_eq!(ok_out("fprintf('%g\\n', asin(NaN))"), "NaN\n");
        assert_eq!(ok_out("fprintf('%g\\n', power(NaN, 0.5))"), "NaN\n");
        // -0 is not negative, so it keeps IEEE's real answers.
        assert_eq!(ok_out("fprintf('%g\\n', sqrt(-0))"), "0\n");
    }

    #[test]
    fn a_long_range_is_a_clean_error_not_an_allocator_abort() {
        // `1:1e15` used to ask the allocator for 8 PB and abort the process.
        let e = err_msg("x = 1:1e15");
        assert!(e.contains("1x1000000000000000"), "{e}");
        assert!(e.contains("exceeds the maximum array size"), "{e}");
        // The limit is `check_shape`'s, not a second policy of its own: one
        // element past the cap is refused. (The range at the cap is 2 GiB, so
        // it is left to `check_shape`'s own test rather than built here.)
        let cap = crate::builtins::args::MAX_ELEMS as f64;
        assert!(range(1.0, 1.0, cap + 1.0).is_err());
        // A count that overflows `f64` itself is refused under its own
        // name, not as the `usize::MAX` it used to saturate to.
        let e = err_msg("x = 0:1e-300:1e300;");
        assert_eq!(e, "Requested 1xInf array exceeds the maximum array size.");
        let e = range(0.0, 1.0, 1e300).unwrap_err().msg;
        assert!(e.contains("1x1e+300"), "{e}");
    }

    #[test]
    fn a_constructor_names_a_size_past_usize_as_asked() {
        assert_eq!(
            err_msg("zeros(1e300)"),
            "Requested 1e+300x1e+300 array exceeds the maximum array size."
        );
        // Indexed growth is cycle 03's, and still names the clamp.
        assert!(err_msg("x = []; x(1e300) = 1;").contains("18446744073709551615"));
    }

    #[test]
    fn e_is_an_ordinary_name() {
        assert_eq!(err_msg("disp(e)"), "Unrecognized function or variable 'e'.");
        assert_eq!(ok_out("e = 5; disp(e)"), "     5\n");
        assert_eq!(ok_out("fprintf('%.4f\\n', exp(1))"), "2.7183\n");
    }

    #[test]
    fn toc_needs_an_earlier_bare_tic() {
        let msg = "You must call TIC without an output argument before calling TOC \
                   without an input argument.";
        assert_eq!(err_msg("toc"), msg);
        assert_eq!(err_msg("x = toc;"), msg);
        // A handle from `t = tic` is not a bare tic.
        assert_eq!(err_msg("t = tic; x = toc;"), msg);
        assert_eq!(ok_out("t = tic; disp(toc(t) >= 0)"), "   1\n");
        assert_eq!(ok_out("tic; x = toc; disp(x >= 0)"), "   1\n");
    }

    #[test]
    fn size_vectors_reach_the_constructors_through_a_call() {
        assert_eq!(
            ok_out("A = ones(2, 3); disp(size(zeros(size(A))))"),
            "     2     3\n"
        );
        assert_eq!(ok_out("disp(size(reshape(1:6, [], 2)))"), "     3     2\n");
        assert_eq!(ok_out("disp(sum([1 2; 3 4], 'all'))"), "    10\n");
    }

    // ---- the nesting limit -------------------------------------------

    /// The message both the parser and the evaluator raise at the limit.
    const TOO_DEEP: &str = "Nesting is too deep. The maximum nesting depth is 10000.";

    /// Runs `f` on a thread with the stack `src/main.rs` gives the
    /// interpreter. The limit only means anything against that stack, and a
    /// test thread's default is a small fraction of it; running these on the
    /// default stack would prove the opposite of what they claim.
    fn on_the_interpreter_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn(f)
            .expect("the test thread should spawn")
            .join()
            .expect("the limit must fire before the stack does");
    }

    /// `-(-(...-1))`, built rather than parsed, so the evaluator's limit can
    /// be reached without the parser's firing first.
    fn nested_neg(depth: usize) -> Expr {
        let mut e = Expr::Num(1.0);
        for _ in 0..depth {
            e = Expr::Neg(Box::new(e));
        }
        e
    }

    /// Acceptance test 16, the parser half: the limit fires at the boundary
    /// and one past it, for every shape of nesting QA D4 found. About 96,000
    /// nested parentheses used to exhaust even the 256 MB stack and abort
    /// with exit 134, which no `Result` can catch.
    #[test]
    fn the_parser_refuses_nesting_past_the_limit() {
        on_the_interpreter_stack(|| {
            // Two levels are spent before the first bracket: the statement,
            // and the expression the statement holds.
            let deepest = MAX_DEPTH - 2;
            for (open, close) in [("(", ")"), ("[", "]"), ("abs(", ")"), ("x(", ")")] {
                let src = |n: usize| format!("x = 1;\n{}1{}", open.repeat(n), close.repeat(n));
                assert!(
                    run(&src(deepest)).0.is_ok(),
                    "{open} at the limit should parse"
                );
                assert_eq!(err_msg(&src(deepest + 1)), TOO_DEEP, "{open}");
            }
            // A flat sum nests by association rather than by recursion: the
            // parser never recurses to build `1+1+...+1`, but the tree it
            // builds is one level deeper per term, and the evaluator does.
            let sum = |n: usize| format!("x = {};", vec!["1"; n].join("+"));
            assert!(run(&sum(MAX_DEPTH - 1)).0.is_ok());
            assert_eq!(err_msg(&sum(MAX_DEPTH)), TOO_DEEP);
            // Nested blocks are counted the same way.
            let blocks = |n: usize| format!("{}\n{}", "if 1\n".repeat(n), "end\n".repeat(n));
            assert_eq!(err_msg(&blocks(MAX_DEPTH + 1)), TOO_DEEP);
            // So are the two chains that fold rather than recurse through the
            // precedence ladder: a transpose chain and a sign chain.
            let quotes = |n: usize| format!("x = 1;\ny = x{};", "'".repeat(n));
            assert!(run(&quotes(MAX_DEPTH - 2)).0.is_ok());
            assert_eq!(err_msg(&quotes(MAX_DEPTH - 1)), TOO_DEEP);
            let signs = |n: usize| format!("y = {}1;", "-".repeat(n));
            assert!(run(&signs(MAX_DEPTH - 2)).0.is_ok());
            assert_eq!(err_msg(&signs(MAX_DEPTH - 1)), TOO_DEEP);
        });
    }

    /// Acceptance test 16, the evaluator half. The tree is built directly, so
    /// the parser's limit cannot fire first and mask it.
    #[test]
    fn the_evaluator_refuses_nesting_past_the_limit() {
        on_the_interpreter_stack(|| {
            let mut it = Interp::with_output(Box::new(io::sink()));
            // The outermost node is level 1, so the deepest legal tree has
            // one fewer node than the limit above its leaf.
            assert!(it.eval(&nested_neg(MAX_DEPTH - 1)).is_ok());
            let e = it.eval(&nested_neg(MAX_DEPTH)).unwrap_err();
            assert_eq!(e.msg, TOO_DEEP);
            // The counter is not left raised: the REPL hands the same
            // interpreter the next line, and it must still work.
            assert_eq!(ok_out("disp(1)"), "     1\n");
            let (r, _) = run("x = 1; disp(x + 1)");
            assert!(r.is_ok());
        });
    }

    // ---- break and continue outside a loop ---------------------------

    /// QA D8. `break` used to unwind out of the whole script, so the
    /// statements after it never ran and the process still exited 0.
    #[test]
    fn break_or_continue_outside_a_loop_is_an_error() {
        for (word, stmt) in [("break", "break"), ("continue", "continue")] {
            let (r, out) = run(&format!("disp(1)\n{stmt}\ndisp(2)"));
            // Everything printed before it is still printed, which is why
            // this is raised when the statement runs and not when it parses.
            assert_eq!(out, "     1\n");
            let e = r.unwrap_err();
            assert_eq!(e.msg, format!("'{word}' is only valid inside a loop."));
            assert_eq!(e.line, Some(2));
            // Inside an `if` that is itself outside a loop, likewise.
            assert!(err_msg(&format!("if 1\n{stmt}\nend")).contains(word));
        }
        // Inside a loop both still work, including from a nested block.
        assert_eq!(
            ok_out("for k = 1:3\nif k == 2\nbreak\nend\ndisp(k)\nend"),
            "     1\n"
        );
        assert_eq!(
            ok_out("for k = 1:3\nif k == 2\ncontinue\nend\ndisp(k)\nend"),
            "     1\n     3\n"
        );
        // ... and the loop that ran does not make a later top-level `break`
        // legal, however it ended.
        assert!(err_msg("for k = 1:2\nbreak\nend\nbreak").contains("only valid inside"));
        assert!(err_msg("for k = 1:2\nundefined_name;\nend").contains("Unrecognized"));
        assert!(err_msg("while 1\nbreak\nend\ncontinue").contains("only valid inside"));
    }

    // ---- the line an `elseif` reports --------------------------------

    /// QA D27. The whole `if` statement is located at its own line, so an
    /// error in a later arm's condition used to be tagged with line 1.
    #[test]
    fn an_error_in_an_elseif_condition_names_the_elseif_line() {
        assert_eq!(err("if 0\nelseif undefined_name\nend").line, Some(2));
        assert_eq!(
            err("disp(1)\nif 0\nelseif 0\nelseif undefined_name\nend").line,
            Some(4)
        );
        // The `if`'s own condition still names the `if`.
        assert_eq!(err("if undefined_name\nelseif 1\nend").line, Some(1));
        // An error in a body still names the body's line, not the arm's.
        assert_eq!(err("if 0\nelseif 1\nundefined_name;\nend").line, Some(3));
    }

    // ---- logical conversion ------------------------------------------

    /// Acceptance test 14 (QA D5): `NaN` is neither true nor false.
    #[test]
    fn a_nan_is_refused_wherever_a_logical_is_wanted() {
        let want = "NaN's cannot be converted to logicals.";
        for src in [
            "if NaN, end",
            "if NaN, disp('true'), end",
            "while NaN, end",
            "disp(NaN & 1)",
            "disp(1 | NaN)",
            "disp(~NaN)",
            "disp(NaN && 1)",
            "disp(NaN || 1)",
            "disp(1 && NaN)",
            "disp(0 || NaN)",
            "if [1 NaN], end",
        ] {
            assert_eq!(err_msg(src), want, "{src}");
        }
        // A short-circuit that never reaches the `NaN` never converts it,
        // which is how MATLAB behaves too: the operand is not evaluated.
        assert_eq!(ok_out("disp(1 || NaN)"), "   1\n");
        assert_eq!(ok_out("disp(0 && NaN)"), "   0\n");
        // Everything else converts as it always did.
        assert_eq!(ok_out("if Inf, disp(1), end"), "     1\n");
        assert_eq!(ok_out("disp(~0)"), "   1\n");
        assert_eq!(ok_out("disp([1 0] & [1 1])"), "   1   0\n");
        assert_eq!(ok_out("disp(isnan(NaN))"), "   1\n");
    }

    /// Acceptance test 13: `&&` and `||` need one value to branch on.
    #[test]
    fn the_short_circuit_operators_reject_arrays_and_empties() {
        let want = "Operands to the logical AND (&&) and OR (||) operators \
                    must be convertible to logical scalar values.";
        for src in [
            "disp([1 1] && 1)",
            "disp([] || 1)",
            "disp(1 && [1 1])",
            "disp(0 || [])",
            "disp('ab' && 1)",
        ] {
            assert_eq!(err_msg(src), want, "{src}");
        }
        // Short-circuiting still stops before the second operand, so a
        // right-hand side that would be refused is never reached.
        assert_eq!(ok_out("disp(0 && [1 1])"), "   0\n");
        assert_eq!(ok_out("disp(1 || [])"), "   1\n");
        // `if` is unaffected: it takes an array and an empty.
        assert_eq!(ok_out("if [1 1], disp(1), end"), "     1\n");
        assert_eq!(ok_out("n = 0;\nif [], n = 1; end\ndisp(n)"), "     0\n");
    }

    // ---- chained ranges ----------------------------------------------

    /// Acceptance test 2: MATLAB reads `1:2:3:4` as `(1:2:3):4`, and so does
    /// SplatCrab now that `parse_range` loops; it used to be a parse error.
    ///
    /// Both spellings then reach the same place: `1:2:3` is the `1x2`
    /// `[1 3]`, and a colon whose start is not a scalar is SplatCrab's own
    /// deliberate error. MATLAB is understood to take the first element
    /// instead, which is a separate row in Known bugs and not this bullet's
    /// business; what this bullet owns is that the two spellings agree.
    #[test]
    fn a_chained_range_reads_left_to_right() {
        assert_eq!(err_msg("disp(1:2:3:4)"), err_msg("disp((1:2:3):4)"));
        assert_eq!(err_msg("disp(1:2:3:4)"), "range start must be a scalar.");
        assert_eq!(err_msg("disp(1:2:3:4:5)"), err_msg("disp(((1:2:3):4):5)"));
        // A chain whose left-hand range is a single element evaluates, and
        // gives what the parenthesised spelling gives.
        assert_eq!(ok_out("disp(1:2:1:4)"), ok_out("disp((1:2:1):4)"));
        assert_eq!(ok_out("disp(1:2:1:4)"), "     1     2     3     4\n");
        // The two- and three-operand forms are untouched.
        assert_eq!(ok_out("disp(1:3)"), "     1     2     3\n");
        assert_eq!(ok_out("disp(1:2:5)"), "     1     3     5\n");
        // A bare colon in an index still is one.
        assert_eq!(ok_out("A = [1 2; 3 4]; disp(A(:, 1)')"), "     1     3\n");
        assert_eq!(ok_out("x = 1:4; disp(x(2:3))"), "     2     3\n");
    }

    // ---- empty-result shapes -----------------------------------------

    /// Acceptance test 12. Each of these was a `0x1` or a `1x0` where MATLAB
    /// gives a `0x0`, and `disp([])` printed `[]` where MATLAB prints nothing.
    #[test]
    fn empty_results_have_matlabs_shapes() {
        assert_eq!(ok_out("disp(size(find([])))"), "     0     0\n");
        assert_eq!(ok_out("disp(size(diag([])))"), "     0     0\n");
        assert_eq!(ok_out("disp(size(''))"), "     0     0\n");
        assert_eq!(ok_out("disp(size(num2str([])))"), "     0     0\n");
        // `disp([])` prints nothing at all; `x = []` still shows its `[]`,
        // which is a separate deviation scheduled to cycle 02.
        assert_eq!(ok_out("disp([])"), "");
        assert_eq!(ok_out("x = []"), "x =\n\n     []\n\n");
        // `disp('')` is still a line with nothing on it.
        assert_eq!(ok_out("disp('')"), "\n");
        // A shape that has an orientation to keep still keeps it.
        assert_eq!(ok_out("disp(size(find([0 0])))"), "     1     0\n");
        assert_eq!(ok_out("disp(size(find([0; 0])))"), "     0     1\n");
        assert_eq!(ok_out("disp(size(find([1 0 1])))"), "     1     2\n");
    }

    /// `s(:)` is a column in MATLAB. It used to come back as a row, because a
    /// `Value::Str` was a row of characters with nowhere to put any other
    /// shape, and then as a column of codes; since cycle 02 it is a char
    /// column (QA D17).
    #[test]
    fn a_colon_index_of_a_char_is_a_column() {
        assert_eq!(ok_out("s = 'abc'; disp(size(s(:)))"), "     3     1\n");
        assert_eq!(ok_out("s = 'abc'; disp(s(:))"), "a\nb\nc\n");
        assert_eq!(ok_out("s = 'abc'; disp(class(s(:)))"), "char\n");
        // Every other index of a char is still a char.
        assert_eq!(ok_out("s = 'abc'; disp(s(2))"), "b\n");
        assert_eq!(ok_out("s = 'abc'; disp(s([3 1]))"), "ca\n");
        assert_eq!(ok_out("s = 'abc'; disp(s(2:3))"), "bc\n");
        assert_eq!(ok_out("s = 'abc'; disp(size(s([1;2])))"), "     1     2\n");
    }

    // ---- the byte-order mark -----------------------------------------

    /// QA D29. Three bytes before the first statement are an encoding marker,
    /// not source; they used to be `unexpected character '\u{feff}'`.
    #[test]
    fn a_leading_byte_order_mark_is_skipped() {
        assert_eq!(ok_out("\u{feff}disp(1)"), "     1\n");
        // Only a leading one. A mark in the middle is a real stray character.
        let e = err_msg("disp(1)\n\u{feff}disp(2)");
        assert!(e.contains("unexpected character"), "{e}");
    }

    // ---- classes -----------------------------------------------------

    /// The class of the value `src` leaves in `ans`.
    fn class_of(src: &str) -> Class {
        let buf = Rc::new(RefCell::new(Vec::new()));
        let mut it = Interp::with_output(Box::new(Shared(buf)));
        it.run(&format!("{src};")).unwrap();
        it.vars["ans"].mat().class
    }

    /// The propagation table: arithmetic is double, comparisons and logical
    /// operators are logical, whatever the operands' classes were.
    #[test]
    fn operators_class_their_results_by_the_propagation_table() {
        use Class::*;
        for (src, want) in [
            ("1 + 2", Double),
            ("true + true", Double),
            ("'a' + 1", Double),
            ("-true", Double),
            ("+'a'", Double),
            ("'ab' * 2", Double),
            ("true * [1 2]", Double),
            ("2 ^ true", Double),
            ("1 < 2", Logical),
            ("'a' == 'a'", Logical),
            ("[1 2] ~= 2", Logical),
            ("~1", Logical),
            ("~'a'", Logical),
            ("[1 0] & 1", Logical),
            ("[1 0] | 0", Logical),
            ("1 && 1", Logical),
            ("0 || 0", Logical),
            ("true'", Logical),
            ("('ab')'", Char),
            ("1:3", Double),
        ] {
            assert_eq!(class_of(src), want, "{src}");
        }
        assert_eq!(ok_out("disp(+'a')"), "    97\n");
    }

    /// Concatenation: char if any operand is, else logical if all are, else
    /// double; a 0x0 double takes no part in the vote.
    #[test]
    fn concatenation_classes_its_result() {
        use Class::*;
        for (src, want) in [
            ("['a' 66]", Char),
            ("[65 'a']", Char),
            ("['a' true]", Char),
            ("[true false]", Logical),
            ("[true; false]", Logical),
            ("[true 2]", Double),
            ("[[] 'abc']", Char),
            ("['abc' []]", Char),
            ("[[] true]", Logical),
            ("[[] []]", Double),
            ("['' '']", Char),
            ("['ab'; 'cd']", Char),
            ("[1 2]", Double),
        ] {
            assert_eq!(class_of(src), want, "{src}");
        }
        assert_eq!(ok_out("disp(['a' 66])"), "aB\n");
        assert_eq!(ok_out("s = []; s = [s 'abc']; disp(s)"), "abc\n");
        assert_eq!(ok_out("disp(['ab'; 'cd'])"), "ab\ncd\n");
        let e = err_msg("x = ['ab'; 'c'];");
        assert!(e.contains("Dimensions"), "{e}");
    }

    /// Indexing keeps the class; indexed assignment keeps the left-hand
    /// side's, converting what is stored into it.
    #[test]
    fn indexed_assignment_keeps_the_left_hand_class() {
        assert_eq!(ok_out("s = 'abc'; s(1) = 'X'; disp(s)"), "Xbc\n");
        assert_eq!(
            ok_out("s = 'abc'; s(2) = 'Z'; disp(s + 0)"),
            "    97    90    99\n"
        );
        // Growth pads a char with the code unit 0 and stays a char.
        assert_eq!(
            ok_out("s = 'abc'; s(5) = 'e'; disp(double(s))"),
            "    97    98    99     0   101\n"
        );
        assert_eq!(ok_out("s = 'abc'; s(5) = 'e'; disp(class(s))"), "char\n");
        // A double target stores the numeric value of a char.
        assert_eq!(
            ok_out("y = [1 2 3]; y(2) = 'a'; disp(y)"),
            "     1    97     3\n"
        );
        // A logical target stores logical(value), refusing NaN as ever.
        assert_eq!(ok_out("x = true(1,3); x(2) = 5; disp(x)"), "   1   1   1\n");
        assert_eq!(
            ok_out("x = false(1,2); x(2) = 5; disp(class(x))"),
            "logical\n"
        );
        assert_eq!(
            err_msg("x = true(1,2); x(1) = NaN;"),
            "NaN's cannot be converted to logicals."
        );
        // A new variable, or the 0x0 double `[]`, takes the class assigned.
        assert_eq!(ok_out("n(3) = 'c'; disp(class(n))"), "char\n");
        assert_eq!(ok_out("s = []; s(1) = 'a'; s(2) = 'b'; disp(s)"), "ab\n");
        assert_eq!(ok_out("t = []; t(2) = true; disp(class(t))"), "logical\n");
        // Two subscripts, and growth in two dimensions, keep it too.
        assert_eq!(
            ok_out("c = ['ab'; 'cd']; c(2, 1) = 'X'; disp(c)"),
            "ab\nXd\n"
        );
        assert_eq!(ok_out("c = 'ab'; c(2, 2) = 'Y'; disp(class(c))"), "char\n");
        // Reading keeps the class of what is read.
        assert_eq!(
            ok_out("x = [true false true]; disp(class(x(2)))"),
            "logical\n"
        );
        assert_eq!(ok_out("s = 'abc'; disp(class(s([3 1])))"), "char\n");
    }

    /// `for` over a char iterates chars, and `if` takes a char's truth.
    #[test]
    fn a_char_drives_for_and_if() {
        assert_eq!(
            ok_out("for k = 'abc', fprintf('%s:%s ', class(k), k); end"),
            "char:a char:b char:c "
        );
        assert_eq!(ok_out("for c = ['ab'; 'cd'], disp(c'), end"), "ac\nbd\n");
        assert_eq!(
            ok_out("if 'abc', disp(1), end; if [], disp(2), end; if [1 0], disp(3), end"),
            "     1\n"
        );
        assert_eq!(
            ok_out("for k = true(1, 2), disp(class(k)), end"),
            "logical\nlogical\n"
        );
    }

    /// QA D6: an index of class logical is refused, reading and assigning,
    /// rather than read as positions. Cycle 03 implements it.
    #[test]
    fn a_logical_index_is_refused_until_cycle_03() {
        let want = "Logical indexing is not supported yet.";
        for src in [
            "x = [5 6 7]; x(x > 0)",
            "x = [5 6 7]; y = x(x > 5);",
            "x = [5 6 7]; x(x > 0) = 0;",
            "x = [5 6 7]; x(true)",
            "A = [1 2; 3 4]; A(1, [true false])",
            "A = [1 2; 3 4]; A(true, 1) = 9;",
            "x = [5 6 7]; x(isnan(x))",
        ] {
            assert_eq!(err_msg(src), want, "{src}");
        }
        // A double of ones and zeros is still a list of positions, and a
        // char index is its codes, as in MATLAB.
        assert_eq!(ok_out("x = [5 6 7]; disp(x([1 1]))"), "     5     5\n");
        assert_eq!(
            ok_out("x = [5 6 7]; disp(x(double(x > 5) + 1))"),
            "     5     6     6\n"
        );
    }

    /// QA D37: a char is UTF-16 code units from the literal to the output.
    #[test]
    fn a_string_literal_is_utf16_and_prints_back_as_utf8() {
        assert_eq!(ok_out("disp(length('😀'))"), "     2\n");
        assert_eq!(ok_out("fprintf('%d %d\\n', double('😀'))"), "55357 56832\n");
        assert_eq!(ok_out("disp('😀')"), "😀\n");
        assert_eq!(ok_out("disp(length('é'))"), "     1\n");
        assert_eq!(ok_out("s = 'a😀b'; disp(s(2:3))"), "😀\n");
        assert_eq!(ok_out("fprintf('%s|\\n', 'x😀')"), "x😀|\n");
        assert_eq!(
            ok_out("x = sprintf('%s', '😀'); disp(size(x))"),
            "     1     2\n"
        );
        assert_eq!(ok_out("s = '😀'"), "s =\n\n    '😀'\n\n");
        // Half a pair cannot be decoded alone.
        assert_eq!(ok_out("s = '😀'; disp(s(1))"), "\u{FFFD}\n");
    }

    /// The named display of each class, through the interpreter.
    #[test]
    fn a_named_display_carries_the_class_header() {
        assert_eq!(ok_out("x = 5 > 3"), "x =\n\n  logical\n\n   1\n\n");
        assert_eq!(
            ok_out("x = [1 2 3] > 1"),
            "x =\n\n  1×3 logical array\n\n   0   1   1\n\n"
        );
        assert_eq!(ok_out("x = ''"), "x =\n\n  0×0 empty char array\n\n");
        assert_eq!(
            ok_out("x = zeros(0, 3)"),
            "x =\n\n  0×3 empty double matrix\n\n"
        );
        assert_eq!(
            ok_out("y = 1:0"),
            "y =\n\n  1×0 empty double row vector\n\n"
        );
        assert_eq!(ok_out("x = []"), "x =\n\n     []\n\n");
        assert_eq!(
            ok_out("c = ['ab'; 'cd']"),
            "c =\n\n  2×2 char array\n\n    'ab'\n    'cd'\n\n"
        );
    }

    #[test]
    fn matrix_power_cases() {
        let a = rmat(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(matrix_power(&a, 0.0).unwrap(), Matrix::identity(2, 2));

        let inv = a.inv().unwrap();
        let p = matrix_power(&a, -1.0).unwrap();
        assert_eq!((p.rows, p.cols), (2, 2));
        for (g, w) in p.data.iter().zip(&inv.data) {
            close(*g, *w);
        }

        assert_eq!(
            matrix_power(&a, 3.0).unwrap(),
            rmat(2, 2, &[37.0, 54.0, 81.0, 118.0])
        );
        assert_eq!(matrix_power(&a, 1.0).unwrap(), a);

        assert!(matrix_power(&rmat(1, 2, &[1.0, 2.0]), 2.0).is_err());
        assert!(matrix_power(&a, 0.5).is_err());
    }
}
