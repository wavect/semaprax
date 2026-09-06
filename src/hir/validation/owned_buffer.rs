//! Owned Bounded Byte Buffer v1 hostile-HIR authentication.
//!
//! The source verifier admits one write-once chain expression; this trust
//! boundary re-derives the same fact from resolved HIR alone, so a graph,
//! patch, or transaction that never passed through source text cannot forge a
//! buffer whose capacity is unknown, whose element index is out of range, or
//! whose operand is a second owner of an already-frozen buffer.

use super::*;

/// Authenticate one `bytes_zeroed` or `bytes_set` call. The caller has already
/// checked monomorphism and arity.
pub(super) fn require_admitted_chain(
    op: crate::byte_ops::ByteOp,
    args: &[ResolvedExpr],
) -> Result<(), Diagnostic> {
    match op {
        crate::byte_ops::ByteOp::Zeroed => {
            chain_capacity(args)?;
            Ok(())
        }
        crate::byte_ops::ByteOp::Set => {
            let capacity = chain_capacity(args)?;
            let ResolvedExprKind::Usize(index) = &args[1].kind else {
                return Err(hir_error(
                    "owned byte buffer element index must be one usize literal",
                ));
            };
            if *index >= capacity {
                return Err(hir_error(format!(
                    "owned byte buffer element index {index} is outside the capacity {capacity}"
                )));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The capacity the argument list's buffer position resolves to. For
/// `bytes_zeroed` that is its own literal operand; for `bytes_set` it is the
/// capacity of the chain its first operand must syntactically be.
fn chain_capacity(args: &[ResolvedExpr]) -> Result<u64, Diagnostic> {
    let mut links = 0usize;
    let mut current = &args[0];
    loop {
        if let ResolvedExprKind::Usize(capacity) = &current.kind {
            // The `bytes_zeroed` allocation site itself: its own operand is the
            // literal capacity rather than a previous chain link.
            return (*capacity <= crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES)
                .then_some(*capacity)
                .ok_or_else(|| {
                    hir_error(format!(
                        "owned byte buffer capacity exceeds the admitted {} bytes",
                        crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES
                    ))
                });
        }
        let ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args: inner,
        } = &current.kind
        else {
            return Err(hir_error(
                "owned byte buffer operand must be its write-once chain's previous link",
            ));
        };
        let op = instance
            .is_none()
            .then(|| crate::byte_ops::by_id(callee.as_str()))
            .flatten()
            .filter(|op| op.is_owned_buffer_chain())
            .ok_or_else(|| {
                hir_error("owned byte buffer operand must be its write-once chain's previous link")
            })?;
        if !type_arguments.is_empty() || inner.len() != op.arity() {
            return Err(hir_error("owned byte buffer chain link has a forged shape"));
        }
        if op == crate::byte_ops::ByteOp::Set {
            links += 1;
            if links > crate::byte_ops::MAX_BUFFER_FILL_SITES {
                return Err(hir_error(
                    "owned byte buffer fill exceeds the admitted element count",
                ));
            }
        }
        current = &inner[0];
    }
}
