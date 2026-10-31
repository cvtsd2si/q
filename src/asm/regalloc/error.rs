#[derive(Debug)]
pub struct RegAllocError {
    pub msg: String,
}

impl std::error::Error for RegAllocError {}

impl std::fmt::Display for RegAllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "regalloc: {}", self.msg)
    }
}
