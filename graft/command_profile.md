---
covers: []
---
# command_profile.rs

- MAX_FUNCTIONS · constant · L17-L17 — const MAX_FUNCTIONS: usize = 256;
- MAX_STABLE_ID_BYTES · constant · L18-L18 — const MAX_STABLE_ID_BYTES: usize = 128;
- CommandProfilePlan · struct · L21-L24 — pub(crate) struct CommandProfilePlan
- prepare · function · L27-L128 — pub(crate) fn prepare(program: &ResolvedProgram, command_id: &str) -> Result<Self, Diagnostic>
- function_id · function · L130-L132 — pub(crate) fn function_id(&self) -> &DeclarationId
- stdout_capacity · function · L134-L136 — pub(crate) const fn stdout_capacity(&self) -> u64
- validate_function · function · L139-L303 — fn validate_function(
- validate_stdout_external_argument · function · L305-L345 — fn validate_stdout_external_argument(
- internal_parameter · function · L347-L353 — fn internal_parameter(ty: &ResolvedType, ownership: OwnershipMode) -> bool
- internal_result · function · L355-L365 — fn internal_result(ty: &ResolvedType) -> bool
- internal_expression_type · function · L367-L369 — fn internal_expression_type(ty: &ResolvedType) -> bool
- require_explicit · function · L371-L386 — fn require_explicit(
- reject_call_cycles · function · L388-L421 — fn reject_call_cycles(
- visit · function · L391-L413 — fn visit(
- validate_id · function · L423-L437 — fn validate_id(id: &str) -> Result<(), Diagnostic>
- admission · function · L439-L441 — fn admission(message: impl Into<String>) -> Diagnostic
- capacity · function · L443-L445 — fn capacity(message: impl Into<String>) -> Diagnostic
- tests · module · L448-L506 — mod tests
- COMMAND · constant · L453-L467 — const COMMAND: &str = r#"
- resolved · function · L469-L472 — fn resolved(source: &str) -> crate::hir::ResolvedProgram
- exact_two_slice_bool_boundary_is_admitted · function · L475-L480 — fn exact_two_slice_bool_boundary_is_admitted()
- signature_and_non_command_stdout_authority_fail_closed · function · L483-L505 — fn signature_and_non_command_stdout_authority_fail_closed()
