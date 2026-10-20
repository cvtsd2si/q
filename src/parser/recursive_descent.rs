use crate::lexer::token::{Span, Token, TokenKind};
use crate::parser::ast::{
    BinaryExpressionKind, BlockItem, Declaration, Expression, Literal, Param, Program, Statement,
    Type,
};
use crate::parser::error::ParserError;
use std::collections::VecDeque;

pub struct Parser<'src> {
    src: &'src str,
    tokens: VecDeque<Token>,
    curr: Option<Token>,
    prev: Option<Token>,
}

impl<'src> Parser<'src> {
    pub fn new(src: &'src str, tokens: VecDeque<Token>) -> Self {
        Self {
            src,
            tokens,
            curr: None,
            prev: None,
        }
    }

    pub fn parse(&mut self) -> Result<Program, ParserError> {
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

    fn consume(&mut self, kind: TokenKind) -> Result<Token, ParserError> {
        match self.curr {
            Some(token) if token.kind == kind => {
                self.advance();

                Ok(token)
            }
            Some(token) => Err(ParserError {
                src: self.src.to_string(),
                span: token.span,
            }),
            None => Err(ParserError {
                src: self.src.to_string(),
                span: Span {
                    start: self.src.len(),
                    end: self.src.len(),
                },
            }),
        }
    }

    fn consume_primitive(&mut self) -> Result<Token, ParserError> {
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
                _ => Err(ParserError {
                    src: self.src.to_string(),
                    span: token.span,
                }),
            },
            None => Err(ParserError {
                src: self.src.to_string(),
                span: Span {
                    start: self.src.len(),
                    end: self.src.len(),
                },
            }),
        }
    }

    fn parse_block_item(&mut self) -> Result<BlockItem, ParserError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Fn => Ok(BlockItem::Decl(self.parse_decl()?)),
                TokenKind::Ret | TokenKind::Let | TokenKind::Print | TokenKind::If => {
                    Ok(BlockItem::Stmt(self.parse_stmt()?))
                }
                _ => unreachable!("got: {:?}", token.kind),
            },
            None => unreachable!(),
        }
    }

    fn parse_decl(&mut self) -> Result<Declaration, ParserError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Fn => Ok(self.parse_fn()?),
                _ => unimplemented!(),
            },
            None => unreachable!(),
        }
    }

    fn parse_stmt(&mut self) -> Result<Statement, ParserError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Let => Ok(self.parse_let()?),
                TokenKind::Ret => Ok(self.parse_ret()?),
                TokenKind::Print => Ok(self.parse_print()?),
                TokenKind::If => Ok(self.parse_if()?),
                TokenKind::LBrace => Ok(Statement::Block {
                    body: self.parse_block()?,
                }),
                _ => unimplemented!("got: {:?}", self.curr),
            },
            None => unreachable!(),
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

    fn parse_fn(&mut self) -> Result<Declaration, ParserError> {
        self.consume(TokenKind::Fn)?;

        let name = self.consume(TokenKind::Identifier)?;

        self.consume(TokenKind::LParen)?;

        let mut params = vec![];
        while !self.check(TokenKind::RParen) {
            let param_name = self.consume(TokenKind::Identifier)?;
            self.consume(TokenKind::Colon)?;
            let param_ty = self.consume_primitive()?.into();

            params.push(Param {
                name: self.src[param_name.span.start..param_name.span.end].to_string(),
                ty: param_ty,
            });

            if self.check(TokenKind::RParen) {
                break;
            }

            self.consume(TokenKind::Comma)?;
        }

        self.consume(TokenKind::RParen)?;
        self.consume(TokenKind::Arrow)?;

        let ret = self.consume_primitive()?.into();
        let body = self.parse_block()?;

        Ok(Declaration::Fn {
            name: self.src[name.span.start..name.span.end].to_string(),
            ret,
            params,
            body: Statement::Block { body },
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, ParserError> {
        self.consume(TokenKind::LBrace)?;

        let mut body = vec![];

        while !self.check(TokenKind::RBrace) {
            body.push(self.parse_stmt()?);
        }

        self.consume(TokenKind::RBrace)?;

        Ok(body)
    }

    fn parse_ret(&mut self) -> Result<Statement, ParserError> {
        self.consume(TokenKind::Ret)?;
        let value = self.parse_expr()?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Ret { value })
    }

    fn parse_print(&mut self) -> Result<Statement, ParserError> {
        self.consume(TokenKind::Print)?;
        let value = self.parse_expr()?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Print { value })
    }

    fn parse_let(&mut self) -> Result<Statement, ParserError> {
        self.consume(TokenKind::Let)?;
        let name = self.consume(TokenKind::Identifier)?;
        self.consume(TokenKind::Colon)?;
        let ty = Some(self.consume_primitive()?.into());
        self.consume(TokenKind::Equal)?;
        let init = self.parse_expr()?;
        self.consume(TokenKind::Semicolon)?;

        Ok(Statement::Let {
            name: self.src[name.span.start..name.span.end].to_string(),
            init,
            ty,
        })
    }

    fn parse_if(&mut self) -> Result<Statement, ParserError> {
        self.consume(TokenKind::If)?;

        self.consume(TokenKind::LParen)?;
        let cond = self.parse_expr()?;
        self.consume(TokenKind::RParen)?;

        let then_block = self.parse_stmt()?.into();

        let else_block = if self.check(TokenKind::Else) {
            self.consume(TokenKind::Else)?;
            Some(
                Statement::Block {
                    body: self.parse_block()?,
                }
                .into(),
            )
        } else {
            None
        };

        Ok(Statement::If {
            cond,
            then_block,
            else_block,
        })
    }

    fn parse_expr(&mut self) -> Result<Expression, ParserError> {
        self.term()
    }

    fn term(&mut self) -> Result<Expression, ParserError> {
        let mut result = self.factor()?;
        let begin = result.span();

        while self.check_many_greedy(&[TokenKind::Plus, TokenKind::Minus]) {
            let kind = match self.prev.as_ref() {
                Some(token) => match token.kind {
                    TokenKind::Plus => BinaryExpressionKind::Add,
                    TokenKind::Minus => BinaryExpressionKind::Sub,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            };

            let right = self.factor()?;
            let end = right.span();

            result = Expression::Binary {
                kind,
                lhs: result.clone().into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn factor(&mut self) -> Result<Expression, ParserError> {
        let mut result = self.call()?;
        let begin = result.span();

        while self.check_many_greedy(&[TokenKind::Star, TokenKind::Slash]) {
            let kind = match self.prev.as_ref() {
                Some(token) => match token.kind {
                    TokenKind::Star => BinaryExpressionKind::Mul,
                    TokenKind::Slash => BinaryExpressionKind::Div,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            };

            let right = self.call()?;
            let end = right.span();

            result = Expression::Binary {
                kind,
                lhs: result.clone().into(),
                rhs: right.into(),
                ty: None,
                span: begin + end,
            };
        }

        Ok(result)
    }

    fn call(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.primary()?;

        loop {
            if self.check(TokenKind::LParen) {
                let target = match expr {
                    Expression::Variable { name, ty, span } => name,
                    _ => unreachable!(),
                };

                expr = self.finish_call(&target)?;
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn finish_call(&mut self, target: &str) -> Result<Expression, ParserError> {
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
            span: rparen.span,
        })
    }

    fn primary(&mut self) -> Result<Expression, ParserError> {
        match self.curr {
            Some(token) => match token.kind {
                TokenKind::Identifier => self.parse_var(),
                TokenKind::Number => self.parse_literal(),
                TokenKind::True | TokenKind::False => self.parse_bool(&token),
                _ => unreachable!("got: {:?}", token),
            },
            None => unreachable!(),
        }
    }

    fn parse_var(&mut self) -> Result<Expression, ParserError> {
        let token = self.consume(TokenKind::Identifier)?;

        Ok(Expression::Variable {
            name: self.src[token.span.start..token.span.end].to_string(),
            ty: None,
            span: token.span,
        })
    }

    fn parse_literal(&mut self) -> Result<Expression, ParserError> {
        let token = self.consume(TokenKind::Number)?;

        Ok(Expression::Literal {
            val: Literal::Int64(
                self.src[token.span.start..token.span.end]
                    .parse::<i64>()
                    .unwrap(),
            ),
            ty: Some(Type::Int64),
            span: token.span,
        })
    }

    fn parse_bool(&mut self, token: &Token) -> Result<Expression, ParserError> {
        self.advance();

        Ok(Expression::Literal {
            val: Literal::Bool(match token.kind {
                TokenKind::True => true,
                TokenKind::False => false,
                _ => unreachable!(),
            }),
            ty: Some(Type::Boolean),
            span: token.span,
        })
    }
}
