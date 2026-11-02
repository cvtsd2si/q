use crate::tokenizer::token::Span;

pub mod analysis;
pub mod codegen;
pub mod parser;
pub mod runtime;
pub mod semantics;
pub mod tokenizer;

pub trait PrintSrc {
    fn print_src(&self, src: &str);
}

#[derive(Debug)]
pub struct QError {
    pub kind: QErrorKind,
    pub msg: String,
    pub code: u32,
    pub span: Span,
}

impl QError {
    pub fn carries_span(&self) -> bool {
        matches!(
            self.kind,
            QErrorKind::Tokenize
                | QErrorKind::Parse
                | QErrorKind::LabelCollect
                | QErrorKind::LoopLabel
                | QErrorKind::Resolve
                | QErrorKind::Typecheck
        )
    }
}

impl PrintSrc for QError {
    fn print_src(&self, src: &str) {
        let line_start = src[..self.span.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = src[self.span.start..]
            .find('\n')
            .map_or(src.len(), |i| self.span.start + i);

        let line = &src[line_start..line_end];

        let col = self.span.start - line_start;
        let width = (self.span.end - self.span.start).max(1);

        eprint!("{line}");
        eprintln!();

        for _ in 0..col {
            eprint!(" ");
        }

        for _ in 0..width {
            eprint!("^");
        }

        eprintln!();
    }
}

#[derive(Debug)]
pub enum QErrorKind {
    Tokenize,
    Parse,
    Resolve,
    LabelCollect,
    LabelCheck,
    LoopLabel,
    Typecheck,
    Bytecode,
    Regalloc,
    Jit,
    Vm,
}

impl std::error::Error for QError {}

impl std::fmt::Display for QError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.msg)?;
        writeln!(f, "err: {:?}", self.kind)?;
        writeln!(f)
    }
}
