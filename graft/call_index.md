---
covers: []
---
# call_index.rs

- reset_capacity_high_water · function · L14-L16 — fn reset_capacity_high_water()
- capacity_high_water · function · L18-L20 — fn capacity_high_water() -> usize
- note_capacity_high_water · function · L22-L24 — fn note_capacity_high_water(bytes: usize)
- PersistentCallableKind · enum · L27-L30 — pub(crate) enum PersistentCallableKind
- text · function · L33-L38 — pub(crate) const fn text(self) -> &'static str
- CallRegion · enum · L42-L46 — pub(crate) enum CallRegion
- PersistentCallSite · struct · L49-L58 — pub(crate) struct PersistentCallSite
- PersistentCallIndex · struct · L60-L66 — pub(crate) struct PersistentCallIndex
- build · function · L69-L155 — pub(crate) fn build(program: &ResolvedProgram) -> Result<Self, Diagnostic>
- site · function · L157-L159 — pub(crate) fn site(&self, expression: &str) -> Option<&PersistentCallSite>
- calls_by_owner · function · L161-L163 — pub(crate) fn calls_by_owner(&self) -> &BTreeMap<DeclarationId, BTreeSet<DeclarationId>>
- callers_by_callee · function · L165-L167 — pub(crate) fn callers_by_callee(&self) -> &BTreeMap<DeclarationId, BTreeSet<DeclarationId>>
- kind · function · L169-L171 — pub(crate) fn kind(&self, owner: &DeclarationId) -> Option<PersistentCallableKind>
- origin · function · L173-L175 — pub(crate) fn origin(&self, owner: &DeclarationId) -> Option<IdentityOrigin>
- add_owner · function · L177-L231 — fn add_owner(
- visit_expr · function · L233-L410 — fn visit_expr(
- Frame · enum · L241-L244 — enum Frame<'a>
- child · function · L246-L342 — fn child(expression: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr>
- call_index_error · function · L413-L415 — fn call_index_error(message: String) -> Diagnostic
- tests · module · L418-L440 — mod tests
- expression_lookup_requires_the_exact_indexed_id · function · L422-L439 — fn expression_lookup_requires_the_exact_indexed_id()
