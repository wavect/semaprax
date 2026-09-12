# project/npm/command_v2.rs

- USEFUL_DATA_COMMAND_V2_PACKAGE_PATHS · constant · L18-L19 — pub(super) const USEFUL_DATA_COMMAND_V2_PACKAGE_PATHS: [&str; 7] =
- prepare · function · L21-L82 — pub(super) fn prepare(
- require_profile · function · L84-L101 — fn require_profile(manifest: &ProjectManifest) -> Result<&str, Diagnostic>
- render_metadata · function · L103-L111 — fn render_metadata(name: &str, version: &str, command: &str, wasm_sha256: &str) -> String
- render_package · function · L116-L143 — fn render_package(
- replace_once · function · L145-L152 — fn replace_once(source: String, from: &str, to: &str) -> Result<String, Diagnostic>
- artifact_text · function · L154-L157 — fn artifact_text(artifacts: &[NpmArtifact; 7], path: &str) -> Result<String, Diagnostic>
- replace_artifact · function · L159-L170 — fn replace_artifact(
- render_command_adapter · function · L172-L226 — fn render_command_adapter(command: &str) -> String
- validate_replayed · function · L228-L305 — pub(super) fn validate_replayed(
- replay_manifest · function · L307-L320 — fn replay_manifest(
- artifact_bytes · function · L322-L328 — fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 7], path: &str) -> Result<&'a [u8], Diagnostic>
- tests · module · L331-L346 — mod tests
- metadata_v2_is_one_exact_canonical_line · function · L335-L345 — fn metadata_v2_is_one_exact_canonical_line()
