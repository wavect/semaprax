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
            },
            span,
        })
    }
}
