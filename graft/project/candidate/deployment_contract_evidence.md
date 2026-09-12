# project/candidate/deployment_contract_evidence.rs

- Result · type · L14-L14 — type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
- PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_SCHEMA · constant · L16-L17 — pub const PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_SCHEMA: &str =
- PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA · constant · L18-L19 — pub const PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA: &str =
- MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_BYTES · constant · L20-L20 — pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_DECLARATION_BYTES: usize = 65_536;
- MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_BYTES · constant · L21-L21 — pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_BYTES: usize = 2 * 1024 * 1024;
- MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEYS · constant · L22-L22 — pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEYS: usize = 64;
- MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEY_BYTES · constant · L23-L23 — pub const MAX_PROJECT_CANDIDATE_DEPLOYMENT_CONFIGURATION_KEY_BYTES: usize = 128;
- DECLARATION_DOMAIN · constant · L25-L26 — const DECLARATION_DOMAIN: &[u8] =
- AREA_ORDER · constant · L27-L36 — const AREA_ORDER: [&str; 8] = [
- analysis_deployment_contract_evidence · function · L42-L200 — pub fn analysis_deployment_contract_evidence(
- authenticate_declaration · function · L203-L340 — fn authenticate_declaration(
- validate_coverage · function · L342-L362 — fn validate_coverage(candidate: &ProjectCandidate, coverage: &Value) -> Result<()>
- require_keys · function · L364-L369 — fn require_keys(object: &Map<String, Value>, keys: &[&str], message: &'static str) -> Result<()>
- validate_digest · function · L371-L383 — fn validate_digest(value: &str) -> Result<()>
- invalid · function · L385-L387 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L388-L390 — fn capacity(message: &'static str) -> Vec<Diagnostic>
- binding · function · L391-L393 — fn binding(message: &'static str) -> Vec<Diagnostic>
