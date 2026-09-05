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
    /// One unsuffixed decimal literal whose magnitude is exactly
    /// `i64::MAX + 1`. The grammar admits it only as the immediate operand of
    /// a unary `-`, where it denotes `i64::MIN`; every other position keeps
    /// the stable `SPX-P003` out-of-range rejection. Carrying the magnitude
    /// as its own token keeps tokenization context-free: the lexer never
    /// consults a preceding `-`, so subtraction is unaffected.
    IntMinMagnitude,
    /// One integer literal with an explicit `i32` suffix and its exact value.
    Int32(i32),
    /// The `i32` counterpart of [`TokenKind::IntMinMagnitude`]: a suffixed
    /// decimal literal whose magnitude is exactly `i32::MAX + 1`.
    Int32MinMagnitude,
    Float(FloatLiteral),
    /// One `char` literal held as its exact Unicode scalar value.
    Char(u32),
    /// One `u8` literal held as its exact value.
    Uint8(u8),
    /// One target-independent unsigned 64-bit literal with a `usize` suffix.
    Usize(u64),
    String(String),
    At,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
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
    /// Refutable Match v1: a single `|` separating or-pattern alternatives.
    /// The expression grammar never accepts it, so only pattern positions
    /// observe the new token.
    Pipe,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// One `//` comment the lexer skipped, kept so the canonical formatter can
/// put it back. The grammar never sees comments; they are trivia with a
/// position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    /// The comment text after the leading `//`, with trailing whitespace
    /// removed. Interior text is verbatim, so `//x` and `// x` stay distinct.
    pub text: String,
    /// Byte offset of the leading `//` in the source.
    pub offset: usize,
    /// One-based line of the comment.
    pub line: usize,
    /// `true` when only whitespace precedes the comment on its line; `false`
    /// when it trails a token on the same line.
    pub own_line: bool,
    /// One-based line on which the previous token ended, or zero before the
    /// first token. A comment on the very next line sticks to that token's
    /// item when the formatter puts it back.
    pub previous_token_line: usize,
}

/// Every comment of one source file plus the offset of its first token, which
/// separates file-header comments from comments inside the module.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Comments {
    pub items: Vec<Comment>,
    pub first_token_offset: usize,
}

pub fn lex(source: &str, path: &str) -> Result<Vec<Token>, Diagnostic> {
    lex_with_comments(source, path).map(|(tokens, _)| tokens)
}

/// Lex the source and also return every comment, in source order.
pub fn lex_with_comments(source: &str, path: &str) -> Result<(Vec<Token>, Comments), Diagnostic> {
    if source.starts_with('\u{feff}') {
        return Err(Diagnostic::error(
            "SPX-P001",
            "file starts with a UTF-8 byte-order mark",
            Span {
                start: 0,
                end: '\u{feff}'.len_utf8(),
                line: 1,
                column: 1,
            },
        )
        .at_path(path)
        .with_help("remove the UTF-8 byte-order mark before the `module` header"));
    }
    Lexer {
        source,
        path,
        offset: 0,
        line: 1,
        column: 1,
        token_on_line: false,
        previous_token_line: 0,
        comments: Vec::new(),
    }
    .lex_all()
}

struct Lexer<'a> {
    source: &'a str,
    path: &'a str,
    offset: usize,
    line: usize,
    column: usize,
    /// Whether a token has already been produced on the current line, which
    /// decides whether a comment is on its own line or trails a token.
    token_on_line: bool,
    previous_token_line: usize,
    comments: Vec<Comment>,
}

impl Lexer<'_> {
    fn lex_all(mut self) -> Result<(Vec<Token>, Comments), Diagnostic> {
        let mut tokens = Vec::new();
        while self.offset < self.source.len() {
            self.skip_trivia();
            if self.offset >= self.source.len() {
                break;
            }
            tokens.push(self.next_token()?);
            self.token_on_line = true;
            self.previous_token_line = self.line;
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            span: self.span_from(self.offset, self.line, self.column),
        });
        let first_token_offset = tokens[0].span.start;
        Ok((
            tokens,
            Comments {
                items: self.comments,
                first_token_offset,
            },
        ))
    }

    fn skip_trivia(&mut self) {
        loop {
            while matches!(self.peek(), Some(character) if character.is_whitespace()) {
                self.bump();
            }
            if self.starts_with("//") {
                let offset = self.offset;
                let line = self.line;
                let own_line = !self.token_on_line;
                while !matches!(self.peek(), None | Some('\n')) {
                    self.bump();
                }
                self.comments.push(Comment {
                    text: self.source[offset + 2..self.offset].trim_end().to_owned(),
                    offset,
                    line,
                    own_line,
                    previous_token_line: self.previous_token_line,
                });
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
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
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
            '|' => TokenKind::Pipe,
            '"' => return self.string_token(start, line, column),
            '\'' => return self.char_token(start, line, column),
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
                if self.starts_with("i32") {
                    return self.int32_token(start, line, column);
                }
                if self.starts_with("u8") {
                    return self.uint8_token(start, line, column);
                }
                if self.starts_with("usize") {
                    return self.usize_token(start, line, column);
                }
                if self.peek().is_some_and(is_ident_start) {
                    return Err(self.error(
                        "SPX-P003",
                        "integer literals accept only an `i32`, `u8`, or `usize` suffix",
                        self.span_from(start, line, column),
                    ));
                }
                let text = &self.source[start..self.offset];
                match text.parse::<i64>() {
                    Ok(number) => TokenKind::Int(number),
                    // Signed Minimum Literals v1: exactly `i64::MAX + 1`
                    // survives tokenization so that a directly applied unary
                    // `-` can spell `i64::MIN`. The parser rejects it with
                    // the same stable diagnostic everywhere else.
                    Err(_) if is_signed_minimum_magnitude(text, I64_MIN_MAGNITUDE) => {
                        TokenKind::IntMinMagnitude
                    }
                    Err(_) => {
                        return Err(self.error(
                            "SPX-P003",
                            "integer literal is outside the i64 range",
                            self.span_from(start, line, column),
                        ))
                    }
                }
            }
            value if is_ident_start(value) => {
                while matches!(self.peek(), Some(next) if is_ident_continue(next)) {
                    self.bump();
                }
                TokenKind::Ident(self.source[start..self.offset].to_owned())
            }
            '&' => {
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

    fn int32_token(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Token, Diagnostic> {
        for _ in 0..3 {
            self.bump();
        }
        if self.peek().is_some_and(is_ident_continue) {
            return Err(self.error(
                "SPX-P003",
                "integer literals accept only an `i32` suffix",
                self.span_from(start, line, column),
            ));
        }
        let text = &self.source[start..self.offset - 3];
        let kind = match text.parse::<i32>() {
            Ok(value) => TokenKind::Int32(value),
            // Signed Minimum Literals v1: see `TokenKind::IntMinMagnitude`.
            Err(_) if is_signed_minimum_magnitude(text, I32_MIN_MAGNITUDE) => {
                TokenKind::Int32MinMagnitude
            }
            Err(_) => {
                return Err(self.error(
                    "SPX-P003",
                    "integer literal is outside the i32 range",
                    self.span_from(start, line, column),
                ))
            }
        };
        Ok(Token {
            kind,
            span: self.span_from(start, line, column),
        })
    }

    fn uint8_token(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Token, Diagnostic> {
        for _ in 0..2 {
            self.bump();
        }
        if self.peek().is_some_and(is_ident_continue) {
            return Err(self.error(
                "SPX-P003",
                "integer literals accept only a `u8` suffix",
                self.span_from(start, line, column),
            ));
        }
        let text = &self.source[start..self.offset - 2];
        let number = text.parse::<i64>().map_err(|_| {
            self.error(
                "SPX-P003",
                "u8 literal is outside the u8 range",
                self.span_from(start, line, column),
            )
        })?;
        let value = u8::try_from(number).map_err(|_| {
            self.error(
                "SPX-P003",
                "u8 literal is outside the u8 range",
                self.span_from(start, line, column),
            )
        })?;
        Ok(Token {
            kind: TokenKind::Uint8(value),
            span: self.span_from(start, line, column),
        })
    }

    fn usize_token(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Token, Diagnostic> {
        for _ in 0..5 {
            self.bump();
        }
        if self.peek().is_some_and(is_ident_continue) {
            return Err(self.error(
                "SPX-T260",
                "usize literals require exactly the `usize` suffix",
                self.span_from(start, line, column),
            ));
        }
        let text = &self.source[start..self.offset - 5];
        let value = text.parse::<u64>().map_err(|_| {
            self.error(
                "SPX-T260",
                "usize literal is outside the target-independent u64 range",
                self.span_from(start, line, column),
            )
        })?;
        Ok(Token {
            kind: TokenKind::Usize(value),
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
        let mut suffix_len = 0usize;
        if self.starts_with("f32") || self.starts_with("f64") {
            wide = self.starts_with("f64");
            suffix_len = 3;
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
        // Parse the unsuffixed digits directly in the declared precision so a
        // literal rounds once from decimal digits. An explicit suffix is never
        // part of the number, in either precision.
        let digits = &text[..text.len() - suffix_len];
        let value = if wide {
            digits.parse::<f64>().map_err(|_| {
                self.error(
                    "SPX-P003",
                    "floating-point literal is outside the f64 range",
                    self.span_from(start, line, column),
                )
            })?
        } else {
            digits.parse::<f32>().map(f64::from).map_err(|_| {
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
                    Some('\'') => value.push('\''),
                    Some('\\') => value.push('\\'),
                    Some('u') if self.take('{') => {
                        let mut digits = String::new();
                        while matches!(self.peek(), Some(next) if next.is_ascii_hexdigit()) {
                            digits.push(self.bump().expect("hex digit peeked"));
                        }
                        if !self.take('}') || digits.is_empty() || digits.len() > 6 {
                            return Err(self.error(
                                "SPX-P005",
                                "unicode escape requires one to six hexadecimal digits in braces",
                                self.span_from(start, line, column),
                            ));
                        }
                        let scalar = u32::from_str_radix(&digits, 16).map_err(|_| {
                            self.error(
                                "SPX-P005",
                                "unicode escape is outside the Unicode range",
                                self.span_from(start, line, column),
                            )
                        })?;
                        let scalar = char::from_u32(scalar).ok_or_else(|| {
                            self.error(
                                "SPX-P005",
                                "unicode escape is not a Unicode scalar value",
                                self.span_from(start, line, column),
                            )
                        })?;
                        value.push(scalar);
                    }
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

    /// Lex one `char` literal: exactly one Unicode scalar value between
    /// single quotes. The canonical escapes are `\n`, `\r`, `\t`, `\0`,
    /// `\\`, `\'`, and `\u{...}` with one to six hexadecimal digits.
    fn char_token(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Token, Diagnostic> {
        let value = match self.bump() {
            Some('\'') => {
                return Err(self.error(
                    "SPX-P008",
                    "char literal requires exactly one Unicode scalar value",
                    self.span_from(start, line, column),
                ));
            }
            Some('\\') => self.char_escape(start, line, column)?,
            Some(character) => character,
            None => {
                return Err(self.error(
                    "SPX-P006",
                    "unterminated char literal",
                    self.span_from(start, line, column),
                ));
            }
        };
        match self.peek() {
            Some('\'') => {
                self.bump();
            }
            Some(_) => {
                return Err(self.error(
                    "SPX-P008",
                    "char literal requires exactly one Unicode scalar value",
                    self.span_from(start, line, column),
                ));
            }
            None => {
                return Err(self.error(
                    "SPX-P006",
                    "unterminated char literal",
                    self.span_from(start, line, column),
                ));
            }
        }
        if self.peek().is_some_and(is_ident_continue) {
            return Err(self.error(
                "SPX-P008",
                "char literal requires exactly one Unicode scalar value",
                self.span_from(start, line, column),
            ));
        }
        Ok(Token {
            kind: TokenKind::Char(value as u32),
            span: self.span_from(start, line, column),
        })
    }

    fn char_escape(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<char, Diagnostic> {
        match self.bump() {
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some('0') => Ok('\0'),
            Some('\'') => Ok('\''),
            Some('\\') => Ok('\\'),
            Some('u') if self.take('{') => {
                let mut digits = String::new();
                while matches!(self.peek(), Some(next) if next.is_ascii_hexdigit()) {
                    digits.push(self.bump().expect("hex digit peeked"));
                }
                if !self.take('}') || digits.is_empty() || digits.len() > 6 {
                    return Err(self.error(
                        "SPX-P007",
                        "unicode escape requires one to six hexadecimal digits in braces",
                        self.span_from(start, line, column),
                    ));
                }
                let scalar = u32::from_str_radix(&digits, 16).map_err(|_| {
                    self.error(
                        "SPX-P007",
                        "unicode escape is outside the Unicode range",
                        self.span_from(start, line, column),
                    )
                })?;
                char::from_u32(scalar).ok_or_else(|| {
                    self.error(
                        "SPX-P007",
                        "unicode escape is not a Unicode scalar value",
                        self.span_from(start, line, column),
                    )
                })
            }
            Some(other) => Err(self.error(
                "SPX-P007",
                format!("unsupported char escape `\\{other}`"),
                self.span_from(start, line, column),
            )),
            None => Err(self.error(
                "SPX-P006",
                "unterminated char literal",
                self.span_from(start, line, column),
            )),
        }
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
            self.token_on_line = false;
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

/// The decimal magnitude of `i64::MIN`, which is one past `i64::MAX`.
const I64_MIN_MAGNITUDE: &str = "9223372036854775808";

/// The decimal magnitude of `i32::MIN`, which is one past `i32::MAX`.
const I32_MIN_MAGNITUDE: &str = "2147483648";

/// Whether `text` spells exactly the magnitude of a signed minimum. The rule
/// is numeric rather than textual, so leading zeros are ignored the same way
/// every other in-range decimal literal ignores them.
fn is_signed_minimum_magnitude(text: &str, magnitude: &str) -> bool {
    text.trim_start_matches('0') == magnitude
}

fn is_ident_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_ident_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests;
