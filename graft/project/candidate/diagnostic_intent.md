# project/candidate/diagnostic_intent.rs

- apply · function · L7-L45 — pub(super) fn apply(
- exact · function · L47-L55 — fn exact(value: &Value, fields: &[&str]) -> Result<(), Vec<Diagnostic>>
- text · function · L56-L60 — fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, Vec<Diagnostic>>
- grammar · function · L61-L63 — fn grammar(message: &'static str) -> Vec<Diagnostic>
- stale · function · L64-L66 — pub(super) fn stale(message: &'static str) -> Vec<Diagnostic>
- rebase_conflict · function · L67-L69 — pub(super) fn rebase_conflict() -> Vec<Diagnostic>
