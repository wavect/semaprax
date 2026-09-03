//! Iterative unsafe-boundary scan shared by both HIR validation passes.

use super::*;

/// `true` when the resolved expression tree contains an unsafe boundary
/// statement anywhere inside its blocks, branches, arms, or nested bodies.
pub(super) fn contains_unsafe_boundary(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements.iter().rev() {
                    if matches!(statement, ResolvedStatement::Unsafe { .. }) {
                        return true;
                    }
                    for index in (0..statement.child_count()).rev() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
            ResolvedExprKind::Call { args, .. } => pending.extend(args.iter().rev()),
            ResolvedExprKind::NativeRustImportCall(call) => {
                pending.extend(call.args.iter().rev());
            }
            ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter().rev()),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                pending.push(end);
                pending.push(start);
                pending.push(source);
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().rev().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.extend(arms.iter().rev().map(|arm| &arm.value));
                pending.push(scrutinee);
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.extend(fields.iter().rev().map(|field| &field.value));
                pending.push(base);
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }
    false
}
