# wasm/aggregate/iterator_ops.rs

- owned · module · L2-L2 — mod owned;
- OWNED_IMPORT_COUNT · constant · L4-L4 — pub(super) const OWNED_IMPORT_COUNT: u32 = owned::IMPORT_COUNT;
- owned_import_names · function · L5-L7 — pub(super) fn owned_import_names() -> [&'static str; OWNED_IMPORT_COUNT as usize]
- owned_import_base · function · L8-L10 — pub(super) fn owned_import_base(program: &ResolvedProgram) -> u32
- ITER_HANDLE_OFFSET · constant · L12-L12 — const ITER_HANDLE_OFFSET: u32 = 0;
- ITER_CURSOR_OFFSET · constant · L13-L13 — pub(super) const ITER_CURSOR_OFFSET: u32 = 8;
- STEP_TAG_OFFSET · constant · L14-L14 — const STEP_TAG_OFFSET: u32 = 0;
- STEP_ITEM_OFFSET · constant · L15-L15 — const STEP_ITEM_OFFSET: u32 = 8;
- STEP_REST_OFFSET · constant · L16-L16 — const STEP_REST_OFFSET: u32 = 16;
- bind_variant_match_fields · function · L19-L104 — pub(super) fn bind_variant_match_fields(
- copy_iterator_value · function · L106-L176 — pub(super) fn copy_iterator_value(
- emit_iterator_op · function · L178-L217 — pub(super) fn emit_iterator_op(
- iterator_argument · function · L219-L245 — fn iterator_argument(
- emit_vec_into_iter · function · L247-L290 — fn emit_vec_into_iter(
- emit_iter_next · function · L292-L408 — fn emit_iter_next(
- store_vec_element_memory_bits · function · L410-L428 — fn store_vec_element_memory_bits(&mut self, ty: &ResolvedType) -> Result<(), Diagnostic>
- clear_iterator · function · L430-L441 — pub(super) fn clear_iterator(&mut self, value: &Value) -> Result<(), Diagnostic>
