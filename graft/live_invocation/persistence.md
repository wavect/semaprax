# live_invocation/persistence.rs

- tests · module · L89-L89 — mod tests;
- PERSISTED_JOURNAL_SCHEMA · constant · L95-L95 — pub const PERSISTED_JOURNAL_SCHEMA: &str = "semaprax.live-invocation.persisted-journal.v1";
- JournalSink · interface · L103-L110 — pub trait JournalSink
- persist · function · L109-L109 — fn persist(&mut self, journal: &[JournalEntry]) -> Result<(), CheckpointStoreError>;
- CheckpointJournalSink · struct · L117-L121 — pub struct CheckpointJournalSink<'a>
- new · function · L126-L132 — pub fn new(store: &'a mut dyn CheckpointStore, invocation: impl Into<String>) -> Self
- resume · function · L138-L148 — pub fn resume(
- generation · function · L153-L155 — pub fn generation(&self) -> u64
- persist · function · L159-L165 — fn persist(&mut self, journal: &[JournalEntry]) -> Result<(), CheckpointStoreError>
- encode_envelope · function · L174-L185 — pub fn encode_envelope(invocation: &str, generation: u64, journal: &[JournalEntry]) -> String
- RecoveryError · enum · L193-L207 — pub enum RecoveryError
- RecoveredJournal · struct · L217-L220 — pub struct RecoveredJournal
- recover_journal · function · L227-L256 — pub fn recover_journal(
- KEYS · constant · L233-L233 — const KEYS: [&str; 5] = ["schema", "invocation", "generation", "chain", "entries"];
