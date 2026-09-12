# cleanup_plan/replay/record_destructure.rs

- update · module · L13-L13 — pub(super) mod update;
- DEPTH_LIMIT · constant · L15-L15 — const DEPTH_LIMIT: usize = 64;
- OWNED_LEAF_LIMIT · constant · L16-L16 — const OWNED_LEAF_LIMIT: usize = 256;
- FIELD_WORK_LIMIT · constant · L17-L17 — const FIELD_WORK_LIMIT: usize = 4_096;
- admits_owned_match_result · function · L19-L78 — pub(super) fn admits_owned_match_result(
- finish_owned_match_result · function · L80-L103 — pub(super) fn finish_owned_match_result(
- ExpectedBinding · struct · L106-L109 — pub(super) struct ExpectedBinding
- ExpectedDestructure · struct · L111-L114 — pub(super) struct ExpectedDestructure
- contains_nested · function · L116-L125 — pub(super) fn contains_nested(fields: &[ResolvedRecordMatchPatternField]) -> bool
- function_contains · function · L127-L137 — pub(super) fn function_contains(function: &crate::hir::ResolvedFunction) -> bool
- Pending · struct · L139-L146 — struct Pending<'a>
- replay · function · L148-L310 — pub(super) fn replay(
