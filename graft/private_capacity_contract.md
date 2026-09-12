---
covers: []
---
# private_capacity_contract.rs

- PRELUDE_CAPACITY_IDENTITIES · constant · L6-L17 — pub(crate) const PRELUDE_CAPACITY_IDENTITIES: [&str; 10] = [
- declaration_index_upper · function · L19-L43 — pub(crate) fn declaration_index_upper(
- type_facts_layout_upper · function · L45-L56 — pub(crate) fn type_facts_layout_upper(
- resolved_type_owned_capacity · function · L58-L101 — fn resolved_type_owned_capacity(ty: &crate::hir::ResolvedType) -> Option<usize>
- shape_owned_capacity · function · L107-L130 — fn shape_owned_capacity(shape: &crate::cleanup::FieldLivenessShape) -> Option<usize>
- cleanup_inventory_owned_capacity · function · L136-L191 — pub(crate) fn cleanup_inventory_owned_capacity(
- storage_owned_capacity · function · L193-L207 — fn storage_owned_capacity(storage: &crate::cleanup_plan::StorageId) -> Option<usize>
- cleanup_place_owned_capacity · function · L209-L223 — fn cleanup_place_owned_capacity(place: &crate::cleanup_plan::CleanupPlace) -> Option<usize>
- staged_result_owned_capacity · function · L225-L282 — fn staged_result_owned_capacity(
- cleanup_plan_owned_capacity · function · L284-L491 — pub(crate) fn cleanup_plan_owned_capacity(
