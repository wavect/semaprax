//! Checked, consuming migration from an actual durable Suspend into a new runtime.
//! Persisted recovery uses caller-trusted snapshots bound to exact runtime roots.
#[path = "typed_migration/durable.rs"]
mod durable;
#[path = "typed_migration/handoff.rs"]
mod handoff;
#[path = "typed_migration/linked.rs"]
mod linked;
use super::*;
use crate::agent_lifecycle::iterative::effects::TargetStageBackend;
use crate::agent_lifecycle::iterative::IterativeStatus;
use crate::agent_runtime_v2::checkpoint::CheckpointUsage;
use crate::hir::{self, DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
use crate::interpreter::retained_call::{
    evaluate_retained_call, prepare_retained_call, RetainedCallEvaluation, RetainedCallOutcome,
    RetainedValue,
};
pub use durable::{
    resume_migrated_agent_runtime_v2, DurableMigrationFailure,
    MeteredAgentRuntimeV2DurableMigrationEvidence, ResumedMigratedAgentRuntimeV2,
};

/// Retained source identities selected by the existing checked typed migration
/// association.  The live-source migration facade uses these only after
/// `linked::programs` has validated the exact Project/agent relationship.
pub(crate) struct LinkedSourceIdentity {
    pub(crate) source_path: String,
    pub(crate) agent_id: String,
}

pub(crate) fn validate_linked_source_migration(
    previous: &AgentRuntimeV2,
    destination: &AgentRuntimeV2,
    migration: &str,
) -> Result<(LinkedSourceIdentity, LinkedSourceIdentity)> {
    let _ = linked::programs(previous, destination, migration)?;
    Ok((
        retained_source_identity(previous)?,
        retained_source_identity(destination)?,
    ))
}

fn retained_source_identity(runtime: &AgentRuntimeV2) -> Result<LinkedSourceIdentity> {
    let deployment: serde_json::Value = serde_json::from_str(runtime.deployment.canonical_json())
        .map_err(|_| refused("migration.deployment"))?;
    let source_path = deployment["facts"]["source_path"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| refused("migration.source_path"))?;
    let agent_id = deployment["facts"]["agent_id"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| refused("migration.agent_id"))?;
    Ok(LinkedSourceIdentity {
        source_path: source_path.to_owned(),
        agent_id: agent_id.to_owned(),
    })
}

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
    /// Local fixture only: exercises the destination-side durable/backend
    /// parity routes directly against a hand-built seed, without the full
    /// checked migration/handoff pipeline this type is otherwise only ever
    /// produced through.
    #[cfg(test)]
    pub(crate) fn for_test(
        value: RetainedValue,
        binding: ExecutionRoot,
        usage: CheckpointUsage,
        iterations: usize,
        stages: usize,
        max_reserved_fuel: u64,
    ) -> Self {
        Self {
            value,
            binding,
            usage,
            iterations,
            stages,
            max_reserved_fuel,
        }
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
        self.run_selected(handler, cancellation, None)
    }

    /// Continue the checked migrated State on an explicitly held stage target.
    /// This selects destination stages only: the pure migration call has already
    /// completed on the interpreter. Usage retains reservation accounting.
    pub fn run_with_backend(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        selected: TargetStageBackend<'_>,
    ) -> std::result::Result<AgentRuntimeV2MigrationEvidence, AgentRuntimeV2MigrationFailure> {
        self.run_selected(handler, cancellation, Some(selected))
    }

    fn run_selected(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        selected: Option<TargetStageBackend<'_>>,
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
                selected,
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
    migrate_suspended_agent_runtime_v2_inner(
        previous,
        suspended,
        destination,
        expected_previous_revision,
        expected_destination_revision,
        migration_function,
        max_migration_steps,
        max_reserved_fuel,
        None,
    )
}

/// Consume a durable suspension and evaluate its checked pure migration on an
/// explicitly held target. The selected execution is metered independently of
/// interpreter instruction steps, and its receipt is bound into the durable
/// migration root before the destination handoff can be committed.
#[allow(clippy::too_many_arguments)]
pub fn migrate_suspended_agent_runtime_v2_with_backend(
    previous: AgentRuntimeV2,
    suspended: AgentRuntimeV2DurableEvidence,
    destination: AgentRuntimeV2,
    expected_previous_revision: &str,
    expected_destination_revision: &str,
    migration_function: &str,
    max_migration_steps: usize,
    max_reserved_fuel: u64,
    selected: TargetStageBackend<'_>,
    semantic_fuel_limit: u64,
) -> std::result::Result<MigratedAgentRuntimeV2, AgentRuntimeV2MigrationFailure> {
    migrate_suspended_agent_runtime_v2_inner(
        previous,
        suspended,
        destination,
        expected_previous_revision,
        expected_destination_revision,
        migration_function,
        max_migration_steps,
        max_reserved_fuel,
        Some((selected, semantic_fuel_limit)),
    )
}

#[allow(clippy::too_many_arguments)]
fn migrate_suspended_agent_runtime_v2_inner(
    previous: AgentRuntimeV2,
    suspended: AgentRuntimeV2DurableEvidence,
    destination: AgentRuntimeV2,
    expected_previous_revision: &str,
    expected_destination_revision: &str,
    migration_function: &str,
    max_migration_steps: usize,
    max_reserved_fuel: u64,
    selected: Option<(TargetStageBackend<'_>, u64)>,
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
        // A caller-selected backend must prove its retained source and closed
        // semantic profile before migration fuel is reserved. In particular,
        // a linked runtime without a single retained Wasm source cannot spend
        // the reservation and then fall back to the interpreter.
        let selected_call = if let Some((selected, semantic_fuel_limit)) = selected {
            let call =
                prepare_migration_call(&new_program, migration_function, &old_state, &new_state)?;
            destination
                .lifecycle
                .validate_target_retained_call_metered(
                    &new_program,
                    &call,
                    selected,
                    semantic_fuel_limit,
                )?;
            Some(call)
        } else {
            None
        };
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
        let (migrated, target_execution) = match selected {
            Some((selected, semantic_fuel_limit)) => evaluate_migration_on_target(
                &destination.lifecycle,
                &new_program,
                selected_call
                    .as_ref()
                    .ok_or_else(|| refused("migration.target_call"))?,
                &new_state,
                value,
                max_migration_steps,
                selected,
                semantic_fuel_limit,
            )?,
            None => (
                evaluate_migration(
                    &new_program,
                    migration_function,
                    &old_state,
                    &new_state,
                    value,
                    max_migration_steps,
                )?,
                None,
            ),
        };
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
        if let Some(target_execution) = target_execution {
            facts["target_execution"] = target_execution;
        }
        let schema = if facts.get("target_execution").is_some() {
            "semaprax.agent-state-migration.v4"
        } else {
            schema
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
pub(crate) fn flat_state(
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
pub(crate) fn evaluate_migration(
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

#[allow(clippy::too_many_arguments)]
fn evaluate_migration_on_target(
    lifecycle: &crate::agent_lifecycle::iterative::effects::CompiledTypedEffects,
    program: &hir::ResolvedProgram,
    call: &crate::interpreter::retained_call::PreparedRetainedCall,
    new_state: &DeclarationId,
    value: &RetainedValue,
    max_steps: usize,
    selected: TargetStageBackend<'_>,
    semantic_fuel_limit: u64,
) -> Result<(RetainedValue, Option<serde_json::Value>)> {
    let first = lifecycle.execute_target_retained_call_metered(
        program,
        call,
        std::slice::from_ref(value),
        max_steps,
        selected,
        semantic_fuel_limit,
    )?;
    let second = lifecycle.execute_target_retained_call_metered(
        program,
        call,
        std::slice::from_ref(value),
        max_steps,
        selected,
        semantic_fuel_limit,
    )?;
    if first.execution_binding != second.execution_binding
        || first.evaluation.outcome != second.evaluation.outcome
    {
        return Err(refused("migration.replay"));
    }
    let first_facts = target_evaluation_facts(&first.evaluation)?;
    let second_facts = target_evaluation_facts(&second.evaluation)?;
    // The target's instruction counter is an implementation observation, but
    // semantic work and copy-out cleanup are part of the deterministic pure
    // migration contract. A differing receipt must not become a durable
    // handoff that recovery could treat as already settled.
    if first_facts["semantic_work"] != second_facts["semantic_work"]
        || first_facts["copy_out_cleanup_events"] != second_facts["copy_out_cleanup_events"]
    {
        return Err(refused("migration.replay"));
    }
    let RetainedCallOutcome::Returned(value) = first.evaluation.outcome else {
        return Err(refused("migration.did_not_return"));
    };
    if !matches!(&value, RetainedValue::Record(record) if record.record == *new_state)
        || crate::agent_lifecycle::encode_value(&value).len() > 262_144
    {
        return Err(refused("migration.result_state"));
    }
    Ok((
        value,
        Some(json!({
            "execution_binding": first.execution_binding,
            "semantic_fuel_limit": semantic_fuel_limit,
            "evaluations": [
                first_facts,
                second_facts,
            ],
        })),
    ))
}

fn target_evaluation_facts(evaluation: &RetainedCallEvaluation) -> Result<serde_json::Value> {
    let work = evaluation
        .semantic_work
        .as_ref()
        .ok_or_else(|| refused("migration.semantic_work"))?;
    let finalizers = work.finalizer_events.as_ref().map(|events| {
        events
            .iter()
            .map(|event| json!([event.function.as_str(), event.liveness_flag]))
            .collect::<Vec<_>>()
    });
    Ok(json!({
        "instruction_steps": evaluation.steps_used,
        "semantic_work": {
            "fuel_used": work.fuel_used,
            "fuel_limit": work.fuel_limit,
            "exhausted": work.exhausted,
            "finalizer_events": finalizers,
        },
        "copy_out_cleanup_events": evaluation.cleanup_events.len(),
    }))
}

/// Identical checked call boundary for fresh evaluation and trusted recovery.
pub(crate) fn prepare_migration_call(
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
