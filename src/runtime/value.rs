use std::io::Write;

use crate::parser::ast::{Literal, Type};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Value {
    pub tag: ValueTag,
    pub payload: u64,
}

impl Value {
    pub fn cast(self, from: Type, to: Type) -> Self {
        enum Numeric {
            Signed(i64),
            Unsigned(u64),
            Float32(f32),
            Float64(f64),
        }

        /* Decode the src val.  */
        let value = match from {
            Type::Int8 => Numeric::Signed(self.payload as i8 as i64),
            Type::Int16 => Numeric::Signed(self.payload as i16 as i64),
            Type::Int32 => Numeric::Signed(self.payload as i32 as i64),
            Type::Int64 => Numeric::Signed(self.payload as i64),

            Type::UInt8 => Numeric::Unsigned(self.payload as u8 as u64),
            Type::UInt16 => Numeric::Unsigned(self.payload as u16 as u64),
            Type::UInt32 => Numeric::Unsigned(self.payload as u32 as u64),
            Type::UInt64 => Numeric::Unsigned(self.payload),

            Type::Float32 => Numeric::Float32(f32::from_bits(self.payload as u32)),
            Type::Float64 => Numeric::Float64(f64::from_bits(self.payload)),

            _ => unreachable!("Invalid source type: {from:?}"),
        };

        macro_rules! cast_as {
            ($ty:ty) => {
                match value {
                    Numeric::Signed(n) => n as $ty,
                    Numeric::Unsigned(n) => n as $ty,
                    Numeric::Float32(n) => n as $ty,
                    Numeric::Float64(n) => n as $ty,
                }
            };
        }

        /* Encode the dst val.  */
        let payload = match to {
            Type::Int8 => cast_as!(i8) as i64 as u64,
            Type::Int16 => cast_as!(i16) as i64 as u64,
            Type::Int32 => cast_as!(i32) as i64 as u64,
            Type::Int64 => cast_as!(i64) as u64,

            Type::UInt8 => cast_as!(u8) as u64,
            Type::UInt16 => cast_as!(u16) as u64,
            Type::UInt32 => cast_as!(u32) as u64,
            Type::UInt64 => cast_as!(u64),

            Type::Float32 => cast_as!(f32).to_bits() as u64,
            Type::Float64 => cast_as!(f64).to_bits(),

            _ => unreachable!("Invalid destination type: {to:?}"),
        };

        Self {
            tag: ValueTag::from(to),
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
impl_arithmetic!(Mul, mul, wrapping_mul, *);
impl_arithmetic!(Div, div, wrapping_div, /);
impl_arithmetic!(Rem, rem, wrapping_rem, %);

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
            ValueTag::Float32 => (self.payload as u32 ^ 0x8000_0000) as u64,
            ValueTag::Float64 => self.payload ^ 0x8000_0000_0000_0000,
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
        use ValueTag::*;

        match (self.tag, rhs.tag) {
            (Int8, Int8) => (self.payload as i8).partial_cmp(&(rhs.payload as i8)),
            (UInt8, UInt8) => (self.payload as u8).partial_cmp(&(rhs.payload as u8)),

            (Int16, Int16) => (self.payload as i16).partial_cmp(&(rhs.payload as i16)),
            (UInt16, UInt16) => (self.payload as u16).partial_cmp(&(rhs.payload as u16)),

            (Int32, Int32) => (self.payload as i32).partial_cmp(&(rhs.payload as i32)),
            (UInt32, UInt32) => (self.payload as u32).partial_cmp(&(rhs.payload as u32)),

            (Int64, Int64) => (self.payload as i64).partial_cmp(&(rhs.payload as i64)),
            (UInt64, UInt64) => self.payload.partial_cmp(&rhs.payload),

            (Float32, Float32) => {
                f32::from_bits(self.payload as u32).partial_cmp(&f32::from_bits(rhs.payload as u32))
            }
            (Float64, Float64) => {
                f64::from_bits(self.payload).partial_cmp(&f64::from_bits(rhs.payload))
            }

            (Bool, Bool) => (self.payload != 0).partial_cmp(&(rhs.payload != 0)),

            (Void, Void) => Some(std::cmp::Ordering::Equal),

            _ => None,
        }
    }
}

impl std::cmp::PartialEq for Value {
    fn eq(&self, rhs: &Self) -> bool {
        self.partial_cmp(rhs) == Some(std::cmp::Ordering::Equal)
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
