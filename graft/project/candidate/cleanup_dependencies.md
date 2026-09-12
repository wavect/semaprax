# project/candidate/cleanup_dependencies.rs

- PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA · constant · L12-L13 — pub const PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA: &str =
- PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA · constant · L14-L15 — pub const PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES · constant · L16-L16 — pub const MAX_PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_BYTES: usize = 8 * 1024 * 1024;
- REPORT_DOMAIN · constant · L17-L17 — const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-cleanup-dependencies.report.v1\0";
- SIDE_DOMAIN · constant · L18-L18 — const SIDE_DOMAIN: &[u8] = b"semaprax.candidate-cleanup-dependencies.side.v1\0";
- Result · type · L19-L19 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- cleanup_dependencies · function · L24-L66 — pub fn cleanup_dependencies(&self, expected_candidate: &str, target: &str) -> Result<String>
- verify_cleanup_dependencies · function · L70-L105 — pub fn verify_cleanup_dependencies(
- side · function · L108-L125 — fn side(
- render · function · L127-L129 — fn render(value: Value) -> Result<String>
- invalid · function · L130-L135 — fn invalid() -> Vec<Diagnostic>
- capacity · function · L136-L141 — fn capacity() -> Vec<Diagnostic>
