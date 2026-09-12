# hir/validation/box_intrinsic.rs

- reject_reserved_identities · function · L3-L37 — pub(super) fn reject_reserved_identities(program: &ResolvedProgram) -> Result<(), Diagnostic>
- authenticate_owned_wrapper · function · L38-L46 — pub(super) fn authenticate_owned_wrapper(
- reject_function · function · L47-L59 — fn reject_function(function: &ResolvedFunction) -> Result<(), Diagnostic>
- is_call · function · L60-L65 — pub(super) fn is_call(callee: &DeclarationId, instance: &Option<FunctionInstanceId>) -> bool
- is_intrinsic_id · function · L66-L70 — pub(super) fn is_intrinsic_id(callee: &DeclarationId) -> bool
- is_type · function · L71-L83 — pub(super) fn is_type(
- intrinsic_signature · function · L87-L101 — pub(super) fn intrinsic_signature(
- signature · function · L104-L141 — pub(super) fn signature(
