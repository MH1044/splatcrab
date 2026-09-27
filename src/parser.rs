//! Recursive-descent parser producing statements and expressions.
//!
//! Precedence (loosest to tightest), following MATLAB:
//!   ||   &&   |   &   comparison   :   + -   * / \ .* ./   unary - ~   ^ .^   transpose

use crate::lexer::Token;

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
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    For(String, Expr, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Break,
    Continue,
}

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    /// Depth of `name( ... )` argument lists we are inside; enables `end` and bare `:`.
    in_index: usize,
}

type R<T> = Result<T, String>;

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Parser {
            toks,
            pos: 0,
            in_index: 0,
        }
    }

    fn peek(&self) -> &Token {
        &self.toks[self.pos]
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
            Err(format!("expected {:?} but found {:?}", t, self.peek()))
        }
    }

    fn skip_terminators(&mut self) {
        while matches!(self.peek(), Token::Semi | Token::Comma | Token::Newline) {
            self.next();
        }
    }

    pub fn parse_program(&mut self) -> R<Vec<Stmt>> {
        let stmts = self.parse_block(&[])?;
        if self.peek() != &Token::Eof {
            return Err(format!("unexpected {:?}", self.peek()));
        }
        Ok(stmts)
    }

    fn parse_block(&mut self, stops: &[Token]) -> R<Vec<Stmt>> {
        let mut out = Vec::new();
        loop {
            self.skip_terminators();
            let p = self.peek();
            if *p == Token::Eof || stops.contains(p) {
                return Ok(out);
            }
            out.push(self.parse_stmt()?);
        }
    }

    fn parse_stmt(&mut self) -> R<Stmt> {
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
                        t => return Err(format!("expected 'end' to close 'if', found {:?}", t)),
                    }
                }
            }
            Token::For => {
                self.next();
                let name = match self.next() {
                    Token::Ident(n) => n,
                    t => return Err(format!("expected loop variable after 'for', found {:?}", t)),
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
            Token::End | Token::Else | Token::ElseIf => Err(format!(
                "unexpected {:?} with no matching block",
                self.peek()
            )),
            _ => {
                let e = self.parse_expr()?;
                if self.peek() == &Token::Assign {
                    self.next();
                    let rhs = self.parse_expr()?;
                    let show = self.end_stmt()?;
                    match e {
                        Expr::Ident(n) => Ok(Stmt::Assign(n, rhs, show)),
                        Expr::Index(n, args) => Ok(Stmt::IndexAssign(n, args, rhs, show)),
                        _ => Err("invalid assignment target".to_string()),
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
            t => Err(format!("unexpected {:?}", t)),
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
            t => Err(format!("unexpected {:?} in expression", t)),
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
                Token::Eof => return Err("unterminated matrix literal: missing ']'".to_string()),
                _ => row.push(self.parse_expr()?),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_expr(src: &str) -> Expr {
        let toks = lex(src).expect("lex should succeed");
        let mut p = Parser::new(toks);
        p.parse_expr().expect("expression should parse")
    }

    fn parse(src: &str) -> Vec<Stmt> {
        let toks = lex(src).expect("lex should succeed");
        let mut p = Parser::new(toks);
        p.parse_program().expect("program should parse")
    }

    fn parse_result(src: &str) -> Result<Vec<Stmt>, String> {
        let toks = lex(src).expect("lex should succeed");
        let mut p = Parser::new(toks);
        p.parse_program()
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
            vec![Stmt::Assign("x".to_string(), num(3.0), false)]
        );
    }

    #[test]
    fn missing_semicolon_shows_result() {
        assert_eq!(
            parse("x = 3"),
            vec![Stmt::Assign("x".to_string(), num(3.0), true)]
        );
    }

    #[test]
    fn indexed_assignment_uses_index_assign() {
        assert_eq!(
            parse("x(end+1) = 3"),
            vec![Stmt::IndexAssign(
                "x".to_string(),
                vec![bin(BinOp::Add, Expr::End, num(1.0))],
                num(3.0),
                true,
            )]
        );
    }

    #[test]
    fn bare_colon_in_an_index_is_expr_colon() {
        assert_eq!(
            parse("A(:, 1)"),
            vec![Stmt::Expr(
                Expr::Index("A".to_string(), vec![Expr::Colon, num(1.0)]),
                true
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
            vec![Stmt::Expr(bin(BinOp::Add, num(1.0), num(2.0)), true)]
        );
    }

    #[test]
    fn statements_separated_by_newlines_and_semicolons() {
        assert_eq!(
            parse("x = 1;\ny = 2\n"),
            vec![
                Stmt::Assign("x".to_string(), num(1.0), false),
                Stmt::Assign("y".to_string(), num(2.0), true),
            ]
        );
    }

    #[test]
    fn call_with_several_arguments() {
        assert_eq!(
            parse("disp(1, x);"),
            vec![Stmt::Expr(
                Expr::Index("disp".to_string(), vec![num(1.0), ident("x")]),
                false
            )]
        );
    }

    // ---- blocks ----------------------------------------------------------

    #[test]
    fn if_elseif_else_block() {
        assert_eq!(
            parse("if a\n  1;\nelseif b\n  2;\nelse\n  3;\nend\n"),
            vec![Stmt::If(
                vec![
                    (ident("a"), vec![Stmt::Expr(num(1.0), false)]),
                    (ident("b"), vec![Stmt::Expr(num(2.0), false)]),
                ],
                Some(vec![Stmt::Expr(num(3.0), false)]),
            )]
        );
    }

    #[test]
    fn if_without_else_has_no_tail() {
        assert_eq!(
            parse("if a\n  1;\nend"),
            vec![Stmt::If(
                vec![(ident("a"), vec![Stmt::Expr(num(1.0), false)])],
                None
            )]
        );
    }

    #[test]
    fn for_loop_with_comma_separators() {
        assert_eq!(
            parse("for k = 1:3, disp(k); end"),
            vec![Stmt::For(
                "k".to_string(),
                range(num(1.0), None, num(3.0)),
                vec![Stmt::Expr(
                    Expr::Index("disp".to_string(), vec![ident("k")]),
                    false
                )],
            )]
        );
    }

    #[test]
    fn while_loop_with_break() {
        assert_eq!(
            parse("while a\nbreak\nend"),
            vec![Stmt::While(ident("a"), vec![Stmt::Break])]
        );
    }

    #[test]
    fn while_loop_with_continue() {
        assert_eq!(
            parse("while a\ncontinue;\nend"),
            vec![Stmt::While(ident("a"), vec![Stmt::Continue])]
        );
    }

    #[test]
    fn nested_blocks() {
        assert_eq!(
            parse("for i = 1:2\n  if i > 1\n    break\n  end\nend"),
            vec![Stmt::For(
                "i".to_string(),
                range(num(1.0), None, num(2.0)),
                vec![Stmt::If(
                    vec![(bin(BinOp::Gt, ident("i"), num(1.0)), vec![Stmt::Break],)],
                    None
                )],
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
}
