# region_report/replay.rs

- verify_envelope · function · L3-L157 — pub(super) fn verify_envelope(envelope: &str) -> Result<VerifiedRegionReport, Diagnostic>
- PAYLOAD_KEY · constant · L32-L32 — const PAYLOAD_KEY: &str = "\"payload\":";
- verify_envelope_against_source · function · L161-L178 — pub(super) fn verify_envelope_against_source(
- bound_source_digest · function · L180-L189 — fn bound_source_digest(envelope: &str) -> Result<String, Diagnostic>
- replay_function · function · L192-L563 — fn replay_function(
