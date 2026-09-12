# agent_interaction_schema/render.rs

- NONCLAIMS · constant · L17-L26 — const NONCLAIMS: [&str; 8] = [
- render_types · function · L30-L40 — pub(crate) fn render_types(types: &[TypeDecl]) -> String
- render_type · function · L42-L64 — fn render_type(decl: &TypeDecl) -> String
- render_case · function · L66-L72 — fn render_case(case: &CaseRow) -> String
- render_fields · function · L74-L88 — fn render_fields(fields: &[FieldRow]) -> String
- render_type_ref · function · L90-L100 — pub(crate) fn render_type_ref(ty: &FieldType) -> String
- render_scalar_ref · function · L102-L119 — fn render_scalar_ref(representation: Representation) -> String
- render_revision_body · function · L123-L128 — pub(crate) fn render_revision_body(root_type_id: &str, rendered_types: &str) -> String
- render_schema · function · L132-L151 — pub(crate) fn render_schema(
