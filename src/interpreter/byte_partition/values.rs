//! Exact private Value receivers. Their caller must hold the validated current
//! partition program plus actual owner-root/edge binding; no default installer.
use super::{flat::PartitionVec, loans::Loans, InvocationRuntime, StoreError};
use crate::interpreter::{construction_fusion::Backing, Flow, OwnedBytesValue, Value};

fn refused(_: crate::construction_fusion::domain::Refusal) -> Flow {
    Flow::Guard("ordinary partition receiving authority refused")
}
/// Called at the canonical whole-Vec/iterator or individual Bytes finalizer.
/// It does not recursively guess aggregate field order or settle a borrowed ID.
pub(super) fn settle<R: InvocationRuntime>(
    value: &mut Value,
    receiver: &mut Loans<'_, R>,
) -> Result<(), Flow> {
    match value {
        Value::Bytes(bytes) if matches!(bytes.bytes, Backing::Partition(_)) => {
            bytes.settle_partition(receiver)
        }
        Value::PartitionVec(vector) => vector.settle(receiver).map_err(refused),
        Value::PartitionIter(iterator) => iterator.settle(receiver).map_err(refused),
        _ => Err(Flow::Guard(
            "partition finalizer requires its exact owned carrier",
        )),
    }
}
/// Apply the one-byte source store to the staged owner in place. On failure,
/// the caller still owns this slot and settles it at its exact cleanup action.
pub(super) fn store_byte<R: InvocationRuntime>(
    value: &mut OwnedBytesValue,
    receiver: &mut Loans<'_, R>,
    index: u64,
    byte: u8,
) -> Result<(), Flow> {
    let Backing::Partition(owner) = &mut value.bytes else {
        return Err(Flow::Guard("partition store requires its affine owner"));
    };
    receiver
        .checked_store(owner, index, &[byte])
        .map_err(|error| match error {
            StoreError::Bounds => Flow::Failure(super::super::owned_buffer::normalize_byte_buffer(
                crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
            )),
            StoreError::Authority(error) => refused(error),
        })
}

/// Move the already paid affine container. No legacy Arc alias is substituted.
pub(super) fn into_iterator<R: InvocationRuntime>(
    value: Value,
    receiver: &mut Loans<'_, R>,
) -> Result<Value, (Flow, Value)> {
    let Value::PartitionVec(vector) = value else {
        return Err((
            Flow::Guard("partition iterator requires its affine Vec"),
            value,
        ));
    };
    match vector.into_iter(receiver) {
        Ok(iterator) => Ok(Value::PartitionIter(iterator)),
        Err((error, vector)) => Err((refused(error), Value::PartitionVec(vector))),
    }
}
/// Numeric classification, funding and complete child publication precede the
/// Value constructor. Language IterStep publication is not performed here.
pub(super) fn partition<R: InvocationRuntime>(
    receiver: &mut Loans<'_, R>,
    owner: R::Owner,
    count: u64,
    width: u64,
) -> Result<Value, (crate::construction_fusion::partition::Preparation, R::Owner)> {
    PartitionVec::partition(receiver, owner, count, width).map(Value::PartitionVec)
}

/// Actual owned carrier handoff: move the original Box without copy, recharge,
/// or conversion from a shared Arc. A fault returns the identical whole value.
pub(super) fn partition_whole(
    receiver: &mut Loans<'_, super::funding::BorrowedAccount<'_>>,
    value: crate::interpreter::OwnedBytesValue,
    count: u64,
    width: u64,
) -> Result<
    Value,
    (
        crate::construction_fusion::partition::Preparation,
        crate::interpreter::OwnedBytesValue,
    ),
> {
    let crate::interpreter::OwnedBytesValue { allocation, bytes } = value;
    let Backing::Whole(owner) = bytes else {
        return Err((
            receiver.preparation_failure(
                crate::construction_fusion::partition::Preparation::Authority(
                    crate::construction_fusion::domain::Refusal::Identity,
                ),
            ),
            crate::interpreter::OwnedBytesValue { allocation, bytes },
        ));
    };
    let allocation_id =
        std::num::NonZeroU32::new(allocation).filter(|id| id.get() <= receiver.allocation_count());
    let Some(allocation_id) =
        allocation_id.filter(|_| owner.account == receiver.account_identity())
    else {
        let error = receiver.preparation_failure(
            crate::construction_fusion::partition::Preparation::Authority(
                crate::construction_fusion::domain::Refusal::Identity,
            ),
        );
        return Err((
            error,
            crate::interpreter::OwnedBytesValue {
                allocation,
                bytes: Backing::Whole(owner),
            },
        ));
    };
    let account = owner.account;
    match PartitionVec::partition(receiver, owner.bytes, count, width) {
        Ok(vector) => Ok(Value::PartitionVec(vector.with_allocation(allocation_id))),
        Err((error, owner)) => Err((
            error,
            crate::interpreter::OwnedBytesValue {
                allocation,
                bytes: Backing::Whole(crate::interpreter::construction_fusion::WholeBacking {
                    account,
                    bytes: owner,
                }),
            },
        )),
    }
}
