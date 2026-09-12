# wasm/internal_strings.rs

- admission · module · L5-L5 — mod admission;
- runtime · module · L6-L6 — mod runtime;
- web · module · L7-L7 — mod web;
- tests · module · L10-L10 — mod tests;
- SCHEMA · constant · L21-L21 — pub const SCHEMA: &str = "semaprax.wasm-internal-strings.v1";
- RUNTIME_SCHEMA · constant · L23-L23 — pub const RUNTIME_SCHEMA: &str = "semaprax.wasm-internal-strings.runtime.v1";
- InternalStringOptions · struct · L27-L33 — pub struct InternalStringOptions
- default · function · L36-L43 — fn default() -> Self
- InternalStringModule · struct · L48-L52 — pub struct InternalStringModule
- wasm_bytes · function · L55-L57 — pub fn wasm_bytes(&self) -> &[u8]
- descriptor · function · L58-L60 — pub fn descriptor(&self) -> &str
- runtime_source · function · L61-L63 — pub fn runtime_source(&self) -> &str
- Export · struct · L66-L70 — pub(super) struct Export
- error · function · L72-L74 — pub(super) fn error(message: impl Into<String>) -> Diagnostic
- emit_module · function · L79-L150 — pub fn emit_module(
- scalar_name · function · L152-L158 — fn scalar_name(ty: &ResolvedType) -> &'static str
- PreparedSelection · type · L160-L160 — type PreparedSelection = (Vec<Export>, BTreeSet<DeclarationId>);
