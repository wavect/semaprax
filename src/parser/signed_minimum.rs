//! Signed Minimum Literals v1.
//!
//! `i64::MIN` and `i32::MIN` have no positive magnitude of their own type, so
//! the lexer keeps exactly `MAX + 1` as [`TokenKind::IntMinMagnitude`] and
//! [`TokenKind::Int32MinMagnitude`] instead of rejecting it. Tokenization
//! stays context-free — the lexer never consults a preceding `-`, so
//! subtraction is untouched — and the grammar decides admission here: the
//! magnitude is a literal only as the immediate operand of a unary `-`, and a
//! stable `SPX-P003` rejection everywhere else.
//!
//! Whitespace and comments between the sign and the magnitude are trivia, so
//! `- 9223372036854775808` is the same literal as `-9223372036854775808`. A
//! parenthesis is not trivia: `-(9223372036854775808)` keeps the rejection
//! because the magnitude is then a positive literal in its own right.
//!
//! Folding to an exact literal, rather than to a negation node, keeps `-MIN`
//! and `MIN / -1` selecting checked arithmetic failures at runtime.

use super::*;

impl Parser {
    /// Whether the cursor sits on a signed-minimum magnitude token.
    pub(super) fn at_signed_minimum(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::IntMinMagnitude | TokenKind::Int32MinMagnitude
        )
    }

    /// Consumes the signed-minimum magnitude the cursor sits on and yields the
    /// exact minimum literal it spells under the unary `-` at `sign`.
    ///
    /// Kept out of `prefix_atom` so the folded literal's temporaries live in
    /// this frame rather than on every level of a deep unary chain, where the
    /// nesting limit is reached before the default stack is.
    #[inline(never)]
    pub(super) fn signed_minimum_literal(&mut self, sign: Span) -> Result<Expr, Diagnostic> {
        let kind = match &self.current().kind {
            TokenKind::IntMinMagnitude => ExprKind::Int(i64::MIN),
            TokenKind::Int32MinMagnitude => ExprKind::Int32(i32::MIN),
            _ => unreachable!("the caller checked the magnitude token"),
        };
        let span = sign.merge(self.bump().span);
        Ok(Expr { kind, span })
    }

    /// The rejection for a token that cannot begin an expression. A
    /// signed-minimum magnitude no unary `-` claimed keeps the exact code,
    /// message, and span the lexer reported before the magnitude became its
    /// own token; everything else stays the ordinary `SPX-P201`.
    pub(super) fn prefix_atom_rejection(&self, token: &Token) -> Diagnostic {
        match token.kind {
            TokenKind::IntMinMagnitude | TokenKind::Int32MinMagnitude => {
                out_of_range(token, &self.path)
            }
            _ => Diagnostic::error("SPX-P201", "expected an expression", token.span)
                .at_path(&self.path),
        }
    }
}

/// The stable out-of-range rejection for a signed-minimum magnitude that no
/// unary `-` claimed.
pub(super) fn out_of_range(token: &Token, path: &str) -> Diagnostic {
    let message = if matches!(token.kind, TokenKind::Int32MinMagnitude) {
        "integer literal is outside the i32 range"
    } else {
        "integer literal is outside the i64 range"
    };
    Diagnostic::error("SPX-P003", message, token.span)
        .at_path(path)
        .with_help(
            "the signed minimum is written as a directly negated literal, \
such as `-9223372036854775808`",
        )
}
