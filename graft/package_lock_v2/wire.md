# package_lock_v2/wire.rs

- bf · function · L9-L11 — macro_rules! bf
- required_str · function · L13-L18 — pub(super) fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, Diagnostic>
- charge · function · L20-L28 — pub(super) fn charge(work: &mut usize, units: usize) -> Result<(), Diagnostic>
- render_wrapper · function · L30-L38 — pub(super) fn render_wrapper(schema: &str, domain: &[u8], payload: &str) -> String
- parse_wrapper · function · L40-L79 — pub(super) fn parse_wrapper<'a>(
- validate_json_depth · function · L81-L113 — fn validate_json_depth(wire: &str) -> Result<(), Diagnostic>
- domain_digest · function · L115-L121 — pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String
- option_error · function · L123-L125 — pub(super) fn option_error(m: impl Into<String>) -> Diagnostic
- wire_error · function · L126-L128 — pub(super) fn wire_error(m: impl Into<String>) -> Diagnostic
- authentication_error · function · L129-L131 — pub(super) fn authentication_error(m: impl Into<String>) -> Diagnostic
- confusion_error · function · L132-L134 — pub(super) fn confusion_error(m: impl Into<String>) -> Diagnostic
- cycle_error · function · L135-L137 — pub(super) fn cycle_error(m: impl Into<String>) -> Diagnostic
- limit_error · function · L138-L140 — pub(super) fn limit_error(m: impl Into<String>) -> Diagnostic
- replay_error · function · L141-L143 — pub(super) fn replay_error(m: impl Into<String>) -> Diagnostic
