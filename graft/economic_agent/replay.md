# economic_agent/replay.rs

- cumulative_usage · function · L22-L47 — pub(super) fn cumulative_usage(events: &[Event]) -> Result<Usage, Diagnostic>
- add · function · L25-L32 — macro_rules! add
- valid_event · function · L48-L86 — pub(super) fn valid_event(kind: &str, status: &str) -> bool
- replay_events · function · L87-L216 — pub(super) fn replay_events(events: &[Event], terminal: &Terminal) -> Result<(), Diagnostic>
- diagnostic_terminal · function · L217-L239 — pub(super) fn diagnostic_terminal(diagnostic: &Diagnostic) -> Terminal
- replay_bundle · function · L241-L345 — pub(super) fn replay_bundle(
- event · function · L347-L376 — pub(super) fn event(
- push_event · function · L378-L387 — pub(super) fn push_event(events: &mut Vec<Event>, event: Event) -> Result<(), Diagnostic>
- journal_digest · function · L388-L392 — pub(super) fn journal_digest(journal: &Journal) -> String
- cas_journal · function · L393-L488 — pub(super) fn cas_journal<H: PaymentJournal>(
- finish_run · function · L489-L571 — pub(super) fn finish_run(
