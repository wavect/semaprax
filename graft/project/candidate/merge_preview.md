# project/candidate/merge_preview.rs

- PROJECT_CANDIDATE_MERGE_PREVIEW_SCHEMA · constant · L10-L11 — pub const PROJECT_CANDIDATE_MERGE_PREVIEW_SCHEMA: &str =
- PROJECT_CANDIDATE_MERGE_PREVIEW_VERIFICATION_SCHEMA · constant · L12-L13 — pub const PROJECT_CANDIDATE_MERGE_PREVIEW_VERIFICATION_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES · constant · L14-L14 — pub const MAX_PROJECT_CANDIDATE_MERGE_PREVIEW_BYTES: usize = 256 * 1024;
- MAX_DIAGNOSTICS · constant · L15-L15 — const MAX_DIAGNOSTICS: usize = 64;
- MAX_DIAGNOSTIC_BYTES · constant · L16-L16 — const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;
- REPORT_DOMAIN · constant · L17-L17 — const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-merge-preview.v1\0";
- Result · type · L18-L18 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- merge_preview · function · L23-L64 — pub fn merge_preview(
- verify_merge_preview · function · L68-L111 — pub fn verify_merge_preview(
- preview_parents · function · L113-L129 — fn preview_parents(
- exact_source · function · L132-L140 — fn exact_source(left: &ProjectRevision, right: &ProjectRevision) -> bool
- direction · function · L142-L183 — fn direction(outcome: &Result<ProjectCandidateRebase>, prefix: usize) -> Result<Value>
- render · function · L185-L188 — fn render(value: Value) -> Result<String>
- capacity · function · L190-L192 — fn capacity(message: &'static str) -> Vec<Diagnostic>
- conflict · function · L194-L196 — fn conflict(message: &'static str) -> Vec<Diagnostic>
- tests · module · L199-L239 — mod tests
- rejection_projection_is_closed_and_never_truncates_diagnostics · function · L203-L219 — fn rejection_projection_is_closed_and_never_truncates_diagnostics()
- diagnostic_budget_counts_utf8_and_both_code_and_message · function · L222-L238 — fn diagnostic_budget_counts_utf8_and_both_code_and_message()
