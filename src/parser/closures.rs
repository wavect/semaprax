//! Typed anonymous function syntax. Eligibility belongs to source verification.
use super::*;

impl Parser {
    pub(super) fn closure_expression(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        self.expect(&TokenKind::LParen, "`(` after anonymous `fn`")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                let (name, span) = self.ident("closure parameter name")?;
                self.expect(&TokenKind::Colon, "`:` after closure parameter name")?;
                let ty = self.ty()?;
                params.push(crate::ast::ClosureParam { name, ty, span });
                if self.at(&TokenKind::RParen) {
                    break;
                }
                self.expect(&TokenKind::Comma, "`,` between closure parameters")?;
                if self.at(&TokenKind::RParen) {
                    return Err(self.error_here(
                        "SPX-P106",
                        "closure parameters do not accept a trailing comma",
                    ));
                }
            }
        }
        self.expect(&TokenKind::RParen, "`)` after closure parameters")?;
        self.expect(&TokenKind::Arrow, "`->` after closure parameters")?;
        let return_type = self.ty()?;
        let body = self.block("closure body")?;
        let span = start.merge(body.span);
        Ok(Expr {
            kind: ExprKind::Closure {
                params,
                return_type,
                body: Box::new(body),
                owning: false,
            },
            span,
        })
    }

    /// Dispatch for a bumped identifier that is not one of `prefix_atom`'s
    /// other reserved leading words: either the start of an owning-capture
    /// closure (`own` immediately followed by `fn`, consumed here in full)
    /// or an ordinary variable reference. Folding this decision into the
    /// existing `Var` catch-all, rather than adding a separate dispatch arm
    /// to the large match in `parser.rs`, keeps that budgeted file's size
    /// unchanged by this addition.
    pub(super) fn ident_or_own_closure(
        &mut self,
        value: String,
        span: Span,
    ) -> Result<Expr, Diagnostic> {
        if value == "own" && self.at_keyword("fn") {
            return self.own_closure(span);
        }
        Ok(Expr {
            kind: ExprKind::Var(value),
            span,
        })
    }

    /// SPX-AI-021 bounded owning-capture closure: `own fn() -> R { body }`.
    /// The `own` keyword is already consumed by the caller; this consumes
    /// `fn` onward. Zero explicit parameters in this bounded profile: the
    /// body is checked (in `source_verify`) as one call transferring exactly
    /// one lexical owned `Bytes` capture. See `docs/CLOSURES-OWNING-V1.md`.
    pub(super) fn own_closure(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        self.keyword("fn")?;
        self.expect(&TokenKind::LParen, "`(` after `own fn`")?;
        if !self.at(&TokenKind::RParen) {
            return Err(self.error_here(
                "SPX-P130",
                "owning-capture closures admit no explicit parameters in this bounded profile",
            ));
        }
        self.expect(&TokenKind::RParen, "`)` after `own fn(`")?;
        self.expect(&TokenKind::Arrow, "`->` after `own fn()`")?;
        let return_type = self.ty()?;
        let body = self.block("owning closure body")?;
        let span = start.merge(body.span);
        Ok(Expr {
            kind: ExprKind::Closure {
                params: Vec::new(),
                return_type,
                body: Box::new(body),
                owning: true,
            },
            span,
        })
    }
}
