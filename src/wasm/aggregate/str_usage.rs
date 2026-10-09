//! String-view helper import census over the complete expression tree.
use super::*;

fn expression_uses_str_ops(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Invoke { callable, args } => {
            expression_uses_str_ops(callable) || args.iter().any(expression_uses_str_ops)
        }
        ResolvedExprKind::LiteralFormat { args, .. } => args.iter().any(expression_uses_str_ops),
        ResolvedExprKind::Call { callee, args, .. } => {
            crate::str_ops::by_id(callee.as_str()).is_some_and(|op| {
                matches!(
                    op,
                    crate::str_ops::StrOp::StartsWith | crate::str_ops::StrOp::Contains
                )
            }) || matches!(
                crate::string_ops::by_id(callee.as_str()),
                Some(
                    crate::string_ops::StringOp::StartsWith | crate::string_ops::StringOp::Contains
                )
            ) || args.iter().any(expression_uses_str_ops)
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(expression_uses_str_ops)
        }
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().any(expression_uses_str_ops),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            expression_uses_str_ops(source)
                || expression_uses_str_ops(start)
                || expression_uses_str_ops(end)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => expression_uses_str_ops(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_uses_str_ops(left) || expression_uses_str_ops(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count())
                    .any(|index| statement.child(index).is_some_and(expression_uses_str_ops))
            }) || expression_uses_str_ops(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_uses_str_ops(condition)
                || expression_uses_str_ops(then_branch)
                || expression_uses_str_ops(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_uses_str_ops(&field.value)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            expression_uses_str_ops(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_deref().is_some_and(expression_uses_str_ops)
                        || expression_uses_str_ops(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_uses_str_ops(base)
                || fields
                    .iter()
                    .any(|field| expression_uses_str_ops(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::FunctionReference { .. } => false,
        ResolvedExprKind::Closure { captures, .. } => captures
            .iter()
            .any(|capture| expression_uses_str_ops(&capture.value)),
    }
}

pub(super) fn program_uses_str_ops(program: &ResolvedProgram) -> bool {
    program.functions.iter().any(|function| {
        expression_uses_str_ops(&function.body)
            || function.requires.iter().any(expression_uses_str_ops)
            || function.ensures.iter().any(expression_uses_str_ops)
    })
}
