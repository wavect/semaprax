# agent_lifecycle/durable/tests.rs

- FIXTURE_PATH · constant · L21-L21 — const FIXTURE_PATH: &str = "agent-checkpoint-unit.spx";
- Store · struct · L25-L25 — struct Store(Vec<String>);
- commit · function · L28-L31 — fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError>
- Counting · struct · L34-L34 — struct Counting(usize);
- read · function · L37-L40 — fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>>
- Never · struct · L44-L44 — struct Never;
- read · function · L47-L49 — fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>>
- fixture · function · L52-L61 — fn fixture() -> DurableAgent
- task · function · L63-L68 — fn task() -> LifecycleTask
- seeded · function · L70-L72 — fn seeded(byte: &str) -> String
- a_forged_but_internally_consistent_journal_cannot_mint_an_authorization · function · L75-L174 — fn a_forged_but_internally_consistent_journal_cannot_mint_an_authorization()
- the_journal_chain_detects_truncation_reordering_and_substitution · function · L177-L215 — fn the_journal_chain_detects_truncation_reordering_and_substitution()
- a_terminal_counter_is_never_reachable_with_a_live_grant · function · L218-L257 — fn a_terminal_counter_is_never_reachable_with_a_live_grant()
