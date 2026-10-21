use std::io::Write;

use crate::parser::ast::{Literal, Type};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Value {
    pub tag: ValueTag,
    pub payload: u64,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum ValueTag {
    UInt8 = 0,
    Int8 = 1,
    UInt16 = 2,
    Int16 = 3,
    UInt32 = 4,
    Int32 = 5,
    UInt64 = 6,
    Int64 = 7,
    Float32 = 8,
    Float64 = 9,
    Bool = 10,
    Void = 11,
}

impl From<Literal> for Value {
    fn from(value: Literal) -> Self {
        let (tag, payload) = match value {
            Literal::UInt8(n) => (ValueTag::UInt8, n as u64),
            Literal::Int8(n) => (ValueTag::Int8, n as u64),
            Literal::UInt16(n) => (ValueTag::UInt16, n as u64),
            Literal::Int16(n) => (ValueTag::Int16, n as u64),
            Literal::UInt32(n) => (ValueTag::UInt32, n as u64),
            Literal::Int32(n) => (ValueTag::Int32, n as u64),
            Literal::UInt64(n) => (ValueTag::UInt64, n),
            Literal::Int64(n) => (ValueTag::Int64, n as u64),
            Literal::Float32(n) => (ValueTag::Float32, n.to_bits() as u64),
            Literal::Float64(n) => (ValueTag::Float64, n.to_bits()),
            Literal::Bool(b) => (ValueTag::Bool, b as u64),
        };

        Self { tag, payload }
    }
}

impl From<Type> for ValueTag {
    fn from(value: Type) -> Self {
        match value {
            Type::UInt8 => ValueTag::UInt8,
            Type::Int8 => ValueTag::Int8,
            Type::UInt16 => ValueTag::UInt16,
            Type::Int16 => ValueTag::Int16,
            Type::UInt32 => ValueTag::UInt32,
            Type::Int32 => ValueTag::Int32,
            Type::UInt64 => ValueTag::UInt64,
            Type::Int64 => ValueTag::Int64,
            Type::Float32 => ValueTag::Float32,
            Type::Float64 => ValueTag::Float64,
            Type::Boolean => ValueTag::Bool,
            _ => unimplemented!(),
        }
    }
}

impl std::ops::Add for Value {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self.tag, rhs.tag) {
            (ValueTag::Int64, ValueTag::Int64) => Self {
                tag: ValueTag::Int64,
                payload: (self.payload as i64).wrapping_add(rhs.payload as i64) as u64,
            },

            _ => unimplemented!(),
        }
    }
}

impl std::ops::Sub for Value {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self.tag, rhs.tag) {
            (ValueTag::Int64, ValueTag::Int64) => Self {
                tag: ValueTag::Int64,
                payload: (self.payload as i64).wrapping_sub(rhs.payload as i64) as u64,
            },

            _ => unimplemented!(),
        }
    }
}

impl TryFrom<u64> for ValueTag {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::UInt8,
            1 => Self::Int8,
            2 => Self::UInt16,
            3 => Self::Int16,
            4 => Self::UInt32,
            5 => Self::Int32,
            6 => Self::UInt64,
            7 => Self::Int64,
            8 => Self::Float32,
            9 => Self::Float64,
            10 => Self::Bool,
            11 => Self::Void,
            _ => return Err(()),
        })
    }
}

pub fn print_value(value: Value) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    writeln!(stdout, "{value:?}")
}
