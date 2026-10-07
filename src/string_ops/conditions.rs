//! Condition-only, allocation-free inspection of a named String owner.
//!
//! HIR keeps the reserved `string_len` call and the owner's ordinary Place
//! type/mode. This derived set changes only that call's synchronous borrowed
//! inspection: it neither creates a clone nor supplies an owned transfer source.
//! Full HIR validation separately authenticates binding availability and type.

use std::collections::BTreeSet;

use crate::ast::{Expr, ExprKind};
use crate::hir::{
    ExpressionId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedStatement, ResolvedType,
};

pub(crate) fn source_named_length(expression: &Expr) -> bool {
    matches!(&expression.kind, ExprKind::Call { name, type_arguments, args }
        if super::by_name(name) == Some(super::StringOp::Len)
            && type_arguments.is_empty() && args.len() == 1
            && matches!(args[0].kind, ExprKind::Var(_)))
}

fn resolved_operand(expression: &ResolvedExpr) -> Option<&ResolvedExpr> {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &expression.kind
    else {
        return None;
    };
    if callee.as_str() != super::LEN_ID
        || !type_arguments.is_empty()
        || instance.is_some()
        || expression.ty != ResolvedType::I64
        || expression.ownership != OwnershipMode::Value
        || args.len() != 1
    {
        return None;
    }
    let argument = &args[0];
    (argument.ty == ResolvedType::String
        && argument.ownership == OwnershipMode::Own
        && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
    .then_some(argument)
}

pub(crate) fn condition_reads(condition: &ResolvedExpr) -> BTreeSet<ExpressionId> {
    let mut reads = BTreeSet::new();
    let mut pending = vec![condition];
    while let Some(expression) = pending.pop() {
        if let Some(argument) = resolved_operand(expression) {
            reads.insert(argument.id.clone());
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
