# package_resolver/solver.rs

- ConstraintTag · enum · L13-L16 — enum ConstraintTag
- ConstraintValue · enum · L19-L22 — enum ConstraintValue
- Constraint · struct · L25-L28 — struct Constraint
- admits · function · L31-L36 — fn admits(&self, version: Version) -> bool
- State · struct · L40-L45 — struct State<'catalog, 'input>
- Solved · struct · L47-L52 — pub(super) struct Solved<'catalog, 'input>
- Search · struct · L54-L62 — struct Search<'catalog, 'input, 'context>
- solve · function · L64-L114 — pub(super) fn solve<'catalog, 'input>(
- visit · function · L117-L196 — fn visit(
- insert_dependencies · function · L198-L246 — fn insert_dependencies(
- validate_graph · function · L249-L298 — fn validate_graph(state: &mut State<'_, '_>) -> bool
- admit_edge_count · function · L300-L302 — pub(super) fn admit_edge_count(count: usize) -> bool
- admit_depth · function · L304-L306 — pub(super) fn admit_depth(depth: usize) -> bool
