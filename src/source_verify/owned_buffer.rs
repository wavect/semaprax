//! Owned Bounded Byte Buffer v1 source admission.
//!
//! One buffer is exactly one write-once chain expression:
//!
//! ```text
//! bytes_set(bytes_set(bytes_zeroed(2usize), 0usize, 65u8), 1usize, 66u8)
//! ```
//!
//! The capacity is a `usize` literal at the single `bytes_zeroed` allocation
//! site and a `bytes_set` buffer operand is syntactically the previous link.
//! Nothing in the chain is nameable, so a partially filled buffer has no
//! observable state, no second owner, and no borrowed view; binding the
//! chain's result is the freeze, after which only the borrowed reads apply.
//!
//! One shape re-opens a frozen buffer: the same-owner replacement
//! `buffer = bytes_set(buffer, index, value)`. The call moves the single owner
//! out of the binding and the assignment publishes the returned owner back
//! into it, so exactly one generation is live at every point. Inside one
//! bounded `while` that is the loop-carried fill. The allocation stays
//! outside the loop.
//!
//! A `bytes_set` element index is any `usize` expression. A literal index at
//! or above the capacity, and any index into an empty buffer, remain
//! compile-time diagnostics; a computed index is checked against the buffer
//! at run time, before the owner transfer commits, and selects the single
//! `semaprax.byte-buffer.v1` failure identically on every backend.

use crate::ast::{Expr, ExprKind, Program};
use crate::byte_ops::{self, ByteOp};
use crate::diagnostic::Diagnostic;

use super::diagnostics::error;

/// Check one `bytes_zeroed` or `bytes_set` call site. The caller has already
/// rejected type arguments and a wrong argument count.
pub(super) fn check_call(
    program: &Program,
    expression: &Expr,
    op: ByteOp,
    name: &str,
    args: &[Expr],
    reopen: bool,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if args.len() != op.arity() {
        return diagnostics;
    }
    match op {
        ByteOp::Zeroed => {
            if byte_ops::owned_buffer_chain_capacity(expression).is_none() {
                diagnostics.push(
                    error(
                        program,
                        "SPX-T271",
                        format!(
                            "`{name}` requires one usize literal capacity of at most {} bytes",
                            byte_ops::MAX_BUFFER_CAPACITY_BYTES
                        ),
                        args[0].span,
                    )
                    .with_help(
                        "write the capacity as a usize literal, for example `bytes_zeroed(4usize)`",
                    ),
                );
            }
        }
        ByteOp::Set => {
            // Same-owner re-open: this exact call is the right-hand side of
            // `buffer = bytes_set(buffer, index, value)`, so its buffer operand
            // is that binding. The capacity is not a literal at this call site,
            // so the established run-time element bound is the only index rule
            // that can apply; it is selected before the owner transfer commits
            // and is exact on every backend.
            if reopen && byte_ops::owned_buffer_operand_is_binding(&args[0]) {
                return diagnostics;
            }
            let Some(capacity) = byte_ops::owned_buffer_chain_capacity(&args[0]) else {
                diagnostics.push(
                    error(
                        program,
                        "SPX-T271",
                        format!(
                            "`{name}` buffer operand must be the enclosing write-once chain's previous `{}` or `{}` call",
                            byte_ops::ZEROED_NAME,
                            byte_ops::SET_NAME
                        ),
                        args[0].span,
                    )
                    .with_help(
                        "build the whole buffer in one expression, then bind the result to freeze it",
                    ),
                );
                return diagnostics;
            };
            // A computed `usize` index is admitted: the store is checked
            // against the buffer at run time on every backend, and selects the
            // one `semaprax.byte-buffer.v1` failure before the owner commits.
            // A literal that is already outside the capacity, and every index
            // into an empty buffer, stay compile-time diagnostics, because
            // neither can ever name an element.
            match byte_ops::owned_buffer_set_index(args) {
                Some(index) if index < capacity => {}
                Some(index) => diagnostics.push(
                    error(
                        program,
                        "SPX-T272",
                        format!("`{name}` index {index} is outside the buffer capacity {capacity}"),
                        args[1].span,
                    )
                    .with_help("index an element below the allocated capacity"),
                ),
                None if capacity == 0 => diagnostics.push(
                    error(
                        program,
                        "SPX-T272",
                        format!("`{name}` cannot index any element of an empty buffer"),
                        args[1].span,
                    )
                    .with_help("allocate the buffer with a capacity above zero"),
                ),
                None => {}
            }
        }
        ByteOp::Len
        | ByteOp::Get
        | ByteOp::Range
        | ByteOp::Copy
        | ByteOp::BytesAsSlice
        | ByteOp::ArrayAsSlice
        | ByteOp::StrAsBytes
        | ByteOp::StringAsStr => {}
    }
    diagnostics
}

/// The payload one `bytes_zeroed` allocation site contributes to the
/// target-neutral owned byte capacity analysis. Admitted source always carries
/// the literal capacity its admission rule requires; an unadmitted site falls
/// back to the conservative whole-array extent so a rejected program is never
/// also under-charged.
pub(super) fn allocation_payload_bytes(args: &[Expr]) -> u64 {
    match args.first().map(|argument| &argument.kind) {
        Some(ExprKind::Usize(capacity)) => {
            (*capacity).min(crate::byte_data_capacity::MAX_ARRAY_BYTES)
        }
        _ => crate::byte_data_capacity::MAX_ARRAY_BYTES,
    }
}
