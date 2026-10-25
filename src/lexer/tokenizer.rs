use crate::lexer::error::TokenizerError;
use crate::lexer::token::{NumberKind, NumberToken, Span, Suffix, Token, TokenKind};
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
                '(' | ')' | '{' | '}' | ';' | ':' | ',' | '+' | '*' | '/' | '~' => {
                    Ok(Token::single(ch, idx))
                }
                '<' | '>' => {
                    if self.chars.next_if(|(_, ch)| *ch == '=').is_some() {
                        Ok(Token {
                            kind: if ch == '<' {
                                TokenKind::LessEqual
                            } else {
                                TokenKind::GreaterEqual
                            },
                            span: Span {
                                start: idx,
                                end: idx + 2,
                            },
                        })
                    } else {
                        Ok(Token::single(ch, idx))
                    }
                }
                '!' => {
                    if self.chars.next_if(|(_, ch)| *ch == '=').is_some() {
                        Ok(Token {
                            kind: TokenKind::NotEqual,
                            span: Span {
                                start: idx,
                                end: idx + 2,
                            },
                        })
                    } else {
                        Ok(Token::single(ch, idx))
                    }
                }
                '=' => {
                    if self.chars.next_if(|(_, ch)| *ch == '=').is_some() {
                        Ok(Token {
                            kind: TokenKind::DoubleEqual,
                            span: Span {
                                start: idx,
                                end: idx + 2,
                            },
                        })
                    } else {
                        Ok(Token::single(ch, idx))
                    }
                }
                '-' => {
                    if self.chars.next_if(|(_, ch)| *ch == '>').is_some() {
                        Ok(Token {
                            kind: TokenKind::Arrow,
                            span: Span {
                                start: idx,
                                end: idx + 2,
                            },
                        })
                    } else {
                        Ok(Token::single(ch, idx))
                    }
                }
                _ if ch.is_ascii_digit() => self.tokenize_number(idx, ch),
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
                        "if" => TokenKind::If,
                        "else" => TokenKind::Else,
                        "loop" => TokenKind::Loop,
                        "while" => TokenKind::While,
                        "do" => TokenKind::Do,
                        "for" => TokenKind::For,
                        "break" => TokenKind::Break,
                        "continue" => TokenKind::Continue,
                        "goto" => TokenKind::Goto,
                        "print" => TokenKind::Print,
                        "ret" => TokenKind::Ret,
                        "let" => TokenKind::Let,
                        "as" => TokenKind::As,
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
                        "true" => TokenKind::True,
                        "false" => TokenKind::False,
                        "str" => TokenKind::Str,
                        _ => TokenKind::Identifier,
                    };

                    Ok(Token {
                        kind,
                        span: Span { start: idx, end },
                    })
                }
                _ => Err(TokenizerError {
                    src: self.src.to_string(),
                    msg: "Encountered unknown token.".to_string(),
                    span: Span {
                        start: idx,
                        end: idx + ch.len_utf8(),
                    },
                }),
            };

            return Some(token);
        }

        None
    }
}

impl<'src> Tokenizer<'src> {
    fn peek_char(&self, n: usize) -> Option<char> {
        self.chars.clone().nth(n).map(|(_, ch)| ch)
    }

    fn position(&mut self) -> usize {
        self.chars
            .peek()
            .map(|(idx, _)| *idx)
            .unwrap_or(self.src.len())
    }

    fn scan_digits(&mut self, radix: u32, initial_digit: bool) -> bool {
        let mut has_digit = initial_digit;

        while let Some(ch) = self.peek_char(0) {
            if ch == '_' {
                self.chars.next();
            } else if ch.is_digit(radix) {
                has_digit = true;
                self.chars.next();
            } else {
                break;
            }
        }

        has_digit
    }

    fn tokenize_number(&mut self, start: usize, first: char) -> Result<Token, TokenizerError> {
        let radix = if first == '0' {
            match self.peek_char(0) {
                Some('x' | 'X') => {
                    self.chars.next();

                    16
                }

                Some('b' | 'B') => {
                    self.chars.next();

                    2
                }

                Some('o' | 'O') => {
                    self.chars.next();

                    8
                }

                _ => 10,
            }
        } else {
            10
        };

        let mut kind = NumberKind::Integer { radix };

        if radix != 10 {
            /* The first char was '0', and we consumed up a radis prefix
             * such as 'x', 'b', or 'o', which means that at least one
             * digit must follow.  */

            if !self.scan_digits(radix, false) {
                let end = self.position();
                return Err(TokenizerError {
                    src: self.src.to_string(),
                    msg: "Expected at least one digit after numeric prefix.".to_string(),
                    span: Span { start, end },
                });
            }

            unreachable!()
        } else {
            /* The first decimal digit has already been consumed.  */

            self.scan_digits(10, true);

            /* Consume the optional fractional part.
             *
             * Avoid eating up '.' when followed by another '.'  */
            let decimal_point = self.peek_char(0) == Some('.')
                && !self
                    .peek_char(1)
                    .is_some_and(|ch| ch == '.' || ch == '_' || ch.is_alphabetic());

            if decimal_point {
                self.chars.next();

                self.scan_digits(10, false);

                kind = NumberKind::Float;
            }

            /* Consume the optional exponent.  */
            if matches!(self.peek_char(0), Some('e' | 'E')) {
                self.chars.next();

                /* Optional exponent sign.  */
                if matches!(self.peek_char(0), Some('+' | '-')) {
                    self.chars.next();
                }

                /* Require at least one digit for exponent.  */
                if !self.scan_digits(10, false) {
                    let end = self.position();

                    return Err(TokenizerError {
                        src: self.src.to_string(),
                        msg: "Expected at least one digit in numeric exponent".to_string(),
                        span: Span { start, end },
                    });
                }

                kind = NumberKind::Float;
            }

            let value_end = self.position();

            /* Parse optional type suffix.  */
            let suffix = if self
                .peek_char(0)
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
            {
                while self
                    .peek_char(0)
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
                {
                    self.chars.next();
                }

                let end = self.position();
                let txt = &self.src[value_end..end];

                let suffix = match Suffix::parse(txt) {
                    Some(suffix) => suffix,
                    None => {
                        return Err(TokenizerError {
                            src: self.src.to_string(),
                            msg: "Unknown numeric suffix.".to_string(),
                            span: Span {
                                start: value_end,
                                end,
                            },
                        });
                    }
                };

                /* Disallow suffix mismatch.  */
                if kind == NumberKind::Float && !suffix.is_float() {
                    return Err(TokenizerError {
                        src: self.src.to_string(),
                        msg: "A floating point literal cannot have an integer suffix.".to_string(),
                        span: Span {
                            start: value_end,
                            end,
                        },
                    });
                }

                if radix != 10 && suffix.is_float() {
                    return Err(TokenizerError {
                        src: self.src.to_string(),
                        msg: "A non-decimal integer cannot have a float suffix.".to_string(),
                        span: Span {
                            start: value_end,
                            end,
                        },
                    });
                }

                if suffix.is_float() {
                    kind = NumberKind::Float;
                }

                Some(suffix)
            } else {
                None
            };

            Ok(Token {
                kind: TokenKind::Number(NumberToken {
                    kind,
                    value_span: Span {
                        start,
                        end: value_end,
                    },
                    suffix,
                }),
                span: Span {
                    start,
                    end: self.position(),
                },
            })
        }
    }
}
