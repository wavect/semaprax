//! Checked arithmetic scratch discovery for ordinary Core-Wasm lowering.
//!
//! This is separate from byte emission so every profile reserves the same
//! locals from one structural HIR traversal.

use crate::ast::{BinaryOp, UnaryOp};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedType};

/// Whether a function body or contract contains i32 arithmetic that needs the
/// reserved i64 scratch pair.
pub(super) fn needs_i32_wide_scratch(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::Unary { op, value } => {
                if *op == UnaryOp::Neg && value.ty == ResolvedType::I32 {
                    return true;
                }
                pending.push(value);
            }
            ResolvedExprKind::Binary { op, left, right } => {
                if matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) && left.ty == ResolvedType::I32
                {
                    return true;
                }
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Closure { captures, .. } => {
                pending.extend(captures.iter().map(|capture| &capture.value))
            }
            ResolvedExprKind::Call { args, .. } => pending.extend(args.iter()),
            ResolvedExprKind::Invoke { callable, args } => {
                pending.push(callable);
                pending.extend(args.iter());
            }
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter()),
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. }
            | ResolvedExprKind::UpdateRecord { fields, .. } => {
                for field in fields {
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard.as_ref());
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                pending.push(operand);
            }
            ResolvedExprKind::Project { base, .. } => pending.push(base),
            ResolvedExprKind::Upcast { source } => pending.push(source),
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
            | ResolvedExprKind::FunctionReference { .. } => {}
        }
    }
    false
}

/// Whether an expression contains checked u8 arithmetic that needs the
/// function-level scratch locals.
pub(super) fn contains_u8_arithmetic(expression: &ResolvedExpr) -> bool {
    contains_checked_arithmetic(expression, &ResolvedType::U8)
}

pub(super) fn contains_usize_arithmetic(expression: &ResolvedExpr) -> bool {
    contains_checked_arithmetic(expression, &ResolvedType::Usize)
}

fn contains_checked_arithmetic(expression: &ResolvedExpr, target: &ResolvedType) -> bool {
    match &expression.kind {
        ResolvedExprKind::Binary { op, left, right: _ }
            if left.ty == *target
                && matches!(
                    *op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) =>
        {
            true
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            contains_checked_arithmetic(left, target) || contains_checked_arithmetic(right, target)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => contains_checked_arithmetic(value, target),
        ResolvedExprKind::Call { args, .. } => args
            .iter()
            .any(|argument| contains_checked_arithmetic(argument, target)),
        ResolvedExprKind::Invoke { callable, args } => {
            contains_checked_arithmetic(callable, target)
                || args
                    .iter()
                    .any(|argument| contains_checked_arithmetic(argument, target))
        }
        ResolvedExprKind::NativeRustImportCall(call) => call
            .args
            .iter()
            .any(|argument| contains_checked_arithmetic(argument, target)),
        ResolvedExprKind::HostCommandCall(call) => call
            .args
            .iter()
            .any(|argument| contains_checked_arithmetic(argument, target)),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            contains_checked_arithmetic(source, target)
                || contains_checked_arithmetic(start, target)
                || contains_checked_arithmetic(end, target)
        }
        ResolvedExprKind::Block { statements, tail } => {
            contains_checked_arithmetic(tail, target)
                || statements.iter().any(|statement| {
                    (0..statement.child_count()).any(|index| {
                        statement
                            .child(index)
                            .is_some_and(|child| contains_checked_arithmetic(child, target))
                    })
                })
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            contains_checked_arithmetic(condition, target)
                || contains_checked_arithmetic(then_branch, target)
                || contains_checked_arithmetic(else_branch, target)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| contains_checked_arithmetic(&field.value, target)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            contains_checked_arithmetic(scrutinee, target)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| contains_checked_arithmetic(guard, target))
                        || contains_checked_arithmetic(&arm.value, target)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            contains_checked_arithmetic(base, target)
                || fields
                    .iter()
                    .any(|field| contains_checked_arithmetic(&field.value, target))
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
            .any(|capture| contains_checked_arithmetic(&capture.value, target)),
    }
}
