# project/image_targets/openapi.rs

- SCHEMA · constant · L9-L9 — const SCHEMA: &str = "semaprax.project-openapi-build.v1";
- DOMAIN · constant · L10-L10 — const DOMAIN: &[u8] = b"semaprax.project-openapi-build.payload.v1\0";
- build_openapi_inline · function · L19-L60 — pub fn build_openapi_inline(&self, max_bytes: usize) -> Result<String, Vec<Diagnostic>>
- projection_build · function · L63-L79 — pub(super) fn projection_build(
- generate · function · L81-L190 — fn generate(revision: &ProjectRevision, max_bytes: usize) -> Result<String, Vec<Diagnostic>>
- HEX · constant · L168-L168 — const HEX: &[u8; 16] = b"0123456789abcdef";
- sha · function · L192-L197 — fn sha(bytes: &[u8]) -> String
