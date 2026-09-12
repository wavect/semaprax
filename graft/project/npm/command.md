# project/npm/command.rs

- USEFUL_DATA_COMMAND_PACKAGE_PATHS · constant · L18-L26 — pub(super) const USEFUL_DATA_COMMAND_PACKAGE_PATHS: [&str; 7] = [
- prepare · function · L28-L79 — pub(super) fn prepare(
- require_profile · function · L81-L93 — fn require_profile(manifest: &ProjectManifest) -> Result<&str, Diagnostic>
- validate_command · function · L95-L140 — pub(super) fn validate_command<'a>(
- render_package · function · L142-L155 — fn render_package(
- render_package_with_metadata · function · L160-L191 — pub(super) fn render_package_with_metadata(
- replace_once · function · L193-L200 — fn replace_once(source: String, from: &str, to: &str) -> Result<String, Diagnostic>
- render_runtime · function · L202-L209 — fn render_runtime(wasm_sha256: &str) -> Result<String, Diagnostic>
- render_bindings · function · L211-L252 — fn render_bindings(exports: &[data::DataExport], wasm_sha256: &str) -> Result<String, Diagnostic>
- render_metadata · function · L254-L259 — fn render_metadata(name: &str, version: &str, command: &str, wasm_sha256: &str) -> String
- render_command_adapter · function · L261-L290 — fn render_command_adapter(command: &str) -> String
- render_package_json · function · L292-L297 — fn render_package_json(name: &str, version: &str) -> String
- validate_replayed · function · L299-L347 — pub(super) fn validate_replayed(
- metadata_selected_exports · function · L349-L355 — fn metadata_selected_exports(value: &serde_json::Value) -> Result<Vec<String>, Diagnostic>
- replay_manifest · function · L357-L368 — fn replay_manifest(
- artifact_bytes · function · L370-L376 — fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 7], path: &str) -> Result<&'a [u8], Diagnostic>
