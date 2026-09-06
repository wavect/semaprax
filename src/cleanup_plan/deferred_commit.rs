//! Which compiler-owned call defers its owner commit past the status split.
//!
//! Most calls transfer their owned arguments at the commit boundary and then
//! split on the propagated status; after that transition even a nonzero status
//! cannot restore them, because the callee owns them. Two compiler-owned
//! operations instead check a bound over arguments the caller has already
//! staged, and must fail *before* the transfer: `vec_push` at full capacity,
//! and `bytes_set` with a computed element index outside its buffer. Deferring
//! their commit to the success branch leaves the owner in its canonical
//! call-argument slot, so ordinary region cleanup destroys it exactly once and
//! no backend has to invent a destruction of its own.

use crate::hir::DeclarationId;

/// The bounded Vec operation this callee names, and whether the call defers its
/// owner commit until the propagated status is known to be zero.
pub(super) fn call_behavior(callee: &DeclarationId) -> (Option<crate::vec_ops::VecOp>, bool) {
    let op = crate::vec_ops::by_id(callee.as_str());
    let deferred = op == Some(crate::vec_ops::VecOp::Push) || is_fallible_byte_operation(callee);
    (op, deferred)
}

/// `true` for a byte operation that is total after HIR admission and therefore
/// publishes no propagated status at all.
pub(super) fn is_total_byte_operation(callee: &DeclarationId) -> bool {
    crate::byte_ops::by_id(callee.as_str()).is_some_and(|op| !op.is_fallible())
}

fn is_fallible_byte_operation(callee: &DeclarationId) -> bool {
    crate::byte_ops::by_id(callee.as_str()).is_some_and(crate::byte_ops::ByteOp::is_fallible)
}
