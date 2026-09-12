# project/candidate/wire.rs

- validate_digest · function · L13-L25 — pub(super) fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>>
- validate_value · function · L28-L71 — pub(super) fn validate_value(value: &Value) -> Result<(), Vec<Diagnostic>>
- digest · function · L73-L79 — pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> String
- render · function · L81-L108 — pub(super) fn render(mut value: Value, limit: usize) -> Result<String, Vec<Diagnostic>>
- Sink · struct · L82-L85 — struct Sink
- write · function · L87-L93 — fn write(&mut self, bytes: &[u8]) -> io::Result<usize>
- flush · function · L94-L96 — fn flush(&mut self) -> io::Result<()>
- target_facts · function · L110-L150 — pub(super) fn target_facts(revision: &ProjectRevision) -> Result<Value, Vec<Diagnostic>>
- preserve_targets · function · L152-L173 — pub(super) fn preserve_targets(base: &Value, candidate: &Value) -> Result<(), Vec<Diagnostic>>
- source_diff · function · L176-L217 — pub(super) fn source_diff(
