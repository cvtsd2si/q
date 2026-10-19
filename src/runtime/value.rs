#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Value {
    pub tag: ValueTag,
    pub payload: i64,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum ValueTag {
    Number = 0,
    Boolean = 1,
}
