//! Typed target interfaces for independently replayed lexical control flow.
//! A target identifies meaning, never a backend label or permission to jump.
use super::{ExpressionId, FunctionExecutionId, ResolvedType, ValueId};
pub use crate::ast::LexicalExitKind;

pub const LEXICAL_CONTROL_FLOW_SCHEMA_V1: &str = "semaprax.lexical-control-flow.v1";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ControlTarget {
    Function(FunctionExecutionId),
    Loop {
        owner: FunctionExecutionId,
        /// The exact condition ID of the enclosing authenticated resolved While.
        site: ExpressionId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowCompletion {
    /// A reachable ordinary value; ownership is validated on the expression.
    Normal(ResolvedType),
    /// No normal value exists; the exact target remains on the exit node.
    Exit,
}
impl FlowCompletion {
    /// Both alternatives are independently checked before this type-only join.
    pub fn join(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Exit, Self::Exit) => Some(Self::Exit),
            (Self::Exit, normal @ Self::Normal(_)) | (normal @ Self::Normal(_), Self::Exit) => {
                Some(normal.clone())
            }
            (Self::Normal(left), Self::Normal(right)) if left == right => Some(self.clone()),
            _ => None,
        }
    }
}

/// Source For lowering must preserve the latch, rather than skip its advance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceForKind {
    Copy,
    BorrowRecord,
    OwnedIterator,
    /// Immutable `Slice<u8>` traversal; each item is an authenticated `byte_get` receiver.
    BorrowBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopLatchKind {
    Condition,
    SourceForStep {
        kind: SourceForKind,
        step: ExpressionId,
    },
    /// Only the authenticated Yield-arm tail runs; never re-enter the authored body.
    OwnedIteratorStep {
        next: ExpressionId,
        assignment: ExpressionId,
        step: ValueId,
        remainder: ValueId,
    },
}

mod walk;
mod selected_child;
pub(crate) use selected_child::{requires_inline,validate_owner_inline};
pub(crate) use walk::{child, contains, requires, ChildCursor};

mod owner;
#[cfg(test)]
pub(crate) use owner::validate_monomorphic_owner_with_work;
pub(crate) use owner::{owner, owner_with, validate_owner};
mod owner_funded;
pub(crate) use owner_funded::{owner_funded, requires_funded, OwnerVerdict};

pub(super) mod rekey;

mod latch;
mod latch_inline;
pub(crate) use latch_inline::validate_latch_inline;
pub(crate) use latch::validate_latch;

mod wrapper;
pub(crate) use wrapper::{source_for_wrapper, source_for_wrapper_with, SourceForWrapper};

mod validation;
pub(crate) use validation::{validate_function, validate_function_with};

pub(crate) fn reachable_arm(arms: &[super::ResolvedMatchArm]) -> Option<&super::ResolvedMatchArm> {
    arms.iter().find(|arm| arm.value.ty != ResolvedType::Never)
}
pub(crate) fn normal_arm(arms: &[super::ResolvedMatchArm]) -> Option<&super::ResolvedMatchArm> {
    reachable_arm(arms).or_else(|| arms.first())
}

pub(crate) fn is_loop_statement(statement: &super::ResolvedStatement) -> bool {
    matches!(
        statement,
        super::ResolvedStatement::While { .. } | super::ResolvedStatement::LexicalWhile { .. }
    )
}

pub(crate) mod return_operand;

/// Only function exits can discard ownership state at a loop join. Break and
/// continue still reach that loop's continuation and must retain moved owners.
pub(crate) fn returns_only(expr: &super::ResolvedExpr) -> bool {
    use super::{ResolvedExpr, ResolvedExprKind as K, ResolvedStatement as S};
    const NORMAL: u8 = 1;
    const RETURN: u8 = 2;
    const ESCAPE: u8 = 4;
    const LOCAL: u8 = 8;
    fn sequence(left: u8, right: impl FnOnce() -> u8) -> u8 {
        (left & !NORMAL) | if left & NORMAL != 0 { right() } else { 0 }
    }
    fn visit(expr: &ResolvedExpr, local_loop: Option<&ExpressionId>, depth: usize) -> u8 {
        if depth > crate::cache_codec::MAX_DEPTH {
            return NORMAL | ESCAPE;
        }
        match &expr.kind {
            K::LexicalExit {
                kind: LexicalExitKind::Return,
                ..
            } => RETURN,
            K::LexicalExit { target, .. } => match target {
                ControlTarget::Loop { site, .. } if local_loop == Some(site) => LOCAL,
                _ => ESCAPE,
            },
            K::Block { statements, tail } => {
                let mut paths = NORMAL;
                for statement in statements {
                    paths = sequence(paths, || match statement {
                        S::While {
                            condition, body, ..
                        }
                        | S::LexicalWhile {
                            condition, body, ..
                        } => sequence(visit(condition, local_loop, depth + 1), || {
                            NORMAL
                                | (visit(body, Some(&condition.id), depth + 1) & (RETURN | ESCAPE))
                        }),
                        S::Let { value, .. }
                        | S::Assign { value, .. }
                        | S::CopyDiscard { value, .. } => visit(value, local_loop, depth + 1),
                        S::Unsafe { body, .. } => visit(body, local_loop, depth + 1),
                    });
                }
                sequence(paths, || visit(tail, local_loop, depth + 1))
            }
            K::If {
                condition,
                then_branch,
                else_branch,
            } => sequence(visit(condition, local_loop, depth + 1), || {
                visit(then_branch, local_loop, depth + 1)
                    | visit(else_branch, local_loop, depth + 1)
            }),
            K::Match {
                scrutinee, arms, ..
            } => sequence(visit(scrutinee, local_loop, depth + 1), || {
                if arms.is_empty() {
                    NORMAL
                } else {
                    arms.iter().fold(0, |paths, arm| {
                        paths
                            | sequence(
                                arm.guard
                                    .as_ref()
                                    .map_or(NORMAL, |guard| visit(guard, local_loop, depth + 1)),
                                || visit(&arm.value, local_loop, depth + 1),
                            )
                    })
                }
            }),
            // Typed contour replay independently refuses control crossing value
            // or callable boundaries before this ownership classification.
            _ => NORMAL,
        }
    }
    visit(expr, None, 0) == RETURN
}

#[cfg(test)]
pub(crate) use owner_funded::{owner_funded_with_work, requires_funded_with_work};
