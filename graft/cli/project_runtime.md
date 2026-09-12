# cli/project_runtime.rs

- execute_held · function · L8-L89 — pub(crate) fn execute_held(
- report_test · function · L94-L155 — fn report_test(execution: &project::ProjectExecution, json: bool) -> Result<(), u8>
- outcome_text · function · L157-L168 — fn outcome_text(outcome: &project::ProjectExecutionOutcome) -> String
- failure_text · function · L172-L190 — fn failure_text(failure: Option<&project::ProjectContractFailure>) -> String
- build_success · function · L192-L202 — pub(crate) fn build_success(
- build_product · function · L204-L226 — fn build_product(target: &str, profile: project::ProjectProfile) -> &'static str
- report_build_success · function · L228-L247 — pub(crate) fn report_build_success(
- report · function · L249-L258 — fn report(errors: &[Diagnostic], json: bool) -> u8
- tests · module · L261-L308 — mod tests
- profile_selected_success_labels_are_exact · function · L265-L307 — fn profile_selected_success_labels_are_exact()
