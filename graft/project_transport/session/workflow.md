# project_transport/session/workflow.rs

- DEFAULT_INLINE_BUILD_BYTES · constant · L15-L15 — const DEFAULT_INLINE_BUILD_BYTES: usize = 512 * 1024;
- rename_derive · function · L18-L79 — pub(super) fn rename_derive(
- change_preview · function · L81-L140 — pub(super) fn change_preview(
- change_impact · function · L142-L148 — pub(super) fn change_impact(
- change_review · function · L150-L156 — pub(super) fn change_review(
- change_artifact · function · L158-L221 — fn change_artifact(
- build · function · L223-L280 — pub(super) fn build(&mut self, id: &RequestId, params: Option<Map<String, Value>>) -> Vec<u8>
- render_change_result · function · L283-L285 — fn render_change_result(prepared: &crate::project::PreparedProjectRename) -> String
- tests · module · L288-L326 — mod tests
- assert_exact_response_boundary · function · L293-L303 — fn assert_exact_response_boundary(id: RequestId, result: &str)
- every_complete_v4_planning_and_build_response_has_an_exact_minimum_boundary · function · L306-L325 — fn every_complete_v4_planning_and_build_response_has_an_exact_minimum_boundary()
