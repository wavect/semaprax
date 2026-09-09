//! Checked, consuming migration from an actual durable Suspend into a new runtime.
//! Persisted recovery uses caller-trusted snapshots bound to exact runtime roots.
#[path = "typed_migration/durable.rs"]
mod durable;
#[path = "typed_migration/handoff.rs"]
mod handoff;
#[path = "typed_migration/linked.rs"]
mod linked;
use super::*;
use crate::agent_lifecycle::iterative::IterativeStatus;
use crate::agent_runtime_v2::checkpoint::CheckpointUsage;
use crate::hir::{self, DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
use crate::interpreter::retained_call::{
    evaluate_retained_call, prepare_retained_call, RetainedCallOutcome, RetainedValue,
};
pub use durable::{
    resume_migrated_agent_runtime_v2, DurableMigrationFailure, ResumedMigratedAgentRuntimeV2,
};

/// An internally produced initial State, never constructed from submitted JSON.
pub(crate) struct MigrationSeed {
    value: RetainedValue,
    binding: ExecutionRoot,
    usage: CheckpointUsage,
    iterations: usize,
    stages: usize,
    max_reserved_fuel: u64,
}
impl MigrationSeed {
    pub(crate) fn value(&self) -> &RetainedValue {
        &self.value
    }
    pub(crate) fn binding_digest(&self) -> &str {
        self.binding.digest()
    }
    pub(crate) fn usage(&self) -> CheckpointUsage {
        self.usage
    }
    pub(crate) fn prior_iterations(&self) -> usize {
        self.iterations
    }
    pub(crate) fn prior_stages(&self) -> usize {
        self.stages
    }
    pub(crate) fn max_reserved_fuel(&self) -> u64 {
        self.max_reserved_fuel
    }
}

/// One newly bound runtime plus the State its checked migration actually returned.
/// Consuming execution acquires fresh authorizations through the ordinary driver.
pub struct MigratedAgentRuntimeV2 {
    runtime: AgentRuntimeV2,
    seed: MigrationSeed,
}
impl MigratedAgentRuntimeV2 {
    pub fn migration_root(&self) -> &ExecutionRoot {
        &self.seed.binding
    }
    pub fn run(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> std::result::Result<AgentRuntimeV2MigrationEvidence, AgentRuntimeV2MigrationFailure> {
        let run = self
            .runtime
            .lifecycle
            .run_from_seed(
                &self.runtime.task,
                &self.runtime.proposals,
                handler,
                self.runtime.budget,
                self.runtime.effects,
                cancellation,
                &self.seed,
            )
            .map_err(|failure| AgentRuntimeV2MigrationFailure {
                diagnostics: failure.diagnostics,
                usage: failure.usage,
                iterations: failure.iterations,
                stages: failure.stages,
                terminal: failure.terminal,
            })?;
        let usage = run.usage;
        let iterations = run.iterations;
        let stages = run.stages;
        let run = run.run;
        let evidence = root(
            "semaprax.evidence-root.migration.v1",
            json!({
                "execution_revision": self.runtime.revision.digest(),
                "migration_root": self.seed.binding.digest(),
                "typed_effect_evidence": run.evidence_digest(),
                "calls": usage.calls, "argument_bytes": usage.argument_bytes,
                "result_bytes": usage.result_bytes, "reserved_fuel": usage.reserved_fuel,
                "iterations": iterations, "stages": stages,
            }),
        );
        Ok(AgentRuntimeV2MigrationEvidence {
            run,
            evidence,
            migration: self.seed.binding,
            usage,
            iterations,
            stages,
        })
    }
}
pub struct AgentRuntimeV2MigrationEvidence {
    run: TypedEffectRun,
    evidence: ExecutionRoot,
    migration: ExecutionRoot,
    usage: CheckpointUsage,
    iterations: usize,
    stages: usize,
}
impl AgentRuntimeV2MigrationEvidence {
    pub fn usage(&self) -> CheckpointUsage {
        self.usage
    }
    pub fn iterations(&self) -> usize {
        self.iterations
    }
    pub fn stages(&self) -> usize {
        self.stages
    }
    pub fn run(&self) -> &TypedEffectRun {
        &self.run
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn migration_root(&self) -> &ExecutionRoot {
        &self.migration
    }
}

/// A failed migrated invocation retains all charged work and any selected status.
pub struct AgentRuntimeV2MigrationFailure {
    diagnostics: Vec<Diagnostic>,
    usage: CheckpointUsage,
    iterations: usize,
    stages: usize,
    terminal: Option<crate::agent_lifecycle::iterative::IterativeRun>,
}
impl AgentRuntimeV2MigrationFailure {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn usage(&self) -> CheckpointUsage {
        self.usage
    }
    pub fn iterations(&self) -> usize {
        self.iterations
    }
    pub fn stages(&self) -> usize {
        self.stages
    }
    pub fn terminal(&self) -> Option<&crate::agent_lifecycle::iterative::IterativeRun> {
        self.terminal.as_ref()
    }
}
impl std::fmt::Debug for AgentRuntimeV2MigrationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntimeV2MigrationFailure")
            .field("diagnostics", &self.diagnostics)
            .field("usage", &self.usage)
            .field("iterations", &self.iterations)
            .field("stages", &self.stages)
            .finish()
    }
}

/// The old product authenticates the retained source that produced the State.
/// The new product independently binds destination policy and all live inputs.
/// The explicit pure migration lives in its exact selected retained source.
#[allow(clippy::too_many_arguments)]
pub fn migrate_suspended_agent_runtime_v2(
    previous: AgentRuntimeV2,
    suspended: AgentRuntimeV2DurableEvidence,
    destination: AgentRuntimeV2,
    expected_previous_revision: &str,
    expected_destination_revision: &str,
    migration_function: &str,
    max_migration_steps: usize,
    max_reserved_fuel: u64,
) -> std::result::Result<MigratedAgentRuntimeV2, AgentRuntimeV2MigrationFailure> {
    let mut charged_usage = suspended.run().usage();
    let charged_iterations = suspended.run().iterations();
    let mut charged_stages = 0;
    let result = (|| -> Result<MigratedAgentRuntimeV2> {
        if previous.revision.digest() != expected_previous_revision
            || destination.revision.digest() != expected_destination_revision
            || suspended.execution_revision() != &previous.revision
        {
            return Err(refused("migration.stale_revision"));
        }
        if previous.program_root == destination.program_root {
            return Err(refused("migration.unchanged_program"));
        }
        let run = suspended.run().run().lifecycle();
        if run.status() != IterativeStatus::Suspend || suspended.run().run().failure().is_some() {
            return Err(refused("migration.requires_actual_suspend"));
        }
        // Cumulative counters include the handoff baseline and every replay reservation.
        let prior_stages = suspended.run().stages();
        let prior_iterations = suspended.run().iterations();
        charged_stages = prior_stages;
        let value = run
            .value()
            .ok_or_else(|| refused("migration.state_missing"))?;
        if crate::agent_lifecycle::encode_value(value).len() > 262_144 {
            return Err(refused("migration.input_state_capacity"));
        }
        let programs = linked::programs(&previous, &destination, migration_function)?;
        let old_program = programs.previous;
        let new_program = programs.destination;
        let old_state = state_type(&previous)?;
        let new_state = state_type(&destination)?;
        let old_shape = flat_state(&old_program, &old_state)?;
        // The migration's old carrier must preserve the exact old nominal and field
        // identities and leaf types, even when the destination State evolved.
        if flat_state(&new_program, &old_state)? != old_shape {
            return Err(refused("migration.old_state_schema_drift"));
        }
        flat_state(&new_program, &new_state)?;
        let mut usage = suspended.run().usage();
        let reservation = u64::try_from(max_migration_steps)
            .ok()
            .and_then(|steps| steps.checked_mul(2))
            .filter(|steps| *steps > 0)
            .ok_or_else(|| refused("migration.fuel"))?;
        usage.reserved_fuel = usage
            .reserved_fuel
            .checked_add(reservation)
            .filter(|fuel| *fuel <= max_reserved_fuel)
            .ok_or_else(|| refused("migration.fuel_exhausted"))?;
        if usage.calls >= destination.effects.max_calls as u64
            || usage
                .argument_bytes
                .checked_add(usage.result_bytes)
                .is_none_or(|bytes| bytes > destination.effects.max_total_bytes as u64)
            || prior_iterations >= destination.budget.max_iterations
            || prior_stages >= destination.budget.max_stages
        {
            return Err(refused("migration.prior_usage_exhausts_destination"));
        }
        charged_usage = usage;
        let migrated = evaluate_migration(
            &new_program,
            migration_function,
            &old_state,
            &new_state,
            value,
            max_migration_steps,
        )?;
        let mut facts = json!({
            "previous_program_root": previous.program_root,
            "destination_program_root": destination.program_root,
            "previous_execution_revision": previous.revision.digest(),
            "destination_execution_revision": destination.revision.digest(),
            "previous_evidence": suspended.evidence_root().digest(),
            "previous_checkpoint": suspended.run().checkpoint_digest(),
            "migration_function": migration_function,
            "max_migration_steps": max_migration_steps,
            "max_reserved_fuel": max_reserved_fuel,
            "prior_iterations": prior_iterations, "prior_stages": prior_stages,
            "calls": usage.calls, "argument_bytes": usage.argument_bytes,
            "result_bytes": usage.result_bytes, "reserved_fuel": usage.reserved_fuel,
            "previous_state": crate::agent_lifecycle::encode_value(value),
            "migrated_state": crate::agent_lifecycle::encode_value(&migrated),
        });
        let schema = if let Some(linked_sources) = programs.linked_sources {
            facts["linked_sources"] = linked_sources;
            facts["previous_handoff"] = json!(suspended.migration_handoff_digest());
            "semaprax.agent-state-migration.v3"
        } else if let Some(predecessor) = suspended.migration_handoff_digest() {
            facts["previous_handoff"] = json!(predecessor);
            "semaprax.agent-state-migration.v2"
        } else {
            "semaprax.agent-state-migration.v1"
        };
        let binding = root(schema, facts);
        Ok(MigratedAgentRuntimeV2 {
            runtime: destination,
            seed: MigrationSeed {
                value: migrated,
                binding,
                usage,
                iterations: prior_iterations,
                stages: prior_stages,
                max_reserved_fuel,
            },
        })
    })();
    result.map_err(|diagnostics| AgentRuntimeV2MigrationFailure {
        diagnostics,
        usage: charged_usage,
        iterations: charged_iterations,
        stages: charged_stages,
        terminal: None,
    })
}

fn selected_program(runtime: &AgentRuntimeV2) -> Result<hir::ResolvedProgram> {
    let document: serde_json::Value = serde_json::from_str(runtime.deployment.canonical_json())
        .map_err(|_| refused("migration.deployment"))?;
    let path = document["facts"]["source_path"]
        .as_str()
        .ok_or_else(|| refused("migration.source_path"))?;
    let source = runtime
        .project
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .ok_or_else(|| refused("migration.retained_source"))?;
    hir::resolve(&crate::check(source.source(), path)?)
}
fn state_type(runtime: &AgentRuntimeV2) -> Result<DeclarationId> {
    let document: serde_json::Value = serde_json::from_str(runtime.lifecycle.canonical_json())
        .map_err(|_| refused("migration.lifecycle"))?;
    document["lifecycle"]["types"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["role"] == "state"))
        .and_then(|row| row["stable_id"].as_str())
        .map(DeclarationId::new)
        .ok_or_else(|| refused("migration.state_role"))
}
fn flat_state(
    program: &hir::ResolvedProgram,
    id: &DeclarationId,
) -> Result<Vec<(DeclarationId, ResolvedType)>> {
    let item = program
        .types
        .iter()
        .find(|item| item.id == *id)
        .ok_or_else(|| refused("migration.state_type"))?;
    let ResolvedTypeDeclarationKind::Record { fields } = &item.kind else {
        return Err(refused("migration.state_record"));
    };
    let persistent = |id: &DeclarationId| {
        program
            .declarations
            .declaration(id)
            .is_some_and(|entry| entry.identity_origin == hir::IdentityOrigin::Explicit)
    };
    if !persistent(id)
        || fields.iter().any(|field| !persistent(&field.id))
        || !item.type_parameters.is_empty()
        || fields.is_empty()
        || fields.len() > 32
        || fields.iter().any(|field| {
            !matches!(
                field.ty,
                ResolvedType::Bool
                    | ResolvedType::I32
                    | ResolvedType::I64
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::Bytes
            )
        })
    {
        return Err(refused("migration.flat_state"));
    }
    Ok(fields
        .iter()
        .map(|field| (field.id.clone(), field.ty.clone()))
        .collect())
}
fn evaluate_migration(
    program: &hir::ResolvedProgram,
    function: &str,
    old_state: &DeclarationId,
    new_state: &DeclarationId,
    value: &RetainedValue,
    max_steps: usize,
) -> Result<RetainedValue> {
    let call = prepare_migration_call(program, function, old_state, new_state)?;
    let first = evaluate_retained_call(program, &call, std::slice::from_ref(value), max_steps)?;
    let second = evaluate_retained_call(program, &call, std::slice::from_ref(value), max_steps)?;
    if first.outcome != second.outcome {
        return Err(refused("migration.replay"));
    }
    let RetainedCallOutcome::Returned(value) = first.outcome else {
        return Err(refused("migration.did_not_return"));
    };
    if !matches!(&value, RetainedValue::Record(record) if record.record == *new_state)
        || crate::agent_lifecycle::encode_value(&value).len() > 262_144
    {
        return Err(refused("migration.result_state"));
    }
    Ok(value)
}

/// Identical checked call boundary for fresh evaluation and trusted recovery.
fn prepare_migration_call(
    program: &hir::ResolvedProgram,
    function: &str,
    old_state: &DeclarationId,
    new_state: &DeclarationId,
) -> Result<crate::interpreter::retained_call::PreparedRetainedCall> {
    let call = prepare_retained_call(program, function)?;
    let entry = program
        .functions
        .iter()
        .find(|entry| entry.id.as_str() == function)
        .ok_or_else(|| refused("migration.function"))?;
    let nominal = |id: &DeclarationId| ResolvedType::Nominal {
        declaration: id.clone(),
        arguments: Vec::new(),
    };
    if entry.params.len() != 1
        || !(entry.params[0].ownership == hir::OwnershipMode::Own
            || (entry.params[0].ownership == hir::OwnershipMode::Value
                && program
                    .declarations
                    .type_facts(&nominal(old_state))
                    .is_some_and(|facts| facts.copy)))
        || entry.params[0].ty != nominal(old_state)
        || entry.return_type != nominal(new_state)
        || call.function_ids().any(|id| {
            program
                .functions
                .iter()
                .find(|item| item.id.as_str() == id)
                .is_none_or(|item| !item.effects.is_empty())
        })
    {
        return Err(refused("migration.pure_signature"));
    }
    Ok(call)
}

#[cfg(test)]
#[path = "typed_migration_tests.rs"]
mod tests;
