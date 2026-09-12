# agent_lifecycle/durable/journal.rs

- JOURNAL_DOMAIN · constant · L24-L24 — const JOURNAL_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.journal.v1\0";
- JournalEntry · enum · L33-L61 — pub(in crate::agent_lifecycle) enum JournalEntry
- rank · function · L65-L73 — pub(in crate::agent_lifecycle) const fn rank(&self) -> u8
- kind · function · L75-L84 — pub(in crate::agent_lifecycle) const fn kind(&self) -> &'static str
- operation · function · L87-L94 — pub(in crate::agent_lifecycle) fn operation(&self) -> Option<&str>
- encode · function · L97-L138 — pub(in crate::agent_lifecycle) fn encode(&self, seq: usize) -> String
- hex · function · L142-L148 — pub(in crate::agent_lifecycle) fn hex(bytes: &[u8]) -> String
- unhex · function · L150-L169 — fn unhex(text: &str) -> Option<Vec<u8>>
- render · function · L172-L182 — pub(in crate::agent_lifecycle) fn render(entries: &[JournalEntry]) -> String
- chain · function · L189-L200 — pub(in crate::agent_lifecycle) fn chain(entries: &[JournalEntry]) -> String
- text · function · L202-L204 — fn text<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str>
- digest_text · function · L206-L214 — fn digest_text<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str>
- decode · function · L219-L299 — pub(in crate::agent_lifecycle) fn decode(value: &Value, limit: usize) -> Option<Vec<JournalEntry>>
- closed · function · L301-L303 — fn closed(entry: &Map<String, Value>, keys: &[&str]) -> Option<()>
