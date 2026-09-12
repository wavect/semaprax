# assurance_manifest/proof_certificate/render.rs

- bformat · function · L19-L23 — macro_rules! bformat
- SCHEMA · constant · L25-L25 — pub const SCHEMA: &str = "semaprax.smt-proof-certificate.v1";
- SOURCE_DIGEST_DOMAIN · constant · L27-L27 — const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.smt-proof-certificate.source.v1\0";
- PAYLOAD_DIGEST_DOMAIN · constant · L28-L28 — const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.smt-proof-certificate.payload.v1\0";
- SCRIPT_DIGEST_DOMAIN · constant · L29-L29 — const SCRIPT_DIGEST_DOMAIN: &[u8] = b"semaprax.smt-proof-certificate.script.v1\0";
- NONCLAIMS_JSON · constant · L35-L44 — const NONCLAIMS_JSON: &str = "\"no_assurance_manifest_merge\",\
- domain_digest · function · L46-L55 — pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String
- source_digest · function · L57-L59 — pub(super) fn source_digest(source: &str) -> String
- payload_digest · function · L61-L63 — pub(super) fn payload_digest(payload_bytes: &[u8]) -> String
- script_digest · function · L65-L67 — pub(super) fn script_digest(script: &str) -> String
- CertificateBody · enum · L75-L81 — pub(super) enum CertificateBody
- RenderInput · struct · L85-L105 — pub(super) struct RenderInput<'a>
- opt_json · function · L107-L112 — fn opt_json(value: &Option<String>) -> String
- opt_usize_json · function · L114-L119 — fn opt_usize_json(value: Option<usize>) -> String
- render_model_entry · function · L121-L132 — fn render_model_entry(name: &str, value: &ModelValue) -> String
- render_counterexample · function · L134-L156 — fn render_counterexample(model: &Model, outcome: &ReplayOutcome) -> String
- render · function · L159-L209 — pub(super) fn render(input: &RenderInput<'_>) -> String
