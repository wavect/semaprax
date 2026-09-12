# package_compatibility/wire.rs

- bf · function · L9-L9 — macro_rules! bf { ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) }; }
- required_str · function · L11-L16 — pub(super) fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, Diagnostic>
- charge · function · L18-L26 — pub(super) fn charge(work: &mut usize, units: usize) -> Result<(), Diagnostic>
- validate_options · function · L28-L30 — pub(super) fn validate_options(options: &CompatibilityOptions) -> Result<(), Diagnostic>
- render_wrapper · function · L32-L40 — pub(super) fn render_wrapper(payload: &str) -> String
- parse_wrapper · function · L42-L67 — pub(super) fn parse_wrapper(wire: &str) -> Result<(), Diagnostic>
- validate_json_depth · function · L69-L101 — fn validate_json_depth(wire: &str) -> Result<(), Diagnostic>
- digest · function · L103-L109 — pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> String
- option_error · function · L111-L113 — pub(super) fn option_error(m: impl Into<String>) -> Diagnostic
- authentication_error · function · L114-L116 — pub(super) fn authentication_error(m: impl Into<String>) -> Diagnostic
- limit_error · function · L117-L119 — pub(super) fn limit_error(m: impl Into<String>) -> Diagnostic
- wire_error · function · L120-L122 — pub(super) fn wire_error(m: impl Into<String>) -> Diagnostic
- replay_error · function · L123-L125 — pub(super) fn replay_error(m: impl Into<String>) -> Diagnostic
