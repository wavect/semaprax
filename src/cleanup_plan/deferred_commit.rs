//! Which compiler-owned call defers its owner commit past the status split.
//!
//! Most calls transfer their owned arguments at the commit boundary and then
//! split on the propagated status; after that transition even a nonzero status
//! cannot restore them, because the callee owns them. Two compiler-owned
//! operations instead check a bound over arguments the caller has already
//! staged, and must fail *before* the transfer: `vec_push` at full capacity,
//! `vec_reserve_exact` beyond the bounded capacity, `vec_set` outside the
//! initialized length, and `bytes_set`/`bytes_set5`/`bytes_set1_or5_from_slice` outside their buffer interval. Deferring their
//! commit to the success branch leaves the owner in its canonical call-argument
//! slot, so ordinary region cleanup destroys it exactly once and no backend has
//! to invent a destruction of its own.

use crate::hir::DeclarationId;

/// The bounded Vec operation this callee names, and whether the call defers its
/// owner commit until the propagated status is known to be zero.
pub(super) fn call_behavior(
    expression: &crate::hir::ResolvedExpr,
) -> (Option<crate::vec_ops::VecOp>, bool) {
    if matches!(&expression.kind, crate::hir::ResolvedExprKind::HostCommandCall(call) if call.operation == crate::hir::ResolvedHostCommandOperation::StdinStreamNext)
    {
        return (None, true);
    }
    let crate::hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        ..
    } = &expression.kind
    else {
        return (None, false);
    };
    let op = crate::vec_ops::by_id(callee.as_str());
    let deferred = matches!(
        op,
        Some(
            crate::vec_ops::VecOp::Push
                | crate::vec_ops::VecOp::ReserveExact
                | crate::vec_ops::VecOp::Set
        )
    ) || callee.as_str() == crate::iterator_ops::NEXT_ID
        || (callee.as_str() == crate::iterator_ops::INTO_ITER_ID
            && matches!(type_arguments.as_slice(), [crate::hir::ResolvedType::Bytes]))
        || is_fallible_byte_operation(callee)
        || (callee.as_str() == crate::box_ops::NEW_ID
            && matches!(type_arguments.as_slice(), [crate::hir::ResolvedType::Bytes]))
        || is_map_reopen(callee);
    (op, deferred)
}

/// String Collections v1 `map_add`/`map_set` check capacity and overflow
/// before they take the staged map, so a failed call leaves it in its
/// call-argument slot for ordinary region cleanup.
pub(super) fn is_map_reopen(callee: &DeclarationId) -> bool {
    crate::map_ops::by_id(callee.as_str()).is_some_and(|op| op.reopens())
        || crate::string_ops::by_id(callee.as_str())
            .is_some_and(crate::string_ops::StringOp::reopens_map)
}

/// `true` for a byte operation that is total after HIR admission and therefore
/// publishes no propagated status at all.
pub(super) fn is_total_byte_operation(callee: &DeclarationId) -> bool {
    crate::byte_ops::by_id(callee.as_str()).is_some_and(|op| !op.is_fallible())
}

pub(super) fn is_infallible_vec_operation(op: Option<crate::vec_ops::VecOp>) -> bool {
    matches!(
        op,
        Some(
            crate::vec_ops::VecOp::Len
                | crate::vec_ops::VecOp::Capacity
                | crate::vec_ops::VecOp::Clear
                | crate::vec_ops::VecOp::Sort
        )
    )
}

pub(super) fn is_infallible_box_operation(callee: &DeclarationId) -> bool {
    matches!(
        crate::box_ops::by_id(callee.as_str()),
        Some(crate::box_ops::BoxOp::Get | crate::box_ops::BoxOp::IntoInner)
    )
}

pub(super) fn expression_is_infallible_compiler_operation(
    expression: &crate::hir::ResolvedExpr,
) -> bool {
    matches!(
        &expression.kind,
        crate::hir::ResolvedExprKind::Call { callee, instance: None, .. }
            if crate::byte_ops::by_id(callee.as_str()).is_some_and(|op| !op.is_fallible())
                || crate::host_io_ops::by_id(callee.as_str()).is_some()
                || is_infallible_vec_operation(crate::vec_ops::by_id(callee.as_str()))
                || is_infallible_box_operation(callee)
                || callee.as_str() == crate::stdin_stream_ops::EOF_ID
    )
}

fn is_fallible_byte_operation(callee: &DeclarationId) -> bool {
    crate::byte_ops::by_id(callee.as_str()).is_some_and(crate::byte_ops::ByteOp::is_fallible)
}
