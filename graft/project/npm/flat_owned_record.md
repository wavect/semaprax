# project/npm/flat_owned_record.rs

- PACKAGE_PATHS · constant · L14-L21 — pub const PACKAGE_PATHS: [&str; 6] = [
- prepare · function · L23-L112 — pub(super) fn prepare(
- validate_replayed · function · L114-L175 — pub(super) fn validate_replayed(
- render_package · function · L177-L214 — fn render_package(
- render_facade · function · L216-L223 — fn render_facade(descriptor: &FlatOwnedRecordApiDescriptor) -> String
- raw_symbol · function · L225-L232 — fn raw_symbol(stable_id: &str) -> String
- artifact_bytes · function · L233-L239 — fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic>
- hostile_source_tests · module · L242-L295 — mod hostile_source_tests
- v9_package_selects_shared_preflight_before_snapshot_and_arena_effects · function · L248-L294 — fn v9_package_selects_shared_preflight_before_snapshot_and_arena_effects()
