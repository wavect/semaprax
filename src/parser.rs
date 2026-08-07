use std::path::Path;

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Param, Program, Span, Type, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::lexer::{lex, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    path: String,
}

impl Parser {
    pub fn new(source: &str, path: &Path) -> Result<Self, Diagnostic> {
        let path = path.display().to_string();
        Ok(Self {
            tokens: lex(source, &path)?,
            cursor: 0,
            path,
        })
    }

    pub fn parse(mut self) -> Result<Program, Diagnostic> {
        self.keyword("module")?;
        let (module, _) = self.ident("module name")?;
        self.take(&TokenKind::Semicolon);

        let permits = if self.at_keyword("permit") {
            self.bump();
            self.effect_set()?
        } else {
            Vec::new()
        };

        let mut functions = Vec::new();
        while !self.at(&TokenKind::Eof) {
            functions.push(self.function(&module)?);
        }
        if functions.is_empty() {
            return Err(self.error_here("SPX-P101", "a module must declare at least one function"));
        }
        Ok(Program {
            path: self.path,
            module,
            permits,
            functions,
        })
    }

    fn function(&mut self, module: &str) -> Result<Function, Diagnostic> {
        let start = self.current().span;
        let mut explicit_id = false;
        let stable_id = if self.take(&TokenKind::At) {
            let (attribute, _) = self.ident("attribute name")?;
            if attribute != "id" {
                return Err(self.error_here(
                    "SPX-P102",
                    format!("unknown attribute `@{attribute}`; only `@id` is supported"),
                ));
            }
            self.expect(&TokenKind::LParen, "`(` after @id")?;
            let id = match &self.bump().kind {
                TokenKind::String(value) => value.clone(),
                _ => return Err(self.error_previous("SPX-P103", "@id expects a string literal")),
            };
            self.expect(&TokenKind::RParen, "`)` after stable id")?;
            explicit_id = true;
            Some(id)
        } else {
            None
        };

        self.keyword("fn")?;
        let (name, name_span) = self.ident("function name")?;
        let stable_id = stable_id.unwrap_or_else(|| format!("auto:{module}.{name}"));
        self.expect(&TokenKind::LParen, "`(` after function name")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                let (param_name, span) = self.ident("parameter name")?;
                self.expect(&TokenKind::Colon, "`:` after parameter name")?;
                let ty = self.ty()?;
                params.push(Param {
                    name: param_name,
                    ty,
                    span,
                });
                if !self.take(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "`)` after parameters")?;
        self.expect(&TokenKind::Arrow, "`->` before return type")?;
        let return_type = self.ty()?;

        let effects = if self.at_keyword("uses") {
            self.bump();
            self.effect_set()?
        } else {
            Vec::new()
        };
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        loop {
            if self.at_keyword("requires") {
                self.bump();
                requires.push(self.expression(0)?);
            } else if self.at_keyword("ensures") {
                self.bump();
                ensures.push(self.expression(0)?);
            } else {
                break;
            }
        }
        self.expect(&TokenKind::LBrace, "`{` before function body")?;
        let body = self.expression(0)?;
        self.take(&TokenKind::Semicolon);
        let end = self
            .expect(&TokenKind::RBrace, "`}` after function body")?
            .span;
        Ok(Function {
            stable_id,
            explicit_id,
            name,
            name_span,
            params,
            return_type,
            effects,
            requires,
            ensures,
            body,
            span: start.merge(end),
        })
    }

    fn expression(&mut self, minimum_precedence: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.prefix()?;
        loop {
            let Some(op) = self.binary_op() else { break };
            let precedence = op.precedence();
            if precedence < minimum_precedence {
                break;
            }
            self.bump();
            let right = self.expression(precedence + 1)?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        let mut expression = match token.kind {
            TokenKind::Int(value) => Expr {
                kind: ExprKind::Int(value),
                span: token.span,
            },
            TokenKind::Ident(value) if value == "true" || value == "false" => Expr {
                kind: ExprKind::Bool(value == "true"),
                span: token.span,
            },
            TokenKind::Ident(value) => Expr {
                kind: ExprKind::Var(value),
                span: token.span,
            },
            TokenKind::Minus | TokenKind::Bang => {
                let op = if matches!(token.kind, TokenKind::Minus) {
                    UnaryOp::Neg
                } else {
                    UnaryOp::Not
                };
                let value = self.expression(7)?;
                let span = token.span.merge(value.span);
                Expr {
                    kind: ExprKind::Unary {
                        op,
                        value: Box::new(value),
                    },
                    span,
                }
            }
            TokenKind::LParen => {
                let inner = self.expression(0)?;
                let end = self
                    .expect(&TokenKind::RParen, "`)` after expression")?
                    .span;
                Expr {
                    kind: inner.kind,
                    span: token.span.merge(end),
                }
            }
            _ => {
                return Err(
                    Diagnostic::error("SPX-P201", "expected an expression", token.span)
                        .at_path(&self.path),
                );
            }
        };

        if self.take(&TokenKind::LParen) {
            let ExprKind::Var(name) = expression.kind else {
                return Err(self.error_previous("SPX-P202", "only named functions can be called"));
            };
            let mut args = Vec::new();
            if !self.at(&TokenKind::RParen) {
                loop {
                    args.push(self.expression(0)?);
                    if !self.take(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            let end = self
                .expect(&TokenKind::RParen, "`)` after call arguments")?
                .span;
            expression = Expr {
                kind: ExprKind::Call { name, args },
                span: expression.span.merge(end),
            };
        }
        Ok(expression)
    }

    fn effect_set(&mut self) -> Result<Vec<String>, Diagnostic> {
        self.expect(&TokenKind::LBrace, "`{` before effect set")?;
        let mut effects = Vec::new();
        if !self.at(&TokenKind::RBrace) {
            loop {
                let (effect, _) = self.ident("effect name")?;
                effects.push(effect);
                if !self.take(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RBrace, "`}` after effect set")?;
        Ok(effects)
    }

    fn ty(&mut self) -> Result<Type, Diagnostic> {
        let (name, span) = self.ident("type")?;
        match name.as_str() {
            "i64" => Ok(Type::I64),
            "bool" => Ok(Type::Bool),
            _ => Err(Diagnostic::error(
                "SPX-T001",
                format!("unknown type `{name}`; v0.1 supports i64 and bool"),
                span,
            )
            .at_path(&self.path)),
        }
    }

    fn binary_op(&self) -> Option<BinaryOp> {
        Some(match self.current().kind {
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Rem,
            TokenKind::EqEq => BinaryOp::Eq,
            TokenKind::BangEq => BinaryOp::Ne,
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::Le => BinaryOp::Le,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::Ge => BinaryOp::Ge,
            TokenKind::AndAnd => BinaryOp::And,
            TokenKind::OrOr => BinaryOp::Or,
            _ => return None,
        })
    }

    fn keyword(&mut self, expected: &str) -> Result<Token, Diagnostic> {
        if self.at_keyword(expected) {
            Ok(self.bump().clone())
        } else {
            Err(self.error_here("SPX-P104", format!("expected `{expected}`")))
        }
    }

    fn ident(&mut self, description: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok((value, token.span)),
            _ => Err(
                Diagnostic::error("SPX-P105", format!("expected {description}"), token.span)
                    .at_path(&self.path),
            ),
        }
    }

    fn expect(&mut self, expected: &TokenKind, description: &str) -> Result<Token, Diagnostic> {
        if self.at(expected) {
            Ok(self.bump().clone())
        } else {
            Err(self.error_here("SPX-P106", format!("expected {description}")))
        }
    }

    fn take(&mut self, expected: &TokenKind) -> bool {
        if self.at(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn at(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(expected)
    }

    fn at_keyword(&self, expected: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(value) if value == expected)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn bump(&mut self) -> &Token {
        let index = self.cursor;
        if !matches!(self.tokens[index].kind, TokenKind::Eof) {
            self.cursor += 1;
        }
        &self.tokens[index]
    }

    fn error_here(&self, code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, message, self.current().span).at_path(&self.path)
    }

    fn error_previous(&self, code: &'static str, message: impl Into<String>) -> Diagnostic {
        let index = self.cursor.saturating_sub(1);
        Diagnostic::error(code, message, self.tokens[index].span).at_path(&self.path)
    }
}
