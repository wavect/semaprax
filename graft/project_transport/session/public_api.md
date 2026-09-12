# project_transport/session/public_api.rs

- DEFAULT_INLINE_NPM_BYTES · constant · L19-L19 — const DEFAULT_INLINE_NPM_BYTES: usize = 8 * 1024 * 1024;
- Descriptor · enum · L21-L26 — enum Descriptor
- derive · function · L29-L45 — fn derive(snapshot: &ProjectSnapshot) -> Result<Self, Vec<Diagnostic>>
- descriptor_schema · function · L47-L54 — fn descriptor_schema(&self) -> &'static str
- carrier_schema · function · L56-L63 — fn carrier_schema(&self) -> &'static str
- canonical_bytes · function · L65-L73 — fn canonical_bytes(&self) -> Vec<u8>
- digest · function · L75-L81 — fn digest(&self) -> String
- verify_carrier · function · L83-L94 — fn verify_carrier(&self, build: &ProjectNpmBuild) -> Result<(), Diagnostic>
- Description · struct · L97-L102 — struct Description
- derive · function · L105-L121 — fn derive(snapshot: &ProjectSnapshot) -> Result<Self, Vec<Diagnostic>>
- render · function · L123-L125 — fn render(&self) -> String
- render_prefix · function · L128-L137 — fn render_prefix(description: &Description) -> String
- render_fields · function · L139-L145 — fn render_fields(description: &Description, build: Option<&str>) -> String
- public_api_describe · function · L148-L157 — pub(super) fn public_api_describe(
- public_npm_build_inline · function · L159-L200 — pub(super) fn public_npm_build_inline(
- tests · module · L204-L220 — mod tests
- v6_success_wrapper_budget_is_exact_and_never_truncates · function · L208-L219 — fn v6_success_wrapper_budget_is_exact_and_never_truncates()
