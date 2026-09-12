# project/candidate/analysis_artifact_evidence.rs

- Result · type · L14-L14 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA · constant · L16-L17 — pub const PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_BYTES · constant · L18-L18 — pub const MAX_PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_BYTES: usize = 10 * 1024 * 1024;
- AREA_ORDER · constant · L20-L29 — const AREA_ORDER: [&str; 8] = [
- analysis_artifact_evidence · function · L34-L126 — pub fn analysis_artifact_evidence(
- parse · function · L129-L136 — fn parse(bytes: &str, schema: &str, message: &'static str) -> Result<Value>
- validate_bindings · function · L138-L189 — fn validate_bindings(
- validate_sources · function · L191-L221 — fn validate_sources(coverage: &Value, projection: &Value) -> Result<()>
- validate_revision_sources · function · L223-L248 — fn validate_revision_sources(
- invalid · function · L250-L252 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L254-L256 — fn capacity(message: &'static str) -> Vec<Diagnostic>
