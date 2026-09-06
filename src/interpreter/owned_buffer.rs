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

use super::{Flow, OwnedBytesValue};

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
