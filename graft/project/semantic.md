# project/semantic.rs

- PROJECT_SEMANTIC_GRAPH_SCHEMA · constant · L16-L16 — pub const PROJECT_SEMANTIC_GRAPH_SCHEMA: &str = "semaprax.project-semantic-graph.v1";
- PROJECT_SEMANTIC_CONTEXT_SCHEMA · constant · L17-L17 — pub const PROJECT_SEMANTIC_CONTEXT_SCHEMA: &str = "semaprax.project-semantic-context.v1";
- PROJECT_SEMANTIC_IMPACT_SCHEMA · constant · L18-L18 — pub const PROJECT_SEMANTIC_IMPACT_SCHEMA: &str = crate::workspace_analysis::PROJECT_IMPACT_SCHEMA;
- ProjectSemanticState · struct · L20-L26 — pub(super) struct ProjectSemanticState
- ProjectRenameFunction · struct · L29-L34 — pub(super) struct ProjectRenameFunction
- new · function · L37-L83 — pub(super) fn new(
- graph · function · L85-L87 — pub(super) fn graph(&self) -> &str
- graph_digest · function · L89-L91 — pub(super) fn graph_digest(&self) -> &str
- image_indexes · function · L93-L95 — pub(super) fn image_indexes(&self) -> serde_json::Value
- image_modules · function · L97-L99 — pub(super) fn image_modules(&self) -> &[workspace_graph::WorkspaceGraphProjectionModule]
- image_edges · function · L101-L103 — pub(super) fn image_edges(&self) -> &[workspace_graph::WorkspaceEdge]
- image_symbol · function · L105-L111 — pub(super) fn image_symbol(&self, id: &str) -> Option<serde_json::Value>
- rename_function · function · L113-L115 — pub(super) fn rename_function(&self, stable_id: &str) -> Option<&ProjectRenameFunction>
- display_rename_equivalent · function · L117-L120 — pub(super) fn display_rename_equivalent(&self, candidate: &Self) -> bool
- context · function · L122-L143 — pub(super) fn context(
- impact · function · L145-L166 — pub(super) fn impact(
- tests · module · L171-L171 — mod tests;
