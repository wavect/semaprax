# assurance_manifest/proof_certificate/verify.rs

- consistency_error · function · L26-L28 — fn consistency_error(message: String) -> Diagnostic
- drift_error · function · L30-L32 — fn drift_error(message: String) -> Diagnostic
- is_sha256_wire_form · function · L34-L42 — fn is_sha256_wire_form(value: &str) -> bool
- object_keys · function · L44-L49 — fn object_keys(value: &Value, context: &str) -> Result<Vec<String>, Diagnostic>
- require_string · function · L51-L55 — fn require_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Diagnostic>
- require_array · function · L57-L61 — fn require_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, Diagnostic>
- check_exact_keys · function · L63-L81 — fn check_exact_keys(
- PAYLOAD_KEYS · constant · L83-L98 — const PAYLOAD_KEYS: [&str; 14] = [
- CheckedBody · enum · L100-L106 — pub(super) enum CheckedBody
- CheckedCertificate · struct · L108-L117 — pub(super) struct CheckedCertificate
- check_model_entries · function · L119-L167 — fn check_model_entries(entries: &[Value]) -> Result<Model, Diagnostic>
- check_counterexample · function · L169-L240 — fn check_counterexample(
- verify_certificate · function · L249-L252 — pub fn verify_certificate(certificate: &str) -> Result<(), Diagnostic>
- check_certificate · function · L254-L420 — fn check_certificate(certificate: &str) -> Result<CheckedCertificate, Diagnostic>
- PAYLOAD_KEY · constant · L277-L277 — const PAYLOAD_KEY: &str = "\"payload\":";
- verify_certificate_against_source · function · L445-L543 — pub fn verify_certificate_against_source(
- verify_certificate_with_solver · function · L555-L574 — pub fn verify_certificate_with_solver(
