//! Retained expression lookup for cleanup/control-flow execution.
use super::*;

pub(super) fn find_expression_by<'a>(
    expression: &'a hir::ResolvedExpr,
    predicate: &impl Fn(&hir::ResolvedExpr) -> bool,
) -> Option<&'a hir::ResolvedExpr> {
    if predicate(expression) {
        return Some(expression);
    }
    match &expression.kind {
        hir::ResolvedExprKind::Closure { captures, .. } => captures
            .iter()
            .find_map(|capture| find_expression_by(&capture.value, predicate)),
        hir::ResolvedExprKind::FunctionReference { .. } => None,
        hir::ResolvedExprKind::Invoke { callable, args } => find_expression_by(callable, predicate)
            .or_else(|| args.iter().find_map(|a| find_expression_by(a, predicate))),
        hir::ResolvedExprKind::Call { args, .. } => args
            .iter()
            .find_map(|argument| find_expression_by(argument, predicate)),
        hir::ResolvedExprKind::NativeRustImportCall(call) => call
            .args
            .iter()
            .find_map(|argument| find_expression_by(argument, predicate)),
        hir::ResolvedExprKind::HostCommandCall(call) => call
            .args
            .iter()
            .find_map(|argument| find_expression_by(argument, predicate)),
        hir::ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => find_expression_by(source, predicate)
            .or_else(|| find_expression_by(start, predicate))
            .or_else(|| find_expression_by(end, predicate)),
        hir::ResolvedExprKind::Unary { value, .. }
        | hir::ResolvedExprKind::Project { base: value, .. }
        | hir::ResolvedExprKind::Try { operand: value, .. }
        | hir::ResolvedExprKind::TryOption { operand: value, .. }
        | hir::ResolvedExprKind::Upcast { source: value } => find_expression_by(value, predicate),
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            find_expression_by(left, predicate).or_else(|| find_expression_by(right, predicate))
        }
        hir::ResolvedExprKind::Block { statements, tail } => statements
            .iter()
            .find_map(|statement| {
                (0..statement.child_count()).find_map(|index| {
                    statement
                        .child(index)
                        .and_then(|child| find_expression_by(child, predicate))
                })
            })
            .or_else(|| find_expression_by(tail, predicate)),
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => find_expression_by(condition, predicate)
            .or_else(|| find_expression_by(then_branch, predicate))
            .or_else(|| find_expression_by(else_branch, predicate)),
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .find_map(|field| find_expression_by(&field.value, predicate)),
        hir::ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => find_expression_by(scrutinee, predicate).or_else(|| {
            arms.iter()
                .find_map(|arm| find_expression_by(&arm.value, predicate))
        }),
        hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            find_expression_by(base, predicate).or_else(|| {
                fields
                    .iter()
                    .find_map(|field| find_expression_by(&field.value, predicate))
            })
        }
        hir::ResolvedExprKind::Int(_)
        | hir::ResolvedExprKind::Int32(_)
        | hir::ResolvedExprKind::Char(_)
        | hir::ResolvedExprKind::Uint8(_)
        | hir::ResolvedExprKind::Usize(_)
        | hir::ResolvedExprKind::ArrayU8(_)
        | hir::ResolvedExprKind::RepeatArrayU8 { .. }
        | hir::ResolvedExprKind::Float32(_)
        | hir::ResolvedExprKind::Float64(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::String(_)
        | hir::ResolvedExprKind::Place(_)
        | hir::ResolvedExprKind::BorrowPlace { .. } => None,
    }
}
