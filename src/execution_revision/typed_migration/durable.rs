//! Caller-owned handoff persistence and exact-runtime recovery. Store snapshots
//! and expected handoff digests are trusted inputs, never ambient authority.
use super::{handoff::Handoff, *};
use crate::agent_lifecycle::iterative::effects::DurableTypedFailure;
use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use serde_json::Value;

const SCHEMA: &str = "semaprax.agent-migrated-checkpoint.v1";
const MAX_BYTES: usize = 8 * 1024 * 1024;

pub struct DurableMigrationFailure {
    diagnostics: Vec<Diagnostic>,
    checkpoint: String,
    durable: Option<DurableTypedFailure>,
}
impl DurableMigrationFailure {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    /// Latest local candidate, including a possibly lost acknowledgement.
    pub fn checkpoint(&self) -> &str {
        &self.checkpoint
    }
    pub fn terminal(&self) -> Option<&crate::agent_lifecycle::iterative::IterativeRun> {
        self.durable
            .as_ref()
            .and_then(DurableTypedFailure::terminal)
    }
}
impl std::fmt::Debug for DurableMigrationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableMigrationFailure")
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

struct Snapshot {
    handoff: Handoff,
    checkpoint: Option<String>,
}
impl Snapshot {
    fn canonical_json(&self) -> String {
        format!(
            "{}\n",
            json!({"schema":SCHEMA,"handoff":self.handoff.canonical_json(),"checkpoint":self.checkpoint})
        )
    }
    fn decode(document: &str, expected: &str) -> Result<Self> {
        if document.len() > MAX_BYTES {
            return Err(refused("migration.snapshot.bytes"));
        }
        let value: Value =
            serde_json::from_str(document).map_err(|_| refused("migration.snapshot.json"))?;
        if value.as_object().is_none_or(|object| object.len() != 3)
            || value["schema"] != SCHEMA
            || !value["checkpoint"].is_null() && !value["checkpoint"].is_string()
        {
            return Err(refused("migration.snapshot.keys"));
        }
        let handoff = Handoff::decode(
            value["handoff"]
                .as_str()
                .ok_or_else(|| refused("migration.snapshot.handoff"))?,
            expected,
        )?;
        let checkpoint = value["checkpoint"].as_str().map(str::to_owned);
        let snapshot = Self {
            handoff,
            checkpoint,
        };
        if snapshot.canonical_json() != document {
            return Err(refused("migration.snapshot.canonical"));
        }
        Ok(snapshot)
    }
}
struct HandoffStore<'a> {
    store: &'a mut dyn CheckpointStore,
    snapshot: Snapshot,
    candidate: String,
}
impl CheckpointStore for HandoffStore<'_> {
    fn commit(
        &mut self,
        generation: u64,
        document: &str,
    ) -> std::result::Result<(), CheckpointStoreError> {
        self.snapshot.checkpoint = Some(document.to_owned());
        self.candidate = self.snapshot.canonical_json();
        if self.candidate.len() > MAX_BYTES {
            return Err(CheckpointStoreError);
        }
        self.store.commit(generation, &self.candidate)
    }
}

impl MigratedAgentRuntimeV2 {
    /// Keep this digest in the caller's trusted migration binding for recovery.
    pub fn handoff_digest(&self) -> Result<String> {
        Ok(Handoff::from_seed(&self.seed)?.digest())
    }
    /// Persist the complete handoff before any destination stage or host call.
    /// The caller supplies exclusive writer authority for a fresh destination store.
    pub fn run_durable(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> std::result::Result<AgentRuntimeV2DurableEvidence, DurableMigrationFailure> {
        run(self, None, handler, cancellation, store)
    }
}

/// A recovered checked runtime plus the trusted journal it must replay.
/// Consuming it cannot be redirected into the non-durable continuation path.
pub struct ResumedMigratedAgentRuntimeV2 {
    migrated: MigratedAgentRuntimeV2,
    snapshot: Snapshot,
}
impl ResumedMigratedAgentRuntimeV2 {
    pub fn run_durable(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        store: &mut dyn CheckpointStore,
    ) -> std::result::Result<AgentRuntimeV2DurableEvidence, DurableMigrationFailure> {
        run(
            self.migrated,
            Some(self.snapshot),
            handler,
            cancellation,
            store,
        )
    }
}

fn run(
    migrated: MigratedAgentRuntimeV2,
    retained: Option<Snapshot>,
    handler: &mut dyn TypedEffectHandler,
    cancellation: &AgentCancellation,
    store: &mut dyn CheckpointStore,
) -> std::result::Result<AgentRuntimeV2DurableEvidence, DurableMigrationFailure> {
    let fresh = retained.is_none();
    let snapshot = match retained {
        Some(snapshot) => snapshot,
        None => Snapshot {
            handoff: Handoff::from_seed(&migrated.seed).map_err(|diagnostics| {
                DurableMigrationFailure {
                    diagnostics,
                    checkpoint: String::new(),
                    durable: None,
                }
            })?,
            checkpoint: None,
        },
    };
    let candidate = snapshot.canonical_json();
    if candidate.len() > MAX_BYTES {
        return Err(DurableMigrationFailure {
            diagnostics: refused("migration.snapshot.bytes"),
            checkpoint: String::new(),
            durable: None,
        });
    }
    let mut store = HandoffStore {
        store,
        snapshot,
        candidate,
    };
    if fresh && store.store.commit(0, &store.candidate).is_err() {
        return Err(DurableMigrationFailure {
            diagnostics: refused("migration.handoff.uncertain_store"),
            checkpoint: store.candidate,
            durable: None,
        });
    }
    let retained_checkpoint = store.snapshot.checkpoint.clone();
    let runtime = migrated.runtime;
    let result = runtime.lifecycle.run_durable_from_seed(
        &runtime.task,
        &runtime.proposals,
        handler,
        runtime.budget,
        runtime.effects,
        cancellation,
        runtime.revision.digest(),
        &runtime.program_root,
        retained_checkpoint.as_deref(),
        &mut store,
        migrated.seed.max_reserved_fuel(),
        &migrated.seed,
    );
    let result = result.map_err(|durable| DurableMigrationFailure {
        diagnostics: durable.diagnostics().to_vec(),
        checkpoint: store.candidate.clone(),
        durable: Some(durable),
    })?;
    let handoff_digest = store.snapshot.handoff.digest();
    let evidence = root(
        "semaprax.evidence-root.durable-migration.v1",
        json!({
            "execution_revision":runtime.revision.digest(), "instance_root":runtime.instance.digest(),
            "migration_root":migrated.seed.binding.digest(), "handoff":handoff_digest,
            "typed_effect_evidence":result.run().evidence_digest(), "checkpoint":result.checkpoint_digest(),
            "iterations":result.iterations(), "stages":result.stages(),
        }),
    );
    Ok(AgentRuntimeV2DurableEvidence::from_migration(
        result,
        evidence,
        runtime.revision,
        handoff_digest,
        store.candidate,
    ))
}

/// Recover only from the caller-authorized trusted store under exclusive writer
/// authority. `expected_handoff_digest` must come from that trusted binding,
/// independently of submitted snapshot bytes. Stored State is trusted producer
/// input; hashes alone do not prove that a migration or host effect occurred.
#[allow(clippy::too_many_arguments)]
pub fn resume_migrated_agent_runtime_v2(
    previous: AgentRuntimeV2,
    destination: AgentRuntimeV2,
    retained_snapshot: &str,
    expected_handoff_digest: &str,
    expected_previous_revision: &str,
    expected_destination_revision: &str,
) -> Result<ResumedMigratedAgentRuntimeV2> {
    if previous.revision.digest() != expected_previous_revision
        || destination.revision.digest() != expected_destination_revision
    {
        return Err(refused("migration.stale_revision"));
    }
    let snapshot = Snapshot::decode(retained_snapshot, expected_handoff_digest)?;
    let handoff = &snapshot.handoff;
    let document: Value =
        serde_json::from_str(&handoff.migration).map_err(|_| refused("migration.handoff.root"))?;
    let schema = document["schema"]
        .as_str()
        .ok_or_else(|| refused("migration.handoff.root"))?;
    let facts = &document["facts"];
    let fields = [
        "previous_program_root",
        "destination_program_root",
        "previous_execution_revision",
        "destination_execution_revision",
        "previous_evidence",
        "previous_checkpoint",
        "migration_function",
        "max_migration_steps",
        "max_reserved_fuel",
        "prior_iterations",
        "prior_stages",
        "calls",
        "argument_bytes",
        "result_bytes",
        "reserved_fuel",
        "previous_state",
        "migrated_state",
    ];
    let chained = schema == "semaprax.agent-state-migration.v2";
    if (!chained && schema != "semaprax.agent-state-migration.v1")
        || facts
            .as_object()
            .is_none_or(|map| map.len() != fields.len() + usize::from(chained))
        || fields.iter().any(|field| facts.get(*field).is_none())
        || chained
            && facts["previous_handoff"]
                .as_str()
                .is_none_or(|digest| !hash_valid(digest))
    {
        return Err(refused("migration.handoff.root"));
    }
    let binding = root(schema, facts.clone());
    if binding.canonical_json() != handoff.migration
        || previous.program_root == destination.program_root
        || facts["previous_program_root"] != previous.program_root
        || facts["destination_program_root"] != destination.program_root
        || facts["previous_execution_revision"] != previous.revision.digest()
        || facts["destination_execution_revision"] != destination.revision.digest()
        || facts["prior_iterations"].as_u64() != Some(handoff.iterations as u64)
        || facts["prior_stages"].as_u64() != Some(handoff.stages as u64)
        || facts["max_reserved_fuel"].as_u64() != Some(handoff.max_reserved_fuel)
        || facts["calls"].as_u64() != Some(handoff.usage.calls)
        || facts["argument_bytes"].as_u64() != Some(handoff.usage.argument_bytes)
        || facts["result_bytes"].as_u64() != Some(handoff.usage.result_bytes)
        || facts["reserved_fuel"].as_u64() != Some(handoff.usage.reserved_fuel)
        || facts["migrated_state"] != crate::agent_lifecycle::encode_value(&handoff.value)
        || handoff.usage.reserved_fuel > handoff.max_reserved_fuel
    {
        return Err(refused("migration.handoff.binding"));
    }
    for key in ["previous_evidence", "previous_checkpoint"] {
        if facts[key].as_str().is_none_or(|digest| !hash_valid(digest)) {
            return Err(refused("migration.handoff.producer"));
        }
    }
    let old_program = selected_program(&previous)?;
    let new_program = selected_program(&destination)?;
    let old_state = state_type(&previous)?;
    let new_state = state_type(&destination)?;
    if flat_state(&old_program, &old_state)? != flat_state(&new_program, &old_state)? {
        return Err(refused("migration.old_state_schema_drift"));
    }
    let fields = flat_state(&new_program, &new_state)?;
    let RetainedValue::Record(record) = &handoff.value else {
        return Err(refused("migration.result_state"));
    };
    if record.record != new_state
        || record.fields.len() != fields.len()
        || record.fields.iter().zip(&fields).any(|(actual, (id, ty))| {
            actual.field != *id
                || !matches!(
                    (&actual.value, ty),
                    (RetainedValue::Bool(_), ResolvedType::Bool)
                        | (RetainedValue::I32(_), ResolvedType::I32)
                        | (RetainedValue::I64(_), ResolvedType::I64)
                        | (RetainedValue::U8(_), ResolvedType::U8)
                        | (RetainedValue::Usize(_), ResolvedType::Usize)
                        | (RetainedValue::Bytes(_), ResolvedType::Bytes)
                )
        })
    {
        return Err(refused("migration.result_state"));
    }
    let function = facts["migration_function"]
        .as_str()
        .ok_or_else(|| refused("migration.function"))?;
    let call = prepare_retained_call(&new_program, function)?;
    let entry = new_program
        .functions
        .iter()
        .find(|entry| entry.id.as_str() == function)
        .ok_or_else(|| refused("migration.function"))?;
    let nominal = |id| ResolvedType::Nominal {
        declaration: id,
        arguments: Vec::new(),
    };
    if entry.params.len() != 1
        || entry.params[0].ty != nominal(old_state)
        || entry.return_type != nominal(new_state)
        || call.function_ids().any(|id| {
            new_program
                .functions
                .iter()
                .find(|entry| entry.id.as_str() == id)
                .is_none_or(|entry| !entry.effects.is_empty())
        })
    {
        return Err(refused("migration.pure_signature"));
    }
    let seed = MigrationSeed {
        value: handoff.value.clone(),
        binding,
        usage: handoff.usage,
        iterations: handoff.iterations,
        stages: handoff.stages,
        max_reserved_fuel: handoff.max_reserved_fuel,
    };
    Ok(ResumedMigratedAgentRuntimeV2 {
        migrated: MigratedAgentRuntimeV2 {
            runtime: destination,
            seed,
        },
        snapshot,
    })
}
fn hash_valid(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
