# package_compatibility/auth.rs

- bf · function · L13-L13 — macro_rules! bf { ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) }; }
- authenticate · function · L15-L134 — pub(super) fn authenticate(
- parse_report · function · L136-L193 — fn parse_report(value: &Value, work: &mut usize) -> Result<Report, crate::diagnostic::Diagnostic>
- exact_subject_report · function · L195-L212 — fn exact_subject_report(subject: &str) -> Result<&str, crate::diagnostic::Diagnostic>
- PAYLOAD · constant · L196-L196 — const PAYLOAD: &str = "\"payload\":";
- REPORT · constant · L197-L197 — const REPORT: &str = "\"report\":";
- END · constant · L198-L198 — const END: &str = ",\"dependencies\":";
- parse_target_rows · function · L214-L228 — fn parse_target_rows(
- lock_context · function · L230-L266 — fn lock_context(
- normalize_selected_coordinates · function · L268-L287 — fn normalize_selected_coordinates(value: &mut Value, selected: &package_lock_v2::Coordinate)
