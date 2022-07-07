use crate::{
    error::{ParseError, ParseResult},
    lexer::Lexer,
    tokens::{Token, TokenType as Tkt},
};

use self::ast::{Bind, Def, Expr, ExprKind, Literal, MatchArm, Pattern, Stmt, StmtKind, VarDecl};

pub mod ast;

pub struct Parser {
    lexer: Lexer,
    current: Token,
}

impl Parser {
    pub fn new(lexer: Lexer) -> ParseResult<Self> {
        let mut this = Parser {
            lexer,
            current: Token::default(),
        };
        this.next()?;
        Ok(this)
    }

    pub fn parse(mut self) -> ParseResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        while self.current.token != Tkt::Eof {
            match self.current.token {
                Tkt::Type => {
                    stmts.push(self.type_()?);
                }

                Tkt::Def => stmts.push(self.def_global()?),
                Tkt::Let => stmts.push(self.let_global()?),

                ref other => self.throw(format!("Unexpected token '{other}'"))?,
            }
        }

        Ok(stmts)
    }

    pub fn let_global(&mut self) -> ParseResult<Stmt> {
        let line = self.current.line;
        let column = self.current.column;

        self.expect(Tkt::Let)?;

        let bind = self.pattern()?;

        self.expect(Tkt::Assign)?;

        let value = self.expr()?;

        Ok(Stmt::new(StmtKind::Let { bind, value }, line, column))
    }

    pub fn parse_expr(mut self) -> ParseResult<Expr> {
        self.expr()
    }

    fn type_(&mut self) -> ParseResult<Stmt> {
        self.expect(Tkt::Type)?;
        let line = self.current.line;
        let column = self.current.column;

        let name = self.var_decl()?;

        self.expect(Tkt::Assign)?;

        let mut variants = vec![];

        while self.current.token != Tkt::With {
            let mut variant = self.var_decl()?.as_str().to_string();
            variant.insert(0, '.');
            variant.insert_str(0, name.as_str());

            let mut args = vec![];
            while let Tkt::Name(name) = self.current.token {
                args.push(name);
                self.next()?;
            }

            variants.push((variant.into(), args));

            if self.current.token != Tkt::With {
                self.expect_and_skip(Tkt::Bar)?;
            }
        }

        self.expect(Tkt::With)?;

        let mut members = vec![];

        while self.current.token != Tkt::End {
            self.expect(Tkt::Def)?;
            let bind = self.var_decl()?;
            let value = self.function()?;

            members.push(Def { bind, value })
        }

        self.expect(Tkt::End)?;

        Ok(Stmt::new(
            StmtKind::Type {
                name,
                variants,
                members,
            },
            line,
            column,
        ))
    }

    fn def_global(&mut self) -> ParseResult<Stmt> {
        let line = self.current.line;
        let column = self.current.column;

        self.expect(Tkt::Def)?;

        let bind = self.var_decl()?;
        let value = self.function()?;

        Ok(Stmt::new(StmtKind::Def(Def { bind, value }), line, column))
    }

    fn next(&mut self) -> ParseResult<()> {
        self.current = self.lexer.next().unwrap()?;
        Ok(())
    }

    fn throw<T>(&self, err: impl Into<String>) -> ParseResult<T> {
        ParseError::throw(self.current.line, self.current.column, err.into())
    }

    fn expect(&mut self, expected: Tkt) -> ParseResult<()> {
        self.assert(expected)?;
        self.next()
    }

    fn assert(&mut self, expected: Tkt) -> ParseResult<()> {
        if self.current.token == expected {
            Ok(())
        } else {
            self.throw(format!(
                "Expected {}, found '{}'",
                expected, self.current.token
            ))
        }
    }

    fn skip(&mut self, tokens: Tkt) -> ParseResult<()> {
        while self.current.token == tokens {
            self.next()?;
        }
        Ok(())
    }

    fn expect_and_skip(&mut self, tokens: Tkt) -> ParseResult<()> {
        self.expect(tokens.clone())?;
        self.skip(tokens)
    }

    fn state(&self) -> (Token, (usize, usize, usize)) {
        (self.current.clone(), self.lexer.state())
    }

    fn set_state(&mut self, (current, state): (Token, (usize, usize, usize))) {
        self.current = current;
        self.lexer.set_state(state);
    }

    fn expr(&mut self) -> ParseResult<Expr> {
        self.pipe()
    }

    fn condition(&mut self) -> ParseResult<Expr> {
        self.assert(Tkt::If)?;

        self.next()?;

        let line = self.current.line;
        let column = self.current.column;

        let cond = self.expr()?;

        self.expect(Tkt::Then)?;

        let then = self.expr()?;

        let else_ = match self.current.token {
            Tkt::Else => {
                self.next()?;
                self.expr()?
            }
            _ => self.throw("Expected 'else' after 'if'")?,
        };

        Ok(Expr::new(
            ExprKind::If {
                cond: Box::new(cond),
                then: Box::new(then),
                else_: Box::new(else_),
            },
            line,
            column,
        ))
    }

    fn args(&mut self) -> ParseResult<Vec<Pattern>> {
        let mut args = vec![self.pattern()?];
        while self.current.token != Tkt::Assign {
            args.push(self.pattern()?);
        }
        Ok(args)
    }

    fn become_(&mut self) -> ParseResult<Expr> {
        self.expect(Tkt::FatArrow)?;

        let line = self.current.line;
        let column = self.current.column;

        let callee = self.expr()?;

        match callee.kind {
            ExprKind::App { callee, args, .. } => Ok(Expr::new(
                ExprKind::App {
                    callee,
                    args,
                    tail: true,
                },
                line,
                column,
            )),
            _ => self.throw("'=>' can only be used on function calls"),
        }
    }

    fn match_(&mut self) -> ParseResult<Expr> {
        self.expect(Tkt::Match)?;

        let line = self.current.line;
        let column = self.current.column;

        let expr = Box::new(self.expr()?);

        self.expect(Tkt::With)?;

        let mut arms = vec![];

        let mut last_state = self.state();
        while let Ok(arm) = self.match_arm() {
            arms.push(arm);
            last_state = self.state();
        }
        self.set_state(last_state);

        Ok(Expr::new(ExprKind::Match { expr, arms }, line, column))
    }

    fn match_arm(&mut self) -> ParseResult<MatchArm> {
        let line = self.current.line;
        let column = self.current.column;
        self.expect(Tkt::Bar)?;

        let cond = self.pattern()?;

        let guard = if self.current.token == Tkt::If {
            self.next()?;
            Some(self.expr()?)
        } else {
            None
        };

        self.expect(Tkt::Arrow)?;

        let body = self.expr()?;

        Ok(MatchArm::new(cond, body, guard, line, column))
    }

    fn try_(&mut self) -> ParseResult<Expr> {
        self.expect(Tkt::Try)?;

        let line = self.current.line;
        let column = self.current.column;

        let body = Box::new(self.expr()?);

        self.expect(Tkt::Rescue)?;

        let bind = self.var_decl()?;

        let rescue = Box::new(self.expr()?);

        Ok(Expr::new(
            ExprKind::Try { body, bind, rescue },
            line,
            column,
        ))
    }

    fn fn_(&mut self) -> ParseResult<Expr> {
        self.expect(Tkt::Fn)?;
        self.function()
    }

    fn function(&mut self) -> ParseResult<Expr> {
        let line = self.current.line;
        let column = self.current.column;

        let args = self.args()?;

        let body = self.fn_body()?;

        Ok(Expr::new(
            ExprKind::Lambda {
                args,
                body: Box::new(body),
            },
            line,
            column,
        ))
    }

    fn fn_body(&mut self) -> ParseResult<Expr> {
        self.expect(Tkt::Assign)?;
        self.expr()
    }

    fn var_decl(&mut self) -> ParseResult<VarDecl> {
        let name = match self.current.token {
            Tkt::Name(id) => id,
            ref other => self.throw(format!("Expected name, found '{}'", other))?,
        };

        self.next()?;

        Ok(name)
    }

    fn pattern(&mut self) -> ParseResult<Pattern> {
        let pat = match self.current.token {
            Tkt::Num(n) => Pattern::Lit(Literal::Num(n)),
            Tkt::Str(ref s) => Pattern::Lit(Literal::Str(s.to_string())),
            Tkt::Nil => Pattern::Lit(Literal::Unit),
            Tkt::True => Pattern::Lit(Literal::Bool(true)),
            Tkt::False => Pattern::Lit(Literal::Bool(false)),
            Tkt::Name(id) => Pattern::Id(id),

            Tkt::Lparen => {
                self.next()?;

                let mut path = vec![self.var_decl()?];
                while self.current.token == Tkt::Dot {
                    self.next()?;
                    path.push(self.var_decl()?);
                }

                let mut pats = vec![];
                while self.current.token != Tkt::Rparen {
                    pats.push(self.pattern()?);
                }

                Pattern::Variant(path, pats)
            }

            ref other => self.throw(format!("Expected pattern, found '{other}'"))?,
        };

        self.next()?;

        Ok(pat)
    }

    fn let_(&mut self) -> ParseResult<Expr> {
        let line = self.current.line;
        let column = self.current.column;

        self.expect(Tkt::Let)?;

        let bind = self.pattern()?;

        self.expect(Tkt::Assign)?;

        let value = self.expr()?;

        self.expect(Tkt::In)?;

        let body = self.expr()?;

        Ok(Expr::new(
            ExprKind::Let {
                bind,
                value: Box::new(value),
                body: Box::new(body),
            },
            line,
            column,
        ))
    }

    fn def_(&mut self) -> ParseResult<Expr> {
        self.expect(Tkt::Def)?;

        let line = self.current.line;
        let column = self.current.column;

        let name = self.var_decl()?;
        let value = self.function()?;

        self.expect(Tkt::In)?;

        let body = self.expr()?;

        Ok(Expr::new(
            ExprKind::Def {
                bind: Bind::new(name, Box::new(value), line, column),
                body: Box::new(body),
            },
            line,
            column,
        ))
    }

    fn pipe(&mut self) -> ParseResult<Expr> {
        let mut left = self.logic_or()?;

        while self.current.token == Tkt::Pipe {
            self.next()?;

            let line = self.current.line;
            let column = self.current.column;

            left = Expr::new(
                ExprKind::App {
                    args: vec![left],
                    callee: Box::new(self.logic_or()?),
                    tail: false,
                },
                line,
                column,
            );
        }

        Ok(left)
    }

    fn logic_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.logic_and()?;

        while let Tkt::Or = self.current.token {
            let op: ast::BinOp = self.current.token.clone().try_into().unwrap();

            self.next()?;
            let right = self.logic_and()?;

            let line = left.line();
            let column = left.column();

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                line,
                column,
            );
        }

        Ok(left)
    }

    fn logic_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.is()?;

        while let Tkt::And = self.current.token {
            let op = self.current.token.clone().try_into().unwrap();

            self.next()?;
            let right = self.is()?;

            let line = left.line();
            let column = left.column();

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                line,
                column,
            );
        }

        Ok(left)
    }

    fn is(&mut self) -> ParseResult<Expr> {
        let mut left = self.eq()?;

        while let Tkt::Is = self.current.token {
            let line = left.line();
            let column = left.column();

            self.next()?;
            let right = self.eq()?;

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: ast::BinOp::Is,
                    right: Box::new(right),
                },
                line,
                column,
            );
        }

        Ok(left)
    }

    fn eq(&mut self) -> ParseResult<Expr> {
        let mut left = self.cmp()?;

        while let Tkt::Eq | Tkt::Ne = self.current.token {
            let op = self.current.clone();
            self.next()?;
            let right = self.cmp()?;

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: op.token.try_into().unwrap(),
                    right: Box::new(right),
                },
                op.line,
                op.column,
            );
        }

        Ok(left)
    }

    fn cmp(&mut self) -> ParseResult<Expr> {
        let mut left = self.cons()?;

        while let Tkt::Less | Tkt::LessEq | Tkt::Greater | Tkt::GreaterEq = self.current.token {
            let op = self.current.clone();
            self.next()?;
            let right = self.cons()?;

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: op.token.try_into().unwrap(),
                    right: Box::new(right),
                },
                op.line,
                op.column,
            );
        }

        Ok(left)
    }

    fn cons(&mut self) -> ParseResult<Expr> {
        let mut left = self.bitwise()?;

        while let Tkt::Cons = self.current.token {
            let op = self.current.clone();
            self.next()?;
            let right = self.cons()?;

            left = Expr::new(
                ExprKind::Cons {
                    head: Box::new(left),
                    tail: Box::new(right),
                },
                op.line,
                op.column,
            );
        }

        Ok(left)
    }

    fn bitwise(&mut self) -> ParseResult<Expr> {
        let mut left = self.term()?;

        while let Tkt::BitOr | Tkt::BitAnd | Tkt::BitXor | Tkt::Shr | Tkt::Shl = self.current.token
        {
            let op = self.current.clone();
            self.next()?;
            let right = self.term()?;

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: op.token.try_into().unwrap(),
                    right: Box::new(right),
                },
                op.line,
                op.column,
            );
        }

        Ok(left)
    }

    fn term(&mut self) -> ParseResult<Expr> {
        let mut left = self.fact()?;

        while let Tkt::Add | Tkt::Sub = self.current.token {
            let op = self.current.clone();
            self.next()?;
            let right = self.fact()?;

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: op.token.try_into().unwrap(),
                    right: Box::new(right),
                },
                op.line,
                op.column,
            );
        }

        Ok(left)
    }

    fn fact(&mut self) -> ParseResult<Expr> {
        let mut left = self.prefix()?;

        while let Tkt::Mul | Tkt::Div | Tkt::Rem = self.current.token {
            let op = self.current.clone();
            self.next()?;
            let right = self.prefix()?;

            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: op.token.try_into().unwrap(),
                    right: Box::new(right),
                },
                op.line,
                op.column,
            );
        }

        Ok(left)
    }

    fn prefix(&mut self) -> ParseResult<Expr> {
        if let Tkt::Sub | Tkt::Not = &self.current.token {
            let op = self.current.clone();
            self.next()?;
            let right = self.prefix()?;
            Ok(Expr::new(
                ExprKind::UnOp(op.token.try_into().unwrap(), Box::new(right)),
                op.line,
                op.column,
            ))
        } else {
            self.call()
        }
    }

    fn call(&mut self) -> ParseResult<Expr> {
        let callee = self.method_ref()?;

        let line = self.current.line;
        let column = self.current.column;

        let mut last_state = self.state();
        let mut args = vec![];

        while let Ok(arg) = self.method_ref() {
            args.push(arg);
            last_state = self.state();
        }

        self.set_state(last_state);

        if args.is_empty() {
            Ok(callee)
        } else {
            let tail = false;
            let callee = Box::new(callee);

            Ok(Expr::new(
                ExprKind::App { callee, args, tail },
                line,
                column,
            ))
        }
    }

    fn method_ref(&mut self) -> ParseResult<Expr> {
        let mut ty = self.primary()?;

        while self.current.token == Tkt::Dot {
            self.next()?;
            let method = self.var_decl()?;

            ty = Expr::new(
                ExprKind::MethodRef {
                    ty: Box::new(ty),
                    method,
                },
                self.current.line,
                self.current.column,
            );
        }

        Ok(ty)
    }

    fn list(&mut self) -> ParseResult<Expr> {
        let line = self.current.line;
        let column = self.current.column;

        self.expect(Tkt::Lbrack)?;

        let mut exprs = Vec::new();
        while self.current.token != Tkt::Rbrack {
            exprs.push(self.expr()?); // compiles the argument

            if self.current.token != Tkt::Rbrack {
                self.expect_and_skip(Tkt::Comma)?;
            }
        }

        self.expect(Tkt::Rbrack)?;

        Ok(Expr::new(ExprKind::List(exprs), line, column))
    }

    fn tuple(&mut self) -> ParseResult<Expr> {
        let line = self.current.line;
        let column = self.current.column;

        let mut exprs = Vec::new();

        self.expect(Tkt::Lparen)?;

        while self.current.token != Tkt::Rparen {
            exprs.push(self.expr()?); // compiles the argument

            if self.current.token != Tkt::Rparen {
                self.expect_and_skip(Tkt::Comma)?;
            }
        }

        self.expect(Tkt::Rparen)?;

        if exprs.len() == 1 {
            Ok(exprs.pop().unwrap())
        } else {
            Ok(Expr::new(ExprKind::Tuple(exprs), line, column))
        }
    }

    fn primary(&mut self) -> ParseResult<Expr> {
        let line = self.current.line;
        let column = self.current.column;

        let obj = match self.current.token.clone() {
            // literals
            Tkt::Num(n) => {
                self.next()?;
                Expr::new(ExprKind::Lit(Literal::Num(n)), line, column)
            }
            Tkt::Str(s) => {
                self.next()?;
                Expr::new(ExprKind::Lit(Literal::Str(s)), line, column)
            }
            Tkt::True => {
                self.next()?;
                Expr::new(ExprKind::Lit(Literal::Bool(true)), line, column)
            }
            Tkt::False => {
                self.next()?;
                Expr::new(ExprKind::Lit(Literal::Bool(false)), line, column)
            }
            Tkt::Name(s) => {
                self.next()?;
                Expr::new(ExprKind::Var(s), line, column)
            }
            Tkt::Sym(s) => {
                self.next()?;
                Expr::new(ExprKind::Lit(Literal::Sym(s)), line, column)
            }
            Tkt::Lbrack => self.list()?,
            Tkt::Lparen => self.tuple()?,
            Tkt::Nil => {
                self.next()?;
                Expr::new(ExprKind::Lit(Literal::Unit), line, column)
            }

            // keywords
            Tkt::Let => self.let_()?,
            Tkt::Def => self.def_()?,
            Tkt::If => self.condition()?,
            Tkt::Fn => self.fn_()?,
            Tkt::FatArrow => self.become_()?,
            Tkt::Match => self.match_()?,
            Tkt::Try => self.try_()?,

            // not supported
            other => self.throw(format!("unexpected token '{}'", other))?,
        };

        Ok(obj)
    }
}
