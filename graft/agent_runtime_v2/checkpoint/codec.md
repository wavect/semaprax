# agent_runtime_v2/checkpoint/codec.rs

- keys · function · L3-L9 — pub(super) fn keys(v: &Value, expected: &[&str]) -> Result<(), Diagnostic>
- text · function · L10-L12 — pub(super) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, Diagnostic>
- number · function · L13-L15 — fn number(v: &Value, key: &str) -> Result<u64, Diagnostic>
- usage · function · L16-L18 — pub(super) fn usage(v: CheckpointUsage) -> Value
- read_usage · function · L19-L30 — fn read_usage(v: &Value) -> Result<CheckpointUsage, Diagnostic>
- binding · function · L31-L33 — pub(super) fn binding(j: &OperationCheckpoint) -> Value
- context · function · L34-L47 — fn context(c: &EffectContext) -> Result<Value, Diagnostic>
- read_context · function · L48-L72 — fn read_context(v: &Value) -> Result<EffectContext, Diagnostic>
- event · function · L73-L94 — pub(super) fn event(e: &JournalEvent) -> Result<Value, Diagnostic>
- read_event · function · L95-L131 — fn read_event(v: &Value) -> Result<JournalEvent, Diagnostic>
- encode · function · L132-L138 — pub(super) fn encode(j: &OperationCheckpoint) -> String
- decode · function · L139-L219 — pub(super) fn decode(
