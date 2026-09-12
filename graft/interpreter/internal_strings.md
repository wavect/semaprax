# interpreter/internal_strings.rs

- tests · module · L12-L12 — mod tests;
- wire · module · L13-L13 — mod wire;
- SCHEMA · constant · L15-L15 — pub const SCHEMA: &str = "semaprax.interpret.internal-strings.v1";
- MAX_SOURCE_BYTES · constant · L16-L16 — pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
- MAX_ENVELOPE_BYTES · constant · L17-L17 — pub const MAX_ENVELOPE_BYTES: usize = 16 * 1024 * 1024;
- PAYLOAD_DIGEST_DOMAIN · constant · L18-L18 — pub(super) const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.interpret.internal-strings.payload.v1\0";
- interpret · function · L21-L34 — pub fn interpret(
- verify_envelope · function · L37-L44 — pub fn verify_envelope(envelope: &str) -> Result<(), Diagnostic>
- verify_envelope_against_source · function · L47-L68 — pub fn verify_envelope_against_source(
- signature_is_admitted · function · L70-L85 — pub(super) fn signature_is_admitted(
