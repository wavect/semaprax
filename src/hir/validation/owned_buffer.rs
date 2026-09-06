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
    reopen: bool,
) -> Result<(), Diagnostic> {
    match op {
        crate::byte_ops::ByteOp::Zeroed => {
            if chain_capacity(args, false)?.is_none() {
                return Err(hir_error(
                    "owned byte buffer capacity must be a literal at its allocation site",
                ));
            }
            Ok(())
        }
        crate::byte_ops::ByteOp::Set => {
            let capacity = chain_capacity(args, reopen)?;
            if args[1].ty != crate::hir::ResolvedType::Usize {
                return Err(hir_error(
                    "owned byte buffer element index is not a usize expression",
                ));
            }
            // A computed index is admitted and checked at run time against the
            // transferred buffer. An index that can never name an element is
            // still refused here, so a graph or patch route cannot smuggle in
            // a store the source verifier would reject. The loop-carried fill
            // moves one whole binding instead of a literal-capacity chain, so
            // no static capacity is derivable and only the run-time bound
            // applies.
            let Some(capacity) = capacity else {
                return Ok(());
            };
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
fn chain_capacity(args: &[ResolvedExpr], reopen: bool) -> Result<Option<u64>, Diagnostic> {
    let mut links = 0usize;
    let mut current = &args[0];
    loop {
        if let ResolvedExprKind::Usize(capacity) = &current.kind {
            // The `bytes_zeroed` allocation site itself: its own operand is the
            // literal capacity rather than a previous chain link.
            return (*capacity <= crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES)
                .then_some(Some(*capacity))
                .ok_or_else(|| {
                    hir_error(format!(
                        "owned byte buffer capacity exceeds the admitted {} bytes",
                        crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES
                    ))
                });
        }
        // Same-owner re-open: the buffer operand is the whole binding this call
        // moves, and the enclosing assignment republishes the returned owner
        // into that same binding. Ownership, not a literal chain, is what keeps
        // exactly one generation live, and the run-time element bound covers
        // every index.
        if reopen
            && matches!(&current.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
        {
            return Ok(None);
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

/// Authenticate one compiler-owned byte operation inside a bounded `while`.
///
/// Two shapes qualify. The established read-only indexed profile is
/// `byte_len`/`byte_get` over an authenticated borrowed slice alias. The
/// Owned Bounded Byte Buffer v1 loop-carried fill is `bytes_set` whose buffer
/// operand is one whole owned binding the call moves; the enclosing assignment
/// republishes the returned owner into that same binding, so exactly one
/// generation is live at every point. `bytes_zeroed` qualifies for neither, so
/// the allocation stays outside the loop, where the target-neutral owned byte
/// capacity analysis and the fixed Core-Wasm arena can size it.
pub(super) fn require_admitted_while_operation(
    validator: &HirValidator<'_>,
    expression: &ResolvedExpr,
    callee: &DeclarationId,
    operation: crate::byte_ops::ByteOp,
    args: &[ResolvedExpr],
) -> Result<(), Diagnostic> {
    if operation == crate::byte_ops::ByteOp::Set {
        if args.len() != operation.arity()
            || args
                .iter()
                .enumerate()
                .any(|(index, argument)| !operation.accepts_resolved(index, &argument.ty))
            || expression.ty != operation.return_type()
        {
            return Err(hir_error(
                "while loop byte buffer fill is outside Owned Bounded Byte Buffer v1",
            ));
        }
        let buffer = &args[0];
        let ResolvedExprKind::Place(place) = &buffer.kind else {
            return Err(hir_error(
                "while loop byte buffer fill requires one whole owned buffer binding",
            ));
        };
        if !place.projections.is_empty()
            || buffer.ty != ResolvedType::Bytes
            || buffer.ownership != OwnershipMode::Own
        {
            return Err(hir_error(
                "while loop byte buffer fill requires one whole owned buffer binding",
            ));
        }
        return Ok(());
    }
    if !matches!(
        operation,
        crate::byte_ops::ByteOp::Len | crate::byte_ops::ByteOp::Get
    ) || args.len() != operation.arity()
        || args
            .iter()
            .enumerate()
            .any(|(index, argument)| !operation.accepts_resolved(index, &argument.ty))
        || expression.ty != operation.return_type()
        || expression.ownership != OwnershipMode::Value
    {
        return Err(hir_error(format!(
            "while loop byte operation `{callee}` is outside the read-only indexed profile"
        )));
    }
    let slice = &args[0];
    let ResolvedExprKind::Place(place) = &slice.kind else {
        return Err(hir_error(
            "while loop indexed byte reads require an existing byte-slice alias",
        ));
    };
    if slice.ty != ResolvedType::SliceU8
        || slice.ownership != OwnershipMode::Borrow
        || !place.projections.is_empty()
        || (!validator.byte_slice_aliases.contains_key(&place.root)
            && validator
                .program
                .declarations
                .byte_slice_provenance(&place.root)
                .is_none())
    {
        return Err(hir_error(
            "while loop indexed byte read lacks authenticated slice provenance",
        ));
    }
    Ok(())
}
