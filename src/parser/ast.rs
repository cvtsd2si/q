use crate::lexer::token::{Span, Token, TokenKind};

#[derive(Debug)]
pub struct Program {
    pub block_items: Vec<BlockItem>,
}

#[derive(Debug)]
pub enum BlockItem {
    Decl(Declaration),
    Stmt(Statement),
}

#[derive(Debug)]
pub enum Declaration {
    Fn {
        name: String,
        ret: Type,
        params: Vec<Param>,
        body: Vec<Statement>,
    },
}

#[derive(Debug)]
pub enum Statement {
    Ret {
        value: Expression,
    },
    Let {
        name: String,
        init: Expression,
        ty: Type,
    },
}

#[derive(Debug, Clone)]
pub enum Expression {
    Literal {
        val: i64,
        span: Span,
    },
    Variable {
        name: String,
        ty: Option<Type>,
        span: Span,
    },
    Binary {
        kind: BinaryExpressionKind,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        ty: Option<Type>,
        span: Span,
    },
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal { span, .. } => *span,
            Expression::Variable { span, .. } => *span,
            Expression::Binary { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryExpressionKind {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy)]
pub enum Type {
    UInt8,
    Int8,
    UInt16,
    Int16,
    UInt32,
    Int32,
    UInt64,
    Int64,
    Float32,
    Float64,
    Boolean,
    String,
}

impl From<Token> for Type {
    fn from(value: Token) -> Self {
        match value.kind {
            TokenKind::U8 => Type::UInt8,
            TokenKind::I8 => Type::Int8,
            TokenKind::U16 => Type::UInt16,
            TokenKind::I16 => Type::Int16,
            TokenKind::U32 => Type::UInt32,
            TokenKind::I32 => Type::Int32,
            TokenKind::U64 => Type::UInt64,
            TokenKind::I64 => Type::Int64,
            TokenKind::F32 => Type::Float32,
            TokenKind::F64 => Type::Float64,
            TokenKind::Bool => Type::Boolean,
            TokenKind::Str => Type::String,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}
