# project/flat_owned_record/settlement.rs

- FlatOwnedRecordSettlement · struct · L10-L12 — pub struct FlatOwnedRecordSettlement
- SettlementState · enum · L15-L22 — enum SettlementState
- received · function · L25-L29 — pub const fn received() -> Self
- authenticated · function · L30-L32 — pub fn authenticated(&mut self) -> Result<(), Diagnostic>
- copy_completed · function · L33-L35 — pub fn copy_completed(&mut self) -> Result<(), Diagnostic>
- settlement_completed · function · L36-L38 — pub fn settlement_completed(&mut self) -> Result<(), Diagnostic>
- publish · function · L39-L41 — pub fn publish(&mut self) -> Result<(), Diagnostic>
- fail · function · L42-L50 — pub fn fail(&mut self) -> Result<(), Diagnostic>
- is_published · function · L51-L53 — pub const fn is_published(self) -> bool
- advance · function · L54-L67 — fn advance(
