#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn single(ch: char, pos: usize) -> Self {
        Self {
            kind: Self::kind4ch(ch),
            start: pos,
            end: pos + 1,
        }
    }

    fn kind4ch(ch: char) -> TokenKind {
        match ch {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            ';' => TokenKind::Semicolon,
            ':' => TokenKind::Colon,
            ',' => TokenKind::Comma,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '=' => TokenKind::Equal,
            _ => panic!("Cannot find TokenKind for {}", ch),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    Fn,
    Identifier,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    Arrow,
    Ret,
    Let,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Bool,
    Str,
    Semicolon,
    Number,
}
