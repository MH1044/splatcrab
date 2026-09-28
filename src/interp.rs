//! Tree-walking interpreter.

use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Instant;

use crate::bail;
use crate::builtins::math::powf_real;
use crate::builtins::{self, Registry};
use crate::error;
use crate::lexer::scan;
use crate::parser::{Access, BinOp, Expr, LValue, Located, MAX_DEPTH, Parser, Stmt};
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

/// One subscript after evaluation, zero-based.
///
/// `eval_index_args` is the one place a one-based subscript becomes one of
/// these (invariant 2); everything that reads a `Sel` is zero-based.
#[derive(Clone, Debug, PartialEq)]
enum Sel {
    /// `:`, every position along the dimension it indexes.
    All,
    /// Positions, zero-based, with the shape the index had: the index
    /// array's own, or for a logical mask the shape `find(mask)` would have.
    /// `max` is the largest one-based position asked for, kept as an `f64`:
    /// a position past `usize` saturates in `idx`, and growth must still be
    /// able to name the size that was asked for (`x(1e300) = 1`).
    List {
        idx: Vec<usize>,
        rows: usize,
        cols: usize,
        max: f64,
    },
}

impl Sel {
    /// A list of positions shaped as a row, for tests and callers that build
    /// one directly.
    #[cfg(test)]
    fn row(idx: Vec<usize>) -> Sel {
        let max = idx.iter().map(|&k| k as f64 + 1.0).fold(0.0, f64::max);
        let cols = idx.len();
        Sel::List {
            idx,
            rows: 1,
            cols,
            max,
        }
    }

    /// The zero-based positions this selects along a dimension of `n`.
    fn positions(&self, n: usize) -> Vec<usize> {
        match self {
            Sel::All => (0..n).collect(),
            Sel::List { idx, .. } => idx.clone(),
        }
    }

    /// How many positions this selects along a dimension of `n`.
    fn count(&self, n: usize) -> usize {
        match self {
            Sel::All => n,
            Sel::List { idx, .. } => idx.len(),
        }
    }

    /// The largest one-based position, or `n` for a colon.
    fn extent(&self, n: usize) -> f64 {
        match self {
            Sel::All => n as f64,
            Sel::List { max, .. } => *max,
        }
    }

    /// True when this selects every position along a dimension of `n`, in
    /// any order: `:`, `1:end` or `[2 1]` of a two-row matrix. A deletion
    /// treats such a subscript as a colon.
    fn covers(&self, n: usize) -> bool {
        match self {
            Sel::All => true,
            Sel::List { idx, .. } => {
                let mut seen = vec![false; n];
                for &k in idx {
                    if k < n {
                        seen[k] = true;
                    }
                }
                seen.into_iter().all(|b| b)
            }
        }
    }
}

/// Which elements a read takes, and the shape of what it returns:
/// `resolve_read`'s answer, which `gather` carries out.
#[derive(Debug, PartialEq)]
struct Gather {
    /// Linear, zero-based, column-major positions in the source, in the
    /// order the result holds them.
    pos: Vec<usize>,
    rows: usize,
    cols: usize,
}

/// Where an assignment stores, and the shape the target has afterwards:
/// `resolve_write`'s answer. Computing it changes nothing, so every check is
/// done before the target is touched.
#[derive(Debug, PartialEq)]
struct Scatter {
    /// The target's shape after growth; its own shape when it does not grow.
    rows: usize,
    cols: usize,
    /// Linear, zero-based positions in the grown target, in the order the
    /// right-hand side's elements are taken.
    pos: Vec<usize>,
}

/// What a deletion keeps, and the shape of what is left.
#[derive(Debug, PartialEq)]
struct Keep {
    pos: Vec<usize>,
    rows: usize,
    cols: usize,
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

    /// The builtin library, read-only: what `env::completions` lists.
    pub fn builtins(&self) -> &Registry {
        &self.builtins
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
                let result = match self.call_form(e) {
                    Some((n, args)) => {
                        let a = self.eval_args(args)?;
                        self.call_builtin(n, a, 0)?
                    }
                    None => vec![self.eval(e)?],
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
            Stmt::Assign(target, e, show) => {
                match (target.chain.as_slice(), e) {
                    // `x(i) = []` with the literal `[]` is a deletion, not a
                    // store of an empty; see `is_deletion`.
                    ([Access::Paren(args)], e) if is_deletion(e) => {
                        self.delete_index(&target.name, args)?
                    }
                    _ => {
                        let v = self.eval(e)?;
                        self.assign_to(target, v)?;
                    }
                }
                if *show {
                    self.show_var(&target.name)?;
                }
                Ok(Flow::Normal)
            }
            Stmt::MultiAssign(targets, e, show) => {
                let values = self.eval_outputs(e, targets.len())?;
                // Each output is assigned, then shown, in the order written;
                // a `~` takes its output and drops it.
                for (target, v) in targets.iter().zip(values) {
                    if let Some(target) = target {
                        self.assign_to(target, v)?;
                        if *show {
                            self.show_var(&target.name)?;
                        }
                    }
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
            Expr::Access(n, chain) => self.eval_access(n, chain),
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

    /// The builtin call an expression makes, if it is one: a name that is
    /// not a variable, bare or with one `(...)`. Those are the forms that can
    /// be asked for other than one value; everything else is evaluated.
    fn call_form<'e>(&self, e: &'e Expr) -> Option<(&'e str, &'e [Expr])> {
        match e {
            Expr::Ident(n) if !self.vars.contains_key(n) => Some((n, &[])),
            Expr::Access(n, chain) if !self.vars.contains_key(n) => match chain.as_slice() {
                [Access::Paren(args)] => Some((n, args)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The `n` values the right-hand side of `[a, b, ...] = rhs` supplies.
    ///
    /// A call is asked for `n` values and must produce at least that many;
    /// a builtin that produces fewer, such as `sum`, is "Too many output
    /// arguments." Any other expression is one value, which satisfies a
    /// single target and is "Insufficient number of outputs ..." for more.
    /// The expression is evaluated either way, so an error inside it is
    /// reported first.
    fn eval_outputs(&mut self, e: &Expr, n: usize) -> R<Vec<Value>> {
        let values = match self.call_form(e) {
            Some((name, args)) => {
                let a = self.eval_args(args)?;
                self.call_builtin(name, a, n)?
            }
            None => {
                let v = self.eval(e)?;
                if n > 1 {
                    bail!(error::insufficient_outputs());
                }
                vec![v]
            }
        };
        if values.len() < n {
            bail!(error::too_many_outputs());
        }
        Ok(values)
    }

    /// Shows a variable under its own name, after an assignment to it.
    fn show_var(&mut self, name: &str) -> R<()> {
        let shown = match self.vars.get(name) {
            Some(v) => v.display(name),
            None => return Ok(()),
        };
        self.emit(&shown)
    }

    /// `name` followed by its access chain.
    ///
    /// On a variable the first link is applied to the variable where it is
    /// stored, so `x(k)` in a loop reads one element rather than copying `x`.
    /// A name that is not a variable is a call: its first `(...)` is the
    /// argument list, and a call with no parentheses takes no arguments.
    /// Every link after the first applies to the value so far.
    fn eval_access(&mut self, name: &str, chain: &[Access]) -> R<Value> {
        let Some((first, rest)) = chain.split_first() else {
            return self.eval_node(&Expr::Ident(name.to_string()));
        };
        let mut v = if self.vars.contains_key(name) {
            match first {
                Access::Paren(args) => self.index_var(name, args)?,
                other => bail!(container_access(other)),
            }
        } else {
            match first {
                Access::Paren(args) => {
                    let a = self.eval_args(args)?;
                    self.call_for_value(name, a)?
                }
                other => {
                    let v = self.call_for_value(name, vec![])?;
                    self.apply_access(v, other)?
                }
            }
        };
        for a in rest {
            v = self.apply_access(v, a)?;
        }
        Ok(v)
    }

    /// One link of a chain applied to a value. A matrix has only `(...)`;
    /// braces and fields are cycle 07's containers.
    fn apply_access(&mut self, v: Value, a: &Access) -> R<Value> {
        match a {
            Access::Paren(args) => {
                let m = v.into_mat();
                let sel = self.eval_index_args(m.rows, m.cols, args)?;
                let g = resolve_read(m.rows, m.cols, &sel)?;
                Ok(Value::Mat(gather(&m, &g)))
            }
            other => Err(container_access(other)),
        }
    }

    /// `name(args)` of a variable, read where the variable is stored.
    ///
    /// The subscripts are evaluated first, knowing only the variable's
    /// shape, because they need `&mut self`; the variable is looked up again
    /// afterwards rather than borrowed across them. A subscript that clears
    /// it (`x(clear('x'))`) therefore finds it gone instead of reading freed
    /// storage, and one that could change its shape is judged against the
    /// shape it has now.
    fn index_var(&mut self, name: &str, args: &[Expr]) -> R<Value> {
        let (rows, cols) = match self.vars.get(name) {
            Some(v) => (v.mat().rows, v.mat().cols),
            None => return Err(error::undefined(name)),
        };
        let sel = self.eval_index_args(rows, cols, args)?;
        let m = match self.vars.get(name) {
            Some(v) => v.mat(),
            None => return Err(error::undefined(name)),
        };
        // Indexing keeps the class, so `s(2)` of a char is a char and `s(:)`
        // is a char column (QA D17).
        let g = resolve_read(m.rows, m.cols, &sel)?;
        Ok(Value::Mat(gather(m, &g)))
    }

    /// Evaluates the subscripts of an index into an array of `rows x cols`.
    ///
    /// **Invariant 2: this is the one place a one-based subscript becomes
    /// zero-based.** Everything downstream of it, reading, assignment and
    /// deletion alike, works on the [`Sel`]s it returns.
    ///
    /// `end` is the number of elements for a single subscript, the rows and
    /// the columns in the first two positions of several, and `1` in any
    /// position after those: a matrix has a trailing singleton dimension in
    /// every position past the second (QA D22).
    ///
    /// A logical subscript is a mask, never a list of positions (QA D6): it
    /// selects the positions `find(mask)` would return, in the shape `find`
    /// would give them, so `x(x > 0)` of `[5 6 7]` is `5 6 7`. A mask shorter
    /// than the array selects among the elements it covers, and a `true`
    /// past the end is a position past the end, for the reader or the writer
    /// to judge.
    fn eval_index_args(&mut self, rows: usize, cols: usize, args: &[Expr]) -> R<Vec<Sel>> {
        if args.is_empty() {
            bail!(error::indexing_rank());
        }
        let mut out = Vec::with_capacity(args.len());
        for (k, a) in args.iter().enumerate() {
            if matches!(a, Expr::Colon) {
                out.push(Sel::All);
                continue;
            }
            let end_val = match (args.len(), k) {
                (1, _) => rows * cols,
                (_, 0) => rows,
                (_, 1) => cols,
                _ => 1,
            };
            self.end_stack.push(end_val);
            let v = self.eval_mat(a);
            self.end_stack.pop();
            let v = v?;
            out.push(if v.class == Class::Logical {
                mask_positions(&v)
            } else {
                index_positions(&v, k + 1)?
            });
        }
        Ok(out)
    }

    /// Assigns `v` to a target, whatever its chain.
    fn assign_to(&mut self, target: &LValue, v: Value) -> R<()> {
        match target.chain.as_slice() {
            [] => {
                self.vars.insert(target.name.clone(), v);
                Ok(())
            }
            [Access::Paren(args)] => self.assign_index(&target.name, args, v),
            // A brace or a field anywhere in the chain is a container's,
            // which a matrix is not; the first one says which. What is left
            // is `x(1)(2) = v`, which MATLAB does not allow either.
            chain => match chain.iter().find(|a| !matches!(a, Access::Paren(_))) {
                Some(a) => Err(container_access(a)),
                None => Err(error::invalid_assignment_target()),
            },
        }
    }

    /// The variable an indexed assignment or a deletion changes, where it is
    /// stored, created as `[]` if it does not exist yet. Looked up by `&str`
    /// so that the common case, an existing variable, allocates nothing.
    fn target_mut(&mut self, name: &str) -> &mut Matrix {
        if !self.vars.contains_key(name) {
            self.vars
                .insert(name.to_string(), Value::Mat(Matrix::empty()));
        }
        match self.vars.get_mut(name) {
            Some(Value::Mat(m)) => m,
            None => unreachable!("inserted above"),
        }
    }

    /// `name(args) = rhs`, in place.
    ///
    /// Everything that can fail is done first: the subscripts, the class
    /// conversion of the right-hand side, the growth and its size check, and
    /// the element count. Only then is the variable's storage touched, and it
    /// is changed where it lies, never cloned. Growth along the last
    /// dimension (a row gaining columns, a column gaining rows, a matrix
    /// gaining columns) keeps the column-major layout, so it is a `resize`
    /// of the storage, which `Vec` amortises: `z(end+1) = k` in a loop is
    /// linear overall, not quadratic.
    fn assign_index(&mut self, name: &str, args: &[Expr], rhs: Value) -> R<()> {
        let rhs = rhs.into_mat();
        // The left-hand side keeps its class, so `s(1) = 'X'` of a char stays
        // a char and `y(2) = 'a'` of a double stores `97`. A variable that
        // does not exist yet, or is the 0x0 double `[]`, takes the class of
        // what is assigned into it, which is how `s = []; s(1) = 'a'` builds
        // a char.
        let (rows, cols, class) = match self.vars.get(name) {
            Some(v) => (v.mat().rows, v.mat().cols, v.mat().class),
            None => (0, 0, Class::Double),
        };
        let class = if class == Class::Double && rows == 0 && cols == 0 {
            rhs.class
        } else {
            class
        };
        let rhs = rhs.to_class(class)?;
        let sel = self.eval_index_args(rows, cols, args)?;
        // Judged against the shape the variable has now; see `index_var`.
        let (rows, cols) = match self.vars.get(name) {
            Some(v) => (v.mat().rows, v.mat().cols),
            None => (0, 0),
        };
        let plan = resolve_write(rows, cols, &sel, &rhs)?;
        let m = self.target_mut(name);
        m.class = class;
        scatter(m, &plan, &rhs);
        Ok(())
    }

    /// `name(args) = []`: deletes elements, rows or columns, keeping the
    /// class. Checked in full before the variable changes.
    fn delete_index(&mut self, name: &str, args: &[Expr]) -> R<()> {
        let (rows, cols) = match self.vars.get(name) {
            Some(v) => (v.mat().rows, v.mat().cols),
            None => (0, 0),
        };
        let sel = self.eval_index_args(rows, cols, args)?;
        let (rows, cols) = match self.vars.get(name) {
            Some(v) => (v.mat().rows, v.mat().cols),
            None => (0, 0),
        };
        let keep = resolve_delete(rows, cols, &sel)?;
        let m = self.target_mut(name);
        let data: Vec<f64> = keep.pos.iter().map(|&p| m.data[p]).collect();
        m.data = data;
        m.rows = keep.rows;
        m.cols = keep.cols;
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

// ---- index resolution ------------------------------------------------
//
// Reading, assignment and deletion share one pipeline: `eval_index_args`
// turns the subscripts into zero-based `Sel`s, a `resolve_*` function judges
// them against the array's shape and says which positions are involved and
// what shape results, and `gather`, `scatter` or the deletion carry that out.
// The `resolve_*` functions change nothing, which is what lets an assignment
// validate everything before it touches the target.

/// The error for a brace or field access on a matrix.
fn container_access(a: &Access) -> error::MError {
    match a {
        Access::Brace(_) => error::brace_indexing_unsupported(),
        _ => error::dot_indexing_unsupported(),
    }
}

/// The deletion form: the right-hand side is the literal `[]` (or `[ ]`, or
/// any bracket with no elements), written as such. An empty value that
/// arrives any other way, `e = []; x(2) = e`, is an ordinary assignment and
/// must match the element count, as in MATLAB.
fn is_deletion(e: &Expr) -> bool {
    matches!(e, Expr::Matrix(rows) if rows.is_empty())
}

/// A logical subscript: the positions of its `true` elements, shaped as
/// `find(mask)` would shape them. A row mask gives a row, a `0x0` mask a
/// `0x0`, and any other mask a column.
fn mask_positions(v: &Matrix) -> Sel {
    let idx: Vec<usize> = v
        .data
        .iter()
        .enumerate()
        .filter(|(_, x)| **x != 0.0)
        .map(|(i, _)| i)
        .collect();
    let max = idx.last().map_or(0.0, |&k| k as f64 + 1.0);
    let n = idx.len();
    let (rows, cols) = if v.rows == 0 && v.cols == 0 {
        (0, 0)
    } else if v.rows == 1 {
        (1, n)
    } else {
        (n, 1)
    };
    Sel::List {
        idx,
        rows,
        cols,
        max,
    }
}

/// A numeric (or char) subscript in position `pos`: each element must be a
/// positive integer. It becomes zero-based here; a position past `usize`
/// saturates, and `max` keeps it exactly for growth to name.
fn index_positions(v: &Matrix, pos: usize) -> R<Sel> {
    let mut idx = Vec::with_capacity(v.numel());
    let mut max = 0.0f64;
    for &x in &v.data {
        if x.fract() != 0.0 || x < 1.0 {
            bail!(error::index_not_positive_integer(pos));
        }
        max = max.max(x);
        idx.push(x as usize - 1);
    }
    Ok(Sel::List {
        idx,
        rows: v.rows,
        cols: v.cols,
        max,
    })
}

/// The subscripts past the second, which index trailing singleton
/// dimensions. Each must select position 1 exactly once: `:`, `1`, `end` or
/// a mask `true`. A position past 1 is the ordinary bounds error when
/// reading; `grows` says the caller is assigning, where it would need an N-D
/// array, as would selecting position 1 twice or not at all.
fn check_trailing(sel: &[Sel], grows: bool) -> R<()> {
    for (k, s) in sel.iter().enumerate().skip(2) {
        if let Sel::List { idx, .. } = s {
            if idx.iter().any(|&i| i > 0) {
                if grows {
                    bail!(error::nd_unsupported());
                }
                bail!(error::index_exceeds_bound(k + 1, 1));
            }
            if idx.len() != 1 {
                bail!(error::nd_unsupported());
            }
        }
    }
    Ok(())
}

/// Which elements `m(sel)` reads from an array of `rows x cols`, and the
/// shape of the result. Every position is bounds-checked here.
///
/// One subscript is linear: a vector indexed by a vector keeps the source's
/// orientation, anything else takes the shape of the index, and `:` is a
/// column. Two or more are rows by columns, with the result's size judged by
/// `check_shape` before anything is allocated.
fn resolve_read(rows: usize, cols: usize, sel: &[Sel]) -> R<Gather> {
    let numel = rows * cols;
    if let [one] = sel {
        return match one {
            Sel::All => Ok(Gather {
                pos: (0..numel).collect(),
                rows: numel,
                cols: 1,
            }),
            Sel::List {
                idx,
                rows: ir,
                cols: ic,
                ..
            } => {
                if idx.iter().any(|&k| k >= numel) {
                    bail!(error::index_exceeds_numel(numel));
                }
                let is_vector = rows == 1 || cols == 1;
                let (r, c) = if is_vector && (*ir == 1 || *ic == 1) {
                    if rows == 1 {
                        (1, idx.len())
                    } else {
                        (idx.len(), 1)
                    }
                } else {
                    (*ir, *ic)
                };
                Ok(Gather {
                    pos: idx.clone(),
                    rows: r,
                    cols: c,
                })
            }
        };
    }
    if let Sel::List { idx, .. } = &sel[0] {
        if idx.iter().any(|&r| r >= rows) {
            bail!(error::index_exceeds_bound(1, rows));
        }
    }
    if let Sel::List { idx, .. } = &sel[1] {
        if idx.iter().any(|&c| c >= cols) {
            bail!(error::index_exceeds_bound(2, cols));
        }
    }
    check_trailing(sel, false)?;
    // A read of several subscripts sizes its result from the subscripts, not
    // from the array: `A(ones(1, 1e5), ones(1, 1e5))` asks for 1e10 elements
    // out of a 2x2 `A`. The bounds tests come first, so an out-of-range
    // subscript is still reported as one.
    let (nr, nc) = (sel[0].count(rows), sel[1].count(cols));
    crate::builtins::args::check_shape(nr as f64, nc as f64)?;
    let (rs, cs) = (sel[0].positions(rows), sel[1].positions(cols));
    let mut pos = Vec::with_capacity(nr * nc);
    for &c in &cs {
        for &r in &rs {
            pos.push(c * rows + r);
        }
    }
    Ok(Gather {
        pos,
        rows: nr,
        cols: nc,
    })
}

/// Carries out a read, keeping the source's class.
fn gather(m: &Matrix, g: &Gather) -> Matrix {
    let data = g.pos.iter().map(|&p| m.data[p]).collect();
    Matrix::new(g.rows, g.cols, data).with_class(m.class)
}

/// Where `m(sel) = rhs` stores into an array of `rows x cols`, and the shape
/// the array grows to. Nothing is changed; see `scatter`.
///
/// A position past the end grows the array: a single subscript grows a
/// vector along its length (an empty becomes a row) and cannot grow a matrix
/// at all; two subscripts grow either dimension. The grown size stays an
/// `f64` until `check_shape` has judged it, so `x = []; x(1e300) = 1` names
/// `1x1e+300` rather than the `usize` it would have saturated to. The
/// right-hand side is a scalar, which fills every position, or has exactly
/// one element per position.
fn resolve_write(rows: usize, cols: usize, sel: &[Sel], rhs: &Matrix) -> R<Scatter> {
    let numel = rows * cols;
    let (nr, nc, pos) = if let [one] = sel {
        let need = one.extent(numel);
        let (nr, nc) = if need <= numel as f64 {
            (rows, cols)
        } else {
            let (r, c) = if numel == 0 || rows == 1 {
                (1.0, need)
            } else if cols == 1 {
                (need, 1.0)
            } else {
                bail!(error::ambiguous_growth());
            };
            crate::builtins::args::check_shape(r, c)?
        };
        (nr, nc, one.positions(numel))
    } else {
        check_trailing(sel, true)?;
        // A colon over a dimension the target does not have yet takes the
        // right-hand side's extent: `A = []; A(:, 1) = [1; 2]` is 2x1.
        let span = |s: &Sel, have: usize, theirs: usize| match s {
            Sel::All if have == 0 => theirs,
            _ => have,
        };
        let (rspan, cspan) = (span(&sel[0], rows, rhs.rows), span(&sel[1], cols, rhs.cols));
        let r = sel[0].extent(rspan).max(rows as f64);
        let c = sel[1].extent(cspan).max(cols as f64);
        let (nr, nc) = crate::builtins::args::check_shape(r, c)?;
        let (rs, cs) = (sel[0].positions(rspan), sel[1].positions(cspan));
        let mut pos = Vec::with_capacity(rs.len() * cs.len());
        for &c in &cs {
            for &r in &rs {
                pos.push(c * nr + r);
            }
        }
        (nr, nc, pos)
    };
    if !rhs.is_scalar() && rhs.numel() != pos.len() {
        bail!(error::assignment_size(pos.len(), rhs.numel()));
    }
    Ok(Scatter {
        rows: nr,
        cols: nc,
        pos,
    })
}

/// Carries out a write planned by `resolve_write`, growing `m` in place.
///
/// When the growth keeps every existing element at its linear position (the
/// row count is unchanged, or a column grows longer, or there was nothing to
/// keep), the storage is resized where it lies; otherwise the elements move
/// to their new column-major positions. New elements are zero, which for a
/// char is the code unit 0.
fn scatter(m: &mut Matrix, plan: &Scatter, rhs: &Matrix) {
    let (nr, nc) = (plan.rows, plan.cols);
    if (nr, nc) != (m.rows, m.cols) {
        let in_place = m.rows == nr || m.data.is_empty() || (m.cols == 1 && nc == 1);
        if in_place {
            m.data.resize(nr * nc, 0.0);
        } else {
            let mut data = vec![0.0; nr * nc];
            for c in 0..m.cols {
                data[c * nr..c * nr + m.rows]
                    .copy_from_slice(&m.data[c * m.rows..(c + 1) * m.rows]);
            }
            m.data = data;
        }
        m.rows = nr;
        m.cols = nc;
    }
    if rhs.is_scalar() {
        let v = rhs.data[0];
        for &p in &plan.pos {
            m.data[p] = v;
        }
    } else {
        for (&p, &v) in plan.pos.iter().zip(&rhs.data) {
            m.data[p] = v;
        }
    }
}

/// What `m(sel) = []` leaves of an array of `rows x cols`.
///
/// One subscript deletes elements by linear position: a vector keeps its
/// orientation, a matrix becomes a row, and `x(:) = []` leaves a 0x0. With
/// two or more, at most one of the first two may select part of its
/// dimension; a subscript that selects all of it (`:`, `1:end`) counts as a
/// colon, and that one removes whole rows or columns. Every subscript past
/// the second must be a colon too, as a singleton it can only select all of.
/// A deletion that removes nothing leaves the array as it is.
fn resolve_delete(rows: usize, cols: usize, sel: &[Sel]) -> R<Keep> {
    let numel = rows * cols;
    let unchanged = || Keep {
        pos: (0..numel).collect(),
        rows,
        cols,
    };
    if let [one] = sel {
        let Sel::List { idx, .. } = one else {
            return Ok(Keep {
                pos: Vec::new(),
                rows: 0,
                cols: 0,
            });
        };
        if idx.iter().any(|&k| k >= numel) {
            bail!(error::index_exceeds_numel(numel));
        }
        let mut gone = vec![false; numel];
        for &k in idx {
            gone[k] = true;
        }
        let pos: Vec<usize> = (0..numel).filter(|&k| !gone[k]).collect();
        if pos.len() == numel {
            return Ok(unchanged());
        }
        let n = pos.len();
        let (r, c) = if cols == 1 && rows != 1 {
            (n, 1)
        } else {
            (1, n)
        };
        return Ok(Keep {
            pos,
            rows: r,
            cols: c,
        });
    }
    for (k, (s, n)) in sel.iter().zip([rows, cols]).enumerate() {
        if let Sel::List { idx, .. } = s {
            if idx.iter().any(|&i| i >= n) {
                bail!(error::index_exceeds_bound(k + 1, n));
            }
        }
    }
    for (k, s) in sel.iter().enumerate().skip(2) {
        if let Sel::List { idx, .. } = s {
            if idx.iter().any(|&i| i > 0) {
                bail!(error::index_exceeds_bound(k + 1, 1));
            }
            if !s.covers(1) {
                bail!(error::null_assignment_indices());
            }
        }
    }
    let (row_all, col_all) = (sel[0].covers(rows), sel[1].covers(cols));
    // The dimension that loses something: the one subscript that is not a
    // colon, or, when both select everything, the one written as a list, or
    // the rows when both are `:`.
    let by_cols = match (row_all, col_all) {
        (false, false) => bail!(error::null_assignment_indices()),
        (true, false) => true,
        (false, true) => false,
        (true, true) => matches!(sel[1], Sel::List { .. }),
    };
    let (dim_sel, n) = if by_cols {
        (&sel[1], cols)
    } else {
        (&sel[0], rows)
    };
    let mut gone = vec![false; n];
    for k in dim_sel.positions(n) {
        gone[k] = true;
    }
    if !gone.iter().any(|&g| g) {
        return Ok(unchanged());
    }
    let left = gone.iter().filter(|&&g| !g).count();
    let pos: Vec<usize> = (0..numel)
        .filter(|&p| !gone[if by_cols { p / rows } else { p % rows }])
        .collect();
    let (r, c) = if by_cols { (rows, left) } else { (left, cols) };
    Ok(Keep {
        pos,
        rows: r,
        cols: c,
    })
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
        let read = |sel: &[Sel]| resolve_read(a.rows, a.cols, sel).map(|g| gather(&a, &g));
        let big = || Sel::row(vec![0; 20_000]);
        let e = read(&[big(), big()]).unwrap_err().msg;
        assert_eq!(
            e,
            "Requested 20000x20000 array exceeds the maximum array size."
        );
        // An out-of-range subscript is still reported as one: the bounds
        // tests come before the size guard.
        let out = Sel::row(vec![5; 20_000]);
        let e = read(&[out, big()]).unwrap_err().msg;
        assert!(e.contains("exceeds array bounds"), "{e}");
        // A result that fits is unaffected.
        let ok = read(&[Sel::row(vec![0]), Sel::All]).unwrap();
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
        // Indexed growth names it as asked too, since cycle 03.
        assert_eq!(
            err_msg("x = []; x(1e300) = 1;"),
            "Requested 1x1e+300 array exceeds the maximum array size."
        );
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

    // ---- logical indexing (cycle 03) ---------------------------------

    /// A mask as `find(mask)` would read it: positions, zero-based, shaped
    /// as `find` shapes them.
    #[test]
    fn a_mask_resolves_to_the_positions_find_would_give() {
        let row = Matrix::row(vec![0.0, 1.0, 1.0]).with_class(Class::Logical);
        assert_eq!(
            mask_positions(&row),
            Sel::List {
                idx: vec![1, 2],
                rows: 1,
                cols: 2,
                max: 3.0
            }
        );
        let col = Matrix::col(vec![1.0, 0.0, 1.0]).with_class(Class::Logical);
        assert_eq!(
            mask_positions(&col),
            Sel::List {
                idx: vec![0, 2],
                rows: 2,
                cols: 1,
                max: 3.0
            }
        );
        // A matrix mask gives a column; a 0x0 mask a 0x0; no trues, nothing.
        let sq = rmat(2, 2, &[1.0, 0.0, 0.0, 1.0]).with_class(Class::Logical);
        assert_eq!(
            mask_positions(&sq),
            Sel::List {
                idx: vec![0, 3],
                rows: 2,
                cols: 1,
                max: 4.0
            }
        );
        let none = Matrix::empty().with_class(Class::Logical);
        assert_eq!(
            mask_positions(&none),
            Sel::List {
                idx: vec![],
                rows: 0,
                cols: 0,
                max: 0.0
            }
        );
        let falses = Matrix::row(vec![0.0, 0.0]).with_class(Class::Logical);
        assert_eq!(
            mask_positions(&falses),
            Sel::List {
                idx: vec![],
                rows: 1,
                cols: 0,
                max: 0.0
            }
        );
    }

    /// Acceptance tests 1, 2, 3 and 16 (QA D6): a mask is a mask, including
    /// one with no zeros, which used to read as the position 1 three times.
    #[test]
    fn a_logical_index_selects_by_mask() {
        assert_eq!(
            ok_out("x = [5 3 8 1]; disp(x(x > 2))"),
            "     5     3     8\n"
        );
        assert_eq!(
            ok_out("x = 1:6; x(x > 4) = 0; disp(x)"),
            "     1     2     3     4     0     0\n"
        );
        assert_eq!(
            ok_out("A = [1 2 3; 4 5 6; 7 8 9]; disp(A(A > 5)')"),
            "     7     8     6     9\n"
        );
        assert_eq!(
            ok_out("x = [5 6 7]; disp(x(x > 0)); x(x > 0) = 0; disp(x)"),
            "     5     6     7\n     0     0     0\n"
        );
        // A double of ones is still a list of positions, and a char index is
        // its codes, as in MATLAB.
        assert_eq!(ok_out("x = [5 6 7]; disp(x([1 1]))"), "     5     5\n");
        assert_eq!(
            ok_out("x = [5 6 7]; disp(x(double(x > 5) + 1))"),
            "     5     6     6\n"
        );
        // The mask of a column vector reads a column, and a mask with no
        // trues reads an empty of the vector's orientation.
        assert_eq!(ok_out("c = [4; 5; 6]; disp(c(c ~= 5))"), "     4\n     6\n");
        assert_eq!(
            ok_out("x = [5 6 7]; disp(size(x(x > 9)))"),
            "     1     0\n"
        );
        // Reading keeps the source's class, and a mask of a char is a char.
        assert_eq!(ok_out("s = 'abcd'; disp(s(s ~= 'b'))"), "acd\n");
    }

    /// Acceptance test 19: a short mask selects among what it covers; a
    /// `true` past the end grows the array on assignment.
    #[test]
    fn a_mask_shorter_or_longer_than_the_array() {
        assert_eq!(
            ok_out("x = [10 20 30]; disp(x(logical([1 0])))"),
            "    10\n"
        );
        assert_eq!(
            ok_out("A = [1 2; 3 4]; disp(A(logical([1 0 0 1])))"),
            "     1     4\n"
        );
        assert_eq!(
            ok_out("A = [1 2; 3 4]; disp(A(logical([1 0; 0 1])))"),
            "     1\n     4\n"
        );
        assert_eq!(
            ok_out("y = [1 2]; y(logical([0 0 1])) = 9; disp(y)"),
            "     1     2     9\n"
        );
        // Falses past the end are harmless; a true past the end is the
        // out-of-bounds error on read, as the position would be.
        assert_eq!(
            ok_out("x = [10 20]; disp(x(logical([0 1 0 0])))"),
            "    20\n"
        );
        assert_eq!(
            err_msg("x = [10 20]; x(logical([0 0 1]))"),
            "Index exceeds the number of array elements. Index must not exceed 2."
        );
    }

    #[test]
    fn a_mask_works_in_either_subscript_of_two() {
        assert_eq!(
            ok_out("A = [1 2; 3 4; 5 6]; disp(A(A(:, 1) > 1, :))"),
            "     3     4\n     5     6\n"
        );
        assert_eq!(
            ok_out("A = [1 2; 3 4]; disp(A(:, logical([0 1])))"),
            "     2\n     4\n"
        );
        assert_eq!(
            ok_out("A = [1 2; 3 4]; A(logical([1 0]), :) = 0; disp(A)"),
            "     0     0\n     3     4\n"
        );
        let e = err_msg("A = [1 2; 3 4]; A(logical([0 0 1]), 1)");
        assert_eq!(
            e,
            "Index in position 1 exceeds array bounds. Index must not exceed 2."
        );
    }

    /// Acceptance test 11: the invalid-index message has MATLAB's ending.
    #[test]
    fn an_invalid_index_names_logical_values() {
        let want = "Index in position 1 is invalid. Array indices must be positive \
                    integers or logical values.";
        assert_eq!(err_msg("x = 1:5; x(0)"), want);
        assert_eq!(err_msg("x = 1:5; x(1.5)"), want);
        assert_eq!(err_msg("x = 1:5; x(-1) = 2;"), want);
        assert_eq!(err_msg("x = 1:5; x(NaN)"), want);
        assert_eq!(
            err_msg("A = eye(2); A(1, 0)"),
            "Index in position 2 is invalid. Array indices must be positive \
             integers or logical values."
        );
    }

    // ---- deletion ----------------------------------------------------

    /// Acceptance tests 4, 5 and 6.
    #[test]
    fn deletion_follows_matlabs_shape_rules() {
        assert_eq!(
            ok_out("x = 1:5; x(2) = []; disp(x); x(logical([1 0 1 0])) = []; disp(x)"),
            "     1     3     4     5\n     3     5\n"
        );
        assert_eq!(
            ok_out("A = [1 2 3; 4 5 6]; A(:, 2) = []; disp(A); A(1, :) = []; disp(A)"),
            "     1     3\n     4     6\n     4     6\n"
        );
        assert_eq!(
            ok_out("A = [1 2; 3 4]; A(2) = []; disp(size(A))"),
            "     1     3\n"
        );
        assert_eq!(
            err_msg("A = [1 2; 3 4]; A(1, 2) = []"),
            "A null assignment can have only one non-colon index."
        );
        // A column keeps its orientation; `x(:) = []` leaves a 0x0.
        assert_eq!(
            ok_out("c = [1; 2; 3]; c(1) = []; disp(size(c))"),
            "     2     1\n"
        );
        assert_eq!(
            ok_out("x = 1:3; x(:) = []; disp(size(x))"),
            "     0     0\n"
        );
        // A subscript that spans its whole dimension counts as a colon.
        assert_eq!(
            ok_out("A = [1 2 3; 4 5 6]; A(1:end, [1 3]) = []; disp(A)"),
            "     2\n     5\n"
        );
        // Deleting nothing leaves the array alone, and the class survives.
        assert_eq!(
            ok_out("A = eye(2); A([]) = []; disp(size(A))"),
            "     2     2\n"
        );
        assert_eq!(
            ok_out("s = 'abc'; s(2) = []; disp(s); disp(class(s))"),
            "ac\nchar\n"
        );
        // A trailing singleton subscript is a colon here too.
        assert_eq!(
            ok_out("A = [1 2; 3 4]; A(:, 1, 1) = []; disp(A)"),
            "     2\n     4\n"
        );
        // Out of range is the usual bounds error, and changes nothing.
        let (r, _) = run("x = 1:3; x(5) = [];");
        assert!(r.unwrap_err().msg.contains("must not exceed 3"));
        let (r, _) = run("A = eye(2); A(:, 3) = [];");
        assert!(
            r.unwrap_err()
                .msg
                .contains("position 2 exceeds array bounds")
        );
    }

    /// Only the literal `[]` deletes. An empty that arrives as a value is
    /// stored, and must match the element count like any other.
    #[test]
    fn only_the_literal_brackets_delete() {
        assert_eq!(
            err_msg("e = []; x = 1:3; x(2) = e;"),
            "Unable to perform assignment because the left side has 1 elements and \
             the right side has 0."
        );
        assert_eq!(ok_out("x = 1:3; x(2) = [ ]; disp(x)"), "     1     3\n");
        // A statement display after a deletion shows what is left.
        assert_eq!(ok_out("x = 1:3; x(1) = []"), "x =\n\n     2     3\n\n");
    }

    #[test]
    fn the_deletion_resolver_keeps_what_is_left() {
        // 2x3, deleting column 2.
        let k = resolve_delete(2, 3, &[Sel::All, Sel::row(vec![1])]).unwrap();
        assert_eq!(
            k,
            Keep {
                pos: vec![0, 1, 4, 5],
                rows: 2,
                cols: 2
            }
        );
        // Deleting row 1.
        let k = resolve_delete(2, 3, &[Sel::row(vec![0]), Sel::All]).unwrap();
        assert_eq!(
            k,
            Keep {
                pos: vec![1, 3, 5],
                rows: 1,
                cols: 3
            }
        );
        // Linear deletion from a matrix makes a row; repeats count once.
        let k = resolve_delete(2, 2, &[Sel::row(vec![1, 1])]).unwrap();
        assert_eq!(
            k,
            Keep {
                pos: vec![0, 2, 3],
                rows: 1,
                cols: 3
            }
        );
        // Both colons remove every row.
        let k = resolve_delete(2, 3, &[Sel::All, Sel::All]).unwrap();
        assert_eq!((k.rows, k.cols, k.pos.len()), (0, 3, 0));
        let e = resolve_delete(2, 2, &[Sel::row(vec![0]), Sel::row(vec![1])]);
        assert_eq!(
            e.unwrap_err().msg,
            "A null assignment can have only one non-colon index."
        );
    }

    // ---- assignment in place -----------------------------------------

    #[test]
    fn the_write_resolver_plans_growth_without_touching_anything() {
        let rhs = Matrix::scalar(9.0);
        // A row grows along its length.
        let p = resolve_write(1, 2, &[Sel::row(vec![3])], &rhs).unwrap();
        assert_eq!(
            p,
            Scatter {
                rows: 1,
                cols: 4,
                pos: vec![3]
            }
        );
        // A column grows down; an empty becomes a row.
        let p = resolve_write(2, 1, &[Sel::row(vec![2])], &rhs).unwrap();
        assert_eq!((p.rows, p.cols), (3, 1));
        let p = resolve_write(0, 0, &[Sel::row(vec![2])], &rhs).unwrap();
        assert_eq!((p.rows, p.cols), (1, 3));
        // Two subscripts grow either dimension, and positions are in the
        // grown shape.
        let p = resolve_write(2, 2, &[Sel::row(vec![2]), Sel::row(vec![2])], &rhs).unwrap();
        assert_eq!(
            p,
            Scatter {
                rows: 3,
                cols: 3,
                pos: vec![8]
            }
        );
        // A matrix cannot grow through one subscript.
        let e = resolve_write(2, 2, &[Sel::row(vec![4])], &rhs).unwrap_err();
        assert_eq!(e.msg, "Attempt to grow array along ambiguous dimension.");
        // The count is checked against the positions.
        let two = Matrix::row(vec![1.0, 2.0]);
        let e = resolve_write(1, 3, &[Sel::row(vec![0, 1, 2])], &two).unwrap_err();
        assert!(e.msg.contains("left side has 3 elements"), "{}", e.msg);
    }

    /// Acceptance test 17: the size asked for is named, not the `usize` it
    /// would saturate to.
    #[test]
    fn growth_past_usize_names_the_size_asked_for() {
        let msg = "Requested 1x1e+300 array exceeds the maximum array size.";
        assert_eq!(err_msg("x = []; x(1e300) = 1"), msg);
        let huge = Sel::List {
            idx: vec![usize::MAX],
            rows: 1,
            cols: 1,
            max: 1e300,
        };
        let e = resolve_write(0, 0, &[huge], &Matrix::scalar(1.0)).unwrap_err();
        assert_eq!(e.msg, msg);
        assert_eq!(
            err_msg("x = zeros(3, 1); x(1e300) = 1;"),
            "Requested 1e+300x1 array exceeds the maximum array size."
        );
        assert_eq!(
            err_msg("A = []; A(2, 1e20) = 1;"),
            "Requested 2x1e+20 array exceeds the maximum array size."
        );
        // Reading there is just out of bounds.
        assert!(err_msg("x = 1:3; x(1e300)").contains("must not exceed 3"));
    }

    /// Validate first, then mutate: a failing assignment leaves the variable
    /// exactly as it was.
    #[test]
    fn a_failed_assignment_changes_nothing() {
        for src in [
            "x = 1:3; x(5) = [1 2];",
            "x = 1:3; x(1e300) = 1;",
            "x = true(1, 3); x(5) = NaN;",
            "x = 1:3; x(0) = 1;",
            "x = 1:3; x([2 9]) = [7 8 9];",
        ] {
            let buf = Rc::new(RefCell::new(Vec::new()));
            let mut it = Interp::with_output(Box::new(Shared(buf)));
            assert!(it.run(src).is_err(), "{src}");
            let x = it.vars["x"].mat();
            assert_eq!((x.rows, x.cols, x.data.len()), (1, 3, 3), "{src}");
        }
    }

    /// Acceptance test 10.
    #[test]
    fn colon_assignment_and_growth_from_empty() {
        assert_eq!(
            ok_out("A = zeros(2); A(:) = 1:4; disp(A); x = []; x(3) = 1; disp(x)"),
            "     1     3\n     2     4\n     0     0     1\n"
        );
        // A matrix grows rows and columns, keeping its elements in place.
        assert_eq!(
            ok_out("A = [1 2; 3 4]; A(3, 1) = 5; disp(A)"),
            "     1     2\n     3     4\n     5     0\n"
        );
        assert_eq!(
            ok_out("A = [1 2; 3 4]; A(1, 3) = 5; disp(A)"),
            "     1     2     5\n     3     4     0\n"
        );
    }

    /// Acceptance test 13's mechanism: growth by one along a row's length is
    /// a resize of the storage where it lies, so its capacity grows
    /// geometrically rather than one element at a time.
    #[test]
    fn appending_to_a_row_reuses_its_storage() {
        let mut it = Interp::with_output(Box::new(io::sink()));
        it.run("z = [];").unwrap();
        let mut reallocations = 0;
        let mut last = std::ptr::null();
        for _ in 0..10_000 {
            it.run("z(end+1) = 1;").unwrap();
            let p = it.vars["z"].mat().data.as_ptr();
            if p != last {
                reallocations += 1;
                last = p;
            }
        }
        let z = it.vars["z"].mat();
        assert_eq!((z.rows, z.cols), (1, 10_000));
        assert!(reallocations < 64, "{reallocations} reallocations");
        assert_eq!(
            ok_out("z = []; for k = 1:2000, z(end+1) = k; end; disp(numel(z)); disp(z(2000))"),
            "        2000\n        2000\n"
        );
    }

    // ---- trailing singleton subscripts (QA D22) ----------------------

    /// Acceptance test 15.
    #[test]
    fn trailing_singleton_subscripts_are_accepted() {
        assert_eq!(
            ok_out("A = [1 2; 3 4]; disp(A(2, 1, 1)); disp(A(:, :, 1)); A(1, 2, 1) = 9; disp(A)"),
            "     3\n     1     2\n     3     4\n     1     9\n     3     4\n"
        );
        assert_eq!(
            err_msg("A = [1 2; 3 4]; A(1, 1, 2)"),
            "Index in position 3 exceeds array bounds. Index must not exceed 1."
        );
        // `end` is 1 in a third position, and a fourth is the same.
        assert_eq!(ok_out("A = [1 2; 3 4]; disp(A(2, 2, end))"), "     4\n");
        assert_eq!(ok_out("A = [1 2; 3 4]; disp(A(1, 2, 1, 1))"), "     2\n");
        assert_eq!(
            err_msg("A = [1 2; 3 4]; A(1, 1, 1, 3)"),
            "Index in position 4 exceeds array bounds. Index must not exceed 1."
        );
        // A page past the first would need an N-D array.
        assert_eq!(
            err_msg("A = [1 2; 3 4]; A(1, 1, 2) = 5;"),
            "N-D arrays are not supported."
        );
        assert_eq!(
            err_msg("A = [1 2; 3 4]; A(:, :, [1 1])"),
            "N-D arrays are not supported."
        );
        // No subscripts at all is still refused.
        assert_eq!(
            err_msg("A = 1; A()"),
            "Only 1-D and 2-D indexing is supported."
        );
    }

    // ---- braces, fields and chains on a matrix -----------------------

    /// Acceptance test 18.
    #[test]
    fn brace_and_dot_access_on_a_matrix_are_errors() {
        let brace = "Brace indexing is not supported for variables of this type.";
        let dot = "Dot indexing is not supported for variables of this type.";
        assert_eq!(err_msg("x = [1 2]; x{1}"), brace);
        assert_eq!(err_msg("x = [1 2]; x.a"), dot);
        assert_eq!(err_msg("x = [1 2]; n = 'a'; x.(n)"), dot);
        assert_eq!(err_msg("x = [1 2]; y = x(1).a;"), dot);
        assert_eq!(err_msg("x = [1 2]; y = x(1){1};"), brace);
        // On a builtin's value too, and on an assignment target.
        assert_eq!(err_msg("pi.a"), dot);
        assert_eq!(err_msg("y = pi{1};"), brace);
        assert_eq!(err_msg("x = [1 2]; x{1} = 3;"), brace);
        assert_eq!(err_msg("x = [1 2]; x.a = 3;"), dot);
        assert_eq!(err_msg("x = [1 2]; x(1).a = 3;"), dot);
        // An undefined name is still undefined first.
        assert_eq!(err_msg("q{1}"), "Unrecognized function or variable 'q'.");
        // A second `(...)` indexes the value so far.
        assert_eq!(ok_out("x = [5 6 7]; disp(x(2:3)(2))"), "     7\n");
        assert_eq!(ok_out("disp(size(ones(2, 3))(2))"), "     3\n");
        assert_eq!(
            err_msg("x = [1 2]; x(1)(1) = 3;"),
            "invalid assignment target"
        );
    }

    // ---- multiple assignment (QA D32) --------------------------------

    /// Acceptance tests 7, 8 and 9.
    #[test]
    fn multiple_outputs_are_assigned_and_shown_in_order() {
        assert_eq!(
            ok_out("[m, i] = max([3 9 2])"),
            "m =\n\n     9\n\ni =\n\n     2\n\n"
        );
        assert_eq!(
            ok_out(
                "[r, c] = size(zeros(2, 5)); fprintf('%d %d\\n', r, c); \
                 [~, i] = min([4 2 8]); disp(i); [s, idx] = sort([3 1 2]); disp(idx)"
            ),
            "2 5\n     2\n     2     3     1\n"
        );
        assert_eq!(
            ok_out("[r, c] = find([0 1; 1 0]); disp([r c])"),
            "     2     1\n     1     2\n"
        );
        // A placeholder is not assigned, and a suppressed list shows nothing.
        assert_eq!(ok_out("[~, i] = max([1 5]); disp(i)"), "     2\n");
        let e = err_msg("[~, i] = max([1 5]); m");
        assert_eq!(e, "Unrecognized function or variable 'm'.");
        // The one-target form, and an indexed target.
        assert_eq!(ok_out("[x] = size(ones(2, 3), 1)"), "x =\n\n     2\n\n");
        assert_eq!(ok_out("[n] = 7;"), "");
        assert_eq!(
            ok_out("v = [0 0]; [v(2), k] = max([4 8]); disp(v); disp(k)"),
            "     0     8\n     2\n"
        );
        // A multiple assignment does not set `ans`.
        assert!(err_msg("[a, b] = size(1); ans").contains("'ans'"));
    }

    /// Acceptance test 12.
    #[test]
    fn asking_for_more_outputs_than_there_are_is_an_error() {
        assert_eq!(
            err_msg("[a, b] = 5"),
            "Insufficient number of outputs from right hand side of equal sign to \
             satisfy assignment."
        );
        assert_eq!(err_msg("x = 1; [a, b] = x;"), err_msg("[a, b] = 5"));
        assert_eq!(err_msg("[a, b] = sum([1 2])"), "Too many output arguments.");
        assert_eq!(
            err_msg("[a, b, c] = max([1 2])"),
            "Too many output arguments."
        );
        assert_eq!(err_msg("[a, b] = disp(1)"), "Too many output arguments.");
        assert_eq!(
            err_msg("[a, b, c, d] = find(1)"),
            "Too many output arguments."
        );
        assert_eq!(
            err_msg("[a, b] = max([1 2], [3 0])"),
            "Too many output arguments."
        );
        // The right-hand side's own error comes first.
        assert_eq!(
            err_msg("[a, b] = nope(1)"),
            "Unrecognized function or variable 'nope'."
        );
        // Nothing is assigned when the outputs fall short.
        let mut it = Interp::with_output(Box::new(io::sink()));
        assert!(it.run("[a, b] = sum(1);").is_err());
        assert!(!it.vars.contains_key("a") && !it.vars.contains_key("b"));
    }

    /// The `nargout` forms of `size`, `max`, `min`, `sort` and `find`,
    /// through the interpreter; the builtins' own tests cover the edges.
    #[test]
    fn builtins_answer_as_many_outputs_as_asked() {
        assert_eq!(
            ok_out("[r, c, p] = size(ones(2, 3)); fprintf('%d %d %d\\n', r, c, p)"),
            "2 3 1\n"
        );
        assert_eq!(ok_out("[r] = size(ones(2, 3)); disp(r)"), "     2     3\n");
        assert_eq!(
            ok_out("[m, i] = min([4 1; 2 3]); disp(m); disp(i)"),
            "     2     1\n     2     1\n"
        );
        assert_eq!(
            ok_out("[s, i] = sort([3 1 2], 'descend'); disp(s); disp(i)"),
            "     3     2     1\n     1     3     2\n"
        );
        assert_eq!(
            ok_out("[r, c, v] = find([0 7; 5 0]); disp([r c v])"),
            "     2     1     5\n     1     2     7\n"
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
