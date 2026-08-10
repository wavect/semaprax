use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};

use super::build::build_plan;
use super::CleanupPlan;

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
    if actual.slots != expected.slots {
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
    if actual != expected {
        return Err(noncanonical(function, "representation"));
    }
    Ok(())
}

fn noncanonical(function: &DeclarationId, component: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-H006",
        format!("cleanup plan for function `{function}` has a non-canonical {component}"),
    )
}
