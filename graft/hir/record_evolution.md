# hir/record_evolution.rs

- MAX_DECLARATIONS · constant · L5-L5 — const MAX_DECLARATIONS: usize = 4096;
- MAX_VISITS · constant · L6-L6 — const MAX_VISITS: usize = 1_048_576;
- MAX_DEPTH · constant · L7-L7 — const MAX_DEPTH: usize = 256;
- MAX_BYTES · constant · L8-L8 — const MAX_BYTES: usize = 16 * 1024 * 1024;
- Budget · struct · L10-L13 — struct Budget
- charge · function · L16-L23 — fn charge(&mut self, bytes: usize, depth: usize) -> Result<(), Diagnostic>
- capacity · function · L26-L31 — fn capacity() -> Diagnostic
- record_evolution_type_facts · function · L37-L48 — pub(crate) fn record_evolution_type_facts(
- record_evolution_concrete_type_facts · function · L52-L84 — pub(crate) fn record_evolution_concrete_type_facts(
- retain_declaration · function · L87-L159 — fn retain_declaration(
- retain_field · function · L161-L171 — fn retain_field(
- retain_type · function · L173-L200 — fn retain_type(
