# project/image_targets/c.rs

- SCHEMA · constant · L9-L9 — const SCHEMA: &str = "semaprax.project-c-build.v1";
- DOMAIN · constant · L10-L10 — const DOMAIN: &[u8] = b"semaprax.project-c-build.payload.v1\0";
- NATIVE_PATH · constant · L11-L11 — const NATIVE_PATH: &str = "native/entry.c";
- build_c_inline · function · L20-L61 — pub fn build_c_inline(&self, max_bytes: usize) -> Result<String, Vec<Diagnostic>>
- projection_build · function · L64-L80 — pub(super) fn projection_build(
- ReplayedCarrier · struct · L85-L88 — pub(in crate::project) struct ReplayedCarrier
- replay_carrier · function · L94-L370 — pub(in crate::project) fn replay_carrier(
- require_keys · function · L372-L386 — fn require_keys(
- decode_artifact · function · L388-L422 — fn decode_artifact(row: &Value, max_bytes: usize) -> Result<Vec<u8>, Vec<Diagnostic>>
- verify_exports · function · L424-L542 — fn verify_exports(
- generate · function · L544-L694 — fn generate(revision: &ProjectRevision, max_bytes: usize) -> Result<String, Vec<Diagnostic>>
- artifact · function · L696-L725 — fn artifact(
- HEX · constant · L716-L716 — const HEX: &[u8; 16] = b"0123456789abcdef";
- sha · function · L726-L731 — fn sha(bytes: &[u8]) -> String
