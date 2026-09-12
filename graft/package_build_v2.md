---
covers: []
---
# package_build_v2.rs

- admission · module · L7-L7 — mod admission;
- model · module · L8-L8 — mod model;
- wire · module · L9-L9 — mod wire;
- generate · function · L17-L37 — pub fn generate(
- verify · function · L40-L75 — pub fn verify(
- BuiltPackage · struct · L77-L81 — struct BuiltPackage
- build · function · L84-L205 — fn build(
- artifact_bytes · function · L207-L214 — fn artifact_bytes(build: &LinkedOfflinePackageBuild) -> Result<usize, Diagnostic>
- artifact_bytes_with_limit · function · L215-L227 — fn artifact_bytes_with_limit(
- option_error · function · L228-L230 — fn option_error(message: impl Into<String>) -> Diagnostic
- authentication_error · function · L231-L233 — fn authentication_error(message: impl Into<String>) -> Diagnostic
- association_error · function · L234-L236 — fn association_error(message: impl Into<String>) -> Diagnostic
- profile_error · function · L237-L239 — fn profile_error(message: impl Into<String>) -> Diagnostic
- limit_error · function · L240-L242 — fn limit_error(message: impl Into<String>) -> Diagnostic
- wire_error · function · L243-L245 — fn wire_error(message: impl Into<String>) -> Diagnostic
- replay_error · function · L246-L248 — fn replay_error(message: impl Into<String>) -> Diagnostic
- map_compiler_error · function · L249-L251 — fn map_compiler_error(_: &Diagnostic) -> Diagnostic
- map_nested_error · function · L252-L263 — fn map_nested_error(error: &Diagnostic) -> Diagnostic
- tests · module · L266-L266 — mod tests;
