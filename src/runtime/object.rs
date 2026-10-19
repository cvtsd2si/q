use crate::{
    parser::ast::Literal,
    runtime::value::{Value, ValueTag},
};

#[derive(Debug, Clone)]
pub enum Object {
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
    Bool(bool),
}

impl From<Literal> for Object {
    fn from(value: Literal) -> Self {
        match value {
            Literal::UInt8(uint8) => Object::UInt8(uint8),
            Literal::Int8(int8) => Object::Int8(int8),
            Literal::UInt16(uint16) => Object::UInt16(uint16),
            Literal::Int16(int16) => Object::Int16(int16),
            Literal::UInt32(uint32) => Object::UInt32(uint32),
            Literal::Int32(int32) => Object::Int32(int32),
            Literal::UInt64(uint64) => Object::UInt64(uint64),
            Literal::Int64(int64) => Object::Int64(int64),
            Literal::Bool(boolean) => Object::Bool(boolean),
            Literal::Float32(float32) => Object::Float32(float32),
            Literal::Float64(float64) => Object::Float64(float64),
        }
    }
}

impl From<Object> for Value {
    fn from(value: Object) -> Self {
        match value {
            Object::UInt8(uint8) => Value {
                tag: ValueTag::UInt8,
                payload: uint8 as u64,
            },
            Object::Int8(int8) => Value {
                tag: ValueTag::Int8,
                payload: int8 as u64,
            },
            Object::UInt16(uint16) => Value {
                tag: ValueTag::UInt16,
                payload: uint16 as u64,
            },
            Object::Int16(int16) => Value {
                tag: ValueTag::UInt16,
                payload: int16 as u64,
            },
            Object::UInt32(uint32) => Value {
                tag: ValueTag::UInt32,
                payload: uint32 as u64,
            },
            Object::Int32(int32) => Value {
                tag: ValueTag::Int32,
                payload: int32 as u64,
            },
            Object::UInt64(uint64) => Value {
                tag: ValueTag::UInt64,
                payload: uint64 as u64,
            },
            Object::Int64(int64) => Value {
                tag: ValueTag::Int64,
                payload: int64 as u64,
            },

            Object::Float32(float32) => Value {
                tag: ValueTag::Float32,
                payload: float32.to_bits() as u64,
            },

            Object::Float64(float64) => Value {
                tag: ValueTag::Float64,
                payload: float64.to_bits(),
            },

            Object::Bool(b) => Value {
                tag: ValueTag::Bool,
                payload: if b { 1 } else { 0 },
            },
        }
    }
}

impl From<Value> for Object {
    fn from(value: Value) -> Self {
        let n = value.payload;

        match value.tag {
            ValueTag::UInt8 => Self::UInt8(n as u8),
            ValueTag::Int8 => Self::Int8(n as i8),
            ValueTag::UInt16 => Self::UInt16(n as u16),
            ValueTag::Int16 => Self::Int16(n as i16),
            ValueTag::UInt32 => Self::UInt32(n as u32),
            ValueTag::Int32 => Self::Int32(n as i32),
            ValueTag::UInt64 => Self::UInt64(n),
            ValueTag::Int64 => Self::Int64(n as i64),
            ValueTag::Float32 => Self::Float32(f32::from_bits(n as u32)),
            ValueTag::Float64 => Self::Float64(f64::from_bits(n)),
            ValueTag::Bool => Self::Bool(n != 0),
        }
    }
}
