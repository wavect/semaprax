# project/candidate/analysis_evidence.rs

- Result · type · L13-L13 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_SCHEMA · constant · L15-L16 — pub const PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_BYTES · constant · L17-L17 — pub const MAX_PROJECT_CANDIDATE_ANALYSIS_EVIDENCE_BYTES: usize = 3 * 1024 * 1024;
- AREA_ORDER · constant · L19-L28 — const AREA_ORDER: [&str; 8] = [
- analysis_evidence · function · L34-L130 — pub fn analysis_evidence(
- parse · function · L133-L145 — fn parse(bytes: &str, schema: &str, owner: &'static str) -> Result<Value>
- validate_bindings · function · L147-L197 — fn validate_bindings(candidate: &ProjectCandidate, coverage: &Value, replay: &Value) -> Result<()>
- unique_area · function · L199-L209 — fn unique_area<'a>(areas: &'a mut [Value], name: &str) -> Result<&'a mut Value>
- invalid · function · L211-L213 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L215-L217 — fn capacity(message: &'static str) -> Vec<Diagnostic>
