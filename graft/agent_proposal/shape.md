# agent_proposal/shape.rs

- Representation · enum · L17-L24 — pub(crate) enum Representation
- name · function · L27-L36 — pub(crate) fn name(self) -> &'static str
- bounds · function · L39-L47 — pub(crate) fn bounds(self) -> Option<(&'static str, &'static str)>
- of · function · L49-L69 — fn of(ty: &ResolvedType) -> Option<Self>
- FieldRow · struct · L74-L77 — pub(crate) struct FieldRow
- CaseRow · struct · L81-L84 — pub(crate) struct CaseRow
- Shape · enum · L88-L91 — pub(crate) enum Shape
- kind · function · L94-L99 — pub(crate) fn kind(&self) -> &'static str
- derive · function · L109-L114 — pub(crate) fn derive(
- derive_role · function · L121-L171 — pub(crate) fn derive_role(
- field_rows · function · L173-L196 — fn field_rows(
- persistent · function · L198-L213 — fn persistent(
- render · function · L217-L241 — pub(crate) fn render(shape: &Shape) -> String
- render_fields · function · L243-L270 — fn render_fields(fields: &[FieldRow]) -> String
