---
covers: []
---
# package_resolution_snapshot.rs

- model · module · L6-L6 — mod model;
- wire · module · L7-L7 — mod wire;
- INPUT_SCHEMA · constant · L11-L11 — pub const INPUT_SCHEMA: &str = "semaprax.offline-package-resolution-input.v1";
- MAX_FIXED_INPUT_BYTES · constant · L17-L17 — pub const MAX_FIXED_INPUT_BYTES: usize = 1_114;
- MAX_REQUIREMENT_FRAMING_BYTES · constant · L18-L20 — pub const MAX_REQUIREMENT_FRAMING_BYTES: usize = package_resolver::MAX_REQUIREMENTS
- MAX_CAPABILITY_FRAMING_BYTES · constant · L21-L23 — pub const MAX_CAPABILITY_FRAMING_BYTES: usize = package_resolver::MAX_ALLOWED_CAPABILITIES
- MAX_SUBJECT_DELIMITER_BYTES · constant · L24-L24 — pub const MAX_SUBJECT_DELIMITER_BYTES: usize = package_resolver::MAX_SUBJECTS - 1;
- MAX_INPUT_FRAMING_BYTES · constant · L25-L28 — pub const MAX_INPUT_FRAMING_BYTES: usize = MAX_FIXED_INPUT_BYTES
- MAX_INPUT_BYTES · constant · L29-L30 — pub const MAX_INPUT_BYTES: usize =
- MAX_INPUT_RENDER_BYTES · constant · L31-L31 — pub const MAX_INPUT_RENDER_BYTES: usize = MAX_INPUT_BYTES * 3 + MAX_INPUT_FRAMING_BYTES * 2;
- MAX_SNAPSHOT_BYTES · constant · L32-L33 — pub const MAX_SNAPSHOT_BYTES: usize =
- INPUT_DOMAIN · constant · L35-L35 — const INPUT_DOMAIN: &[u8] = b"semaprax.offline-package-resolution-input.v1\0";
- generate · function · L39-L56 — pub fn generate(
- verify · function · L60-L83 — pub fn verify(snapshot: &ResolutionSnapshot) -> Result<VerifiedResolution, Diagnostic>
- input_error · function · L85-L87 — fn input_error(message: impl Into<String>) -> Diagnostic
- authentication_error · function · L89-L91 — fn authentication_error(message: impl Into<String>) -> Diagnostic
- limit_error · function · L93-L95 — fn limit_error(message: impl Into<String>) -> Diagnostic
- wire_error · function · L97-L99 — fn wire_error(message: impl Into<String>) -> Diagnostic
- replay_error · function · L101-L103 — fn replay_error(message: impl Into<String>) -> Diagnostic
- map_resolver_error · function · L105-L113 — fn map_resolver_error(error: Diagnostic) -> Diagnostic
- tests · module · L116-L116 — mod tests;
