# public_generic_consumer/rust_calling.rs

- OwnedByteField · struct · L45-L47 — pub struct OwnedByteField
- new · function · L50-L54 — pub fn new(identity: impl Into<String>) -> Self
- field_name · function · L56-L58 — fn field_name(&self) -> String
- RecordShape · struct · L68-L70 — pub struct RecordShape
- new · function · L73-L75 — pub fn new(fields: Vec<OwnedByteField>) -> Self
- duplicate_identity · function · L77-L87 — fn duplicate_identity(&self) -> Option<&str>
- ShapeError · enum · L96-L108 — pub enum ShapeError
- fmt · function · L111-L122 — fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
- CallingConsumer · struct · L128-L130 — pub struct CallingConsumer
- files · function · L133-L135 — pub fn files(&self) -> &[(String, String)]
- CRATE_NAME · constant · L141-L141 — pub const CRATE_NAME: &str = "spx-pg-rust-calling-consumer";
- LIB_NAME · constant · L142-L142 — pub const LIB_NAME: &str = "spx_pg_rust_calling_consumer";
- generate_rust_calling_consumer · function · L150-L197 — pub fn generate_rust_calling_consumer(
- render · module · L199-L199 — mod render;
- tests · module · L202-L202 — mod tests;
