use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};

use super::build::build_plan;
use super::{CleanupPlan, CleanupSlot};

pub(crate) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in &program.functions {
        let expected = build_plan(program, function)?;
        validate_canonical_plan(&function.id, &function.cleanup_plan, &expected)?;
    }
    for instance in &program.function_instances {
        let expected = build_plan(program, &instance.function)?;
        validate_canonical_plan(
            &instance.template,
            &instance.function.cleanup_plan,
            &expected,
        )?;
    }
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
