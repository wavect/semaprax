//! Owned Bounded Byte Buffer v1 source admission.
//!
//! One buffer is exactly one write-once chain expression:
//!
//! ```text
//! bytes_set(bytes_set(bytes_zeroed(2usize), 0usize, 65u8), 1usize, 66u8)
//! ```
//!
//! The capacity is a `usize` literal at the single `bytes_zeroed` allocation
//! site, every `bytes_set` index is a `usize` literal strictly below it, and a
//! `bytes_set` buffer operand is syntactically the previous link. Nothing in
//! the chain is nameable, so a partially filled buffer has no observable
//! state, no second owner, and no borrowed view; binding the chain's result is
//! the freeze, after which only the borrowed reads apply.
//!
//! Capacity exhaustion and an out-of-range element index are therefore
//! compile-time diagnostics rather than backend accidents.

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
                None => diagnostics.push(
                    error(
                        program,
                        "SPX-T272",
                        format!("`{name}` requires one usize literal element index"),
                        args[1].span,
                    )
                    .with_help("write the index as a usize literal, for example `0usize`"),
                ),
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
