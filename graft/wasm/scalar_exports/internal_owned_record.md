# wasm/scalar_exports/internal_owned_record.rs

- validate_program · function · L6-L73 — pub(super) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic>
- validate_instance · function · L75-L99 — pub(super) fn validate_instance(
- validate_generic_closure · function · L101-L155 — fn validate_generic_closure(program: &ResolvedProgram) -> Result<(), Diagnostic>
- collect_generic_calls · function · L157-L177 — fn collect_generic_calls(
- validate_expression · function · L179-L312 — pub(super) fn validate_expression(
- record_pattern_is_exact · function · L314-L318 — fn record_pattern_is_exact(program: &ResolvedProgram, pattern: &ResolvedMatchPattern) -> bool
- internal_type · function · L320-L330 — fn internal_type(program: &ResolvedProgram, ty: &ResolvedType) -> bool
- body_error · function · L332-L336 — fn body_error(function_id: &DeclarationId) -> Diagnostic
