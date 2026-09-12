# package_resolver_v2/solver.rs

- ConstraintTag · enum · L12-L15 — enum ConstraintTag
- Constraint · struct · L18-L21 — struct Constraint
- admits · function · L24-L26 — fn admits(&self, version: Version) -> bool
- State · struct · L30-L37 — struct State<'catalog, 'input>
- Solved · struct · L39-L44 — pub(super) struct Solved<'catalog, 'input>
- Search · struct · L46-L54 — struct Search<'catalog, 'input, 'context>
- solve · function · L56-L106 — pub(super) fn solve<'catalog, 'input>(
- visit · function · L109-L188 — fn visit(
- insert_dependencies · function · L190-L245 — fn insert_dependencies(
- validate_graph · function · L248-L297 — fn validate_graph(state: &mut State<'_, '_>) -> bool
- admit_edge_count · function · L299-L301 — pub(super) fn admit_edge_count(count: usize) -> bool
- admit_depth · function · L303-L305 — pub(super) fn admit_depth(depth: usize) -> bool
