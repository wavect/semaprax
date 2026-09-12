# cli/context.rs

- SCHEMA_V1 · constant · L13-L13 — const SCHEMA_V1: &str = "semaprax.project-agent-context.v1";
- invalid_projection · function · L15-L20 — fn invalid_projection() -> Vec<Diagnostic>
- member · function · L22-L27 — fn member<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Vec<Diagnostic>>
- compact · function · L29-L111 — fn compact(full: &str, max_bytes: usize) -> Result<String, Vec<Diagnostic>>
- project · function · L114-L141 — pub(crate) fn project(
