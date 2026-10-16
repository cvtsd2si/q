use crate::lexer::token::Span;

#[derive(Debug)]
pub struct TokenizerError {
    pub src: String,
    pub msg: String,
    pub span: Span,
}

impl std::fmt::Display for TokenizerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line_start = self.src[..self.span.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = self.src[self.span.start..]
            .find('\n')
            .map_or(self.src.len(), |i| self.span.start + i);

        let line = &self.src[line_start..line_end];

        let col = self.span.start - line_start;
        let width = self.span.end - self.span.start;

        write!(f, "tokenizer: ")?;
        writeln!(f)?;

        writeln!(f, "{}", self.msg)?;

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
