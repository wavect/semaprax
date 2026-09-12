# interpreter/process.rs

- ADMITTED_EFFECTS · constant · L10-L17 — pub(super) const ADMITTED_EFFECTS: [&str; 6] = [
- ProcessState · struct · L18-L21 — pub(super) struct ProcessState<'a>
- new · function · L23-L28 — pub(super) fn new(provider: &'a mut dyn ProcessProvider) -> Self
- settle · function · L29-L31 — pub(super) fn settle(self) -> Result<(), ProcessFailure>
- failure · function · L33-L43 — pub(super) fn failure(error: ProcessFailure) -> Flow
- extent · function · L44-L46 — fn extent(value: u64) -> Result<usize, Flow>
- evaluate_process_operation · function · L48-L108 — pub(super) fn evaluate_process_operation(
