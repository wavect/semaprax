//! Graph-side expression traversal seam (REF-11).
//!
//! Every graph probe declares which scope it walks. The scopes differ only in
//! who owns a closure's deferred body:
//!
//! | Scope | Closure captures | Closure body | Match guards | Users |
//! | --- | --- | --- | --- | --- |
//! | [`ClosureBodies::Deferred`] (current function) | yes | no | yes | `visit_expr_calls`, `visit_expr_call_instances`, `expression_has_command_append` |
//! | [`ClosureBodies::Structural`] (feature discovery) | yes | yes | yes | `expression_has_while`, `expression_has_stdout_write` |
//!
//! A deferred closure body is not evaluated by the function that constructs
//! the closure: its exact execution edges belong to the derived closure body
//! graph, so current-function call collectors exclude it, exactly like
//! `hir::push_resolved_expression_children_in_authored_order`, whose
//! behavior this seam reuses and does not change. Structural feature probes
//! discover source meaning anywhere in a declaration, so they include it.
//!
//! Children are visited iteratively in authored order: scrutinee, then each
//! arm's guard before its value; captures before a structural closure body;
//! callable before arguments; block statements before the tail. Function
//! references, generic-instance metadata, and pattern or type facts are node
//! data, not expression children: each collector reads them from the node it
//! visits. A walk never evaluates an effect or invokes a closure.
//!
//! Probes that still recurse by hand (`expression_has_usize`,
//! `expression_has_byte_range`, `expression_has_command_io`,
//! `expression_has_explicit_match_mode`, `expression_has_record_pattern`,
//! `expression_has_refutable_match` and the type collectors) are structural
//! and already visit guards and closure bodies; they are not migrated here.

use super::*;

/// Who owns a closure's deferred body during one traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClosureBodies {
    /// Current-function scope: captures are evaluated at construction; the
    /// deferred body is not part of this function's evaluation.
    Deferred,
    /// Structural discovery: the body is part of the declaration's source.
    Structural,
}

/// Push `expression`'s children so that popping yields authored order.
pub(super) fn push_children<'a>(
    expression: &'a ResolvedExpr,
    closure_bodies: ClosureBodies,
    pending: &mut Vec<&'a ResolvedExpr>,
) {
    // Exhaustive on purpose: a new expression variant must decide here
    // whether it owns a deferred body before any migrated probe compiles.
    match &expression.kind {
        ResolvedExprKind::Closure { body, .. } => {
            if closure_bodies == ClosureBodies::Structural {
                // Pushed beneath the captures, so it pops after them.
                pending.push(body);
            }
        }
        ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::Invoke { .. }
        | ResolvedExprKind::Int(_)
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
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::ByteRange { .. }
        | ResolvedExprKind::Call { .. }
        | ResolvedExprKind::NativeRustImportCall(_)
        | ResolvedExprKind::HostCommandCall(_)
        | ResolvedExprKind::Unary { .. }
        | ResolvedExprKind::Binary { .. }
        | ResolvedExprKind::Block { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::Yield { .. } => {}
    }
    hir::push_resolved_expression_children_in_authored_order(expression, pending);
}

/// Visit `root` and its descendants in authored preorder without recursion.
pub(super) fn for_each_preorder<'a>(
    root: &'a ResolvedExpr,
    closure_bodies: ClosureBodies,
    mut visit: impl FnMut(&'a ResolvedExpr),
) {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        visit(expression);
        push_children(expression, closure_bodies, &mut pending);
    }
}

/// Presence scan: stops at the first expression satisfying `predicate`.
pub(super) fn any_expression(
    root: &ResolvedExpr,
    closure_bodies: ClosureBodies,
    mut predicate: impl FnMut(&ResolvedExpr) -> bool,
) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if predicate(expression) {
            return true;
        }
        push_children(expression, closure_bodies, &mut pending);
    }
    false
}
