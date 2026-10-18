use crate::runtime::value::{Value, ValueTag};

#[derive(Debug, Clone)]
pub enum Object {
    Number(i64),
}

impl From<Object> for Value {
    fn from(value: Object) -> Self {
        match value {
            Object::Number(n) => Value {
                tag: ValueTag::Number,
                payload: n,
            },
        }
    }
}
