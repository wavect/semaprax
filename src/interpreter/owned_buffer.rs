//! Owned Bounded Byte Buffer v1 evaluation.
//!
//! `bytes_zeroed` is the only allocating step. It charges the same site count
//! and payload sum as `bytes_copy`, because the verified profile counts one
//! owned byte allocation family rather than two. `bytes_set` charges nothing:
//! it receives the single live owner, stores one byte, and returns that same
//! logical allocation identity, so a fill can never grow the accounting or
//! create a second owner.

use std::sync::Arc;

use crate::conformance::{NormalizedStatus, Retryability, StatusClass};

use super::{Evaluator, Flow, OwnedBytesValue, Value};

/// The single Owned Bounded Byte Buffer v1 runtime failure. A computed
/// `bytes_set` index at or above the transferred buffer's length selects this
/// status before the store, so the buffer is never partially written.
fn normalize_byte_buffer(code: u32) -> NormalizedStatus {
    NormalizedStatus::try_new(
        crate::byte_ops::SET_STATUS_DOMAIN,
        code,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("compiler-owned owned byte buffer status table is valid")
}

/// Charge one allocation site and return the zeroed buffer it allocates.
pub(super) fn zeroed(
    capacity: u64,
    next_byte_allocation: &mut u32,
    allocated_byte_payload: &mut u64,
) -> Result<OwnedBytesValue, Flow> {
    if capacity > crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES {
        return Err(Flow::Guard(
            "owned byte buffer capacity exceeds the verified profile limit",
        ));
    }
    let length = usize::try_from(capacity)
        .map_err(|_| Flow::Guard("owned byte buffer capacity does not fit usize"))?;
    let next_count = next_byte_allocation
        .checked_add(1)
        .ok_or(Flow::Guard("owned byte allocation count overflowed"))?;
    if next_count > crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
        return Err(Flow::Guard(
            "owned byte allocation count exceeds verified capacity",
        ));
    }
    let next_payload = allocated_byte_payload
        .checked_add(capacity)
        .ok_or(Flow::Guard("owned byte payload accounting overflowed"))?;
    if next_payload > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES {
        return Err(Flow::Guard("owned byte payload exceeds verified capacity"));
    }
    *next_byte_allocation = next_count;
    *allocated_byte_payload = next_payload;
    Ok(OwnedBytesValue {
        allocation: next_count,
        bytes: Arc::from(vec![0u8; length].as_slice()),
    })
}

/// Store one byte into the transferred owner and hand that owner back.
///
/// The element index is any admitted `usize` expression, so the bound is
/// checked here rather than assumed. An index at or above the transferred
/// buffer's length selects the single `semaprax.byte-buffer.v1` failure before
/// any byte is written, which is the same predicate and the same normalized
/// status the native and Core-Wasm backends select.
pub(super) fn set(buffer: &OwnedBytesValue, index: u64, byte: u8) -> Result<OwnedBytesValue, Flow> {
    let Some(slot) = usize::try_from(index)
        .ok()
        .filter(|slot| *slot < buffer.bytes.len())
    else {
        return Err(Flow::Failure(normalize_byte_buffer(
            crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
        )));
    };
    let mut filled = buffer.bytes.to_vec();
    filled[slot] = byte;
    Ok(OwnedBytesValue {
        allocation: buffer.allocation,
        bytes: Arc::from(filled.as_slice()),
    })
}

/// Store five consecutive bytes after one all-or-nothing bounds preflight.
///
/// The preflight deliberately happens before cloning or writing, so this is
/// observationally the same as five successful `bytes_set` calls while a
/// failed store selects the existing operation status without publishing a
/// partially updated owner.
pub(super) fn set5(
    buffer: &OwnedBytesValue,
    index: u64,
    bytes: [u8; 5],
) -> Result<OwnedBytesValue, Flow> {
    let Some(slot) = usize::try_from(index).ok().filter(|slot| {
        buffer
            .bytes
            .len()
            .checked_sub(*slot)
            .is_some_and(|remaining| remaining >= 5)
    }) else {
        return Err(Flow::Failure(normalize_byte_buffer(
            crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
        )));
    };
    let mut filled = buffer.bytes.to_vec();
    filled[slot..slot + 5].copy_from_slice(&bytes);
    Ok(OwnedBytesValue {
        allocation: buffer.allocation,
        bytes: Arc::from(filled.as_slice()),
    })
}

/// Store either one supplied byte or five ordered bytes read from a borrowed
/// slice. The wide source read is total: each missing source position supplies
/// zero after the destination interval has been preflighted.
pub(super) fn set1_or5(
    buffer: &OwnedBytesValue,
    index: u64,
    wide: bool,
    one: u8,
    source: &[u8],
    source_start: u64,
) -> Result<OwnedBytesValue, Flow> {
    let width = if wide { 5 } else { 1 };
    let Some(slot) = usize::try_from(index).ok().filter(|slot| {
        buffer
            .bytes
            .len()
            .checked_sub(*slot)
            .is_some_and(|remaining| remaining >= width)
    }) else {
        return Err(Flow::Failure(normalize_byte_buffer(
            crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
        )));
    };
    let mut filled = buffer.bytes.to_vec();
    if wide {
        let start = usize::try_from(source_start).ok();
        for offset in 0..5 {
            filled[slot + offset] = start
                .and_then(|start| start.checked_add(offset))
                .and_then(|source_index| source.get(source_index))
                .copied()
                .unwrap_or(0);
        }
    } else {
        filled[slot] = one;
    }
    Ok(OwnedBytesValue {
        allocation: buffer.allocation,
        bytes: Arc::from(filled.as_slice()),
    })
}

/// Evaluate one compiler-owned owned-buffer operation after the caller has
/// evaluated every operand from left to right.
impl Evaluator<'_> {
    pub(super) fn evaluate_owned_buffer_operation(
        &mut self,
        op: crate::byte_ops::ByteOp,
        values: &[Value],
    ) -> Result<Value, Flow> {
        match (op, values) {
            (crate::byte_ops::ByteOp::Zeroed, [Value::Usize(capacity)]) => zeroed(
                *capacity,
                &mut self.next_byte_allocation,
                &mut self.allocated_byte_payload,
            )
            .map(Value::Bytes),
            (
                crate::byte_ops::ByteOp::Set,
                [Value::Bytes(buffer), Value::Usize(index), Value::Uint8(byte)],
            ) => set(buffer, *index, *byte).map(Value::Bytes),
            (
                crate::byte_ops::ByteOp::Set5,
                [Value::Bytes(buffer), Value::Usize(index), Value::Uint8(first), Value::Uint8(second), Value::Uint8(third), Value::Uint8(fourth), Value::Uint8(fifth)],
            ) => set5(buffer, *index, [*first, *second, *third, *fourth, *fifth]).map(Value::Bytes),
            (
                crate::byte_ops::ByteOp::Set1Or5,
                [Value::Bytes(buffer), Value::Usize(index), Value::Bool(wide), Value::Uint8(one), Value::BorrowedSlice(source), Value::Usize(source_start)],
            ) => set1_or5(buffer, *index, *wide, *one, source.bytes(), *source_start)
                .map(Value::Bytes),
            _ => Err(Flow::Guard("ill-typed borrowed byte operation operand")),
        }
    }
}
