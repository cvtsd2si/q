use crate::lexer::token::{Span, Suffix, Token, TokenKind};

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
        ret: Option<Type>,
        params: Vec<Param>,
        body: Statement,
    },
}

#[derive(Debug)]
pub enum Statement {
    Ret {
        value: Option<Expression>,
        ret_ty: Option<Type>,
    },
    Let {
        name: String,
        init: Expression,
        ty: Option<Type>,
    },
    Print {
        value: Expression,
    },
    If {
        cond: Expression,
        then_block: Box<Statement>,
        else_block: Option<Box<Statement>>,
    },
    Block {
        body: Vec<Statement>,
    },
    While {
        cond: Expression,
        body: Box<Statement>,
        label: Option<String>,
    },
    Break {
        label: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum Expression {
    Literal {
        val: LiteralVal,
        ty: Option<Type>,
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
    Call {
        target: String,
        args: Vec<Expression>,
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
            Expression::Call { span, .. } => *span,
        }
    }

    pub fn ty(&self) -> Option<Type> {
        match self {
            Expression::Literal { val, ty, span } => *ty,
            Expression::Variable { name, ty, span } => *ty,
            Expression::Binary {
                kind,
                lhs,
                rhs,
                ty,
                span,
            } => *ty,
            Expression::Call {
                target,
                args,
                ty,
                span,
            } => *ty,
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

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, Copy)]
pub enum Literal {
    Bool(bool),
    UInt8(u8),
    Int8(i8),
    UInt16(u16),
    Int16(i16),
    UInt32(u32),
    Int32(i32),
    UInt64(u64),
    Int64(i64),
    Float32(f32),
    Float64(f64),
}

#[derive(Debug, Clone)]
pub enum LiteralVal {
    Integer(u128),
    Float(String),
    Bool(bool),
}

impl TryFrom<(&LiteralVal, Type)> for Literal {
    type Error = String;

    fn try_from((val, ty): (&LiteralVal, Type)) -> Result<Self, Self::Error> {
        let overflow = || format!("Literal {val:?} is out of range for {ty:?}");

        match (val, ty) {
            (LiteralVal::Integer(n), Type::UInt8) => {
                Ok(Self::UInt8(u8::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Integer(n), Type::UInt16) => {
                Ok(Self::UInt16(u16::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Integer(n), Type::UInt32) => {
                Ok(Self::UInt32(u32::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Integer(n), Type::UInt64) => {
                Ok(Self::UInt64(u64::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Integer(n), Type::Int8) => {
                Ok(Self::Int8(i8::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Integer(n), Type::Int16) => {
                Ok(Self::Int16(i16::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Integer(n), Type::Int32) => {
                Ok(Self::Int32(i32::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Integer(n), Type::Int64) => {
                Ok(Self::Int64(i64::try_from(*n).map_err(|_| overflow())?))
            }

            (LiteralVal::Float(s), Type::Float32) => {
                let n = s.parse::<f32>().map_err(|e| e.to_string())?;

                if !n.is_finite() {
                    return Err(overflow());
                }

                Ok(Self::Float32(n))
            }

            (LiteralVal::Float(s), Type::Float64) => {
                let n = s.parse::<f64>().map_err(|e| e.to_string())?;

                if !n.is_finite() {
                    return Err(overflow());
                }

                Ok(Self::Float64(n))
            }

            (LiteralVal::Bool(b), Type::Boolean) => Ok(Self::Bool(*b)),

            _ => Err(format!("Cannot convert {val:?} into {ty:?}")),
        }
    }
}

impl From<Suffix> for Type {
    fn from(suffix: Suffix) -> Self {
        match suffix {
            Suffix::U8 => Self::UInt8,
            Suffix::I8 => Self::Int8,
            Suffix::U16 => Self::UInt16,
            Suffix::I16 => Self::Int16,
            Suffix::U32 => Self::UInt32,
            Suffix::I32 => Self::Int32,
            Suffix::U64 => Self::UInt64,
            Suffix::I64 => Self::Int64,
            Suffix::F32 => Self::Float32,
            Suffix::F64 => Self::Float64,
        }
    }
}

impl Type {
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Type::UInt8
                | Type::Int8
                | Type::UInt16
                | Type::Int16
                | Type::UInt32
                | Type::Int32
                | Type::UInt64
                | Type::Int64
                | Type::Float32
                | Type::Float64
        )
    }
}
