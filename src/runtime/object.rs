use crate::{
    parser::ast::Literal,
    runtime::value::{Value, ValueTag},
};

#[derive(Debug, Clone)]
pub enum Object {
    Number(i64),
    Boolean(bool),
}

impl From<Literal> for Object {
    fn from(value: Literal) -> Self {
        match value {
            Literal::UInt8(uint8) => Object::Number(uint8 as i64),
            Literal::Int8(int8) => Object::Number(int8 as i64),
            Literal::UInt16(uint16) => Object::Number(uint16 as i64),
            Literal::Int16(int16) => Object::Number(int16 as i64),
            Literal::UInt32(uint32) => Object::Number(uint32 as i64),
            Literal::Int32(int32) => Object::Number(int32 as i64),
            Literal::UInt64(uint64) => Object::Number(uint64 as i64),
            Literal::Int64(int64) => Object::Number(int64 as i64),
            Literal::Bool(boolean) => Object::Boolean(boolean),
            _ => unreachable!(),
        }
    }
}

impl From<Object> for Value {
    fn from(value: Object) -> Self {
        match value {
            Object::Number(n) => Value {
                tag: ValueTag::Number,
                payload: n,
            },
            Object::Boolean(b) => Value {
                tag: ValueTag::Boolean,
                payload: if b { 1 } else { 0 },
            },
        }
    }
}
