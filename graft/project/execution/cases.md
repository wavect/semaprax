# project/execution/cases.rs

- TEST_CASE_PREFIX · constant · L22-L22 — pub const TEST_CASE_PREFIX: &str = "test_";
- MAX_CONTRACT_TEXT_BYTES · constant · L25-L25 — pub(super) const MAX_CONTRACT_TEXT_BYTES: usize = 4096;
- ProjectContractFailure · struct · L29-L39 — pub struct ProjectContractFailure
- ProjectContractArgument · struct · L42-L47 — pub struct ProjectContractArgument
- SkippedTestCase · struct · L53-L56 — pub struct SkippedTestCase
- ProjectTestCase · struct · L60-L67 — pub struct ProjectTestCase
- stable_id · function · L70-L72 — pub fn stable_id(&self) -> &str
- name · function · L74-L76 — pub fn name(&self) -> &str
- outcome · function · L78-L80 — pub const fn outcome(&self) -> &ProjectExecutionOutcome
- steps_used · function · L82-L84 — pub const fn steps_used(&self) -> usize
- max_steps · function · L86-L88 — pub const fn max_steps(&self) -> usize
- failure · function · L90-L92 — pub const fn failure(&self) -> Option<&ProjectContractFailure>
- passed · function · L95-L97 — pub const fn passed(&self) -> bool
- CaseRun · enum · L100-L103 — pub(super) enum CaseRun
- contract_failure · function · L106-L140 — pub(super) fn contract_failure(
- case_selection · function · L146-L169 — pub(super) fn case_selection<'a>(
- skipped_selection · function · L173-L211 — pub(super) fn skipped_selection(
- run_cases · function · L215-L272 — pub(super) fn run_cases(
