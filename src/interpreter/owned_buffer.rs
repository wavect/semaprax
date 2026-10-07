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
pub(super) fn set(
    mut buffer: OwnedBytesValue,
    index: u64,
    byte: u8,
) -> Result<OwnedBytesValue, Flow> {
    let Some(slot) = usize::try_from(index)
        .ok()
        .filter(|slot| *slot < buffer.bytes.len())
    else {
        return Err(Flow::Failure(normalize_byte_buffer(
            crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
        )));
    };
    let filled = Arc::make_mut(&mut buffer.bytes);
    filled[slot] = byte;
    Ok(buffer)
}

/// Store five consecutive bytes after one all-or-nothing bounds preflight.
///
/// The preflight deliberately happens before cloning or writing, so this is
/// observationally the same as five successful `bytes_set` calls while a
/// failed store selects the existing operation status without publishing a
/// partially updated owner.
pub(super) fn set5(
    mut buffer: OwnedBytesValue,
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
    let filled = Arc::make_mut(&mut buffer.bytes);
    filled[slot..slot + 5].copy_from_slice(&bytes);
    Ok(buffer)
}

/// Store either one supplied byte or five ordered bytes read from a borrowed
/// slice. The selector's high bit chooses the wide path; its remaining bits
/// give the source offset. The wide source read is total: each missing source
/// position supplies zero after the destination interval has been preflighted.
pub(super) fn set1_or5(
    mut buffer: OwnedBytesValue,
    index: u64,
    one: u8,
    source: &[u8],
    selector: u64,
) -> Result<OwnedBytesValue, Flow> {
    let wide = selector & crate::byte_ops::SET1_OR5_WIDE_TAG != 0;
    let source_start = selector & !crate::byte_ops::SET1_OR5_WIDE_TAG;
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
    let filled = Arc::make_mut(&mut buffer.bytes);
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
    Ok(buffer)
}

/// Tagged source store. The copy branch preflights its complete six- or
/// forty-eight-byte destination interval before the owner is moved.
pub(super) fn set1_or6_or48(
    mut buffer: OwnedBytesValue,
    index: u64,
    one: u8,
    source: &[u8],
    selector: u64,
) -> Result<OwnedBytesValue, Flow> {
    let copy = selector & crate::byte_ops::SET1_OR6_OR48_COPY_TAG != 0;
    let wide48 = selector & crate::byte_ops::SET1_OR6_OR48_WIDE48_TAG != 0;
    let source_start = selector & crate::byte_ops::SET1_OR6_OR48_OFFSET_MASK;
    let width = if copy {
        if wide48 {
            48
        } else {
            6
        }
    } else {
        1
    };
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
    let filled = Arc::make_mut(&mut buffer.bytes);
    if copy {
        let start = usize::try_from(source_start).ok();
        for offset in 0..width {
            filled[slot + offset] = start
                .and_then(|start| start.checked_add(offset))
                .and_then(|at| source.get(at))
                .copied()
                .unwrap_or(0);
        }
    } else {
        filled[slot] = one;
    }
    Ok(buffer)
}

/// Evaluate one compiler-owned owned-buffer operation after the caller has
/// evaluated every operand from left to right.
impl Evaluator<'_> {
    pub(super) fn evaluate_owned_buffer_operation(
        &mut self,
        op: crate::byte_ops::ByteOp,
        mut values: Vec<Value>,
    ) -> Result<Value, Flow> {
        // Remove the transferred owner only after all operands have evaluated.
        // No extra strong reference is retained on the unique store path.
        let owner = if op != crate::byte_ops::ByteOp::Zeroed {
            match values.first_mut() {
                Some(value @ Value::Bytes(_)) => Some(std::mem::replace(value, Value::Moved)),
                _ => None,
            }
        } else {
            None
        };
        match (op, values.as_slice()) {
            (crate::byte_ops::ByteOp::Zeroed, [Value::Usize(capacity)]) => zeroed(
                *capacity,
                &mut self.next_byte_allocation,
                &mut self.allocated_byte_payload,
            )
            .map(Value::Bytes),
            (
                crate::byte_ops::ByteOp::Set,
                [Value::Moved, Value::Usize(index), Value::Uint8(byte)],
            ) => set(take_buffer(owner)?, *index, *byte).map(Value::Bytes),
            (
                crate::byte_ops::ByteOp::Set5,
                [Value::Moved, Value::Usize(index), Value::Uint8(first), Value::Uint8(second), Value::Uint8(third), Value::Uint8(fourth), Value::Uint8(fifth)],
            ) => set5(
                take_buffer(owner)?,
                *index,
                [*first, *second, *third, *fourth, *fifth],
            )
            .map(Value::Bytes),
            (
                crate::byte_ops::ByteOp::Set1Or5,
                [Value::Moved, Value::Usize(index), Value::Uint8(one), Value::BorrowedSlice(source), Value::Usize(selector)],
            ) => set1_or5(take_buffer(owner)?, *index, *one, source.bytes(), *selector)
                .map(Value::Bytes),
            (
                crate::byte_ops::ByteOp::Set1Or6Or48,
                [Value::Moved, Value::Usize(index), Value::Uint8(one), Value::BorrowedSlice(source), Value::Usize(selector)],
            ) => set1_or6_or48(take_buffer(owner)?, *index, *one, source.bytes(), *selector)
                .map(Value::Bytes),
            _ => Err(Flow::Guard("ill-typed borrowed byte operation operand")),
        }
    }
}

fn take_buffer(owner: Option<Value>) -> Result<OwnedBytesValue, Flow> {
    match owner {
        Some(Value::Bytes(buffer)) => Ok(buffer),
        _ => Err(Flow::Guard(
            "owned buffer operation lost its transferred owner",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(capacity: usize) -> OwnedBytesValue {
        OwnedBytesValue {
            allocation: 1,
            bytes: Arc::from(vec![0; capacity]),
        }
    }

    #[test]
    fn sg05_unique_stores_retain_backing_allocation_at_every_capacity() {
        for capacity in [1, 4096, 65536, 131072] {
            let mut owner = buffer(capacity);
            let pointer = owner.bytes.as_ptr();
            for byte in 0..64 {
                owner = set(owner, 0, byte).unwrap();
                assert_eq!(owner.bytes.as_ptr(), pointer);
                assert_eq!(owner.allocation, 1);
            }
            assert_eq!(owner.bytes[0], 63);
        }
        assert!(matches!(set(buffer(0), 0, 1), Err(Flow::Failure(_))));
        assert!(matches!(set(buffer(1), u64::MAX, 1), Err(Flow::Failure(_))));
    }

    #[test]
    fn sg05_wide_and_tagged_stores_preserve_unique_backing_and_shared_snapshots() {
        let mut owner = buffer(64);
        let pointer = owner.bytes.as_ptr();
        owner = set5(owner, 0, [1, 2, 3, 4, 5]).unwrap();
        owner = set1_or5(owner, 5, 0, &[6; 5], crate::byte_ops::SET1_OR5_WIDE_TAG).unwrap();
        owner = set1_or6_or48(
            owner,
            10,
            0,
            &[7; 48],
            crate::byte_ops::SET1_OR6_OR48_COPY_TAG | crate::byte_ops::SET1_OR6_OR48_WIDE48_TAG,
        )
        .unwrap();
        assert_eq!(owner.bytes.as_ptr(), pointer);
        assert_eq!(&owner.bytes[..5], &[1, 2, 3, 4, 5]);
        assert_eq!(&owner.bytes[5..10], &[6; 5]);
        assert_eq!(&owner.bytes[10..58], &[7; 48]);
        let snapshot = owner.bytes.clone();
        owner = set(owner, 0, 99).unwrap();
        assert_eq!(snapshot[0], 1);
        assert_eq!(owner.bytes[0], 99);
        assert_ne!(owner.bytes.as_ptr(), snapshot.as_ptr());
        let snapshot = owner.bytes.clone();
        assert!(matches!(set5(owner, 63, [0; 5]), Err(Flow::Failure(_))));
        assert_eq!(snapshot[63], 0);
    }
}
