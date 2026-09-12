# project/npm/command_v3.rs

- LANGUAGE_COMMAND_IO_PACKAGE_PATHS · constant · L19-L27 — pub(super) const LANGUAGE_COMMAND_IO_PACKAGE_PATHS: [&str; 7] = [
- prepare · function · L29-L97 — pub(super) fn prepare(
- render_package · function · L99-L119 — fn render_package(name: &str, version: &str, command: &str, wasm: &[u8]) -> [NpmArtifact; 7]
- render_runtime · function · L121-L136 — fn render_runtime(wasm_sha256: &str, command: &str) -> String
- raw_symbol · function · L138-L145 — fn raw_symbol(stable_id: &str) -> String
- render_adapter · function · L147-L153 — fn render_adapter() -> String
- validate_replayed · function · L155-L196 — pub(super) fn validate_replayed(
- artifact_bytes · function · L198-L204 — fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 7], path: &str) -> Result<&'a [u8], Diagnostic>
- tests · module · L208-L208 — mod tests;
