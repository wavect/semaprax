# wasm/aggregate/function_value.rs

- TablePlan · struct · L13-L19 — pub(super) struct TablePlan
- table_plan · function · L24-L72 — pub(super) fn table_plan(program: &ResolvedProgram) -> Result<TablePlan, Diagnostic>
- abi_signature · function · L74-L92 — pub(super) fn abi_signature(
- type_indexes · function · L94-L113 — pub(super) fn type_indexes(
- table_indexes · function · L115-L129 — pub(super) fn table_indexes(
- execution_target · function · L131-L133 — pub(super) fn execution_target(target: &ResolvedFunction) -> FunctionExecutionId
- callable_signature · function · L135-L142 — pub(super) fn callable_signature(expr: &ResolvedExpr) -> Result<&ResolvedType, Diagnostic>
- emit_function_reference · function · L145-L166 — pub(super) fn emit_function_reference(
- emit_function_invoke · function · L168-L275 — pub(super) fn emit_function_invoke(
- program_uses_byte_range · function · L278-L297 — pub(super) fn program_uses_byte_range(program: &ResolvedProgram) -> bool
- program_uses_owned_buffer · function · L299-L318 — pub(in crate::wasm) fn program_uses_owned_buffer(program: &ResolvedProgram) -> bool
- hex_identity · function · L320-L327 — pub(super) fn hex_identity(id: &DeclarationId) -> String
- hex_execution_identity · function · L329-L339 — pub(super) fn hex_execution_identity(id: &FunctionExecutionId) -> String
- vec_import_base · function · L341-L353 — pub(in crate::wasm) fn vec_import_base(program: &ResolvedProgram) -> u32
- box_import_base · function · L355-L377 — pub(in crate::wasm) fn box_import_base(program: &ResolvedProgram) -> u32
- executable_functions · function · L379-L398 — pub(super) fn executable_functions(
