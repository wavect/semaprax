# package_resolver_v2/model.rs

- bf · function · L19-L21 — macro_rules! bf
- ParsedRequirement · struct · L24-L29 — pub(super) struct ParsedRequirement
- validate_input · function · L31-L68 — pub(super) fn validate_input(
- validate_identity · function · L70-L80 — pub(super) fn validate_identity(value: &str, label: &str) -> Result<(), Diagnostic>
- validate_values · function · L82-L97 — fn validate_values(values: &[String], maximum: usize, label: &str) -> Result<(), Diagnostic>
- render_evidence · function · L99-L168 — pub(super) fn render_evidence(
- recheck_lock_policy · function · L170-L207 — pub(super) fn recheck_lock_policy(lock: &str, input: &ResolutionInput) -> Result<(), Diagnostic>
- exact_lock_bytes · function · L209-L225 — pub(super) fn exact_lock_bytes(evidence: &str) -> Result<&str, Diagnostic>
- START · constant · L210-L210 — const START: &str = "\"lock\":";
- structural_json_value_end · function · L227-L264 — fn structural_json_value_end(bytes: &[u8], start: usize) -> Result<usize, Diagnostic>
- catalog_digest · function · L266-L279 — pub(super) fn catalog_digest<'a>(entries: impl Iterator<Item = &'a str>, count: usize) -> String
