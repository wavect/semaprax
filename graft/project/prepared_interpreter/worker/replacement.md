# project/prepared_interpreter/worker/replacement.rs

- WorkerState · struct · L7-L10 — pub(super) struct WorkerState
- ReplacementRequest · struct · L12-L18 — pub(super) struct ReplacementRequest
- validate_expected_revision · function · L20-L34 — pub(super) fn validate_expected_revision(expected: &str) -> Result<(), Vec<Diagnostic>>
- process · function · L37-L71 — pub(super) fn process(state: &mut WorkerState, request: ReplacementRequest) -> bool
- TestHook · enum · L74-L81 — pub(super) enum TestHook
- before_prepare · function · L85-L94 — fn before_prepare(&self)
- after_commit · function · L96-L100 — fn after_commit(&self)
