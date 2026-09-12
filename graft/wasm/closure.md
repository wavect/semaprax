# wasm/closure.rs

- adapter_types · function · L5-L24 — pub(super) fn adapter_types(
- append_adapters · function · L29-L65 — pub(super) fn append_adapters(
- load · function · L67-L75 — fn load(program: &ResolvedProgram, ty: &ResolvedType) -> Result<(u8, u32), Diagnostic>
- closure_profile · function · L78-L80 — pub(super) fn closure_profile(&self) -> bool
- closure_destination · function · L82-L84 — fn closure_destination(&self, expression: &ResolvedExpr) -> Result<Pointer, Diagnostic>
- emit_closure_reference · function · L86-L112 — pub(super) fn emit_closure_reference(
- emit_closure · function · L114-L134 — pub(super) fn emit_closure(&mut self, expression: &ResolvedExpr) -> Result<Value, Diagnostic>
- tests · module · L138-L198 — mod tests
- wasm_closures_failure_retains_status_output_sentinel_and_restores_frames · function · L142-L197 — fn wasm_closures_failure_retains_status_output_sentinel_and_restores_frames()
