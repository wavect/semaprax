---
covers: []
---
# diagnostic.rs

- Severity · enum · L7-L10 — pub enum Severity
- is_error · function · L13-L15 — pub fn is_error(self) -> bool
- as_str · function · L17-L22 — pub fn as_str(self) -> &'static str
- Diagnostic · struct · L26-L33 — pub struct Diagnostic
- error · function · L36-L45 — pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self
- warning · function · L47-L56 — pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self
- io · function · L58-L67 — pub fn io(code: &'static str, message: impl Into<String>) -> Self
- at_path · function · L69-L72 — pub fn at_path(mut self, path: impl Into<String>) -> Self
- with_help · function · L74-L77 — pub fn with_help(mut self, help: impl Into<String>) -> Self
- json · function · L79-L106 — pub fn json(&self) -> String
- fmt · function · L110-L135 — fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
- write_human_path · function · L138-L149 — fn write_human_path(f: &mut fmt::Formatter<'_>, path: &str) -> fmt::Result
- quote_json · function · L151-L169 — pub fn quote_json(value: &str) -> String
- tests · module · L172-L320 — mod tests
- span · function · L177-L184 — fn span() -> Span
- display_renders_each_location_combination_in_a_stable_order · function · L187-L214 — fn display_renders_each_location_combination_in_a_stable_order()
- displayed_paths_escape_control_bytes · function · L217-L236 — fn displayed_paths_escape_control_bytes()
- json_uses_bare_nulls_for_absent_fields_and_a_fixed_key_order · function · L239-L264 — fn json_uses_bare_nulls_for_absent_fields_and_a_fixed_key_order()
- quote_json_escapes_every_control_character · function · L267-L302 — fn quote_json_escapes_every_control_character()
- quote_json_never_exceeds_the_active_output_budget · function · L305-L319 — fn quote_json_never_exceeds_the_active_output_budget()
