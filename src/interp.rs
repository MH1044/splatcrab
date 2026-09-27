//! Tree-walking interpreter.

use std::collections::HashMap;

use crate::lexer::lex;
use crate::parser::{BinOp, Expr, Parser, Stmt};
use crate::value::{nonfinite, Matrix, Value};

type R<T> = Result<T, String>;

pub struct Interp {
    pub vars: HashMap<String, Value>,
    /// Value of `end` for the index argument currently being evaluated.
    end_stack: Vec<usize>,
    rng: u64,
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
        Interp {
            vars: HashMap::new(),
            end_stack: Vec::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
        }
    }

    pub fn run(&mut self, src: &str) -> R<()> {
        let toks = lex(src)?;
        let stmts = Parser::new(toks).parse_program()?;
        self.exec_block(&stmts)?;
        Ok(())
    }

    // ---- statements --------------------------------------------------

    fn exec_block(&mut self, stmts: &[Stmt]) -> R<Flow> {
        for s in stmts {
            match self.exec(s)? {
                Flow::Normal => {}
                f => return Ok(f),
            }
        }
        Ok(Flow::Normal)
    }

    fn exec(&mut self, stmt: &Stmt) -> R<Flow> {
        match stmt {
            Stmt::Expr(e, show) => {
                // A statement-level call to a builtin may produce no value (disp, fprintf, ...).
                let result = match e {
                    Expr::Ident(n) if !self.vars.contains_key(n) => self.call_builtin(n, vec![])?,
                    Expr::Index(n, args) if !self.vars.contains_key(n) => {
                        let a = self.eval_args(args)?;
                        self.call_builtin(n, a)?
                    }
                    _ => Some(self.eval(e)?),
                };
                if let Some(v) = result {
                    let name = match e {
                        Expr::Ident(n) if self.vars.contains_key(n) => n.clone(),
                        _ => "ans".to_string(),
                    };
                    if name == "ans" {
                        self.vars.insert(name.clone(), v.clone());
                    }
                    if *show {
                        print!("{}", v.display(&name));
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Assign(name, e, show) => {
                let v = self.eval(e)?;
                if *show {
                    print!("{}", v.display(name));
                }
                self.vars.insert(name.clone(), v);
                Ok(Flow::Normal)
            }
            Stmt::IndexAssign(name, args, e, show) => {
                let rhs = self.eval(e)?;
                self.assign_index(name, args, rhs)?;
                if *show {
                    print!("{}", self.vars[name].display(name));
                }
                Ok(Flow::Normal)
            }
            Stmt::If(arms, otherwise) => {
                for (cond, body) in arms {
                    if self.eval_mat(cond)?.is_true() {
                        return self.exec_block(body);
                    }
                }
                if let Some(body) = otherwise {
                    return self.exec_block(body);
                }
                Ok(Flow::Normal)
            }
            Stmt::For(name, e, body) => {
                let m = self.eval_mat(e)?;
                for c in 0..m.cols {
                    let column: Vec<f64> = (0..m.rows).map(|r| m.get(r, c)).collect();
                    let v = if m.rows == 1 {
                        Matrix::scalar(column[0])
                    } else {
                        Matrix::col(column)
                    };
                    self.vars.insert(name.clone(), Value::Mat(v));
                    if let Flow::Break = self.exec_block(body)? {
                        break;
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::While(cond, body) => {
                while self.eval_mat(cond)?.is_true() {
                    if let Flow::Break = self.exec_block(body)? {
                        break;
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
        }
    }

    // ---- expressions -------------------------------------------------

    fn eval(&mut self, e: &Expr) -> R<Value> {
        match e {
            Expr::Num(v) => Ok(Value::Mat(Matrix::scalar(*v))),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Ident(n) => {
                if let Some(v) = self.vars.get(n) {
                    return Ok(v.clone());
                }
                self.call_builtin(n, vec![])?
                    .ok_or_else(|| format!("'{}' does not return a value.", n))
            }
            Expr::End => self
                .end_stack
                .last()
                .map(|n| Value::Mat(Matrix::scalar(*n as f64)))
                .ok_or_else(|| "'end' is only valid inside an index expression.".to_string()),
            Expr::Colon => Err("':' on its own is only valid inside an index expression.".to_string()),
            Expr::Index(n, args) => self.index(n, args),
            Expr::Matrix(rows) => self.build_matrix(rows),
            Expr::Neg(a) => Ok(Value::Mat(self.eval_mat(a)?.map(|x| -x))),
            Expr::Not(a) => Ok(Value::Mat(
                self.eval_mat(a)?.map(|x| if x == 0.0 { 1.0 } else { 0.0 }),
            )),
            Expr::Transpose(a) => Ok(Value::Mat(self.eval_mat(a)?.transpose())),
            Expr::Range(a, step, b) => {
                let a = self.eval_scalar(a, "range start")?;
                let b = self.eval_scalar(b, "range end")?;
                let s = match step {
                    Some(s) => self.eval_scalar(s, "range step")?,
                    None => 1.0,
                };
                Ok(Value::Mat(range(a, s, b)))
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
            .ok_or_else(|| format!("{} must be a scalar.", what))
    }

    fn eval_args(&mut self, args: &[Expr]) -> R<Vec<Value>> {
        args.iter().map(|a| self.eval(a)).collect()
    }

    fn binary(&mut self, op: BinOp, a: &Expr, b: &Expr) -> R<Value> {
        // Short-circuit logical operators.
        match op {
            BinOp::AndAnd => {
                let l = self.eval_mat(a)?.is_true();
                let v = l && self.eval_mat(b)?.is_true();
                return Ok(Value::Mat(Matrix::scalar(v as u8 as f64)));
            }
            BinOp::OrOr => {
                let l = self.eval_mat(a)?.is_true();
                let v = l || self.eval_mat(b)?.is_true();
                return Ok(Value::Mat(Matrix::scalar(v as u8 as f64)));
            }
            _ => {}
        }
        let a = self.eval_mat(a)?;
        let b = self.eval_mat(b)?;
        let bool_op = |f: fn(f64, f64) -> bool| {
            move |x: f64, y: f64| if f(x, y) { 1.0 } else { 0.0 }
        };
        let r = match op {
            BinOp::Add => a.zip(&b, "+", |x, y| x + y)?,
            BinOp::Sub => a.zip(&b, "-", |x, y| x - y)?,
            BinOp::EMul => a.zip(&b, ".*", |x, y| x * y)?,
            BinOp::EDiv => a.zip(&b, "./", |x, y| x / y)?,
            BinOp::EPow => a.zip(&b, ".^", f64::powf)?,
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
                    Matrix::scalar(a.data[0].powf(b.data[0]))
                } else if let Some(p) = b.scalar_value() {
                    matrix_power(&a, p)?
                } else {
                    return Err(
                        "Matrix exponent is not supported; use '.^' for element-wise power."
                            .to_string(),
                    );
                }
            }
            BinOp::Eq => a.zip(&b, "==", bool_op(|x, y| x == y))?,
            BinOp::Ne => a.zip(&b, "~=", bool_op(|x, y| x != y))?,
            BinOp::Lt => a.zip(&b, "<", bool_op(|x, y| x < y))?,
            BinOp::Le => a.zip(&b, "<=", bool_op(|x, y| x <= y))?,
            BinOp::Gt => a.zip(&b, ">", bool_op(|x, y| x > y))?,
            BinOp::Ge => a.zip(&b, ">=", bool_op(|x, y| x >= y))?,
            BinOp::And => a.zip(&b, "&", bool_op(|x, y| x != 0.0 && y != 0.0))?,
            BinOp::Or => a.zip(&b, "|", bool_op(|x, y| x != 0.0 || y != 0.0))?,
            BinOp::AndAnd | BinOp::OrOr => unreachable!(),
        };
        Ok(Value::Mat(r))
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
                return self
                    .call_builtin(name, a)?
                    .ok_or_else(|| format!("'{}' does not return a value.", name));
            }
        };
        let (m, is_str) = match var {
            Value::Str(s) => (Value::Str(s).into_mat(), true),
            Value::Mat(m) => (m, false),
        };
        let sel = self.eval_index_args(&m, args)?;
        let out = index_read(&m, &sel)?;
        if is_str {
            Ok(Value::Str(
                out.data
                    .iter()
                    .map(|c| char::from_u32(*c as u32).unwrap_or('?'))
                    .collect(),
            ))
        } else {
            Ok(Value::Mat(out))
        }
    }

    fn eval_index_args(&mut self, m: &Matrix, args: &[Expr]) -> R<Vec<Sel>> {
        if args.is_empty() || args.len() > 2 {
            return Err("Only 1-D and 2-D indexing is supported.".to_string());
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
            let mut idx = Vec::with_capacity(v.numel());
            for x in &v.data {
                if x.fract() != 0.0 || *x < 1.0 {
                    return Err(format!(
                        "Index in position {} is invalid. Array indices must be positive integers.",
                        k + 1
                    ));
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
            return Err("Deleting elements with '= []' is not supported yet.".to_string());
        }
        let mut m = match self.vars.get(name) {
            Some(v) => v.clone().into_mat(),
            None => Matrix::empty(),
        };
        let sel = self.eval_index_args(&m, args)?;
        let size_err = |lhs: usize, r: usize| {
            format!(
                "Unable to perform assignment because the left side has {} elements and the right side has {}.",
                lhs, r
            )
        };
        if sel.len() == 1 {
            let idx: Vec<usize> = match &sel[0] {
                Sel::All => (0..m.numel()).collect(),
                Sel::List(i, _, _) => i.clone(),
            };
            let need = idx.iter().copied().max().map_or(0, |x| x + 1);
            if need > m.numel() {
                if m.is_empty() {
                    m = Matrix::filled(1, need, 0.0);
                } else if m.rows == 1 {
                    m.data.resize(need, 0.0);
                    m.cols = need;
                } else if m.cols == 1 {
                    m.data.resize(need, 0.0);
                    m.rows = need;
                } else {
                    return Err("Attempt to grow array along ambiguous dimension.".to_string());
                }
            }
            if !rhs.is_scalar() && rhs.numel() != idx.len() {
                return Err(size_err(idx.len(), rhs.numel()));
            }
            for (n, &k) in idx.iter().enumerate() {
                m.data[k] = if rhs.is_scalar() { rhs.data[0] } else { rhs.data[n] };
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
                let mut grown = Matrix::filled(nr, nc, 0.0);
                for c in 0..m.cols {
                    for r in 0..m.rows {
                        grown.set(r, c, m.get(r, c));
                    }
                }
                m = grown;
            }
            let count = rows.len() * cols.len();
            if !rhs.is_scalar() && rhs.numel() != count {
                return Err(size_err(count, rhs.numel()));
            }
            let mut n = 0;
            for &c in &cols {
                for &r in &rows {
                    let v = if rhs.is_scalar() { rhs.data[0] } else { rhs.data[n] };
                    m.set(r, c, v);
                    n += 1;
                }
            }
        }
        self.vars.insert(name.to_string(), Value::Mat(m));
        Ok(())
    }

    // ---- builtins ----------------------------------------------------

    fn next_rand(&mut self) -> f64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (r >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Returns Ok(None) for builtins that produce no value.
    fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> R<Option<Value>> {
        use std::f64::consts::{E, PI};
        let nargs = args.len();
        let mat = |i: usize| -> R<Matrix> {
            args.get(i)
                .cloned()
                .map(|v| v.into_mat())
                .ok_or_else(|| format!("Not enough input arguments for '{}'.", name))
        };
        let scalar = |i: usize| -> R<f64> {
            mat(i)?
                .scalar_value()
                .ok_or_else(|| format!("Argument {} to '{}' must be a scalar.", i + 1, name))
        };
        let dim = |i: usize| -> R<usize> {
            let v = scalar(i)?;
            if v < 0.0 || v.fract() != 0.0 {
                return Err(format!("Size arguments to '{}' must be non-negative integers.", name));
            }
            Ok(v as usize)
        };
        let unary = |f: fn(f64) -> f64| -> R<Option<Value>> { ok(mat(0)?.map(f)) };

        match name {
            // constants
            "pi" => ok(Matrix::scalar(PI)),
            "e" => ok(Matrix::scalar(E)),
            "Inf" | "inf" => ok(Matrix::scalar(f64::INFINITY)),
            "NaN" | "nan" => ok(Matrix::scalar(f64::NAN)),
            "eps" => ok(Matrix::scalar(f64::EPSILON)),
            "true" => ok(Matrix::scalar(1.0)),
            "false" => ok(Matrix::scalar(0.0)),

            // constructors
            "zeros" | "ones" | "eye" | "rand" => {
                let (r, c) = match nargs {
                    0 => (1, 1),
                    1 => {
                        let n = dim(0)?;
                        (n, n)
                    }
                    _ => (dim(0)?, dim(1)?),
                };
                let m = match name {
                    "zeros" => Matrix::filled(r, c, 0.0),
                    "ones" => Matrix::filled(r, c, 1.0),
                    "eye" => Matrix::identity(r, c),
                    _ => {
                        let mut m = Matrix::filled(r, c, 0.0);
                        for v in &mut m.data {
                            *v = self.next_rand();
                        }
                        m
                    }
                };
                ok(m)
            }
            "linspace" => {
                let a = scalar(0)?;
                let b = scalar(1)?;
                let n = if nargs >= 3 { dim(2)? } else { 100 };
                let data = (0..n)
                    .map(|k| {
                        if n == 1 {
                            b
                        } else {
                            a + (b - a) * k as f64 / (n - 1) as f64
                        }
                    })
                    .collect();
                ok(Matrix::row(data))
            }

            // shape queries
            "size" => {
                let m = mat(0)?;
                if nargs >= 2 {
                    let d = dim(1)?;
                    let v = match d {
                        1 => m.rows,
                        2 => m.cols,
                        _ => 1,
                    };
                    ok(Matrix::scalar(v as f64))
                } else {
                    ok(Matrix::row(vec![m.rows as f64, m.cols as f64]))
                }
            }
            "numel" => ok(Matrix::scalar(mat(0)?.numel() as f64)),
            "length" => {
                let m = mat(0)?;
                let n = if m.is_empty() { 0 } else { m.rows.max(m.cols) };
                ok(Matrix::scalar(n as f64))
            }
            "isempty" => ok(Matrix::scalar(mat(0)?.is_empty() as u8 as f64)),
            "isscalar" => ok(Matrix::scalar(mat(0)?.is_scalar() as u8 as f64)),
            "isvector" => ok(Matrix::scalar(mat(0)?.is_vector() as u8 as f64)),

            // reductions
            "sum" | "prod" | "mean" | "any" | "all" => {
                let m = mat(0)?;
                let d = if nargs >= 2 { Some(dim(1)?) } else { None };
                let f: fn(&[f64]) -> f64 = match name {
                    "sum" => |xs| xs.iter().sum(),
                    "prod" => |xs| xs.iter().product(),
                    "mean" => |xs| xs.iter().sum::<f64>() / xs.len() as f64,
                    "any" => |xs| xs.iter().any(|v| *v != 0.0) as u8 as f64,
                    _ => |xs| xs.iter().all(|v| *v != 0.0) as u8 as f64,
                };
                ok(reduce(&m, d, f))
            }
            "max" | "min" => {
                let is_max = name == "max";
                let m = mat(0)?;
                if nargs == 2 {
                    let b = mat(1)?;
                    let f = move |x: f64, y: f64| {
                        if x.is_nan() {
                            y
                        } else if y.is_nan() {
                            x
                        } else if is_max {
                            x.max(y)
                        } else {
                            x.min(y)
                        }
                    };
                    return ok(m.zip(&b, name, f)?);
                }
                if m.is_empty() {
                    return ok(Matrix::empty());
                }
                let d = if nargs >= 3 { Some(dim(2)?) } else { None };
                let f = move |xs: &[f64]| {
                    xs.iter()
                        .copied()
                        .filter(|v| !v.is_nan())
                        .fold(f64::NAN, |acc, v| {
                            if acc.is_nan() {
                                v
                            } else if is_max {
                                acc.max(v)
                            } else {
                                acc.min(v)
                            }
                        })
                };
                ok(reduce(&m, d, f))
            }
            "cumsum" | "cumprod" => {
                let m = mat(0)?;
                let is_sum = name == "cumsum";
                let mut out = m.clone();
                if m.rows == 1 {
                    let mut acc = if is_sum { 0.0 } else { 1.0 };
                    for v in &mut out.data {
                        acc = if is_sum { acc + *v } else { acc * *v };
                        *v = acc;
                    }
                } else {
                    for c in 0..m.cols {
                        let mut acc = if is_sum { 0.0 } else { 1.0 };
                        for r in 0..m.rows {
                            acc = if is_sum { acc + m.get(r, c) } else { acc * m.get(r, c) };
                            out.set(r, c, acc);
                        }
                    }
                }
                ok(out)
            }

            // element-wise math
            "abs" => unary(f64::abs),
            "sqrt" => unary(f64::sqrt),
            "exp" => unary(f64::exp),
            "log" => unary(f64::ln),
            "log2" => unary(f64::log2),
            "log10" => unary(f64::log10),
            "sin" => unary(f64::sin),
            "cos" => unary(f64::cos),
            "tan" => unary(f64::tan),
            "asin" => unary(f64::asin),
            "acos" => unary(f64::acos),
            "atan" => unary(f64::atan),
            "sinh" => unary(f64::sinh),
            "cosh" => unary(f64::cosh),
            "tanh" => unary(f64::tanh),
            "floor" => unary(f64::floor),
            "ceil" => unary(f64::ceil),
            "round" => unary(f64::round),
            "fix" => unary(f64::trunc),
            "sign" => unary(|x| {
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }),
            "isnan" => unary(|x| x.is_nan() as u8 as f64),
            "isinf" => unary(|x| x.is_infinite() as u8 as f64),
            "isfinite" => unary(|x| x.is_finite() as u8 as f64),
            "mod" => ok(mat(0)?.zip(&mat(1)?, "mod", |x, y| {
                if y == 0.0 {
                    x
                } else {
                    x - (x / y).floor() * y
                }
            })?),
            "rem" => ok(mat(0)?.zip(&mat(1)?, "rem", |x, y| x - (x / y).trunc() * y)?),
            "atan2" => ok(mat(0)?.zip(&mat(1)?, "atan2", f64::atan2)?),
            "hypot" => ok(mat(0)?.zip(&mat(1)?, "hypot", f64::hypot)?),
            "power" => ok(mat(0)?.zip(&mat(1)?, "power", f64::powf)?),

            // linear algebra
            "transpose" => ok(mat(0)?.transpose()),
            "inv" => ok(mat(0)?.inv()?),
            "det" => ok(Matrix::scalar(mat(0)?.det()?)),
            "trace" => {
                let m = mat(0)?;
                if m.rows != m.cols {
                    return Err("Matrix must be square for 'trace'.".to_string());
                }
                ok(Matrix::scalar((0..m.rows).map(|i| m.get(i, i)).sum()))
            }
            "diag" => {
                let m = mat(0)?;
                if m.is_vector() {
                    let n = m.numel();
                    let mut out = Matrix::filled(n, n, 0.0);
                    for (i, v) in m.data.iter().enumerate() {
                        out.set(i, i, *v);
                    }
                    ok(out)
                } else {
                    ok(Matrix::col((0..m.rows.min(m.cols)).map(|i| m.get(i, i)).collect()))
                }
            }
            "norm" => {
                let m = mat(0)?;
                if !m.is_vector() && !m.is_empty() {
                    return Err("'norm' currently supports vectors only.".to_string());
                }
                ok(Matrix::scalar(m.data.iter().map(|v| v * v).sum::<f64>().sqrt()))
            }
            "dot" => {
                let a = mat(0)?;
                let b = mat(1)?;
                if a.numel() != b.numel() {
                    return Err("Vectors must be the same length for 'dot'.".to_string());
                }
                ok(Matrix::scalar(
                    a.data.iter().zip(&b.data).map(|(x, y)| x * y).sum(),
                ))
            }
            "reshape" => {
                let m = mat(0)?;
                let r = dim(1)?;
                let c = dim(2)?;
                if r * c != m.numel() {
                    return Err(format!(
                        "To reshape the number of elements must not change ({} vs {}x{}).",
                        m.numel(),
                        r,
                        c
                    ));
                }
                ok(Matrix::new(r, c, m.data))
            }
            "repmat" => {
                let m = mat(0)?;
                let r = dim(1)?;
                let c = if nargs >= 3 { dim(2)? } else { r };
                let mut out = Matrix::filled(m.rows * r, m.cols * c, 0.0);
                for j in 0..out.cols {
                    for i in 0..out.rows {
                        out.set(i, j, m.get(i % m.rows.max(1), j % m.cols.max(1)));
                    }
                }
                ok(out)
            }
            "fliplr" => {
                let m = mat(0)?;
                let mut out = m.clone();
                for c in 0..m.cols {
                    for r in 0..m.rows {
                        out.set(r, m.cols - 1 - c, m.get(r, c));
                    }
                }
                ok(out)
            }
            "flipud" => {
                let m = mat(0)?;
                let mut out = m.clone();
                for c in 0..m.cols {
                    for r in 0..m.rows {
                        out.set(m.rows - 1 - r, c, m.get(r, c));
                    }
                }
                ok(out)
            }
            "find" => {
                let m = mat(0)?;
                let idx: Vec<f64> = m
                    .data
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v != 0.0)
                    .map(|(i, _)| (i + 1) as f64)
                    .collect();
                ok(if m.rows == 1 { Matrix::row(idx) } else { Matrix::col(idx) })
            }
            "sort" => {
                let m = mat(0)?;
                if !m.is_vector() && !m.is_empty() {
                    return Err("'sort' currently supports vectors only.".to_string());
                }
                let mut out = m.clone();
                out.data
                    .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                ok(out)
            }

            // output
            "disp" => {
                match args.first() {
                    Some(Value::Str(s)) => println!("{}", s),
                    Some(Value::Mat(m)) => print!("{}", m.format()),
                    None => return Err("Not enough input arguments for 'disp'.".to_string()),
                }
                Ok(None)
            }
            "fprintf" => {
                print!("{}", format_printf(&args)?);
                Ok(None)
            }
            "sprintf" => Ok(Some(Value::Str(format_printf(&args)?))),
            "num2str" => {
                let s = match args.first() {
                    Some(Value::Str(s)) => s.clone(),
                    Some(Value::Mat(m)) => m
                        .data
                        .iter()
                        .map(|v| num2str(*v))
                        .collect::<Vec<_>>()
                        .join("  "),
                    None => return Err("Not enough input arguments for 'num2str'.".to_string()),
                };
                Ok(Some(Value::Str(s)))
            }
            "error" => {
                let msg = match args.first() {
                    Some(Value::Str(_)) => format_printf(&args)?,
                    _ => "error".to_string(),
                };
                Err(msg)
            }

            // workspace
            "clear" => {
                self.vars.clear();
                Ok(None)
            }
            "clc" => {
                print!("\x1B[2J\x1B[H");
                Ok(None)
            }
            "who" | "whos" => {
                let mut names: Vec<&String> = self.vars.keys().collect();
                names.sort();
                if names.is_empty() {
                    return Ok(None);
                }
                println!("Your variables are:\n");
                for n in names {
                    match &self.vars[n] {
                        Value::Mat(m) => println!("  {:<12} {}x{} double", n, m.rows, m.cols),
                        Value::Str(s) => println!("  {:<12} 1x{} char", n, s.chars().count()),
                    }
                }
                println!();
                Ok(None)
            }

            _ => Err(format!("Undefined function or variable '{}'.", name)),
        }
    }
}

fn ok(m: Matrix) -> R<Option<Value>> {
    Ok(Some(Value::Mat(m)))
}

// ---- helpers ---------------------------------------------------------

fn range(a: f64, s: f64, b: f64) -> Matrix {
    if s == 0.0 || (b - a) / s < 0.0 || !a.is_finite() || !b.is_finite() || !s.is_finite() {
        return Matrix::new(1, 0, Vec::new());
    }
    let n = ((b - a) / s + 1e-10).floor() as usize + 1;
    Matrix::row((0..n).map(|k| a + k as f64 * s).collect())
}

fn matrix_power(a: &Matrix, p: f64) -> R<Matrix> {
    if a.rows != a.cols {
        return Err("Matrix must be square for '^'. Use '.^' for element-wise power.".to_string());
    }
    if p.fract() != 0.0 {
        return Err("Only integer matrix powers are supported.".to_string());
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

fn reduce(m: &Matrix, dim: Option<usize>, f: impl Fn(&[f64]) -> f64) -> Matrix {
    if m.rows == 0 && m.cols == 0 {
        return Matrix::scalar(f(&[]));
    }
    let dim = dim.unwrap_or(if m.rows == 1 { 2 } else { 1 });
    if dim == 1 {
        Matrix::row(
            (0..m.cols)
                .map(|c| f(&m.data[c * m.rows..(c + 1) * m.rows]))
                .collect(),
        )
    } else {
        Matrix::col(
            (0..m.rows)
                .map(|r| {
                    let xs: Vec<f64> = (0..m.cols).map(|c| m.get(r, c)).collect();
                    f(&xs)
                })
                .collect(),
        )
    }
}

fn index_read(m: &Matrix, sel: &[Sel]) -> R<Matrix> {
    if sel.len() == 1 {
        match &sel[0] {
            Sel::All => Ok(Matrix::col(m.data.clone())),
            Sel::List(idx, ir, ic) => {
                let mut data = Vec::with_capacity(idx.len());
                for &k in idx {
                    if k >= m.numel() {
                        return Err(format!(
                            "Index exceeds the number of array elements. Index must not exceed {}.",
                            m.numel()
                        ));
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
            return Err(format!(
                "Index in position 1 exceeds array bounds. Index must not exceed {}.",
                m.rows
            ));
        }
        if cols.iter().any(|c| *c >= m.cols) {
            return Err(format!(
                "Index in position 2 exceeds array bounds. Index must not exceed {}.",
                m.cols
            ));
        }
        let mut data = Vec::with_capacity(rows.len() * cols.len());
        for &c in &cols {
            for &r in &rows {
                data.push(m.get(r, c));
            }
        }
        Ok(Matrix::new(rows.len(), cols.len(), data))
    }
}

fn hcat(vals: Vec<Value>) -> R<Value> {
    if vals.is_empty() {
        return Ok(Value::Mat(Matrix::empty()));
    }
    if vals.iter().all(|v| matches!(v, Value::Str(_))) {
        let mut s = String::new();
        for v in vals {
            if let Value::Str(t) = v {
                s.push_str(&t);
            }
        }
        return Ok(Value::Str(s));
    }
    let mats: Vec<Matrix> = vals
        .into_iter()
        .map(|v| v.into_mat())
        .filter(|m| !m.is_empty())
        .collect();
    if mats.is_empty() {
        return Ok(Value::Mat(Matrix::empty()));
    }
    let rows = mats[0].rows;
    if mats.iter().any(|m| m.rows != rows) {
        return Err("Dimensions of arrays being concatenated are not consistent.".to_string());
    }
    let mut data = Vec::new();
    let mut cols = 0;
    for m in &mats {
        data.extend_from_slice(&m.data);
        cols += m.cols;
    }
    Ok(Value::Mat(Matrix::new(rows, cols, data)))
}

fn vcat(vals: Vec<Value>) -> R<Value> {
    if vals.len() == 1 {
        return Ok(vals.into_iter().next().unwrap());
    }
    let mats: Vec<Matrix> = vals
        .into_iter()
        .map(|v| v.into_mat())
        .filter(|m| !m.is_empty())
        .collect();
    if mats.is_empty() {
        return Ok(Value::Mat(Matrix::empty()));
    }
    let cols = mats[0].cols;
    if mats.iter().any(|m| m.cols != cols) {
        return Err("Dimensions of arrays being concatenated are not consistent.".to_string());
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
    Ok(Value::Mat(out))
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

fn num2str(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let digits = if v.abs() >= 1.0 {
            (v.abs().log10().floor() as i32 + 5).max(5) as usize
        } else {
            5
        };
        fmt_g(v, digits)
    }
}

enum PArg {
    N(f64),
    S(String),
}

/// Shared implementation of fprintf / sprintf. Cycles the format over the
/// flattened arguments like MATLAB does.
fn format_printf(args: &[Value]) -> R<String> {
    let fmt = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err("The first argument must be a format string.".to_string()),
    };
    let mut flat: Vec<PArg> = Vec::new();
    for a in &args[1..] {
        match a {
            Value::Str(s) => flat.push(PArg::S(s.clone())),
            Value::Mat(m) => flat.extend(m.data.iter().map(|v| PArg::N(*v))),
        }
    }
    let has_args = !flat.is_empty();
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut ai = 0;
    loop {
        let mut i = 0;
        let mut specs_in_pass = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                match chars[i] {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '\\' => out.push('\\'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
                i += 1;
                continue;
            }
            if c != '%' {
                out.push(c);
                i += 1;
                continue;
            }
            i += 1;
            if i < chars.len() && chars[i] == '%' {
                out.push('%');
                i += 1;
                continue;
            }
            let mut flags = String::new();
            while i < chars.len() && "-+ 0#".contains(chars[i]) {
                flags.push(chars[i]);
                i += 1;
            }
            let mut width = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                width.push(chars[i]);
                i += 1;
            }
            let mut prec: Option<usize> = None;
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                let mut p = String::new();
                while i < chars.len() && chars[i].is_ascii_digit() {
                    p.push(chars[i]);
                    i += 1;
                }
                prec = Some(p.parse().unwrap_or(0));
            }
            if i >= chars.len() {
                return Err("Invalid format specifier.".to_string());
            }
            let conv = chars[i];
            i += 1;
            specs_in_pass += 1;
            if has_args && ai >= flat.len() {
                // Ran out of data: MATLAB stops here.
                return Ok(out);
            }
            let arg = if has_args {
                ai += 1;
                Some(&flat[ai - 1])
            } else {
                None
            };
            let body = match (conv, arg) {
                (_, None) => String::new(),
                ('d' | 'i' | 'u', Some(PArg::N(v))) => {
                    if v.fract() == 0.0 && v.is_finite() {
                        format!("{}", *v as i64)
                    } else {
                        fmt_g(*v, prec.map_or(6, |p| p.max(1)))
                    }
                }
                ('f' | 'F', Some(PArg::N(v))) => {
                    if v.is_finite() {
                        format!("{:.*}", prec.unwrap_or(6), v)
                    } else {
                        nonfinite(*v)
                    }
                }
                ('e' | 'E', Some(PArg::N(v))) => fmt_e(*v, prec.unwrap_or(6)),
                ('g' | 'G', Some(PArg::N(v))) => fmt_g(*v, prec.unwrap_or(6)),
                ('c', Some(PArg::N(v))) => char::from_u32(*v as u32).unwrap_or('?').to_string(),
                ('s', Some(PArg::N(v))) => fmt_g(*v, prec.unwrap_or(6)),
                (_, Some(PArg::S(s))) => s.clone(),
                (other, _) => return Err(format!("Unsupported format specifier '%{}'.", other)),
            };
            let width: usize = width.parse().unwrap_or(0);
            let len = body.chars().count();
            if len >= width {
                out.push_str(&body);
            } else if flags.contains('-') {
                out.push_str(&body);
                out.extend(std::iter::repeat(' ').take(width - len));
            } else if flags.contains('0') && matches!(arg, Some(PArg::N(_))) {
                let (sign, digits) = match body.strip_prefix('-') {
                    Some(d) => ("-", d.to_string()),
                    None => ("", body.clone()),
                };
                out.push_str(sign);
                out.extend(std::iter::repeat('0').take(width - len));
                out.push_str(&digits);
            } else {
                out.extend(std::iter::repeat(' ').take(width - len));
                out.push_str(&body);
            }
        }
        if !has_args || ai >= flat.len() || specs_in_pass == 0 {
            break;
        }
    }
    Ok(out)
}
