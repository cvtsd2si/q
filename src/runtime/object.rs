use crate::runtime::value::{Value, ValueTag};

#[derive(Debug, Clone)]
pub enum Object {
    Number(i32),
}

impl From<Object> for Value {
    fn from(value: Object) -> Self {
        match value {
            Object::Number(n) => Value {
                tag: ValueTag::Number,
                payload: (n as i64) as u64,
            },
        }
    }
}
