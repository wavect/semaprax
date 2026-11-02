use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};

#[cfg(test)]
use super::build::build_plan;
use super::{CleanupPlan, CleanupSlot};

#[cfg(test)]
pub(in crate::cleanup_plan) mod owned;
pub(crate) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_phase = crate::workflow_profile::plan_comparison::active_paid_phase(
        crate::workflow_profile::plan_comparison::PaidPhase::CleanupValidation,
    );

    let mut frontier = crate::loop_calls::byte_writer::validation_frontier::Frontier::new(program);
    for function in &program.functions {
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_canonical = crate::workflow_profile::plan_comparison::active_paid_phase(
            crate::workflow_profile::plan_comparison::PaidPhase::CanonicalRebuild,
        );
        let current = frontier.current()?;
        let expected = super::build::helper_index::validation::build_current(program, &current)?;
        validate_canonical_plan(&function.id, &function.cleanup_plan, &expected)?;
        drop(expected);
        current.finish()?;
        frontier.advance()?;
    }
    for instance in &program.function_instances {
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_canonical = crate::workflow_profile::plan_comparison::active_paid_phase(
            crate::workflow_profile::plan_comparison::PaidPhase::CanonicalRebuild,
        );
        let current = frontier.current()?;
        let expected = super::build::helper_index::validation::build_current(program, &current)?;
        validate_canonical_plan(
            &instance.template,
            &instance.function.cleanup_plan,
            &expected,
        )?;
        drop(expected);
        current.finish()?;
        frontier.advance()?;
    }
    drop(frontier);
    // Complete fresh independent replay never borrows frontier proofs.
    super::replay::validate_program(program)
}

fn validate_canonical_plan(
    function: &DeclarationId,
    actual: &CleanupPlan,
    expected: &CleanupPlan,
) -> Result<(), Diagnostic> {
    if actual.schema != expected.schema {
        return Err(noncanonical(function, "schema"));
    }
    if actual.entry != expected.entry {
        return Err(noncanonical(function, "entry block"));
    }
    if actual.entry_state != expected.entry_state {
        return Err(noncanonical(function, "entry liveness state"));
    }
    if !slots_equal(&actual.slots, &expected.slots)? {
        return Err(noncanonical(function, "storage slot sequence"));
    }
    if actual.status_sources != expected.status_sources {
        return Err(noncanonical(function, "status-source sequence"));
    }
    if actual.blocks != expected.blocks {
        return Err(noncanonical(function, "block sequence"));
    }
    if actual.edges != expected.edges {
        return Err(noncanonical(function, "edge sequence"));
    }
    if actual.regions != expected.regions {
        return Err(noncanonical(function, "cleanup-region sequence"));
    }
    if actual.exits != expected.exits {
        return Err(noncanonical(function, "exit sequence"));
    }
    if actual.lexical_control != expected.lexical_control {
        return Err(noncanonical(function, "lexical-control certificate"));
    }
    Ok(())
}

fn slots_equal(actual: &[CleanupSlot], expected: &[CleanupSlot]) -> Result<bool, Diagnostic> {
    if actual.len() != expected.len() {
        return Ok(false);
    }
    for (actual, expected) in actual.iter().zip(expected) {
        if actual.id != expected.id
            || actual.storage != expected.storage
            || actual.ty != expected.ty
            || actual.storage_index != expected.storage_index
            || !crate::cleanup::field_liveness_shapes_equal(
                &actual.field_liveness_shape,
                &expected.field_liveness_shape,
            )?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn noncanonical(function: &DeclarationId, component: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-H006",
        format!("cleanup plan for function `{function}` has a non-canonical {component}"),
    )
}

#[cfg(test)]
mod frontier_tests;
