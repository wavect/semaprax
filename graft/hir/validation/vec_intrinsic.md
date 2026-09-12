# hir/validation/vec_intrinsic.rs

- reject_reserved_identities · function · L5-L19 — pub(super) fn reject_reserved_identities(program: &ResolvedProgram) -> Result<(), Diagnostic>
- reject_reserved_declaration · function · L21-L31 — pub(super) fn reject_reserved_declaration(declaration: &Declaration) -> Result<(), Diagnostic>
- reject_reserved_function · function · L33-L43 — pub(super) fn reject_reserved_function(function: &ResolvedFunction) -> Result<(), Diagnostic>
- reject_reserved_template · function · L45-L57 — pub(super) fn reject_reserved_template(
- is_type · function · L59-L71 — pub(super) fn is_type(
- is_owned_vec_carrier · function · L76-L79 — pub(super) fn is_owned_vec_carrier(program: &ResolvedProgram, ty: &ResolvedType) -> bool
- is_call · function · L81-L83 — pub(super) fn is_call(callee: &DeclarationId, instance: &Option<FunctionInstanceId>) -> bool
- signature · function · L85-L111 — pub(super) fn signature(
