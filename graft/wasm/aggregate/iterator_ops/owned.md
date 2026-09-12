# wasm/aggregate/iterator_ops/owned.rs

- IMPORT_COUNT · constant · L5-L5 — pub(super) const IMPORT_COUNT: u32 = 3;
- import_names · function · L7-L13 — pub(super) const fn import_names() -> [&'static str; IMPORT_COUNT as usize]
- import_base · function · L15-L28 — pub(super) fn import_base(program: &ResolvedProgram) -> u32
- emit_owned_vec_into_iter · function · L31-L71 — pub(super) fn emit_owned_vec_into_iter(
- emit_owned_iter_next · function · L73-L121 — pub(super) fn emit_owned_iter_next(
- poison_iterator_frame · function · L123-L125 — fn poison_iterator_frame(&mut self, pointer: Pointer)
- poison_step_frame · function · L127-L129 — fn poison_step_frame(&mut self, pointer: Pointer)
- poison_frame · function · L131-L138 — fn poison_frame(&mut self, pointer: Pointer, size: i64)
- trap_if_i64_nonzero_at · function · L140-L146 — fn trap_if_i64_nonzero_at(&mut self, pointer: Pointer)
- validate_iterator_frame · function · L148-L172 — fn validate_iterator_frame(&mut self, pointer: Pointer)
- validate_step_frame · function · L174-L265 — fn validate_step_frame(&mut self, pointer: Pointer, source: Pointer)
- emit_owned_iterator_status · function · L267-L289 — fn emit_owned_iterator_status(&mut self, expression: &ResolvedExpr) -> Result<(), Diagnostic>
