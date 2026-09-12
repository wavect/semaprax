---
covers: []
---
# package_build.rs

- admission · module · L8-L8 — mod admission;
- model · module · L9-L9 — mod model;
- wire · module · L10-L10 — pub(crate) mod wire;
- generate · function · L18-L32 — pub fn generate(
- verify · function · L34-L68 — pub fn verify(
- BuiltPackage · struct · L70-L73 — struct BuiltPackage
- build · function · L75-L191 — fn build(
- validate_wasm_inventory · function · L193-L255 — pub(crate) fn validate_wasm_inventory(
- artifact_bytes · function · L257-L264 — fn artifact_bytes(build: &OfflinePackageBuild) -> Result<usize, Diagnostic>
- artifact_bytes_with_limit · function · L266-L277 — fn artifact_bytes_with_limit(
- option_error · function · L279-L281 — fn option_error(message: impl Into<String>) -> Diagnostic
- authentication_error · function · L283-L285 — fn authentication_error(message: impl Into<String>) -> Diagnostic
- association_error · function · L287-L289 — fn association_error(message: impl Into<String>) -> Diagnostic
- profile_error · function · L291-L293 — fn profile_error(message: impl Into<String>) -> Diagnostic
- limit_error · function · L295-L297 — fn limit_error(message: impl Into<String>) -> Diagnostic
- wire_error · function · L299-L301 — fn wire_error(message: impl Into<String>) -> Diagnostic
- replay_error · function · L303-L305 — fn replay_error(message: impl Into<String>) -> Diagnostic
- map_nested_error · function · L307-L317 — fn map_nested_error(error: &Diagnostic, context: &str) -> Diagnostic
- tests · module · L320-L320 — mod tests;
