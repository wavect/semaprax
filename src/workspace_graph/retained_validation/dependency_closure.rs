use std::collections::{BTreeMap, BTreeSet};

use super::super::WorkspaceResolvedModule;
use crate::diagnostic::Diagnostic;
use crate::hir;

/// Keep the established v1 function inventory unless a newer bundled member
/// is reached from that inventory.  The caller has already authenticated every
/// supplied function; an absent callee is deliberately left to the ordinary
/// linker, which reports the existing diagnostic.
pub(in crate::workspace_graph) fn retain_legacy_useful_data_dependency_closure(
    functions: Vec<hir::LinkedScalarFunction>,
    incompatible: &BTreeSet<hir::DeclarationId>,
    callees: impl Fn(&hir::ResolvedFunction) -> BTreeSet<hir::DeclarationId>,
) -> Result<Vec<hir::LinkedScalarFunction>, Vec<Diagnostic>> {
    let mut available = BTreeMap::new();
    for linked in functions {
        if available
            .insert(linked.function.id.clone(), linked)
            .is_some()
        {
            return Err(vec![super::super::graph_error(
                "SPX-G173",
                "workspace dependency closure duplicates an authenticated function",
            )]);
        }
    }
    let mut pending = available
        .keys()
        .filter(|id| !incompatible.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut retained = BTreeSet::new();
    while let Some(id) = pending.pop_first() {
        let Some(linked) = available.get(&id) else {
            return Err(vec![super::super::graph_error(
                "SPX-G173",
                "workspace dependency closure names an unauthenticated function",
            )]);
        };
        if !retained.insert(id) {
            continue;
        }
        for callee in callees(&linked.function) {
            if available.contains_key(&callee) && !retained.contains(&callee) {
                pending.insert(callee);
            }
        }
    }
    retained
        .into_iter()
        .map(|id| {
            available.remove(&id).ok_or_else(|| {
                vec![super::super::graph_error(
                    "SPX-G173",
                    "workspace dependency closure lost an authenticated function",
                )]
            })
        })
        .collect()
}

/// A bundled dependency may carry newer declarations beside its legacy scalar
/// surface.  A v1 consumer retains that newer surface only when an otherwise
/// admitted declaration calls it; authored modules never use this exception.
pub(in crate::workspace_graph) fn useful_data_v1_dependency_fallback(
    module: &WorkspaceResolvedModule,
) -> bool {
    module.path.starts_with("dependencies/")
        && (!module.types.is_empty()
            || !module.interfaces.is_empty()
            || !module.function_templates.is_empty()
            || !module.function_instances.is_empty()
            || module.functions.iter().any(|function| {
                !hir::useful_data_workspace_return_admitted(&function.return_type)
                    || function.params.iter().any(|parameter| {
                        !hir::useful_data_workspace_parameter_admitted(
                            &parameter.ty,
                            parameter.ownership,
                        )
                    })
            }))
}
