use crate::ast::Span;
use crate::diagnostic::Diagnostic;

/// One deterministic floating-point literal.
///
/// `wide` selects the declared type: `true` is `f64` and `false` is an
/// explicit `f32` suffix. The value always round-trips through the canonical
/// formatter, so revisions hash stable bytes.
#[derive(Clone, Copy, Debug)]
pub struct FloatLiteral {
    pub value: f64,
    pub wide: bool,
}

impl PartialEq for FloatLiteral {
    fn eq(&self, other: &Self) -> bool {
        self.wide == other.wide && self.value.to_bits() == other.value.to_bits()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(i64),
    Float(FloatLiteral),
    String(String),
    At,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    ColonColon,
    Dot,
    Comma,
    Semicolon,
    Arrow,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Question,
    Bang,
    Eq,
    FatArrow,
    EqEq,
    BangEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str, path: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer {
        source,
        path,
        offset: 0,
        line: 1,
        column: 1,
    }
    .lex_all()
}

struct Lexer<'a> {
    source: &'a str,
    path: &'a str,
    offset: usize,
    line: usize,
    column: usize,
}

impl Lexer<'_> {
    fn lex_all(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        while self.offset < self.source.len() {
            self.skip_trivia();
            if self.offset >= self.source.len() {
                break;
            }
            tokens.push(self.next_token()?);
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            span: self.span_from(self.offset, self.line, self.column),
        });
        Ok(tokens)
    }

    fn skip_trivia(&mut self) {
        loop {
            while matches!(self.peek(), Some(character) if character.is_whitespace()) {
                self.bump();
            }
            if self.starts_with("//") {
                while !matches!(self.peek(), None | Some('\n')) {
                    self.bump();
                }
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Result<Token, Diagnostic> {
        let start = self.offset;
        let line = self.line;
        let column = self.column;
        let character = self.bump().expect("lexer called at end of input");
        let kind = match character {
            '@' => TokenKind::At,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            ':' if self.take(':') => TokenKind::ColonColon,
            ':' => TokenKind::Colon,
            '.' => TokenKind::Dot,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '%' => TokenKind::Percent,
            '?' => TokenKind::Question,
            '-' if self.take('>') => TokenKind::Arrow,
            '-' => TokenKind::Minus,
            '/' => TokenKind::Slash,
            '!' if self.take('=') => TokenKind::BangEq,
            '!' => TokenKind::Bang,
            '=' if self.take('=') => TokenKind::EqEq,
            '=' if self.take('>') => TokenKind::FatArrow,
            '=' => TokenKind::Eq,
            '<' if self.take('=') => TokenKind::Le,
            '<' => TokenKind::Lt,
            '>' if self.take('=') => TokenKind::Ge,
            '>' => TokenKind::Gt,
            '&' if self.take('&') => TokenKind::AndAnd,
            '|' if self.take('|') => TokenKind::OrOr,
            '"' => return self.string_token(start, line, column),
            value if value.is_ascii_digit() => {
                while matches!(self.peek(), Some(next) if next.is_ascii_digit()) {
                    self.bump();
                }
                if self.peek() == Some('.')
                    && self
                        .source
                        .as_bytes()
                        .get(self.offset + 1)
                        .is_some_and(|next| next.is_ascii_digit())
                {
                    return self.float_token(start, line, column);
                }
                let text = &self.source[start..self.offset];
                let number = text.parse::<i64>().map_err(|_| {
                    self.error(
                        "SPX-P003",
                        "integer literal is outside the i64 range",
                        self.span_from(start, line, column),
                    )
                })?;
                TokenKind::Int(number)
            }
            value if is_ident_start(value) => {
                while matches!(self.peek(), Some(next) if is_ident_continue(next)) {
                    self.bump();
                }
                TokenKind::Ident(self.source[start..self.offset].to_owned())
            }
            '&' | '|' => {
                return Err(self.error(
                    "SPX-P002",
                    format!("unexpected `{character}`; did you mean `{character}{character}`?"),
                    self.span_from(start, line, column),
                ));
            }
            _ => {
                return Err(self.error(
                    "SPX-P001",
                    format!("unexpected character `{character}`"),
                    self.span_from(start, line, column),
                ));
            }
        };
        Ok(Token {
            kind,
            span: self.span_from(start, line, column),
        })
    }

    fn float_token(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Token, Diagnostic> {
        // Fraction digits (the caller consumed the leading integer digits and
        // validated that a fraction follows).
        self.bump();
        while matches!(self.peek(), Some(next) if next.is_ascii_digit()) {
            self.bump();
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            let mut cursor = self.offset + 1;
            let mut exponent_digits = 0usize;
            if matches!(self.source.as_bytes().get(cursor), Some(b'+') | Some(b'-')) {
                cursor += 1;
            }
            while self
                .source
                .as_bytes()
                .get(cursor)
                .is_some_and(|next| next.is_ascii_digit())
            {
                cursor += 1;
                exponent_digits += 1;
            }
            if exponent_digits == 0 {
                return Err(self.error(
                    "SPX-P003",
                    "floating-point exponent requires at least one digit",
                    self.span_from(start, line, column),
                ));
            }
            while self.offset < cursor {
                self.bump();
            }
        }
        let mut wide = true;
        if self.starts_with("f32") || self.starts_with("f64") {
            wide = self.starts_with("f64");
            for _ in 0..3 {
                self.bump();
            }
        } else if self.peek().is_some_and(is_ident_start) {
            return Err(self.error(
                "SPX-P003",
                "floating-point literals accept only an `f32` or `f64` suffix",
                self.span_from(start, line, column),
            ));
        }
        if self.peek().is_some_and(is_ident_continue) {
            return Err(self.error(
                "SPX-P003",
                "floating-point literals accept only an `f32` or `f64` suffix",
                self.span_from(start, line, column),
            ));
        }
        let text = &self.source[start..self.offset];
        let value = if wide {
            text.parse::<f64>().map_err(|_| {
                self.error(
                    "SPX-P003",
                    "floating-point literal is outside the f64 range",
                    self.span_from(start, line, column),
                )
            })?
        } else {
            // Parse the unsuffixed text directly in the declared precision so
            // an f32 literal rounds once from decimal digits.
            text[..text.len() - 3]
                .parse::<f32>()
                .map(f64::from)
                .map_err(|_| {
                    self.error(
                        "SPX-P003",
                        "floating-point literal is outside the f32 range",
                        self.span_from(start, line, column),
                    )
                })?
        };
        if !value.is_finite() {
            return Err(self.error(
                "SPX-P003",
                "floating-point literal is outside the f64 range",
                self.span_from(start, line, column),
            ));
        }
        Ok(Token {
            kind: TokenKind::Float(FloatLiteral { value, wide }),
            span: self.span_from(start, line, column),
        })
    }

    fn string_token(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Token, Diagnostic> {
        let mut value = String::new();
        loop {
            match self.bump() {
                Some('"') => break,
                Some('\\') => match self.bump() {
                    Some('n') => value.push('\n'),
                    Some('r') => value.push('\r'),
                    Some('t') => value.push('\t'),
                    Some('"') => value.push('"'),
                    Some('\\') => value.push('\\'),
                    Some(other) => {
                        return Err(self.error(
                            "SPX-P005",
                            format!("unsupported string escape `\\{other}`"),
                            self.span_from(start, line, column),
                        ));
                    }
                    None => {
                        return Err(self.error(
                            "SPX-P004",
                            "unterminated string literal",
                            self.span_from(start, line, column),
                        ));
                    }
                },
                Some(character) => value.push(character),
                None => {
                    return Err(self.error(
                        "SPX-P004",
                        "unterminated string literal",
                        self.span_from(start, line, column),
                    ));
                }
            }
        }
        Ok(Token {
            kind: TokenKind::String(value),
            span: self.span_from(start, line, column),
        })
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }

    fn take(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source[self.offset..].starts_with(value)
    }

    fn span_from(&self, start: usize, line: usize, column: usize) -> Span {
        Span {
            start,
            end: self.offset,
            line,
            column,
        }
    }

    fn error(&self, code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic::error(code, message, span).at_path(self.path)
    }
}

fn is_ident_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_ident_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}
