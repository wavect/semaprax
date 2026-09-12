# codegen/native_emit/closure.rs

- CAPTURE_SLOTS · constant · L13-L13 — pub(super) const CAPTURE_SLOTS: usize = 8;
- enabled · function · L15-L17 — pub(super) fn enabled(program: &hir::ResolvedProgram) -> bool
- carrier_type · function · L19-L30 — pub(super) fn carrier_type(ty: &ResolvedType) -> Result<String, Diagnostic>
- entry_type · function · L32-L34 — fn entry_type(ty: &ResolvedType) -> Result<String, Diagnostic>
- emit_carrier_declarations · function · L39-L121 — pub(super) fn emit_carrier_declarations(
- pack · function · L123-L136 — pub(super) fn pack(ty: &ResolvedType, value: &str) -> Result<String, Diagnostic>
- unpack · function · L138-L151 — fn unpack(ty: &ResolvedType, cell: usize) -> Result<String, Diagnostic>
- closure_functions · function · L153-L160 — pub(super) fn closure_functions(
- thunk_symbol · function · L162-L168 — pub(super) fn thunk_symbol(id: &hir::ExpressionId) -> String
- reference_thunk_symbol · function · L170-L176 — pub(super) fn reference_thunk_symbol(id: &hir::ExpressionId) -> String
- write_thunk_signature · function · L178-L214 — fn write_thunk_signature(
- emit_thunk_prototypes_for_function · function · L216-L246 — fn emit_thunk_prototypes_for_function(
- emit_thunk_prototypes · function · L248-L265 — pub(super) fn emit_thunk_prototypes(
- emit_thunks · function · L267-L405 — pub(super) fn emit_thunks(
