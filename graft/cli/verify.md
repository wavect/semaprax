# cli/verify.rs

- MAX_CAPSULE_BYTES · constant · L21-L21 — const MAX_CAPSULE_BYTES: u64 = 16 * 1024 * 1024;
- USAGE · constant · L23-L24 — const USAGE: &str =
- ROUTES · constant · L27-L64 — pub(crate) const ROUTES: &[(&str, usize, &str)] = &[
- VerifyOptions · struct · L66-L68 — pub(crate) struct VerifyOptions
- parse · function · L70-L82 — pub(crate) fn parse(args: &[String]) -> Result<VerifyOptions, u8>
- capsule_schema · function · L84-L126 — fn capsule_schema(path: &Path) -> Result<String, Diagnostic>
- unrecognized · function · L128-L142 — fn unrecognized(path: &Path, schema: &str, operands: usize) -> Diagnostic
- run · function · L146-L178 — pub(crate) fn run(
- read · function · L180-L187 — fn read(path: &Path) -> Result<String, Vec<Diagnostic>>
- agent_bundle · function · L191-L211 — fn agent_bundle(
- tests · module · L214-L244 — mod tests
- strings · function · L217-L219 — fn strings(values: &[&str]) -> Vec<String>
- verify_grammar_is_closed · function · L222-L234 — fn verify_grammar_is_closed()
- route_table_is_closed_and_unique · function · L237-L243 — fn route_table_is_closed_and_unique()
