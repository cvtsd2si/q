#[derive(Debug)]
pub struct TokenizerError {
    pub src: String,
    pub start: usize,
    pub end: usize,
}

impl std::fmt::Display for TokenizerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line_start = self.src[..self.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = self.src[self.start..]
            .find('\n')
            .map_or(self.src.len(), |i| self.start + i);

        let line = &self.src[line_start..line_end];

        let col = self.start - line_start;
        let width = self.end - self.start;

        write!(f, "tokenizer: ")?;
        writeln!(f)?;

        write!(f, "{}", line)?;
        writeln!(f)?;

        for _ in 0..col {
            write!(f, " ")?;
        }

        for _ in 0..width {
            write!(f, "^")?;
        }

        writeln!(f)
    }
}

impl std::error::Error for TokenizerError {}
