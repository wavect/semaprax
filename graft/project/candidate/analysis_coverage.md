# project/candidate/analysis_coverage.rs

- Result · type · L15-L15 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA · constant · L17-L18 — pub const PROJECT_CANDIDATE_ANALYSIS_COVERAGE_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES · constant · L19-L19 — pub const MAX_PROJECT_CANDIDATE_ANALYSIS_COVERAGE_BYTES: usize = MAX_IMAGE_ANALYSIS_COVERAGE_BYTES;
- analysis_coverage · function · L24-L54 — pub fn analysis_coverage(&self, expected_candidate: &str) -> Result<String>
- invalid · function · L57-L59 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L61-L63 — fn capacity(message: &'static str) -> Vec<Diagnostic>
