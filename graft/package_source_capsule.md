---
covers: []
---
# package_source_capsule.rs

- admission · module · L6-L6 — mod admission;
- model · module · L7-L7 — mod model;
- wire · module · L8-L8 — mod wire;
- generate · function · L23-L39 — pub fn generate(
- verify · function · L41-L64 — pub fn verify(
- verify_for_linked_build · function · L70-L87 — pub(crate) fn verify_for_linked_build(
- verify_for_semantic_graph · function · L89-L106 — pub(crate) fn verify_for_semantic_graph(
- semantic_graph_source_digest · function · L111-L113 — pub(crate) fn semantic_graph_source_digest(source: &str) -> String
- verify_linked · function · L115-L151 — fn verify_linked(
- option_error · function · L153-L155 — fn option_error(message: impl Into<String>) -> Diagnostic
- authentication_error · function · L156-L158 — fn authentication_error(message: impl Into<String>) -> Diagnostic
- association_error · function · L159-L161 — fn association_error(message: impl Into<String>) -> Diagnostic
- profile_error · function · L162-L164 — fn profile_error(message: impl Into<String>) -> Diagnostic
- limit_error · function · L165-L167 — fn limit_error(message: impl Into<String>) -> Diagnostic
- wire_error · function · L168-L170 — fn wire_error(message: impl Into<String>) -> Diagnostic
- replay_error · function · L171-L173 — fn replay_error(message: impl Into<String>) -> Diagnostic
- map_nested_error · function · L175-L186 — fn map_nested_error(error: &Diagnostic) -> Diagnostic
- map_graph_error · function · L188-L195 — fn map_graph_error(error: Diagnostic) -> Diagnostic
- tests · module · L198-L198 — mod tests;
