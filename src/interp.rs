//! Tree-walking interpreter.

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Instant, SystemTime};

use crate::bail;
use crate::builtins::math::powf_real;
use crate::builtins::{self, Registry};
use crate::error;
use crate::lexer::{scan, scan_known};
use crate::parser::binop_text;
use crate::parser::{
    Access, AnonFn, BinOp, CaseArm, Expr, Function, LValue, Located, MAX_DEPTH, Parser, Program,
    Stmt,
};
use crate::value::{CellArray, Class, Func, Matrix, StructArray, Value, blank, nonfinite};

/// Every fallible path in the interpreter returns this. It lives in
/// `error.rs`; the re-export is what let cycle 01b swap `String` for `MError`
/// without touching a single `use crate::interp::R` in `builtins/`.
pub use crate::error::R;

/// How many user function calls (and path scripts) may be running at once.
/// One more is MATLAB's "Maximum recursion limit of 500 reached.", a clean
/// error; see the Design notes of `docs/modules/05-functions-and-scoping.md`
/// for the stack this leaves against the 256 MB interpreter thread.
pub const MAX_RECURSION: usize = 500;

/// One workspace: the base workspace, or one running user function's.
///
/// `frames[0]` is the base workspace and is never popped; a call pushes a
/// frame and pops it however the call ends. A function sees only its own
/// frame, so its variables, and its `end`, are its own (invariant 3).
pub struct Frame {
    pub vars: HashMap<String, Value>,
    /// Value of `end` for the index argument currently being evaluated in
    /// this frame. It moved here from `Interp` in cycle 05, which is what
    /// keeps `x(f(end))` binding `end` to `x` whatever `f` indexes.
    end_stack: Vec<usize>,
    /// The file the running code came from, whose local functions are the
    /// first functions a name resolves to (invariant 4). A path script
    /// running in this workspace swaps its own in for as long as it runs.
    unit: Rc<Unit>,
    /// The function this frame runs, `None` for the base workspace.
    func_name: Option<String>,
    /// What `nargin` and `nargout` answer inside the function.
    nargin: usize,
    nargout: usize,
}

impl Frame {
    fn new(unit: Rc<Unit>, func_name: Option<String>) -> Frame {
        Frame {
            vars: HashMap::new(),
            end_stack: Vec::new(),
            unit,
            func_name,
            nargin: 0,
            nargout: 0,
        }
    }
}

/// One parsed source text: a script (the code `run` was given, or a script
/// file on the path) or a function file. A function handle made in it holds
/// it (cycle 06), which is why it is public and `Debug`.
#[derive(Default, Debug)]
pub struct Unit {
    /// The script's statements; empty for a function file.
    stmts: Vec<Located>,
    /// Every function the text defines, by its own name: a script's local
    /// functions, or a function file's entry and subfunctions.
    functions: HashMap<String, Rc<Function>>,
    /// A function file's first function, which its file name calls.
    entry: Option<Rc<Function>>,
    /// The path of the file it was read from, which `e.stack` reports
    /// (cycle 07); empty for the code `run` was given, whose file the
    /// interpreter is not told.
    pub(crate) file: String,
}

impl Unit {
    /// A text whose statements are empty and which defines a function is a
    /// function file, whose first function is its entry. Anything else is
    /// a script. A second definition of one name is ignored.
    fn from_program(prog: Program) -> Unit {
        let is_function_file = prog.stmts.is_empty() && !prog.functions.is_empty();
        let mut unit = Unit {
            stmts: prog.stmts,
            ..Unit::default()
        };
        for f in prog.functions {
            let f = Rc::new(f);
            if is_function_file && unit.entry.is_none() {
                unit.entry = Some(f.clone());
            }
            unit.functions.entry(f.name.clone()).or_insert(f);
        }
        unit
    }
}

/// A function or script file read from disk, and when.
struct CachedFile {
    /// The generation it was last known good in; see `Interp::generation`.
    generation: u64,
    /// Its modification time and length when it was read, which is how a
    /// stale entry is told apart from a changed file.
    stamp: Option<(SystemTime, u64)>,
    unit: Rc<Unit>,
}

pub struct Interp {
    /// The call stack; see [`Frame`]. Never empty.
    frames: Vec<Frame>,
    /// The code `run` is running, whose local functions every frame can
    /// call after its own file's (invariant 4).
    script: Rc<Unit>,
    /// How many user calls are running: the recursion count.
    calls: usize,
    /// The directory every path lookup resolves against: function files,
    /// `addpath` and `rmpath`. Seeded from the process's working directory
    /// and never read from `std::env` again; cycle 13's `cd` changes it.
    pub cwd: PathBuf,
    /// The folders `addpath` added, first searched first. The current
    /// folder is searched before all of them.
    search_path: Vec<PathBuf>,
    /// Bumped by `addpath`, `rmpath` and every `run`, so that no cached
    /// lookup or file from before is used without being checked again.
    generation: u64,
    /// Parsed files, keyed by path.
    files: HashMap<PathBuf, CachedFile>,
    /// Which file a name resolved to, and in which generation.
    lookups: HashMap<String, (u64, Option<PathBuf>)>,
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
    /// The second sink, for diagnostics a program goes on after: `warning`
    /// (cycle 04), and cycle 11's `fprintf(2, ...)`. Stderr in a script and
    /// at the REPL; under `--protocol` and `--ui` the same capture as `out`,
    /// so a warning lands in an `eval`'s `out` in the order it was raised.
    pub err: Box<dyn Write>,
    /// The message of the last error raised, caught or not: what `lasterr`
    /// returns. Empty before the first.
    pub(crate) last_err: String,
}

enum Flow {
    Normal,
    Break,
    Continue,
    /// `return`: ends the running function or script.
    Return,
}

/// What [`Interp::eval_request`] found the expression to be: a call, asked
/// for the outputs wanted; an access chain, whose values are a cs-list of
/// any length (cycle 07); or any other expression, one value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Got {
    Call,
    List,
    One,
}

/// One link of an assignment target after its subscripts are evaluated:
/// what [`assign_chain`] walks (cycle 07). A dynamic field has become the
/// name it evaluated to.
#[derive(Debug)]
enum Link {
    Paren(Vec<Sel>),
    Brace(Vec<Sel>),
    Field(String),
}

/// What a builtin asks [`Interp::call_nested`] to call: a function by name,
/// as `feval('sin', 0)` names one, or a function handle.
pub(crate) enum Callee<'a> {
    Name(&'a str),
    Handle(&'a Func),
}

/// What a `switch` compares its cases against.
enum Subject {
    Text(String),
    Num(f64),
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

    /// Builds an interpreter that writes its output to `out` and its
    /// warnings to stderr.
    pub fn with_output(out: Box<dyn Write>) -> Self {
        Self::with_sinks(out, Box::new(io::stderr()))
    }

    /// Builds an interpreter over both sinks: `out` for output, `err` for
    /// warnings.
    pub fn with_sinks(out: Box<dyn Write>, err: Box<dyn Write>) -> Self {
        let script = Rc::new(Unit::default());
        Interp {
            frames: vec![Frame::new(script.clone(), None)],
            script,
            calls: 0,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            search_path: Vec::new(),
            generation: 0,
            files: HashMap::new(),
            lookups: HashMap::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            builtins: builtins::registry(),
            start: Instant::now(),
            tic_mark: None,
            depth: 0,
            loop_depth: 0,
            out,
            err,
            last_err: String::new(),
        }
    }

    /// The builtin library, read-only: what `env::completions` lists.
    pub fn builtins(&self) -> &Registry {
        &self.builtins
    }

    /// The running frame: the base workspace, or the innermost call's.
    fn frame(&self) -> &Frame {
        self.frames.last().expect("frames[0] is never popped")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("frames[0] is never popped")
    }

    /// The running frame's variables: the base workspace's between runs,
    /// which is what `who`, the protocol's `workspace` and its completions
    /// read.
    pub fn vars(&self) -> &HashMap<String, Value> {
        &self.frame().vars
    }

    pub fn vars_mut(&mut self) -> &mut HashMap<String, Value> {
        &mut self.frame_mut().vars
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

    /// The single place a warning leaves the evaluator. `out` is flushed
    /// first, so that when the two sinks are two streams that end up on one
    /// terminal, what was printed before the warning shows before it.
    pub(crate) fn emit_err(&mut self, s: &str) -> R<()> {
        self.out.flush().map_err(error::output)?;
        self.err.write_all(s.as_bytes()).map_err(error::output)?;
        self.err.flush().map_err(error::output)
    }

    /// Runs `src` as a script in the base workspace: statements, then any
    /// local functions, which the statements can call. What script mode
    /// runs a file with.
    pub fn run(&mut self, src: &str) -> R<()> {
        self.run_with(src, true)
    }

    /// Runs `src` as a command-line entry: the REPL's, a protocol `eval`'s
    /// and so the browser page's. It is a script in every way but one: a
    /// `function` block is refused before anything runs, with MATLAB's
    /// "Function definitions are not supported in this context."
    pub fn run_command(&mut self, src: &str) -> R<()> {
        self.run_with(src, false)
    }

    fn run_with(&mut self, src: &str, functions: bool) -> R<()> {
        // An entry that failed part-way left its counters raised, and the
        // REPL hands the same interpreter the next line. Without this, one
        // over-deep expression would make every later statement too deep and
        // a `break` left mid-loop would make a later top-level one legal.
        // Every call pops its own frame however it ends, so the frames are
        // already back to the base workspace; that is only made sure of.
        self.depth = 0;
        self.loop_depth = 0;
        self.calls = 0;
        self.frames.truncate(1);
        self.frame_mut().end_stack.clear();
        // A file edited since the last entry is read again.
        self.generation += 1;
        let result = self.run_entry(src, functions);
        if let Err(e) = &result {
            self.last_err = e.msg.clone();
        }
        result
    }

    fn run_entry(&mut self, src: &str, functions: bool) -> R<()> {
        // Command syntax depends on which names are variables (`x -1` is an
        // expression when `x` is one), so the lexer is told the workspace.
        let vars = self.vars();
        let lexed = scan_known(src, &|name| vars.contains_key(name))?;
        let prog = Parser::with_lines(lexed).parse_program()?;
        if let (false, Some(f)) = (functions, prog.functions.first()) {
            bail!(error::function_not_supported_here().at(f.line));
        }
        let unit = Rc::new(Unit::from_program(prog));
        // The script's local functions are callable while it runs, and
        // only then.
        let script = std::mem::replace(&mut self.script, unit.clone());
        let base = std::mem::replace(&mut self.frames[0].unit, unit.clone());
        let result = self.exec_block(&unit.stmts);
        self.script = script;
        self.frames[0].unit = base;
        // A `return` at the top ends the script, which is all it can do.
        result.map(|_| ())
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
                // empty Vec, and so is a handle whose body is such a call.
                let (result, got) = self.eval_request(e, 0)?;
                // A cs-list as a statement, `c{:}`, is each of its values in
                // turn as `ans` (cycle 07).
                if got == Got::List && result.len() != 1 {
                    for v in result {
                        self.vars_mut().insert("ans".to_string(), v.clone());
                        if *show {
                            self.emit(&v.display("ans"))?;
                        }
                    }
                    return Ok(Flow::Normal);
                }
                if let Some(v) = result.into_iter().next() {
                    let name = match e {
                        Expr::Ident(n) if self.vars().contains_key(n) => n.clone(),
                        _ => "ans".to_string(),
                    };
                    if name == "ans" {
                        self.vars_mut().insert(name.clone(), v.clone());
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
                    // `s.list(2) = []`, `c{1}(2) = []`: a deletion at the
                    // end of a longer chain, in the container it names.
                    ([.., Access::Paren(_)], e) if is_deletion(e) => {
                        self.delete_at(&target.name, &target.chain)?
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
                let m = match self.eval(e)? {
                    Value::Mat(m) => m,
                    // A cell or a struct array iterates its columns, each a
                    // cell or a struct of its own (cycle 07).
                    v @ (Value::Cell(_) | Value::Struct(_)) => {
                        self.loop_depth += 1;
                        let flow = self.run_for_items(name, &v, body);
                        self.loop_depth -= 1;
                        return flow;
                    }
                    v => bail!(error::not_an_array(v.class_name())),
                };
                // A loop that does not run still assigns its variable, as
                // MATLAB does: after `k = 7; for k = []; end`, `k` is the
                // empty, not `7`, and a name that did not exist comes into
                // existence. The value is the empty that the next column
                // would have been, which gives `[]` a 0x0 and `1:0` a 1x0,
                // the shapes Octave assigns. MATLAB's exact shape is
                // unsettled, so no golden case asserts it.
                if m.cols == 0 {
                    let empty = Matrix::new(m.rows, 0, Vec::new()).with_class(m.class);
                    self.vars_mut().insert(name.clone(), Value::Mat(empty));
                }
                self.loop_depth += 1;
                let flow = self.run_for(name, &m, body);
                self.loop_depth -= 1;
                flow
            }
            Stmt::While(cond, body) => {
                self.loop_depth += 1;
                let flow = self.run_while(cond, body);
                self.loop_depth -= 1;
                flow
            }
            // MATLAB errors on a `break` with no loop around it; Octave 8.4
            // gives a parse error. It is raised here rather than in the
            // parser so that everything the script printed first is still
            // printed, which is what the script's own output shows.
            Stmt::Switch(subject, arms, otherwise) => self.exec_switch(subject, arms, otherwise),
            Stmt::Try(body, var, handler) => {
                // An error unwinding out of the body can leave the nesting
                // counters raised (a failed `deepen` does not undo itself),
                // so they are put back to what they were at the `try` before
                // the handler runs. `end_stack` is popped on every path.
                let (depth, loop_depth, ends) =
                    (self.depth, self.loop_depth, self.frame().end_stack.len());
                match self.exec_block(body) {
                    Ok(flow) => Ok(flow),
                    Err(e) => {
                        self.depth = depth;
                        self.loop_depth = loop_depth;
                        self.frame_mut().end_stack.truncate(ends);
                        self.last_err = e.msg.clone();
                        if let Some(name) = var {
                            self.vars_mut().insert(name.clone(), Value::Exception(e));
                        }
                        self.exec_block(handler)
                    }
                }
            }
            Stmt::Break if self.loop_depth == 0 => Err(error::break_outside_loop()),
            Stmt::Continue if self.loop_depth == 0 => Err(error::continue_outside_loop()),
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
            Stmt::Return => Ok(Flow::Return),
        }
    }

    /// `switch subject, case ..., otherwise ..., end`.
    ///
    /// The subject is a scalar or a character vector. A char subject matches
    /// a char case with the same text and never a number, so `switch 'a'`
    /// does not match `case 97`; a numeric or logical subject matches a
    /// scalar case of equal value that is not a char, whatever its class.
    /// Case values are evaluated in order, only until one matches; a
    /// `case {a, b}` list matches when any of its values does.
    fn exec_switch(
        &mut self,
        subject: &Expr,
        arms: &[CaseArm],
        otherwise: &Option<Vec<Located>>,
    ) -> R<Flow> {
        let subject = match self.eval(subject)? {
            Value::Mat(m) if m.class == Class::Char && m.rows <= 1 => Subject::Text(m.text()),
            Value::Mat(m) if m.is_scalar() && m.class != Class::Char => Subject::Num(m.data[0]),
            _ => bail!(error::switch_expression()),
        };
        for arm in arms {
            for e in &arm.values {
                let v = self.eval(e).map_err(|err| err.at(arm.line))?;
                let hit = match (&subject, &v) {
                    (Subject::Text(s), Value::Mat(m)) => {
                        m.class == Class::Char && m.rows <= 1 && m.text() == *s
                    }
                    (Subject::Num(x), Value::Mat(m)) => {
                        m.class != Class::Char && m.is_scalar() && m.data[0] == *x
                    }
                    _ => false,
                };
                if hit {
                    return self.exec_block(&arm.body);
                }
            }
        }
        match otherwise {
            Some(body) => self.exec_block(body),
            None => Ok(Flow::Normal),
        }
    }

    /// The iterations of a `for`, split out so the caller can put
    /// `loop_depth` back however the body ends: normally, on a `break`, or on
    /// an error unwinding through it. A `loop_depth` left raised would make a
    /// later top-level `break` legal. A `return` in the body ends the loop
    /// and is passed on, to end the function or script around it.
    fn run_for(&mut self, name: &str, m: &Matrix, body: &[Located]) -> R<Flow> {
        for c in 0..m.cols {
            let column: Vec<f64> = (0..m.rows).map(|r| m.get(r, c)).collect();
            // Each column keeps the class, so `for k = 'abc'` iterates chars.
            let v = if m.rows == 1 {
                Matrix::scalar(column[0])
            } else {
                Matrix::col(column)
            }
            .with_class(m.class);
            self.vars_mut().insert(name.to_string(), Value::Mat(v));
            match self.exec_block(body)? {
                Flow::Break => break,
                Flow::Return => return Ok(Flow::Return),
                Flow::Normal | Flow::Continue => {}
            }
        }
        Ok(Flow::Normal)
    }

    /// `for x = c` over a cell or a struct array (cycle 07): one iteration
    /// per column, `x` being that column as a cell or a struct array of its
    /// own, so over a row each is 1x1. As for a matrix, a loop over no
    /// columns still assigns its variable, the empty of the same kind.
    fn run_for_items(&mut self, name: &str, v: &Value, body: &[Located]) -> R<Flow> {
        let (rows, cols) = v.dims();
        let column = |j: usize| match v {
            Value::Cell(c) => Value::cell(CellArray::new(
                rows,
                1,
                c.data[j * rows..(j + 1) * rows].to_vec(),
            )),
            Value::Struct(s) => Value::strukt(StructArray::new(
                rows,
                1,
                s.fields.clone(),
                s.elems[j * rows..(j + 1) * rows].to_vec(),
            )),
            v => v.clone(),
        };
        if cols == 0 {
            let empty = match v {
                Value::Struct(s) => {
                    Value::strukt(StructArray::new(rows, 0, s.fields.clone(), Vec::new()))
                }
                _ => Value::cell(CellArray::new(rows, 0, Vec::new())),
            };
            self.vars_mut().insert(name.to_string(), empty);
        }
        for j in 0..cols {
            let x = column(j);
            self.vars_mut().insert(name.to_string(), x);
            match self.exec_block(body)? {
                Flow::Break => break,
                Flow::Return => return Ok(Flow::Return),
                Flow::Normal | Flow::Continue => {}
            }
        }
        Ok(Flow::Normal)
    }

    /// The iterations of a `while`; see [`run_for`](Interp::run_for).
    fn run_while(&mut self, cond: &Expr, body: &[Located]) -> R<Flow> {
        while self.eval_mat(cond)?.truth()? {
            match self.exec_block(body)? {
                Flow::Break => break,
                Flow::Return => return Ok(Flow::Return),
                Flow::Normal | Flow::Continue => {}
            }
        }
        Ok(Flow::Normal)
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
                if let Some(v) = self.vars().get(n) {
                    return Ok(v.clone());
                }
                self.call_for_value(n, vec![])
            }
            Expr::End => self
                .frame()
                .end_stack
                .last()
                .map(|n| Value::Mat(Matrix::scalar(*n as f64)))
                .ok_or_else(error::end_outside_index),
            Expr::Colon => Err(error::colon_outside_index()),
            // Where one value is wanted, a cs-list must be exactly one.
            Expr::Access(n, chain) => one_value(self.eval_access(n, chain)?),
            Expr::Matrix(rows) => self.build_matrix(rows),
            Expr::Cell(rows) => self.build_cell(rows),
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
            Expr::FuncHandle(name) => Ok(self.named_handle(name)),
            Expr::AnonFn(def) => Ok(self.anon_handle(def, true)),
        }
    }

    /// `@name`, bound where it is made (cycle 06): to the local function
    /// the name resolves to here by invariant 4's order, if there is one.
    /// A variable of the name plays no part, as `@` names a function.
    pub(crate) fn named_handle(&self, name: &str) -> Value {
        let local = self.local_function(name);
        Value::Func(Rc::new(Func::Named {
            name: name.to_string(),
            local,
        }))
    }

    /// `@(params) body`, made here (cycle 06). With `capture`, every name
    /// the body reads that is a variable of the running frame now is
    /// snapshotted with its value, so `a = 10; f = @(x) x + a; a = 0`
    /// leaves `f` adding 10. A name that is not a variable now is looked up
    /// as a function when the body runs, never as a variable of wherever
    /// the handle is called from. `str2func` captures nothing.
    pub(crate) fn anon_handle(&self, def: &Rc<AnonFn>, capture: bool) -> Value {
        let captured = if capture {
            def.free_names()
                .into_iter()
                .filter_map(|n| self.vars().get(&n).map(|v| (n.clone(), v.clone())))
                .collect()
        } else {
            Vec::new()
        };
        Value::Func(Rc::new(Func::Anon {
            def: def.clone(),
            captured,
            unit: self.frame().unit.clone(),
        }))
    }

    fn eval_mat(&mut self, e: &Expr) -> R<Matrix> {
        self.eval(e)?.into_mat()
    }

    fn eval_scalar(&mut self, e: &Expr, what: &str) -> R<f64> {
        self.eval_mat(e)?
            .scalar_value()
            .ok_or_else(|| error::not_a_scalar(what))
    }

    /// The arguments of a call. A cs-list among them, `f(c{:})` or
    /// `f(s.name)` of a struct array, is spread into as many arguments as
    /// it has values, none included (cycle 07).
    fn eval_args(&mut self, args: &[Expr]) -> R<Vec<Value>> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.extend(self.eval_multi(a)?);
        }
        Ok(out)
    }

    /// An expression where a cs-list is welcome: a call's argument, an
    /// element of `[...]` or of `{...}`. An access chain gives all of its
    /// values; anything else its one value.
    fn eval_multi(&mut self, e: &Expr) -> R<Vec<Value>> {
        match e {
            Expr::Access(n, chain) => {
                self.deepen()?;
                let v = self.eval_access(n, chain);
                self.depth -= 1;
                v
            }
            e => Ok(vec![self.eval(e)?]),
        }
    }

    /// An operand of a binary operator: a matrix, or MATLAB R2020a's
    /// refusal naming the operator and the operand's class (cycle 07).
    fn operand(&mut self, e: &Expr, op: BinOp) -> R<Matrix> {
        match self.eval(e)? {
            Value::Mat(m) => Ok(m),
            v => Err(error::operator_unsupported(binop_text(op), v.class_name())),
        }
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> R<Value> {
        // Short-circuit logical operators. Each operand must be convertible to
        // one logical value, so `[1 1] && 1` and `[] || 1` are errors rather
        // than `1`; the right-hand side is still not evaluated when the left
        // already decides the answer, so `0 && undefined_fn()` is `0`.
        match op {
            BinOp::AndAnd => {
                let l = self.operand(a, op)?.logical_scalar()?;
                let v = l && self.operand(b, op)?.logical_scalar()?;
                return Ok(Value::Mat(Matrix::from_bool(v)));
            }
            BinOp::OrOr => {
                let l = self.operand(a, op)?.logical_scalar()?;
                let v = l || self.operand(b, op)?.logical_scalar()?;
                return Ok(Value::Mat(Matrix::from_bool(v)));
            }
            _ => {}
        }
        // Both operands are evaluated before either is judged, so `c + x`
        // with `x` undefined names `x`, and the class named is the first
        // operand's that is not an array.
        let (a, b) = match (self.eval(a)?, self.eval(b)?) {
            (Value::Mat(a), Value::Mat(b)) => (a, b),
            (Value::Mat(_), v) | (v, _) => {
                bail!(error::operator_unsupported(binop_text(op), v.class_name()))
            }
        };
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
                elems.extend(self.eval_multi(e)?);
            }
            row_vals.push(hcat(elems)?);
        }
        vcat(row_vals)
    }

    /// `{a, b; c, d}` (cycle 07): each value is one element of the cell,
    /// whatever it is, so `{c}` of a cell nests it. A cs-list among them
    /// gives one element per value. Every row must have as many elements
    /// as the first; a row that comes to none is left out, so `{}` is 0x0.
    fn build_cell(&mut self, rows: &[Vec<Expr>]) -> R<Value> {
        let mut grid: Vec<Vec<Value>> = Vec::with_capacity(rows.len());
        for row in rows {
            let mut elems = Vec::with_capacity(row.len());
            for e in row {
                elems.extend(self.eval_multi(e)?);
            }
            if !elems.is_empty() {
                grid.push(elems);
            }
        }
        let Some(first) = grid.first() else {
            return Ok(Value::cell(CellArray::default()));
        };
        let (nr, nc) = (grid.len(), first.len());
        if grid.iter().any(|r| r.len() != nc) {
            bail!(error::concat_dims());
        }
        // Row-major as written; column-major as stored.
        let mut cells: Vec<std::vec::IntoIter<Value>> =
            grid.into_iter().map(Vec::into_iter).collect();
        let mut data = Vec::with_capacity(nr * nc);
        for _ in 0..nc {
            for r in cells.iter_mut() {
                data.extend(r.next());
            }
        }
        Ok(Value::cell(CellArray::new(nr, nc, data)))
    }

    // ---- indexing ----------------------------------------------------

    /// The call an expression makes, if it is one: a name that is not a
    /// variable, bare or with one `(...)`. Those are the forms that can
    /// be asked for other than one value; everything else is evaluated.
    fn call_form<'e>(&self, e: &'e Expr) -> Option<(&'e str, &'e [Expr])> {
        match e {
            Expr::Ident(n) if !self.vars().contains_key(n) => Some((n, &[])),
            Expr::Access(n, chain) if !self.vars().contains_key(n) => match chain.as_slice() {
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
        let (values, got) = self.eval_request(e, n)?;
        match got {
            // A cs-list, `[a, b] = c{:}` (cycle 07), supplies its values in
            // order, and too few is the right-hand side's shortfall.
            Got::List if values.len() < n => bail!(error::insufficient_outputs()),
            Got::One if n > 1 => bail!(error::insufficient_outputs()),
            _ if values.len() < n => bail!(error::too_many_outputs()),
            _ => {}
        }
        Ok(values)
    }

    /// `e` asked for `nargout` values: a call (a function by name, or a
    /// handle variable called with one `(...)`) is asked for exactly that
    /// many and may give fewer, for the caller to judge; anything else is
    /// evaluated to its one value. The flag says whether it was a call.
    ///
    /// This is also how an anonymous function's body runs (cycle 06), which
    /// is what carries `nargout` through a body that is a single call:
    /// `f = @(v) max(v); [m, i] = f(v)` asks `max` for two.
    fn eval_request(&mut self, e: &Expr, nargout: usize) -> R<(Vec<Value>, Got)> {
        if let Some((name, args)) = self.call_form(e) {
            let a = self.eval_args(args)?;
            return Ok((self.call_function(name, a, nargout)?, Got::Call));
        }
        if let Some((f, args)) = self.handle_form(e) {
            let a = self.eval_args(args)?;
            return Ok((self.call_handle(&f, a, nargout)?, Got::Call));
        }
        if let Expr::Access(..) = e {
            return Ok((self.eval_multi(e)?, Got::List));
        }
        Ok((vec![self.eval(e)?], Got::One))
    }

    /// The handle an expression calls, if it is `f(args)` with `f` a
    /// variable holding a function handle and nothing after the `(...)`.
    fn handle_form<'e>(&self, e: &'e Expr) -> Option<(Rc<Func>, &'e [Expr])> {
        match e {
            Expr::Access(n, chain) => match (self.vars().get(n), chain.as_slice()) {
                (Some(Value::Func(f)), [Access::Paren(args)]) => Some((f.clone(), args)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Shows a variable under its own name, after an assignment to it.
    fn show_var(&mut self, name: &str) -> R<()> {
        let shown = match self.vars().get(name) {
            Some(v) => v.display(name),
            None => return Ok(()),
        };
        self.emit(&shown)
    }

    /// `name` followed by its access chain, as the values it gives: one for
    /// most chains, and a cs-list of any length for a brace index that
    /// selects several elements of a cell or a field of a struct array
    /// (cycle 07). A caller that wants one value says so with
    /// [`one_value`].
    ///
    /// On a variable, the leading fields of scalar structs are walked where
    /// the variable is stored, and a `(...)` of a matrix reached that way is
    /// read in place, so `x(k)` and `s.data(k)` in a loop read one element
    /// rather than copying the array. A name that is not a variable is a
    /// call: its first `(...)` is the argument list, and a call with no
    /// parentheses takes no arguments. Every link after that applies to the
    /// value so far, which must be one value: a cs-list in the middle of a
    /// chain, `p.name(1)` of a 1x2 `p`, is the cs-list error.
    fn eval_access(&mut self, name: &str, chain: &[Access]) -> R<Vec<Value>> {
        let (mut vals, done) = if self.vars().contains_key(name) {
            self.read_var(name, chain)?
        } else {
            match chain.first() {
                Some(Access::Paren(args)) => {
                    let a = self.eval_args(args)?;
                    (vec![self.call_for_value(name, a)?], 1)
                }
                _ => (vec![self.call_for_value(name, vec![])?], 0),
            }
        };
        for a in &chain[done..] {
            let v = one_value(vals)?;
            vals = self.apply_access(v, a)?;
        }
        Ok(vals)
    }

    /// The start of a chain on the variable `name`: its leading fields of
    /// scalar structs walked in place, then a `(...)` of what they reach
    /// when that is a matrix (read in place) or a handle (called). Returns
    /// the values and how many links it used; the rest are the caller's.
    ///
    /// The subscripts are evaluated knowing only the shape, because they
    /// need `&mut self`; the path is walked again afterwards rather than
    /// borrowed across them. A subscript that clears the variable
    /// (`x(clear('x'))`) therefore finds it gone instead of reading freed
    /// storage, and one that could change its shape is judged against the
    /// shape it has now.
    fn read_var(&mut self, name: &str, chain: &[Access]) -> R<(Vec<Value>, usize)> {
        let nf = self.plain_fields(name, chain);
        if let Some(Access::Paren(args)) = chain.get(nf) {
            let fields = &chain[..nf];
            let shape = match self.at_fields(name, fields) {
                Some(Value::Mat(m)) => Some((m.rows, m.cols)),
                _ => None,
            };
            if let Some((rows, cols)) = shape {
                let sel = self.eval_index_args(rows, cols, args)?;
                let Some(Value::Mat(m)) = self.at_fields(name, fields) else {
                    return Err(error::undefined(name));
                };
                // Indexing keeps the class, so `s(2)` of a char is a char and
                // `s(:)` is a char column (QA D17).
                let g = resolve_read(m.rows, m.cols, &sel)?;
                return Ok((vec![Value::Mat(gather(m, &g))], nf + 1));
            }
            // A handle's `(...)` is a call of the handle.
            if let Some(Value::Func(f)) = self.at_fields(name, fields) {
                let f = f.clone();
                let a = self.eval_args(args)?;
                return Ok((vec![self.call_handle_for_value(&f, a)?], nf + 1));
            }
        }
        let v = self
            .at_fields(name, &chain[..nf])
            .cloned()
            .ok_or_else(|| error::undefined(name))?;
        Ok((vec![v], nf))
    }

    /// How many links at the start of `chain` are fields that exist, each
    /// of a 1x1 struct, starting from the variable `name`.
    fn plain_fields(&self, name: &str, chain: &[Access]) -> usize {
        let mut at = self.vars().get(name);
        let mut n = 0;
        for a in chain {
            match (at, a) {
                (Some(Value::Struct(s)), Access::Field(f)) if s.numel() == 1 => {
                    match s.field_index(f) {
                        Some(i) => at = Some(&s.elems[0][i]),
                        None => break,
                    }
                }
                _ => break,
            }
            n += 1;
        }
        n
    }

    /// The value the variable `name` holds at the end of `fields`, which
    /// [`plain_fields`](Interp::plain_fields) has vouched are all fields of
    /// scalar structs; `None` if the path has gone since.
    fn at_fields(&self, name: &str, fields: &[Access]) -> Option<&Value> {
        let mut at = self.vars().get(name)?;
        for a in fields {
            match (at, a) {
                (Value::Struct(s), Access::Field(f)) if s.numel() == 1 => {
                    at = &s.elems[0][s.field_index(f)?];
                }
                _ => return None,
            }
        }
        Some(at)
    }

    /// One link of a chain applied to one value, giving the values it
    /// selects. A matrix has only `(...)`; a cell has `(...)`, which gives a
    /// cell, and `{...}`, which gives the elements themselves; a struct
    /// array has `(...)`, which gives a struct array, and its fields, one
    /// value per element (cycle 07).
    fn apply_access(&mut self, v: Value, a: &Access) -> R<Vec<Value>> {
        match (v, a) {
            (Value::Exception(e), Access::Field(f)) => Ok(vec![exception_field(&e, f)?]),
            (Value::Exception(e), Access::DynField(f)) => match self.eval(f)?.text() {
                Some(f) => Ok(vec![exception_field(&e, &f)?]),
                None => Err(error::dot_indexing_unsupported()),
            },
            // A handle a chain produced, `add(3)(4)` or `c{1}(2)`, is
            // called, as chained indexing reads each link in turn.
            (Value::Func(f), Access::Paren(args)) => {
                let a = self.eval_args(args)?;
                Ok(vec![self.call_handle_for_value(&f, a)?])
            }
            (Value::Mat(m), Access::Paren(args)) => {
                let sel = self.eval_index_args(m.rows, m.cols, args)?;
                let g = resolve_read(m.rows, m.cols, &sel)?;
                Ok(vec![Value::Mat(gather(&m, &g))])
            }
            (Value::Cell(c), Access::Brace(args)) => {
                let sel = self.eval_index_args(c.rows, c.cols, args)?;
                let g = resolve_read(c.rows, c.cols, &sel)?;
                Ok(pick(&c.data, &g.pos))
            }
            (Value::Cell(c), Access::Paren(args)) => {
                let sel = self.eval_index_args(c.rows, c.cols, args)?;
                let g = resolve_read(c.rows, c.cols, &sel)?;
                let data = pick(&c.data, &g.pos);
                Ok(vec![Value::cell(CellArray::new(g.rows, g.cols, data))])
            }
            (Value::Struct(s), Access::Paren(args)) => {
                let sel = self.eval_index_args(s.rows, s.cols, args)?;
                let g = resolve_read(s.rows, s.cols, &sel)?;
                Ok(vec![Value::strukt(StructArray::new(
                    g.rows,
                    g.cols,
                    s.fields.clone(),
                    pick(&s.elems, &g.pos),
                ))])
            }
            (Value::Struct(s), Access::Field(f)) => struct_field(&s, f),
            (Value::Struct(s), Access::DynField(x)) => {
                let f = self.field_name(x)?;
                struct_field(&s, &f)
            }
            (v, Access::Paren(_)) => Err(error::not_an_array(v.class_name())),
            (_, other) => Err(container_access(other)),
        }
    }

    /// The name a dynamic field `.(expr)` evaluates to: text, or the
    /// refusal.
    fn field_name(&mut self, e: &Expr) -> R<String> {
        self.eval(e)?
            .text()
            .ok_or_else(error::dynamic_field_not_text)
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
            self.frame_mut().end_stack.push(end_val);
            let v = self.eval_mat(a);
            self.frame_mut().end_stack.pop();
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
    ///
    /// Since cycle 07 every chain goes one way: its subscripts and dynamic
    /// field names are evaluated first, against the shapes the path has
    /// now (`resolve_links`), and [`assign_chain`] then stores `v` at the
    /// end of the path, where it lies, creating whatever the path does not
    /// hold yet. A variable that did not exist is created as `[]` for the
    /// walk and removed again if the assignment fails, so a failed
    /// assignment changes nothing.
    fn assign_to(&mut self, target: &LValue, v: Value) -> R<()> {
        if target.chain.is_empty() {
            self.vars_mut().insert(target.name.clone(), v);
            return Ok(());
        }
        // `assign_chain` recurses once per link; the parser reads a chain
        // as a flat list and bounds nothing, so the bound is here.
        if target.chain.len() > MAX_DEPTH {
            bail!(error::nesting_too_deep(MAX_DEPTH));
        }
        let links = self.resolve_links(&target.name, &target.chain)?;
        let name = &target.name;
        let existed = self.vars().contains_key(name);
        if !existed {
            self.vars_mut().insert(name.clone(), blank());
        }
        let slot = self
            .vars_mut()
            .get_mut(name)
            .expect("the variable exists or was inserted above");
        let r = assign_chain(slot, &links, v);
        if r.is_err() && !existed {
            self.vars_mut().remove(name);
        }
        r
    }

    /// The links of an assignment target with their subscripts evaluated
    /// (cycle 07): each `(...)` and `{...}` against the shape of what the
    /// path holds at that point, so `end` means what it would mean if the
    /// path were read, and `0x0` where the path holds nothing yet; each
    /// `.(expr)` becomes the field it names, which must be a valid name.
    fn resolve_links(&mut self, name: &str, chain: &[Access]) -> R<Vec<Link>> {
        let mut links = Vec::with_capacity(chain.len());
        for a in chain {
            let link = match a {
                Access::Field(f) => Link::Field(f.clone()),
                Access::DynField(x) => {
                    let f = self.field_name(x)?;
                    if !is_identifier(&f) {
                        bail!(error::invalid_field_name(&f));
                    }
                    Link::Field(f)
                }
                Access::Paren(args) => {
                    let (rows, cols) = self.shape_at(name, &links);
                    Link::Paren(self.eval_index_args(rows, cols, args)?)
                }
                Access::Brace(args) => {
                    let (rows, cols) = self.shape_at(name, &links);
                    Link::Brace(self.eval_index_args(rows, cols, args)?)
                }
            };
            links.push(link);
        }
        Ok(links)
    }

    /// The shape of what the variable `name` holds at the end of `links`,
    /// walked by reference, or `0x0` where the path does not reach: an
    /// undefined variable, a missing field, a position past the end.
    fn shape_at(&self, name: &str, links: &[Link]) -> (usize, usize) {
        /// A value, or element `k` of a struct array, which is not a value
        /// of its own until a field of it is taken.
        #[derive(Clone, Copy)]
        enum At<'a> {
            V(&'a Value),
            E(&'a StructArray, usize),
        }
        let Some(v) = self.vars().get(name) else {
            return (0, 0);
        };
        let mut at = At::V(v);
        for l in links {
            let next = match (at, l) {
                (At::V(Value::Struct(s)), Link::Field(f)) if s.numel() == 1 => {
                    s.field_index(f).map(|i| At::V(&s.elems[0][i]))
                }
                (At::E(s, k), Link::Field(f)) => s.field_index(f).map(|i| At::V(&s.elems[k][i])),
                (At::V(Value::Cell(c)), Link::Brace(sel)) => {
                    one_position(c.rows, c.cols, sel).map(|p| At::V(&c.data[p]))
                }
                (At::V(Value::Struct(s)), Link::Paren(sel)) => {
                    one_position(s.rows, s.cols, sel).map(|p| At::E(s, p))
                }
                _ => None,
            };
            match next {
                Some(n) => at = n,
                None => return (0, 0),
            }
        }
        match at {
            At::V(v) => v.dims(),
            At::E(..) => (1, 1),
        }
    }

    /// `name(args) = []`: deletes elements, rows or columns of a matrix, a
    /// cell or a struct array, keeping its kind and class. Checked in full
    /// before the variable changes.
    fn delete_index(&mut self, name: &str, args: &[Expr]) -> R<()> {
        let (rows, cols) = self.vars().get(name).map_or((0, 0), Value::dims);
        let sel = self.eval_index_args(rows, cols, args)?;
        match self.vars_mut().get_mut(name) {
            Some(v) => delete_in(v, &sel),
            None => {
                let mut v = blank();
                delete_in(&mut v, &sel)?;
                self.vars_mut().insert(name.to_string(), v);
                Ok(())
            }
        }
    }

    /// `s.list(2) = []` or `c{1}(2) = []` (cycle 07): a deletion at the end
    /// of a longer chain, in the container the rest of the chain names,
    /// which must already exist.
    fn delete_at(&mut self, name: &str, chain: &[Access]) -> R<()> {
        let links = self.resolve_links(name, chain)?;
        let Some((Link::Paren(sel), path)) = links.split_last() else {
            bail!(error::invalid_assignment_target());
        };
        let v = self
            .vars_mut()
            .get_mut(name)
            .ok_or_else(|| error::undefined(name))?;
        delete_in(nav_mut(v, path)?, sel)
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

    /// The single value an expression wants from a call.
    fn call_for_value(&mut self, name: &str, args: Vec<Value>) -> R<Value> {
        self.call_function(name, args, 1)?
            .into_iter()
            .next()
            .ok_or_else(error::too_many_outputs)
    }

    // ---- functions (cycle 05) ------------------------------------------

    /// Calls the function `name`, which is not a variable, asking it for
    /// `nargout` values. **Invariant 4**, after the variable its callers
    /// have already ruled out: the running file's local functions, then the
    /// script's, then a file on the path, then a builtin. A user file
    /// therefore shadows a builtin of its name. `feval` comes in here too.
    pub(crate) fn call_function(
        &mut self,
        name: &str,
        args: Vec<Value>,
        nargout: usize,
    ) -> R<Vec<Value>> {
        if let Some((unit, f)) = self.local_function(name) {
            return self.call_user(unit, f, name, args, nargout);
        }
        self.call_global(name, args, nargout)
    }

    /// Invariant 4 past the local functions: a file on the path, then a
    /// builtin. What a named handle with no local binding calls (cycle 06).
    fn call_global(&mut self, name: &str, args: Vec<Value>, nargout: usize) -> R<Vec<Value>> {
        if let Some(path) = self.find_file(name) {
            let unit = self.load(&path, name)?;
            return match unit.entry.clone() {
                Some(f) => self.call_user(unit, f, name, args, nargout),
                None => self.run_script(unit, name, args, nargout),
            };
        }
        self.call_builtin(name, args, nargout)
    }

    /// A builtin's call back into the interpreter, counted as one level of
    /// the nesting budget that every frame shares. **Every builtin that
    /// calls a function goes through here**: `feval` of a name or a handle,
    /// and `arrayfun` (cycle 06). Without it a chain of such calls re-entered
    /// the evaluator with nothing counting, and 499 frames each running a
    /// 600-deep `feval` chain overflowed the stack (cycle 05's review): the
    /// one kind of recursion invariant 6 had not yet bounded.
    pub(crate) fn call_nested(
        &mut self,
        callee: Callee,
        args: Vec<Value>,
        nargout: usize,
    ) -> R<Vec<Value>> {
        self.deepen()?;
        let r = match callee {
            Callee::Name(name) => self.call_function(name, args, nargout),
            Callee::Handle(f) => self.call_handle(f, args, nargout),
        };
        self.depth -= 1;
        r
    }

    /// Calls a function handle, asking it for `nargout` values (cycle 06).
    ///
    /// A named handle bound to a local function calls it, wherever it is
    /// called from; one with no binding resolves its name against the path
    /// and the builtins now. An anonymous function runs in a frame of its
    /// own; see [`Interp::call_anon`].
    pub(crate) fn call_handle(
        &mut self,
        f: &Func,
        args: Vec<Value>,
        nargout: usize,
    ) -> R<Vec<Value>> {
        match f {
            Func::Named {
                name,
                local: Some((unit, func)),
            } => self.call_user(unit.clone(), func.clone(), name, args, nargout),
            Func::Named { name, local: None } => self.call_global(name, args, nargout),
            Func::Anon {
                def,
                captured,
                unit,
            } => self.call_anon(f, def, captured, unit, args, nargout),
        }
    }

    /// The single value an expression wants from a handle call.
    fn call_handle_for_value(&mut self, f: &Func, args: Vec<Value>) -> R<Value> {
        self.call_handle(f, args, 1)?
            .into_iter()
            .next()
            .ok_or_else(error::too_many_outputs)
    }

    /// Runs an anonymous function in a frame of its own (cycle 06).
    ///
    /// More arguments than parameters is refused before anything runs, as
    /// for a user function, and so carries no trace entry. The frame holds
    /// the captured variables and then the parameters, and has its own
    /// `end` stack; its code resolves functions against the file the handle
    /// was made in. It counts against the recursion limit like any call.
    /// The body is asked for `nargout` values: a body that is one call
    /// passes the request on, and any other body is its one value. An error
    /// leaving the body gains a trace entry named by the function's
    /// `func2str` text.
    fn call_anon(
        &mut self,
        f: &Func,
        def: &AnonFn,
        captured: &[(String, Value)],
        unit: &Rc<Unit>,
        args: Vec<Value>,
        nargout: usize,
    ) -> R<Vec<Value>> {
        // A last parameter `varargin` takes the rest, as a function's does
        // (cycle 07).
        let var_in = def.params.last().is_some_and(|p| p == "varargin");
        if !var_in && args.len() > def.params.len() {
            bail!(error::too_many_args());
        }
        self.enter()?;
        let mut frame = Frame::new(unit.clone(), None);
        frame.nargin = args.len();
        frame.nargout = nargout;
        for (n, v) in captured {
            frame.vars.insert(n.clone(), v.clone());
        }
        let named = def.params.len() - var_in as usize;
        let mut args = args.into_iter();
        for (p, v) in def.params[..named].iter().zip(args.by_ref()) {
            if p != "~" {
                frame.vars.insert(p.clone(), v);
            }
        }
        if var_in {
            let rest = CellArray::row(args.collect());
            frame.vars.insert("varargin".to_string(), Value::cell(rest));
        }
        self.frames.push(frame);
        let result = self.eval_request(&def.body, nargout);
        self.frames.pop();
        self.calls -= 1;
        let (values, _) = result.map_err(|e| e.leaving_file(&f.text(), &unit.file))?;
        Ok(values)
    }

    /// `str2func(text)` (cycle 06): a text that starts with `@` is parsed
    /// as one handle form with the ordinary parser, counting its nesting
    /// from where the evaluator is, as a file parsed at a call does; any
    /// other text is a function name. The handle is made here, so a name
    /// binds as `@name` written here would, and an anonymous function
    /// captures nothing, since `str2func` has no access to the workspace
    /// it is called from. A parse error carries no line of its own, since
    /// the line would be the text's and not the program's.
    pub(crate) fn str2func(&mut self, text: &str) -> R<Value> {
        let src = text.trim();
        if !src.starts_with('@') {
            return Ok(self.named_handle(src));
        }
        let unlined = |mut e: error::MError| {
            e.line = None;
            e
        };
        let lexed = scan(src).map_err(unlined)?;
        let e = Parser::with_lines(lexed)
            .at_depth(self.depth)
            .parse_handle()
            .map_err(unlined)?;
        // `parse_handle` returns nothing else; the last arm is never taken.
        match e {
            Expr::AnonFn(def) => Ok(self.anon_handle(&def, false)),
            Expr::FuncHandle(name) => Ok(self.named_handle(&name)),
            _ => Err(error::arg_not_a_handle(1, "str2func")),
        }
    }

    /// True when a call to `name` reaches the builtin of that name: no local
    /// function and no file on the path shadows it.
    pub(crate) fn reaches_builtin(&mut self, name: &str) -> bool {
        self.local_function(name).is_none() && self.find_file(name).is_none()
    }

    /// `name` among the running file's local functions, then among the
    /// script's, with the file it belongs to.
    fn local_function(&self, name: &str) -> Option<(Rc<Unit>, Rc<Function>)> {
        [&self.frame().unit, &self.script]
            .into_iter()
            .find_map(|u| u.functions.get(name).map(|f| (u.clone(), f.clone())))
    }

    /// Counts one more running call, refusing the one past the limit.
    fn enter(&mut self) -> R<()> {
        if self.calls >= MAX_RECURSION {
            bail!(error::recursion_limit(MAX_RECURSION));
        }
        self.calls += 1;
        Ok(())
    }

    /// Runs the user function `f` from `unit` in a frame of its own.
    ///
    /// The checks a call fails before it starts come first, and so carry no
    /// trace: more arguments than parameters, more outputs than the function
    /// has, and the recursion limit. The body then runs with its parameters
    /// bound, its own `end` stack and no loop around it, so a `break` in it
    /// cannot reach a loop in the caller. An error leaving the body gains a
    /// trace entry, `name` at the line it was raised on, and takes the
    /// caller's line next; see [`error::MError::leaving`].
    ///
    /// Asked for `nargout` values, the function returns that many of its
    /// outputs, and one it did not assign is MATLAB's "Output argument ...
    /// not assigned" error. Asked for none, as a statement asks, it returns
    /// its first output if it assigned one, which becomes `ans`.
    ///
    /// A last parameter named `varargin` takes every argument past the
    /// named ones, as a 1xN cell (0x0 when there are none), and a last
    /// output named `varargout` supplies every output past the named ones
    /// from its elements, in order (cycle 07). `nargin` and `nargout` count
    /// every argument and every output asked for, those in `varargin` and
    /// `varargout` included, which is MATLAB's rule inside the function.
    fn call_user(
        &mut self,
        unit: Rc<Unit>,
        f: Rc<Function>,
        name: &str,
        args: Vec<Value>,
        nargout: usize,
    ) -> R<Vec<Value>> {
        let var_in = f.params.last().is_some_and(|p| p == "varargin");
        let var_out = f.outputs.last().is_some_and(|o| o == "varargout");
        let named_in = f.params.len() - var_in as usize;
        let named_out = f.outputs.len() - var_out as usize;
        if !var_in && args.len() > f.params.len() {
            bail!(error::too_many_args());
        }
        if !var_out && nargout > f.outputs.len() {
            bail!(error::too_many_outputs());
        }
        self.enter()?;
        let file = unit.file.clone();
        let mut frame = Frame::new(unit, Some(name.to_string()));
        frame.nargin = args.len();
        frame.nargout = nargout;
        let mut args = args.into_iter();
        for (p, v) in f.params[..named_in].iter().zip(args.by_ref()) {
            if p != "~" {
                frame.vars.insert(p.clone(), v);
            }
        }
        if var_in {
            let rest = CellArray::row(args.collect());
            frame.vars.insert("varargin".to_string(), Value::cell(rest));
        }
        self.frames.push(frame);
        let loop_depth = std::mem::take(&mut self.loop_depth);
        let result = self.exec_block(&f.body);
        self.loop_depth = loop_depth;
        let frame = self.frames.pop().expect("the frame pushed above");
        self.calls -= 1;
        result.map_err(|e| e.leaving_file(name, &file))?;
        let mut vars = frame.vars;
        let mut out = Vec::new();
        for (k, o) in f.outputs[..named_out]
            .iter()
            .enumerate()
            .take(nargout.max(1))
        {
            match vars.remove(o) {
                Some(v) => out.push(v),
                None if k < nargout => bail!(error::output_not_assigned(o, name)),
                None => {}
            }
        }
        // The outputs past the named ones, from `varargout`; asked for none,
        // a function whose only output is `varargout` gives its first
        // element if it has one, which becomes `ans`.
        let want = nargout.max(1);
        if var_out && out.len() == named_out && want > named_out {
            let extra: Vec<Value> = match vars.remove("varargout") {
                Some(Value::Cell(c)) => c.data.clone(),
                Some(_) => bail!(error::varargout_not_a_cell()),
                None => Vec::new(),
            };
            let mut extra = extra.into_iter();
            for k in named_out..want {
                match extra.next() {
                    Some(v) => out.push(v),
                    None if k < nargout => {
                        bail!(error::varargout_not_assigned(k - named_out + 1, name))
                    }
                    None => {}
                }
            }
        }
        Ok(out)
    }

    /// Runs a script file from the path in the caller's workspace, as
    /// MATLAB does: its assignments are the caller's variables. A script
    /// takes no inputs and gives no outputs, so either is the call's own
    /// error. It counts against the recursion limit like a function, and an
    /// error leaving it gains a trace entry, so that the line reported at the
    /// top is never a line of the script file.
    fn run_script(
        &mut self,
        unit: Rc<Unit>,
        name: &str,
        args: Vec<Value>,
        nargout: usize,
    ) -> R<Vec<Value>> {
        if !args.is_empty() {
            bail!(error::too_many_args());
        }
        if nargout > 0 {
            bail!(error::too_many_outputs());
        }
        self.enter()?;
        let running = std::mem::replace(&mut self.frame_mut().unit, unit.clone());
        let loop_depth = std::mem::take(&mut self.loop_depth);
        let result = self.exec_block(&unit.stmts);
        self.loop_depth = loop_depth;
        self.frame_mut().unit = running;
        self.calls -= 1;
        result.map_err(|e| e.leaving_file(name, &unit.file))?;
        Ok(Vec::new())
    }

    /// The `.m` file `name` resolves to: `name.m` in the current folder,
    /// then in each folder `addpath` added, in order. A name that is not an
    /// identifier resolves to nothing, so `feval('../x')` cannot reach
    /// outside the path. The answer is kept for the rest of the generation,
    /// which is what keeps a builtin call in a loop from asking the file
    /// system every time.
    fn find_file(&mut self, name: &str) -> Option<PathBuf> {
        if let Some((generation, found)) = self.lookups.get(name) {
            if *generation == self.generation {
                return found.clone();
            }
        }
        let found = if is_identifier(name) {
            let file = format!("{}.m", name);
            std::iter::once(&self.cwd)
                .chain(&self.search_path)
                .map(|dir| dir.join(&file))
                .find(|p| is_exact_file(p))
        } else {
            None
        };
        self.lookups
            .insert(name.to_string(), (self.generation, found.clone()));
        found
    }

    /// The parsed file at `path`, called `name`, from the cache when the
    /// cached copy is known good: read in this generation, or read before
    /// with the modification time and length the file still has. A parse
    /// error in the file is reported with a trace entry for it, since its
    /// line is a line of that file.
    fn load(&mut self, path: &Path, name: &str) -> R<Rc<Unit>> {
        let generation = self.generation;
        let stamp = if let Some(c) = self.files.get_mut(path) {
            if c.generation == generation {
                return Ok(c.unit.clone());
            }
            let stamp = file_stamp(path);
            if stamp.is_some() && stamp == c.stamp {
                c.generation = generation;
                return Ok(c.unit.clone());
            }
            stamp
        } else {
            file_stamp(path)
        };
        let bytes =
            std::fs::read(path).map_err(|e| error::cannot_read(&path.display().to_string(), &e))?;
        let src = String::from_utf8_lossy(&bytes);
        // Parsed on the evaluator's stack, so counted from its depth.
        let depth = self.depth;
        let prog = scan(&src)
            .and_then(|lexed| Parser::with_lines(lexed).at_depth(depth).parse_program())
            .map_err(|e| e.leaving(name))?;
        let mut unit = Unit::from_program(prog);
        unit.file = path.display().to_string();
        let unit = Rc::new(unit);
        self.files.insert(
            path.to_path_buf(),
            CachedFile {
                generation,
                stamp,
                unit: unit.clone(),
            },
        );
        Ok(unit)
    }

    /// `nargin` or `nargout` of the running function; outside every
    /// function, MATLAB's error. A script on the path runs in its caller's
    /// frame, so inside one called from a function these are the
    /// function's.
    pub(crate) fn call_counts(&self) -> R<(usize, usize)> {
        let frame = self.frame();
        match frame.func_name {
            Some(_) => Ok((frame.nargin, frame.nargout)),
            None => Err(error::nargin_outside_function()),
        }
    }

    /// `exist(name)`: `1` for a variable of the running frame, `2` for a
    /// file on the path, `5` for a builtin, `0` otherwise, the values of
    /// the MathWorks `exist` page. A function local to the running file is
    /// none of those and gives `0`; what MATLAB gives there is unverified,
    /// and no case asserts it.
    pub(crate) fn exist(&mut self, name: &str) -> f64 {
        if self.vars().contains_key(name) {
            1.0
        } else if self.find_file(name).is_some() {
            2.0
        } else if self.builtins.contains_key(name) {
            5.0
        } else {
            0.0
        }
    }

    /// A folder as `addpath` and `rmpath` name it, resolved against
    /// [`Interp::cwd`] when it is relative.
    fn resolve_dir(&self, dir: &str) -> PathBuf {
        let p = Path::new(dir);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.cwd.join(p)
        }
    }

    /// `addpath(d1, d2, ...)`: puts the folders at the front of the path in
    /// the order given, moving one already on it rather than listing it
    /// twice. A folder that does not exist is a warning and is left off, as
    /// in MATLAB. Every cached lookup is stale afterwards.
    pub(crate) fn add_path(&mut self, dirs: &[String]) -> R<()> {
        let mut front = Vec::new();
        for d in dirs {
            let p = self.resolve_dir(d);
            if p.is_dir() {
                self.search_path.retain(|q| *q != p);
                front.retain(|q| *q != p);
                front.push(p);
            } else {
                self.emit_err(&error::warning_line(&error::addpath_not_a_folder(d)))?;
            }
        }
        front.append(&mut self.search_path);
        self.search_path = front;
        self.generation += 1;
        Ok(())
    }

    /// `rmpath(d1, ...)`: takes the folders off the path. One that is not on
    /// it is a warning. Every cached lookup is stale afterwards.
    pub(crate) fn remove_path(&mut self, dirs: &[String]) -> R<()> {
        for d in dirs {
            let p = self.resolve_dir(d);
            if self.search_path.contains(&p) {
                self.search_path.retain(|q| *q != p);
            } else {
                self.emit_err(&error::warning_line(&error::rmpath_not_on_path(d)))?;
            }
        }
        self.generation += 1;
        Ok(())
    }
}

/// A file's modification time and length, `None` if either is unknown.
fn file_stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// True when `p` is a file whose name is spelled exactly as asked.
///
/// Windows and macOS match file names without regard to case, so `is_file`
/// alone let `ADDONE(1)` find `addone.m` there and not on Linux: resolution
/// depended on the platform (cycle 05's review). MATLAB's names are
/// case-sensitive, so the directory listing must hold the exact name.
fn is_exact_file(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    let (Some(dir), Some(name)) = (p.parent(), p.file_name()) else {
        return false;
    };
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|e| e.file_name() == name)
    })
}

/// True for a MATLAB identifier: a letter, then letters, digits and `_`.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
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

/// `e.message`, `e.identifier` and, since cycle 07, `e.stack` of an
/// `MException`. Any other name is the Dot error a matrix gives.
fn exception_field(e: &error::MError, field: &str) -> R<Value> {
    match field {
        "message" => Ok(Value::str(&e.msg)),
        "identifier" => Ok(Value::str(e.identifier())),
        "stack" => Ok(exception_stack(e)),
        _ => Err(error::dot_indexing_unsupported()),
    }
}

/// `e.stack` (cycle 07): an Nx1 struct array with the fields `file`,
/// `name` and `line`, one element per frame the error unwound out of,
/// innermost first, which is cycle 05's trace. `file` is the path of the
/// function's file, and empty for a function local to the code that was
/// run, whose file the interpreter is not told; `line` is `[]` for an
/// anonymous function, which has no line. An error raised outside every
/// function has no frames, and its stack is 0x1.
fn exception_stack(e: &error::MError) -> Value {
    let fields = ["file", "name", "line"].map(String::from).to_vec();
    let elems: Vec<Vec<Value>> = e
        .stack()
        .iter()
        .map(|s| {
            let line = s
                .line
                .map_or_else(blank, |l| Value::Mat(Matrix::scalar(l as f64)));
            vec![Value::str(&s.file), Value::str(&s.name), line]
        })
        .collect();
    Value::strukt(StructArray::new(elems.len(), 1, fields, elems))
}

/// The one value a cs-list must be where one is wanted, or MATLAB's
/// error naming how many it had (cycle 07).
fn one_value(mut vals: Vec<Value>) -> R<Value> {
    if vals.len() == 1 {
        Ok(vals.pop().expect("one value"))
    } else {
        Err(error::cs_list_count(vals.len()))
    }
}

/// Field `f` of every element of a struct array, in column-major order:
/// the cs-list `s.f` gives (cycle 07).
fn struct_field(s: &StructArray, f: &str) -> R<Vec<Value>> {
    let i = s.field_index(f).ok_or_else(|| error::no_such_field(f))?;
    Ok(s.elems.iter().map(|e| e[i].clone()).collect())
}

/// The items at `pos`, zero-based and linear, in that order: a read of a
/// cell's or a struct array's elements, as `gather` is a matrix's.
fn pick<T: Clone>(items: &[T], pos: &[usize]) -> Vec<T> {
    pos.iter().map(|&p| items[p].clone()).collect()
}

/// The one linear position `sel` selects in bounds of a `rows x cols`
/// array, if it selects exactly one.
fn one_position(rows: usize, cols: usize, sel: &[Sel]) -> Option<usize> {
    match resolve_read(rows, cols, sel).ok()?.pos[..] {
        [p] => Some(p),
        _ => None,
    }
}

/// Grows column-major `items` from `rows x cols` to `nr x nc`, keeping
/// every item at its row and column and filling the new ones with `fill`:
/// `scatter`'s growth, for a cell's elements or a struct array's (cycle
/// 07). When the linear positions do not move, the storage is resized in
/// place, which `Vec` amortises.
fn regrid<T>(
    items: &mut Vec<T>,
    (rows, cols): (usize, usize),
    (nr, nc): (usize, usize),
    fill: impl Fn() -> T,
) {
    if (rows, cols) == (nr, nc) {
        return;
    }
    if rows == nr || items.is_empty() || (cols == 1 && nc == 1) {
        items.resize_with(nr * nc, fill);
        return;
    }
    let mut old = std::mem::take(items).into_iter();
    let mut out = Vec::with_capacity(nr * nc);
    for c in 0..nc {
        for r in 0..nr {
            if c < cols && r < rows {
                out.extend(old.next());
            } else {
                out.push(fill());
            }
        }
    }
    *items = out;
}

/// The items at `pos`, which is increasing, moved out of `items`: what a
/// deletion keeps.
fn keep_positions<T>(items: Vec<T>, pos: &[usize]) -> Vec<T> {
    let mut want = pos.iter().peekable();
    let mut out = Vec::with_capacity(pos.len());
    for (k, v) in items.into_iter().enumerate() {
        if want.peek() == Some(&&k) {
            want.next();
            out.push(v);
        }
    }
    out
}

/// Stores `rhs` at the end of `links`, starting from `cur` (cycle 07).
///
/// It recurses once per link; `assign_to` has bounded the chain by
/// [`MAX_DEPTH`]. Whatever the path does not hold yet is created as it is
/// assigned: a field of `[]` makes a struct, a brace of `[]` a cell,
/// `p(2).name` of `[]` a struct array, and a position past the end grows
/// the container with `[]` elements. Nothing is created or grown until the
/// assignment below it has succeeded: a new element or field is built on
/// its own first and only then put in place, and the container `[]`
/// becomes is built beside it, so a failure leaves `cur` as it was.
fn assign_chain(cur: &mut Value, links: &[Link], rhs: Value) -> R<()> {
    let Some((first, rest)) = links.split_first() else {
        *cur = rhs;
        return Ok(());
    };
    if cur.is_blank() {
        let empty = match first {
            Link::Field(_) => Some(StructArray::scalar(Vec::new(), Vec::new())),
            Link::Paren(_) if !rest.is_empty() => Some(StructArray::default()),
            _ => None,
        };
        if let Some(s) = empty {
            let mut v = Value::strukt(s);
            assign_chain(&mut v, links, rhs)?;
            *cur = v;
            return Ok(());
        }
        if let Link::Brace(_) = first {
            let mut v = Value::cell(CellArray::default());
            assign_chain(&mut v, links, rhs)?;
            *cur = v;
            return Ok(());
        }
    }
    match first {
        Link::Field(f) => {
            let Value::Struct(rc) = cur else {
                bail!(error::dot_assign_unsupported());
            };
            match (rc.numel(), rc.field_index(f)) {
                (1, Some(i)) => assign_chain(&mut Rc::make_mut(rc).elems[0][i], rest, rhs),
                (0 | 1, _) => {
                    let mut child = blank();
                    assign_chain(&mut child, rest, rhs)?;
                    let s = Rc::make_mut(rc);
                    if s.elems.is_empty() {
                        (s.rows, s.cols) = (1, 1);
                        s.elems.push(vec![blank(); s.fields.len()]);
                    }
                    let i = s.ensure_field(f);
                    s.elems[0][i] = child;
                    Ok(())
                }
                _ => Err(error::scalar_struct_required()),
            }
        }
        Link::Brace(sel) => {
            let Value::Cell(rc) = cur else {
                bail!(error::brace_assign_unsupported());
            };
            let plan = resolve_write(rc.rows, rc.cols, sel, (1, 1))?;
            let [p] = plan.pos[..] else {
                bail!(error::cs_list_count(plan.pos.len()));
            };
            if (plan.rows, plan.cols) == (rc.rows, rc.cols) {
                return assign_chain(&mut Rc::make_mut(rc).data[p], rest, rhs);
            }
            let mut child = blank();
            assign_chain(&mut child, rest, rhs)?;
            let c = Rc::make_mut(rc);
            regrid(&mut c.data, (c.rows, c.cols), (plan.rows, plan.cols), blank);
            (c.rows, c.cols) = (plan.rows, plan.cols);
            c.data[p] = child;
            Ok(())
        }
        Link::Paren(sel) if rest.is_empty() => assign_paren(cur, sel, rhs),
        // `p(k).name = v`: one element of a struct array, then its field.
        Link::Paren(sel) => {
            let Some((Link::Field(f), rest)) = rest.split_first() else {
                bail!(error::invalid_assignment_target());
            };
            let Value::Struct(rc) = cur else {
                bail!(error::dot_assign_unsupported());
            };
            let plan = resolve_write(rc.rows, rc.cols, sel, (1, 1))?;
            let [p] = plan.pos[..] else {
                bail!(error::cs_list_count(plan.pos.len()));
            };
            let grows = (plan.rows, plan.cols) != (rc.rows, rc.cols);
            match (grows, rc.field_index(f)) {
                (false, Some(i)) => assign_chain(&mut Rc::make_mut(rc).elems[p][i], rest, rhs),
                _ => {
                    let mut child = blank();
                    assign_chain(&mut child, rest, rhs)?;
                    let s = Rc::make_mut(rc);
                    let i = s.ensure_field(f);
                    let nf = s.fields.len();
                    regrid(
                        &mut s.elems,
                        (s.rows, s.cols),
                        (plan.rows, plan.cols),
                        || vec![blank(); nf],
                    );
                    (s.rows, s.cols) = (plan.rows, plan.cols);
                    s.elems[p][i] = child;
                    Ok(())
                }
            }
        }
    }
}

/// `x(sel) = rhs`, the last link of a chain: a matrix into a matrix, a
/// cell into a cell, a struct array into a struct array with the same
/// fields (cycle 07). `[]` takes whichever arrives. Anything else is a
/// conversion MATLAB does not make: `c(2) = 5` of a cell, `x(2) = {1}` of
/// a matrix.
fn assign_paren(cur: &mut Value, sel: &[Sel], rhs: Value) -> R<()> {
    match (cur, rhs) {
        (Value::Mat(m), Value::Mat(r)) => assign_matrix(m, sel, r),
        (cur, rhs @ (Value::Cell(_) | Value::Struct(_))) if cur.is_blank() => {
            let mut v = match &rhs {
                Value::Struct(r) => {
                    Value::strukt(StructArray::new(0, 0, r.fields.clone(), Vec::new()))
                }
                _ => Value::cell(CellArray::default()),
            };
            assign_paren(&mut v, sel, rhs)?;
            *cur = v;
            Ok(())
        }
        (Value::Cell(rc), Value::Cell(r)) => {
            let plan = resolve_write(rc.rows, rc.cols, sel, (r.rows, r.cols))?;
            let c = Rc::make_mut(rc);
            regrid(&mut c.data, (c.rows, c.cols), (plan.rows, plan.cols), blank);
            (c.rows, c.cols) = (plan.rows, plan.cols);
            for (k, &p) in plan.pos.iter().enumerate() {
                c.data[p] = r.data[if r.data.len() == 1 { 0 } else { k }].clone();
            }
            Ok(())
        }
        (Value::Struct(rc), Value::Struct(r)) => {
            if !rc.same_fields(&r) {
                bail!(error::dissimilar_structs());
            }
            let plan = resolve_write(rc.rows, rc.cols, sel, (r.rows, r.cols))?;
            let s = Rc::make_mut(rc);
            let nf = s.fields.len();
            regrid(
                &mut s.elems,
                (s.rows, s.cols),
                (plan.rows, plan.cols),
                || vec![blank(); nf],
            );
            (s.rows, s.cols) = (plan.rows, plan.cols);
            for (k, &p) in plan.pos.iter().enumerate() {
                let vals = s.reordered(&r, if r.numel() == 1 { 0 } else { k });
                s.elems[p] = vals;
            }
            Ok(())
        }
        (Value::Cell(_), r) => Err(error::conversion("cell", r.class_name())),
        (Value::Struct(_), r) => Err(error::conversion("struct", r.class_name())),
        (Value::Mat(m), r @ (Value::Cell(_) | Value::Struct(_))) => {
            Err(error::conversion(m.class.name(), r.class_name()))
        }
        (Value::Mat(_), r) => Err(error::not_an_array(r.class_name())),
        (cur, _) => Err(error::not_an_array(cur.class_name())),
    }
}

/// `m(sel) = rhs` of a matrix, in place.
///
/// Everything that can fail is done first: the class conversion of the
/// right-hand side, the growth and its size check, and the element count.
/// Only then is the storage touched, and it is changed where it lies,
/// never cloned. Growth along the last dimension (a row gaining columns, a
/// column gaining rows, a matrix gaining columns) keeps the column-major
/// layout, so it is a `resize` of the storage, which `Vec` amortises:
/// `z(end+1) = k` in a loop is linear overall, not quadratic.
///
/// The left-hand side keeps its class, so `s(1) = 'X'` of a char stays a
/// char and `y(2) = 'a'` of a double stores `97`. The 0x0 double `[]`, which
/// is also what a variable that does not exist yet starts as, takes the
/// class of what is assigned into it, which is how `s = []; s(1) = 'a'`
/// builds a char.
fn assign_matrix(m: &mut Matrix, sel: &[Sel], rhs: Matrix) -> R<()> {
    let class = if m.class == Class::Double && m.rows == 0 && m.cols == 0 {
        rhs.class
    } else {
        m.class
    };
    let rhs = rhs.to_class(class)?;
    let plan = resolve_write(m.rows, m.cols, sel, (rhs.rows, rhs.cols))?;
    m.class = class;
    scatter(m, &plan, &rhs);
    Ok(())
}

/// `v(sel) = []` of a matrix, a cell or a struct array: what
/// `resolve_delete` keeps, moved into place (cycle 07 added the
/// containers).
fn delete_in(v: &mut Value, sel: &[Sel]) -> R<()> {
    if matches!(v, Value::Exception(_) | Value::Func(_)) {
        bail!(error::not_an_array(v.class_name()));
    }
    let (rows, cols) = v.dims();
    let keep = resolve_delete(rows, cols, sel)?;
    match v {
        Value::Mat(m) => {
            m.data = keep.pos.iter().map(|&p| m.data[p]).collect();
            (m.rows, m.cols) = (keep.rows, keep.cols);
        }
        Value::Cell(rc) => {
            let c = Rc::make_mut(rc);
            c.data = keep_positions(std::mem::take(&mut c.data), &keep.pos);
            (c.rows, c.cols) = (keep.rows, keep.cols);
        }
        Value::Struct(rc) => {
            let s = Rc::make_mut(rc);
            s.elems = keep_positions(std::mem::take(&mut s.elems), &keep.pos);
            (s.rows, s.cols) = (keep.rows, keep.cols);
        }
        Value::Exception(_) | Value::Func(_) => {}
    }
    Ok(())
}

/// The value at the end of `links` from `cur`, which must exist, borrowed
/// for change (cycle 07): where `delete_at` deletes. Walked in a loop, so
/// it needs no bound of its own.
fn nav_mut<'a>(mut cur: &'a mut Value, links: &[Link]) -> R<&'a mut Value> {
    /// Where one step goes, judged on a shared borrow first.
    enum Step {
        Field(usize),
        Item(usize),
        Elem(usize, usize),
    }
    let mut k = 0;
    while k < links.len() {
        let step = match (&*cur, &links[k]) {
            (Value::Struct(s), Link::Field(f)) if s.numel() == 1 => {
                Step::Field(s.field_index(f).ok_or_else(|| error::no_such_field(f))?)
            }
            (Value::Struct(_), Link::Field(_)) => bail!(error::scalar_struct_required()),
            (Value::Cell(c), Link::Brace(sel)) => {
                let g = resolve_read(c.rows, c.cols, sel)?;
                let [p] = g.pos[..] else {
                    bail!(error::cs_list_count(g.pos.len()));
                };
                Step::Item(p)
            }
            (Value::Struct(s), Link::Paren(sel)) => {
                let Some(Link::Field(f)) = links.get(k + 1) else {
                    bail!(error::invalid_assignment_target());
                };
                let g = resolve_read(s.rows, s.cols, sel)?;
                let [p] = g.pos[..] else {
                    bail!(error::cs_list_count(g.pos.len()));
                };
                let i = s.field_index(f).ok_or_else(|| error::no_such_field(f))?;
                k += 1;
                Step::Elem(p, i)
            }
            (_, Link::Brace(_)) => bail!(error::brace_indexing_unsupported()),
            (_, Link::Field(_)) => bail!(error::dot_indexing_unsupported()),
            (_, Link::Paren(_)) => bail!(error::invalid_assignment_target()),
        };
        cur = match (cur, step) {
            (Value::Struct(rc), Step::Field(i)) => &mut Rc::make_mut(rc).elems[0][i],
            (Value::Cell(rc), Step::Item(p)) => &mut Rc::make_mut(rc).data[p],
            (Value::Struct(rc), Step::Elem(p, i)) => &mut Rc::make_mut(rc).elems[p][i],
            // Every step above was judged on this same value.
            _ => bail!(error::invalid_assignment_target()),
        };
        k += 1;
    }
    Ok(cur)
}

/// `setfield(s, 'a', 'b', v)`: `v` stored at the field path, created where
/// it does not exist, as `s.a.b = v` would store it (cycle 07).
pub(crate) fn set_fields(cur: &mut Value, fields: &[String], rhs: Value) -> R<()> {
    if fields.len() > MAX_DEPTH {
        bail!(error::nesting_too_deep(MAX_DEPTH));
    }
    for f in fields {
        if !is_identifier(f) {
            bail!(error::invalid_field_name(f));
        }
    }
    let links: Vec<Link> = fields.iter().cloned().map(Link::Field).collect();
    assign_chain(cur, &links, rhs)
}

/// True for a name MATLAB accepts as a field: an identifier (cycle 07).
pub(crate) fn is_field_name(name: &str) -> bool {
    is_identifier(name)
}

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
fn resolve_write(rows: usize, cols: usize, sel: &[Sel], (rr, rc): (usize, usize)) -> R<Scatter> {
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
        let (rspan, cspan) = (span(&sel[0], rows, rr), span(&sel[1], cols, rc));
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
    if rr * rc != 1 && rr * rc != pos.len() {
        bail!(error::assignment_size(pos.len(), rr * rc));
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

/// `[a, b]` or `[a; b]` where a cell or a struct takes part (cycle 07), or
/// `None` when none does and the operands are the matrices' business.
///
/// With a cell among them the result is a cell: a cell contributes its
/// elements, anything else becomes one element of its own, so `[{1}, 2]`
/// is `{1, 2}`. With a struct, every operand must be a struct array with
/// the same fields, and the result is a struct array. Either way the 0x0
/// double `[]` and every empty operand contribute nothing, as they do to a
/// matrix, and the sizes must agree along the other dimension.
fn concat_containers(vals: &mut Vec<Value>, vertical: bool) -> Option<R<Value>> {
    let join = |parts: Vec<(usize, usize)>| -> R<(usize, usize)> {
        let (r0, c0) = parts[0];
        if vertical {
            if parts.iter().any(|&(_, c)| c != c0) {
                bail!(error::concat_dims());
            }
            Ok((parts.iter().map(|&(r, _)| r).sum(), c0))
        } else {
            if parts.iter().any(|&(r, _)| r != r0) {
                bail!(error::concat_dims());
            }
            Ok((r0, parts.iter().map(|&(_, c)| c).sum()))
        }
    };
    // Column-major items of each part, laid out as the joined array holds
    // them.
    fn lay<T: Clone>(parts: &[(usize, usize, &[T])], vertical: bool, cols: usize) -> Vec<T> {
        if !vertical {
            return parts.iter().flat_map(|p| p.2.iter().cloned()).collect();
        }
        let mut out = Vec::new();
        for c in 0..cols {
            for &(r, _, items) in parts {
                out.extend_from_slice(&items[c * r..(c + 1) * r]);
            }
        }
        out
    }
    if vals.iter().any(|v| matches!(v, Value::Cell(_))) {
        let cells: Vec<Rc<CellArray>> = std::mem::take(vals)
            .into_iter()
            .filter(|v| v.numel() > 0)
            .map(|v| match v {
                Value::Cell(c) => c,
                v => Rc::new(CellArray::new(1, 1, vec![v])),
            })
            .collect();
        if cells.is_empty() {
            return Some(Ok(Value::cell(CellArray::default())));
        }
        let shape = match join(cells.iter().map(|c| (c.rows, c.cols)).collect()) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        let parts: Vec<(usize, usize, &[Value])> = cells
            .iter()
            .map(|c| (c.rows, c.cols, c.data.as_slice()))
            .collect();
        let data = lay(&parts, vertical, shape.1);
        return Some(Ok(Value::cell(CellArray::new(shape.0, shape.1, data))));
    }
    if vals.iter().any(|v| matches!(v, Value::Struct(_))) {
        let mut structs: Vec<Rc<StructArray>> = Vec::new();
        for v in std::mem::take(vals) {
            match v {
                Value::Struct(s) => structs.push(s),
                v if v.is_blank() => {}
                v => return Some(Err(error::conversion("struct", v.class_name()))),
            }
        }
        let first = structs[0].clone();
        if structs.iter().any(|s| !first.same_fields(s)) {
            return Some(Err(error::struct_concat_fields()));
        }
        structs.retain(|s| s.numel() > 0);
        if structs.is_empty() {
            return Some(Ok(Value::Struct(first)));
        }
        let shape = match join(structs.iter().map(|s| (s.rows, s.cols)).collect()) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        // Every part's elements in the first part's field order.
        let reordered: Vec<Vec<Vec<Value>>> = structs
            .iter()
            .map(|s| (0..s.numel()).map(|k| first.reordered(s, k)).collect())
            .collect();
        let parts: Vec<(usize, usize, &[Vec<Value>])> = structs
            .iter()
            .zip(&reordered)
            .map(|(s, e)| (s.rows, s.cols, e.as_slice()))
            .collect();
        let elems = lay(&parts, vertical, shape.1);
        return Some(Ok(Value::strukt(StructArray::new(
            shape.0,
            shape.1,
            first.fields.clone(),
            elems,
        ))));
    }
    None
}

/// Rows of values joined as the bracket `[r1; r2; ...]` would join them:
/// what `cell2mat` does with a cell's rows (cycle 07).
pub(crate) fn concat_rows(rows: Vec<Vec<Value>>) -> R<Value> {
    let joined = rows.into_iter().map(hcat).collect::<R<Vec<Value>>>()?;
    vcat(joined)
}

fn hcat(mut vals: Vec<Value>) -> R<Value> {
    if let Some(r) = concat_containers(&mut vals, false) {
        return r;
    }
    // A handle is one function, not an element (cycle 06): `[f 1]`, and
    // `[f]` too, which is a bracket of one.
    if vals.iter().any(|v| matches!(v, Value::Func(_))) {
        bail!(error::handle_concatenation());
    }
    let all: Vec<Matrix> = vals.into_iter().map(Value::into_mat).collect::<R<_>>()?;
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

fn vcat(mut vals: Vec<Value>) -> R<Value> {
    if vals.len() == 1 {
        return Ok(vals.into_iter().next().unwrap());
    }
    if let Some(r) = concat_containers(&mut vals, true) {
        return r;
    }
    let all: Vec<Matrix> = vals.into_iter().map(Value::into_mat).collect::<R<_>>()?;
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
        it.vars()["ans"].mat().unwrap().class
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
        let p = resolve_write(1, 2, &[Sel::row(vec![3])], (rhs.rows, rhs.cols)).unwrap();
        assert_eq!(
            p,
            Scatter {
                rows: 1,
                cols: 4,
                pos: vec![3]
            }
        );
        // A column grows down; an empty becomes a row.
        let p = resolve_write(2, 1, &[Sel::row(vec![2])], (rhs.rows, rhs.cols)).unwrap();
        assert_eq!((p.rows, p.cols), (3, 1));
        let p = resolve_write(0, 0, &[Sel::row(vec![2])], (rhs.rows, rhs.cols)).unwrap();
        assert_eq!((p.rows, p.cols), (1, 3));
        // Two subscripts grow either dimension, and positions are in the
        // grown shape.
        let p = resolve_write(
            2,
            2,
            &[Sel::row(vec![2]), Sel::row(vec![2])],
            (rhs.rows, rhs.cols),
        )
        .unwrap();
        assert_eq!(
            p,
            Scatter {
                rows: 3,
                cols: 3,
                pos: vec![8]
            }
        );
        // A matrix cannot grow through one subscript.
        let e = resolve_write(2, 2, &[Sel::row(vec![4])], (rhs.rows, rhs.cols)).unwrap_err();
        assert_eq!(e.msg, "Attempt to grow array along ambiguous dimension.");
        // The count is checked against the positions.
        let two = Matrix::row(vec![1.0, 2.0]);
        let e = resolve_write(1, 3, &[Sel::row(vec![0, 1, 2])], (two.rows, two.cols)).unwrap_err();
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
        let e = resolve_write(0, 0, &[huge], (1, 1)).unwrap_err();
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
            let x = it.vars()["x"].mat().unwrap();
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
            let p = it.vars()["z"].mat().unwrap().data.as_ptr();
            if p != last {
                reallocations += 1;
                last = p;
            }
        }
        let z = it.vars()["z"].mat().unwrap();
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
        // An assignment says so in MATLAB's assignment form (cycle 07).
        let brace_asg = "Unable to perform assignment because brace indexing is not supported for variables of this type.";
        let dot_asg = "Unable to perform assignment because dot indexing is not supported for variables of this type.";
        assert_eq!(err_msg("x = [1 2]; x{1} = 3;"), brace_asg);
        assert_eq!(err_msg("x = [1 2]; x.a = 3;"), dot_asg);
        assert_eq!(err_msg("x = [1 2]; x(1).a = 3;"), dot_asg);
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
        assert!(!it.vars().contains_key("a") && !it.vars().contains_key("b"));
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

    // ---- switch, try, warnings and commands (cycle 04) -----------------

    #[test]
    fn switch_matches_numbers_by_value_and_text_by_text() {
        let src = "x = 2; switch x, case 1, disp('one'), case {2, 3}, disp('two or three'), \
                   otherwise, disp('other'), end";
        assert_eq!(ok_out(src), "two or three\n");
        assert_eq!(
            ok_out("switch 9, case 1, disp(1), otherwise, disp(0), end"),
            "     0\n"
        );
        // A char never matches a number by its code, either way round.
        assert_eq!(ok_out("switch 'a', case 97, disp(1), end"), "");
        assert_eq!(ok_out("switch 97, case 'a', disp(1), end"), "");
        assert_eq!(
            ok_out("switch 'abc', case 'ab', disp(1), case 'abc', disp(2), end"),
            "     2\n"
        );
        // The class of a number does not matter.
        assert_eq!(ok_out("switch true, case 1, disp(1), end"), "     1\n");
        assert_eq!(
            ok_out("switch 1, case {'1', true}, disp(1), end"),
            "     1\n"
        );
        // An empty char is a character vector, and matches an empty one.
        assert_eq!(ok_out("switch '', case '', disp(1), end"), "     1\n");
        // Only the first matching arm runs, and cases after it are not
        // evaluated at all.
        assert_eq!(
            ok_out("switch 1, case 1, disp(1), case undefined_name, disp(2), end"),
            "     1\n"
        );
    }

    #[test]
    fn switch_refuses_a_subject_that_is_not_a_scalar_or_a_character_vector() {
        const MSG: &str = "SWITCH expression must be a scalar or a character vector.";
        assert_eq!(err_msg("switch [1 2], case 1, end"), MSG);
        assert_eq!(err_msg("switch [], case 1, end"), MSG);
        assert_eq!(err_msg("switch ['ab'; 'cd'], case 1, end"), MSG);
        assert_eq!(err_msg("try, error('x'), catch e, end; switch e, end"), MSG);
        // An error in a case value names the case's line.
        assert_eq!(err("switch 1\ncase 2\ncase nope\nend").line, Some(3));
    }

    #[test]
    fn break_and_continue_pass_through_switch_and_try() {
        assert_eq!(
            ok_out("for k = 1:5, switch k, case 3, break, end, fprintf('%d', k); end"),
            "12"
        );
        assert_eq!(
            ok_out("for k = 1:4, switch k, case 2, continue, end, fprintf('%d', k); end"),
            "134"
        );
        assert_eq!(
            ok_out("for k = 1:4, try, if k == 3, break, end, fprintf('%d', k); catch, end, end"),
            "12"
        );
        assert_eq!(
            ok_out(
                "k = 0; while true, k = k + 1; try, error('x'), catch, break, end, end; disp(k)"
            ),
            "     1\n"
        );
    }

    #[test]
    fn try_catches_every_runtime_error_and_binds_it() {
        assert_eq!(
            ok_out(
                "try, error('boom'), catch e, disp(e.message), disp(isempty(e.identifier)), end"
            ),
            "boom\n   1\n"
        );
        assert_eq!(
            ok_out(
                "try, error('MyPkg:myid', 'Value %d bad', 7), catch e, disp(e.identifier), disp(e.message), end"
            ),
            "MyPkg:myid\nValue 7 bad\n"
        );
        assert_eq!(
            ok_out("try, x = [1 2] * [3 4]; catch, disp('caught'), end"),
            "caught\n"
        );
        assert_eq!(
            ok_out("try, undefined_thing + 1, catch e, disp(e.message), end"),
            "Unrecognized function or variable 'undefined_thing'.\n"
        );
        // A `try` without a `catch` swallows the error; the statements
        // after the failing one do not run.
        assert_eq!(ok_out("try, error('x'), disp(1), end, disp(2)"), "     2\n");
        // Output before the error stays printed.
        assert_eq!(
            ok_out("try, disp(1), error('x'), catch, disp(2), end"),
            "     1\n     2\n"
        );
        // An error in the handler is not caught by its own `try`.
        assert_eq!(err_msg("try, error('a'), catch, error('b'), end"), "b");
    }

    #[test]
    fn an_exception_has_two_fields_a_class_and_a_display() {
        let pre = "try, error('a:b', 'msg'), catch e, end; ";
        assert_eq!(ok_out(&format!("{pre}disp(class(e))")), "MException\n");
        assert_eq!(ok_out(&format!("{pre}disp(e.('message'))")), "msg\n");
        assert_eq!(ok_out(&format!("{pre}disp(e.message(1:2))")), "ms\n");
        assert_eq!(
            ok_out(&format!("{pre}e")),
            "e =\n\n  MException (a:b): msg\n\n"
        );
        assert_eq!(
            ok_out(&format!("{pre}disp(e)")),
            "  MException (a:b): msg\n"
        );
        assert_eq!(
            ok_out("try, error('plain'), catch e, end; e"),
            "e =\n\n  MException: plain\n\n"
        );
        const DOT: &str = "Dot indexing is not supported for variables of this type.";
        assert_eq!(err_msg(&format!("{pre}e.Message")), DOT);
        // Its fields are read, never assigned (cycle 07's assignment text).
        assert_eq!(
            err_msg(&format!("{pre}e.message = 'x'")),
            "Unable to perform assignment because dot indexing is not supported for variables of this type."
        );
        // It is not an array; a binary operator says so in MATLAB's words
        // since cycle 07, and everything else in SplatCrab's.
        const NOT: &str = "This operation is not supported for a value of class 'MException'.";
        assert_eq!(
            err_msg(&format!("{pre}e + 1")),
            "Operator '+' is not supported for operands of type 'MException'."
        );
        assert_eq!(err_msg(&format!("{pre}-e")), NOT);
        assert_eq!(err_msg(&format!("{pre}e(1)")), NOT);
        assert_eq!(err_msg(&format!("{pre}sum(e)")), NOT);
        assert_eq!(err_msg(&format!("{pre}x = [e 1]")), NOT);
    }

    #[test]
    fn rethrow_raises_the_error_unchanged() {
        assert_eq!(
            ok_out(
                "try, try, error('in'), catch e, rethrow(e), end, catch e2, disp(['outer: ' e2.message]), end"
            ),
            "outer: in\n"
        );
        assert_eq!(
            ok_out(
                "try, try, error('p:q', 'm'), catch e, rethrow(e), end, catch f, disp(f.identifier), end"
            ),
            "p:q\n"
        );
        // Its line is the line it was first raised on.
        let e = err("try\n  error('first')\ncatch e\nend\nrethrow(e)");
        assert_eq!((e.msg.as_str(), e.line), ("first", Some(2)));
        assert_eq!(
            err_msg("rethrow(5)"),
            "Undefined function 'rethrow' for input arguments of type 'double'."
        );
    }

    #[test]
    fn lasterr_is_the_last_message_caught_or_not() {
        assert_eq!(ok_out("disp(isempty(lasterr))"), "   1\n");
        assert_eq!(
            ok_out("try, error('one'), catch, end; disp(lasterr)"),
            "one\n"
        );
        let mut it = Interp::with_output(Box::new(io::sink()));
        assert!(it.run("error('two')").is_err());
        assert_eq!(it.last_err, "two");
    }

    #[test]
    fn a_try_that_catches_a_nesting_error_leaves_the_counters_right() {
        on_the_interpreter_stack(|| {
            let mut it = Interp::with_output(Box::new(io::sink()));
            it.run("x = 1;").unwrap();
            let before = (it.depth, it.loop_depth);
            // Built rather than parsed, so the evaluator's limit fires.
            let stmt = Stmt::Try(
                vec![Located {
                    stmt: Stmt::Expr(nested_neg(MAX_DEPTH + 5), false),
                    line: 1,
                }],
                Some("e".into()),
                vec![],
            );
            it.loop_depth = 1;
            assert!(it.exec(&stmt).is_ok());
            assert_eq!(it.depth, before.0);
            assert_eq!(it.loop_depth, 1);
            assert!(matches!(it.vars().get("e"), Some(Value::Exception(e)) if e.msg == TOO_DEEP));
        });
    }

    #[test]
    fn deep_switch_and_try_are_refused_not_overflowed() {
        on_the_interpreter_stack(|| {
            let n = MAX_DEPTH + 1;
            let tries = format!("{}{}", "try\n".repeat(n), "end\n".repeat(n));
            assert_eq!(err_msg(&tries), TOO_DEEP);
            let switches = format!("{}{}", "switch 1\ncase 1\n".repeat(n), "end\n".repeat(n));
            assert_eq!(err_msg(&switches), TOO_DEEP);
            // Well inside the limit, both run.
            let ok = format!("{}disp(1)\n{}", "try\n".repeat(1000), "end\n".repeat(1000));
            assert_eq!(ok_out(&ok), "     1\n");
        });
    }

    /// Runs `src` with both sinks writing into one buffer, as the protocol
    /// captures them.
    fn both(src: &str) -> String {
        let buf = Rc::new(RefCell::new(Vec::new()));
        let mut it =
            Interp::with_sinks(Box::new(Shared(buf.clone())), Box::new(Shared(buf.clone())));
        it.run(src).unwrap();
        String::from_utf8(buf.borrow().clone()).unwrap()
    }

    #[test]
    fn a_warning_goes_to_the_error_sink_in_order() {
        assert_eq!(
            both("disp(1); warning('careful %d', 1); disp(2)"),
            "     1\nWarning: careful 1\n     2\n"
        );
        // Out alone does not see it.
        assert_eq!(ok_out("warning('w')"), "");
        // The same argument rules as `error`: one argument is literal.
        assert_eq!(both("warning('100% sure')"), "Warning: 100% sure\n");
        assert_eq!(both("warning('a:b', 'x %d', 3)"), "Warning: x 3\n");
        assert_eq!(both("warning('')"), "");
    }

    #[test]
    fn command_syntax_calls_the_name_with_char_arguments() {
        assert_eq!(ok_out("disp hello"), "hello\n");
        assert_eq!(ok_out("disp 'two words'"), "two words\n");
        assert_eq!(ok_out("disp -1"), "-1\n");
        assert_eq!(ok_out("x = 3; x -1"), "ans =\n\n     2\n\n");
        assert_eq!(ok_out("class hello"), "ans =\n\n    'char'\n\n");
        assert_eq!(ok_out("class hello;"), "");
        assert_eq!(
            err_msg("hold on"),
            "Unrecognized function or variable 'hold'."
        );
        assert_eq!(
            err_msg("x = 1; clear x\nx"),
            "Unrecognized function or variable 'x'."
        );
    }

    #[test]
    fn clear_all_and_clear_of_names() {
        let mut it = Interp::with_output(Box::new(io::sink()));
        it.run("x = 1; y = 2; z = 3;").unwrap();
        it.run("clear x y").unwrap();
        let mut names: Vec<&String> = it.vars().keys().collect();
        names.sort();
        assert_eq!(names, ["z"]);
        it.run("a = 1; clear all").unwrap();
        assert!(it.vars().is_empty());
        // A variable known from an earlier entry is an expression, not a
        // command.
        it.run("x = 3;").unwrap();
        let buf = Rc::new(RefCell::new(Vec::new()));
        it.out = Box::new(Shared(buf.clone()));
        it.run("x -1").unwrap();
        assert_eq!(
            String::from_utf8(buf.borrow().clone()).unwrap(),
            "ans =\n\n     2\n\n"
        );
    }

    #[test]
    fn a_block_comment_is_not_run() {
        assert_eq!(ok_out("%{\ndisp(111)\n%}\ndisp(1)"), "     1\n");
        assert_eq!(ok_out("disp(1)\n%{\ndisp(2)"), "     1\n");
    }

    // ---- functions and scoping (cycle 05) -------------------------------

    /// A fresh directory under the system's temporary one, removed when
    /// dropped, for the tests that put function files on disk.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> TempDir {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "splatcrab-{}-{}-{}",
                tag,
                std::process::id(),
                n
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("temp dir");
            TempDir(dir)
        }

        fn write(&self, rel: &str, text: &str) {
            let p = self.0.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).expect("temp subdir");
            }
            std::fs::write(p, text).expect("temp file");
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An interpreter whose working directory is `dir`, and its output.
    fn in_dir(dir: &TempDir) -> (Interp, Rc<RefCell<Vec<u8>>>) {
        let buf = Rc::new(RefCell::new(Vec::new()));
        let mut it = Interp::with_sinks(Box::new(Shared(buf.clone())), Box::new(io::sink()));
        it.cwd = dir.0.clone();
        (it, buf)
    }

    fn take(buf: &Rc<RefCell<Vec<u8>>>) -> String {
        String::from_utf8(std::mem::take(&mut *buf.borrow_mut())).unwrap()
    }

    #[test]
    fn a_local_function_is_called_with_its_outputs_and_nargin() {
        assert_eq!(
            ok_out("disp(sq(4))\nfunction y = sq(x)\n    y = x^2;\nend"),
            "    16\n"
        );
        assert_eq!(
            ok_out(
                "[s, p] = sp(2, 3);\ndisp([s p])\nfunction [s, p] = sp(a, b)\ns = a + b; p = a * b;\nend"
            ),
            "     5     6\n"
        );
        assert_eq!(
            ok_out(
                "disp(f(1)); disp(f(1, 2))\nfunction r = f(a, b)\nif nargin < 2, b = 10; end\nr = a + b;\nend"
            ),
            "    11\n     3\n"
        );
        // nargout: 1 in an expression, 0 as a statement, 2 for two targets.
        assert_eq!(
            ok_out(
                "disp(h()); h\n[a, b] = h(); disp(a)\nfunction [r, q] = h()\nr = nargout; q = 0;\nend"
            ),
            "     1\nans =\n\n     0\n\n     2\n"
        );
        // A statement asks for nothing, so an unassigned output is no error.
        assert_eq!(ok_out("g()\nfunction y = g()\nend"), "");
    }

    #[test]
    fn return_leaves_the_function_from_inside_a_loop() {
        assert_eq!(
            ok_out(
                "disp(early(5)); disp(early(-5))\nfunction r = early(x)\nr = 0; if x > 0, r = 1; return; end\nr = -1;\nend"
            ),
            "     1\n    -1\n"
        );
        assert_eq!(
            ok_out(
                "disp(first(7))\nfunction k = first(n)\nfor k = 1:10\nwhile true\nif k == n, return; end\nbreak\nend\nend\nk = 0;\nend"
            ),
            "     7\n"
        );
        // At the top it ends the script.
        assert_eq!(ok_out("disp(1)\nreturn\ndisp(2)"), "     1\n");
    }

    /// A function sees only its own variables, and `break` in it never
    /// reaches a loop in its caller.
    #[test]
    fn a_frame_isolates_variables() {
        assert_eq!(
            ok_out("x = 1; g2(); disp(x)\nfunction g2()\nx = 99;\nend"),
            "     1\n"
        );
        let e = err("x = 1; g3()\nfunction g3()\ndisp(x)\nend");
        assert_eq!(e.msg, "Unrecognized function or variable 'x'.");
        assert_eq!(e.line, Some(1));
        let mut it = Interp::with_output(Box::new(io::sink()));
        it.run("a = 1; f(2)\nfunction f(b)\nc = b;\nend").unwrap();
        let mut names: Vec<&String> = it.vars().keys().collect();
        names.sort();
        assert_eq!(names, ["a"]);
        assert_eq!(
            err_msg("for k = 1:2\nf()\nend\nfunction f()\nbreak\nend"),
            "'break' is only valid inside a loop."
        );
    }

    /// Invariant 3: `end` is the running frame's. In `x(f(end))` it is
    /// `x`'s, and inside `f` it is only ever what `f` itself indexes.
    #[test]
    fn a_frame_isolates_end() {
        assert_eq!(
            ok_out(
                "x = [10 20 30 40];\ndisp(x(f(end)))\nfunction r = f(n)\nv = [1 2];\nr = v(end) + n - 3;\nend"
            ),
            "    30\n"
        );
        // A function with no index of its own cannot see the caller's.
        let e = err("x = [1 2 3];\ny = x(g())\nfunction r = g()\nr = end;\nend");
        assert!(e.msg.contains("end"), "{}", e.msg);
    }

    #[test]
    fn the_argument_and_output_checks() {
        let e = err("sq(1, 2)\nfunction y = sq(x)\ny = x;\nend");
        assert_eq!(
            (e.msg.as_str(), e.stack().len()),
            ("Too many input arguments.", 0)
        );
        let e = err("z = bad(1)\nfunction y = bad(x)\nend");
        assert_eq!(
            e.msg,
            "Output argument \"y\" (and maybe others) not assigned during call to \"bad\"."
        );
        assert!(e.stack().is_empty());
        assert_eq!(
            err_msg("z = g()\nfunction g()\nend"),
            "Too many output arguments."
        );
        assert_eq!(
            err_msg("[a, b] = f()\nfunction a = f()\na = 1;\nend"),
            "Too many output arguments."
        );
        assert_eq!(
            err_msg("x = nargin"),
            "You can only call nargin/nargout from within a MATLAB function."
        );
    }

    /// The trace: one entry per function the error left, innermost first,
    /// and the error's own line is the calling script's.
    #[test]
    fn an_error_in_a_function_carries_a_trace() {
        let e = err(
            "x = 1;\nouter()\nfunction outer()\ninner();\nend\nfunction inner()\nerror('boom');\nend",
        );
        assert_eq!(e.msg, "boom");
        assert_eq!(e.line, Some(2));
        assert_eq!(e.trace(), "  in inner (line 7)\n  in outer (line 4)\n");
        // A caught error keeps its trace, and the script goes on.
        assert_eq!(
            ok_out("try\nf()\ncatch e\ndisp(e.message)\nend\nfunction f()\nerror('x');\nend"),
            "x\n"
        );
    }

    /// The recursion limit is a clean error at 501 frames. It is run on the
    /// interpreter's own stack, as the nesting limit's tests are.
    #[test]
    fn the_recursion_limit_is_a_clean_error() {
        on_the_interpreter_stack(|| {
            let e = err("inf_rec(1)\nfunction r = inf_rec(n)\nr = inf_rec(n + 1);\nend");
            assert_eq!(e.msg, "Maximum recursion limit of 500 reached.");
            assert_eq!(e.stack().len(), MAX_RECURSION);
            // Exactly 500 frames are allowed.
            let depth = |n: usize| {
                format!(
                    "disp(d({}))\nfunction r = d(n)\nif n <= 1, r = 1; else, r = 1 + d(n - 1); end\nend",
                    n
                )
            };
            assert_eq!(ok_out(&depth(MAX_RECURSION)), "   500\n");
            assert_eq!(
                err_msg(&depth(MAX_RECURSION + 1)),
                "Maximum recursion limit of 500 reached."
            );
            // Recovered from, the interpreter is back at its base frame.
            let mut it = Interp::with_output(Box::new(io::sink()));
            it.run("try\nf(1)\ncatch\nend\nx = 1;\nfunction f(n)\nf(n + 1);\nend")
                .unwrap();
            assert_eq!(it.frames.len(), 1);
            assert!(it.vars().contains_key("x"));
        });
    }

    /// Invariant 6 against the frames: the nesting counter is shared by
    /// every frame, so 500 frames each deep in an expression reach one
    /// clean error or the other, never the end of the stack.
    #[test]
    fn deep_frames_stay_within_the_stack() {
        on_the_interpreter_stack(|| {
            for wrap in ["-(", "abs(", "1+("] {
                for k in [5, 18, 1000] {
                    let src = format!(
                        "r = f(1);\nfunction r = f(n)\nr = {}f(n + 1){};\nend",
                        wrap.repeat(k),
                        ")".repeat(k)
                    );
                    let msg = err_msg(&src);
                    assert!(
                        msg == "Maximum recursion limit of 500 reached." || msg == TOO_DEEP,
                        "{wrap} {k}: {msg}"
                    );
                }
            }
        });
    }

    /// The REPL, the protocol and the page refuse a definition before
    /// running anything.
    #[test]
    fn a_command_entry_refuses_a_function() {
        let mut it = Interp::with_output(Box::new(io::sink()));
        let e = it.run_command("x = 1;\nfunction f()\nend").unwrap_err();
        assert_eq!(
            (e.msg.as_str(), e.line),
            (
                "Function definitions are not supported in this context.",
                Some(2)
            )
        );
        assert!(it.vars().is_empty());
    }

    /// Invariant 4: variable, the running file's local functions, the
    /// script's, a file on the path, a builtin.
    #[test]
    fn names_resolve_in_order() {
        let dir = TempDir::new("resolve");
        dir.write("max.m", "function m = max(x)\nm = 42;\nend\n");
        dir.write("sq.m", "function y = sq(x)\ny = -1;\nend\n");
        dir.write(
            "helper.m",
            "function y = helper(x)\ny = [sq(x) twice(x)];\n\nfunction z = sq(x)\nz = 100;\n",
        );
        let (mut it, buf) = in_dir(&dir);
        // A path file shadows a builtin; a variable shadows both.
        it.run("disp(max([1 5 2]))").unwrap();
        assert_eq!(take(&buf), "    42\n");
        it.run("max = [7 8]; disp(max(2))").unwrap();
        assert_eq!(take(&buf), "     8\n");
        it.run("clear max").unwrap();
        // A script's local function shadows a path file of its name.
        it.run("disp(sq(3))\nfunction y = sq(x)\ny = x^2;\nend")
            .unwrap();
        assert_eq!(take(&buf), "     9\n");
        // Inside helper.m, its own `sq` comes first, then the script's
        // `twice`: the running file, then the script.
        it.run("disp(helper(3))\nfunction z = twice(x)\nz = 2 * x;\nend")
            .unwrap();
        assert_eq!(take(&buf), "   100     6\n");
        // Without the script, `twice` is nowhere.
        let e = it.run("helper(3)").unwrap_err();
        assert_eq!(e.msg, "Unrecognized function or variable 'twice'.");
        assert_eq!(e.trace(), "  in helper (line 2)\n");
        // A subfunction is private to its file.
        dir.write(
            "helper2.m",
            "function y = helper2(x)\ny = x;\nfunction z = inner(x)\nz = x;\n",
        );
        assert_eq!(
            it.run("helper2(1); inner(1)").unwrap_err().msg,
            "Unrecognized function or variable 'inner'."
        );
    }

    #[test]
    fn a_script_on_the_path_runs_in_the_callers_workspace() {
        let dir = TempDir::new("script");
        dir.write("setup.m", "a = 7;\nif a > 0, return; end\na = 0;\n");
        let (mut it, buf) = in_dir(&dir);
        it.run("setup; disp(a)").unwrap();
        assert_eq!(take(&buf), "     7\n");
        // Called from a function, it fills the function's workspace.
        it.run("clear all\ndisp(f())\ndisp(exist('a'))\nfunction r = f()\nsetup\nr = a;\nend")
            .unwrap();
        assert_eq!(take(&buf), "     7\n     0\n");
        assert_eq!(
            it.run("setup(1)").unwrap_err().msg,
            "Too many input arguments."
        );
        assert_eq!(
            it.run("x = setup").unwrap_err().msg,
            "Too many output arguments."
        );
    }

    #[test]
    fn exist_and_feval() {
        let dir = TempDir::new("exist");
        dir.write("addone.m", "function y = addone(x)\ny = x + 1;\nend\n");
        let (mut it, buf) = in_dir(&dir);
        it.run("x = 1; disp([exist('x') exist('addone') exist('max') exist('nosuch')])")
            .unwrap();
        assert_eq!(take(&buf), "     1     2     5     0\n");
        it.run("disp(feval('addone', 41)); disp(feval('max', [1 3 2]))")
            .unwrap();
        assert_eq!(take(&buf), "    42\n     3\n");
        // feval skips variables, as a string names a function.
        it.run("addone = 5; disp(feval('addone', 1))").unwrap();
        assert_eq!(take(&buf), "     2\n");
        it.run("[m, k] = feval('max', [1 3 2]); disp([m k])")
            .unwrap();
        assert_eq!(take(&buf), "     3     2\n");
        // Not an identifier, never a path.
        it.run("disp(exist('../addone'))").unwrap();
        assert_eq!(take(&buf), "     0\n");
    }

    /// `addpath` and `rmpath` bump the generation, so a lookup cached
    /// before them is never used after.
    #[test]
    fn the_cache_generation() {
        let dir = TempDir::new("cache");
        dir.write("a/f.m", "function r = f()\nr = 1;\nend\n");
        dir.write("b/f.m", "function r = f()\nr = 2;\nend\n");
        let (mut it, buf) = in_dir(&dir);
        let g0 = it.generation;
        it.run("addpath('a'); disp(f()); addpath('b'); disp(f()); rmpath('b'); disp(f())")
            .unwrap();
        assert_eq!(take(&buf), "     1\n     2\n     1\n");
        // One for the run, one for each change of the path.
        assert_eq!(it.generation, g0 + 4);
        assert_eq!(it.search_path, vec![dir.0.join("a")]);
        // Adding a folder already on the path moves it, never lists it twice.
        it.run("addpath('b', 'a'); addpath('a')").unwrap();
        assert_eq!(it.search_path, vec![dir.0.join("a"), dir.0.join("b")]);
        it.run("rmpath('a', 'b')").unwrap();
        assert!(it.search_path.is_empty());
        // A file changed between entries is read again; within one it is not.
        it.run("addpath('a'); disp(f())").unwrap();
        assert_eq!(take(&buf), "     1\n");
        dir.write("a/f.m", "function r = f()\nr = 33;\nend\n");
        it.run("disp(f())").unwrap();
        assert_eq!(take(&buf), "    33\n");
        // A cached file is known good for the rest of its generation.
        let path = dir.0.join("a").join("f.m");
        assert_eq!(it.files[&path].generation, it.generation);
        // The name cache forgets a file that has gone.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            it.run("f()").unwrap_err().msg,
            "Unrecognized function or variable 'f'."
        );
    }

    #[test]
    fn addpath_and_rmpath_warn_for_a_folder_they_cannot_use() {
        let dir = TempDir::new("warn");
        let buf = Rc::new(RefCell::new(Vec::new()));
        let mut it = Interp::with_sinks(Box::new(io::sink()), Box::new(Shared(buf.clone())));
        it.cwd = dir.0.clone();
        it.run("addpath('nothere'); rmpath('nothere')").unwrap();
        assert_eq!(
            take(&buf),
            "Warning: Name is nonexistent or not a directory: nothere\n\
             Warning: \"nothere\" not found in path.\n"
        );
    }

    /// A parse error in a file on the path names that file's line in the
    /// trace, and the calling line at the top.
    #[test]
    fn a_broken_file_on_the_path_is_a_clean_error() {
        let dir = TempDir::new("broken");
        dir.write("broken.m", "function y = broken()\ny = (1;\nend\n");
        let (mut it, _) = in_dir(&dir);
        let e = it.run("x = 1;\ny = broken()").unwrap_err();
        assert_eq!(e.line, Some(2));
        assert_eq!(e.trace(), "  in broken (line 2)\n");
    }

    /// A file parsed while the evaluator is already deep counts its nesting
    /// from there, so the two together stay under the one limit.
    #[test]
    fn a_file_parsed_deep_shares_the_nesting_budget() {
        on_the_interpreter_stack(|| {
            let dir = TempDir::new("deepparse");
            let body = format!("{}1{}", "(".repeat(6000), ")".repeat(6000));
            dir.write(
                "deepf.m",
                &format!("function y = deepf()\ny = {};\nend\n", body),
            );
            let (mut it, _) = in_dir(&dir);
            // Alone, the file parses and runs.
            it.run("x = deepf();").unwrap();
            // From inside 6,000 levels of calls it does not: the parser
            // starts where the evaluator is.
            let dir2 = TempDir::new("deepparse2");
            dir2.write(
                "deepf.m",
                &format!("function y = deepf()\ny = {};\nend\n", body),
            );
            let (mut it, _) = in_dir(&dir2);
            let src = format!("x = {}deepf(){};", "abs(".repeat(6000), ")".repeat(6000));
            let e = it.run(&src).unwrap_err();
            assert_eq!(e.msg, TOO_DEEP);
            assert_eq!(e.stack().len(), 1);
        });
    }

    /// Command syntax inside a function: its parameters are variables, and
    /// the script's variables are not.
    #[test]
    fn command_syntax_knows_a_functions_parameters() {
        assert_eq!(
            ok_out("disp(f(5))\nfunction r = f(a)\nr = a -1;\nend"),
            "     4\n"
        );
    }

    // ---- function handles (cycle 06) -----------------------------------

    #[test]
    fn a_handle_is_called_through_its_variable() {
        assert_eq!(
            ok_out(
                "f = @(x) x.^2; disp(f(4)); g = @(x, y) x + y; disp(g(1, 2)); z = @() 42; disp(z())"
            ),
            "    16\n     3\n    42\n"
        );
        // A bare handle name is the handle, not a call.
        assert_eq!(ok_out("z = @() 42; w = z; disp(w())"), "    42\n");
        // A handle a call returned is called by the next link.
        assert_eq!(ok_out("add = @(a) @(b) a + b; disp(add(3)(4))"), "     7\n");
        // At statement level the value becomes `ans`.
        assert_eq!(ok_out("f = @(x) x + 1; f(2)"), "ans =\n\n     3\n\n");
        assert_eq!(err_msg("f = @(x) x; f(1, 2)"), "Too many input arguments.");
        // Fewer arguments than parameters is fine until one is read.
        assert_eq!(ok_out("f = @(x, y) x; disp(f(5))"), "     5\n");
    }

    /// Capture happens when the handle is made: a snapshot of each name the
    /// body reads that is a variable then. A name that is not one then is
    /// a function when the body runs, never the caller's variable.
    #[test]
    fn capture_is_a_snapshot_at_creation() {
        assert_eq!(
            ok_out("a = 10; f = @(x) x + a; a = 0; disp(f(1))"),
            "    11\n"
        );
        assert_eq!(
            ok_out("v = [1 2 3]; f = @(k) v(k); v(2) = 50; disp(f(2))"),
            "     2\n"
        );
        assert_eq!(
            ok_out("add = @(a) @(b) a + b; add3 = add(3); disp(add3(4))"),
            "     7\n"
        );
        assert_eq!(
            err_msg("g = @(n) g(n); g(1)"),
            "Unrecognized function or variable 'g'."
        );
        // Created before `b` exists, the body cannot see it later.
        assert_eq!(
            err_msg("f = @() b; b = 1; f()"),
            "Unrecognized function or variable 'b'."
        );
        // Nor a variable of the frame it is called from.
        assert_eq!(
            err_msg("r = call(@() q)\nfunction r = call(f)\nq = 1;\nr = f();\nend"),
            "Unrecognized function or variable 'q'."
        );
        // A name that was no variable resolves as a function of the file
        // the handle was made in.
        assert_eq!(
            ok_out("f = @(x) sq(x); disp(f(3))\nfunction r = sq(x)\nr = x * x;\nend"),
            "     9\n"
        );
        // Only the names the body reads are captured.
        let mut it = Interp::with_output(Box::new(io::sink()));
        it.run("a = 1; b = 2; f = @(x) x + a;").unwrap();
        match it.vars().get("f") {
            Some(Value::Func(f)) => match &**f {
                Func::Anon { captured, .. } => {
                    let names: Vec<&str> = captured.iter().map(|(n, _)| n.as_str()).collect();
                    assert_eq!(names, ["a"]);
                }
                f => panic!("not anonymous: {f:?}"),
            },
            v => panic!("not a handle: {v:?}"),
        }
    }

    /// The body runs in a frame of its own: its `end` is its own, and what
    /// it assigns through a script it calls stays in it.
    #[test]
    fn an_anonymous_call_has_a_frame_of_its_own() {
        assert_eq!(ok_out("w = [1 2 3]; k = @() w(end); disp(k())"), "     3\n");
        assert_eq!(
            ok_out("x = [10 20 30]; f = @(n) n; disp(x(f(end)))"),
            "    30\n"
        );
        let mut it = Interp::with_output(Box::new(io::sink()));
        it.run("f = @(x) x + 1; y = f(1);").unwrap();
        assert_eq!(it.frames.len(), 1);
        assert_eq!(it.calls, 0);
        let mut names: Vec<&String> = it.vars().keys().collect();
        names.sort();
        assert_eq!(names, ["f", "y"]);
        // `nargin` belongs to named functions.
        assert_eq!(
            err_msg("f = @() nargin; f()"),
            "You can only call nargin/nargout from within a MATLAB function."
        );
    }

    /// `nargout` passes through a body that is one call; any other body is
    /// its one value.
    #[test]
    fn nargout_propagates_through_a_single_call_body() {
        assert_eq!(
            ok_out("f = @(v) max(v); [m, i] = f([1 5 2]); disp(i)"),
            "     2\n"
        );
        assert_eq!(
            ok_out(
                "f = @(v) g(v); [a, b] = f(1); disp([a b])\nfunction [p, q] = g(x)\np = x; q = nargout;\nend"
            ),
            "     1     2\n"
        );
        // A handle held in a captured variable passes it on too.
        assert_eq!(
            ok_out("h = @(v) max(v); f = @(v) h(v); [m, i] = f([4 9]); disp(i)"),
            "     2\n"
        );
        // As a statement the body is asked for nothing, so a call that
        // gives nothing is legal and sets no `ans`.
        let mut it = Interp::with_output(Box::new(io::sink()));
        it.run("q = @() disp(1); q()").unwrap();
        assert!(!it.vars().contains_key("ans"));
        assert_eq!(ok_out("q = @() disp(7); q()"), "     7\n");
        assert_eq!(
            err_msg("q = @() disp(7); x = q();"),
            "Too many output arguments."
        );
        assert_eq!(
            err_msg("f = @(x) x + 1; [a, b] = f(1)"),
            "Too many output arguments."
        );
    }

    /// `@name` binds where it is made: a handle to a local function keeps
    /// calling it from a file where the name means something else.
    #[test]
    fn a_named_handle_keeps_its_binding() {
        let dir = TempDir::new("handles");
        dir.write(
            "apply.m",
            "function r = apply(f, v)\nr = f(v);\nend\nfunction r = sq(x)\nr = -1;\nend\n",
        );
        dir.write(
            "getsub.m",
            "function h = getsub()\nh = @inner;\nend\nfunction r = inner(x)\nr = 100 + x;\nend\n",
        );
        let (mut it, buf) = in_dir(&dir);
        // The script's `sq`, not apply.m's own.
        it.run("disp(apply(@sq, 3))\nfunction r = sq(x)\nr = x * x;\nend")
            .unwrap();
        assert_eq!(take(&buf), "     9\n");
        // A subfunction's handle, called where its name resolves to nothing.
        it.run("h = getsub(); disp(h(1)); disp(feval(h, 2))")
            .unwrap();
        assert_eq!(take(&buf), "   101\n   102\n");
        // An unbound name resolves against the path and the builtins when
        // called, never the local functions of wherever it has gone.
        it.run("disp(apply(@abs, -3))").unwrap();
        assert_eq!(take(&buf), "     3\n");
        assert_eq!(
            it.run("f = @nosuch; f(1)").unwrap_err().msg,
            "Unrecognized function or variable 'nosuch'."
        );
    }

    /// Recursion through a handle meets the limit a direct call meets, and
    /// an anonymous call counts as a call.
    #[test]
    fn recursion_through_handles_is_bounded() {
        on_the_interpreter_stack(|| {
            let limit = "Maximum recursion limit of 500 reached.";
            for src in [
                "r = viah(1)\nfunction r = viah(n)\nh = @viah;\nr = h(n + 1);\nend",
                "r = f(1)\nfunction r = f(n)\ng = @(k) f(k + 1);\nr = g(n);\nend",
                "r = f(1)\nfunction r = f(n)\nr = feval(@f, n + 1);\nend",
                "r = f(1)\nfunction r = f(n)\nr = arrayfun(@(k) f(k + 1), n);\nend",
                "a = @(f, x) arrayfun(@(y) f(f, y + 1), x);\nr = a(a, 1);",
                "h = @(g, n) feval(g, g, n + 1);\nx = h(h, 1);",
            ] {
                assert_eq!(err_msg(src), limit, "{src}");
            }
            // A long run of `@feval` handles is peeled, not recursed.
            let src = format!("disp(feval({}@sin, 0))", "@feval, 'feval', ".repeat(3000));
            assert_eq!(ok_out(&src), "     0\n");
        });
    }

    /// A builtin's call back into the interpreter counts one level of the
    /// shared nesting budget, a name or a handle alike, and gives it back.
    #[test]
    fn call_nested_counts_against_the_nesting_budget() {
        let mut it = Interp::with_output(Box::new(io::sink()));
        it.run("f = @() pi;").unwrap();
        let Some(Value::Func(f)) = it.vars().get("f").cloned() else {
            panic!("no handle");
        };
        it.depth = MAX_DEPTH;
        let e = it.call_nested(Callee::Handle(&f), vec![], 1).unwrap_err();
        assert_eq!(e.msg, TOO_DEEP);
        it.depth = MAX_DEPTH;
        let e = it.call_nested(Callee::Name("pi"), vec![], 1).unwrap_err();
        assert_eq!(e.msg, TOO_DEEP);
        it.depth = MAX_DEPTH - 1;
        assert!(it.call_nested(Callee::Name("pi"), vec![], 1).is_ok());
        assert!(it.call_nested(Callee::Handle(&f), vec![], 1).is_ok());
        assert_eq!(it.depth, MAX_DEPTH - 1);
        // `feval` and `arrayfun` go through it: at the edge of the budget
        // they are refused rather than recursing.
        it.depth = MAX_DEPTH;
        let args = vec![Value::Func(f.clone()), Value::Mat(Matrix::row(vec![1.0]))];
        assert_eq!(
            it.call_function("feval", vec![Value::Func(f.clone())], 1)
                .unwrap_err()
                .msg,
            TOO_DEEP
        );
        assert_eq!(
            it.call_function("arrayfun", args, 1).unwrap_err().msg,
            TOO_DEEP
        );
    }

    #[test]
    fn feval_arrayfun_func2str_and_str2func() {
        assert_eq!(
            ok_out(
                "disp(arrayfun(@(x) x * 2, [1 2 3])); disp(arrayfun(@(a, b) a * b, [1 2], [3 4]))"
            ),
            "     2     4     6\n     3     8\n"
        );
        // The shape is the input's, the class the results'.
        assert_eq!(
            ok_out("disp(size(arrayfun(@(x) x, ones(2, 3))))"),
            "     2     3\n"
        );
        assert_eq!(
            ok_out("disp(class(arrayfun(@(x) x > 1, [1 2])))"),
            "logical\n"
        );
        assert_eq!(ok_out("disp(size(arrayfun(@(x) x, [])))"), "     0     0\n");
        assert_eq!(
            ok_out("[m, k] = arrayfun(@(x) max([x 5]), [1 7]); disp([m; k])"),
            "     5     7\n     2     1\n"
        );
        // As a statement, a function that gives nothing is called for its
        // effect.
        assert_eq!(ok_out("arrayfun(@(x) disp(x), [1 2])"), "     1\n     2\n");
        assert_eq!(
            err_msg("arrayfun(@(a, b) a + b, [1 2], [1 2 3])"),
            "All of the input arguments must be of the same size and shape."
        );
        assert_eq!(
            err_msg("y = arrayfun(@(x) [x x], [1 2]);"),
            "Non-scalar in Uniform output, at index 1, output 1. Set 'UniformOutput' to false."
        );
        assert_eq!(
            err_msg("y = arrayfun(5, [1 2]);"),
            "Argument 1 to 'arrayfun' must be a function handle."
        );
        assert_eq!(
            ok_out("disp(feval(@(x) x + 1, 1)); disp(feval('sin', 0))"),
            "     2\n     0\n"
        );
        assert_eq!(
            ok_out(
                "disp(func2str(@(x) x.^2 + 1)); f = str2func('@(x) x*3'); disp(f(2)); disp(func2str(@sin))"
            ),
            "@(x)x.^2+1\n     6\nsin\n"
        );
        assert_eq!(ok_out("g = str2func('abs'); disp(g(-2))"), "     2\n");
        assert_eq!(
            err_msg("disp(func2str(5))"),
            "Argument 1 to 'func2str' must be a function handle."
        );
        // `str2func` captures nothing.
        assert_eq!(
            err_msg("a = 1; f = str2func('@() a'); f()"),
            "Unrecognized function or variable 'a'."
        );
        // A text that does not parse is the parser's error, with no line of
        // its own: the line reported is the program's.
        let e = err("x = 1;\nf = str2func('@(x) x +');");
        assert_eq!(
            (e.msg.as_str(), e.line),
            ("unexpected end of input in expression", Some(2))
        );
        assert_eq!(err_msg("f = str2func('@sin + 1');"), "unexpected '+'");
    }

    #[test]
    fn a_handle_displays_and_answers_class_and_isa() {
        assert_eq!(
            ok_out("f = @(x) x + 1"),
            "f =\n\n  function_handle with value:\n\n    @(x)x+1\n\n"
        );
        assert_eq!(
            ok_out("g = @sin"),
            "g =\n\n  function_handle with value:\n\n    @sin\n\n"
        );
        assert_eq!(
            ok_out(
                "f = @(x) x + 1; disp(class(f)); disp(isa(f, 'function_handle')); disp(isa(f, 'double'))"
            ),
            "function_handle\n   1\n   0\n"
        );
        assert_eq!(ok_out("disp(isa(1, 'function_handle'))"), "   0\n");
        assert_eq!(ok_out("disp(@(x) x + 1); disp(@sin)"), "@(x)x+1\n@sin\n");
        // A handle is one function, not an array.
        let concat =
            "Nonscalar arrays of function handles are not allowed; use cell arrays instead.";
        assert_eq!(err_msg("f = @sin; x = [f 1];"), concat);
        assert_eq!(err_msg("f = @sin; x = [f];"), concat);
        assert_eq!(
            err_msg("f = @sin; x = f + 1;"),
            "Operator '+' is not supported for operands of type 'function_handle'."
        );
        assert_eq!(
            err_msg("f = @sin; x = -f;"),
            "This operation is not supported for a value of class 'function_handle'."
        );
    }

    /// An error leaving an anonymous function names it by its `func2str`
    /// text in the trace.
    #[test]
    fn the_trace_names_an_anonymous_function_by_its_text() {
        let e = err("x = 1;\ng = @(n) g(n); g(1)");
        assert_eq!(
            (e.line, e.trace()),
            (Some(2), "  in @(n)g(n)\n".to_string())
        );
        let e = err("f = @(x) bad(x); f(1)\nfunction r = bad(x)\nerror('boom');\nend");
        assert_eq!(e.trace(), "  in bad (line 3)\n  in @(x)bad(x)\n");
        // Refused before it runs, a call carries no entry.
        assert!(err("f = @(x) x; f(1, 2)").stack().is_empty());
    }

    /// A chain of handles, each capturing the one before, is freed without
    /// recursion. The test thread has Rust's default 2 MB stack, where the
    /// old recursive drop overflowed long before 100,000 links; on the
    /// interpreter's 256 MB thread it took about half a million (cycle 06's
    /// review). Both the explicit `clear` and the drop at the end are
    /// exercised.
    #[test]
    fn a_long_chain_of_captured_handles_is_freed_without_recursion() {
        let chain = "h = @() 1; for k = 1:100000, h = @() h() + 1; end";
        assert_eq!(ok_out(&format!("{chain}; clear h; disp(1)")), "     1\n");
        assert_eq!(ok_out(&format!("{chain}; g = h; disp(2)")), "     2\n");
    }

    /// The names a body reads, through nesting: each is captured once, a
    /// parameter binds only inside its own function, and a name bound by an
    /// inner function is still free outside it. Speed is pinned by the golden
    /// case `free_names_nested`, whose old cost, 174 s, was past the
    /// harness's timeout; a unit test runs on a 2 MB thread, too shallow for
    /// thousands of nested `@()`.
    #[test]
    fn free_names_through_nesting() {
        let names: Vec<String> = (0..2000).map(|i| format!("v{i}")).collect();
        let src = format!(
            "{}; f = {}[{} {}]; disp(numel(f()()()()()()()()()()))",
            names
                .iter()
                .map(|n| format!("{n} = 1"))
                .collect::<Vec<_>>()
                .join("; "),
            "@() ".repeat(10),
            names.join(" "),
            names.join(" ")
        );
        assert_eq!(ok_out(&src), "        4000\n");
        // `x` is f's parameter, so the outer function does not capture the
        // script's `x`; the inner one captures f's `x` (10) when f runs.
        // `y` did not exist when f was made and is no variable of f's frame,
        // so the inner body looks it up as a function and finds none.
        assert_eq!(
            ok_out("x = 1; f = @(x) @() x * 2; g = f(10); disp(g())"),
            "    20\n"
        );
        assert_eq!(
            err_msg("f = @(x) @() x + y; y = 5; g = f(10); g()"),
            "Unrecognized function or variable 'y'."
        );
    }

    // ---- cells and structs (cycle 07) --------------------------------

    /// A cell literal nests what it is given; braces read the contents and
    /// parentheses a cell; a chain indexes into an element.
    #[test]
    fn cells_are_built_and_read_by_brace_and_paren() {
        assert_eq!(
            ok_out(
                "c = {1, 'two', [3 4]}; disp(class(c)); disp(c{2}); disp(c{3}(2)); disp(size(c(2:3)))"
            ),
            "cell\ntwo\n     4\n     1     2\n"
        );
        // Whitespace separates, a semicolon or a newline starts a row, and
        // the storage is column-major like a matrix's.
        assert_eq!(
            ok_out("c = {1 -2; 3 4}; disp(c{2}); disp(c{1, 2})"),
            "     3\n    -2\n"
        );
        assert_eq!(ok_out("c = {1, 2\n3, 4}; disp(size(c))"), "     2     2\n");
        // `{c}` of a cell nests it rather than joining it.
        assert_eq!(
            ok_out("c = {1}; d = {c, 2}; disp(class(d{1})); disp(d{1}{1})"),
            "cell\n     1\n"
        );
        assert_eq!(ok_out("c = {}; disp(size(c))"), "     0     0\n");
        assert_eq!(err_msg("c = {1, 2; 3}"), error::concat_dims().msg);
        assert_eq!(
            ok_out("c = {1, 2, 3}; disp(c{end}); d = c([1 3]); disp(d{2})"),
            "     3\n     3\n"
        );
        assert_eq!(
            err_msg("c = {1, 2}; c{3}"),
            error::index_exceeds_numel(2).msg
        );
    }

    /// Growth, deletion and the assignment of cells into cells; a failed
    /// assignment leaves the cell as it was.
    #[test]
    fn a_cell_grows_and_shrinks() {
        assert_eq!(
            ok_out(
                "c = cell(1, 3); disp(isempty(c{1})); c{5} = 'x'; disp(numel(c)); c(2) = []; disp(numel(c))"
            ),
            "   1\n     5\n     4\n"
        );
        assert_eq!(
            ok_out("c = {1}; c{2, 3} = 5; disp(size(c)); disp(isempty(c{2, 2}))"),
            "     2     3\n   1\n"
        );
        assert_eq!(
            ok_out("c = {1, 2, 3}; c(2) = {9}; disp(c{2}); c(1:2) = {0}; disp([c{:}])"),
            "     9\n     0     0     3\n"
        );
        assert_eq!(
            ok_out("x = []; x{2} = 1; disp(class(x)); disp(size(x))"),
            "cell\n     1     2\n"
        );
        assert_eq!(ok_out("q{3} = 1; disp(size(q))"), "     1     3\n");
        assert_eq!(
            err_msg("c = {1}; c(2) = 5;"),
            error::conversion("cell", "double").msg
        );
        assert_eq!(
            err_msg("x = [1 2]; x(2) = {3};"),
            error::conversion("double", "cell").msg
        );
        assert_eq!(
            err_msg("x = 1; x{1} = 2;"),
            error::brace_assign_unsupported().msg
        );
        assert_eq!(
            ok_out("c = {1, 2}; try, c(5) = 7; catch, end; disp(size(c))"),
            "     1     2\n"
        );
        // An assignment that creates the variable and fails leaves none.
        assert_eq!(
            ok_out("try, q.a{2}(0) = 1; catch, end; disp(exist('q'))"),
            "     0\n"
        );
    }

    /// A copy is independent: the shared storage is copied on the first
    /// write, and only then.
    #[test]
    fn containers_are_values() {
        assert_eq!(
            ok_out("c = {1, 2}; d = c; d{1} = 9; disp(c{1}); disp(d{1})"),
            "     1\n     9\n"
        );
        assert_eq!(
            ok_out("s.a = 1; t = s; t.a = 2; disp(s.a); disp(t.a)"),
            "     1\n     2\n"
        );
        assert_eq!(
            ok_out("c = {1}; c{2} = c; disp(class(c{2})); disp(numel(c{2}))"),
            "cell\n     1\n"
        );
    }

    /// Field assignment creates what does not exist, down any path; a
    /// dynamic field is a field named at run time.
    #[test]
    fn fields_are_created_by_assignment() {
        assert_eq!(
            ok_out(
                "s = struct('x', 5, 'y', [1 2]); disp(s.y(2)); s.inner.v = 3; s.inner.v = s.inner.v + 1; disp(s.inner.v); n = 'x'; disp(s.(n))"
            ),
            "     2\n     4\n     5\n"
        );
        assert_eq!(
            ok_out("n = 'k'; s.(n) = 7; s.(n) = s.(n) + 1; disp(s.k)"),
            "     8\n"
        );
        assert_eq!(
            ok_out("s.data(end + 1) = 4; s.data(end + 1) = 5; disp(s.data)"),
            "     4     5\n"
        );
        assert_eq!(
            ok_out("s.c{2} = 'b'; disp(class(s.c)); disp(s.c{2})"),
            "cell\nb\n"
        );
        assert_eq!(ok_out("x = []; x.a = 1; disp(isstruct(x))"), "   1\n");
        assert_eq!(
            err_msg("x = 1; x.a = 2;"),
            error::dot_assign_unsupported().msg
        );
        assert_eq!(err_msg("s.a = 1; s.b"), error::no_such_field("b").msg);
        assert_eq!(err_msg("s.(5) = 1;"), error::dynamic_field_not_text().msg);
        assert_eq!(
            err_msg("s.('a b') = 1;"),
            error::invalid_field_name("a b").msg
        );
        // A failure deep in the path adds no field on the way.
        assert_eq!(
            ok_out("s.a = 1; try, s.b.c = [1 2]; s.b.c(0) = 1; catch, end; disp(isfield(s, 'b'))"),
            "   1\n"
        );
        assert_eq!(
            ok_out("s.a = 1; try, s.z.w(0) = 1; catch, end; disp(isfield(s, 'z'))"),
            "   0\n"
        );
    }

    /// `p(k).f = v` grows a struct array; `p.f` of one is a cs-list; a
    /// struct array needs one element for a field assignment.
    #[test]
    fn a_struct_array_grows_by_element() {
        assert_eq!(
            ok_out(
                "p(1).name = 'A'; p(2).name = 'B'; disp(numel(p)); disp(p(2).name); disp(class(p)); q = [p.name]; disp(q)"
            ),
            "     2\nB\nstruct\nAB\n"
        );
        assert_eq!(
            ok_out("p(3).v = 1; disp(size(p)); disp(isempty(p(1).v))"),
            "     1     3\n   1\n"
        );
        assert_eq!(
            ok_out("p(2).a = 1; p(1).b = 2; disp(isempty(p(2).b)); disp(p(1).b)"),
            "   1\n     2\n"
        );
        assert_eq!(
            err_msg("p(2).a = 1; p.a = 3;"),
            error::scalar_struct_required().msg
        );
        assert_eq!(err_msg("p(2).a = 1; z = p.a;"), error::cs_list_count(2).msg);
        assert_eq!(ok_out("p(2).a = 1; p(1) = []; disp(numel(p))"), "     1\n");
        assert_eq!(
            err_msg("s.a = 1; t.b = 2; s(2) = t;"),
            error::dissimilar_structs().msg
        );
        assert_eq!(
            ok_out("s.a = 1; t.a = 2; s(2) = t; disp([s.a])"),
            "     1     2\n"
        );
    }

    /// Where a cs-list goes: spread into a call's arguments, a bracket and
    /// a brace; a statement shows each value; one value is wanted anywhere
    /// else.
    #[test]
    fn a_cs_list_spreads_or_must_be_one() {
        assert_eq!(
            ok_out("c = {1, 2, 3}; disp([c{:}]); disp(max(c{2:3}))"),
            "     1     2     3\n     3\n"
        );
        assert_eq!(
            ok_out("c = {1, 2}; d = {c{:}, 3}; disp(size(d))"),
            "     1     3\n"
        );
        assert_eq!(ok_out("c = {}; disp(size([c{:}]))"), "     0     0\n");
        assert_eq!(
            ok_out("c = {1, 2}; c{:}"),
            "ans =\n\n     1\n\nans =\n\n     2\n\n"
        );
        assert_eq!(ok_out("c = {1, 2}; [a, b] = c{:}; disp(b)"), "     2\n");
        assert_eq!(
            err_msg("c = {1, 2}; y = c{:};"),
            error::cs_list_count(2).msg
        );
        assert_eq!(err_msg("c = {}; y = c{:};"), error::cs_list_count(0).msg);
        assert_eq!(
            err_msg("c = {1, 2}; [a, b, d] = c{:};"),
            error::insufficient_outputs().msg
        );
        assert_eq!(
            err_msg("c = {1, 2}; y = c{:} + 1;"),
            error::cs_list_count(2).msg
        );
    }

    /// `for` over a cell or a struct array takes one column at a time.
    #[test]
    fn for_iterates_the_columns_of_a_container() {
        assert_eq!(
            ok_out("for c = {1, 'a'}, disp(class(c)), end"),
            "cell\ncell\n"
        );
        assert_eq!(
            ok_out("for c = {1; 2}, disp(size(c)), end"),
            "     2     1\n"
        );
        assert_eq!(
            ok_out("for s = struct('v', {4, 5}), disp(s.v), end"),
            "     4\n     5\n"
        );
        assert_eq!(ok_out("for c = {}, disp(1), end; disp(class(c))"), "cell\n");
    }

    /// A last parameter `varargin` takes the rest of the arguments and a
    /// last output `varargout` gives the rest of the outputs; `nargin` and
    /// `nargout` count them all.
    #[test]
    fn varargin_and_varargout() {
        let fns = "\nfunction r = cnt(varargin)\nr = nargin;\nend\nfunction varargout = mv()\nvarargout{1} = 1; varargout{2} = 2;\nend\nfunction [a, varargout] = two(x, varargin)\na = x; varargout = varargin; disp(nargout)\nend";
        assert_eq!(
            ok_out(&format!("disp(cnt(1, 2, 3)); disp(cnt()){fns}")),
            "     3\n     0\n"
        );
        assert_eq!(
            ok_out(&format!("[a, b] = mv(); disp([a b]){fns}")),
            "     1     2\n"
        );
        assert_eq!(ok_out(&format!("mv(){fns}")), "ans =\n\n     1\n\n");
        assert_eq!(
            ok_out(&format!("[p, q, r] = two(1, 2, 3); disp([p q r]){fns}")),
            "     3\n     1     2     3\n"
        );
        assert_eq!(
            err_msg(&format!("[a, b, c] = mv();{fns}")),
            error::varargout_not_assigned(3, "mv").msg
        );
        assert_eq!(
            ok_out("f = @(varargin) numel(varargin); disp(f(1, 2, 3))"),
            "     3\n"
        );
        assert_eq!(
            ok_out(&format!("c = {{1, 2}}; disp(cnt(c{{:}}, 3)){fns}")),
            "     3\n"
        );
    }

    /// A binary operator refuses a value that is not an array in MATLAB's
    /// R2020a words, naming the operator and the first such operand's
    /// class; a unary one keeps SplatCrab's generic text.
    #[test]
    fn an_operator_on_a_container_names_its_class() {
        assert_eq!(
            err_msg("c = {1}; c + 1"),
            error::operator_unsupported("+", "cell").msg
        );
        assert_eq!(
            err_msg("c = {1}; 1 + c"),
            error::operator_unsupported("+", "cell").msg
        );
        assert_eq!(
            err_msg("s.a = 1; s * 2"),
            error::operator_unsupported("*", "struct").msg
        );
        assert_eq!(
            err_msg("c = {1}; c == 1"),
            error::operator_unsupported("==", "cell").msg
        );
        assert_eq!(
            err_msg("f = @sin; f && 1"),
            error::operator_unsupported("&&", "function_handle").msg
        );
        assert_eq!(err_msg("c = {1}; -c"), error::not_an_array("cell").msg);
    }

    /// `e.stack` is an Nx1 struct array of the frames, innermost first.
    #[test]
    fn e_stack_is_a_struct_array_of_the_frames() {
        let src = "try, g1(), catch e, end\ndisp(size(e.stack)); disp(e.stack(1).name); disp(e.stack(1).line); disp(e.stack(2).name); disp(isempty(e.stack(1).file))\nfunction g1()\ng2();\nend\nfunction g2()\nerror('x:y', 'deep');\nend";
        assert_eq!(ok_out(src), "     2     1\ng2\n     7\ng1\n   1\n");
        assert_eq!(
            ok_out(
                "try, error('x'), catch e, end; disp(size(e.stack)); f = fieldnames(e.stack); disp([f{:}])"
            ),
            "     0     1\nfilenameline\n"
        );
    }

    /// A deep chain through cells, structs and handles is freed without
    /// recursion, on this test's own 2 MB thread, whether it is cleared or
    /// reassigned or simply dropped with the interpreter.
    #[test]
    fn deep_chains_through_containers_are_freed_iteratively() {
        let cells = "c = {}; for k = 1:100000, c = {c}; end";
        assert_eq!(ok_out(&format!("{cells}; clear c; disp(1)")), "     1\n");
        assert_eq!(ok_out(&format!("{cells}; c = 0; disp(2)")), "     2\n");
        assert_eq!(ok_out(&format!("{cells}; disp(3)")), "     3\n");
        let mixed = "c = {}; for k = 1:100000, h = @() c; c = {h}; end";
        assert_eq!(ok_out(&format!("{mixed}; clear c h; disp(4)")), "     4\n");
        let structs = "s = 0; for k = 1:100000, s = struct('a', {{s}}); end";
        assert_eq!(ok_out(&format!("{structs}; clear s; disp(5)")), "     5\n");
        let fields = "s = 0; for k = 1:100000, t.a = s; s = t; end";
        assert_eq!(ok_out(&format!("{fields}; clear s t; disp(6)")), "     6\n");
        // Displaying one is bounded too: a nested cell is summarised.
        assert_eq!(
            ok_out(&format!("{cells}; c")),
            "c =\n\n  1×1 cell array\n\n    {1×1 cell}\n\n"
        );
    }

    /// Brackets make a cell when a cell takes part, and a struct array
    /// when structs do.
    #[test]
    fn brackets_join_cells_and_structs() {
        assert_eq!(
            ok_out("d = [{1}, 2]; disp(class(d)); disp(numel(d))"),
            "cell\n     2\n"
        );
        assert_eq!(ok_out("d = [{1}; {2}]; disp(size(d))"), "     2     1\n");
        assert_eq!(ok_out("d = [{}, {1}, []]; disp(size(d))"), "     1     1\n");
        assert_eq!(
            ok_out("s.a = 1; t.a = 2; u = [s t]; disp(size(u)); disp(u(2).a)"),
            "     1     2\n     2\n"
        );
        assert_eq!(
            err_msg("s.a = 1; t.b = 2; u = [s t];"),
            error::struct_concat_fields().msg
        );
        assert_eq!(
            err_msg("s.a = 1; u = [s 1];"),
            error::conversion("struct", "double").msg
        );
        assert_eq!(
            err_msg("c = {1, 2}; d = [c; {1}];"),
            error::concat_dims().msg
        );
    }

    /// The link resolver judges `end` against what the path holds, and a
    /// path that does not exist yet against `0x0`.
    #[test]
    fn assignment_links_resolve_end_along_the_path() {
        assert_eq!(
            ok_out("s.c = {1, 2}; s.c{end + 1} = 3; disp(numel(s.c))"),
            "     3\n"
        );
        assert_eq!(ok_out("s.c{end + 1} = 3; disp(numel(s.c))"), "     1\n");
        assert_eq!(
            ok_out("p(2).v = [1 2]; p(2).v(end + 1) = 3; disp(p(2).v)"),
            "     1     2     3\n"
        );
        assert_eq!(
            ok_out("c = {[1 2]}; c{1}(end) = []; disp(c{1})"),
            "     1\n"
        );
    }

    /// The pure helpers: growth of column-major items and what a deletion
    /// keeps.
    #[test]
    fn regrid_and_keep_positions_respect_column_major_order() {
        let mut v = vec![1, 2, 3, 4];
        regrid(&mut v, (2, 2), (3, 2), || 0);
        assert_eq!(v, [1, 2, 0, 3, 4, 0]);
        let mut v = vec![1, 2];
        regrid(&mut v, (1, 2), (1, 4), || 0);
        assert_eq!(v, [1, 2, 0, 0]);
        let mut v: Vec<i32> = Vec::new();
        regrid(&mut v, (0, 0), (2, 1), || 7);
        assert_eq!(v, [7, 7]);
        assert_eq!(
            keep_positions(vec!['a', 'b', 'c', 'd'], &[0, 2, 3]),
            ['a', 'c', 'd']
        );
        assert_eq!(one_position(2, 2, &[Sel::row(vec![3])]), Some(3));
        assert_eq!(one_position(2, 2, &[Sel::row(vec![4])]), None);
        assert_eq!(one_position(2, 2, &[Sel::row(vec![0, 1])]), None);
    }
}
