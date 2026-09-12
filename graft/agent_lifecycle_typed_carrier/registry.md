# agent_lifecycle_typed_carrier/registry.rs

- TypedCarrierOperation · struct · L30-L34 — pub struct TypedCarrierOperation
- TypedCarrierRegistry · struct · L40-L42 — pub struct TypedCarrierRegistry
- new · function · L46-L48 — pub fn new(operations: Vec<TypedCarrierOperation>) -> Self
- resolve · function · L58-L71 — pub fn resolve(
- TypedCarrierHandler · interface · L79-L81 — pub trait TypedCarrierHandler
- execute · function · L80-L80 — fn execute(&mut self, operation_id: &str, argument: &RetainedValue) -> Vec<u8>;
- call_typed_operation · function · L103-L130 — pub fn call_typed_operation(
