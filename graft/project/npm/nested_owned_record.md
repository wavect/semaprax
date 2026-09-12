# project/npm/nested_owned_record.rs

- PACKAGE_PATHS · constant · L19-L26 — pub const PACKAGE_PATHS: [&str; 6] = [
- prepare · function · L28-L114 — pub(super) fn prepare(
- validate_replayed · function · L116-L184 — pub(super) fn validate_replayed(
- render_package · function · L186-L221 — fn render_package(
- render_runtime_prelude · function · L223-L232 — fn render_runtime_prelude(wasm_digest: &str, capacity: u32) -> String
- render_facts · function · L234-L253 — fn render_facts(descriptor: &NestedOwnedRecordApiDescriptor) -> Result<String, Diagnostic>
- render_typescript · function · L255-L303 — fn render_typescript(descriptor: &NestedOwnedRecordApiDescriptor) -> Result<String, Diagnostic>
- subject · function · L305-L312 — fn subject(descriptor: &NestedOwnedRecordApiDescriptor) -> PublicApiSubject<'_>
- parameter_wire · function · L313-L320 — fn parameter_wire(value: PublicApiParameterType) -> &'static str
- parameter_ts · function · L321-L328 — fn parameter_ts(value: PublicApiParameterType) -> &'static str
- leaf_wire · function · L329-L336 — fn leaf_wire(value: NestedOwnedRecordLeafType) -> &'static str
- raw_symbol · function · L337-L344 — fn raw_symbol(id: &str) -> String
- artifact_bytes · function · L345-L351 — fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic>
- tests · module · L354-L425 — mod tests
- v11_runtime_has_one_preflight_copy_commit_and_post_settlement_publication · function · L360-L375 — fn v11_runtime_has_one_preflight_copy_commit_and_post_settlement_publication()
- v11_package_replays_two_owned_occurrences_without_legacy_widening · function · L378-L424 — fn v11_package_replays_two_owned_occurrences_without_legacy_widening()
