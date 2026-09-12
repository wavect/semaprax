# cleanup_plan/validate.rs

- validate_program · function · L7-L21 — pub(crate) fn validate_program(program: &ResolvedProgram) -> Result<(), Diagnostic>
- validate_canonical_plan · function · L23-L56 — fn validate_canonical_plan(
- slots_equal · function · L58-L76 — fn slots_equal(actual: &[CleanupSlot], expected: &[CleanupSlot]) -> Result<bool, Diagnostic>
- noncanonical · function · L78-L83 — fn noncanonical(function: &DeclarationId, component: &str) -> Diagnostic
