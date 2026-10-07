//! Condition-only, allocation-free inspection of named String owners.
//!
//! HIR keeps the reserved String operation and each owner's ordinary Place
//! type/mode. This derived set changes only the admitted synchronous borrowed
//! reads: they neither create clones nor supply owned transfer sources. Full
//! HIR validation separately authenticates binding availability and types.

use std::collections::BTreeSet;

use crate::ast::{Expr, ExprKind};
use crate::hir::{
    ExpressionId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedStatement, ResolvedType,
};

pub(crate) fn source_named_read(expression: &Expr) -> bool {
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return false;
    };
    let Some(op) = super::by_name(name) else {
        return false;
    };
    is_condition_read(op)
        && type_arguments.is_empty()
        && args.len() == op.arity()
        && args.iter().enumerate().all(|(index, argument)| {
            op.param_types().get(index) == Some(&ResolvedType::String)
                && op.param_ownership(index) == OwnershipMode::Borrow
                && matches!(argument.kind, ExprKind::Var(_))
        })
        && matches!(op.return_type(), ResolvedType::I64 | ResolvedType::Bool)
}

fn is_condition_read(op: super::StringOp) -> bool {
    matches!(
        op,
        super::StringOp::Len
            | super::StringOp::IsEmpty
            | super::StringOp::StartsWith
            | super::StringOp::Contains
    )
}

fn resolved_operands(expression: &ResolvedExpr) -> Option<&[ResolvedExpr]> {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &expression.kind
    else {
        return None;
    };
    let op = super::by_id(callee.as_str())?;
    if !is_condition_read(op)
        || !type_arguments.is_empty()
        || instance.is_some()
        || expression.ty != op.return_type()
        || expression.ownership != OwnershipMode::Value
        || args.len() != op.arity()
    {
        return None;
    }
    args.iter()
        .enumerate()
        .all(|(index, argument)| {
            op.param_ownership(index) == OwnershipMode::Borrow
                && argument.ty == ResolvedType::String
                && argument.ownership == OwnershipMode::Own
                && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
        })
        .then_some(args)
}

pub(crate) fn condition_reads(condition: &ResolvedExpr) -> BTreeSet<ExpressionId> {
    let mut reads = BTreeSet::new();
    let mut pending = vec![condition];
    while let Some(expression) = pending.pop() {
        if let Some(arguments) = resolved_operands(expression) {
            reads.extend(arguments.iter().map(|argument| argument.id.clone()));
            continue;
        }
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            pending.push(tail);
            for statement in statements {
                if let ResolvedStatement::While { condition, .. } = statement {
                    pending.push(condition);
                } else {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
        } else {
            pending.extend(crate::interpreter::trace_child_expressions(expression));
        }
    }
    reads
}

pub(crate) fn function_reads(function: &ResolvedFunction) -> BTreeSet<ExpressionId> {
    let mut reads = BTreeSet::new();
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::While { condition, .. } = statement {
                    reads.extend(condition_reads(condition));
                }
            }
        }
        pending.extend(crate::interpreter::trace_child_expressions(expression));
    }
    reads
}
