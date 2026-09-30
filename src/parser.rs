//! Recursive-descent parser producing statements and expressions.
//!
//! Precedence (loosest to tightest), following MATLAB:
//!   ||   &&   |   &   comparison   :   + -   * / \ .* ./ .\   unary - ~   ^ .^   transpose

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::bail;
use crate::error::{self, R};
use crate::lexer::{Lexed, Token};

/// `@(params) body`, an anonymous function as written (cycle 06).
///
#[derive(Clone, Debug, PartialEq)]
pub struct AnonFn {
    /// The parameter names in order, `~` kept as `"~"`, as a function's are.
    pub params: Vec<String>,
    pub body: Expr,
}

impl AnonFn {
    pub fn new(params: Vec<String>, body: Expr) -> AnonFn {
        AnonFn { params, body }
    }

    /// Every name the body reads that no enclosing parameter list binds, in
    /// the order first read and without repeats: what is captured when the
    /// function is made, those of them that are variables at that moment.
    ///
    /// It is one walk of the tree, linear in its size, and nothing is kept.
    /// It used to be a list stored on every `AnonFn` and built from the
    /// lists of the functions nested in it, with repeats found by rescanning
    /// the list: quadratic in the names a body reads and, nested, a full copy
    /// per level, so 3,000 nested `@()` around 3,000 names took minutes
    /// (cycle 06's review). The walk recurses once per level of the tree,
    /// which the parser has already bounded by [`MAX_DEPTH`].
    pub fn free_names(&self) -> Vec<String> {
        let mut w = FreeNames::default();
        w.scope(&self.params, &self.body);
        w.out
    }

    /// `func2str`'s text: `@(x,y)` and the body rendered by [`render`].
    pub fn text(&self) -> String {
        let mut out = Text::whole();
        self.render_into(&mut out);
        out.s
    }

    /// The first `chars` characters of [`AnonFn::text`], and no more: the
    /// render stops once it has them, so it costs time in proportion to
    /// `chars`, never to the function's size. What a workspace preview
    /// shows of a handle (cycle U2).
    pub fn text_up_to(&self, chars: usize) -> String {
        let mut out = Text::up_to(chars);
        self.render_into(&mut out);
        out.s
    }

    fn render_into(&self, out: &mut Text) {
        out.push_str("@(");
        for (k, p) in self.params.iter().enumerate() {
            if out.full() {
                return;
            }
            if k > 0 {
                out.push(',');
            }
            out.push_str(p);
        }
        out.push(')');
        render_into(&self.body, out);
    }
}

/// The state of [`AnonFn::free_names`]'s walk: how many enclosing
/// parameter lists bind each name, the names already reported, and the
/// report itself.
#[derive(Default)]
struct FreeNames<'a> {
    bound: HashMap<&'a str, usize>,
    seen: HashSet<&'a str>,
    out: Vec<String>,
}

impl<'a> FreeNames<'a> {
    /// Walks `body` with `params` bound, and unbinds them after.
    fn scope(&mut self, params: &'a [String], body: &'a Expr) {
        for p in params {
            *self.bound.entry(p.as_str()).or_insert(0) += 1;
        }
        self.walk(body);
        for p in params {
            if let Some(n) = self.bound.get_mut(p.as_str()) {
                *n -= 1;
                if *n == 0 {
                    self.bound.remove(p.as_str());
                }
            }
        }
    }

    /// A name read: reported the first time it is read while unbound.
    fn read(&mut self, n: &'a str) {
        if !self.bound.contains_key(n) && self.seen.insert(n) {
            self.out.push(n.to_string());
        }
    }

    /// Every name `e` reads: a bare name, the name an access chain starts
    /// from, and the names inside its arguments. A field name is not a name.
    fn walk(&mut self, e: &'a Expr) {
        match e {
            Expr::Num(_)
            | Expr::Imag(_)
            | Expr::Str(_)
            | Expr::End
            | Expr::Colon
            | Expr::FuncHandle(_) => {}
            Expr::Ident(n) => self.read(n),
            Expr::Access(n, chain) => {
                self.read(n);
                for a in chain {
                    match a {
                        Access::Paren(args) | Access::Brace(args) => {
                            args.iter().for_each(|x| self.walk(x))
                        }
                        Access::Field(_) => {}
                        Access::DynField(x) => self.walk(x),
                    }
                }
            }
            Expr::Matrix(rows) | Expr::Cell(rows) => {
                rows.iter().flatten().for_each(|x| self.walk(x))
            }
            Expr::Neg(a)
            | Expr::Pos(a)
            | Expr::Not(a)
            | Expr::Transpose(a)
            | Expr::DotTranspose(a) => self.walk(a),
            Expr::Binary(_, a, b) => {
                self.walk(a);
                self.walk(b);
            }
            Expr::Range(a, s, b) => {
                self.walk(a);
                if let Some(s) = s {
                    self.walk(s);
                }
                self.walk(b);
            }
            Expr::AnonFn(f) => self.scope(&f.params, &f.body),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    /// An imaginary literal, `2.5i`: the number that multiplies the unit
    /// (cycle 10).
    Imag(f64),
    Str(String),
    Ident(String),
    /// `end` inside an index expression.
    End,
    /// Bare `:` inside an index expression.
    Colon,
    /// `[a b; c d]` — rows of elements.
    Matrix(Vec<Vec<Expr>>),
    /// `{a b; c d}`, a cell literal (cycle 07): rows of elements, each of
    /// which becomes one element of the cell, whatever it is.
    Cell(Vec<Vec<Expr>>),
    /// `name` followed by one or more accesses: `x(2)`, `f(a, b)`, `c{1}`,
    /// `s.a`, `s.(n)` and chains of them such as `c{1}(2).b`. The chain is
    /// never empty; a bare name is [`Expr::Ident`]. The first `(...)` is
    /// indexing if `name` is a variable and a call otherwise.
    Access(String, Vec<Access>),
    Neg(Box<Expr>),
    /// Unary plus. It is not a no-op: it is arithmetic, so `+'a'` is the
    /// double `97` and `+true` the double `1`.
    Pos(Box<Expr>),
    Not(Box<Expr>),
    /// `a'`, the conjugate transpose (cycle 10; a plain one before it).
    Transpose(Box<Expr>),
    /// `a.'`, the plain transpose (cycle 10).
    DotTranspose(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `a:b` or `a:s:b`
    Range(Box<Expr>, Option<Box<Expr>>, Box<Expr>),
    /// `@name`, a handle to a named function (cycle 06).
    FuncHandle(String),
    /// `@(params) body` (cycle 06). Shared, since every evaluation of it
    /// makes a function value that holds the definition.
    AnonFn(Rc<AnonFn>),
}

/// How tightly each form binds, loosest first, following the parser's
/// precedence ladder: what [`render`] compares to decide on parentheses.
fn prec(e: &Expr) -> u8 {
    match e {
        // An anonymous function's body runs to the end of the expression,
        // so it binds loosest of all.
        Expr::AnonFn(_) => 0,
        Expr::Binary(op, ..) => binop_prec(*op),
        Expr::Range(..) => 6,
        Expr::Neg(_) | Expr::Pos(_) | Expr::Not(_) => 9,
        Expr::Transpose(_) | Expr::DotTranspose(_) => 11,
        _ => 12,
    }
}

fn binop_prec(op: BinOp) -> u8 {
    match op {
        BinOp::OrOr => 1,
        BinOp::AndAnd => 2,
        BinOp::Or => 3,
        BinOp::And => 4,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 5,
        BinOp::Add | BinOp::Sub => 7,
        BinOp::Mul | BinOp::Div | BinOp::LDiv | BinOp::EMul | BinOp::EDiv | BinOp::ELDiv => 8,
        BinOp::Pow | BinOp::EPow => 10,
    }
}

pub(crate) fn binop_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::LDiv => "\\",
        BinOp::Pow => "^",
        BinOp::EMul => ".*",
        BinOp::EDiv => "./",
        BinOp::ELDiv => ".\\",
        BinOp::EPow => ".^",
        BinOp::Eq => "==",
        BinOp::Ne => "~=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&",
        BinOp::Or => "|",
        BinOp::AndAnd => "&&",
        BinOp::OrOr => "||",
    }
}

/// `e` as source text, rendered from the tree: what `func2str` shows of an
/// anonymous function's body (cycle 06).
///
/// No space around a binary operator, a comma between the elements of a
/// bracket and a semicolon between its rows, so `[x 1]` comes back as
/// `[x,1]` and still has two elements. The tree keeps no parentheses, so
/// they are put back only where the precedence needs them: `(x+1)*2`, but
/// `x+1*2`. Numbers are written in the shortest form that reads back as the
/// same value; a string in single quotes with its quotes doubled.
pub fn render(e: &Expr) -> String {
    let mut s = Text::whole();
    render_into(e, &mut s);
    s.s
}

/// Where [`render_into`] writes: the text so far and, for a render with a
/// budget, how many more characters it may take. Once the budget is spent
/// every push is dropped and the walk goes no further: each node returns as
/// it is reached and each list stops at its next element. Every node
/// rendered writes at least one character, so a render with a budget visits
/// at most about as many nodes as the budget, plus the path down to the
/// first character, which [`MAX_DEPTH`] bounds.
struct Text {
    s: String,
    left: Option<usize>,
}

impl Text {
    /// A render of the whole tree, as `func2str` and the display make.
    fn whole() -> Text {
        Text {
            s: String::new(),
            left: None,
        }
    }

    /// A render that stops after its first `chars` characters.
    fn up_to(chars: usize) -> Text {
        Text {
            s: String::new(),
            left: Some(chars),
        }
    }

    /// True once the budget is spent: nothing more will be written.
    fn full(&self) -> bool {
        self.left == Some(0)
    }

    fn push(&mut self, c: char) {
        match &mut self.left {
            None => self.s.push(c),
            Some(0) => {}
            Some(n) => {
                *n -= 1;
                self.s.push(c);
            }
        }
    }

    fn push_str(&mut self, t: &str) {
        if self.left.is_none() {
            self.s.push_str(t);
            return;
        }
        for c in t.chars() {
            if self.full() {
                break;
            }
            self.push(c);
        }
    }
}

#[cfg(test)]
thread_local! {
    /// How many nodes [`render_into`] has been called on, on this thread:
    /// what a unit test counts to show that a render with a budget visits
    /// a bounded number of them.
    pub(crate) static RENDERED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn render_into(e: &Expr, s: &mut Text) {
    #[cfg(test)]
    RENDERED.with(|n| n.set(n.get() + 1));
    if s.full() {
        return;
    }
    // A child that binds looser than `min` is parenthesised.
    fn child(e: &Expr, min: u8, s: &mut Text) {
        if prec(e) < min {
            s.push('(');
            render_into(e, s);
            s.push(')');
        } else {
            render_into(e, s);
        }
    }
    // The exponent may be a signed operand without parentheses, `2^-1`,
    // as long as what the sign applies to is itself a postfix operand.
    fn exponent_ok(e: &Expr) -> bool {
        match e {
            Expr::Neg(a) | Expr::Pos(a) | Expr::Not(a) => exponent_ok(a),
            e => prec(e) >= 11,
        }
    }
    fn list(args: &[Expr], s: &mut Text) {
        for (k, a) in args.iter().enumerate() {
            if s.full() {
                return;
            }
            if k > 0 {
                s.push(',');
            }
            render_into(a, s);
        }
    }
    match e {
        Expr::Num(v) => s.push_str(&number_text(*v)),
        Expr::Imag(v) => {
            s.push_str(&number_text(*v));
            s.push('i');
        }
        Expr::Str(t) => {
            s.push('\'');
            for c in t.chars() {
                if s.full() {
                    break;
                }
                if c == '\'' {
                    s.push_str("''");
                } else {
                    s.push(c);
                }
            }
            s.push('\'');
        }
        Expr::Ident(n) => s.push_str(n),
        Expr::End => s.push_str("end"),
        Expr::Colon => s.push(':'),
        Expr::Matrix(rows) | Expr::Cell(rows) => {
            let (open, close) = match e {
                Expr::Cell(_) => ('{', '}'),
                _ => ('[', ']'),
            };
            s.push(open);
            for (k, row) in rows.iter().enumerate() {
                if s.full() {
                    break;
                }
                if k > 0 {
                    s.push(';');
                }
                list(row, s);
            }
            s.push(close);
        }
        Expr::Access(n, chain) => {
            s.push_str(n);
            for a in chain {
                if s.full() {
                    break;
                }
                match a {
                    Access::Paren(args) => {
                        s.push('(');
                        list(args, s);
                        s.push(')');
                    }
                    Access::Brace(args) => {
                        s.push('{');
                        list(args, s);
                        s.push('}');
                    }
                    Access::Field(f) => {
                        s.push('.');
                        s.push_str(f);
                    }
                    Access::DynField(x) => {
                        s.push_str(".(");
                        render_into(x, s);
                        s.push(')');
                    }
                }
            }
        }
        Expr::Neg(a) | Expr::Pos(a) | Expr::Not(a) => {
            s.push(match e {
                Expr::Neg(_) => '-',
                Expr::Pos(_) => '+',
                _ => '~',
            });
            child(a, 9, s);
        }
        Expr::Transpose(a) => {
            child(a, 11, s);
            s.push('\'');
        }
        Expr::DotTranspose(a) => {
            child(a, 11, s);
            s.push_str(".'");
        }
        Expr::Binary(op, a, b) => {
            let p = binop_prec(*op);
            // Every binary operator folds to the left, so the left operand
            // may be the same operator and the right one may not.
            child(a, p, s);
            s.push_str(binop_text(*op));
            if p == 10 && exponent_ok(b) {
                render_into(b, s);
            } else {
                child(b, p + 1, s);
            }
        }
        Expr::Range(a, step, b) => {
            // `1:2:3:4` is `(1:2:3):4`, so a stepped range reads back on
            // the left unparenthesised; `(1:2):3` must keep its brackets or
            // it would read back as the stepped `1:2:3`.
            match &**a {
                Expr::Range(_, Some(_), _) => render_into(a, s),
                a => child(a, 7, s),
            }
            if let Some(st) = step {
                s.push(':');
                child(st, 7, s);
            }
            s.push(':');
            child(b, 7, s);
        }
        Expr::FuncHandle(n) => {
            s.push('@');
            s.push_str(n);
        }
        Expr::AnonFn(f) => f.render_into(s),
    }
}

/// A number literal as it reads back: the shortest round-tripping decimal,
/// in exponent form outside `1e-5 <= |v| < 1e15`.
fn number_text(v: f64) -> String {
    if !v.is_finite() {
        return crate::value::nonfinite(v);
    }
    let a = v.abs();
    if a == 0.0 || (1e-5..1e15).contains(&a) {
        return format!("{}", v);
    }
    let t = format!("{:e}", v);
    match t.split_once('e') {
        Some((m, e)) if e.starts_with('-') => format!("{}e{}", m, e),
        Some((m, e)) => format!("{}e+{}", m, e),
        None => t,
    }
}

/// One link of an access chain, in the order written.
#[derive(Clone, Debug, PartialEq)]
pub enum Access {
    /// `(args)`: indexing, or the arguments of a call.
    Paren(Vec<Expr>),
    /// `{args}`: brace indexing, which cycle 07's cells give a meaning.
    Brace(Vec<Expr>),
    /// `.name`: a field, which cycle 07's structs give a meaning.
    Field(String),
    /// `.(expr)`: a dynamic field name.
    DynField(Box<Expr>),
}

/// An assignment target: a name and the accesses that select what inside it
/// is assigned. `x = ...` has an empty chain and `x(2) = ...` a one-link one.
#[derive(Clone, Debug, PartialEq)]
pub struct LValue {
    pub name: String,
    pub chain: Vec<Access>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    LDiv,
    Pow,
    EMul,
    EDiv,
    /// Elementwise left divide `.\`: `a.\b` is `b ./ a`.
    ELDiv,
    EPow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    AndAnd,
    OrOr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// Expression statement; bool = display result (no trailing `;`).
    Expr(Expr, bool),
    /// `target = rhs`; bool = display the assigned variable.
    Assign(LValue, Expr, bool),
    /// `[a, ~, c] = rhs`: one target per output, `None` for a `~`
    /// placeholder, which asks for the output and discards it. `[x] = rhs`
    /// is the one-target form.
    MultiAssign(Vec<Option<LValue>>, Expr, bool),
    If(Vec<IfArm>, Option<Vec<Located>>),
    For(String, Expr, Vec<Located>),
    While(Expr, Vec<Located>),
    /// `switch subject`, its `case` arms in order, and the `otherwise` body.
    Switch(Expr, Vec<CaseArm>, Option<Vec<Located>>),
    /// `try body catch var handler end`: the name a `catch` on the same line
    /// binds, if any, and the handler, empty for a `try` with no `catch`.
    Try(Vec<Located>, Option<String>, Vec<Located>),
    Break,
    Continue,
    /// `return`: leaves the running function, or the running script
    /// (cycle 05).
    Return,
}

/// A `function` block (cycle 05): its name, the names of its outputs and of
/// its parameters in order, its body, and the line its `function` is on.
/// A parameter written `~` is ignored, and is kept as `"~"` so that the
/// argument count still counts it.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub outputs: Vec<String>,
    pub params: Vec<String>,
    pub body: Vec<Located>,
    pub line: u32,
}

/// A whole source text: its statements and the functions defined after
/// them. A function file is one whose statements are empty and whose first
/// function is its entry; a script may end in local functions.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Program {
    pub stmts: Vec<Located>,
    pub functions: Vec<Function>,
}

/// One `case` of a `switch`: the values it matches, the line the `case` is
/// on, and the body to run.
///
/// `case {a, b}` is a list of two values and `case a` a list of one. The
/// braces are syntax here, not a cell: cells arrive in cycle 07, and a case
/// list is the one place a brace can open a value before then.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseArm {
    pub values: Vec<Expr>,
    pub line: u32,
    pub body: Vec<Located>,
}

/// One `if` or `elseif` arm: a condition, the line that condition is written
/// on, and the body to run when it holds.
///
/// The line is on the arm for the same reason [`Located`] exists: an error
/// raised while evaluating an `elseif` condition must report the `elseif`'s
/// own line, not the line of the `if` that opened the statement (QA D27).
/// `Expr` itself stays position-free, so expression trees still compare over
/// structure alone.
#[derive(Clone, Debug, PartialEq)]
pub struct IfArm {
    pub cond: Expr,
    pub line: u32,
    pub body: Vec<Located>,
}

/// The deepest nesting the parser will build and the evaluator will walk.
///
/// Both recursions are one frame per level, so one number bounds both: a
/// program the parser accepts is one the evaluator can walk without running
/// out of stack. See the Design notes of `docs/modules/01e-display-and-parser.md`
/// for how the number was chosen and the margin it leaves against the 256 MB
/// stack `src/main.rs` reserves.
pub const MAX_DEPTH: usize = 10_000;

/// A statement with the source line it starts on.
///
/// The line sits on a wrapper rather than inside every `Stmt` variant so that
/// shape and position stay separable: `Stmt` and `Expr` still compare over
/// structure alone, and a block body is a `Vec<Located>`, which is what lets a
/// runtime error inside a `for` body report the body's line.
#[derive(Clone, Debug, PartialEq)]
pub struct Located {
    pub stmt: Stmt,
    pub line: u32,
}

pub struct Parser {
    toks: Vec<Token>,
    /// `lines[k]` is the source line of `toks[k]`. Empty when the parser was
    /// built from tokens alone, and then every statement reports line 1.
    lines: Vec<u32>,
    pos: usize,
    /// Depth of `name( ... )` argument lists we are inside; enables `end` and bare `:`.
    in_index: usize,
    /// How deeply nested the construct being parsed is; see [`MAX_DEPTH`].
    depth: usize,
}

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Parser {
            toks,
            lines: Vec::new(),
            pos: 0,
            in_index: 0,
            depth: 0,
        }
    }

    /// The parser the interpreter uses: tokens with the lines they came from.
    pub fn with_lines(lexed: Lexed) -> Self {
        Parser {
            toks: lexed.tokens,
            lines: lexed.lines,
            pos: 0,
            in_index: 0,
            depth: 0,
        }
    }

    /// The same parser, counting its nesting from `depth` rather than from
    /// zero. A function file is parsed when it is first called, however
    /// deep the evaluator already is, on the same stack; starting from the
    /// evaluator's depth keeps the two recursions together under the one
    /// [`MAX_DEPTH`], so the file cannot overflow what the caller left.
    pub fn at_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Counts one more level of nesting, refusing anything past [`MAX_DEPTH`].
    ///
    /// Two things call it, because both make the tree one level deeper.
    /// Recursive descent does: `((x))`, `[[x]]`, `f(f(x))` and a nested block
    /// each add a level. So does a left-folding loop: `1+1+1` is
    /// `(1+1)+1`, and a flat sum of 280,000 terms is a 280,000-deep tree even
    /// though the parser never recursed to build it. Counting only recursion
    /// would leave that sum to overflow the evaluator's stack instead.
    fn deepen(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            let line = self.line();
            bail!(error::nesting_too_deep(MAX_DEPTH).at(line));
        }
        Ok(())
    }

    fn peek(&self) -> &Token {
        &self.toks[self.pos]
    }

    /// The source line of the token about to be read.
    fn line(&self) -> u32 {
        self.lines.get(self.pos).copied().unwrap_or(1)
    }

    fn peek_at(&self, k: usize) -> &Token {
        self.toks.get(self.pos + k).unwrap_or(&Token::Eof)
    }

    fn next(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, t: &Token) -> bool {
        if self.peek() == t {
            self.next();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: Token) -> R<()> {
        if self.peek() == &t {
            self.next();
            Ok(())
        } else {
            let line = self.line();
            bail!(error::expected_token(&t, self.peek()).at(line))
        }
    }

    fn skip_terminators(&mut self) {
        while matches!(self.peek(), Token::Semi | Token::Comma | Token::Newline) {
            self.next();
        }
    }

    /// A whole source text: statements, then any `function` blocks.
    ///
    /// Once a function has been defined, only another may follow: a
    /// statement after one is MATLAB's "Function definitions in a script
    /// must appear at the end of the file.", reported on the statement's
    /// line. A function ends at its `end`, or, if it has none, at the next
    /// `function` or the end of the text; see [`Parser::parse_function`].
    pub fn parse_program(&mut self) -> R<Program> {
        let mut prog = Program {
            stmts: self.parse_block(&[Token::Function])?,
            functions: Vec::new(),
        };
        loop {
            self.skip_terminators();
            match self.peek() {
                Token::Eof => return Ok(prog),
                Token::Function => {
                    let f = self.parse_function()?;
                    prog.functions.push(f);
                }
                // An `end` with nothing open, after a function that took
                // its own.
                t @ (Token::End
                | Token::Else
                | Token::ElseIf
                | Token::Case
                | Token::Otherwise
                | Token::Catch) => {
                    let line = self.line();
                    bail!(error::block_with_no_opener(t).at(line));
                }
                _ => {
                    let line = self.line();
                    bail!(error::functions_at_end().at(line));
                }
            }
        }
    }

    /// `function [outs] = name(params)`, its body, and its `end` if it has
    /// one.
    ///
    /// The header is `function name`, `function name(a, b)`,
    /// `function y = name(...)` or `function [y, z] = name(...)`, with `~`
    /// allowed for a parameter. The body is read as a block that stops at
    /// `end` or at the next `function`: the blocks inside it take their own
    /// `end`s, so the first `end` at the body's own level is the function's.
    /// A body that stops at a `function` or at the end of the text had no
    /// `end`, which MATLAB allows in a function file.
    ///
    /// SplatCrab accepts a file that mixes the two forms, and a script whose
    /// functions have no `end`; MATLAB is stricter about both. Neither
    /// changes what a file that MATLAB accepts means.
    fn parse_function(&mut self) -> R<Function> {
        let line = self.line();
        self.expect(Token::Function)?;
        let mut outputs = Vec::new();
        if self.eat(&Token::LBracket) {
            loop {
                match self.next() {
                    Token::RBracket => break,
                    Token::Comma => {}
                    Token::Ident(n) => outputs.push(n),
                    t => bail!(error::unexpected_token(&t).at(line)),
                }
            }
            self.expect(Token::Assign)?;
        }
        let mut name = self.header_name(line)?;
        if outputs.is_empty() && self.eat(&Token::Assign) {
            outputs.push(name);
            name = self.header_name(line)?;
        }
        let mut params = Vec::new();
        if self.eat(&Token::LParen) {
            loop {
                match self.next() {
                    Token::RParen => break,
                    Token::Comma => {}
                    Token::Ident(n) => params.push(n),
                    Token::Not => params.push("~".to_string()),
                    t => bail!(error::unexpected_token(&t).at(line)),
                }
            }
        }
        match self.peek() {
            Token::Newline | Token::Semi | Token::Comma | Token::Eof => {}
            t => {
                let at = self.line();
                bail!(error::unexpected_token(t).at(at));
            }
        }
        self.deepen()?;
        let body = self.parse_block(&[Token::End, Token::Function]);
        self.depth -= 1;
        let body = body?;
        self.eat(&Token::End);
        Ok(Function {
            name,
            outputs,
            params,
            body,
            line,
        })
    }

    /// The function's name in its header.
    fn header_name(&mut self, line: u32) -> R<String> {
        match self.next() {
            Token::Ident(n) => Ok(n),
            t => bail!(error::unexpected_token(&t).at(line)),
        }
    }

    fn parse_block(&mut self, stops: &[Token]) -> R<Vec<Located>> {
        let mut out = Vec::new();
        loop {
            self.skip_terminators();
            let p = self.peek();
            if *p == Token::Eof || stops.contains(p) {
                return Ok(out);
            }
            // The line of the token that opens the statement is the line a
            // runtime error inside it reports.
            let line = self.line();
            // A block inside a block is a level of nesting like any other:
            // `if 1, if 1, ... end end` recurses through here once per level,
            // in the parser and again in `Interp::exec`.
            self.deepen()?;
            let stmt = self.parse_stmt()?;
            self.depth -= 1;
            out.push(Located { stmt, line });
        }
    }

    fn parse_stmt(&mut self) -> R<Stmt> {
        let line = self.line();
        match self.peek().clone() {
            Token::If => {
                self.next();
                let mut arms = Vec::new();
                // The condition's own line, not the statement's: they differ
                // for every `elseif`, and an error in one must name it.
                let cond_line = self.line();
                let cond = self.parse_expr()?;
                let body = self.parse_block(&[Token::End, Token::Else, Token::ElseIf])?;
                arms.push(IfArm {
                    cond,
                    line: cond_line,
                    body,
                });
                loop {
                    match self.next() {
                        Token::ElseIf => {
                            let cond_line = self.line();
                            let cond = self.parse_expr()?;
                            let body =
                                self.parse_block(&[Token::End, Token::Else, Token::ElseIf])?;
                            arms.push(IfArm {
                                cond,
                                line: cond_line,
                                body,
                            });
                        }
                        Token::Else => {
                            let body = self.parse_block(&[Token::End])?;
                            self.expect(Token::End)?;
                            return Ok(Stmt::If(arms, Some(body)));
                        }
                        Token::End => return Ok(Stmt::If(arms, None)),
                        t => bail!(error::expected_end_of_if(&t).at(line)),
                    }
                }
            }
            Token::For => {
                self.next();
                let name = match self.next() {
                    Token::Ident(n) => n,
                    t => bail!(error::expected_loop_variable(&t).at(line)),
                };
                self.expect(Token::Assign)?;
                let range = self.parse_expr()?;
                let body = self.parse_block(&[Token::End])?;
                self.expect(Token::End)?;
                Ok(Stmt::For(name, range, body))
            }
            Token::While => {
                self.next();
                let cond = self.parse_expr()?;
                let body = self.parse_block(&[Token::End])?;
                self.expect(Token::End)?;
                Ok(Stmt::While(cond, body))
            }
            Token::Break => {
                self.next();
                self.end_stmt()?;
                Ok(Stmt::Break)
            }
            Token::Continue => {
                self.next();
                self.end_stmt()?;
                Ok(Stmt::Continue)
            }
            Token::Return => {
                self.next();
                self.end_stmt()?;
                Ok(Stmt::Return)
            }
            // A definition inside a block: `if c, function f(), end, end`.
            // Only the top level of a file can hold one.
            Token::Function => bail!(error::function_not_supported_here().at(line)),
            Token::Switch => {
                self.next();
                let subject = self.parse_expr()?;
                self.skip_terminators();
                let mut arms = Vec::new();
                let mut otherwise = None;
                loop {
                    let arm_line = self.line();
                    match self.next() {
                        Token::Case if otherwise.is_none() => {
                            let values = if self.eat(&Token::LBrace) {
                                self.case_list()?
                            } else {
                                vec![self.parse_expr()?]
                            };
                            let body =
                                self.parse_block(&[Token::Case, Token::Otherwise, Token::End])?;
                            arms.push(CaseArm {
                                values,
                                line: arm_line,
                                body,
                            });
                        }
                        Token::Otherwise if otherwise.is_none() => {
                            otherwise = Some(self.parse_block(&[
                                Token::Case,
                                Token::Otherwise,
                                Token::End,
                            ])?);
                        }
                        Token::End => return Ok(Stmt::Switch(subject, arms, otherwise)),
                        t => bail!(error::expected_end_of("switch", &t).at(arm_line)),
                    }
                }
            }
            Token::Try => {
                self.next();
                let body = self.parse_block(&[Token::Catch, Token::End])?;
                let mut var = None;
                let mut handler = Vec::new();
                if self.eat(&Token::Catch) {
                    // An identifier straight after `catch`, on its line, is
                    // the name the error is bound to; a comma or a newline
                    // first means there is none.
                    if let Token::Ident(name) = self.peek().clone() {
                        self.next();
                        if !matches!(
                            self.peek(),
                            Token::Comma | Token::Semi | Token::Newline | Token::End | Token::Eof
                        ) {
                            let line = self.line();
                            bail!(error::unexpected_token(self.peek()).at(line));
                        }
                        var = Some(name);
                    }
                    handler = self.parse_block(&[Token::End])?;
                }
                self.expect(Token::End)?;
                Ok(Stmt::Try(body, var, handler))
            }
            Token::End
            | Token::Else
            | Token::ElseIf
            | Token::Case
            | Token::Otherwise
            | Token::Catch => {
                bail!(error::block_with_no_opener(self.peek()).at(line))
            }
            _ => {
                if self.peek() == &Token::LBracket {
                    if let Some(targets) = self.try_targets() {
                        // `try_targets` stopped on the `=`.
                        self.next();
                        let rhs = self.parse_expr()?;
                        let show = self.end_stmt()?;
                        return Ok(Stmt::MultiAssign(targets, rhs, show));
                    }
                }
                let e = self.parse_expr()?;
                if self.peek() == &Token::Assign {
                    self.next();
                    let rhs = self.parse_expr()?;
                    let show = self.end_stmt()?;
                    let target = match e {
                        Expr::Ident(name) => LValue {
                            name,
                            chain: Vec::new(),
                        },
                        Expr::Access(name, chain) => LValue { name, chain },
                        _ => bail!(error::invalid_assignment_target().at(line)),
                    };
                    Ok(Stmt::Assign(target, rhs, show))
                } else {
                    let show = self.end_stmt()?;
                    Ok(Stmt::Expr(e, show))
                }
            }
        }
    }

    /// The values of `case {a, b; c}`, after the `{`, through the `}`. The
    /// lexer treats these braces as it treats brackets, so whitespace, a
    /// comma, a semicolon and a newline all separate values; every one of
    /// them is a value to match, whatever the shape the separators give.
    fn case_list(&mut self) -> R<Vec<Expr>> {
        let mut values = Vec::new();
        loop {
            match self.peek() {
                Token::RBrace => {
                    self.next();
                    return Ok(values);
                }
                Token::Comma | Token::Semi => {
                    self.next();
                }
                Token::Eof => {
                    let line = self.line();
                    bail!(error::expected_token(&Token::RBrace, self.peek()).at(line))
                }
                _ => values.push(self.parse_expr()?),
            }
        }
    }

    /// The target list of `[a, ~, c] = ...`, when the statement starting at
    /// the `[` is one, leaving the parser on the `=`. Anything else, such as
    /// the matrix literal of `[a, b]` or `[1 2] == x`, rewinds to the `[` and
    /// returns `None`, so the statement is parsed as an expression and any
    /// error is reported by that parse, as it always was.
    ///
    /// A target is a name with an optional access chain, and a `~` alone
    /// between separators is a placeholder. The lexer has already turned
    /// the whitespace of `[a b]` and `[a ~]` into commas.
    fn try_targets(&mut self) -> Option<Vec<Option<LValue>>> {
        let (pos, depth, in_index) = (self.pos, self.depth, self.in_index);
        match self.targets() {
            Ok(Some(t)) => Some(t),
            _ => {
                self.pos = pos;
                self.depth = depth;
                self.in_index = in_index;
                None
            }
        }
    }

    fn targets(&mut self) -> R<Option<Vec<Option<LValue>>>> {
        self.expect(Token::LBracket)?;
        let mut out = Vec::new();
        loop {
            match self.next() {
                Token::Not if matches!(self.peek(), Token::Comma | Token::RBracket) => {
                    out.push(None)
                }
                Token::Ident(name) => {
                    let chain = self.parse_chain()?;
                    out.push(Some(LValue { name, chain }));
                }
                _ => return Ok(None),
            }
            match self.next() {
                Token::Comma => {}
                Token::RBracket => break,
                _ => return Ok(None),
            }
        }
        Ok((self.peek() == &Token::Assign).then_some(out))
    }

    /// Consume a statement terminator. Returns true if the result should be displayed.
    fn end_stmt(&mut self) -> R<bool> {
        match self.peek() {
            Token::Semi => {
                self.next();
                Ok(false)
            }
            Token::Comma | Token::Newline => {
                self.next();
                Ok(true)
            }
            Token::Eof
            | Token::End
            | Token::Else
            | Token::ElseIf
            | Token::Case
            | Token::Otherwise
            | Token::Catch
            | Token::Function => Ok(true),
            _ => {
                let line = self.line();
                bail!(error::unexpected_token(self.peek()).at(line))
            }
        }
    }

    // ---- expressions -------------------------------------------------

    /// Every nested expression enters here: a parenthesised group, a matrix
    /// element, an index or call argument, and the expression of a statement.
    /// That is what makes [`Parser::depth`] the nesting depth.
    pub fn parse_expr(&mut self) -> R<Expr> {
        self.deepen()?;
        let e = self.parse_oror();
        self.depth -= 1;
        e
    }

    /// The whole of a text that is one expression, with nothing after it
    /// but line ends: what `input` evaluates and `str2num` reads (cycle
    /// 11).
    pub fn parse_whole_expr(&mut self) -> R<Expr> {
        while matches!(self.peek(), Token::Newline) {
            self.pos += 1;
        }
        let e = self.parse_expr()?;
        while matches!(self.peek(), Token::Newline) {
            self.pos += 1;
        }
        match self.peek() {
            Token::Eof => Ok(e),
            t => {
                let line = self.line();
                bail!(error::unexpected_token(t).at(line))
            }
        }
    }

    fn parse_oror(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut left = self.parse_andand()?;
        while self.eat(&Token::OrOr) {
            self.deepen()?;
            let right = self.parse_andand()?;
            left = Expr::Binary(BinOp::OrOr, Box::new(left), Box::new(right));
        }
        self.depth = save;
        Ok(left)
    }

    fn parse_andand(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut left = self.parse_or()?;
        while self.eat(&Token::AndAnd) {
            self.deepen()?;
            let right = self.parse_or()?;
            left = Expr::Binary(BinOp::AndAnd, Box::new(left), Box::new(right));
        }
        self.depth = save;
        Ok(left)
    }

    fn parse_or(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut left = self.parse_and()?;
        while self.eat(&Token::Or) {
            self.deepen()?;
            let right = self.parse_and()?;
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(right));
        }
        self.depth = save;
        Ok(left)
    }

    fn parse_and(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut left = self.parse_cmp()?;
        while self.eat(&Token::And) {
            self.deepen()?;
            let right = self.parse_cmp()?;
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(right));
        }
        self.depth = save;
        Ok(left)
    }

    fn parse_cmp(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut left = self.parse_range()?;
        loop {
            let op = match self.peek() {
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.next();
            self.deepen()?;
            let right = self.parse_range()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        self.depth = save;
        Ok(left)
    }

    /// `a:b`, `a:s:b`, and any number of further colons after them.
    ///
    /// MATLAB reads `1:2:3:4` as `(1:2:3):4`: the first three operands form
    /// the stepped range, and each colon after that starts a new two-operand
    /// range whose start is everything so far. The loop is what makes that
    /// work; it used to handle two colons and stop, so `1:2:3:4` was a parse
    /// error.
    fn parse_range(&mut self) -> R<Expr> {
        // Bare `:` inside an index: `A(:, 1)`
        if self.in_index > 0
            && self.peek() == &Token::Colon
            && matches!(
                self.peek_at(1),
                Token::Comma | Token::RParen | Token::RBrace
            )
        {
            self.next();
            return Ok(Expr::Colon);
        }
        let save = self.depth;
        let mut e = self.parse_add()?;
        while self.eat(&Token::Colon) {
            self.deepen()?;
            let second = self.parse_add()?;
            // A third operand only joins the range being built now. After
            // `1:2:3` is closed, `:4` opens a fresh one around it.
            e = if self.eat(&Token::Colon) {
                let third = self.parse_add()?;
                Expr::Range(Box::new(e), Some(Box::new(second)), Box::new(third))
            } else {
                Expr::Range(Box::new(e), None, Box::new(second))
            };
        }
        self.depth = save;
        Ok(e)
    }

    fn parse_add(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.next();
            self.deepen()?;
            let right = self.parse_mul()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        self.depth = save;
        Ok(left)
    }

    fn parse_mul(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Backslash => BinOp::LDiv,
                Token::DotStar => BinOp::EMul,
                Token::DotSlash => BinOp::EDiv,
                Token::DotBackslash => BinOp::ELDiv,
                _ => break,
            };
            self.next();
            self.deepen()?;
            let right = self.parse_unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        self.depth = save;
        Ok(left)
    }

    fn parse_unary(&mut self) -> R<Expr> {
        // A prefix chain such as `----x` recurses once per sign.
        match self.peek() {
            Token::Minus => {
                self.deepen()?;
                self.next();
                let e = Expr::Neg(Box::new(self.parse_unary()?));
                self.depth -= 1;
                Ok(e)
            }
            Token::Plus => {
                self.deepen()?;
                self.next();
                let e = Expr::Pos(Box::new(self.parse_unary()?));
                self.depth -= 1;
                Ok(e)
            }
            Token::Not => {
                self.deepen()?;
                self.next();
                let e = Expr::Not(Box::new(self.parse_unary()?));
                self.depth -= 1;
                Ok(e)
            }
            _ => self.parse_power(),
        }
    }

    fn parse_power(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut base = self.parse_postfix()?;
        loop {
            let op = match self.peek() {
                Token::Caret => BinOp::Pow,
                Token::DotCaret => BinOp::EPow,
                _ => break,
            };
            self.next();
            self.deepen()?;
            // MATLAB allows a unary sign directly in the exponent: 2^-1
            let exp = self.parse_power_operand()?;
            base = Expr::Binary(op, Box::new(base), Box::new(exp));
        }
        self.depth = save;
        Ok(base)
    }

    fn parse_power_operand(&mut self) -> R<Expr> {
        match self.peek() {
            Token::Minus => {
                self.deepen()?;
                self.next();
                let e = Expr::Neg(Box::new(self.parse_power_operand()?));
                self.depth -= 1;
                Ok(e)
            }
            Token::Plus => {
                self.deepen()?;
                self.next();
                let e = Expr::Pos(Box::new(self.parse_power_operand()?));
                self.depth -= 1;
                Ok(e)
            }
            Token::Not => {
                self.deepen()?;
                self.next();
                let e = Expr::Not(Box::new(self.parse_power_operand()?));
                self.depth -= 1;
                Ok(e)
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> R<Expr> {
        let save = self.depth;
        let mut e = self.parse_primary()?;
        loop {
            let wrap: fn(Box<Expr>) -> Expr = if self.eat(&Token::Transpose) {
                Expr::Transpose
            } else if self.eat(&Token::DotTranspose) {
                Expr::DotTranspose
            } else {
                break;
            };
            self.deepen()?;
            e = wrap(Box::new(e));
        }
        self.depth = save;
        Ok(e)
    }

    fn parse_primary(&mut self) -> R<Expr> {
        let line = self.line();
        match self.next() {
            // A signed hexadecimal or binary literal with its top bit set
            // (cycle 16), `0xFFs8`, is the one number token that is
            // negative. It is the negation of its magnitude, the tree `-1`
            // makes, so it keeps a negation's precedence wherever it is
            // rendered back: `@() 0xFFs8^2` is `@()(-1)^2`, never `@()-1^2`,
            // which reads back as `-(1^2)`. The negation is one level of
            // nesting, as the sign of `-1` is, so the evaluator can walk
            // whatever this accepts.
            Token::Num(v) if v < 0.0 => {
                self.deepen()?;
                self.depth -= 1;
                Ok(Expr::Neg(Box::new(Expr::Num(-v))))
            }
            Token::Num(v) => Ok(Expr::Num(v)),
            Token::Imag(v) => Ok(Expr::Imag(v)),
            Token::Str(s) => Ok(Expr::Str(s)),
            Token::LParen => {
                let e = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(e)
            }
            Token::LBracket => self.parse_matrix(),
            // A cell literal (cycle 07). The lexer has already told it from
            // a brace index: this `{` does not follow the end of a value.
            Token::LBrace => self.parse_cell(),
            Token::End if self.in_index > 0 => Ok(Expr::End),
            Token::Ident(name) => {
                let chain = self.parse_chain()?;
                if chain.is_empty() {
                    Ok(Expr::Ident(name))
                } else {
                    Ok(Expr::Access(name, chain))
                }
            }
            Token::At => self.parse_handle_rest(line),
            // Anything else that cannot start a value.
            t => bail!(error::unexpected_in_expression(&t).at(line)),
        }
    }

    /// After an `@`: `@name`, or `@(params) body` (cycle 06). The body is
    /// one expression and runs as far as an expression can, so in
    /// `arrayfun(@(x) x * 2, v)` it stops at the comma. It is not an index
    /// argument even inside one, so `end` and a bare `:` mean nothing in it
    /// until an index of its own opens.
    fn parse_handle_rest(&mut self, line: u32) -> R<Expr> {
        match self.next() {
            Token::Ident(name) => Ok(Expr::FuncHandle(name)),
            Token::LParen => {
                let mut params = Vec::new();
                loop {
                    match self.next() {
                        Token::RParen if params.is_empty() => break,
                        Token::Ident(n) => params.push(n),
                        Token::Not => params.push("~".to_string()),
                        t => bail!(error::unexpected_token(&t).at(line)),
                    }
                    match self.next() {
                        Token::RParen => break,
                        Token::Comma => {}
                        t => bail!(error::unexpected_token(&t).at(line)),
                    }
                }
                let in_index = std::mem::take(&mut self.in_index);
                let body = self.parse_expr();
                self.in_index = in_index;
                Ok(Expr::AnonFn(Rc::new(AnonFn::new(params, body?))))
            }
            // An `@` followed by neither is the `@`'s error, as a bare `@`
            // was before handles existed.
            _ => bail!(error::unexpected_in_expression(&Token::At).at(line)),
        }
    }

    /// The whole of a `str2func` text that starts with `@`: one handle
    /// form and nothing after it (cycle 06).
    pub fn parse_handle(&mut self) -> R<Expr> {
        let line = self.line();
        self.expect(Token::At)?;
        let e = self.parse_handle_rest(line)?;
        match self.peek() {
            Token::Eof => Ok(e),
            t => {
                let line = self.line();
                bail!(error::unexpected_token(t).at(line))
            }
        }
    }

    /// The accesses after a name, in the order written: `(args)`, `{args}`,
    /// `.name` and `.(expr)`, as many as follow. The chain is a flat list, so
    /// a long one deepens nothing; each argument is an expression of its own
    /// and is counted there.
    fn parse_chain(&mut self) -> R<Vec<Access>> {
        let mut chain = Vec::new();
        loop {
            match self.peek() {
                Token::LParen => {
                    self.next();
                    chain.push(Access::Paren(self.parse_args(Token::RParen)?));
                }
                Token::LBrace => {
                    self.next();
                    chain.push(Access::Brace(self.parse_args(Token::RBrace)?));
                }
                Token::Dot => {
                    self.next();
                    let line = self.line();
                    match self.next() {
                        Token::Ident(field) => chain.push(Access::Field(field)),
                        Token::LParen => {
                            // A dynamic field name is not an index argument,
                            // so `end` and a bare `:` mean nothing in it,
                            // even inside an index further out.
                            let in_index = std::mem::take(&mut self.in_index);
                            let e = self.parse_expr();
                            self.in_index = in_index;
                            let e = e?;
                            self.expect(Token::RParen)?;
                            chain.push(Access::DynField(Box::new(e)));
                        }
                        t => bail!(error::unexpected_in_expression(&t).at(line)),
                    }
                }
                _ => return Ok(chain),
            }
        }
    }

    /// The arguments of `(...)` or `{...}`, after the opener, through the
    /// closer. `end` and a bare `:` are legal inside.
    fn parse_args(&mut self, close: Token) -> R<Vec<Expr>> {
        self.in_index += 1;
        let args = self.arg_list(&close);
        self.in_index -= 1;
        let args = args?;
        self.expect(close)?;
        Ok(args)
    }

    fn arg_list(&mut self, close: &Token) -> R<Vec<Expr>> {
        let mut args = Vec::new();
        if self.peek() != close {
            loop {
                args.push(self.parse_expr()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
        }
        Ok(args)
    }

    fn parse_matrix(&mut self) -> R<Expr> {
        let mut rows: Vec<Vec<Expr>> = Vec::new();
        let mut row: Vec<Expr> = Vec::new();
        loop {
            match self.peek() {
                Token::RBracket => {
                    self.next();
                    if !row.is_empty() {
                        rows.push(row);
                    }
                    return Ok(Expr::Matrix(rows));
                }
                Token::Semi => {
                    self.next();
                    if !row.is_empty() {
                        rows.push(std::mem::take(&mut row));
                    }
                }
                Token::Comma => {
                    self.next();
                }
                Token::Eof => {
                    let line = self.line();
                    bail!(error::unterminated_matrix().at(line))
                }
                _ => {
                    let line = self.line();
                    let e = self.parse_expr()?;
                    // A handle is one function, never an element of an
                    // array: `[@(x) x+1]` and `[@sin 1]` are refused here,
                    // where the source says so (cycle 06).
                    if matches!(e, Expr::AnonFn(_) | Expr::FuncHandle(_)) {
                        bail!(error::handle_concatenation().at(line));
                    }
                    row.push(e)
                }
            }
        }
    }

    /// The rows of a cell literal `{...}`, after the `{`, through the `}`
    /// (cycle 07). The separators are a bracket's: the lexer has turned the
    /// whitespace between elements into commas and the newlines between
    /// rows into semicolons. Unlike a bracket, a cell literal may hold a
    /// handle, `{@(x) x + 1, 2}`: each element is one value of the cell.
    fn parse_cell(&mut self) -> R<Expr> {
        let mut rows: Vec<Vec<Expr>> = Vec::new();
        let mut row: Vec<Expr> = Vec::new();
        loop {
            match self.peek() {
                Token::RBrace => {
                    self.next();
                    if !row.is_empty() {
                        rows.push(row);
                    }
                    return Ok(Expr::Cell(rows));
                }
                Token::Semi => {
                    self.next();
                    if !row.is_empty() {
                        rows.push(std::mem::take(&mut row));
                    }
                }
                Token::Comma => {
                    self.next();
                }
                Token::Eof => {
                    let line = self.line();
                    bail!(error::expected_token(&Token::RBrace, self.peek()).at(line))
                }
                _ => {
                    row.push(self.parse_expr()?);
                    // An element ends at a separator or the closer. What
                    // else can follow one is what an anonymous function's
                    // body left, whose spaces separate nothing: the `1` of
                    // `{@(x) x 1}`.
                    if !matches!(
                        self.peek(),
                        Token::Comma | Token::Semi | Token::RBrace | Token::Eof
                    ) {
                        let line = self.line();
                        bail!(error::unexpected_token(self.peek()).at(line));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{lex, scan};

    fn parse_expr(src: &str) -> Expr {
        let toks = lex(src).expect("lex should succeed");
        let mut p = Parser::new(toks);
        p.parse_expr().expect("expression should parse")
    }

    fn parse(src: &str) -> Vec<Located> {
        parse_result(src).expect("program should parse")
    }

    fn parse_result(src: &str) -> R<Vec<Located>> {
        let lexed = scan(src).expect("lex should succeed");
        let mut p = Parser::with_lines(lexed);
        p.parse_program().map(|prog| {
            assert!(prog.functions.is_empty(), "no functions expected");
            prog.stmts
        })
    }

    /// A statement and the line it is expected to start on.
    fn at(line: u32, stmt: Stmt) -> Located {
        Located { stmt, line }
    }

    /// An `if` arm: its condition, the line that condition is on, and its body.
    fn arm(cond: Expr, line: u32, body: Vec<Located>) -> IfArm {
        IfArm { cond, line, body }
    }

    fn num(v: f64) -> Expr {
        Expr::Num(v)
    }

    fn ident(s: &str) -> Expr {
        Expr::Ident(s.to_string())
    }

    fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
        Expr::Binary(op, Box::new(l), Box::new(r))
    }

    fn neg(e: Expr) -> Expr {
        Expr::Neg(Box::new(e))
    }

    fn not(e: Expr) -> Expr {
        Expr::Not(Box::new(e))
    }

    fn tr(e: Expr) -> Expr {
        Expr::Transpose(Box::new(e))
    }

    fn range(a: Expr, step: Option<Expr>, b: Expr) -> Expr {
        Expr::Range(Box::new(a), step.map(Box::new), Box::new(b))
    }

    /// A plain assignment target.
    fn lv(name: &str) -> LValue {
        LValue {
            name: name.to_string(),
            chain: Vec::new(),
        }
    }

    /// `name(args)`, as the parser builds it: an access chain of one `(...)`.
    fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::Access(name.to_string(), vec![Access::Paren(args)])
    }

    // ---- precedence and associativity ---------------------------------

    #[test]
    fn unary_minus_binds_looser_than_power() {
        assert_eq!(parse_expr("-2^2"), neg(bin(BinOp::Pow, num(2.0), num(2.0))));
    }

    #[test]
    fn unary_sign_is_allowed_in_the_exponent() {
        assert_eq!(parse_expr("2^-1"), bin(BinOp::Pow, num(2.0), neg(num(1.0))));
        assert_eq!(
            parse_expr("2^+1"),
            bin(BinOp::Pow, num(2.0), Expr::Pos(Box::new(num(1.0))))
        );
    }

    #[test]
    fn power_is_left_associative() {
        assert_eq!(
            parse_expr("2^3^2"),
            bin(BinOp::Pow, bin(BinOp::Pow, num(2.0), num(3.0)), num(2.0))
        );
    }

    #[test]
    fn transpose_binds_tighter_than_multiplication() {
        assert_eq!(
            parse_expr("a'*b"),
            bin(BinOp::Mul, tr(ident("a")), ident("b"))
        );
    }

    #[test]
    fn transpose_binds_tighter_than_unary_minus() {
        assert_eq!(parse_expr("-a'"), neg(tr(ident("a"))));
    }

    #[test]
    fn repeated_transpose_nests() {
        assert_eq!(parse_expr("a''"), tr(tr(ident("a"))));
    }

    #[test]
    fn not_binds_tighter_than_comparison() {
        assert_eq!(
            parse_expr("~a == b"),
            bin(BinOp::Eq, not(ident("a")), ident("b"))
        );
    }

    #[test]
    fn comparisons_are_left_associative() {
        assert_eq!(
            parse_expr("a == b < c"),
            bin(
                BinOp::Lt,
                bin(BinOp::Eq, ident("a"), ident("b")),
                ident("c")
            )
        );
    }

    #[test]
    fn oror_is_looser_than_andand() {
        assert_eq!(
            parse_expr("a || b && c"),
            bin(
                BinOp::OrOr,
                ident("a"),
                bin(BinOp::AndAnd, ident("b"), ident("c"))
            )
        );
    }

    #[test]
    fn or_is_looser_than_and() {
        assert_eq!(
            parse_expr("a | b & c"),
            bin(
                BinOp::Or,
                ident("a"),
                bin(BinOp::And, ident("b"), ident("c"))
            )
        );
    }

    #[test]
    fn addition_is_left_associative() {
        assert_eq!(
            parse_expr("a - b - c"),
            bin(
                BinOp::Sub,
                bin(BinOp::Sub, ident("a"), ident("b")),
                ident("c")
            )
        );
    }

    #[test]
    fn multiplication_is_left_associative() {
        assert_eq!(
            parse_expr("a/b*c"),
            bin(
                BinOp::Mul,
                bin(BinOp::Div, ident("a"), ident("b")),
                ident("c")
            )
        );
    }

    #[test]
    fn elementwise_power_binds_tighter_than_elementwise_product() {
        assert_eq!(
            parse_expr("a.*b./c.^d"),
            bin(
                BinOp::EDiv,
                bin(BinOp::EMul, ident("a"), ident("b")),
                bin(BinOp::EPow, ident("c"), ident("d"))
            )
        );
    }

    #[test]
    fn elementwise_left_divide_sits_with_the_other_products() {
        assert_eq!(
            parse_expr("a.\\b"),
            bin(BinOp::ELDiv, ident("a"), ident("b"))
        );
        // Same precedence level as `.*`, so it is left associative with it.
        assert_eq!(
            parse_expr("a.\\b.*c"),
            bin(
                BinOp::EMul,
                bin(BinOp::ELDiv, ident("a"), ident("b")),
                ident("c")
            )
        );
        // `.\` binds looser than `.^`.
        assert_eq!(
            parse_expr("a.\\b.^c"),
            bin(
                BinOp::ELDiv,
                ident("a"),
                bin(BinOp::EPow, ident("b"), ident("c"))
            )
        );
    }

    #[test]
    fn range_endpoint_absorbs_addition() {
        assert_eq!(
            parse_expr("1:3+1"),
            range(num(1.0), None, bin(BinOp::Add, num(3.0), num(1.0)))
        );
    }

    #[test]
    fn range_with_step() {
        assert_eq!(
            parse_expr("1:2:9"),
            range(num(1.0), Some(num(2.0)), num(9.0))
        );
    }

    /// MATLAB reads `1:2:3:4` as `(1:2:3):4`: the first three operands make
    /// the stepped range, and each colon after that opens a fresh
    /// two-operand range around everything so far. It used to be a parse
    /// error, because `parse_range` handled two colons and did not loop.
    #[test]
    fn a_chain_of_colons_folds_to_the_left() {
        assert_eq!(
            parse_expr("1:2:3:4"),
            range(range(num(1.0), Some(num(2.0)), num(3.0)), None, num(4.0))
        );
        // Spelling the grouping out gives the same tree.
        assert_eq!(parse_expr("1:2:3:4"), parse_expr("(1:2:3):4"));
        // Past four operands the rule keeps applying to what has been built:
        // the range closed by `1:2:3` becomes the start of the next one, and
        // that one takes a step of its own if a further colon offers it.
        assert_eq!(parse_expr("1:2:3:4:5"), parse_expr("(1:2:3):4:5"));
        assert_eq!(parse_expr("1:2:3:4:5:6"), parse_expr("((1:2:3):4:5):6"));
        // Two colons and one colon are unchanged.
        assert_eq!(parse_expr("1:3"), range(num(1.0), None, num(3.0)));
        assert_eq!(
            parse_expr("1:2:9"),
            range(num(1.0), Some(num(2.0)), num(9.0))
        );
    }

    #[test]
    fn comparison_is_looser_than_range() {
        assert_eq!(
            parse_expr("1:3 == x"),
            bin(BinOp::Eq, range(num(1.0), None, num(3.0)), ident("x"))
        );
    }

    #[test]
    fn parentheses_override_precedence() {
        assert_eq!(
            parse_expr("(1+2)*3"),
            bin(BinOp::Mul, bin(BinOp::Add, num(1.0), num(2.0)), num(3.0))
        );
    }

    // ---- matrix literals ------------------------------------------------

    #[test]
    fn matrix_rows_split_on_semicolon() {
        assert_eq!(
            parse_expr("[1 2; 3 4]"),
            Expr::Matrix(vec![vec![num(1.0), num(2.0)], vec![num(3.0), num(4.0)],])
        );
    }

    #[test]
    fn matrix_whitespace_rule_reaches_the_parser() {
        assert_eq!(
            parse_expr("[1 -2]"),
            Expr::Matrix(vec![vec![num(1.0), neg(num(2.0))]])
        );
        assert_eq!(
            parse_expr("[1 - 2]"),
            Expr::Matrix(vec![vec![bin(BinOp::Sub, num(1.0), num(2.0))]])
        );
    }

    #[test]
    fn empty_matrix_has_no_rows() {
        assert_eq!(parse_expr("[]"), Expr::Matrix(vec![]));
    }

    // ---- statements -----------------------------------------------------

    #[test]
    fn semicolon_suppresses_display() {
        assert_eq!(
            parse("x = 3;"),
            vec![at(1, Stmt::Assign(lv("x"), num(3.0), false))]
        );
    }

    #[test]
    fn missing_semicolon_shows_result() {
        assert_eq!(
            parse("x = 3"),
            vec![at(1, Stmt::Assign(lv("x"), num(3.0), true))]
        );
    }

    #[test]
    fn indexed_assignment_uses_index_assign() {
        assert_eq!(
            parse("x(end+1) = 3"),
            vec![at(
                1,
                Stmt::Assign(
                    LValue {
                        name: "x".to_string(),
                        chain: vec![Access::Paren(vec![bin(BinOp::Add, Expr::End, num(1.0))])],
                    },
                    num(3.0),
                    true,
                )
            )]
        );
    }

    #[test]
    fn bare_colon_in_an_index_is_expr_colon() {
        assert_eq!(
            parse("A(:, 1)"),
            vec![at(
                1,
                Stmt::Expr(call("A", vec![Expr::Colon, num(1.0)]), true)
            )]
        );
    }

    #[test]
    fn end_inside_an_index_is_expr_end() {
        assert_eq!(parse_expr("a(end)"), call("a", vec![Expr::End]));
    }

    #[test]
    fn expression_statement() {
        assert_eq!(
            parse("1 + 2"),
            vec![at(1, Stmt::Expr(bin(BinOp::Add, num(1.0), num(2.0)), true))]
        );
    }

    #[test]
    fn statements_separated_by_newlines_and_semicolons() {
        assert_eq!(
            parse("x = 1;\ny = 2\n"),
            vec![
                at(1, Stmt::Assign(lv("x"), num(1.0), false)),
                at(2, Stmt::Assign(lv("y"), num(2.0), true)),
            ]
        );
    }

    #[test]
    fn call_with_several_arguments() {
        assert_eq!(
            parse("disp(1, x);"),
            vec![at(
                1,
                Stmt::Expr(call("disp", vec![num(1.0), ident("x")]), false)
            )]
        );
    }

    // ---- blocks ----------------------------------------------------------

    #[test]
    fn if_elseif_else_block() {
        assert_eq!(
            parse("if a\n  1;\nelseif b\n  2;\nelse\n  3;\nend\n"),
            vec![at(
                1,
                Stmt::If(
                    vec![
                        arm(ident("a"), 1, vec![at(2, Stmt::Expr(num(1.0), false))]),
                        // The `elseif` condition carries line 3, its own.
                        arm(ident("b"), 3, vec![at(4, Stmt::Expr(num(2.0), false))]),
                    ],
                    Some(vec![at(6, Stmt::Expr(num(3.0), false))]),
                )
            )]
        );
    }

    #[test]
    fn if_without_else_has_no_tail() {
        assert_eq!(
            parse("if a\n  1;\nend"),
            vec![at(
                1,
                Stmt::If(
                    vec![arm(ident("a"), 1, vec![at(2, Stmt::Expr(num(1.0), false))])],
                    None
                )
            )]
        );
    }

    #[test]
    fn for_loop_with_comma_separators() {
        assert_eq!(
            parse("for k = 1:3, disp(k); end"),
            vec![at(
                1,
                Stmt::For(
                    "k".to_string(),
                    range(num(1.0), None, num(3.0)),
                    vec![at(1, Stmt::Expr(call("disp", vec![ident("k")]), false))],
                )
            )]
        );
    }

    #[test]
    fn while_loop_with_break() {
        assert_eq!(
            parse("while a\nbreak\nend"),
            vec![at(1, Stmt::While(ident("a"), vec![at(2, Stmt::Break)]))]
        );
    }

    #[test]
    fn while_loop_with_continue() {
        assert_eq!(
            parse("while a\ncontinue;\nend"),
            vec![at(1, Stmt::While(ident("a"), vec![at(2, Stmt::Continue)]))]
        );
    }

    #[test]
    fn nested_blocks() {
        assert_eq!(
            parse("for i = 1:2\n  if i > 1\n    break\n  end\nend"),
            vec![at(
                1,
                Stmt::For(
                    "i".to_string(),
                    range(num(1.0), None, num(2.0)),
                    vec![at(
                        2,
                        Stmt::If(
                            vec![arm(
                                bin(BinOp::Gt, ident("i"), num(1.0)),
                                2,
                                vec![at(3, Stmt::Break)],
                            )],
                            None
                        )
                    )],
                )
            )]
        );
    }

    // ---- errors ------------------------------------------------------------

    #[test]
    fn end_outside_an_index_is_an_error() {
        assert!(parse_result("end").is_err());
        assert!(parse_result("x = end").is_err());
    }

    #[test]
    fn unterminated_matrix_literal_is_an_error() {
        assert!(parse_result("[1 2").is_err());
        assert!(parse_result("[1 2; 3").is_err());
    }

    #[test]
    fn indexing_a_matrix_literal_is_an_error() {
        assert!(parse_result("[1 2](1)").is_err());
    }

    #[test]
    fn invalid_assignment_target_is_an_error() {
        assert!(parse_result("1 = 2").is_err());
        assert!(parse_result("a + b = 2").is_err());
    }

    #[test]
    fn unclosed_block_is_an_error() {
        assert!(parse_result("if a\n1;").is_err());
        assert!(parse_result("for k = 1:3\ndisp(k);").is_err());
    }

    #[test]
    fn unmatched_paren_is_an_error() {
        assert!(parse_result("(1 + 2").is_err());
    }

    /// A parse message names the token the way it was written, not by its
    /// Rust variant: `unexpected ';'`, never `unexpected Semi`.
    #[test]
    fn a_parse_error_names_the_token_as_it_was_written() {
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("y = x + ;"), "unexpected ';' in expression");
        assert_eq!(msg("y = x + "), "unexpected end of input in expression");
        assert_eq!(msg("y = x + \n"), "unexpected end of line in expression");
        assert_eq!(msg("y = x + )"), "unexpected ')' in expression");
        assert_eq!(msg("y = (1 + 2;"), "expected ')' but found ';'");
        assert_eq!(msg("end"), "unexpected 'end' with no matching block");
        assert_eq!(
            msg("for 1 = 1:2\nend"),
            "expected loop variable after 'for', found '1'"
        );
        // A number shows its value and an identifier its name. `00x1F` lexes
        // as `00` then the identifier `x1F`, the message that once read
        // `unexpected Ident("x1F")`. It was `0x1F` until cycle 16 made that
        // a hexadecimal literal; a `0` before it keeps the old reading.
        assert_eq!(msg("x = 00x1F"), "unexpected 'x1F'");
        assert_eq!(msg("x = 1 2"), "unexpected '2'");
        assert_eq!(msg("x = 1 0.3"), "unexpected '0.3'");
        // Nothing in a message says `Token` or a variant name any more.
        for src in ["y = x + ;", "end", "x = 00x1F", "y = (1 + 2;"] {
            let m = msg(src);
            assert!(!m.contains("Semi") && !m.contains("Ident("), "{src}: {m}");
        }
    }

    /// The depth counter measures nesting, not size: it is restored as each
    /// construct closes, so a long flat program never approaches the limit
    /// however many statements and operators it contains. The boundary
    /// itself is tested in `interp.rs`, where the test can run on the stack
    /// the interpreter actually uses.
    #[test]
    fn depth_is_nesting_and_not_program_length() {
        let many = vec!["x = f(1) + [2 3] * 4 - (5 + 6);"; 2000].join("\n");
        assert!(parse_result(&many).is_ok());
        // Sibling expressions inside one statement do not accumulate either.
        let args = vec!["(1 + 2)"; 2000].join(", ");
        assert!(parse_result(&format!("x = f({args});")).is_ok());
        let row = vec!["(1 + 2)"; 2000].join(" ");
        assert!(parse_result(&format!("x = [{row}];")).is_ok());
    }

    // ---- line numbers ------------------------------------------------------

    fn lines_of(src: &str) -> Vec<u32> {
        parse(src).iter().map(|s| s.line).collect()
    }

    #[test]
    fn a_statement_takes_the_line_of_the_token_that_opens_it() {
        assert_eq!(lines_of("x = 1;\n\ny = 2;\nz = 3;"), vec![1, 3, 4]);
        // A statement spread over several lines is located at its first.
        assert_eq!(lines_of("x = 1 + ...\n2;\ny = 3;"), vec![1, 3]);
        // Several statements on one line all share it.
        assert_eq!(lines_of("a = 1; b = 2; c = 3;"), vec![1, 1, 1]);
    }

    #[test]
    fn a_block_body_keeps_its_own_lines() {
        let stmts = parse("for k = 1:2\n  disp(k);\n  disp(k);\nend");
        assert_eq!(stmts[0].line, 1);
        match &stmts[0].stmt {
            Stmt::For(_, _, body) => {
                assert_eq!(body.iter().map(|s| s.line).collect::<Vec<_>>(), vec![2, 3])
            }
            other => panic!("expected a for loop, got {other:?}"),
        }
    }

    #[test]
    fn a_parse_error_names_the_line_of_the_offending_token() {
        assert_eq!(parse_result("x = 5;\ny = x + ;").unwrap_err().line, Some(2));
        assert_eq!(parse_result("x = 1;\n\n1 = 2").unwrap_err().line, Some(3));
        assert_eq!(parse_result("x = 1;\nend").unwrap_err().line, Some(2));
        assert_eq!(parse_result("x = 1;\ny = [1 2").unwrap_err().line, Some(2));
    }

    #[test]
    fn tokens_without_lines_still_parse_and_report_line_one() {
        // `Parser::new` is what the expression tests and the REPL's
        // `needs_more` use; with no line table every statement is line 1.
        let toks = lex("x = 1;\ny = 2;").expect("lex should succeed");
        let stmts = Parser::new(toks)
            .parse_program()
            .expect("should parse")
            .stmts;
        assert_eq!(stmts.iter().map(|s| s.line).collect::<Vec<_>>(), vec![1, 1]);
    }

    // ---- access chains and multiple assignment (cycle 03) --------------

    fn access(name: &str, chain: Vec<Access>) -> Expr {
        Expr::Access(name.to_string(), chain)
    }

    /// Acceptance test 14: each form of access, and a chain of all three.
    #[test]
    fn access_chains_parse_in_the_order_written() {
        assert_eq!(
            parse_expr("x{2}"),
            access("x", vec![Access::Brace(vec![num(2.0)])])
        );
        assert_eq!(
            parse_expr("s.a"),
            access("s", vec![Access::Field("a".to_string())])
        );
        assert_eq!(
            parse_expr("s.(n)"),
            access("s", vec![Access::DynField(Box::new(ident("n")))])
        );
        assert_eq!(
            parse_expr("c{1}(2).b"),
            access(
                "c",
                vec![
                    Access::Brace(vec![num(1.0)]),
                    Access::Paren(vec![num(2.0)]),
                    Access::Field("b".to_string()),
                ]
            )
        );
        // `end` and a bare `:` are index arguments inside braces too.
        assert_eq!(
            parse_expr("c{end, :}"),
            access("c", vec![Access::Brace(vec![Expr::End, Expr::Colon])])
        );
        // A plain call is a one-link chain, and a bare name no chain at all.
        assert_eq!(parse_expr("f(1)"), call("f", vec![num(1.0)]));
        assert_eq!(parse_expr("f()"), call("f", vec![]));
        assert_eq!(parse_expr("f"), ident("f"));
        // Postfix operators apply to the whole chain.
        assert_eq!(
            parse_expr("s.a'"),
            tr(access("s", vec![Access::Field("a".to_string())]))
        );
        assert_eq!(
            parse_expr("-c{1}.^2"),
            neg(bin(
                BinOp::EPow,
                access("c", vec![Access::Brace(vec![num(1.0)])]),
                num(2.0)
            ))
        );
    }

    #[test]
    fn a_dynamic_field_is_not_an_index_argument() {
        // `end` inside `.( )` is not the `end` of an enclosing index.
        assert!(parse_result("x(s.(end))").is_err());
        assert_eq!(
            parse_expr("x(s.(n), end)"),
            call(
                "x",
                vec![
                    access("s", vec![Access::DynField(Box::new(ident("n")))]),
                    Expr::End
                ]
            )
        );
    }

    #[test]
    fn a_dot_must_be_followed_by_a_name_or_a_parenthesis() {
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("s.1"), "unexpected '0.1'");
        assert_eq!(msg("s.;"), "unexpected ';' in expression");
        assert_eq!(msg("s.end"), "unexpected 'end' in expression");
    }

    /// An `@` that starts no handle is a clean parse error naming it; since
    /// cycle 06, `@name` and `@(x) body` are handles.
    #[test]
    fn a_bare_at_is_a_parse_error() {
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("f = @"), "unexpected '@' in expression");
        assert_eq!(msg("f = @3"), "unexpected '@' in expression");
        assert_eq!(msg("@"), "unexpected '@' in expression");
        assert_eq!(msg("x = 1 @ 2"), "unexpected '@'");
        // Since cycle 07 a brace opens a value, a cell literal, which must
        // be closed.
        assert!(parse_result("c = {1}").is_ok());
        assert_eq!(msg("c = {1"), "expected '}' but found end of input");
    }

    #[test]
    fn a_chain_is_an_assignment_target() {
        assert_eq!(
            parse("c{1}(2).b = 5;"),
            vec![at(
                1,
                Stmt::Assign(
                    LValue {
                        name: "c".to_string(),
                        chain: vec![
                            Access::Brace(vec![num(1.0)]),
                            Access::Paren(vec![num(2.0)]),
                            Access::Field("b".to_string()),
                        ],
                    },
                    num(5.0),
                    false
                )
            )]
        );
    }

    fn target(name: &str, chain: Vec<Access>) -> Option<LValue> {
        Some(LValue {
            name: name.to_string(),
            chain,
        })
    }

    /// Acceptance test 14: `[a, ~, c] = f(x)` is one statement with three
    /// targets, the middle one a placeholder.
    #[test]
    fn a_bracketed_target_list_is_a_multi_assign() {
        let three = Stmt::MultiAssign(
            vec![target("a", vec![]), None, target("c", vec![])],
            call("f", vec![ident("x")]),
            true,
        );
        assert_eq!(parse("[a, ~, c] = f(x)"), vec![at(1, three.clone())]);
        // Whitespace separates targets as it separates elements.
        assert_eq!(parse("[a ~, c] = f(x)"), vec![at(1, three.clone())]);
        assert_eq!(parse("[a,~,c]=f(x)"), vec![at(1, three)]);
        // `[a ~ c]` is the matrix `[a, ~c]`, as in a literal: the lexer puts
        // no separator after a `~`, so it is not a target list.
        assert!(parse_result("[a ~ c] = f(x)").is_err());
        // The one-target form, and suppression.
        assert_eq!(
            parse("[x] = size(A, 1);"),
            vec![at(
                1,
                Stmt::MultiAssign(
                    vec![target("x", vec![])],
                    call("size", vec![ident("A"), num(1.0)]),
                    false
                )
            )]
        );
        // A target may be indexed.
        assert_eq!(
            parse("[v(2), ~] = max(w);"),
            vec![at(
                1,
                Stmt::MultiAssign(
                    vec![target("v", vec![Access::Paren(vec![num(2.0)])]), None],
                    call("max", vec![ident("w")]),
                    false
                )
            )]
        );
    }

    /// A bracket that is not a target list is the matrix literal it always
    /// was, and its errors are the ones it always had.
    #[test]
    fn a_bracket_that_is_not_a_target_list_is_an_expression() {
        assert_eq!(
            parse("[a, b]"),
            vec![at(
                1,
                Stmt::Expr(Expr::Matrix(vec![vec![ident("a"), ident("b")]]), true)
            )]
        );
        assert_eq!(
            parse("[a b] == c;"),
            vec![at(
                1,
                Stmt::Expr(
                    bin(
                        BinOp::Eq,
                        Expr::Matrix(vec![vec![ident("a"), ident("b")]]),
                        ident("c")
                    ),
                    false
                )
            )]
        );
        assert_eq!(
            parse("[~a, b]"),
            vec![at(
                1,
                Stmt::Expr(Expr::Matrix(vec![vec![not(ident("a")), ident("b")]]), true)
            )]
        );
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("[1, b] = 2"), "invalid assignment target");
        assert_eq!(msg("[a; b] = 2"), "invalid assignment target");
        assert_eq!(msg("[] = 2"), "invalid assignment target");
        assert_eq!(msg("[a + 1, b] = 2"), "invalid assignment target");
        assert_eq!(msg("[a, b"), "unterminated matrix literal: missing ']'");
    }

    // ---- switch and try (cycle 04) --------------------------------------

    fn show(e: Expr) -> Stmt {
        Stmt::Expr(call("disp", vec![e]), true)
    }

    #[test]
    fn a_switch_has_its_cases_in_order_and_an_otherwise() {
        assert_eq!(
            parse("switch x\ncase 1\ndisp(1)\ncase {2, 3}\ndisp(2)\notherwise\ndisp(0)\nend"),
            vec![at(
                1,
                Stmt::Switch(
                    ident("x"),
                    vec![
                        CaseArm {
                            values: vec![num(1.0)],
                            line: 2,
                            body: vec![at(3, show(num(1.0)))],
                        },
                        CaseArm {
                            values: vec![num(2.0), num(3.0)],
                            line: 4,
                            body: vec![at(5, show(num(2.0)))],
                        },
                    ],
                    Some(vec![at(7, show(num(0.0)))]),
                )
            )]
        );
    }

    #[test]
    fn a_switch_on_one_line_and_case_lists_of_every_spelling() {
        let stmts = parse("switch s, case {'a' 'b'; 'c'}, x = 1; end");
        match &stmts[0].stmt {
            Stmt::Switch(subject, arms, None) => {
                assert_eq!(*subject, ident("s"));
                assert_eq!(arms.len(), 1);
                assert_eq!(
                    arms[0].values,
                    vec![
                        Expr::Str("a".into()),
                        Expr::Str("b".into()),
                        Expr::Str("c".into())
                    ]
                );
                assert_eq!(
                    arms[0].body,
                    vec![at(1, Stmt::Assign(lv("x"), num(1.0), false))]
                );
            }
            other => panic!("expected a switch, got {other:?}"),
        }
        // No arms at all is a switch that does nothing.
        assert_eq!(
            parse("switch x\nend"),
            vec![at(1, Stmt::Switch(ident("x"), vec![], None))]
        );
    }

    #[test]
    fn a_switch_refuses_what_its_arms_cannot_hold() {
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(
            msg("switch x\ndisp(1)\nend"),
            "expected 'end' to close 'switch', found 'disp'"
        );
        assert_eq!(
            msg("switch x\notherwise\ncase 1\nend"),
            "expected 'end' to close 'switch', found 'case'"
        );
        assert_eq!(
            msg("switch x\notherwise\notherwise\nend"),
            "expected 'end' to close 'switch', found 'otherwise'"
        );
        // A newline inside the braces separates values, as in a bracket, so
        // the `end` is read as one.
        assert_eq!(
            msg("switch x\ncase {1, 2\nend"),
            "unexpected 'end' in expression"
        );
        assert_eq!(
            msg("switch x\ncase {1, 2"),
            "expected '}' but found end of input"
        );
        assert_eq!(msg("case 1"), "unexpected 'case' with no matching block");
        assert_eq!(
            msg("otherwise"),
            "unexpected 'otherwise' with no matching block"
        );
        assert_eq!(msg("catch"), "unexpected 'catch' with no matching block");
    }

    #[test]
    fn a_catch_on_its_line_binds_an_identifier() {
        let body = vec![at(2, show(num(1.0)))];
        let handler = vec![at(4, show(ident("e")))];
        assert_eq!(
            parse("try\ndisp(1)\ncatch e\ndisp(e)\nend"),
            vec![at(
                1,
                Stmt::Try(body.clone(), Some("e".into()), handler.clone())
            )]
        );
        // A newline or a comma after `catch` binds nothing, and what follows
        // is the handler's first statement.
        assert_eq!(
            parse("try\ndisp(1)\ncatch\ne\nend"),
            vec![at(
                1,
                Stmt::Try(
                    body.clone(),
                    None,
                    vec![at(4, Stmt::Expr(ident("e"), true))]
                )
            )]
        );
        assert_eq!(
            parse("try, x = 1; catch, disp(2), end"),
            vec![at(
                1,
                Stmt::Try(
                    vec![at(1, Stmt::Assign(lv("x"), num(1.0), false))],
                    None,
                    vec![at(1, show(num(2.0)))]
                )
            )]
        );
        // A `try` with no `catch` has an empty handler.
        assert_eq!(
            parse("try, x = 1; end"),
            vec![at(
                1,
                Stmt::Try(
                    vec![at(1, Stmt::Assign(lv("x"), num(1.0), false))],
                    None,
                    vec![]
                )
            )]
        );
    }

    #[test]
    fn try_holds_any_statement_a_multi_assign_included() {
        let stmts = parse("try\n[a, ~] = size(x);\ncatch err\n[b] = f(err);\nend");
        match &stmts[0].stmt {
            Stmt::Try(body, Some(name), handler) => {
                assert_eq!(name, "err");
                assert!(matches!(body[0].stmt, Stmt::MultiAssign(ref t, _, false) if t.len() == 2));
                assert!(
                    matches!(handler[0].stmt, Stmt::MultiAssign(ref t, _, false) if t.len() == 1)
                );
            }
            other => panic!("expected a try, got {other:?}"),
        }
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("try\nx = 1;"), "expected 'end' but found end of input");
        assert_eq!(msg("try, catch e f, end"), "unexpected 'f'");
    }

    // ---- function definitions (cycle 05) -------------------------------

    fn program(src: &str) -> R<Program> {
        Parser::with_lines(scan(src).expect("lex should succeed")).parse_program()
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    /// Every header form, with and without `end`.
    #[test]
    fn function_headers_parse_in_every_form() {
        let p = program("disp(sq(4))\nfunction y = sq(x)\n    y = x^2;\nend").unwrap();
        assert_eq!(p.stmts.len(), 1);
        let f = &p.functions[0];
        assert_eq!(
            (f.name.as_str(), &f.outputs, &f.params, f.line),
            ("sq", &strings(&["y"]), &strings(&["x"]), 2)
        );
        assert_eq!(
            f.body,
            vec![at(
                3,
                Stmt::Assign(lv("y"), bin(BinOp::Pow, ident("x"), num(2.0)), false)
            )]
        );

        let f = &program("function [s, p] = sp(a, b)\ns = a + b; p = a * b;\nend")
            .unwrap()
            .functions[0];
        assert_eq!(
            (&f.outputs, &f.params),
            (&strings(&["s", "p"]), &strings(&["a", "b"]))
        );
        assert_eq!(f.body.len(), 2);
        // `[s p]` is `[s, p]`, the lexer's whitespace rule.
        let f = &program("function [s p] = sp()\nend").unwrap().functions[0];
        assert_eq!(f.outputs, strings(&["s", "p"]));

        // No outputs, no arguments, with and without parentheses.
        for src in ["function g2()\nx = 99;\nend", "function g2\nx = 99;\nend"] {
            let f = &program(src).unwrap().functions[0];
            assert_eq!(f.name, "g2");
            assert!(f.outputs.is_empty() && f.params.is_empty(), "{src}");
            assert_eq!(f.body, vec![at(2, Stmt::Assign(lv("x"), num(99.0), false))]);
        }
        // An ignored parameter keeps its place.
        let f = &program("function f(~, b)\nend").unwrap().functions[0];
        assert_eq!(f.params, strings(&["~", "b"]));
        // On one line.
        let f = &program("function r = f(x), r = x; end").unwrap().functions[0];
        assert_eq!(f.body.len(), 1);
    }

    /// A function file's functions may all go without `end`: each body runs
    /// to the next `function` or the end of the text, and the blocks inside
    /// keep their own `end`s.
    #[test]
    fn functions_without_end_run_to_the_next_function() {
        let p = program(
            "function y = helper(x)\nif x > 0\ny = twice(x);\nend\n\nfunction z = twice(x)\nz = 2 * x;\n",
        )
        .unwrap();
        assert!(p.stmts.is_empty());
        let names: Vec<&str> = p.functions.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["helper", "twice"]);
        assert_eq!(p.functions[0].body.len(), 1);
        assert!(matches!(p.functions[0].body[0].stmt, Stmt::If(..)));
        assert_eq!(p.functions[1].line, 6);
        // An end-less body can hold a `return`.
        let p = program("function f\nreturn\n").unwrap();
        assert_eq!(p.functions[0].body, vec![at(2, Stmt::Return)]);
    }

    /// Where a definition may not go.
    #[test]
    fn a_statement_after_a_function_is_refused() {
        let e = program("x = 1;\nfunction f()\nend\ny = 2;").unwrap_err();
        assert_eq!(
            (e.msg.as_str(), e.line),
            (
                "Function definitions in a script must appear at the end of the file.",
                Some(4)
            )
        );
        // Inside a block.
        let e = program("if 1\nfunction f()\nend\nend").unwrap_err();
        assert_eq!(
            (e.msg.as_str(), e.line),
            (
                "Function definitions are not supported in this context.",
                Some(2)
            )
        );
        // A stray `end` after a function that took its own.
        let e = program("function f()\nend\nend").unwrap_err();
        assert_eq!(e.msg, "unexpected 'end' with no matching block");
        // A header that is not one.
        assert_eq!(program("function 3").unwrap_err().msg, "unexpected '3'");
        assert_eq!(
            program("function y = f(x) y").unwrap_err().msg,
            "unexpected 'y'"
        );
    }

    // ---- function handles (cycle 06) ----------------------------------

    fn anon(params: &[&str], body: Expr) -> Expr {
        Expr::AnonFn(Rc::new(AnonFn::new(
            params.iter().map(|p| p.to_string()).collect(),
            body,
        )))
    }

    #[test]
    fn the_two_handle_forms_parse() {
        assert_eq!(parse_expr("@sin"), Expr::FuncHandle("sin".to_string()));
        assert_eq!(
            parse_expr("@(x, y) x + y"),
            anon(&["x", "y"], bin(BinOp::Add, ident("x"), ident("y")))
        );
        assert_eq!(parse_expr("@() 42"), anon(&[], num(42.0)));
        assert_eq!(parse_expr("@(~, y) y"), anon(&["~", "y"], ident("y")));
        // The body runs to the end of the expression, so a nested handle is
        // the whole body of the outer one.
        assert_eq!(
            parse_expr("@(a) @(b) a + b"),
            anon(
                &["a"],
                anon(&["b"], bin(BinOp::Add, ident("a"), ident("b")))
            )
        );
        // In an argument list the body stops at the comma.
        assert_eq!(
            parse_expr("arrayfun(@(x) x * 2, [1 2 3])"),
            call(
                "arrayfun",
                vec![
                    anon(&["x"], bin(BinOp::Mul, ident("x"), num(2.0))),
                    Expr::Matrix(vec![vec![num(1.0), num(2.0), num(3.0)]]),
                ]
            )
        );
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("f = @(x"), "unexpected end of input");
        assert_eq!(msg("f = @(1) 2"), "unexpected '1'");
        assert_eq!(msg("f = @(x,) 2"), "unexpected ')'");
    }

    /// `end` and a bare `:` belong to an index: a body inside an index
    /// argument is not one until it opens its own.
    #[test]
    fn an_anonymous_body_is_not_an_index_argument() {
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("y = x(@() end)"), "unexpected 'end' in expression");
        assert_eq!(
            parse_expr("@() x(end)"),
            anon(
                &[],
                Expr::Access("x".to_string(), vec![Access::Paren(vec![Expr::End])])
            )
        );
    }

    /// Acceptance test 12, the parser half: in a brace list, an anonymous
    /// function and the value after it are two elements; in a bracket, a
    /// handle is refused.
    #[test]
    fn a_handle_in_braces_is_one_element_and_in_brackets_an_error() {
        let two = vec![
            anon(&["x"], bin(BinOp::Add, ident("x"), num(1.0))),
            num(2.0),
        ];
        // The list of a brace, as a cell literal will read it in cycle 07.
        let mut p = Parser::new(lex("{@(x) x + 1, 2}").unwrap());
        assert_eq!(p.next(), Token::LBrace);
        assert_eq!(p.parse_args(Token::RBrace).unwrap(), two);
        assert_eq!(p.peek(), &Token::Eof);
        // The one brace list that evaluates today, a `case`'s.
        let prog = parse("switch 1\ncase {@(x) x + 1, 2}\nend");
        match &prog[0].stmt {
            Stmt::Switch(_, arms, _) => assert_eq!(arms[0].values, two),
            s => panic!("not a switch: {s:?}"),
        }
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        let concat =
            "Nonscalar arrays of function handles are not allowed; use cell arrays instead.";
        for src in [
            "[@(x) x+1]",
            "y = [@(x) x + 1, 2]",
            "y = [1 @sin]",
            "y = [1; @sin]",
        ] {
            assert_eq!(msg(src), concat, "{src}");
        }
        // Where the body swallows a space, what is left is not an operator,
        // and a cell literal (cycle 07) wants a separator after an element.
        assert_eq!(msg("y = {@(x) x 1}"), "unexpected '1'");
        // In a cell literal, a handle is an element like any other.
        match &parse("y = {@(x) x + 1, 2}")[0].stmt {
            Stmt::Assign(_, Expr::Cell(rows), _) => assert_eq!(rows, &vec![two.clone()]),
            s => panic!("not a cell literal: {s:?}"),
        }
    }

    /// The names a body reads that are not its parameters: what is
    /// captured when the function is made.
    #[test]
    fn an_anonymous_function_knows_the_names_it_reads() {
        let free = |src: &str| match parse_expr(src) {
            Expr::AnonFn(f) => f.free_names(),
            e => panic!("not a handle: {e:?}"),
        };
        assert_eq!(free("@(x) x + a + max(b, a)"), ["a", "max", "b"]);
        assert_eq!(free("@(x) s.f(k).g"), ["s", "k"]);
        assert_eq!(free("@(a) @(b) a + b + c"), ["c"]);
        assert_eq!(free("@() @sin"), Vec::<String>::new());
        assert_eq!(free("@(x) [x y; -z 1:n]"), ["y", "z", "n"]);
    }

    /// `func2str`'s rendering: the forms the spec records, then brackets,
    /// unary minus and the parentheses precedence needs.
    #[test]
    fn func2str_renders_from_the_tree() {
        let text = |src: &str| match parse_expr(src) {
            Expr::AnonFn(f) => f.text(),
            e => panic!("not a handle: {e:?}"),
        };
        // Recorded in the spec (acceptance tests 6 and 7).
        assert_eq!(text("@(x) x.^2 + 1"), "@(x)x.^2+1");
        assert_eq!(text("@(x) x + 1"), "@(x)x+1");
        assert_eq!(text("@(x) x*3"), "@(x)x*3");
        // Brackets keep their elements apart with commas.
        assert_eq!(text("@(x) [x 1]"), "@(x)[x,1]");
        assert_eq!(text("@(x) [x 1; 2 -x]"), "@(x)[x,1;2,-x]");
        assert_eq!(text("@() []"), "@()[]");
        // Unary minus, and where it needs its operand bracketed.
        assert_eq!(text("@(x) -x"), "@(x)-x");
        assert_eq!(text("@(x) -(x + 1)"), "@(x)-(x+1)");
        assert_eq!(text("@(x) -x^2"), "@(x)-x^2");
        assert_eq!(text("@(x) (-x)^2"), "@(x)(-x)^2");
        assert_eq!(text("@(x) 2^-x"), "@(x)2^-x");
        assert_eq!(text("@(x) ~x & x"), "@(x)~x&x");
        // Precedence and association.
        assert_eq!(text("@(x) (x + 1) * 2"), "@(x)(x+1)*2");
        assert_eq!(text("@(x) x + 1 * 2"), "@(x)x+1*2");
        assert_eq!(text("@(x) x - (1 - x)"), "@(x)x-(1-x)");
        assert_eq!(text("@(x) (x - 1) - x"), "@(x)x-1-x");
        assert_eq!(text("@(x) x' * (x')'"), "@(x)x'*x''");
        assert_eq!(text("@(x) (x + 1)'"), "@(x)(x+1)'");
        assert_eq!(text("@(a, b) a == b || ~a"), "@(a,b)a==b||~a");
        // Ranges: a stepped range reads back on the left, a plain one not.
        assert_eq!(text("@() 1:2:3:4"), "@()1:2:3:4");
        assert_eq!(text("@() (1:2):3"), "@()(1:2):3");
        assert_eq!(text("@(n) 1:n+1"), "@(n)1:n+1");
        // Indexing, fields, strings, numbers, handles.
        assert_eq!(text("@(x) x(:, end)"), "@(x)x(:,end)");
        assert_eq!(text("@(s) s.a.(k)"), "@(s)s.a.(k)");
        assert_eq!(text("@() 'it''s'"), "@()'it''s'");
        assert_eq!(text("@() 0.5 + 1e-20 + 1e20 + 3"), "@()0.5+1e-20+1e+20+3");
        assert_eq!(text("@(f) f(@sin)"), "@(f)f(@sin)");
        assert_eq!(text("@(a) @(b) a + b"), "@(a)@(b)a+b");
    }

    /// What `func2str` renders parses back to the same tree.
    #[test]
    fn a_rendered_body_parses_back_to_itself() {
        for src in [
            "@(x) [x 1; -x (x + 1) * 2]",
            "@(x, y) -x.^-y' + (x - y) / 2 \\ 3",
            "@(v) v(end - 1:end, :)'",
            "@() (1:2:3):4:5",
            "@(a) a & ~(a | a) || a && a",
            "@(s) s.b{2}.(n)(1)",
            "@(q) ((q))",
            // A negative literal (cycle 16), as a base, an exponent, under
            // a transpose and under a sign of its own.
            "@() 0xFFs8^2 + 2^0x80s8'",
            "@() -0xFFs8^2",
            "@(x) [0x80s8.^2 x; 0b11111111s8' -x]",
        ] {
            let e = parse_expr(src);
            assert_eq!(parse_expr(&render(&e)), e, "{src} -> {}", render(&e));
        }
    }

    /// A signed literal with its top bit set (cycle 16) is the one number
    /// token that is negative. It parses as the negation of its magnitude,
    /// the tree the same value written in decimal makes, so its value is
    /// unchanged and it renders with a negation's precedence.
    #[test]
    fn a_negative_literal_is_the_negation_of_its_magnitude() {
        assert_eq!(parse_expr("0xFFs8"), neg(num(1.0)));
        assert_eq!(parse_expr("0xFFs8"), parse_expr("-1"));
        assert_eq!(parse_expr("0x80s8"), parse_expr("-128"));
        assert_eq!(parse_expr("0b10000000s8"), neg(num(128.0)));
        assert_eq!(
            parse_expr("0x8000000000000000s64"),
            neg(num(9_223_372_036_854_775_808.0))
        );
        // A literal that is not negative is a number as before.
        assert_eq!(parse_expr("0x7Fs8"), num(127.0));
        assert_eq!(parse_expr("0xFFu8"), num(255.0));
        assert_eq!(parse_expr("0x0s8"), num(0.0));
        // As a base it keeps the negation's precedence: `(-1)^2`.
        assert_eq!(
            parse_expr("0xFFs8^2"),
            bin(BinOp::Pow, neg(num(1.0)), num(2.0))
        );
        assert_eq!(
            parse_expr("-0xFFs8^2"),
            neg(bin(BinOp::Pow, neg(num(1.0)), num(2.0)))
        );
        let text = |src: &str| match parse_expr(src) {
            Expr::AnonFn(f) => f.text(),
            e => panic!("not a handle: {e:?}"),
        };
        assert_eq!(text("@() 0xFFs8^2"), "@()(-1)^2");
        assert_eq!(text("@() 0x80s8.^2"), "@()(-128).^2");
        assert_eq!(text("@() -0xFFs8^2"), "@()-(-1)^2");
        assert_eq!(text("@() 2^0x80s8"), "@()2^-128");
        assert_eq!(text("@() 0xFFs8'"), "@()(-1)'");
        assert_eq!(text("@() [1 0xFFs8]"), "@()[1,-1]");
        assert_eq!(text("@() 0x2A"), "@()42");
    }

    /// `str2func`'s parse: one handle form, and nothing after it.
    #[test]
    fn a_str2func_text_is_one_handle() {
        let handle = |src: &str| Parser::new(lex(src).unwrap()).parse_handle();
        assert_eq!(
            handle("@(x) x*3").unwrap(),
            anon(&["x"], bin(BinOp::Mul, ident("x"), num(3.0)))
        );
        assert_eq!(handle("@sin").unwrap(), Expr::FuncHandle("sin".to_string()));
        assert_eq!(handle("@sin + 1").unwrap_err().msg, "unexpected '+'");
        assert_eq!(
            handle("sin").unwrap_err().msg,
            "expected '@' but found 'sin'"
        );
    }

    // ---- cell literals (cycle 07) ------------------------------------

    #[test]
    fn a_cell_literal_is_rows_of_elements() {
        assert_eq!(
            parse_expr("{1, 'a'; 2, 3}"),
            Expr::Cell(vec![
                vec![num(1.0), Expr::Str("a".into())],
                vec![num(2.0), num(3.0)],
            ])
        );
        assert_eq!(parse_expr("{}"), Expr::Cell(vec![]));
        assert_eq!(
            parse_expr("{1 -2}"),
            Expr::Cell(vec![vec![num(1.0), neg(num(2.0))]])
        );
        assert_eq!(
            parse_expr("{{1}}"),
            Expr::Cell(vec![vec![Expr::Cell(vec![vec![num(1.0)]])]])
        );
        // An element may be a cs-list, a chain like any other.
        assert_eq!(
            parse_expr("{c{:}}"),
            Expr::Cell(vec![vec![access(
                "c",
                vec![Access::Brace(vec![Expr::Colon])]
            )]])
        );
        let msg = |src: &str| parse_result(src).unwrap_err().msg;
        assert_eq!(msg("c = {1, 2"), "expected '}' but found end of input");
    }

    /// `func2str` reads a cell literal back in braces, and it parses back
    /// to the same tree.
    #[test]
    fn a_cell_literal_renders_in_braces() {
        let e = parse_expr("@() {1, 'a'; x, {2}}");
        let Expr::AnonFn(f) = &e else {
            panic!("not a handle: {e:?}");
        };
        assert_eq!(f.text(), "@(){1,'a';x,{2}}");
        assert_eq!(parse_expr(&f.text()), e);
        // The names a cell literal reads are captured.
        assert_eq!(f.free_names(), ["x"]);
    }
}
