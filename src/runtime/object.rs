use crate::{
    parser::ast::Literal,
    runtime::value::{Value, ValueTag},
};

#[derive(Debug, Clone)]
pub enum Object {
    Number(i64),
    Boolean(bool),
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
