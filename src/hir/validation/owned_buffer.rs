//! Owned Bounded Byte Buffer v1 hostile-HIR authentication.
//!
//! The source verifier admits one write-once chain expression; this trust
//! boundary re-derives the same fact from resolved HIR alone, so a graph,
//! patch, or transaction that never passed through source text cannot forge a
//! buffer whose capacity is unknown, whose element index can never name an
//! element, or whose operand is a second owner of an already-frozen buffer. A
//! computed index is admitted on both projections and checked at run time.

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
            if args[1].ty != crate::hir::ResolvedType::Usize {
                return Err(hir_error(
                    "owned byte buffer element index is not a usize expression",
                ));
            }
            // A computed index is admitted and checked at run time against the
            // transferred buffer. An index that can never name an element is
            // still refused here, so a graph or patch route cannot smuggle in
            // a store the source verifier would reject.
            match &args[1].kind {
                ResolvedExprKind::Usize(index) if *index >= capacity => Err(hir_error(format!(
                    "owned byte buffer element index {index} is outside the capacity {capacity}"
                ))),
                ResolvedExprKind::Usize(_) => Ok(()),
                _ if capacity == 0 => Err(hir_error(
                    "owned byte buffer element index cannot name an element of an empty buffer",
                )),
                _ => Ok(()),
            }
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
