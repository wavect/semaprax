//! Statement `if`: `if <condition> { <statements> }`, optionally followed by
//! `else if` and `else` branches, in statement position.
//!
//! This is parse-level sugar with an unchanged canonical form, like `else if`.
//! A statement `if` lowers to the value discard agents had to spell by hand,
//! `let _if1 = if <condition> { <statements>; 0 } else { 0 };`. The binding
//! is an ordinary immutable `i64` local, because a block admits no shadowing:
//! each discard takes the next `_if<n>` name that no identifier in the file
//! already spells, so the canonical text parses back to the same program. A branch that
//! ends without a value receives the `i64` tail `0`, and a missing `else`
//! becomes `else { 0 }`, so resolution, both verifiers, cleanup plans, the
//! semantic graph, the interpreter, and native and Wasm code see exactly the
//! value `if` they already admit. A branch that does end with a value keeps an
//! integer literal tail as is and otherwise discards it with `let _if<n> = <value>;`
//! before the `0`, so branches never disagree on the discarded type.
//!
//! A chain in which every branch ends with a value and a final `else` exists
//! is an ordinary value `if`: it stays the block's final expression when `}`,
//! `;`, or an operator follows it, and is discarded as a statement only when
//! another statement follows. The `if` is parsed once; no input that parsed
//! before changes meaning.

use std::collections::BTreeSet;

use crate::ast::{Expr, ExprKind, Span, Statement};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

/// The `_if<n>` discard names handed out while parsing one file.
#[derive(Default)]
pub(super) struct DiscardNames {
    next: usize,
    /// Every identifier the file spells, gathered on first use.
    taken: Option<BTreeSet<String>>,
}

/// What an `if` at the start of a block item turned out to be.
pub(super) enum IfItem {
    /// A statement `if`, already lowered to `let _if<n> = if …;`.
    Statement(Statement),
    /// A value `if` that is (the start of) the block's final expression.
    Value(Expr),
}

/// A branch body before the chain decides whether it carries a value.
struct Branch {
    statements: Vec<Statement>,
    tail: Option<Expr>,
    span: Span,
}

struct Arm {
    start: Span,
    condition: Expr,
    body: Branch,
}

impl Parser {
    pub(super) fn statement_if(&mut self) -> Result<IfItem, Diagnostic> {
        let start = self.keyword("if")?.span;
        let mut arms = Vec::new();
        let mut otherwise = None;
        let mut arm_start = start;
        loop {
            let condition = self.expression_with_record_literals(0, false)?;
            let body = self.branch("`if` condition")?;
            arms.push(Arm {
                start: arm_start,
                condition,
                body,
            });
            if !self.at_keyword("else") {
                break;
            }
            self.bump();
            if self.at_keyword("if") {
                arm_start = self.bump().span;
                continue;
            }
            otherwise = Some(self.branch("`else`")?);
            break;
        }
        let valued = otherwise
            .as_ref()
            .is_some_and(|branch| branch.tail.is_some())
            && arms.iter().all(|arm| arm.body.tail.is_some());
        let expression = self.assemble(arms, otherwise, valued);
        if valued {
            let before = self.cursor;
            let expression = self.postfix(expression, true)?;
            let expression = self.binary_continuation(expression, 0, true)?;
            if self.cursor != before
                || self.at(&TokenKind::RBrace)
                || self.at(&TokenKind::Semicolon)
            {
                return Ok(IfItem::Value(expression));
            }
            return Ok(IfItem::Statement(self.discard(start, expression, None)));
        }
        let end = self
            .take(&TokenKind::Semicolon)
            .then(|| self.previous_span());
        Ok(IfItem::Statement(self.discard(start, expression, end)))
    }

    fn branch(&mut self, description: &str) -> Result<Branch, Diagnostic> {
        let start = self
            .expect(&TokenKind::LBrace, &format!("`{{` before {description}"))?
            .span;
        let (statements, tail, end) = self
            .block_contents(true)
            .map_err(|diagnostic| Self::attach_block_help(diagnostic, description))?;
        Ok(Branch {
            statements,
            tail,
            span: start.merge(end),
        })
    }

    /// A statement `if` is the last item of a block that needs a value. The
    /// value grammar reports what that `if` lacks (an `else`, or a branch
    /// value), so the diagnostic is the one this input always produced.
    pub(super) fn valueless_tail_if(&mut self, checkpoint: usize) -> Diagnostic {
        self.cursor = checkpoint;
        match self.expression(0) {
            Err(diagnostic) => diagnostic,
            Ok(_) => self.error_here("SPX-P203", "block requires a final value expression"),
        }
    }

    fn assemble(&mut self, arms: Vec<Arm>, otherwise: Option<Branch>, valued: bool) -> Expr {
        let mut alternative = match otherwise {
            Some(branch) => self.finish(branch, valued),
            None => {
                let last = arms.last().map_or_else(Span::default, |arm| arm.body.span);
                let end = point(last);
                Expr {
                    kind: ExprKind::Block {
                        statements: Vec::new(),
                        tail: Box::new(zero(end)),
                    },
                    span: end,
                }
            }
        };
        let count = arms.len();
        for (index, arm) in arms.into_iter().rev().enumerate() {
            let then_branch = self.finish(arm.body, valued);
            let span = arm.start.merge(alternative.span);
            let conditional = Expr {
                kind: ExprKind::If {
                    condition: Box::new(arm.condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(alternative),
                },
                span,
            };
            // `else if` is the nested `else { if … }` block, exactly as the value
            // grammar builds it.
            alternative = if index + 1 == count {
                conditional
            } else {
                Expr {
                    span: conditional.span,
                    kind: ExprKind::Block {
                        statements: Vec::new(),
                        tail: Box::new(conditional),
                    },
                }
            };
        }
        alternative
    }

    fn finish(&mut self, branch: Branch, valued: bool) -> Expr {
        let Branch {
            mut statements,
            tail,
            span,
        } = branch;
        let end = point(span);
        let tail = match tail {
            Some(tail) if valued || matches!(tail.kind, ExprKind::Int(_)) => tail,
            Some(value) => {
                statements.push(self.discard(value.span, value, None));
                zero(end)
            }
            None => zero(end),
        };
        Expr {
            kind: ExprKind::Block {
                statements,
                tail: Box::new(tail),
            },
            span,
        }
    }

    fn discard(&mut self, start: Span, value: Expr, end: Option<Span>) -> Statement {
        let span = start.merge(end.unwrap_or(value.span));
        Statement::Let {
            name: self.discard_name(),
            name_span: start,
            mutable: false,
            declared: None,
            value,
            span,
        }
    }

    fn discard_name(&mut self) -> String {
        let tokens = &self.tokens;
        let taken = self.discards.taken.get_or_insert_with(|| {
            tokens
                .iter()
                .filter_map(|token| match &token.kind {
                    TokenKind::Ident(name) => Some(name.clone()),
                    _ => None,
                })
                .collect()
        });
        loop {
            self.discards.next += 1;
            let name = format!("_if{}", self.discards.next);
            if taken.insert(name.clone()) {
                return name;
            }
        }
    }
}

/// The `0` a valueless branch yields, placed at the branch's closing brace.
fn zero(span: Span) -> Expr {
    Expr {
        kind: ExprKind::Int(0),
        span,
    }
}

/// The last byte of `span`, where a synthesised value is reported.
fn point(span: Span) -> Span {
    Span {
        start: span.end.saturating_sub(1).max(span.start),
        ..span
    }
}
