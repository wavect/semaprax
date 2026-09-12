# agent_runtime_v2/checkpoint/value.rs

- identifier · function · L6-L12 — pub(super) fn identifier(id: &str) -> bool
- encode · function · L13-L52 — pub(super) fn encode(value: &RetainedValue) -> Result<Value, Diagnostic>
- decode · function · L53-L115 — pub(super) fn decode(v: &Value) -> Result<RetainedValue, Diagnostic>
- fields · function · L116-L135 — pub(super) fn fields(values: &[(String, RetainedValue)]) -> Result<Value, Diagnostic>
- decode_fields · function · L136-L154 — pub(super) fn decode_fields(v: &Value) -> Result<Vec<(String, RetainedValue)>, Diagnostic>
- transport_bytes · function · L157-L166 — pub(super) fn transport_bytes(values: &[(String, RetainedValue)]) -> Result<u64, Diagnostic>
