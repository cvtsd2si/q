use crate::lexer::error::TokenizerError;
use crate::lexer::token::{Token, TokenKind};
use std::iter::Peekable;
use std::str::CharIndices;

pub struct Tokenizer<'src> {
    src: &'src str,
    chars: Peekable<CharIndices<'src>>,
}

impl<'src> Tokenizer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src,
            chars: src.char_indices().peekable(),
        }
    }
}

impl<'src> Iterator for Tokenizer<'src> {
    type Item = Result<Token, TokenizerError>;

    fn next(&mut self) -> Option<Result<Token, TokenizerError>> {
        while let Some((idx, ch)) = self.chars.next() {
            if ch.is_whitespace() {
                continue;
            }

            let token = match ch {
                '(' | ')' | '{' | '}' | ';' | ':' | ',' | '+' | '*' | '/' | '=' => {
                    Ok(Token::single(ch, idx))
                }
                '-' => {
                    if self.chars.next_if(|(_, ch)| *ch == '>').is_some() {
                        Ok(Token {
                            kind: TokenKind::Arrow,
                            start: idx,
                            end: idx + 2,
                        })
                    } else {
                        Ok(Token::single(ch, idx))
                    }
                }
                _ if ch.is_ascii_digit() => {
                    let mut seen_fp = false;

                    while let Some((_, ch)) = self.chars.peek() {
                        match ch {
                            '0'..='9' => {
                                self.chars.next();
                            }

                            '.' if !seen_fp => {
                                seen_fp = true;
                                self.chars.next();
                            }

                            '.' => {
                                return Some(Err(TokenizerError {
                                    src: self.src.to_string(),
                                    msg: "Already seen fp.".to_string(),
                                    start: idx,
                                    end: idx + 1,
                                }));
                            }

                            _ => break,
                        }
                    }

                    let end = self
                        .chars
                        .peek()
                        .map(|(idx, _)| *idx)
                        .unwrap_or(self.src.len());

                    Ok(Token {
                        kind: TokenKind::Number,
                        start: idx,
                        end,
                    })
                }
                _ if ch.is_alphabetic() || ch == '_' => {
                    while self
                        .chars
                        .peek()
                        .is_some_and(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
                    {
                        self.chars.next();
                    }

                    let end = self
                        .chars
                        .peek()
                        .map(|(idx, _)| *idx)
                        .unwrap_or(self.src.len());

                    let kind = match &self.src[idx..end] {
                        "fn" => TokenKind::Fn,
                        "ret" => TokenKind::Ret,
                        "let" => TokenKind::Let,
                        "u8" => TokenKind::U8,
                        "i8" => TokenKind::I8,
                        "u16" => TokenKind::U16,
                        "i16" => TokenKind::I16,
                        "u32" => TokenKind::U32,
                        "i32" => TokenKind::I32,
                        "u64" => TokenKind::U64,
                        "i64" => TokenKind::I64,
                        "f32" => TokenKind::F32,
                        "f64" => TokenKind::F64,
                        "bool" => TokenKind::Bool,
                        "str" => TokenKind::Str,
                        _ => TokenKind::Identifier,
                    };

                    Ok(Token {
                        kind,
                        start: idx,
                        end,
                    })
                }
                _ => Err(TokenizerError {
                    src: self.src.to_string(),
                    msg: "Encountered unknown token.".to_string(),
                    start: idx,
                    end: idx + 1,
                }),
            };

            return Some(token);
        }

        None
    }
}
