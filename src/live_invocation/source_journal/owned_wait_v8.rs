//! Inert v8 combined journal proof data. No store, owner, or evidence authority.
//! Production binding and physical append adapters are owned by the typed runtime.
mod fold;
mod model;
#[cfg(test)]
mod tests;
mod wire;

use super::{SourceInvocationBinding, SourceJournalEntry, SourceJournalError};
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;
use serde_json::Value;

pub(super) const SCHEMA: &str = "semaprax.live-invocation.source-persisted-journal.v8";
pub(super) const RECORD_DOMAIN: &[u8] = b"semaprax.live-invocation.source-record.v8\0";
const MAX_DEPTH: usize = 24;

/// Inert body only; even authenticated decoding cannot construct a runtime owner.
#[derive(Clone, Debug, Eq, PartialEq)]
enum EntryV8 {
    Ordinary(SourceJournalEntry),
    Owned(model::OwnedBodyV8),
}

/// Expected chain facts borrowed from the independently checked context.
/// There is deliberately no public or production context factory in this slice.
pub(super) struct ExpectedRowV8<'a> {
    pub invocation: &'a str,
    pub generation: &'a str,
    pub seq: u32,
    pub prev_mac: &'a str,
    pub ordinary: &'a SourceInvocationBinding,
}

/// Future checked binder creates this only after its actual compiler-owned
/// carrier validators have replayed the inert row. No production factory yet.
pub(super) struct ValidatedEntryV8 {
    entry: EntryV8,
    observation: Option<crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8>,
}

/// Sealed expected facts, to be constructed by the actual typed binding/store
/// join. Neither codec bytes nor generic callbacks can mint this context.
pub(super) struct FoldContextV8 {
    ordinary: SourceInvocationBinding,
    created: model::OwnedBodyV8,
    plan_digest: String,
    cleanup_plan_digest: String,
    signature: Value,
    helper: String,
    authorize: String,
    granted: String,
    refused: String,
    refused_cleanup_empty: bool,
    checked_binding:
        std::sync::Arc<crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8>,
}
