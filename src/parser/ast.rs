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
    DoWhile {
        cond: Expression,
        body: Box<Statement>,
        label: Option<String>,
    },
    For {
        init: Option<Box<Statement>>,
        cond: Option<Expression>,
        advancement: Option<Expression>,
        body: Box<Statement>,
        label: Option<String>,
    },
    Loop {
        body: Box<Statement>,
        label: Option<String>,
    },
    Break {
        label: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
    Goto {
        label: String,
    },
    Labeled {
        name: String,
        stmt: Box<Statement>,
    },
    Expr {
        expr: Expression,
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
    Assign {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        ty: Option<Type>,
        span: Span,
    },
    Unary {
        kind: UnaryExpressionKind,
        expr: Box<Expression>,
        ty: Option<Type>,
        span: Span,
    },
    Cast {
        expr: Box<Expression>,
        target_ty: Type,
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
            Expression::Assign { span, .. } => *span,
            Expression::Unary { span, .. } => *span,
            Expression::Cast { span, .. } => *span,
        }
    }

    pub fn ty(&self) -> Option<Type> {
        match self {
            Expression::Literal { ty, .. } => *ty,
            Expression::Variable { ty, .. } => *ty,
            Expression::Binary { ty, .. } => *ty,
            Expression::Call { ty, .. } => *ty,
            Expression::Assign { ty, .. } => *ty,
            Expression::Unary { ty, .. } => *ty,
            Expression::Cast { ty, .. } => *ty,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryExpressionKind {
    Negate,
    LogicalNot,
    BitwiseNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryExpressionKind {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    And,
    Or,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    NotEqual,
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
    NegativeInteger(u128),
    Float(String),
    Bool(bool),
}

impl TryFrom<(&LiteralVal, Type)> for Literal {
    type Error = String;

    fn try_from((val, ty): (&LiteralVal, Type)) -> Result<Self, Self::Error> {
        let overflow = || format!("Literal {val:?} is out of range for {ty:?}");
        let negative = |n: u128| -> Result<i128, String> {
            let magnitude = i128::try_from(n).map_err(|_| overflow())?;
            Ok(-magnitude)
        };

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

            (LiteralVal::NegativeInteger(n), Type::Int8) => Ok(Self::Int8(
                i8::try_from(negative(*n)?).map_err(|_| overflow())?,
            )),

            (LiteralVal::NegativeInteger(n), Type::Int16) => Ok(Self::Int16(
                i16::try_from(negative(*n)?).map_err(|_| overflow())?,
            )),

            (LiteralVal::NegativeInteger(n), Type::Int32) => Ok(Self::Int32(
                i32::try_from(negative(*n)?).map_err(|_| overflow())?,
            )),

            (LiteralVal::NegativeInteger(n), Type::Int64) => Ok(Self::Int64(
                i64::try_from(negative(*n)?).map_err(|_| overflow())?,
            )),

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
        self.is_int() || self.is_float()
    }

    pub fn is_int(self) -> bool {
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
        )
    }

    pub fn is_float(self) -> bool {
        matches!(self, Type::Float32 | Type::Float64)
    }

    pub fn is_signed(self) -> bool {
        matches!(self, Type::Int8 | Type::Int16 | Type::Int32 | Type::Int64)
    }

    pub fn alignment(self) -> usize {
        match self {
            Type::UInt8 | Type::Int8 => 1,
            Type::UInt16 | Type::Int16 => 2,
            Type::Int32 | Type::UInt32 | Type::Float32 => 4,
            Type::UInt64 | Type::Int64 | Type::Float64 => 8,
            _ => unreachable!(),
        }
    }

    pub fn size_of(self) -> usize {
        match self {
            Type::Boolean => std::mem::size_of::<bool>(),
            Type::UInt8 => std::mem::size_of::<u8>(),
            Type::Int8 => std::mem::size_of::<i8>(),
            Type::UInt16 => std::mem::size_of::<u16>(),
            Type::Int16 => std::mem::size_of::<i16>(),
            Type::UInt32 => std::mem::size_of::<u32>(),
            Type::Int32 => std::mem::size_of::<i32>(),
            Type::UInt64 => std::mem::size_of::<u64>(),
            Type::Int64 => std::mem::size_of::<i64>(),
            Type::Float32 => std::mem::size_of::<f32>(),
            Type::Float64 => std::mem::size_of::<f64>(),
            _ => unimplemented!(),
        }
    }
}

impl From<Literal> for Type {
    fn from(value: Literal) -> Type {
        match value {
            Literal::Bool(_) => Type::Boolean,
            Literal::Int8(_) => Type::Int8,
            Literal::UInt8(_) => Type::UInt8,
            Literal::Int16(_) => Type::Int16,
            Literal::UInt16(_) => Type::UInt16,
            Literal::Int32(_) => Type::Int32,
            Literal::UInt32(_) => Type::UInt32,
            Literal::Int64(_) => Type::Int64,
            Literal::UInt64(_) => Type::UInt64,
            Literal::Float32(_) => Type::Float32,
            Literal::Float64(_) => Type::Float64,
        }
    }
}
