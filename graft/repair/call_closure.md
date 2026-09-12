# repair/call_closure.rs

- call_graph · function · L4-L35 — pub(super) fn call_graph(program: &hir::ResolvedProgram) -> Result<CallGraph, Vec<Diagnostic>>
- has_call_cycle · function · L37-L70 — pub(super) fn has_call_cycle(graph: &BTreeMap<DeclarationId, BTreeSet<DeclarationId>>) -> bool
- collect_calls · function · L72-L195 — fn collect_calls(
- tests · module · L198-L221 — mod tests
- closure_body_calls_contribute_dependencies_and_call_site_budget_without_counting_creation · function · L202-L220 — fn closure_body_calls_contribute_dependencies_and_call_site_budget_without_counting_creation()
