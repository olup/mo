use crate::span::Span;
use crate::token::{Keyword, Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    index: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().collect(),
            index: 0,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while let Some((start, ch)) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.bump();
                }
                '\n' => {
                    self.bump();
                    tokens.push(Token::new(TokenKind::Newline, start, start + 1));
                }
                '/' if self.peek_next_char() == Some('/') => self.skip_line_comment(),
                '/' if self.peek_next_char() == Some('*') => self.skip_block_comment()?,
                '"' => tokens.push(self.string()?),
                '\'' => tokens.push(self.char_lit()?),
                '0'..='9' => tokens.push(self.number()),
                ch if is_ident_start(ch) => tokens.push(self.ident_or_keyword()),
                _ => tokens.push(self.punct()?),
            }
        }

        let end = self.source.len();
        tokens.push(Token::new(TokenKind::Eof, end, end));
        Ok(tokens)
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars.get(self.index).copied()
    }

    fn peek_next_char(&self) -> Option<char> {
        self.peek_nth_char(1)
    }

    fn peek_nth_char(&self, offset: usize) -> Option<char> {
        self.chars.get(self.index + offset).map(|(_, ch)| *ch)
    }

    fn bump(&mut self) -> Option<(usize, char)> {
        let value = self.peek();
        if value.is_some() {
            self.index += 1;
        }
        value
    }

    fn skip_line_comment(&mut self) {
        while let Some((_, ch)) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.bump();
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let (start, _) = self.bump().unwrap();
        self.bump();
        let mut depth = 1;

        while let Some((_, ch)) = self.bump() {
            match (ch, self.peek().map(|(_, next)| next)) {
                ('/', Some('*')) => {
                    self.bump();
                    depth += 1;
                }
                ('*', Some('/')) => {
                    self.bump();
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        Err(LexError {
            message: "unterminated block comment".into(),
            span: Span::new(start, self.source.len()),
        })
    }

    fn string(&mut self) -> Result<Token, LexError> {
        let (start, _) = self.bump().unwrap();
        let mut value = String::new();

        while let Some((end, ch)) = self.bump() {
            match ch {
                '"' => return Ok(Token::new(TokenKind::String(value), start, end + 1)),
                '\\' => {
                    let Some((_, escaped)) = self.bump() else {
                        break;
                    };
                    value.push(match escaped {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        '"' => '"',
                        '\\' => '\\',
                        other => other,
                    });
                }
                other => value.push(other),
            }
        }

        Err(LexError {
            message: "unterminated string literal".into(),
            span: Span::new(start, self.source.len()),
        })
    }

    fn char_lit(&mut self) -> Result<Token, LexError> {
        let (start, _) = self.bump().unwrap();
        let mut value = String::new();

        while let Some((end, ch)) = self.bump() {
            match ch {
                '\'' => return Ok(Token::new(TokenKind::Char(value), start, end + 1)),
                '\\' => {
                    if let Some((_, escaped)) = self.bump() {
                        value.push(escaped);
                    }
                }
                other => value.push(other),
            }
        }

        Err(LexError {
            message: "unterminated char literal".into(),
            span: Span::new(start, self.source.len()),
        })
    }

    fn number(&mut self) -> Token {
        let (start, _) = self.peek().unwrap();
        let mut end = start;
        let mut seen_dot = false;

        while let Some((idx, ch)) = self.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                end = idx + ch.len_utf8();
                self.bump();
            } else if ch == '.'
                && !seen_dot
                && self.peek_next_char().is_some_and(|c| c.is_ascii_digit())
            {
                seen_dot = true;
                end = idx + 1;
                self.bump();
            } else {
                break;
            }
        }

        let text = self.source[start..end].to_string();
        let kind = if seen_dot {
            TokenKind::Float(text)
        } else {
            TokenKind::Int(text)
        };
        Token::new(kind, start, end)
    }

    fn ident_or_keyword(&mut self) -> Token {
        let (start, _) = self.peek().unwrap();
        let mut end = start;

        while let Some((idx, ch)) = self.peek() {
            if is_ident_continue(ch) {
                end = idx + ch.len_utf8();
                self.bump();
            } else {
                break;
            }
        }

        let text = &self.source[start..end];
        let kind = Keyword::from_ident(text)
            .map(TokenKind::Keyword)
            .unwrap_or_else(|| TokenKind::Ident(text.to_string()));
        Token::new(kind, start, end)
    }

    fn punct(&mut self) -> Result<Token, LexError> {
        let (start, ch) = self.bump().unwrap();
        let next = self.peek().map(|(_, next)| next);
        let kind = match (ch, next) {
            ('<', Some('<')) if self.peek_nth_char(1) == Some('=') => {
                self.bump();
                self.bump();
                TokenKind::LtLtEq
            }
            ('>', Some('>')) if self.peek_nth_char(1) == Some('=') => {
                self.bump();
                self.bump();
                TokenKind::GtGtEq
            }
            ('-', Some('>')) => {
                self.bump();
                TokenKind::Arrow
            }
            ('+', Some('=')) => {
                self.bump();
                TokenKind::PlusEq
            }
            ('-', Some('=')) => {
                self.bump();
                TokenKind::MinusEq
            }
            ('*', Some('=')) => {
                self.bump();
                TokenKind::StarEq
            }
            ('/', Some('=')) => {
                self.bump();
                TokenKind::SlashEq
            }
            ('%', Some('=')) => {
                self.bump();
                TokenKind::PercentEq
            }
            ('&', Some('&')) => {
                self.bump();
                TokenKind::AmpAmp
            }
            ('&', Some('=')) => {
                self.bump();
                TokenKind::AmpEq
            }
            ('|', Some('|')) => {
                self.bump();
                TokenKind::PipePipe
            }
            ('|', Some('=')) => {
                self.bump();
                TokenKind::PipeEq
            }
            ('<', Some('<')) => {
                self.bump();
                TokenKind::LtLt
            }
            ('>', Some('>')) => {
                self.bump();
                TokenKind::GtGt
            }
            ('=', Some('>')) => {
                self.bump();
                TokenKind::FatArrow
            }
            ('=', Some('=')) => {
                self.bump();
                TokenKind::EqEq
            }
            ('!', Some('=')) => {
                self.bump();
                TokenKind::BangEq
            }
            ('<', Some('=')) => {
                self.bump();
                TokenKind::Le
            }
            ('>', Some('=')) => {
                self.bump();
                TokenKind::Ge
            }
            ('@', _) => TokenKind::At,
            ('.', _) => TokenKind::Dot,
            (',', _) => TokenKind::Comma,
            (':', _) => TokenKind::Colon,
            (';', _) => TokenKind::Semicolon,
            ('(', _) => TokenKind::LParen,
            (')', _) => TokenKind::RParen,
            ('{', _) => TokenKind::LBrace,
            ('}', _) => TokenKind::RBrace,
            ('[', _) => TokenKind::LBracket,
            (']', _) => TokenKind::RBracket,
            ('<', _) => TokenKind::Lt,
            ('>', _) => TokenKind::Gt,
            ('&', _) => TokenKind::Amp,
            ('*', _) => TokenKind::Star,
            ('^', _) => TokenKind::Caret,
            ('?', _) => TokenKind::Question,
            ('+', _) => TokenKind::Plus,
            ('-', _) => TokenKind::Minus,
            ('/', _) => TokenKind::Slash,
            ('%', _) => TokenKind::Percent,
            ('=', _) => TokenKind::Eq,
            ('!', _) => TokenKind::Bang,
            ('|', _) => TokenKind::Pipe,
            _ => {
                return Err(LexError {
                    message: format!("unexpected character `{ch}`"),
                    span: Span::new(start, start + ch.len_utf8()),
                })
            }
        };
        let end = self.peek().map(|(idx, _)| idx).unwrap_or(self.source.len());
        Ok(Token::new(kind, start, end))
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}
