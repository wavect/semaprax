# wasm/owned_data_exports/nested_record.rs

- MAX_DEPTH · constant · L15-L15 — const MAX_DEPTH: usize = 64;
- MAX_LEAVES · constant · L16-L16 — const MAX_LEAVES: usize = 4_096;
- MAX_OWNED_LEAVES · constant · L17-L17 — const MAX_OWNED_LEAVES: usize = 256;
- NestedRecordLayout · struct · L20-L24 — pub(in crate::wasm) struct NestedRecordLayout
- NestedLeafLayout · struct · L27-L31 — pub(in crate::wasm) struct NestedLeafLayout
- prepare · function · L33-L120 — pub(super) fn prepare(
- DerivedLeaf · type · L122-L122 — type DerivedLeaf = (Vec<crate::hir::DeclarationId>, u32, FlatRecordFieldKind);
- independently_flatten · function · L124-L184 — fn independently_flatten(
- leaf_kind · function · L186-L193 — fn leaf_kind(kind: NestedOwnedRecordLeafType) -> FlatRecordFieldKind
- emit_publication · function · L196-L275 — pub(super) fn emit_publication(
