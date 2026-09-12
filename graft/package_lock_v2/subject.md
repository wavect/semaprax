# package_lock_v2/subject.rs

- bf · function · L19-L21 — macro_rules! bf
- create_subject · function · L23-L51 — pub(super) fn create_subject(
- parse_subject · function · L53-L55 — pub(super) fn parse_subject(bytes: &str, work: &mut usize) -> Result<Subject, Diagnostic>
- parse_subject_for_resolution · function · L57-L62 — pub(super) fn parse_subject_for_resolution(
- parse_subject_impl · function · L64-L162 — fn parse_subject_impl(
- render_subject_payload · function · L164-L171 — fn render_subject_payload(
- exact_report_bytes · function · L173-L185 — fn exact_report_bytes(payload: &str) -> Result<&str, Diagnostic>
- START · constant · L174-L174 — const START: &str = "\"report\":";
- END · constant · L175-L175 — const END: &str = ",\"dependencies\":";
- validate_dependencies · function · L187-L206 — pub(super) fn validate_dependencies(
- validate_capabilities · function · L208-L228 — fn validate_capabilities(values: &[String]) -> Result<(), Diagnostic>
- validate_coordinate · function · L230-L249 — fn validate_coordinate(value: &Coordinate) -> Result<(), Diagnostic>
- parse_coordinates · function · L251-L263 — fn parse_coordinates(value: &Value) -> Result<Vec<Coordinate>, Diagnostic>
- parse_strings · function · L265-L276 — fn parse_strings(value: &Value) -> Result<Vec<String>, Diagnostic>
- render_coordinate · function · L278-L284 — pub(super) fn render_coordinate(value: &Coordinate) -> String
