# codegen/native_iter.rs

- owned · module · L4-L4 — mod owned;
- emit_runtime · function · L5-L35 — pub(super) fn emit_runtime(
- program_uses_iterator · function · L37-L65 — pub(super) fn program_uses_iterator(program: &crate::hir::ResolvedProgram) -> bool
- c_type · function · L67-L81 — pub(super) fn c_type(ty: &crate::hir::ResolvedType) -> Option<&'static str>
- RUNTIME_C · constant · L83-L119 — const RUNTIME_C: &str = r#"typedef struct
- item_read · function · L121-L139 — pub(super) fn item_read(carrier: &str, ty: &crate::hir::ResolvedType) -> String
- item_bits · function · L141-L147 — pub(super) fn item_bits(code: &str, ty: &crate::hir::ResolvedType) -> String
