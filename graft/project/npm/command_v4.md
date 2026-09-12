# project/npm/command_v4.rs

- LINE_COMMAND_IO_PACKAGE_PATHS · constant · L19-L27 — pub(super) const LINE_COMMAND_IO_PACKAGE_PATHS: [&str; 7] = [
- prepare · function · L29-L95 — pub(super) fn prepare(
- render_package · function · L97-L117 — fn render_package(name: &str, version: &str, command: &str, wasm: &[u8]) -> [NpmArtifact; 7]
- render_runtime · function · L119-L134 — fn render_runtime(wasm_sha256: &str, command: &str) -> String
- raw_symbol · function · L136-L143 — fn raw_symbol(stable_id: &str) -> String
- render_adapter · function · L145-L151 — fn render_adapter() -> String
- validate_replayed · function · L153-L194 — pub(super) fn validate_replayed(
- artifact_bytes · function · L196-L202 — fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 7], path: &str) -> Result<&'a [u8], Diagnostic>
- tests · module · L206-L206 — mod tests;
