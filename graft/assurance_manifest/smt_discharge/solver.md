# assurance_manifest/smt_discharge/solver.rs

- ENV_Z3_PATH · constant · L23-L23 — pub const ENV_Z3_PATH: &str = "SEMAPRAX_SMT_Z3_PATH";
- Provisioning · struct · L27-L34 — pub struct Provisioning
- provision_from_env · function · L41-L54 — pub fn provision_from_env() -> Option<Provisioning>
- RunLimits · struct · L59-L62 — pub struct RunLimits
- default · function · L68-L73 — fn default() -> Self
- Verdict · enum · L81-L109 — pub enum Verdict
- read_capped · function · L111-L116 — fn read_capped(mut source: impl Read, max_bytes: usize, tx: mpsc::Sender<Vec<u8>>)
- run · function · L124-L204 — pub fn run(provisioning: &Provisioning, script: &str, limits: &RunLimits) -> Verdict
- solver_version · function · L212-L230 — pub fn solver_version(provisioning: &Provisioning) -> Option<String>
- classify · function · L242-L273 — fn classify(stdout_text: &str, exit_status: Option<&std::process::ExitStatus>) -> Verdict
- tests · module · L276-L345 — mod tests
- provision_from_env_rejects_unset_empty_relative_and_missing_paths · function · L280-L287 — fn provision_from_env_rejects_unset_empty_relative_and_missing_paths()
- classify_recognizes_every_defined_first_token · function · L290-L298 — fn classify_recognizes_every_defined_first_token()
- classify_rejects_sat_with_no_model_as_malformed · function · L301-L308 — fn classify_rejects_sat_with_no_model_as_malformed()
- classify_rejects_an_unrecognized_first_token_as_malformed · function · L311-L316 — fn classify_rejects_an_unrecognized_first_token_as_malformed()
- classify_rejects_empty_output_with_a_successful_exit_as_malformed · function · L319-L321 — fn classify_rejects_empty_output_with_a_successful_exit_as_malformed()
- classify_recognizes_a_first_token_even_over_a_failing_exit_status · function · L324-L331 — fn classify_recognizes_a_first_token_even_over_a_failing_exit_status()
- classify_reports_a_crash_for_empty_output_plus_a_failing_exit · function · L334-L344 — fn classify_reports_a_crash_for_empty_output_plus_a_failing_exit()
