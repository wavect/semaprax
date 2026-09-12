# economic_agent/agent_core.rs

- terminal_floor · function · L20-L29 — pub(super) fn terminal_floor(&self) -> Result<usize, Diagnostic>
- new · function · L32-L43 — pub fn new(
- execute · function · L46-L81 — pub fn execute(&mut self, source: &AgentRun) -> Result<EconomicRun, Vec<Diagnostic>>
- pre_call · function · L83-L110 — pub(super) fn pre_call(&self, started: u64, maximum_output: usize) -> Result<(), Diagnostic>
- elapsed_ms · function · L112-L118 — pub(super) fn elapsed_ms(&self, started: u64) -> Result<u64, Diagnostic>
- finish_failure · function · L121-L219 — pub(super) fn finish_failure(
