# cleanup_plan/replay/path_summary.rs

- cleanup_plan_requires_path_replay · function · L7-L20 — pub(super) fn cleanup_plan_requires_path_replay(function: &ResolvedFunction) -> bool
- cleanup_inert_path_product_can_be_summarized · function · L22-L31 — pub(super) fn cleanup_inert_path_product_can_be_summarized(
- STATUS_ONLY_PATH_SUMMARY_THRESHOLD · constant · L33-L33 — pub(super) const STATUS_ONLY_PATH_SUMMARY_THRESHOLD: usize = 512;
- CLEANUP_INERT_EDGE_SUMMARY_THRESHOLD · constant · L34-L34 — const CLEANUP_INERT_EDGE_SUMMARY_THRESHOLD: usize = 1_024;
- cleanup_inert_large_decisions_can_be_summarized · function · L36-L39 — pub(super) fn cleanup_inert_large_decisions_can_be_summarized(function: &ResolvedFunction) -> bool
- status_only_paths_can_be_summarized · function · L46-L61 — pub(super) fn status_only_paths_can_be_summarized(function: &ResolvedFunction) -> bool
- plan_structure_units · function · L63-L81 — pub(super) fn plan_structure_units(plan: &crate::cleanup_plan::CleanupPlan) -> usize
- validate_replay_size_budget · function · L83-L124 — pub(super) fn validate_replay_size_budget(function: &ResolvedFunction) -> Result<(), Diagnostic>
