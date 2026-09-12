# project/candidate/environment_review.rs

- Result · type · L20-L20 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA · constant · L22-L23 — pub const PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES · constant · L24-L27 — pub const MAX_PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_BYTES: usize =
- REPORT_DOMAIN · constant · L29-L29 — const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-environment-aware-review.v1\0";
- environment_aware_review · function · L36-L125 — pub fn environment_aware_review(
- parse_exact · function · L128-L148 — fn parse_exact(bytes: &str, schema: &str, limit: usize, owner: &'static str) -> Result<Value>
- validate_bindings · function · L150-L181 — fn validate_bindings(
- validate_source_inventory · function · L183-L216 — fn validate_source_inventory(candidate: &ProjectCandidate, value: &Value) -> Result<()>
- validate_changed_sources · function · L218-L267 — fn validate_changed_sources(candidate: &ProjectCandidate, value: &Value) -> Result<()>
- sha256 · function · L269-L274 — fn sha256(bytes: &str) -> String
- invalid · function · L276-L278 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L279-L281 — fn capacity(message: &'static str) -> Vec<Diagnostic>
- binding · function · L282-L284 — fn binding(message: &'static str) -> Vec<Diagnostic>
