# public_generic_abi/wasm/registry.rs

- HANDLE_INVALID · constant · L22-L22 — pub const HANDLE_INVALID: &str = "SPX-PG915";
- WRONG_KIND · constant · L26-L26 — pub const WRONG_KIND: &str = "SPX-PG916";
- invalid · function · L28-L30 — fn invalid(message: impl Into<String>) -> Diagnostic
- wrong_kind · function · L32-L34 — fn wrong_kind(message: impl Into<String>) -> Diagnostic
- HandleRole · enum · L40-L45 — pub enum HandleRole
- is_leaf · function · L48-L50 — fn is_leaf(self) -> bool
- RegistryEntry · struct · L56-L60 — pub struct RegistryEntry
- HandleRegistry · struct · L68-L70 — pub struct HandleRegistry
- new · function · L73-L77 — pub fn new() -> Self
- len · function · L79-L81 — pub fn len(&self) -> usize
- is_empty · function · L83-L85 — pub fn is_empty(&self) -> bool
- insert · function · L92-L98 — pub fn insert(&mut self, handle: Handle, entry: RegistryEntry)
- get · function · L103-L115 — pub fn get(&self, handle: Handle, role: HandleRole) -> Result<RegistryEntry, Diagnostic>
- remove · function · L120-L128 — pub fn remove(
- live_leaves_in_order · function · L133-L143 — pub fn live_leaves_in_order(&self, leaf_role: HandleRole) -> Vec<(Handle, RegistryEntry)>
- tests · module · L147-L147 — mod tests;
