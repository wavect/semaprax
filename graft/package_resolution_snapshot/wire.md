# package_resolution_snapshot/wire.rs

- NONCLAIMS · constant · L10-L20 — const NONCLAIMS: [&str; 9] = [
- ParsedInput · struct · L22-L25 — pub(super) struct ParsedInput
- render_input · function · L27-L97 — pub(super) fn render_input(
- parse_input · function · L99-L227 — pub(super) fn parse_input(wire: &str) -> Result<ParsedInput, Diagnostic>
- exact_subjects · function · L229-L278 — fn exact_subjects(payload: &str) -> Result<(Vec<String>, usize, usize), Diagnostic>
- START · constant · L230-L230 — const START: &str = ",\"subjects\":[";
- FOLLOW · constant · L231-L231 — const FOLLOW: &str = "],\"resolution_max_bytes\":";
- render_canonical · function · L280-L320 — fn render_canonical(
- fixed_framing_fixture_bytes · function · L323-L341 — pub(super) fn fixed_framing_fixture_bytes() -> usize
- object_end · function · L343-L380 — fn object_end(bytes: &[u8], start: usize, limit: usize) -> Result<usize, Diagnostic>
- render_wrapper · function · L382-L390 — fn render_wrapper(payload: &str) -> String
- digest · function · L392-L401 — fn digest(bytes: &[u8]) -> String
- require_keys · function · L403-L413 — fn require_keys(value: &Value, keys: &[&str], label: &str) -> Result<(), Diagnostic>
- required_string · function · L415-L420 — fn required_string(value: &Value, key: &str) -> Result<String, Diagnostic>
