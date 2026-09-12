# project/npm/network_command.rs

- PACKAGE_PATHS · constant · L20-L27 — pub(super) const PACKAGE_PATHS: [&str; 6] = [
- prepare · function · L29-L95 — pub(super) fn prepare(
- render_package · function · L97-L121 — fn render_package(name: &str, version: &str, command: &str, wasm: &[u8]) -> [NpmArtifact; 6]
- raw_symbol · function · L123-L130 — fn raw_symbol(stable_id: &str) -> String
- validate_replayed · function · L132-L168 — pub(super) fn validate_replayed(
- artifact_bytes · function · L170-L176 — fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic>
