# codegen/native_emit/owned_strings.rs

- FunctionOutput · enum · L7-L10 — pub(super) enum FunctionOutput<'a, O>
- write_str · function · L13-L16 — fn write_str(&mut self, value: &str) -> std::fmt::Result
- push_str · function · L20-L25 — fn push_str(&mut self, value: &str)
- push · function · L26-L31 — fn push(&mut self, value: char)
- OwnedStrings · struct · L35-L38 — pub(super) struct OwnedStrings
- register · function · L41-L48 — pub(super) fn register(&mut self, name: &str, declare: bool) -> Result<(), super::Diagnostic>
- declarations · function · L50-L59 — pub(super) fn declarations(&self) -> String
- names · function · L61-L63 — pub(super) fn names(&self) -> Vec<String>
- string_initialize · function · L67-L71 — pub(super) fn string_initialize(&mut self, name: &str)
- string_move · function · L73-L83 — pub(super) fn string_move(&mut self, destination: &str, source: &str)
- string_require_dead · function · L85-L91 — pub(super) fn string_require_dead(&mut self, name: &str)
- string_drop · function · L93-L105 — pub(super) fn string_drop(&mut self, name: &str)
- tests · module · L110-L110 — mod tests;
