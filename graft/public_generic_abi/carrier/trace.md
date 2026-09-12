# public_generic_abi/carrier/trace.rs

- Direction · enum · L17-L20 — pub enum Direction
- TraceLabel · enum · L27-L43 — pub enum TraceLabel
- TraceEvent · struct · L51-L65 — pub struct TraceEvent
- Trace · struct · L72-L74 — pub struct Trace
- new · function · L77-L79 — pub fn new() -> Self
- record · function · L82-L101 — pub(crate) fn record(
- events · function · L104-L106 — pub fn events(&self) -> &[TraceEvent]
- tests · module · L110-L169 — mod tests
- record_assigns_sequential_ordinals_and_preserves_order · function · L114-L149 — fn record_assigns_sequential_ordinals_and_preserves_order()
- trace_carries_no_payload_bytes_by_construction · function · L152-L168 — fn trace_carries_no_payload_bytes_by_construction()
- assert_no_payload_field · function · L157-L157 — fn assert_no_payload_field(_: TraceEvent) {}
