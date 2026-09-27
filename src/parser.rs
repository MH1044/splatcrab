//! Recursive-descent parser producing statements and expressions.
//!
//! Precedence (loosest to tightest), following MATLAB:
//!   ||   &&   |   &   comparison   :   + -   * / \ .* ./ .\   unary - ~   ^ .^   transpose

use crate::bail;
use crate::error::{self, R};
use crate::lexer::{Lexed, Token};

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(String),
    Ident(String),
    /// `end` inside an index expression.
    End,
    /// Bare `:` inside an index expression.
    Colon,
    /// `[a b; c d]` — rows of elements.
    Matrix(Vec<Vec<Expr>>),
    /// `name(args)` — indexing if `name` is a variable, otherwise a call.
    Index(String, Vec<Expr>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Transpose(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `a:b` or `a:s:b`
    Range(Box<Expr>, Option<Box<Expr>>, Box<Expr>),
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
    Assign(String, Expr, bool),
    IndexAssign(String, Vec<Expr>, Expr, bool),
    If(Vec<(Expr, Vec<Located>)>, Option<Vec<Located>>),
    For(String, Expr, Vec<Located>),
    While(Expr, Vec<Located>),
    Break,
    Continue,
}

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
}

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Parser {
            toks,
            lines: Vec::new(),
            pos: 0,
            in_index: 0,
        }
    }

    /// The parser the interpreter uses: tokens with the lines they came from.
    pub fn with_lines(lexed: Lexed) -> Self {
        Parser {
            toks: lexed.tokens,
            lines: lexed.lines,
            pos: 0,
            in_index: 0,
        }
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

    pub fn parse_program(&mut self) -> R<Vec<Located>> {
        let stmts = self.parse_block(&[])?;
        if self.peek() != &Token::Eof {
            let line = self.line();
            bail!(error::unexpected_token(self.peek()).at(line));
        }
        Ok(stmts)
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
            let stmt = self.parse_stmt()?;
            out.push(Located { stmt, line });
        }
    }

    fn parse_stmt(&mut self) -> R<Stmt> {
        let line = self.line();
        match self.peek().clone() {
            Token::If => {
                self.next();
                let mut arms = Vec::new();
                let cond = self.parse_expr()?;
                let body = self.parse_block(&[Token::End, Token::Else, Token::ElseIf])?;
                arms.push((cond, body));
                loop {
                    match self.next() {
                        Token::ElseIf => {
                            let cond = self.parse_expr()?;
                            let body =
                                self.parse_block(&[Token::End, Token::Else, Token::ElseIf])?;
                            arms.push((cond, body));
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
            Token::End | Token::Else | Token::ElseIf => {
                bail!(error::block_with_no_opener(self.peek()).at(line))
            }
            _ => {
                let e = self.parse_expr()?;
                if self.peek() == &Token::Assign {
                    self.next();
                    let rhs = self.parse_expr()?;
                    let show = self.end_stmt()?;
                    match e {
                        Expr::Ident(n) => Ok(Stmt::Assign(n, rhs, show)),
                        Expr::Index(n, args) => Ok(Stmt::IndexAssign(n, args, rhs, show)),
                        _ => bail!(error::invalid_assignment_target().at(line)),
                    }
                } else {
                    let show = self.end_stmt()?;
                    Ok(Stmt::Expr(e, show))
                }
            }
        }
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
            Token::Eof | Token::End | Token::Else | Token::ElseIf => Ok(true),
            _ => {
                let line = self.line();
                bail!(error::unexpected_token(self.peek()).at(line))
            }
        }
    }

    // ---- expressions -------------------------------------------------

    pub fn parse_expr(&mut self) -> R<Expr> {
        self.parse_oror()
    }

    fn parse_oror(&mut self) -> R<Expr> {
        let mut left = self.parse_andand()?;
        while self.eat(&Token::OrOr) {
            let right = self.parse_andand()?;
            left = Expr::Binary(BinOp::OrOr, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_andand(&mut self) -> R<Expr> {
        let mut left = self.parse_or()?;
        while self.eat(&Token::AndAnd) {
            let right = self.parse_or()?;
            left = Expr::Binary(BinOp::AndAnd, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_or(&mut self) -> R<Expr> {
        let mut left = self.parse_and()?;
        while self.eat(&Token::Or) {
            let right = self.parse_and()?;
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> R<Expr> {
        let mut left = self.parse_cmp()?;
        while self.eat(&Token::And) {
            let right = self.parse_cmp()?;
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> R<Expr> {
        let mut left = self.parse_range()?;
        loop {
            let op = match self.peek() {
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => return Ok(left),
            };
            self.next();
            let right = self.parse_range()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn parse_range(&mut self) -> R<Expr> {
        // Bare `:` inside an index: `A(:, 1)`
        if self.in_index > 0
            && self.peek() == &Token::Colon
            && matches!(self.peek_at(1), Token::Comma | Token::RParen)
        {
            self.next();
            return Ok(Expr::Colon);
        }
        let first = self.parse_add()?;
        if !self.eat(&Token::Colon) {
            return Ok(first);
        }
        let second = self.parse_add()?;
        if self.eat(&Token::Colon) {
            let third = self.parse_add()?;
            Ok(Expr::Range(
                Box::new(first),
                Some(Box::new(second)),
                Box::new(third),
            ))
        } else {
            Ok(Expr::Range(Box::new(first), None, Box::new(second)))
        }
    }

    fn parse_add(&mut self) -> R<Expr> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => return Ok(left),
            };
            self.next();
            let right = self.parse_mul()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn parse_mul(&mut self) -> R<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Backslash => BinOp::LDiv,
                Token::DotStar => BinOp::EMul,
                Token::DotSlash => BinOp::EDiv,
                Token::DotBackslash => BinOp::ELDiv,
                _ => return Ok(left),
            };
            self.next();
            let right = self.parse_unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn parse_unary(&mut self) -> R<Expr> {
        match self.peek() {
            Token::Minus => {
                self.next();
                Ok(Expr::Neg(Box::new(self.parse_unary()?)))
            }
            Token::Plus => {
                self.next();
                self.parse_unary()
            }
            Token::Not => {
                self.next();
                Ok(Expr::Not(Box::new(self.parse_unary()?)))
            }
            _ => self.parse_power(),
        }
    }

    fn parse_power(&mut self) -> R<Expr> {
        let mut base = self.parse_postfix()?;
        loop {
            let op = match self.peek() {
                Token::Caret => BinOp::Pow,
                Token::DotCaret => BinOp::EPow,
                _ => return Ok(base),
            };
            self.next();
            // MATLAB allows a unary sign directly in the exponent: 2^-1
            let exp = self.parse_power_operand()?;
            base = Expr::Binary(op, Box::new(base), Box::new(exp));
        }
    }

    fn parse_power_operand(&mut self) -> R<Expr> {
        match self.peek() {
            Token::Minus => {
                self.next();
                Ok(Expr::Neg(Box::new(self.parse_power_operand()?)))
            }
            Token::Plus => {
                self.next();
                self.parse_power_operand()
            }
            Token::Not => {
                self.next();
                Ok(Expr::Not(Box::new(self.parse_power_operand()?)))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> R<Expr> {
        let mut e = self.parse_primary()?;
        while self.eat(&Token::Transpose) {
            e = Expr::Transpose(Box::new(e));
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> R<Expr> {
        let line = self.line();
        match self.next() {
            Token::Num(v) => Ok(Expr::Num(v)),
            Token::Str(s) => Ok(Expr::Str(s)),
            Token::LParen => {
                let e = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(e)
            }
            Token::LBracket => self.parse_matrix(),
            Token::End if self.in_index > 0 => Ok(Expr::End),
            Token::Ident(name) => {
                if self.peek() == &Token::LParen {
                    self.next();
                    let mut args = Vec::new();
                    self.in_index += 1;
                    if self.peek() != &Token::RParen {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                    }
                    self.in_index -= 1;
                    self.expect(Token::RParen)?;
                    Ok(Expr::Index(name, args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            t => bail!(error::unexpected_in_expression(&t).at(line)),
        }
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
                _ => row.push(self.parse_expr()?),
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
        p.parse_program()
    }

    /// A statement and the line it is expected to start on.
    fn at(line: u32, stmt: Stmt) -> Located {
        Located { stmt, line }
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

    // ---- precedence and associativity ---------------------------------

    #[test]
    fn unary_minus_binds_looser_than_power() {
        assert_eq!(parse_expr("-2^2"), neg(bin(BinOp::Pow, num(2.0), num(2.0))));
    }

    #[test]
    fn unary_sign_is_allowed_in_the_exponent() {
        assert_eq!(parse_expr("2^-1"), bin(BinOp::Pow, num(2.0), neg(num(1.0))));
        assert_eq!(parse_expr("2^+1"), bin(BinOp::Pow, num(2.0), num(1.0)));
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
            vec![at(1, Stmt::Assign("x".to_string(), num(3.0), false))]
        );
    }

    #[test]
    fn missing_semicolon_shows_result() {
        assert_eq!(
            parse("x = 3"),
            vec![at(1, Stmt::Assign("x".to_string(), num(3.0), true))]
        );
    }

    #[test]
    fn indexed_assignment_uses_index_assign() {
        assert_eq!(
            parse("x(end+1) = 3"),
            vec![at(
                1,
                Stmt::IndexAssign(
                    "x".to_string(),
                    vec![bin(BinOp::Add, Expr::End, num(1.0))],
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
                Stmt::Expr(
                    Expr::Index("A".to_string(), vec![Expr::Colon, num(1.0)]),
                    true
                )
            )]
        );
    }

    #[test]
    fn end_inside_an_index_is_expr_end() {
        assert_eq!(
            parse_expr("a(end)"),
            Expr::Index("a".to_string(), vec![Expr::End])
        );
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
                at(1, Stmt::Assign("x".to_string(), num(1.0), false)),
                at(2, Stmt::Assign("y".to_string(), num(2.0), true)),
            ]
        );
    }

    #[test]
    fn call_with_several_arguments() {
        assert_eq!(
            parse("disp(1, x);"),
            vec![at(
                1,
                Stmt::Expr(
                    Expr::Index("disp".to_string(), vec![num(1.0), ident("x")]),
                    false
                )
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
                        (ident("a"), vec![at(2, Stmt::Expr(num(1.0), false))]),
                        (ident("b"), vec![at(4, Stmt::Expr(num(2.0), false))]),
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
                    vec![(ident("a"), vec![at(2, Stmt::Expr(num(1.0), false))])],
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
                    vec![at(
                        1,
                        Stmt::Expr(Expr::Index("disp".to_string(), vec![ident("k")]), false)
                    )],
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
                            vec![(
                                bin(BinOp::Gt, ident("i"), num(1.0)),
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
        let stmts = Parser::new(toks).parse_program().expect("should parse");
        assert_eq!(stmts.iter().map(|s| s.line).collect::<Vec<_>>(), vec![1, 1]);
    }
}
