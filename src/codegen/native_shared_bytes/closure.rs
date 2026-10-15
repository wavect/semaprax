//! Fail-closed consumer closure for the selected shared-owner Bytes profile.
//!
//! Every ordinary native family that produces, reads, stores, transfers,
//! settles or publishes a Bytes owner is a [`Consumer`]. Each one either names
//! the selected runtime helper that implements it or states why it refuses.
//! Selection refuses before emission when any reachable consumer refuses, so
//! a selected carrier can never reach ordinary `free(ptr)`, a legacy helper or
//! an unauthenticated provider. [`CARRIER_SITES`] pins every production source
//! that touches the ordinary carrier; a new site fails the owning drift test.
use std::collections::BTreeSet;

use crate::byte_ops::ByteOp;
use crate::diagnostic::Diagnostic;
use crate::vec_ops::VecOp;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Consumer {
    // Producers. Each backing event keeps its ordinary charge.
    Zeroed,
    ZeroedBounded,
    Copy,
    DeepClone,
    Partition,
    // Owner transitions and borrowed views.
    Move,
    Drop,
    CleanupFinalizer,
    View,
    Loan,
    Set,
    Set5,
    Set1Or5,
    Set1Or6Or48,
    // Vec<Bytes>, the legacy tag-9 family.
    VecWithCapacity,
    VecPush,
    VecPop,
    VecView,
    VecSet,
    VecClear,
    VecReserveExact,
    VecCloneAt,
    VecLen,
    VecCapacity,
    VecMove,
    VecDrop,
    VecReplace,
    VecSort,
    VecOwnedLeafOnly,
    // Consuming iteration.
    IterFromVec,
    IterNext,
    IterDrop,
    // Public boundary.
    PublicCopyOut,
    PublicSnapshotIn,
    ProviderDrop,
    // Existing ordinary families not converted to the selected carrier.
    LegacyRecordVec,
    OwnedLeafVec,
    RecordIterator,
    ProjectedLeafField,
    NestedOwnedRecord,
    BoxPayload,
    OnceCapture,
    OwnedDataProvider,
    PublicGenericBridge,
    HostIo,
    Resource,
    StreamEpoch,
    ReservedArena,
    SelectedFusedRow,
    AggregateLayout,
    AgentLifecycleExecutor,
    ImportedCall,
    BorrowedNonOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Support {
    /// Implemented by the named selected runtime helper.
    Shared(&'static str),
    /// Not converted; selection refuses before emission.
    Refused(&'static str),
}

/// Assigned codes come from the shared operation owner. A status mapping
/// does not grant source admission or a reachable consumer closure.
pub(crate) const PARTITION_STATUS_CODES: Option<[u32; 4]> =
    Some(crate::construction_fusion::partition::operation::STATUS_CODES);

impl Consumer {
    pub(crate) const fn support(self) -> Support {
        use Support::{Refused, Shared};
        match self {
            Self::Zeroed => Shared("spx_shb_zeroed"),
            Self::ZeroedBounded => Shared("spx_shb_bounded_check"),
            Self::Copy => Shared("spx_shb_copy"),
            Self::DeepClone => Shared("spx_shb_clone"),
            Self::Partition => match PARTITION_STATUS_CODES {
                Some(_) => Shared("spx_shb_partition_prepare"),
                None => Refused("semaprax.byte-partition.v1 status codes are UNASSIGNED"),
            },
            Self::Move => Shared("spx_shb_move"),
            Self::Drop => Shared("spx_shb_drop"),
            Self::CleanupFinalizer => Shared("spx_shb_drop"),
            Self::View => Shared("spx_shb_view"),
            Self::Loan => Shared("spx_shb_loan_begin"),
            Self::Set => Shared("spx_shb_set"),
            Self::Set5 => Shared("spx_shb_set5"),
            Self::Set1Or5 => Shared("spx_shb_set1_or5"),
            Self::Set1Or6Or48 => Shared("spx_shb_set1_or6_or48"),
            Self::VecWithCapacity => Shared("spx_shb_vec_with_capacity"),
            Self::VecPush => Shared("spx_shb_vec_push"),
            Self::VecPop => Shared("spx_shb_vec_pop"),
            Self::VecView => Shared("spx_shb_vec_view"),
            Self::VecSet => Shared("spx_shb_vec_set"),
            Self::VecClear => Shared("spx_shb_vec_clear"),
            Self::VecReserveExact => Shared("spx_shb_vec_reserve_exact"),
            Self::VecCloneAt => Shared("spx_shb_vec_clone_at"),
            Self::VecLen => Shared("spx_shb_vec_len"),
            Self::VecCapacity => Shared("spx_shb_vec_capacity"),
            Self::VecMove => Shared("spx_shb_vec_move"),
            Self::VecDrop => Shared("spx_shb_vec_drop"),
            Self::VecReplace => Refused("ordinary Vec<Bytes> admits no replace"),
            Self::VecSort => Refused("ordinary Vec<Bytes> sorting is not admitted"),
            Self::VecOwnedLeafOnly => Refused("owned-leaf Vec operation"),
            Self::IterFromVec => Shared("spx_shb_iter_from_vec"),
            Self::IterNext => Shared("spx_shb_iter_next"),
            Self::IterDrop => Shared("spx_shb_iter_drop"),
            Self::PublicCopyOut | Self::PublicSnapshotIn | Self::ProviderDrop => {
                Refused("public bridge has no owning contract and gates")
            }
            Self::LegacyRecordVec => Refused("record Vec stores two-word leaves"),
            Self::OwnedLeafVec => Refused("owned-leaf layouts pin 16-byte Bytes fields"),
            Self::RecordIterator => Refused("record iterator settles two-word leaves"),
            Self::ProjectedLeafField => Refused("projected leaf reads copy a two-word carrier"),
            Self::NestedOwnedRecord => Refused("nested owned records embed two-word leaves"),
            Self::BoxPayload => Refused("Box payload drops a two-word carrier"),
            Self::OnceCapture => Refused("affine callable captures drop a two-word carrier"),
            Self::OwnedDataProvider => Refused("owned-data provider publishes two-word owners"),
            Self::PublicGenericBridge => Refused("public generic bridge drops two-word owners"),
            Self::HostIo => Refused("host I/O produces malloc-backed two-word owners"),
            Self::Resource => Refused("resource ABI names the two-word carrier"),
            Self::StreamEpoch => Refused("stdin stream epoch is a separate carrier profile"),
            Self::ReservedArena => Refused("reserved arena is a distinct private profile"),
            Self::SelectedFusedRow => Refused("selected fused rows own a separate private domain"),
            Self::AggregateLayout => Refused("Native64 aggregate layout pins the 16-byte carrier"),
            Self::AgentLifecycleExecutor => {
                Refused("agent lifecycle executor drops two-word owners")
            }
            Self::ImportedCall => Refused("imported and foreign calls have no selected crossing"),
            Self::BorrowedNonOwner => Shared("spx_shb_view"),
        }
    }

    /// Exhaustive: a new byte operation is a compile error here, not a gap.
    pub(crate) const fn from_byte_op(op: ByteOp) -> Self {
        match op {
            ByteOp::Len | ByteOp::Get | ByteOp::Range => Self::BorrowedNonOwner,
            ByteOp::ArrayAsSlice | ByteOp::StrAsBytes | ByteOp::StringAsStr => {
                Self::BorrowedNonOwner
            }
            ByteOp::BytesAsSlice => Self::View,
            ByteOp::Copy => Self::Copy,
            ByteOp::Zeroed => Self::Zeroed,
            ByteOp::ZeroedBounded => Self::ZeroedBounded,
            ByteOp::Partition => Self::Partition,
            ByteOp::Set => Self::Set,
            ByteOp::Set5 => Self::Set5,
            ByteOp::Set1Or5 => Self::Set1Or5,
            ByteOp::Set1Or6Or48 => Self::Set1Or6Or48,
        }
    }

    /// Exhaustive mapping of Vec operations whose element type is `Bytes`.
    pub(crate) const fn from_bytes_vec_op(op: VecOp) -> Self {
        match op {
            VecOp::WithCapacity => Self::VecWithCapacity,
            VecOp::Push => Self::VecPush,
            VecOp::Len => Self::VecLen,
            VecOp::Capacity => Self::VecCapacity,
            VecOp::Get => Self::VecView,
            VecOp::ReserveExact => Self::VecReserveExact,
            VecOp::Set => Self::VecSet,
            VecOp::Clear => Self::VecClear,
            VecOp::CloneAt => Self::VecCloneAt,
            VecOp::Replace => Self::VecReplace,
            VecOp::Sort | VecOp::SortBy => Self::VecSort,
            VecOp::ReserveOwned
            | VecOp::SortOwned
            | VecOp::StrAt
            | VecOp::UniqueBy
            | VecOp::UpdateField => {
                Self::VecOwnedLeafOnly
            }
        }
    }
}

/// Every production source outside this module that names the ordinary
/// two-word carrier or its `free(ptr)` drop, with the families it lowers.
pub(crate) const CARRIER_SITES: &[(&str, &[Consumer])] = &[
    ("src/aggregate_layout.rs", &[Consumer::AggregateLayout]),
    (
        "src/agent_lifecycle/authorization/native_executor.rs",
        &[Consumer::AgentLifecycleExecutor],
    ),
    (
        "src/codegen/native_box/owned_payload.rs",
        &[Consumer::BoxPayload],
    ),
    (
        "src/codegen/native_byte_data.rs",
        &[
            Consumer::Zeroed,
            Consumer::ZeroedBounded,
            Consumer::Copy,
            Consumer::View,
            Consumer::Move,
            Consumer::Drop,
            Consumer::Set,
            Consumer::Set5,
            Consumer::Set1Or5,
            Consumer::Set1Or6Or48,
        ],
    ),
    (
        "src/codegen/native_byte_data/reserved_allocators.c",
        &[Consumer::ReservedArena],
    ),
    (
        "src/codegen/native_byte_data/reserved_drop.c",
        &[Consumer::ReservedArena],
    ),
    (
        "src/codegen/native_byte_data/stream_epoch.rs",
        &[Consumer::StreamEpoch],
    ),
    ("src/codegen/native_bytes.rs", &[Consumer::CleanupFinalizer]),
    (
        "src/codegen/native_bytes/owned_leaf.rs",
        &[Consumer::Drop, Consumer::Move],
    ),
    (
        "src/codegen/native_cleanup_emit.rs",
        &[Consumer::CleanupFinalizer],
    ),
    ("src/codegen/native_command_io.rs", &[Consumer::HostIo]),
    (
        "src/codegen/native_emit/expression/nested_owned.rs",
        &[Consumer::NestedOwnedRecord],
    ),
    (
        "src/codegen/native_emit/expression/vec_ops/field_read.rs",
        &[Consumer::ProjectedLeafField],
    ),
    (
        "src/codegen/native_emit/filesystem_io.rs",
        &[Consumer::HostIo],
    ),
    (
        "src/codegen/native_emit/filesystem_io_v2.rs",
        &[Consumer::HostIo],
    ),
    (
        "src/codegen/native_emit/filesystem_io_v3.rs",
        &[Consumer::HostIo],
    ),
    ("src/codegen/native_emit/http_io.rs", &[Consumer::HostIo]),
    (
        "src/codegen/native_emit/nested_owned.rs",
        &[Consumer::NestedOwnedRecord],
    ),
    ("src/codegen/native_emit/network_io.rs", &[Consumer::HostIo]),
    ("src/codegen/native_emit/once.rs", &[Consumer::OnceCapture]),
    (
        "src/codegen/native_emit/process_runtime.c",
        &[Consumer::HostIo],
    ),
    (
        "src/codegen/native_emit/public_generic_bridge.rs",
        &[Consumer::PublicGenericBridge],
    ),
    (
        "src/codegen/native_emit/public_generic_bridge/allocating.rs",
        &[Consumer::PublicGenericBridge],
    ),
    (
        "src/codegen/native_iter/owned.rs",
        &[
            Consumer::IterFromVec,
            Consumer::IterNext,
            Consumer::IterDrop,
            Consumer::RecordIterator,
        ],
    ),
    (
        "src/codegen/native_owned_data_provider.rs",
        &[Consumer::OwnedDataProvider, Consumer::ProviderDrop],
    ),
    (
        "src/codegen/native_owned_data_provider/nested_record.rs",
        &[Consumer::OwnedDataProvider],
    ),
    ("src/codegen/native_resource.rs", &[Consumer::Resource]),
    (
        "src/codegen/native_vec/owned_leaf.c",
        &[Consumer::OwnedLeafVec],
    ),
    (
        "src/codegen/native_vec/owned_payload.rs",
        &[
            Consumer::VecWithCapacity,
            Consumer::VecPush,
            Consumer::VecSet,
            Consumer::VecClear,
            Consumer::VecReserveExact,
            Consumer::VecLen,
            Consumer::VecCapacity,
            Consumer::VecMove,
            Consumer::VecDrop,
            Consumer::LegacyRecordVec,
        ],
    ),
];

/// The only admission state. The cross-backend join adds an opening variant
/// together with its interpreter and Core-Wasm counterparts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Admission {
    ClosedUntilCrossBackendJoin,
}

/// A complete closure over one program's reachable consumers. Constructed
/// only by [`select`]; fields are private so no caller can assemble one.
#[derive(Debug)]
pub(crate) struct Selection {
    consumers: BTreeSet<Consumer>,
}

impl Selection {
    pub(crate) fn consumers(&self) -> &BTreeSet<Consumer> {
        &self.consumers
    }
}

/// Unsupported consumers are reported first, in canonical order, so the
/// refusal names the exact blocker; a complete closure still refuses while
/// admission is closed.
pub(crate) fn select(
    reachable: impl IntoIterator<Item = Consumer>,
    admission: Admission,
) -> Result<Selection, Diagnostic> {
    let _complete = closed_over(reachable)?;
    match admission {
        Admission::ClosedUntilCrossBackendJoin => Err(super::error(
            "actual producer admission stays closed until the interpreter, native and Core-Wasm closures join",
        )),
    }
}

pub(super) fn closed_over(
    reachable: impl IntoIterator<Item = Consumer>,
) -> Result<Selection, Diagnostic> {
    let consumers: BTreeSet<Consumer> = reachable.into_iter().collect();
    if consumers.is_empty() {
        return Err(super::error("no reachable Bytes owner selects the profile"));
    }
    for consumer in &consumers {
        if let Support::Refused(reason) = consumer.support() {
            return Err(super::error(format!(
                "{consumer:?} is outside the shared-owner closure: {reason}"
            )));
        }
    }
    Ok(Selection { consumers })
}

/// Test-only: a complete closure for exercising emission. Carries no
/// admission and is unreachable from any production route.
#[cfg(test)]
pub(crate) fn closed_over_for_tests(
    reachable: impl IntoIterator<Item = Consumer>,
) -> Result<Selection, Diagnostic> {
    closed_over(reachable)
}
