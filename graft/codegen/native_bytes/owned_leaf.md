# codegen/native_bytes/owned_leaf.rs

- OwnedLeafKind · enum · L6-L11 — pub(super) enum OwnedLeafKind
- c_type · function · L14-L21 — pub(super) fn c_type(self) -> &'static str
- move_call · function · L23-L30 — pub(super) fn move_call(self, source: &str) -> String
- drop_call · function · L32-L39 — pub(super) fn drop_call(self, value: &str) -> String
- emit_transfer · function · L42-L59 — pub(super) fn emit_transfer(source: &ByteSlot, destination: &ByteSlot, context: &str) -> String
