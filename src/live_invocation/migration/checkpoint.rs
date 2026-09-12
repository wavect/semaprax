//! Durable handoff checkpoints for a migrated live invocation.
//!
//! The generic journal sink deliberately knows nothing about migration. This
//! module keeps a migration handoff, carried state, and the destination's
//! causal journal in one atomic [`CheckpointStore`] document. Recovery
//! validates deterministic bytes but grants no authority; a caller that uses
//! the migration dispatch adapter gets a durable combined checkpoint before
//! every destination dispatch.

use serde_json::Value;

use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use crate::diagnostic::quote_json;

use super::super::identity::{digest, hex, unhex, LiveInvocationId};
use super::super::journal::{self, JournalEntry};
use super::super::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveKernelError,
    LiveKernelRun, TurnEffect,
};
use super::super::persistence::JournalSink;
use super::{
    LiveMigrationError, LiveMigrationHandoff, MigratedLiveInvocation, HANDOFF_DOMAIN,
    MAX_MIGRATED_STATE_BYTES,
};

/// Schema of the combined migration-handoff and destination-journal record.
pub const PERSISTED_MIGRATION_HANDOFF_SCHEMA: &str =
    "semaprax.live-invocation.persisted-migration-handoff.v1";

/// Upper bound checked before JSON parsing. It matches the existing durable
/// operation checkpoint cap and still leaves room for the hex encoding of a
/// maximum-sized migrated state plus its bounded journal envelope.
pub const MAX_MIGRATION_CHECKPOINT_BYTES: usize = 2_097_152;

/// A recovered or newly persisted handoff control record. It is the input
/// accepted by [`run_migrated_destination`], which makes that adapter persist
/// the bound handoff and state before destination dispatch. The record is not
/// authority: its bytes are caller-supplied and it is intentionally cloneable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredMigrationHandoff {
    handoff: LiveMigrationHandoff,
    migrated_state: Vec<u8>,
    destination_journal: Vec<JournalEntry>,
    generation: u64,
}

impl RecoveredMigrationHandoff {
    #[must_use]
    pub fn handoff(&self) -> &LiveMigrationHandoff {
        &self.handoff
    }

    /// State bytes whose digest is bound into [`Self::handoff`]. The caller
    /// supplies them to its destination observer/policy before calling the
    /// enforced dispatch route.
    #[must_use]
    pub fn migrated_state(&self) -> &[u8] {
        &self.migrated_state
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// A malformed, mismatched, or tampered migration checkpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationCheckpointError {
    Store(CheckpointStoreError),
    Capacity,
    Malformed,
    SchemaMismatch,
    Generation,
    NonCanonical,
    DestinationMismatch,
    HandoffMismatch,
    StateMismatch,
    Journal(journal::DecodeError),
    ChainMismatch,
}

/// Refusal from the enforced migrated-destination dispatch route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationDestinationError {
    Destination(LiveMigrationError),
    SchemaMismatch,
    ExistingSink,
    Kernel(LiveKernelError),
}

/// The completed destination run and the combined checkpoint generation that
/// contains its exact destination journal.
pub struct MigrationDestinationRun {
    pub run: LiveKernelRun,
    pub generation: u64,
}

fn encode(record: &RecoveredMigrationHandoff) -> String {
    let journal = journal::render(&record.destination_journal);
    format!(
        "{{\"schema\":{},\"destination\":{},\"generation\":{},\"handoff\":{},\"handoff_digest\":{},\"migrated_state\":{},\"migrated_state_digest\":{},\"chain\":{},\"entries\":{}}}\n",
        quote_json(PERSISTED_MIGRATION_HANDOFF_SCHEMA),
        quote_json(record.handoff.destination_identity()),
        record.generation,
        record.handoff.canonical(),
        quote_json(&record.handoff.digest()),
        quote_json(&hex(&record.migrated_state)),
        quote_json(record.handoff.migrated_state_digest()),
        quote_json(&journal::chain(&record.destination_journal)),
        journal.trim_end(),
    )
}

fn valid_generation(generation: u64) -> bool {
    generation != 0 && generation != u64::MAX
}

fn state_matches_handoff(record: &RecoveredMigrationHandoff) -> bool {
    record.migrated_state.len() <= MAX_MIGRATED_STATE_BYTES
        && record.handoff.migrated_state_digest() == digest(HANDOFF_DOMAIN, &record.migrated_state)
}

/// Makes a just-created migration handoff durable at generation one. The
/// store either receives this entire record or retains its prior generation;
/// without this success the migration adapter has no durable control record.
pub fn persist_migration_handoff(
    store: &mut dyn CheckpointStore,
    migration: MigratedLiveInvocation,
) -> Result<RecoveredMigrationHandoff, MigrationCheckpointError> {
    let record = RecoveredMigrationHandoff {
        handoff: migration.handoff,
        migrated_state: migration.migrated_state,
        destination_journal: Vec::new(),
        generation: 1,
    };
    if !state_matches_handoff(&record) {
        return Err(MigrationCheckpointError::StateMismatch);
    }
    let document = encode(&record);
    if document.len() > MAX_MIGRATION_CHECKPOINT_BYTES {
        return Err(MigrationCheckpointError::Capacity);
    }
    store
        .commit(record.generation, &document)
        .map_err(MigrationCheckpointError::Store)?;
    Ok(record)
}

/// Recovers a combined checkpoint only when it is bound to `destination` and
/// every digest/chain link recomputes from the persisted bytes.
pub fn recover_migration_handoff(
    document: &str,
    destination: &LiveInvocationId,
) -> Result<RecoveredMigrationHandoff, MigrationCheckpointError> {
    if document.len() > MAX_MIGRATION_CHECKPOINT_BYTES {
        return Err(MigrationCheckpointError::Capacity);
    }
    if !document.ends_with('\n') {
        return Err(MigrationCheckpointError::NonCanonical);
    }
    let value: Value =
        serde_json::from_str(document).map_err(|_| MigrationCheckpointError::Malformed)?;
    let object = value
        .as_object()
        .ok_or(MigrationCheckpointError::Malformed)?;
    const KEYS: [&str; 9] = [
        "schema",
        "destination",
        "generation",
        "handoff",
        "handoff_digest",
        "migrated_state",
        "migrated_state_digest",
        "chain",
        "entries",
    ];
    if object.len() != KEYS.len() || !KEYS.iter().all(|key| object.contains_key(*key)) {
        return Err(MigrationCheckpointError::Malformed);
    }
    if object["schema"].as_str() != Some(PERSISTED_MIGRATION_HANDOFF_SCHEMA) {
        return Err(MigrationCheckpointError::SchemaMismatch);
    }
    if object["destination"].as_str() != Some(destination.digest()) {
        return Err(MigrationCheckpointError::DestinationMismatch);
    }
    let handoff = LiveMigrationHandoff::decode(&object["handoff"])
        .ok_or(MigrationCheckpointError::Malformed)?;
    if handoff.destination_identity() != destination.digest() {
        return Err(MigrationCheckpointError::DestinationMismatch);
    }
    if object["handoff_digest"].as_str() != Some(handoff.digest().as_str()) {
        return Err(MigrationCheckpointError::HandoffMismatch);
    }
    let generation = object["generation"]
        .as_u64()
        .ok_or(MigrationCheckpointError::Malformed)?;
    if !valid_generation(generation) {
        return Err(MigrationCheckpointError::Generation);
    }
    let migrated_state = unhex(
        object["migrated_state"]
            .as_str()
            .ok_or(MigrationCheckpointError::Malformed)?,
    )
    .ok_or(MigrationCheckpointError::Malformed)?;
    if migrated_state.len() > MAX_MIGRATED_STATE_BYTES
        || handoff.migrated_state_digest() != digest(HANDOFF_DOMAIN, &migrated_state)
        || object["migrated_state_digest"].as_str() != Some(handoff.migrated_state_digest())
    {
        return Err(MigrationCheckpointError::StateMismatch);
    }
    // A migration checkpoint is created before destination turn zero, so its
    // own canonical pre-dispatch state is an empty `entries` array. The
    // generic journal decoder correctly rejects that shape because it is not
    // a standalone causal journal; do not weaken it for this envelope.
    let entries = match object["entries"].as_array() {
        Some(entries) if entries.is_empty() => Vec::new(),
        Some(_) => {
            journal::decode(&object["entries"]).map_err(MigrationCheckpointError::Journal)?
        }
        None => return Err(MigrationCheckpointError::Journal(journal::DecodeError)),
    };
    if object["chain"].as_str() != Some(journal::chain(&entries).as_str()) {
        return Err(MigrationCheckpointError::ChainMismatch);
    }
    let record = RecoveredMigrationHandoff {
        handoff,
        migrated_state,
        destination_journal: entries,
        generation,
    };
    if encode(&record) != document {
        return Err(MigrationCheckpointError::NonCanonical);
    }
    Ok(record)
}

struct MigrationCheckpointSink<'a> {
    store: &'a mut dyn CheckpointStore,
    record: &'a mut RecoveredMigrationHandoff,
}

impl JournalSink for MigrationCheckpointSink<'_> {
    fn persist(&mut self, entries: &[JournalEntry]) -> Result<(), CheckpointStoreError> {
        let Some(next) = self.record.generation.checked_add(1) else {
            return Err(CheckpointStoreError);
        };
        // Every checkpoint this sink publishes must itself be recoverable.
        // `u64::MAX` has no valid successor, so accepting it as `next`
        // would write a document recovery must reject.
        if !valid_generation(next) {
            return Err(CheckpointStoreError);
        }
        let candidate = RecoveredMigrationHandoff {
            handoff: self.record.handoff.clone(),
            migrated_state: self.record.migrated_state.clone(),
            destination_journal: entries.to_vec(),
            generation: next,
        };
        let document = encode(&candidate);
        if document.len() > MAX_MIGRATION_CHECKPOINT_BYTES {
            return Err(CheckpointStoreError);
        }
        self.store.commit(next, &document)?;
        *self.record = candidate;
        Ok(())
    }
}

/// Runs a migrated destination through the only local migration dispatch
/// route. It requires a prior persisted/recovered control record, binds the supplied
/// destination identity and schema to it, and installs a combined checkpoint
/// sink before `run_live_invocation` can reach a model or effect dispatch.
/// Ordinary fresh live invocations remain free to use the generic kernel.
pub fn run_migrated_destination(
    record: &mut RecoveredMigrationHandoff,
    store: &mut dyn CheckpointStore,
    config: &LiveInvocationConfig<'_>,
    handlers: &mut LiveInvocationHandlers<'_>,
    cancellation: &crate::agent_runtime::AgentCancellation,
) -> Result<MigrationDestinationRun, MigrationDestinationError> {
    super::verify_destination_binding(&record.handoff, config.identity)
        .map_err(MigrationDestinationError::Destination)?;
    if config.interaction_schema_digest != record.handoff.destination_schema_digest() {
        return Err(MigrationDestinationError::SchemaMismatch);
    }
    if handlers.sink.is_some() {
        return Err(MigrationDestinationError::ExistingSink);
    }
    // Reborrow every caller seam into a short-lived handler bundle. The
    // generic handler bundle's optional sink remains untouched, while this
    // route cannot accidentally invoke the kernel without its combined
    // migration checkpoint sink.
    let super::super::kernel::LiveInvocationHandlers {
        capability,
        handler,
        decoder,
        gate,
        budget,
        observer,
        policy,
        effect,
        sink: _,
    } = handlers;
    let destination_journal = record.destination_journal.clone();
    let result = {
        let mut sink = MigrationCheckpointSink { store, record };
        let effect = match &mut *effect {
            Some(effect) => Some(&mut **effect as &mut dyn TurnEffect),
            None => None,
        };
        let mut routed_handlers = LiveInvocationHandlers {
            capability,
            handler: &mut **handler,
            decoder: &mut **decoder,
            gate: &mut **gate,
            budget: &mut **budget,
            observer: &mut **observer,
            policy: &mut **policy,
            effect,
            sink: Some(&mut sink),
        };
        run_live_invocation(
            config,
            destination_journal,
            &mut routed_handlers,
            cancellation,
        )
        .map_err(MigrationDestinationError::Kernel)
    };
    result.map(|run| MigrationDestinationRun {
        generation: record.generation,
        run,
    })
}
