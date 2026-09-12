# agent_lifecycle_typed_carrier/checkpoint.rs

- CHECKPOINT_SCHEMA · constant · L41-L41 — pub const CHECKPOINT_SCHEMA: &str = "semaprax.agent-lifecycle-typed-checkpoint.v1";
- CHECKPOINT_TYPE_VERSION · constant · L43-L43 — pub const CHECKPOINT_TYPE_VERSION: u32 = 1;
- MAX_CHECKPOINT_BYTES · constant · L46-L46 — pub const MAX_CHECKPOINT_BYTES: usize = 131_072;
- encode · function · L54-L66 — pub fn encode(value: &DecodedInteractionValue) -> Result<String, Diagnostic>
- decode · function · L71-L119 — pub fn decode(
- object_span · function · L129-L161 — fn object_span(text: &str) -> Option<usize>
