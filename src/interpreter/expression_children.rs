//! Deterministic direct-child enumeration shared by interpreter analyses.

use crate::hir::{ResolvedExpr, ResolvedExprKind};

pub(super) fn child_expressions(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.iter().collect(),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().collect(),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().collect(),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => vec![value.as_ref()],
        ResolvedExprKind::Binary { left, right, .. } => vec![left.as_ref(), right.as_ref()],
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => vec![source.as_ref(), start.as_ref(), end.as_ref()],
        ResolvedExprKind::Block { statements, tail } => {
            let mut collected = Vec::new();
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collected.push(child);
                    }
                }
            }
            collected.push(tail.as_ref());
            collected
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ],
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().map(|field| &field.value).collect()
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            let mut collected = vec![scrutinee.as_ref()];
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collected.push(guard.as_ref());
                }
                collected.push(&arm.value);
            }
            collected
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            let mut collected = vec![base.as_ref()];
            collected.extend(fields.iter().map(|field| &field.value));
            collected
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
        | ResolvedExprKind::BorrowPlace { .. } => Vec::new(),
    }
}
