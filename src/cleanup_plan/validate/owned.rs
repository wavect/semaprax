//! Complete original unindexed rebuild/compare/replay validator.
use super::*;
pub(in crate::cleanup_plan) fn validate_program_unindexed_reference(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
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
    super::scratch_ledger::validate_program(program)
}
