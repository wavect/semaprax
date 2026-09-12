# package_report_v2/wire.rs

- SOURCE_SCHEMA · constant · L12-L12 — pub(super) const SOURCE_SCHEMA: &str = "semaprax.canonical-source.v1";
- SOURCE_DIGEST_DOMAIN · constant · L13-L13 — pub(super) const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report-v2.source.v1\0";
- PAYLOAD_DIGEST_DOMAIN · constant · L14-L14 — pub(super) const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report-v2.payload.v1\0";
- CONTRACT_DIGEST_DOMAIN · constant · L15-L15 — pub(super) const CONTRACT_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report-v2.contract-fact.v1\0";
- bf · function · L17-L19 — macro_rules! bf
- ParsedSubject · struct · L21-L24 — pub(super) struct ParsedSubject
- parse_subject · function · L26-L28 — pub(super) fn parse_subject(envelope: &str) -> Result<ParsedSubject, Diagnostic>
- parse_subject_for_resolution · function · L30-L32 — pub(super) fn parse_subject_for_resolution(envelope: &str) -> Result<ParsedSubject, Diagnostic>
- parse_subject_impl · function · L34-L152 — fn parse_subject_impl(
- PAYLOAD_MARKER · constant · L82-L82 — const PAYLOAD_MARKER: &str = "\"payload\":";
- render_envelope · function · L154-L162 — pub(super) fn render_envelope(payload: &str) -> String
- domain_digest · function · L164-L173 — pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String
