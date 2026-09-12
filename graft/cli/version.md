# cli/version.rs

- SCHEMA · constant · L3-L3 — const SCHEMA: &str = "semaprax.version.v1";
- MATURITY · constant · L4-L4 — const MATURITY: &str = "pre-alpha";
- RUST_MIN · constant · L5-L5 — const RUST_MIN: &str = "1.88";
- VERSION · constant · L6-L6 — const VERSION: &str = env!("CARGO_PKG_VERSION");
- INVALID_COMMIT · constant · L7-L8 — const INVALID_COMMIT: &str =
- Invocation · enum · L11-L14 — pub(crate) enum Invocation
- render · function · L16-L28 — pub(crate) fn render(invocation: Invocation, arguments: &[String]) -> Result<String, String>
- render_human_with_commit · function · L30-L36 — pub(crate) fn render_human_with_commit(commit: Option<&str>) -> Result<String, String>
- render_json_with_commit · function · L38-L50 — pub(crate) fn render_json_with_commit(commit: Option<&str>) -> Result<String, String>
- validated_commit · function · L52-L65 — fn validated_commit(commit: Option<&str>) -> Result<Option<&str>, String>
- tests · module · L68-L117 — mod tests
- argv · function · L71-L73 — fn argv(tokens: &[&str]) -> Vec<String>
- no_arguments_renders_human_output_for_both_invocations · function · L81-L87 — fn no_arguments_renders_human_output_for_both_invocations()
- json_is_admitted_only_as_the_sole_argument_of_the_version_command · function · L93-L116 — fn json_is_admitted_only_as_the_sole_argument_of_the_version_command()
