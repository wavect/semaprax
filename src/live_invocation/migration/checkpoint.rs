//! Durable handoff checkpoints for a migrated live invocation.
//!
//! The generic journal sink deliberately knows nothing about migration. This
//! module keeps a migration handoff, carried state, and the destination's
//! causal journal in one atomic [`CheckpointStore`] document so a successful
//! recovery is a capability to run the destination and every destination
//! dispatch remains preceded by a durable combined checkpoint.

use serde_json::Value;

use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use crate::diagnostic::quote_json;

use super::super::identity::{digest, hex, unhex, LiveInvocationId};
use super::super::journal::{self, JournalEntry};
use super::super::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveKernelError,
    LiveKernelRun,
};
use super::super::persistence::JournalSink;
use super::{
    LiveMigrationError, LiveMigrationHandoff, MigratedLiveInvocation, HANDOFF_DOMAIN,
    MAX_MIGRATED_STATE_BYTES,
};

/// Schema of the combined migration-handoff and destination-journal record.
pub const PERSISTED_MIGRATION_HANDOFF_SCHEMA: &str =
    "semaprax.live-invocation.persisted-migration-handoff.v1";

/// A recovered or newly persisted handoff. It is the only input accepted by
/// [`run_migrated_destination`], which prevents the migration route from
/// dispatching before the bound handoff and state are durable.
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
    Malformed,
    SchemaMismatch,
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

/// Makes a just-created migration handoff durable at generation one. The
/// store either receives this entire record or retains its prior generation;
/// without this success there is no capability to start the destination.
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
    store
        .commit(record.generation, &encode(&record))
        .map_err(MigrationCheckpointError::Store)?;
    Ok(record)
}

/// Recovers a combined checkpoint only when it is bound to `destination` and
/// every digest/chain link recomputes from the persisted bytes.
pub fn recover_migration_handoff(
    document: &str,
    destination: &LiveInvocationId,
) -> Result<RecoveredMigrationHandoff, MigrationCheckpointError> {
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
    let entries = journal::decode(&object["entries"]).map_err(MigrationCheckpointError::Journal)?;
    if object["chain"].as_str() != Some(journal::chain(&entries).as_str()) {
        return Err(MigrationCheckpointError::ChainMismatch);
    }
    Ok(RecoveredMigrationHandoff {
        handoff,
        migrated_state,
        destination_journal: entries,
        generation: object["generation"]
            .as_u64()
            .ok_or(MigrationCheckpointError::Malformed)?,
    })
}

struct MigrationCheckpointSink<'a> {
    store: &'a mut dyn CheckpointStore,
    record: &'a mut RecoveredMigrationHandoff,
}

impl JournalSink for MigrationCheckpointSink<'_> {
    fn persist(&mut self, entries: &[JournalEntry]) -> Result<(), CheckpointStoreError> {
        let next = self.record.generation.saturating_add(1);
        let candidate = RecoveredMigrationHandoff {
            handoff: self.record.handoff.clone(),
            migrated_state: self.record.migrated_state.clone(),
            destination_journal: entries.to_vec(),
            generation: next,
        };
        self.store.commit(next, &encode(&candidate))?;
        *self.record = candidate;
        Ok(())
    }
}

/// Runs a migrated destination through the only local migration dispatch
/// route. It requires a prior persisted/recovered handoff, binds the supplied
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
    let mut sink = MigrationCheckpointSink { store, record };
    let mut routed_handlers = LiveInvocationHandlers {
        capability,
        handler: &mut **handler,
        decoder: &mut **decoder,
        gate: &mut **gate,
        budget: &mut **budget,
        observer: &mut **observer,
        policy: &mut **policy,
        effect: effect.as_deref_mut(),
        sink: Some(&mut sink),
    };
    let result = run_live_invocation(
        config,
        sink.record.destination_journal.clone(),
        &mut routed_handlers,
        cancellation,
    )
    .map_err(MigrationDestinationError::Kernel);
    result.map(|run| MigrationDestinationRun {
        generation: sink.record.generation,
        run,
    })
}
