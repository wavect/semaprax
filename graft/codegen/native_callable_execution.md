# codegen/native_callable_execution.rs

- NORMALIZED_CALLABLE_SYMBOL · constant · L23-L23 — const NORMALIZED_CALLABLE_SYMBOL: &str = "spx_callable_projection";
- NORMALIZED_CONTRACT · constant · L24-L24 — const NORMALIZED_CONTRACT: [u8; 32] = [0xa5; 32];
- NORMALIZED_TARGET_GUARDS · constant · L25-L25 — const NORMALIZED_TARGET_GUARDS: &str = "/* semaprax.native-callable-provider-target-guards.v1 */\n";
- STATUS_CAPACITY · constant · L26-L26 — const STATUS_CAPACITY: u32 = 1;
- ExecutionParameter · struct · L29-L32 — struct ExecutionParameter
- NativeCallableExecutionPlan · struct · L37-L47 — pub(super) struct NativeCallableExecutionPlan
- ConcreteCallableProvider · struct · L49-L53 — pub(super) struct ConcreteCallableProvider
- plan · function · L56-L180 — pub(super) fn plan(
- provider_spec · function · L183-L196 — fn provider_spec(
- normalized_projection · function · L198-L213 — pub(super) fn normalized_projection(&self) -> Result<(String, [u8; 32]), Diagnostic>
- emit_concrete · function · L215-L247 — pub(super) fn emit_concrete(
- emit_hook · function · L249-L382 — fn emit_hook(&self, hook_symbol: &str) -> Result<String, Diagnostic>
- unique_result_commit_ordinal · function · L385-L400 — fn unique_result_commit_ordinal(dictionary: &SemanticEventDictionary) -> Result<u32, Diagnostic>
- normalize_concrete_projection · function · L402-L436 — fn normalize_concrete_projection(
- replace_required · function · L438-L450 — fn replace_required(
- contract_declaration · function · L452-L459 — fn contract_declaration(contract: [u8; 32]) -> String
- execution_error · function · L461-L466 — fn execution_error(message: impl Into<String>) -> Diagnostic
