//! Closed expression-child order for cleanup replay.
use super::*;

pub(super) fn replay_expression_child(
    expression: &ResolvedExpr,
    index: usize,
) -> Option<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::FunctionReference { .. } => None,
        ResolvedExprKind::Invoke { args, .. } => args.get(index),
        ResolvedExprKind::Call { args, .. } => args.get(index),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
        ResolvedExprKind::HostCommandCall(call) => call.args.get(index),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => [source.as_ref(), start.as_ref(), end.as_ref()]
            .get(index)
            .copied(),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => (index == 0).then_some(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            [left.as_ref(), right.as_ref()].get(index).copied()
        }
        ResolvedExprKind::Block { statements, tail } => {
            let mut offset = 0;
            for statement in statements {
                let count = statement.child_count();
                if index < offset + count {
                    return statement.child(index - offset);
                }
                offset += count;
            }
            (index == offset).then_some(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => [
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ]
        .get(index)
        .copied(),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.get(index).map(|field| &field.value)
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                Some(scrutinee.as_ref())
            } else {
                // Refutable Match v1: each arm contributes its optional
                // guard first, then its value.
                let mut cursor = index - 1;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        if cursor == 0 {
                            return Some(guard.as_ref());
                        }
                        cursor -= 1;
                    }
                    if cursor == 0 {
                        return Some(&arm.value);
                    }
                    cursor -= 1;
                }
                None
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => (index == 0)
            .then_some(base.as_ref())
            .or_else(|| fields.get(index - 1).map(|field| &field.value)),
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
        | ResolvedExprKind::BorrowPlace { .. } => None,
    }
}
