use super::*;

pub(super) fn validate_function(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    if crate::hir::iterator_loop::function_has_record_renewal_outside_loop(program, function) {
        return Err(hir_error(
            "record owner renewal is admitted only inside a bounded while body",
        ));
    }
    Ok(())
}

pub(super) fn assignment_is_admitted(
    program: &ResolvedProgram,
    binding: &ResolvedBinding,
    assigned: &ResolvedExpr,
) -> bool {
    crate::vec_ops::is_same_owner_reassignment_hir(program, assigned, &binding.id)
        || crate::stdin_stream_ops::hir_reopen(assigned, &binding.id)
        || crate::byte_ops::is_same_owner_set_hir(assigned, &binding.id)
        || crate::string_ops::is_same_owner_concat_hir(assigned, &binding.id)
        || crate::hir::iterator_loop::is_step_reassignment(assigned, &binding.id)
        || crate::hir::iterator_loop::is_record_owner_renewal(program, binding, assigned)
}
