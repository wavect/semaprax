//! Checked Project-to-Project migration into the source execution journal.
//! A destination store acknowledges the pure-evaluation reservation before
//! any migration code runs. This module does not authenticate store freshness.

#![allow(clippy::items_after_test_module)]

use super::*;
use crate::execution_revision::typed::migration::{
    evaluate_migration, flat_state, prepare_migration_call,
};
use crate::hir::{DeclarationId, ResolvedType};
use crate::interpreter::retained_call::{RetainedField, RetainedRecord};
use crate::live_invocation::source_journal::{
    recover_source_checkpoint, source_migration_state_digest, PolicyMigrationCarryV6,
    PricedMigrationCarryV4, SourceJournalEntry, SourceMigrationCarry, SourceMigrationFailure,
    SourcePolicyBindingV6, MAX_SOURCE_CARRIER_BYTES,
};
use crate::project::{HotReloadSourceAgentHandoff, ProjectRevision};
use serde_json::Value;

/// The retained Project and compiled source lifecycle at one side of a handoff.
/// A v3 predecessor binding is the opaque binding originally returned by a
/// checked preparation; submitted checkpoint bytes cannot select one.
pub struct SourceLiveMigrationEndpoint<'a> {
    pub project: &'a ProjectRevision,
    pub source_path: &'a str,
    pub agent_id: &'a str,
    pub lifecycle: &'a CompiledIterativeLifecycle,
    pub policy: &'a SourceLivePolicy,
    pub budget: IterativeBudget,
}

pub struct SourceLiveMigrationRequest<'a> {
    pub previous: SourceLiveMigrationEndpoint<'a>,
    pub previous_binding: &'a SourceInvocationBinding,
    pub previous_checkpoint: &'a str,
    pub destination: SourceLiveMigrationEndpoint<'a>,
    pub task: &'a LifecycleTask,
    pub migration_function: &'a str,
    pub max_migration_steps: usize,
    /// An independently retained handoff expectation; it cannot be copied
    /// from the untrusted destination checkpoint being recovered.
    pub expected_handoff_digest: Option<&'a str>,
}

/// The checked root domain which migration carries into the destination
/// binding. `Typed` is crate-private so only the retained typed migration
/// facade can supply its already-validated derived roots.
pub(crate) enum InvocationRootProfile {
    Source,
    Typed {
        previous_program_root: String,
        destination_program_root: String,
    },
}

pub struct PreparedSourceLiveMigration<'a> {
    destination: SourceLiveMigrationEndpoint<'a>,
    task: &'a LifecycleTask,
    function: String,
    program: crate::hir::ResolvedProgram,
    old_state_id: DeclarationId,
    new_state_id: DeclarationId,
    previous_state: RetainedValue,
    binding: SourceInvocationBinding,
    carry: SourceMigrationCarry,
    checkpoint: Option<&'a str>,
    max_migration_steps: usize,
}

fn semantic_failure(diagnostics: Vec<Diagnostic>) -> SourceLiveFailure {
    SourceLiveFailure {
        diagnostics,
        selected: Some(SourceTerminalStatus::Rejected),
        checked_run: None,
        checkpoint: None,
        stage_rows: Vec::new(),
        model_dispatches: 0,
        effect_dispatches: 0,
        journal_error: None,
    }
}
fn refused(reason: &str) -> SourceLiveFailure {
    semantic_failure(vec![bad(reason)])
}
fn checked<T>(result: Result<T, Vec<Diagnostic>>) -> Result<T, SourceLiveFailure> {
    result.map_err(semantic_failure)
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    let map = value.as_object()?;
    (map.len() == fields.len() && fields.iter().all(|field| map.contains_key(*field)))
        .then_some(map)
}
fn decode_leaf(value: &Value, ty: &ResolvedType) -> Option<RetainedValue> {
    match ty {
        ResolvedType::Bool => Some(RetainedValue::Bool(value.as_bool()?)),
        ResolvedType::I32 => Some(RetainedValue::I32(value.as_str()?.parse().ok()?)),
        ResolvedType::I64 => Some(RetainedValue::I64(value.as_str()?.parse().ok()?)),
        ResolvedType::U8 => Some(RetainedValue::U8(value.as_str()?.parse().ok()?)),
        ResolvedType::Usize => Some(RetainedValue::Usize(value.as_str()?.parse().ok()?)),
        ResolvedType::Bytes => {
            let bytes = exact_object(value, &["bytes"])?.get("bytes")?.as_str()?;
            if bytes.len() % 2 != 0
                || bytes.len() > MAX_SOURCE_CARRIER_BYTES * 2
                || !bytes.is_ascii()
                || !bytes.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return None;
            }
            let decoded = (0..bytes.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&bytes[index..index + 2], 16).ok())
                .collect::<Option<Vec<_>>>()?;
            Some(RetainedValue::Bytes(decoded))
        }
        _ => None,
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;

    #[test]
    fn non_ascii_bytes_carrier_is_rejected_without_slicing_panic() {
        let bytes = serde_json::json!({"bytes": "aéb"});
        assert_eq!(decode_leaf(&bytes, &ResolvedType::Bytes), None);
        let bytes = serde_json::json!({"bytes": "é"});
        assert_eq!(decode_leaf(&bytes, &ResolvedType::Bytes), None);
    }
}
fn decode_state(
    bytes: &[u8],
    id: &DeclarationId,
    shape: &[(DeclarationId, ResolvedType)],
) -> Result<RetainedValue, SourceLiveFailure> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_CARRIER_BYTES {
        return Err(refused("migration.state_capacity"));
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| refused("migration.state_json"))?;
    let map = exact_object(&value, &["record", "fields"])
        .ok_or_else(|| refused("migration.state_shape"))?;
    if map.get("record").and_then(Value::as_str) != Some(id.as_str()) {
        return Err(refused("migration.state_identity"));
    }
    let items = map
        .get("fields")
        .and_then(Value::as_array)
        .filter(|items| items.len() == shape.len())
        .ok_or_else(|| refused("migration.state_fields"))?;
    let mut fields = Vec::with_capacity(shape.len());
    for (item, (field_id, ty)) in items.iter().zip(shape) {
        let row = exact_object(item, &["field", "value"])
            .ok_or_else(|| refused("migration.state_field"))?;
        if row.get("field").and_then(Value::as_str) != Some(field_id.as_str()) {
            return Err(refused("migration.state_field_identity"));
        }
        let value = row
            .get("value")
            .and_then(|value| decode_leaf(value, ty))
            .ok_or_else(|| refused("migration.state_field_type"))?;
        fields.push(RetainedField {
            field: field_id.clone(),
            value,
        });
    }
    let retained = RetainedValue::Record(RetainedRecord {
        record: id.clone(),
        fields,
    });
    if encode_value(&retained).as_bytes() != bytes {
        return Err(refused("migration.state_noncanonical"));
    }
    Ok(retained)
}

fn selected_project_program(
    endpoint: &SourceLiveMigrationEndpoint<'_>,
    expected_policy_root: Option<&str>,
    typed_profile: bool,
) -> Result<(String, String), SourceLiveFailure> {
    let root = checked(endpoint.project.program_root())?
        .program_root()
        .to_owned();
    let expected_policy_root = expected_policy_root.unwrap_or(root.as_str());
    if endpoint.policy.program_root.as_deref() != Some(expected_policy_root) {
        return Err(refused("migration.project_root"));
    }
    let definition = endpoint
        .project
        .agent_definitions()
        .iter()
        .find(|candidate| {
            candidate.definition().agent_id() == endpoint.agent_id
                && candidate.definition().digest() == endpoint.lifecycle.inner.definition_digest
        })
        .ok_or_else(|| refused("migration.retained_agent"))?;
    let definition_source = definition.definition().canonical_source().to_owned();
    let selected = checked(endpoint.project.linked_agent_program(
        endpoint.source_path,
        endpoint.agent_id,
        &definition_source,
    ))?;
    if !typed_profile {
        let compiled = checked(
            crate::agent_lifecycle::iterative::compile_linked_agent_lifecycle(
                selected,
                &definition_source,
                endpoint.lifecycle.step.id.as_str(),
            ),
        )?;
        if compiled.digest() != endpoint.lifecycle.digest()
            || compiled.canonical_json() != endpoint.lifecycle.canonical_json()
        {
            return Err(refused("migration.lifecycle_drift"));
        }
    }
    // A Typed profile carries the retained deployment-bound typed-effects
    // lifecycle, which is intentionally not byte-equal to this generic
    // lifecycle compilation. Its caller first verifies that exact typed
    // lifecycle through `typed::migration::linked`; this path still resolves
    // the raw Project definition and linked source association above.
    Ok((root, definition_source))
}

pub fn prepare_source_live_migration<'a>(
    request: SourceLiveMigrationRequest<'a>,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    prepare_source_live_migration_inner(request, None, None, None, InvocationRootProfile::Source)
}

/// Consumes compiler-derived hot-reload selection facts through the ordinary
/// authenticated source migration path. The facts restrict endpoint selection;
/// they never provide checkpoint, store, or dispatch authority.
pub fn prepare_source_live_migration_from_hot_reload_handoff<'a>(
    request: SourceLiveMigrationRequest<'a>,
    handoff: &HotReloadSourceAgentHandoff,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    if !handoff.matches_endpoints(
        request.previous.project,
        request.destination.project,
        request.previous.agent_id,
        request.destination.agent_id,
    ) {
        return Err(refused("migration.hot_reload_selection"));
    }
    prepare_source_live_migration(request)
}

/// Preserves checked monetary history through the existing migration evaluator.
pub fn prepare_source_live_priced_migration<'a>(
    request: SourceLiveMigrationRequest<'a>,
    previous_pricing: &SourceLivePricing,
    destination_pricing: &SourceLivePricing,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    prepare_source_live_migration_inner(
        request,
        Some((previous_pricing, destination_pricing)),
        None,
        None,
        InvocationRootProfile::Source,
    )
}

/// Carries acknowledged provider I/O reservations through checked migration.
pub fn prepare_source_live_migration_with_io_limits<'a>(
    request: SourceLiveMigrationRequest<'a>,
    previous_pricing: Option<&SourceLivePricing>,
    destination_pricing: Option<&SourceLivePricing>,
    previous_limits: &SourceIoLimits,
    destination_limits: &SourceIoLimits,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    let pricing = match (previous_pricing, destination_pricing) {
        (Some(previous), Some(destination)) => Some((previous, destination)),
        (None, None) => None,
        _ => return Err(refused("migration.pricing_profile")),
    };
    prepare_source_live_migration_inner(
        request,
        pricing,
        Some((previous_limits, destination_limits)),
        None,
        InvocationRootProfile::Source,
    )
}

/// V6 policy migration retains nonrefundable policy exposure and the next
/// global ordinal. Provider changes are refused; source/deployment policy
/// identities may change only through this checked migration authority.
pub(crate) fn prepare_source_live_policy_migration<'a>(
    request: SourceLiveMigrationRequest<'a>,
    previous_policy: &SourcePolicyBindingV6,
    destination_policy: &SourcePolicyBindingV6,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    prepare_source_live_migration_inner(
        request,
        None,
        None,
        Some((previous_policy, destination_policy)),
        InvocationRootProfile::Source,
    )
}

/// Composes V6 policy carry with the existing V5 I/O carry. Both predecessor
/// profiles and both destination ceilings are verified by the one migration
/// preparation path before it creates any destination checkpoint.
pub(crate) fn prepare_source_live_policy_migration_with_io_limits<'a>(
    request: SourceLiveMigrationRequest<'a>,
    previous_policy: &SourcePolicyBindingV6,
    destination_policy: &SourcePolicyBindingV6,
    previous_limits: &SourceIoLimits,
    destination_limits: &SourceIoLimits,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    prepare_source_live_migration_inner(
        request,
        None,
        Some((previous_limits, destination_limits)),
        Some((previous_policy, destination_policy)),
        InvocationRootProfile::Source,
    )
}

/// The typed durable facade supplies retained, derived roots after it has
/// checked its typed migration association. Raw projects remain independently
/// validated by the shared preparation path.
pub(crate) fn prepare_source_live_policy_migration_profiled<'a>(
    request: SourceLiveMigrationRequest<'a>,
    previous_policy: &SourcePolicyBindingV6,
    destination_policy: &SourcePolicyBindingV6,
    profile: InvocationRootProfile,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    prepare_source_live_migration_inner(
        request,
        None,
        None,
        Some((previous_policy, destination_policy)),
        profile,
    )
}

/// Composes typed V6 policy carry and I/O carry without admitting caller
/// supplied root strings to the public source migration route.
pub(crate) fn prepare_source_live_policy_migration_with_io_limits_profiled<'a>(
    request: SourceLiveMigrationRequest<'a>,
    previous_policy: &SourcePolicyBindingV6,
    destination_policy: &SourcePolicyBindingV6,
    previous_limits: &SourceIoLimits,
    destination_limits: &SourceIoLimits,
    profile: InvocationRootProfile,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    prepare_source_live_migration_inner(
        request,
        None,
        Some((previous_limits, destination_limits)),
        Some((previous_policy, destination_policy)),
        profile,
    )
}

fn prepare_source_live_migration_inner<'a>(
    request: SourceLiveMigrationRequest<'a>,
    pricing: Option<(&SourceLivePricing, &SourceLivePricing)>,
    io: Option<(&SourceIoLimits, &SourceIoLimits)>,
    policy: Option<(&SourcePolicyBindingV6, &SourcePolicyBindingV6)>,
    profile: InvocationRootProfile,
) -> Result<PreparedSourceLiveMigration<'a>, SourceLiveFailure> {
    if pricing.is_some() && policy.is_some() {
        return Err(refused("migration.profile"));
    }
    if request.previous_binding.priced_binding().is_some() != pricing.is_some() {
        return Err(refused("migration.pricing_profile"));
    }
    if request.previous_binding.policy_binding().is_some() != policy.is_some() {
        return Err(refused("migration.policy_profile"));
    }
    if request.previous_binding.io_limits() != io.map(|(previous, _)| previous) {
        return Err(refused("migration.io_profile"));
    }
    let typed_roots = match &profile {
        InvocationRootProfile::Source => None,
        InvocationRootProfile::Typed {
            previous_program_root,
            destination_program_root,
        } => Some((
            previous_program_root.as_str(),
            destination_program_root.as_str(),
        )),
    };
    let typed_profile = matches!(&profile, InvocationRootProfile::Typed { .. });
    let (previous_selected_root, _) = selected_project_program(
        &request.previous,
        typed_roots.map(|roots| roots.0),
        typed_profile,
    )?;
    let (destination_selected_root, destination_definition) = selected_project_program(
        &request.destination,
        typed_roots.map(|roots| roots.1),
        typed_profile,
    )?;
    if previous_selected_root == destination_selected_root {
        return Err(refused("migration.unchanged_project"));
    }
    let (previous_root, destination_root) = match profile {
        InvocationRootProfile::Source => (previous_selected_root, destination_selected_root),
        InvocationRootProfile::Typed {
            previous_program_root,
            destination_program_root,
        } => (previous_program_root, destination_program_root),
    };
    let previous_expected = match (pricing, policy) {
        (_, Some((previous_policy, _))) => request.previous.policy.binding_with_model_policy(
            request.previous.lifecycle,
            request.task,
            request.previous.budget,
            previous_policy.clone(),
        ),
        (Some((previous_pricing, _)), None) => request.previous.policy.binding_priced(
            request.previous.lifecycle,
            request.task,
            request.previous.budget,
            previous_pricing,
        ),
        (None, None) => request.previous.policy.binding(
            request.previous.lifecycle,
            request.task,
            request.previous.budget,
        ),
    }
    .map_err(|error| SourceLiveFailure::initial(error, None))?;
    let previous_expected = if let Some((limits, _)) = io {
        previous_expected
            .with_io_limits(limits.clone(), None)
            .map_err(|error| SourceLiveFailure::initial(error, None))?
    } else {
        previous_expected
    };
    if request
        .previous_binding
        .priced_binding()
        .map(|bound| bound.pricing())
        != previous_expected
            .priced_binding()
            .map(|bound| bound.pricing())
    {
        return Err(refused("migration.previous_pricing"));
    }
    if request.previous_binding.migration().is_none() {
        if request.previous_binding != &previous_expected {
            return Err(refused("migration.previous_binding"));
        }
    } else {
        let carry = request.previous_binding.migration().expect("checked above");
        if carry.destination_program_root != previous_root
            || carry.task_digest
                != crate::live_invocation::source_journal::source_migration_task_digest(
                    &request.task.objective,
                    request.task.budget,
                )
            || !request.previous_binding.matches_proposal_source(
                request.previous.lifecycle.source_revision(),
                &request.previous.policy.deployment_binding,
                &request.task.objective,
                request.task.budget,
                request
                    .previous
                    .lifecycle
                    .proposal_schema()
                    .schema()
                    .digest(),
            )
            || request.previous_binding.ceiling() != request.previous.policy.ceiling
            || request.previous_binding.reservation_units()
                != request.previous.policy.reservation_units
            || request.previous_binding.response_limit() != request.previous.policy.response_limit
            || request.previous_binding.unit() != request.previous.policy.unit
            || request.previous_binding.deadline_millis() != request.previous.policy.deadline_millis
            || request.previous_binding.clock_domain() != request.previous.policy.clock_domain
            || request.previous_binding.max_iterations()
                != request.previous.budget.max_iterations as u32
            || request.previous_binding.max_stages() != request.previous.budget.max_stages as u32
            || request.previous_binding.max_steps_per_stage()
                != Some(request.previous.budget.max_steps_per_stage)
            || request.previous_binding.max_total_steps()
                != Some(request.previous.policy.max_total_steps)
        {
            return Err(refused("migration.previous_binding"));
        }
    }
    let previous = recover_source_checkpoint(request.previous_checkpoint, request.previous_binding)
        .map_err(|error| SourceLiveFailure::initial(error, None))?;
    if previous.is_uncertain() {
        return Err(SourceLiveFailure::initial(
            SourceJournalError::Uncertain,
            Some(previous),
        ));
    }
    let terminal = previous
        .terminal_snapshot()
        .ok_or_else(|| refused("migration.requires_terminal"))?;
    if terminal.status() != SourceTerminalStatus::Suspend {
        return Err(refused("migration.requires_suspend"));
    }
    let old_state_id = request
        .previous
        .lifecycle
        .inner
        .binding
        .type_id("state")
        .clone();
    let new_state_id = request
        .destination
        .lifecycle
        .inner
        .binding
        .type_id("state")
        .clone();
    let old_shape = checked(flat_state(
        &request.previous.lifecycle.inner.program,
        &old_state_id,
    ))?;
    let previous_state = decode_state(
        terminal
            .carrier()
            .ok_or_else(|| refused("migration.state_missing"))?,
        &old_state_id,
        &old_shape,
    )?;
    let selected = checked(request.destination.project.linked_agent_migration_program(
        request.destination.source_path,
        request.destination.agent_id,
        &destination_definition,
        request.migration_function,
    ))?;
    if checked(flat_state(&selected.program, &old_state_id))? != old_shape
        || checked(flat_state(&selected.program, &new_state_id))?
            != checked(flat_state(
                &request.destination.lifecycle.inner.program,
                &new_state_id,
            ))?
    {
        return Err(refused("migration.state_schema_drift"));
    }
    checked(prepare_migration_call(
        &selected.program,
        request.migration_function,
        &old_state_id,
        &new_state_id,
    ))
    .map(|_| ())?;
    let terminal_turn = match previous.entries().last() {
        Some(SourceJournalEntry::TerminalSnapshot {
            turn: Some(turn), ..
        }) => *turn,
        _ => return Err(refused("migration.suspend_turn")),
    };
    let carried_turns = terminal_turn
        .checked_add(1)
        .ok_or_else(|| refused("migration.turn_overflow"))?;
    let (stages, effects, attempts) = match previous.entries().last() {
        Some(SourceJournalEntry::TerminalSnapshot {
            stages,
            effects,
            attempts,
            ..
        }) => (*stages, *effects, *attempts),
        _ => unreachable!(),
    };
    let predecessor_schema = request.previous_binding.schema();
    let mut carry = SourceMigrationCarry {
        handoff_digest: String::new(),
        previous_schema: predecessor_schema.to_owned(),
        previous_invocation: previous.invocation().to_owned(),
        previous_generation: previous.generation(),
        previous_chain: previous.chain().to_owned(),
        previous_program_root: previous_root,
        destination_program_root: destination_root,
        old_state_id: old_state_id.as_str().to_owned(),
        new_state_id: new_state_id.as_str().to_owned(),
        migration_function: request.migration_function.to_owned(),
        migration_closure: selected.revision,
        task_digest: crate::live_invocation::source_journal::source_migration_task_digest(
            &request.task.objective,
            request.task.budget,
        ),
        carried_model_units: previous.committed_reserved_units(),
        carried_stage_fuel: previous.committed_stage_fuel(),
        carried_turns,
        carried_stages: stages,
        carried_effects: effects,
        carried_attempts: attempts,
        previous_ceiling: previous.ceiling(),
        previous_max_iterations: previous.max_iterations(),
        previous_max_stages: previous.max_stages(),
        previous_max_steps_per_stage: previous
            .max_steps_per_stage()
            .ok_or_else(|| refused("migration.previous_stage_profile"))?,
        previous_max_total_steps: previous
            .max_total_steps()
            .ok_or_else(|| refused("migration.previous_stage_profile"))?,
        previous_reservation_units: previous.reservation_units(),
        previous_unit: previous.unit().to_owned(),
        previous_clock_domain: previous.clock_domain().to_owned(),
        previous_last_checked_millis: previous.last_checked_millis(),
        previous_deadline_millis: previous.deadline_millis(),
        evaluation_steps: request.max_migration_steps,
    };
    carry.handoff_digest = carry.digest();
    let seed = request
        .destination
        .policy
        .seed(
            request.destination.lifecycle,
            request.task,
            request.destination.budget,
        )
        .map_err(|error| SourceLiveFailure::initial(error, None))?;
    let binding = match (pricing, policy) {
        (_, Some((_, destination_policy))) => {
            let policy_carry = PolicyMigrationCarryV6::from_predecessor(
                carry.clone(),
                request.previous_binding,
                &previous,
                destination_policy,
            )
            .map_err(|error| SourceLiveFailure::initial(error, None))?;
            let policy = SourceInvocationBinding::bind_policy_migrated_execution(
                seed,
                &SourceLivePolicy::evaluator_profile(),
                policy_carry,
                destination_policy.clone(),
            )
            .map_err(|error| SourceLiveFailure::initial(error, None))?;
            match io {
                Some((_, destination_limits)) => {
                    policy.with_io_limits(destination_limits.clone(), Some(&previous))
                }
                None => Ok(policy),
            }
        }
        (Some((_, destination_pricing)), None) => {
            let destination_pricing = destination_pricing
                .validated(&request.destination.policy.unit)
                .map_err(|error| SourceLiveFailure::initial(error, None))?;
            let priced_carry = PricedMigrationCarryV4::from_predecessor(
                carry.clone(),
                request.previous_binding,
                &previous,
                destination_pricing.clone(),
            )
            .map_err(|error| SourceLiveFailure::initial(error, None))?;
            if let Some((_, limits)) = io {
                SourceInvocationBinding::bind_io_priced_migrated_execution(
                    seed,
                    &SourceLivePolicy::evaluator_profile(),
                    priced_carry,
                    destination_pricing,
                    limits.clone(),
                    &previous,
                )
            } else {
                SourceInvocationBinding::bind_priced_migrated_execution(
                    seed,
                    &SourceLivePolicy::evaluator_profile(),
                    priced_carry,
                    destination_pricing,
                )
            }
        }
        (None, None) if io.is_some() => SourceInvocationBinding::bind_io_migrated_execution(
            seed,
            &SourceLivePolicy::evaluator_profile(),
            carry.clone(),
            io.expect("checked").1.clone(),
            &previous,
        ),
        (None, None) => SourceInvocationBinding::bind_migrated_execution(
            seed,
            &SourceLivePolicy::evaluator_profile(),
            carry.clone(),
        ),
    }
    .map_err(|error| SourceLiveFailure::initial(error, None))?;
    if request
        .expected_handoff_digest
        .is_some_and(|expected| Some(expected) != binding.migration_handoff_digest())
    {
        return Err(refused("migration.handoff_mismatch"));
    }
    Ok(PreparedSourceLiveMigration {
        destination: request.destination,
        task: request.task,
        function: request.migration_function.to_owned(),
        program: selected.program,
        old_state_id,
        new_state_id,
        previous_state,
        binding,
        carry,
        checkpoint: None,
        max_migration_steps: request.max_migration_steps,
    })
}

impl<'a> PreparedSourceLiveMigration<'a> {
    pub fn binding(&self) -> &SourceInvocationBinding {
        &self.binding
    }
    pub fn handoff_digest(&self) -> &str {
        self.binding
            .migration_handoff_digest()
            .expect("prepared migration has a handoff")
    }
    /// Supplies the destination's latest trusted snapshot under exclusive
    /// writer ownership. The expected handoff stays bound by preparation.
    pub fn with_checkpoint(mut self, checkpoint: &'a str) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    /// Consumes a checked Suspend into the same source driver and journal
    /// family. A recovered settled State is schema-checked and never
    /// re-evaluated; an unresolved pure intent requires a new charged ACK.
    pub fn run(
        self,
        source: &mut dyn driver::ProposalSource,
        read: &mut dyn AgentReadOperation,
        store: &mut dyn CheckpointStore,
        clock: &dyn SourceInvocationClock,
        cancellation: &AgentCancellation,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        let mut driver = driver::ReadDriver { read };
        self.run_with_driver(source, &mut driver, store, clock, cancellation)
    }

    /// Runs an already prepared migration through the checked driver supplied
    /// by a retained execution facade. The public source route above retains
    /// the ordinary read-driver adapter.
    pub(crate) fn run_with_driver(
        self,
        source: &mut dyn driver::ProposalSource,
        driver: &mut dyn driver::IterativeDriver,
        store: &mut dyn CheckpointStore,
        clock: &dyn SourceInvocationClock,
        cancellation: &AgentCancellation,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        if !source.checkpoint_policy().is_some_and(|policy| {
            policy.deployment_binding == self.destination.policy.deployment_binding
                && policy.response_limit == self.destination.policy.response_limit
                && policy.reservation_units == self.destination.policy.reservation_units
        }) {
            return Err(refused("migration.proposal_policy"));
        }
        let recovered = self
            .checkpoint
            .map(|bytes| recover_source_checkpoint(bytes, &self.binding))
            .transpose()
            .map_err(|error| SourceLiveFailure::initial(error, None))?;
        if let Some(checkpoint) = recovered.as_ref() {
            if checkpoint.terminal_snapshot().is_some() {
                return Ok(SourceLiveOutcome {
                    checked_run: None,
                    checkpoint: checkpoint.clone(),
                    model_dispatches: 0,
                    effect_dispatches: 0,
                });
            }
            if checkpoint.is_uncertain() {
                return Err(SourceLiveFailure::initial(
                    SourceJournalError::Uncertain,
                    recovered,
                ));
            }
        }
        let initial_floor = recovered.as_ref().map_or(
            self.carry.previous_last_checked_millis,
            RecoveredSourceCheckpoint::last_checked_millis,
        );
        gate(
            clock,
            cancellation,
            &self.binding,
            initial_floor,
            recovered.clone(),
        )?;
        let mut sink = match recovered {
            Some(checkpoint) => SourceCheckpointSink::resume(store, checkpoint)
                .map_err(|error| SourceLiveFailure::initial(error, None))?,
            None => SourceCheckpointSink::new(store, self.binding.clone()),
        };
        if sink.journal().entries().is_empty() {
            let entry = SourceJournalEntry::MigrationOpened {
                handoff_digest: self.handoff_digest().to_owned(),
            };
            sink.append_at(entry, clock.now_millis())
                .map_err(|error| sink_failure(error, &sink))?;
        }
        if matches!(
            sink.journal().entries().last(),
            Some(SourceJournalEntry::MigrationEvaluationFailed { .. })
        ) {
            return Err(refused_with_checkpoint("migration.checked_failure", &sink));
        }
        let migrated = if let Some(bytes) = sink.journal().entries().iter().find_map(|entry| {
            if let SourceJournalEntry::MigrationEvaluationSettled { state, .. } = entry {
                Some(state.as_slice())
            } else {
                None
            }
        }) {
            let shape = checked(flat_state(&self.program, &self.new_state_id))?;
            decode_state(bytes, &self.new_state_id, &shape)?
        } else {
            let attempt = sink
                .journal()
                .entries()
                .iter()
                .filter(|entry| {
                    matches!(entry, SourceJournalEntry::MigrationEvaluationIntent { .. })
                })
                .count();
            let attempt = u32::try_from(attempt)
                .map_err(|_| refused_with_checkpoint("migration.attempt_capacity", &sink))?;
            let fuel = usize::try_from(
                self.carry
                    .evaluation_fuel()
                    .map_err(|error| sink_failure(error, &sink))?,
            )
            .map_err(|_| refused_with_checkpoint("migration.fuel_capacity", &sink))?;
            gate(
                clock,
                cancellation,
                &self.binding,
                sink.journal().last_checked_millis(),
                sink.checkpoint().ok(),
            )?;
            let intent = SourceJournalEntry::MigrationEvaluationIntent { attempt, fuel };
            sink.preflight_at(&intent, clock.now_millis())
                .map_err(|error| sink_failure(error, &sink))?;
            sink.append_at(intent, clock.now_millis())
                .map_err(|error| sink_failure(error, &sink))?;
            if let Err(failure) = gate(
                clock,
                cancellation,
                &self.binding,
                sink.journal().last_checked_millis(),
                sink.checkpoint().ok(),
            ) {
                if failure.journal_error == Some(SourceJournalError::Time) {
                    return Err(failure);
                }
                let reason = if failure.selected == Some(SourceTerminalStatus::Cancelled) {
                    SourceMigrationFailure::Cancelled
                } else {
                    SourceMigrationFailure::DeadlineExceeded
                };
                sink.append_at(
                    SourceJournalEntry::MigrationEvaluationFailed { attempt, reason },
                    clock.now_millis(),
                )
                .map_err(|error| sink_failure(error, &sink))?;
                return Err(failure_with_checkpoint(failure, &sink));
            }
            let evaluated = evaluate_migration(
                &self.program,
                &self.function,
                &self.old_state_id,
                &self.new_state_id,
                &self.previous_state,
                self.max_migration_steps,
            );
            let value = match evaluated {
                Ok(value) => value,
                Err(diagnostics) => {
                    // The checked call has already failed, so its selection is
                    // sticky. Still sample the bound clock after evaluation.
                    // A regression cannot support a valid failure ACK; leave
                    // the pure intent unresolved and charged for recovery.
                    if let Err(refusal) = gate(
                        clock,
                        cancellation,
                        &self.binding,
                        sink.journal().last_checked_millis(),
                        sink.checkpoint().ok(),
                    ) {
                        if refusal.journal_error == Some(SourceJournalError::Time) {
                            let mut failure = semantic_failure(diagnostics);
                            failure.journal_error = Some(SourceJournalError::Time);
                            failure.checkpoint = sink.checkpoint().ok();
                            return Err(failure);
                        }
                    }
                    sink.append_at(
                        SourceJournalEntry::MigrationEvaluationFailed {
                            attempt,
                            reason: SourceMigrationFailure::CheckedCall,
                        },
                        clock.now_millis(),
                    )
                    .map_err(|error| sink_failure(error, &sink))?;
                    let mut failure = semantic_failure(diagnostics);
                    failure.checkpoint = sink.checkpoint().ok();
                    return Err(failure);
                }
            };
            if let Err(failure) = gate(
                clock,
                cancellation,
                &self.binding,
                sink.journal().last_checked_millis(),
                sink.checkpoint().ok(),
            ) {
                if failure.journal_error == Some(SourceJournalError::Time) {
                    return Err(failure);
                }
                let reason = if failure.selected == Some(SourceTerminalStatus::Cancelled) {
                    SourceMigrationFailure::Cancelled
                } else {
                    SourceMigrationFailure::DeadlineExceeded
                };
                sink.append_at(
                    SourceJournalEntry::MigrationEvaluationFailed { attempt, reason },
                    clock.now_millis(),
                )
                .map_err(|error| sink_failure(error, &sink))?;
                return Err(failure_with_checkpoint(failure, &sink));
            }
            let bytes = encode_value(&value).into_bytes();
            if bytes.len() > MAX_SOURCE_CARRIER_BYTES {
                sink.append_at(
                    SourceJournalEntry::MigrationEvaluationFailed {
                        attempt,
                        reason: SourceMigrationFailure::CheckedCall,
                    },
                    clock.now_millis(),
                )
                .map_err(|error| sink_failure(error, &sink))?;
                return Err(refused_with_checkpoint("migration.result_capacity", &sink));
            }
            sink.append_at(
                SourceJournalEntry::MigrationEvaluationSettled {
                    attempt,
                    state_digest: source_migration_state_digest(&bytes),
                    state: bytes,
                },
                clock.now_millis(),
            )
            .map_err(|error| sink_failure(error, &sink))?;
            value
        };
        gate(
            clock,
            cancellation,
            &self.binding,
            sink.journal().last_checked_millis(),
            sink.checkpoint().ok(),
        )?;
        let ledger = CumulativeBudgetLedger::resume_source_shared(
            &sink
                .checkpoint()
                .map_err(|error| sink_failure(error, &sink))?,
            clock,
        )
        .map_err(|_| refused_with_checkpoint("migration.ledger_restore", &sink))?;
        if self.binding.policy_binding().is_some() {
            let checkpoint = sink
                .checkpoint()
                .map_err(|error| sink_failure(error, &sink))?;
            source
                .restore_policy_checkpointed(checkpoint.policy_reservations())
                .map_err(|_| refused_with_checkpoint("migration.policy_restore", &sink))?;
        }
        let mut session = SourceExecutionSession::new(sink, ledger, clock, cancellation);
        if let Err(errors) = session.opened() {
            return Err(session.failure(None, errors));
        }
        let seed = SourceMigrationSeed {
            state: migrated,
            prior_turns: self.carry.carried_turns as usize,
            prior_stages: self.carry.carried_stages as usize,
            prior_effects: self.carry.carried_effects as usize,
        };
        match self.destination.lifecycle.run_with_driver_live_seed(
            self.task,
            source,
            driver,
            self.destination.budget,
            cancellation,
            Some(&mut session),
            Some(&seed),
            false,
        ) {
            Ok(run) => session.complete(Some(run), None, Vec::new()),
            Err(driver::DriverFailure::Diagnostics(errors)) => session.complete(None, None, errors),
            Err(driver::DriverFailure::Persistence {
                terminal,
                diagnostics,
            }) => session.complete(Some(*terminal), None, diagnostics),
        }
    }
}

fn sink_failure(error: SourceJournalError, sink: &SourceCheckpointSink<'_>) -> SourceLiveFailure {
    SourceLiveFailure::initial(error, sink.checkpoint().ok())
}
fn refused_with_checkpoint(reason: &str, sink: &SourceCheckpointSink<'_>) -> SourceLiveFailure {
    let mut failure = refused(reason);
    failure.checkpoint = sink.checkpoint().ok();
    failure
}
fn failure_with_checkpoint(
    mut failure: SourceLiveFailure,
    sink: &SourceCheckpointSink<'_>,
) -> SourceLiveFailure {
    failure.checkpoint = sink.checkpoint().ok();
    failure
}
fn gate(
    clock: &dyn SourceInvocationClock,
    cancellation: &AgentCancellation,
    binding: &SourceInvocationBinding,
    floor: i64,
    checkpoint: Option<RecoveredSourceCheckpoint>,
) -> Result<i64, SourceLiveFailure> {
    if cancellation.is_cancelled() {
        let mut failure = refused("migration.cancelled");
        failure.selected = Some(SourceTerminalStatus::Cancelled);
        failure.checkpoint = checkpoint;
        return Err(failure);
    }
    let now = clock.now_millis();
    if clock.clock_domain() != binding.clock_domain() || now < floor {
        return Err(SourceLiveFailure::initial(
            SourceJournalError::Time,
            checkpoint,
        ));
    }
    if now >= binding.deadline_millis() {
        let mut failure = refused("migration.deadline");
        failure.selected = Some(SourceTerminalStatus::DeadlineExceeded);
        failure.checkpoint = checkpoint;
        return Err(failure);
    }
    Ok(now)
}
