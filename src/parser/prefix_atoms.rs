//! Recursive prefix builders have separate frames from inactive atom arms.
use super::*;
impl Parser {
    #[inline(never)]
    pub(super) fn literal_atom(&self, token: Token) -> Result<Expr, Diagnostic> {
        let expression = match token.kind {
            TokenKind::Int(value) => Expr {
                kind: ExprKind::Int(value),
                span: token.span,
            },
            TokenKind::Int32(value) => Expr {
                kind: ExprKind::Int32(value),
                span: token.span,
            },
            TokenKind::Char(value) => Expr {
                kind: ExprKind::Char(value),
                span: token.span,
            },
            TokenKind::Uint8(value) => Expr {
                kind: ExprKind::Uint8(value),
                span: token.span,
            },
            TokenKind::Usize(value) => Expr {
                kind: ExprKind::Usize(value),
                span: token.span,
            },
            TokenKind::String(value) => Expr {
                kind: ExprKind::String(value),
                span: token.span,
            },
            TokenKind::Float(literal) => Expr {
                kind: if literal.wide {
                    ExprKind::Float64(literal.value.to_bits())
                } else {
                    ExprKind::Float32((literal.value as f32).to_bits())
                },
                span: token.span,
            },
            _ => return Err(self.prefix_atom_rejection(&token)),
        };
        Ok(expression)
    }
    #[inline(never)]
    pub(super) fn identifier_atom(
        &mut self,
        value: String,
        span: Span,
        allow_record_literals: bool,
    ) -> Result<Expr, Diagnostic> {
        match value.as_str() {
            "str" if self.adjacent_text_literal(span) => Ok(self.static_text_literal(span)),
            "true" | "false" => Ok(Expr {
                kind: ExprKind::Bool(value == "true"),
                span,
            }),
            "fn" => self.closure_expression(span),
            "if" => self.if_expression(span),
            "match" => self.match_expression(span),
            "yield" => self.yield_expression(span, allow_record_literals),
            _ => self.ident_or_own_closure(value, span),
        }
    }
    #[inline(never)]
    pub(super) fn unary_atom(
        &mut self,
        token: Token,
        allow_record_literals: bool,
    ) -> Result<Expr, Diagnostic> {
        let mut ops: Vec<(UnaryOp, crate::ast::Span)> = Vec::new();
        let mut current_token = token;
        loop {
            let op = if matches!(current_token.kind, TokenKind::Minus) {
                UnaryOp::Neg
            } else {
                UnaryOp::Not
            };
            if op == UnaryOp::Neg && self.at_signed_minimum() {
                return self.signed_minimum_literal(current_token.span);
            }
            ops.push((op, current_token.span));
            if self.at(&TokenKind::Minus) || self.at(&TokenKind::Bang) {
                current_token = self.bump().clone();
            } else {
                break;
            }
        }
        let mut value = self.expression_with_record_literals(7, allow_record_literals)?;
        for (op, span) in ops.into_iter().rev() {
            let new_span = span.merge(value.span);
            if op == UnaryOp::Neg && matches!(value.kind, ExprKind::Usize(_)) {
                return Err(Diagnostic::error(
                    "SPX-T260",
                    "usize literals cannot be negative",
                    new_span,
                )
                .at_path(&self.path));
            }
            value = Expr {
                kind: ExprKind::Unary {
                    op,
                    value: Box::new(value),
                },
                span: new_span,
            };
        }
        Ok(value)
    }
    #[inline(never)]
    pub(super) fn parenthesized_atom(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        let inner = self.expression(0)?;
        if let Some(diagnostic) = self.tuple_literal() {
            return Err(diagnostic);
        }
        let end = self
            .expect(&TokenKind::RParen, "`)` after expression")?
            .span;
        Ok(Expr {
            kind: inner.kind,
            span: start.merge(end),
        })
    }
}
