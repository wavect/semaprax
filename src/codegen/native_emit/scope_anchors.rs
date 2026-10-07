//! Lexical scope-exit anchors for native blocks.
//!
//! A block selects its canonical CleanupPlan region through anchor storage:
//! its owned `let` bindings and owned statement values. Owned String Loops v1
//! and Text Toolkit v1 also allocate String temporaries nested inside a
//! loop-body statement (a clone for `string_len(text)`, a literal, a call
//! result consumed by a borrowing operation). Those belong to the same body
//! region, which must settle at the end of every iteration, so a while body
//! also anchors them. Nested blocks and match arms own their own regions and
//! are not entered.

use std::collections::BTreeSet;

use super::is_direct_plan_owned;
use crate::cleanup_plan::StorageId;
use crate::hir::{
    ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedStatement,
};

pub(super) fn block_anchors(
    program: &ResolvedProgram,
    plan: &crate::codegen::native_bytes::NativeBytesPlan,
    block: &ResolvedExpr,
    loop_body: bool,
) -> BTreeSet<StorageId> {
    let ResolvedExprKind::Block { statements, tail } = &block.kind else {
        return BTreeSet::new();
    };
    let owned = |storage: &StorageId, ty: &crate::hir::ResolvedType| {
        is_direct_plan_owned(program, ty) || plan.has_projected_leaves(storage)
    };
    let mut anchors = BTreeSet::new();
    for statement in statements {
        if let ResolvedStatement::Let { binding, .. } = statement {
            let storage = StorageId::Value(binding.id.clone());
            if owned(&storage, &binding.ty) {
                anchors.insert(storage);
            }
        }
        let value = match statement {
            ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => {
                Some(value)
            }
            ResolvedStatement::Unsafe { body, .. } => Some(body.as_ref()),
            ResolvedStatement::While { .. } => None,
        };
        if let Some(value) = value {
            let storage = StorageId::Temporary(value.id.clone());
            if owned(&storage, &value.ty) {
                anchors.insert(storage);
            }
            if loop_body {
                nested_temporaries(value, plan, &mut anchors);
            }
        }
    }
    if loop_body {
        nested_temporaries(tail, plan, &mut anchors);
    }
    anchors
}

/// Finalizable temporaries of `root` and its descendants in the same
/// lexical region.
fn nested_temporaries(
    root: &ResolvedExpr,
    plan: &crate::codegen::native_bytes::NativeBytesPlan,
    anchors: &mut BTreeSet<StorageId>,
) {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        let storage = StorageId::Temporary(expression.id.clone());
        if plan.is_region_slot(&storage) {
            anchors.insert(storage);
        }
        match &expression.kind {
            ResolvedExprKind::Block { .. } | ResolvedExprKind::Closure { .. } => {}
            ResolvedExprKind::If { condition, .. } => pending.push(condition),
            ResolvedExprKind::Match { scrutinee, .. } => pending.push(scrutinee),
            _ => pending.extend(crate::interpreter::trace_child_expressions(expression)),
        }
    }
}

/// Every `while` body in `function`, including `for` loops, which resolve to
/// the same statement.
pub(super) fn while_bodies(function: &crate::hir::ResolvedFunction) -> BTreeSet<ExpressionId> {
    let mut bodies = BTreeSet::new();
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::While { body, .. } = statement {
                    bodies.insert(body.id.clone());
                }
            }
        }
        pending.extend(crate::interpreter::trace_child_expressions(expression));
    }
    bodies
}
