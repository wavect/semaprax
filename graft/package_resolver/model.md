# package_resolver/model.rs

- bf · function · L19-L21 — macro_rules! bf
- ParsedRequirement · struct · L24-L29 — pub(super) struct ParsedRequirement
- validate_input · function · L31-L65 — pub(super) fn validate_input(
- validate_identity · function · L67-L77 — pub(super) fn validate_identity(value: &str, label: &str) -> Result<(), Diagnostic>
- validate_values · function · L79-L94 — fn validate_values(values: &[String], maximum: usize, label: &str) -> Result<(), Diagnostic>
- render_evidence · function · L96-L165 — pub(super) fn render_evidence(
- recheck_lock_policy · function · L167-L204 — pub(super) fn recheck_lock_policy(lock: &str, input: &ResolutionInput) -> Result<(), Diagnostic>
- exact_lock_bytes · function · L206-L222 — pub(super) fn exact_lock_bytes(evidence: &str) -> Result<&str, Diagnostic>
- START · constant · L207-L207 — const START: &str = "\"lock\":";
- structural_json_value_end · function · L224-L261 — fn structural_json_value_end(bytes: &[u8], start: usize) -> Result<usize, Diagnostic>
- catalog_digest · function · L263-L276 — pub(super) fn catalog_digest<'a>(entries: impl Iterator<Item = &'a str>, count: usize) -> String
