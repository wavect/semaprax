# project/candidate/analysis_runtime_evidence.rs

- Result · type · L13-L13 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA · constant · L15-L16 — pub const PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES · constant · L17-L17 — pub const MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES: usize = 4 * 1024 * 1024;
- AREA_ORDER · constant · L19-L28 — const AREA_ORDER: [&str; 8] = [
- analysis_runtime_evidence · function · L34-L159 — pub fn analysis_runtime_evidence(
- parse · function · L162-L169 — fn parse(bytes: &str, schema: &str, message: &'static str) -> Result<Value>
- validate_bindings · function · L171-L220 — fn validate_bindings(
- validate_source_bindings · function · L222-L270 — fn validate_source_bindings(coverage: &Value, report: &Value) -> Result<()>
- unique_area · function · L272-L282 — fn unique_area<'a>(areas: &'a mut [Value], name: &str) -> Result<&'a mut Value>
- invalid · function · L284-L286 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L288-L290 — fn capacity(message: &'static str) -> Vec<Diagnostic>
