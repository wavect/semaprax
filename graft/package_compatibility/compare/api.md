# package_compatibility/compare/api.rs

- SCHEMA · constant · L15-L15 — pub const SCHEMA: &str = "semaprax.offline-package-compatibility-evidence.v1";
- MAX_FINDINGS · constant · L16-L16 — pub const MAX_FINDINGS: usize = 2_048;
- MAX_WORK_UNITS · constant · L17-L17 — pub const MAX_WORK_UNITS: usize = 10 * 1024 * 1024;
- MAX_JSON_DEPTH · constant · L18-L18 — pub const MAX_JSON_DEPTH: usize = 64;
- MAX_INPUT_BYTES · constant · L19-L19 — pub const MAX_INPUT_BYTES: usize = 160 * 1024 * 1024;
- MAX_OUTPUT_BYTES · constant · L20-L20 — pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
- MIN_OUTPUT_BYTES · constant · L21-L21 — pub(super) const MIN_OUTPUT_BYTES: usize = 4_096;
- DIGEST_DOMAIN · constant · L22-L23 — pub(in crate::package_compatibility) const DIGEST_DOMAIN: &[u8] =
- INPUT_DOMAIN · constant · L24-L25 — pub(in crate::package_compatibility) const INPUT_DOMAIN: &[u8] =
- bf · function · L27-L27 — macro_rules! bf { ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) }; }
- CompatibilityInput · struct · L30-L35 — pub struct CompatibilityInput
- CompatibilityOptions · struct · L38-L40 — pub struct CompatibilityOptions
- new · function · L42-L47 — pub fn new(max_bytes: usize) -> Result<Self, Diagnostic>
- default · function · L50-L54 — fn default() -> Self
- VerifiedEvidence · struct · L58-L61 — pub struct VerifiedEvidence
- generate · function · L63-L69 — pub fn generate(
- verify · function · L71-L100 — pub fn verify(
- build · function · L102-L134 — fn build(
