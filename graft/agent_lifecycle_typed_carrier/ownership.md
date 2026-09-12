# agent_lifecycle_typed_carrier/ownership.rs

- OwnershipLedger · struct · L43-L45 — pub struct OwnershipLedger
- new · function · L49-L53 — pub fn new() -> Self
- live · function · L58-L60 — pub fn live(&self) -> usize
- open · function · L62-L65 — pub(super) fn open(&self) -> OwnedToken<'_>
- OwnedToken · struct · L72-L74 — pub struct OwnedToken<'a>
- drop · function · L77-L79 — fn drop(&mut self)
- stage_and_evaluate · function · L101-L126 — pub fn stage_and_evaluate(
