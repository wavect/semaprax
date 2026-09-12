# project/admission/native_callback.rs

- MAX_PARAMETERS · constant · L24-L24 — const MAX_PARAMETERS: usize = 8;
- admission_error · function · L26-L28 — fn admission_error(message: impl Into<String>) -> Diagnostic
- declares_callback · function · L35-L41 — pub(super) fn declares_callback(program: &ResolvedProgram) -> bool
- scalar · function · L43-L45 — const fn scalar(ty: &ResolvedType) -> bool
- prepare · function · L48-L142 — pub(super) fn prepare(program: &ResolvedProgram, selected: &[String]) -> Result<(), Diagnostic>
