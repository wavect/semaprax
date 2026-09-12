# project/candidate/source_review.rs

- PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA · constant · L10-L11 — pub const PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES · constant · L12-L12 — pub const MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES: usize = 16 * 1024 * 1024;
- REPORT_DOMAIN · constant · L13-L13 — const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-source-review.v1\0";
- DIFF_DOMAIN · constant · L14-L14 — const DIFF_DOMAIN: &[u8] = b"semaprax.candidate.source-diff.v1\0";
- source_review · function · L25-L28 — pub fn source_review(&self, expected_candidate: &str) -> Result<String, Vec<Diagnostic>>
- source_review_shared · function · L34-L42 — pub(crate) fn source_review_shared(
- build_source_review · function · L44-L138 — fn build_source_review(&self) -> Result<String, Vec<Diagnostic>>
