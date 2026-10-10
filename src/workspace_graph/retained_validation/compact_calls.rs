use super::*;

pub(super) fn reconstruct<'a>(
    programs: &[Program],
    modules: &[WorkspaceResolvedModule],
    edges: &'a [WorkspaceEdge],
) -> Result<Vec<&'a WorkspaceEdge>, Vec<Diagnostic>> {
    let module_paths = modules
        .iter()
        .map(|module| (module.module.as_str(), module.path.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut calls = Vec::new();
    for edge in edges.iter().filter(|edge| edge.kind == "call") {
        push_edge_reference(&mut calls, edge)?;
    }
    calls.sort();
    if calls.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(mismatch());
    }
    let mut replayed = 0usize;
    for program in programs {
        let function_uses = program
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Function)
            .map(|item| (item.alias.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        if function_uses.is_empty() {
            continue;
        }
        for function in &program.functions {
            let owner =
                hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&function.stable_id));
            for (site, expressions) in [
                ("requires", function.requires.as_slice()),
                ("body", std::slice::from_ref(&function.body)),
                ("ensures", function.ensures.as_slice()),
            ] {
                for (root_index, expression) in expressions.iter().enumerate() {
                    let root = match site {
                        "requires" => crate::bounded_output::budgeted_format(format_args!(
                            "requires.{root_index}"
                        )),
                        "body" => crate::bounded_output::budgeted_clone("body"),
                        "ensures" => crate::bounded_output::budgeted_format(format_args!(
                            "ensures.{root_index}"
                        )),
                        _ => unreachable!(),
                    };
                    let mut ordinal = 0usize;
                    visit_ast_call_sites(expression, &root, &mut |name, path| {
                        let call_ordinal = ordinal;
                        ordinal = ordinal
                            .checked_add(1)
                            .ok_or_else(|| vec![limit_error("calls", MAX_CALLS)])?;
                        let Some(module_use) = function_uses.get(name) else {
                            return Ok(());
                        };
                        let target_path = module_paths
                            .get(module_use.target_module.as_str())
                            .ok_or_else(|| {
                                vec![graph_error(
                                    "SPX-G173",
                                    "authenticated call target module has no retained path",
                                )]
                            })?;
                        let expression = hir::workspace_expression_identity(&owner, path);
                        let key = (
                            program.path.as_str(),
                            function.stable_id.as_str(),
                            *target_path,
                            module_use.persistent_id.as_str(),
                            "call",
                            site,
                            expression.as_str(),
                            path,
                            module_use.alias.as_str(),
                            call_ordinal,
                        );
                        if calls
                            .binary_search_by(|edge| edge_key(edge).cmp(&key))
                            .is_err()
                        {
                            return Err(mismatch());
                        }
                        replayed = replayed.checked_add(1).ok_or_else(mismatch)?;
                        Ok(())
                    })?;
                }
            }
        }
    }
    if replayed != calls.len() {
        return Err(mismatch());
    }
    Ok(calls)
}

fn edge_key(edge: &WorkspaceEdge) -> (&str, &str, &str, &str, &str, &str, &str, &str, &str, usize) {
    (
        &edge.caller_path,
        &edge.caller,
        &edge.target_path,
        &edge.target,
        edge.kind,
        edge.site,
        &edge.expression,
        &edge.ast_path,
        &edge.alias,
        edge.ordinal,
    )
}
fn mismatch() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "emitted workspace call edges disagree with authenticated AST/HIR occurrences",
    )]
}
