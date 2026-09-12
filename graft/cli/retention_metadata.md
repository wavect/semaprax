# cli/retention_metadata.rs

- inventory · function · L17-L20 — pub(crate) fn inventory(path: &Path) -> Result<String, Vec<Diagnostic>>
- PlanOptions · struct · L22-L31 — pub(crate) struct PlanOptions<'a>
- plan · function · L33-L87 — pub(crate) fn plan(options: PlanOptions<'_>) -> Result<String, Vec<Diagnostic>>
- decimal · function · L89-L99 — fn decimal(value: &str, field: &'static str) -> Result<u64, Vec<Diagnostic>>
- cli_input · function · L101-L103 — fn cli_input(message: impl Into<String>) -> Vec<Diagnostic>
- persist · function · L105-L146 — pub(crate) fn persist(
- load · function · L148-L179 — pub(crate) fn load(
