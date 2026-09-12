# project/source_hint.rs

- UNRESOLVED_USE_CODE · constant · L22-L22 — const UNRESOLVED_USE_CODE: &str = "SPX-G172";
- UNRESOLVED_USE_MESSAGE · constant · L23-L23 — const UNRESOLVED_USE_MESSAGE: &str = "target module is missing or equals the caller module";
- MAX_HINT_FILE_BYTES · constant · L24-L24 — const MAX_HINT_FILE_BYTES: u64 = 1024 * 1024;
- MAX_SCANNED_FILES · constant · L25-L25 — const MAX_SCANNED_FILES: usize = 512;
- hint_unlisted_module · function · L27-L47 — pub(super) fn hint_unlisted_module(
- unresolved_use_help · function · L49-L78 — fn unresolved_use_help(
- unlisted_declaring_file · function · L82-L129 — fn unlisted_declaring_file(
- declared_module · function · L133-L144 — pub(super) fn declared_module(source: &str) -> Option<&str>
- normalize · function · L146-L152 — fn normalize(path: &Path) -> String
- read_bounded · function · L154-L160 — fn read_bounded(path: &Path) -> Option<String>
- tests · module · L163-L247 — mod tests
- module_header_scan_accepts_comments_and_rejects_other_shapes · function · L167-L181 — fn module_header_scan_accepts_comments_and_rejects_other_shapes()
- only_the_unresolved_use_diagnostic_without_help_is_considered · function · L184-L246 — fn only_the_unresolved_use_diagnostic_without_help_is_considered()
