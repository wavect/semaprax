# wasm/aggregate/owned_strings.rs

- work_bounds · module · L12-L12 — mod work_bounds;
- Cells · struct · L15-L18 — pub(super) struct Cells
- bounded_emission_work · function · L21-L33 — pub(super) fn bounded_emission_work(&self) -> Option<usize>
- insert · function · L34-L39 — pub(super) fn insert(&mut self, local: u32) -> Result<(), Diagnostic>
- scope · function · L41-L51 — pub(super) fn scope(
- emit_scope · function · L53-L69 — pub(super) fn emit_scope(
- emit_all · function · L71-L77 — pub(super) fn emit_all(&self, output: &mut Vec<u8>, escape: Option<u32>)
- emit_clear · function · L80-L83 — pub(super) fn emit_clear(output: &mut Vec<u8>, local: u32)
- emit_empty_guard · function · L85-L89 — pub(super) fn emit_empty_guard(output: &mut Vec<u8>, local: u32)
- emit_drop · function · L93-L102 — pub(super) fn emit_drop(output: &mut Vec<u8>, local: u32)
- emit_literal_data · function · L108-L130 — pub(super) fn emit_literal_data(
