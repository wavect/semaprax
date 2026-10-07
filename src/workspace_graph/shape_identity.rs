//! Exact retained declaration and owner identity checks.
use super::*;
pub(super) fn require_retained_shape_fact<'a>(
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    module: &WorkspaceResolvedModule,
    id: &'a str,
    kind: hir::DeclarationKind,
    owner: Option<&str>,
    seen: &mut BTreeSet<&'a str>,
) -> Result<(), Vec<Diagnostic>> {
    let fact = facts.get(id);
    if !fact.is_some_and(|fact| {
        fact.kind == kind
            && fact.owner.as_deref() == owner
            && fact.path.as_deref() == Some(module.path.as_str())
            && fact.module.as_deref() == Some(module.module.as_str())
    }) || !seen.insert(id)
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "retained workspace declaration shape disagrees with authored identity facts",
        )]);
    }
    Ok(())
}

pub(super) fn top_level_declaration(
    index: &hir::DeclarationIndex,
    declaration: &hir::Declaration,
) -> hir::DeclarationId {
    let mut current = declaration;
    while let Some(owner) = &current.owner {
        let Some(parent) = index.declaration(owner) else {
            break;
        };
        current = parent;
    }
    hir::DeclarationId::new(crate::bounded_output::budgeted_clone(current.id.as_str()))
}
