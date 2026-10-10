//! Stable ordinary callees, without operation or filesystem authority.
use super::*;

pub(super) fn resolved_function_callees(
    function: &hir::ResolvedFunction,
) -> BTreeSet<hir::DeclarationId> {
    let mut callees = BTreeSet::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        hir::visit_resolved_calls(expression, &mut |callee, _, _| {
            callees.insert(callee.clone());
        });
    }
    callees
}
