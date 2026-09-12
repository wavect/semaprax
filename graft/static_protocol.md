---
covers: []
---
# static_protocol.rs

- SCHEMA · constant · L13-L13 — pub const SCHEMA: &str = "semaprax.static-protocol-conformance.v1";
- MAX_IMPLEMENTATIONS · constant · L14-L14 — pub const MAX_IMPLEMENTATIONS: usize = 256;
- MAX_IMPLEMENTATION_MEMBERS · constant · L15-L15 — pub const MAX_IMPLEMENTATION_MEMBERS: usize = 256;
- MAX_TOTAL_MEMBERS · constant · L16-L16 — pub const MAX_TOTAL_MEMBERS: usize = 4096;
- MAX_STABLE_ID_BYTES · constant · L17-L17 — pub const MAX_STABLE_ID_BYTES: usize = 240;
- MAX_FACT_BYTES · constant · L18-L18 — pub const MAX_FACT_BYTES: usize = 4 * 1024 * 1024;
- MAX_METHOD_PARAMETERS · constant · L19-L19 — pub const MAX_METHOD_PARAMETERS: usize = 64;
- valid_binding_id · function · L23-L29 — pub fn valid_binding_id(id: &str) -> bool
- member_matches · function · L34-L64 — pub fn member_matches(
- validate · function · L69-L275 — pub fn validate(program: &Program) -> Result<(), Diagnostic>
- require_import · function · L277-L291 — fn require_import(program: &Program, kind: ModuleUseKind, id: &str) -> Result<(), Diagnostic>
- validate_mapping · function · L293-L331 — fn validate_mapping<'a>(
- facts · function · L334-L339 — pub fn facts(program: &Program) -> Result<Value, Vec<Diagnostic>>
- validate_workspace · function · L343-L511 — pub(crate) fn validate_workspace(programs: &[Program]) -> Result<(), Diagnostic>
- require_workspace_import · function · L513-L537 — fn require_workspace_import(
- workspace_member_matches · function · L539-L577 — fn workspace_member_matches(
- workspace_type_key · function · L579-L632 — fn workspace_type_key(
- visit_ids · function · L634-L690 — fn visit_ids<'a>(
- declaration_facts · function · L694-L775 — pub(crate) fn declaration_facts(program: &Program) -> Result<Value, Vec<Diagnostic>>
- error · function · L777-L779 — fn error(code: &'static str, message: &'static str) -> Diagnostic
- bounded_type_label · function · L781-L789 — fn bounded_type_label(ty: &Type) -> Result<String, Vec<Diagnostic>>
