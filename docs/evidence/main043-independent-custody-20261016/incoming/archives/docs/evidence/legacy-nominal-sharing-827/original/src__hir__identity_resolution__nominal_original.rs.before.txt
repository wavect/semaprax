//! Complete old two-pass receiver backed by the independent pre827 index.
use super::*;
fn pass(program: &Program) -> Result<(ResolvedProgram, FunctionResolutionWork), Diagnostic> {
    (Resolver {
        program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(program),
        declarations: DeclarationIndex::nominal_original_with_prelude(
            program,
            crate::prelude::declarations_for_program(program),
        )?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve_agent_identity_original()
}
pub(crate) fn resolve(
    program: &Program,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Diagnostic> {
    let clause_program = record_invariants::clause_program(program);
    let inventory = clause_program.as_ref().unwrap_or(program);
    let resolved = pass(inventory)?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass(&desugared)
}
