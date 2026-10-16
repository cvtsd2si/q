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
    pub mod r#gen;
}
