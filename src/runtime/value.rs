use std::io::Write;

use crate::parser::ast::{Literal, Type};

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Value {
    pub tag: ValueTag,
    pub payload: u64,
}

impl Value {
    pub fn cast_int(self, to_ty: Type) -> Self {
        /* Interpret the original payload according to its source type.  */
        let value = match self.tag {
            ValueTag::Int8 => (self.payload as i8 as i64) as u64,
            ValueTag::UInt8 => self.payload as u8 as u64,
            ValueTag::Int16 => (self.payload as i16 as i64) as u64,
            ValueTag::UInt16 => self.payload as u16 as u64,
            ValueTag::Int32 => (self.payload as i32 as i64) as u64,
            ValueTag::UInt32 => self.payload as u32 as u64,
            ValueTag::Int64 | ValueTag::UInt64 => self.payload,
            _ => unreachable!("Cannot cast {:?} as integer", self.tag),
        };

        /* Convert to the destination integer type.  */
        let payload = match to_ty {
            Type::Int8 => (value as i8 as i64) as u64,
            Type::UInt8 => value as u8 as u64,
            Type::Int16 => (value as i16 as i64) as u64,
            Type::UInt16 => value as u16 as u64,
            Type::Int32 => (value as i32 as i64) as u64,
            Type::UInt32 => value as u32 as u64,
            Type::Int64 | Type::UInt64 => value,
            _ => unreachable!("Expected integer destination type: {to_ty:?}"),
        };

        Self {
            tag: ValueTag::from(to_ty),
            payload,
        }
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
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
macro_rules! impl_arithmetic {
    ($trait:ident, $method:ident, $wrapping:ident, $op:tt) => {
        impl std::ops::$trait for Value {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                use ValueTag::*;

                let payload = match (self.tag, rhs.tag) {
                    (Int8, Int8) => {
                        (self.payload as i8)
                            .$wrapping(rhs.payload as i8) as u8 as u64
                    },

                    (Int16, Int16) => {
                        (self.payload as i16)
                            .$wrapping(rhs.payload as i16) as u16 as u64
                    },

                    (Int32, Int32) => {
                        (self.payload as i32)
                            .$wrapping(rhs.payload as i32) as u32 as u64
                    },

                    (Int64, Int64) => {
                        (self.payload as i64)
                            .$wrapping(rhs.payload as i64) as u64
                    },

                    (UInt8, UInt8) => {
                        (self.payload as u8)
                            .$wrapping(rhs.payload as u8) as u64
                    },

                    (UInt16, UInt16) => {
                        (self.payload as u16)
                            .$wrapping(rhs.payload as u16) as u64
                    },

                    (UInt32, UInt32) => {
                        (self.payload as u32)
                            .$wrapping(rhs.payload as u32) as u64
                    },

                    (UInt64, UInt64) => {
                        self.payload.$wrapping(rhs.payload)
                    },

                    (Float32, Float32) => {
                        (
                            f32::from_bits(self.payload as u32)
                            $op
                            f32::from_bits(rhs.payload as u32)
                        ).to_bits() as u64
                    },

                    (Float64, Float64) => {
                        (
                            f64::from_bits(self.payload)
                            $op
                            f64::from_bits(rhs.payload)
                        ).to_bits()
                    },

                    _ => panic!(
                        "Unsupported arithmetic operation: {:?} and {:?}",
                        self.tag,
                        rhs.tag
                    ),
                };

                Self {
                    tag: self.tag,
                    payload,
                }
            }
        }
    };
}

impl_arithmetic!(Add, add, wrapping_add, +);
impl_arithmetic!(Sub, sub, wrapping_sub, -);

impl std::ops::Not for Value {
    type Output = Self;

    fn not(self) -> Self::Output {
        let payload = match self.tag {
            ValueTag::Bool => (self.payload == 0) as u64,
            ValueTag::UInt8 => (!(self.payload as u8)) as u64,
            ValueTag::Int8 => (!(self.payload as i8)) as u64,
            ValueTag::UInt16 => (!(self.payload as u16)) as u64,
            ValueTag::Int16 => (!(self.payload as i16)) as u64,
            ValueTag::UInt32 => (!(self.payload as u32)) as u64,
            ValueTag::Int32 => (!(self.payload as i32)) as u64,
            ValueTag::UInt64 => !self.payload,
            ValueTag::Int64 => !self.payload,
            _ => unreachable!("Cannot apply NOT to {:?}", self.tag),
        };

        Self {
            tag: self.tag,
            payload,
        }
    }
}

impl std::ops::Neg for Value {
    type Output = Self;

    fn neg(self) -> Self::Output {
        let payload = match self.tag {
            ValueTag::Int8 => (self.payload as i8).wrapping_neg() as u64,
            ValueTag::Int16 => (self.payload as i16).wrapping_neg() as u64,
            ValueTag::Int32 => (self.payload as i32).wrapping_neg() as u64,
            ValueTag::Int64 => (self.payload as i64).wrapping_neg() as u64,
            _ => unimplemented!("Cannot negate {:?} yet", self.tag),
        };

        Self {
            tag: self.tag,
            payload,
        }
    }
}

impl std::cmp::PartialOrd for Value {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        match (self.tag, rhs.tag) {
            (ValueTag::Int64, ValueTag::Int64) => {
                (self.payload as i64).partial_cmp(&(rhs.payload as i64))
            }
            _ => unimplemented!(),
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        match value {
            true => Value {
                tag: ValueTag::Bool,
                payload: 1,
            },
            false => Value {
                tag: ValueTag::Bool,
                payload: 0,
            },
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

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.tag {
            ValueTag::UInt8 => write!(f, "{}", self.payload as u8),
            ValueTag::Int8 => write!(f, "{}", self.payload as i8),

            ValueTag::UInt16 => write!(f, "{}", self.payload as u16),
            ValueTag::Int16 => write!(f, "{}", self.payload as i16),

            ValueTag::UInt32 => write!(f, "{}", self.payload as u32),
            ValueTag::Int32 => write!(f, "{}", self.payload as i32),

            ValueTag::UInt64 => write!(f, "{}", self.payload),
            ValueTag::Int64 => write!(f, "{}", self.payload as i64),

            ValueTag::Float32 => {
                write!(f, "{}", f32::from_bits(self.payload as u32))
            }

            ValueTag::Float64 => {
                write!(f, "{}", f64::from_bits(self.payload))
            }

            ValueTag::Bool => write!(f, "{}", self.payload != 0),
            ValueTag::Void => write!(f, "()"),
        }
    }
}

pub fn print_value(value: Value) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    writeln!(stdout, "{value}")
}
