pub mod lexer {
    pub mod error;
    pub mod token;
    pub mod tokenizer;
}

pub mod parser {
    pub mod ast;
    pub mod error;
    pub mod recursive_descent;
}

pub mod ir {
    pub mod cfg;
    pub mod r#gen;
}

pub mod asm {
    pub mod emitter;
    pub mod fixup;
    pub mod r#gen;
    pub mod regalloc;
}

pub mod bytecode {
    pub mod r#gen;
}

pub mod runtime {
    pub mod jit;
    pub mod rt;
    pub mod value;
    pub mod vm;
}

pub mod semantics {
    pub mod label_collector;
    pub mod loop_labeler;
    pub mod resolver;
    pub mod typechecker;
}
