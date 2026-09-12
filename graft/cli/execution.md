# cli/execution.rs

- ExecutionInput · enum · L8-L11 — pub(crate) enum ExecutionInput
- ExecutionOptions · struct · L14-L20 — pub(crate) struct ExecutionOptions
- NetworkRunOptions · struct · L23-L29 — pub(crate) struct NetworkRunOptions
- parse_network_run · function · L31-L120 — pub(crate) fn parse_network_run(args: &[String]) -> Result<NetworkRunOptions, u8>
- parse_run · function · L122-L124 — pub(crate) fn parse_run(args: &[String]) -> Result<ExecutionOptions, u8>
- parse_test · function · L126-L128 — pub(crate) fn parse_test(args: &[String]) -> Result<ExecutionOptions, u8>
- parse · function · L130-L238 — fn parse(args: &[String], command: &str, allow_source: bool) -> Result<ExecutionOptions, u8>
- option_value · function · L240-L253 — fn option_value<'a>(
- positive_number · function · L255-L276 — fn positive_number(command: &str, option: &str, value: &str) -> Result<usize, u8>
- number_option_value · function · L278-L288 — fn number_option_value<'a>(
- tests · module · L291-L405 — mod tests
- strings · function · L294-L296 — fn strings(values: &[&str]) -> Vec<String>
- run_preserves_legacy_source_and_selects_projects_explicitly · function · L299-L360 — fn run_preserves_legacy_source_and_selects_projects_explicitly()
- test_accepts_only_default_or_explicit_project_manifests · function · L363-L404 — fn test_accepts_only_default_or_explicit_project_manifests()
