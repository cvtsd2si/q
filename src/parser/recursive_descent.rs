use crate::parser::ast::{
    BinaryExpressionKind, BlockItem, Declaration, Expression, LiteralVal, Param, Program,
    Statement, Type, TypeInterner, UnaryExpressionKind,
};
use crate::tokenizer::token::{NumberKind, NumberToken, Span, Token, TokenKind};
use crate::{QError, QErrorKind};
use std::collections::VecDeque;

pub struct Parser<'src, 'types> {
    src: &'src str,
    tokens: VecDeque<Token>,
    curr: Option<Token>,
    prev: Option<Token>,
    curr_ret_ty: Option<Type>,
    types: &'types mut TypeInterner,
}

impl<'src, 'types> Parser<'src, 'types> {
    pub fn new(src: &'src str, tokens: VecDeque<Token>, types: &'types mut TypeInterner) -> Self {
        Self {
            src,
            tokens,
            curr: None,
            prev: None,
            curr_ret_ty: None,
            types,
        }
    }

    pub fn parse(&mut self) -> Result<Program, QError> {
        self.advance();

        let mut block_items = vec![];
        while self.curr.is_some() {
            block_items.push(self.parse_block_item()?);
        }

        Ok(Program { block_items })
    }

    fn advance(&mut self) {
        if self.prev.is_none() && self.curr.is_none() {
            self.curr = self.tokens.pop_front();
            self.prev = self.curr;
        } else {
            self.prev = self.curr;
            self.curr = self.tokens.pop_front();
        }
    }

    fn consume(&mut self, kind: TokenKind) -> Result<Token, QError> {
        match self.curr {
            Some(token) if token.kind == kind => {
                self.advance();

                Ok(token)
            }
            Some(token) => Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                msg: format!("Encountered unexpected token: {:?}", token),
                span: token.span,
            }),
            None => Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                msg: "Encountered the end of the token stream.".to_string(),
                span: Span { start: 0, end: 0 },
            }),
        }
    }

    fn consume_primitive(&mut self) -> Result<Token, QError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::U8
                | TokenKind::I8
                | TokenKind::U16
                | TokenKind::I16
                | TokenKind::U32
                | TokenKind::I32
                | TokenKind::U64
                | TokenKind::I64
                | TokenKind::F32
                | TokenKind::F64
                | TokenKind::Bool
                | TokenKind::Str => {
                    self.advance();

                    Ok(token)
                }
                _ => Err(QError {
                    code: 1,
                    kind: QErrorKind::Parse,
                    msg: format!("Encountered unknown primitive token kind: {:?}", token),
                    span: token.span,
                }),
            },
            None => Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                msg: "Encountered the end of the token stream.".to_string(),
                span: Span { start: 0, end: 0 },
            }),
        }
    }

    fn parse_block_item(&mut self) -> Result<BlockItem, QError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Fn => Ok(BlockItem::Decl(self.parse_decl()?)),
                TokenKind::Ret
                | TokenKind::Let
                | TokenKind::Print
                | TokenKind::If
                | TokenKind::While
                | TokenKind::Do
                | TokenKind::For
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Goto
                | TokenKind::Identifier
                | TokenKind::Star
                | TokenKind::Ampersand
                | TokenKind::Minus
                | TokenKind::Bang
                | TokenKind::Tilde
                | TokenKind::LParen => self.parse_stmt().map(BlockItem::Stmt),
                _ => Err(QError {
                    code: 1,
                    kind: QErrorKind::Parse,
                    span: Span { start: 0, end: 0 },
                    msg: format!("encountered {:?} while trying to parse a block_item", token),
                }),
            },
            None => Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                span: Span { start: 0, end: 0 },
                msg: "Encountered the end of token stream".to_string(),
            }),
        }
    }

    fn parse_decl(&mut self) -> Result<Declaration, QError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Fn => self.parse_fn(),
                _ => unimplemented!(),
            },
            None => Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                span: Span { start: 0, end: 0 },
                msg: "Encountered the end of token stream".to_string(),
            }),
        }
    }

    fn parse_stmt(&mut self) -> Result<Statement, QError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Let => self.parse_let(),
                TokenKind::Ret => self.parse_ret(),
                TokenKind::Print => self.parse_print(),
                TokenKind::If => self.parse_if(),
                TokenKind::While => self.parse_while(),
                TokenKind::Do => self.parse_do_while(),
                TokenKind::For => self.parse_for(),
                TokenKind::Loop => self.parse_loop(),
                TokenKind::Break => self.parse_break(),
                TokenKind::Continue => self.parse_continue(),
                TokenKind::Goto => self.parse_goto(),
                TokenKind::LBrace => self.parse_block().map(|body| Statement::Block { body }),
                TokenKind::Identifier
                    if self
                        .tokens
                        .front()
                        .is_some_and(|t| t.kind == TokenKind::Colon) =>
                {
                    self.parse_labeled()
                }
                TokenKind::Identifier
                | TokenKind::Star
                | TokenKind::Ampersand
                | TokenKind::Minus
                | TokenKind::Bang
                | TokenKind::Tilde
                | TokenKind::LParen => self.parse_expr_stmt(),
                _ => Err(QError {
                    code: 1,
                    kind: QErrorKind::Parse,
                    span: Span { start: 0, end: 0 },
                    msg: format!("encountered {:?} while trying to parse a stmt", token),
                }),
            },
            None => Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                span: Span { start: 0, end: 0 },
                msg: "Encountered the end of token stream".to_string(),
            }),
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        matches!(self.curr, Some(front) if front.kind == kind)
    }

    fn check_many_greedy(&mut self, kinds: &[TokenKind]) -> bool {
        let matched = kinds.iter().any(|&kind| self.check(kind));

        if matched {
            self.advance();
        }

        matched
    }

    fn parse_fn(&mut self) -> Result<Declaration, QError> {
        self.consume(TokenKind::Fn)?;

        let name = self.consume(TokenKind::Identifier)?;

        self.consume(TokenKind::LParen)?;

        let mut params = vec![];
        while !self.check(TokenKind::RParen) {
            let param_name = self.consume(TokenKind::Identifier)?;
            self.consume(TokenKind::Colon)?;
            let param_ty = self.parse_type()?;

            params.push(Param {
                name: self.src_from_span(param_name.span),
                ty: param_ty,
            });

            if self.check(TokenKind::RParen) {
                break;
            }

            self.consume(TokenKind::Comma)?;
        }

        self.consume(TokenKind::RParen)?;

        let ret_ty = if self.check(TokenKind::Arrow) {
            self.consume(TokenKind::Arrow)?;

            Some(Type::from(self.parse_type()?))
        } else {
            None
        };

        self.curr_ret_ty = ret_ty;

        let body = self.parse_stmt()?;

        self.curr_ret_ty = None;

        Ok(Declaration::Fn {
            name: self.src_from_span(name.span),
            ret: ret_ty,
            params,
            body,
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, QError> {
        self.consume(TokenKind::LBrace)?;

        let mut body = vec![];
        while !self.check(TokenKind::RBrace) {
            body.push(self.parse_stmt()?);
        }

        self.consume(TokenKind::RBrace)?;

        Ok(body)
    }

    fn parse_ret(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Ret)?;

        let value = if self.check(TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Ret {
            value,
            ret_ty: self.curr_ret_ty,
        })
    }

    fn parse_print(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Print)?;
        let value = self.parse_expr()?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Print { value })
    }

    fn parse_let(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Let)?;
        let name = self.consume(TokenKind::Identifier)?;

        let ty = if self.check(TokenKind::Colon) {
            self.consume(TokenKind::Colon)?;
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(TokenKind::Equal)?;
        let init = self.parse_expr()?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Let {
            name: self.src_from_span(name.span),
            init,
            ty,
        })
    }

    fn parse_if(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::If)?;

        self.consume(TokenKind::LParen)?;
        let cond = self.parse_expr()?;
        self.consume(TokenKind::RParen)?;

        let then_block = self.parse_stmt()?.into();

        let else_block = if self.check(TokenKind::Else) {
            self.consume(TokenKind::Else)?;
            Some(self.parse_stmt()?.into())
        } else {
            None
        };

        Ok(Statement::If {
            cond,
            then_block,
            else_block,
        })
    }

    fn parse_while(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::While)?;

        self.consume(TokenKind::LParen)?;
        let cond = self.parse_expr()?;
        self.consume(TokenKind::RParen)?;

        let body = self.parse_stmt()?.into();

        Ok(Statement::While {
            cond,
            body,
            label: None,
        })
    }

    fn parse_do_while(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Do)?;

        let body = self.parse_stmt()?.into();

        self.consume(TokenKind::While)?;

        self.consume(TokenKind::LParen)?;
        let cond = self.parse_expr()?;
        self.consume(TokenKind::RParen)?;

        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::DoWhile {
            cond,
            body,
            label: None,
        })
    }

    fn parse_for(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::For)?;

        self.consume(TokenKind::LParen)?;

        let init = if self.check(TokenKind::Semicolon) {
            self.consume(TokenKind::Semicolon)?;
            None
        } else {
            let init = self.parse_let()?;
            Some(init.into())
        };

        let cond = if self.check(TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        self.consume(TokenKind::Semicolon)?;

        let advancement = if self.check(TokenKind::RParen) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        self.consume(TokenKind::RParen)?;

        let body = self.parse_stmt()?.into();

        Ok(Statement::For {
            init,
            cond,
            advancement,
            body,
            label: None,
        })
    }

    fn parse_loop(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Loop)?;
        let body = self.parse_stmt()?.into();

        Ok(Statement::Loop { body, label: None })
    }

    fn parse_break(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Break)?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Break { label: None })
    }

    fn parse_continue(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Continue)?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Continue { label: None })
    }

    fn parse_goto(&mut self) -> Result<Statement, QError> {
        self.consume(TokenKind::Goto)?;

        let label = self.consume(TokenKind::Identifier)?;

        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Goto {
            label: self.src_from_span(label.span),
        })
    }

    fn parse_labeled(&mut self) -> Result<Statement, QError> {
        let name = self.consume(TokenKind::Identifier)?;

        self.consume(TokenKind::Colon)?;

        let stmt = self.parse_stmt()?;

        Ok(Statement::Labeled {
            name: self.src_from_span(name.span),
            stmt: stmt.into(),
        })
    }

    fn parse_expr_stmt(&mut self) -> Result<Statement, QError> {
        let expr = self.parse_expr()?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Expr { expr })
    }

    fn parse_expr(&mut self) -> Result<Expression, QError> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expression, QError> {
        let mut result = self.or()?;
        let begin = result.span();

        while self.check_many_greedy(&[TokenKind::Equal]) {
            let right = self.assignment()?;
            let end = right.span();

            result = Expression::Assign {
                lhs: result.clone().into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn bitwise_or(&mut self) -> Result<Expression, QError> {
        let mut result = self.bitwise_xor()?;
        let begin = result.span();

        while self.check(TokenKind::Pipe) {
            self.consume(TokenKind::Pipe)?;

            let right = self.bitwise_xor()?;
            let end = right.span();

            result = Expression::Binary {
                kind: BinaryExpressionKind::BitwiseOr,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn bitwise_xor(&mut self) -> Result<Expression, QError> {
        let mut result = self.bitwise_and()?;
        let begin = result.span();

        while self.check(TokenKind::Caret) {
            self.consume(TokenKind::Caret)?;

            let right = self.bitwise_and()?;
            let end = right.span();

            result = Expression::Binary {
                kind: BinaryExpressionKind::BitwiseXor,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn or(&mut self) -> Result<Expression, QError> {
        let mut result = self.and()?;
        let begin = result.span();

        while self.check(TokenKind::DoublePipe) {
            self.consume(TokenKind::DoublePipe)?;

            let right = self.and()?;
            let end = right.span();

            result = Expression::Binary {
                kind: BinaryExpressionKind::Or,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn and(&mut self) -> Result<Expression, QError> {
        let mut result = self.bitwise_or()?;
        let begin = result.span();

        while self.check(TokenKind::DoubleAmpersand) {
            self.consume(TokenKind::DoubleAmpersand)?;

            let right = self.bitwise_or()?;
            let end = right.span();

            result = Expression::Binary {
                kind: BinaryExpressionKind::And,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }
        Ok(result)
    }

    fn bitwise_and(&mut self) -> Result<Expression, QError> {
        let mut result = self.equality()?;
        let begin = result.span();

        while self.check(TokenKind::Ampersand) {
            self.consume(TokenKind::Ampersand)?;

            let right = self.equality()?;
            let end = right.span();

            result = Expression::Binary {
                kind: BinaryExpressionKind::BitwiseAnd,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn equality(&mut self) -> Result<Expression, QError> {
        let mut result = self.relational()?;
        let begin = result.span();

        while self.check_many_greedy(&[TokenKind::DoubleEqual, TokenKind::NotEqual]) {
            let neg = match self.prev.as_ref() {
                Some(token) => match token.kind {
                    TokenKind::NotEqual => true,
                    TokenKind::DoubleEqual => false,
                    _ => unreachable!(),
                },
                None => {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::Parse,
                        span: Span { start: 0, end: 0 },
                        msg: "Encountered the end of token stream (prev: None)".to_string(),
                    });
                }
            };

            let right = self.relational()?;
            let end = right.span();

            result = Expression::Binary {
                kind: if neg {
                    BinaryExpressionKind::NotEqual
                } else {
                    BinaryExpressionKind::Equal
                },
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn relational(&mut self) -> Result<Expression, QError> {
        let mut result = self.bitwise_shift()?;
        let begin = result.span();

        while self.check_many_greedy(&[
            TokenKind::Less,
            TokenKind::LessEqual,
            TokenKind::Greater,
            TokenKind::GreaterEqual,
        ]) {
            let kind = match self.prev.as_ref() {
                Some(token) => match token.kind {
                    TokenKind::Less => BinaryExpressionKind::Less,
                    TokenKind::Greater => BinaryExpressionKind::Greater,
                    TokenKind::LessEqual => BinaryExpressionKind::LessEqual,
                    TokenKind::GreaterEqual => BinaryExpressionKind::GreaterEqual,
                    _ => unreachable!(),
                },
                None => {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::Parse,
                        span: Span { start: 0, end: 0 },
                        msg: "Encountered the end of token stream (prev: None)".to_string(),
                    });
                }
            };

            let right = self.bitwise_shift()?;
            let end = right.span();

            result = Expression::Binary {
                kind,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn bitwise_shift(&mut self) -> Result<Expression, QError> {
        let mut result = self.term()?;
        let begin = result.span();

        while self.check_many_greedy(&[TokenKind::LessLess, TokenKind::GreaterGreater]) {
            let kind = match self.prev.as_ref() {
                Some(token) => match token.kind {
                    TokenKind::LessLess => BinaryExpressionKind::Shl,
                    TokenKind::GreaterGreater => BinaryExpressionKind::Shr,
                    _ => unreachable!(),
                },
                None => {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::Parse,
                        span: Span { start: 0, end: 0 },
                        msg: "Encountered the end of the token stream (prev: None)".to_string(),
                    });
                }
            };

            let right = self.term()?;
            let end = right.span();

            result = Expression::Binary {
                kind,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn term(&mut self) -> Result<Expression, QError> {
        let mut result = self.factor()?;
        let begin = result.span();

        while self.check_many_greedy(&[TokenKind::Plus, TokenKind::Minus]) {
            let kind = match self.prev.as_ref() {
                Some(token) => match token.kind {
                    TokenKind::Plus => BinaryExpressionKind::Add,
                    TokenKind::Minus => BinaryExpressionKind::Sub,
                    _ => unreachable!(),
                },
                None => {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::Parse,
                        span: Span { start: 0, end: 0 },
                        msg: "Encountered the end of the token stream (prev: None)".to_string(),
                    });
                }
            };

            let right = self.factor()?;
            let end = right.span();

            result = Expression::Binary {
                kind,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn factor(&mut self) -> Result<Expression, QError> {
        let mut result = self.cast()?;
        let begin = result.span();

        while self.check_many_greedy(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let kind = match self.prev.as_ref() {
                Some(token) => match token.kind {
                    TokenKind::Star => BinaryExpressionKind::Mul,
                    TokenKind::Slash => BinaryExpressionKind::Div,
                    TokenKind::Percent => BinaryExpressionKind::Rem,
                    _ => unreachable!(),
                },
                None => {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::Parse,
                        span: Span { start: 0, end: 0 },
                        msg: "Encountered the end of the token stream (prev: None)".to_string(),
                    });
                }
            };

            let right = self.cast()?;
            let end = right.span();

            result = Expression::Binary {
                kind,
                lhs: result.into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn cast(&mut self) -> Result<Expression, QError> {
        let mut expr = self.unary()?;
        let begin = expr.span();

        while self.check(TokenKind::As) {
            self.consume(TokenKind::As)?;

            let target = self.consume_primitive()?;
            let target_ty = Type::from(target);

            let end = target.span;

            expr = Expression::Cast {
                expr: expr.into(),
                target_ty,
                ty: None,
                span: begin + end,
            };
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expression, QError> {
        if self.check_many_greedy(&[
            TokenKind::Minus,
            TokenKind::Bang,
            TokenKind::Tilde,
            TokenKind::Ampersand,
            TokenKind::Star,
        ]) {
            let kind = match self.prev {
                Some(token) => match token.kind {
                    TokenKind::Minus => UnaryExpressionKind::Negate,
                    TokenKind::Bang => UnaryExpressionKind::LogicalNot,
                    TokenKind::Tilde => UnaryExpressionKind::BitwiseNot,
                    TokenKind::Ampersand => UnaryExpressionKind::AddrOf,
                    TokenKind::Star => UnaryExpressionKind::Deref,
                    _ => unreachable!(),
                },
                None => {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::Parse,
                        span: Span { start: 0, end: 0 },
                        msg: "Encountered the end of token stream (prev: None)".to_string(),
                    });
                }
            };

            let begin = self.prev.unwrap().span;

            let expr = self.unary()?;
            let end = expr.span();

            return Ok(Expression::Unary {
                kind,
                expr: expr.into(),
                ty: None,
                span: begin + end,
            });
        }

        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expression, QError> {
        let mut expr = self.primary()?;

        loop {
            if self.check(TokenKind::LParen) {
                let begin = expr.span();

                let Expression::Variable { name, .. } = expr else {
                    return Err(QError {
                        code: 1,
                        kind: QErrorKind::Parse,
                        msg: "Expected function name".into(),
                        span: begin,
                    });
                };

                expr = self.finish_call(&name, begin)?;
            } else if self.check(TokenKind::LBracket) {
                let begin = expr.span();

                self.consume(TokenKind::LBracket)?;

                let index = self.parse_expr()?;

                let end = self.consume(TokenKind::RBracket)?.span;

                expr = Expression::Index {
                    array: Box::new(expr),
                    index: Box::new(index),
                    ty: None,
                    span: begin + end,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn finish_call(&mut self, target: &str, begin_span: Span) -> Result<Expression, QError> {
        self.consume(TokenKind::LParen)?;

        let mut args = vec![];
        while !self.check(TokenKind::RParen) {
            args.push(self.parse_expr()?);

            if self.check(TokenKind::RParen) {
                break;
            }

            self.consume(TokenKind::Comma)?;
        }

        let rparen = self.consume(TokenKind::RParen)?;

        Ok(Expression::Call {
            target: target.to_owned(),
            args,
            ty: None,
            span: begin_span + rparen.span,
        })
    }

    fn primary(&mut self) -> Result<Expression, QError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Identifier => self.parse_var(),
                TokenKind::Number(ref number_token) => {
                    self.advance();

                    Ok(Expression::Literal {
                        val: self.parse_numeric_val(number_token)?,
                        ty: number_token.suffix.map(Type::from),
                        span: token.span,
                    })
                }
                TokenKind::True | TokenKind::False => self.parse_bool(&token),
                TokenKind::String => self.parse_string(),
                TokenKind::LParen => self.parse_grouping(),
                TokenKind::LBracket => self.parse_array_literal(),
                _ => unreachable!("got: {token:?}"),
            },
            None => Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                msg: "Encountered the end of the token stream.".to_string(),
                span: Span { start: 0, end: 0 },
            }),
        }
    }

    fn parse_var(&mut self) -> Result<Expression, QError> {
        let name = self.consume(TokenKind::Identifier)?;

        Ok(Expression::Variable {
            name: self.src_from_span(name.span),
            ty: None,
            span: name.span,
        })
    }

    fn parse_numeric_val(&mut self, token: &NumberToken) -> Result<LiteralVal, QError> {
        let raw = self.src_from_span(token.value_span);

        /* Remove separators.  */
        let cleaned = raw.replace('_', "");

        match token.kind {
            NumberKind::Integer { radix } => {
                /* Skip e.g. 0x  */
                let digits = if radix == 10 {
                    cleaned.as_str()
                } else {
                    &cleaned[2..]
                };

                let v = u128::from_str_radix(digits, radix).map_err(|e| QError {
                    code: 1,
                    kind: QErrorKind::Parse,
                    msg: e.to_string(),
                    span: token.value_span,
                })?;

                Ok(LiteralVal::Integer(v))
            }
            NumberKind::Float => Ok(LiteralVal::Float(cleaned)),
        }
    }

    fn parse_grouping(&mut self) -> Result<Expression, QError> {
        self.consume(TokenKind::LParen)?;
        let expr = self.parse_expr()?;
        self.consume(TokenKind::RParen)?;
        Ok(expr)
    }

    fn parse_string(&mut self) -> Result<Expression, QError> {
        let token = self.consume(TokenKind::String)?;

        let contents_span = Span {
            start: token.span.start + 1,
            end: token.span.end - 1,
        };

        let contents = self.src_from_span(contents_span);
        let contents = contents
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
            .replace("\\0", "\0")
            .replace("\\\"", "\"");

        Ok(Expression::Literal {
            val: LiteralVal::String(contents),
            ty: Some(Type::String),
            span: token.span,
        })
    }

    fn parse_bool(&mut self, token: &Token) -> Result<Expression, QError> {
        self.advance();

        Ok(Expression::Literal {
            val: LiteralVal::Bool(token.kind != TokenKind::False),
            ty: Some(Type::Boolean),
            span: token.span,
        })
    }

    fn parse_array_literal(&mut self) -> Result<Expression, QError> {
        let begin = self.consume(TokenKind::LBracket)?.span;

        let mut elements = Vec::new();

        if !self.check(TokenKind::RBracket) {
            loop {
                elements.push(self.parse_expr()?);

                if !self.check(TokenKind::Comma) {
                    break;
                }

                self.consume(TokenKind::Comma)?;

                if self.check(TokenKind::RBracket) {
                    break;
                }
            }
        }

        let end = self.consume(TokenKind::RBracket)?.span;

        Ok(Expression::ArrayLiteral {
            elements,
            ty: None,
            span: begin + end,
        })
    }

    fn parse_type(&mut self) -> Result<Type, QError> {
        if self.check(TokenKind::Star) {
            self.consume(TokenKind::Star)?;

            let pointee = self.parse_type()?;

            return Ok(self.types.intern_ptr(pointee));
        }

        if self.check(TokenKind::LBracket) {
            self.consume(TokenKind::LBracket)?;

            let element = self.parse_type()?;

            self.consume(TokenKind::Semicolon)?;

            let length = self.parse_array_length()?;

            self.consume(TokenKind::RBracket)?;

            Ok(self.types.intern_array(element, length))
        } else {
            Ok(self.consume_primitive()?.into())
        }
    }

    fn parse_array_length(&mut self) -> Result<usize, QError> {
        let token = self.curr.ok_or_else(|| QError {
            code: 1,
            kind: QErrorKind::Parse,
            msg: "Expected array length".into(),
            span: self.prev.map_or(Span { start: 0, end: 0 }, |t| t.span),
        })?;

        let TokenKind::Number(number) = token.kind else {
            return Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                msg: "Expected integer array length".into(),
                span: token.span,
            });
        };

        if !matches!(number.kind, NumberKind::Integer { .. }) || number.suffix.is_some() {
            return Err(QError {
                code: 1,
                kind: QErrorKind::Parse,
                msg: "Array length must be an unsuffixed integer".into(),
                span: token.span,
            });
        }

        let LiteralVal::Integer(value) = self.parse_numeric_val(&number)? else {
            unreachable!()
        };

        let length = usize::try_from(value).map_err(|_| QError {
            code: 1,
            kind: QErrorKind::Parse,
            msg: "Array length is too large".into(),
            span: token.span,
        })?;

        self.advance();

        Ok(length)
    }

    fn src_from_span(&self, span: Span) -> String {
        self.src[span.start..span.end].to_string()
    }
}
