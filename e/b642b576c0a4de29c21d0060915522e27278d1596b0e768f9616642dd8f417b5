use super::*;
use super::before785::top_level_declaration;
pub(super) fn reconstruct_workspace_declaration_facts_with_authority(
    modules: &[(String, ResolvedModuleInput)],
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    authority: Option<&expected_projection::private_layout::authority::invocation::scratch::ScratchInvocation<'_>>,
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let expected = expected_declaration_facts(programs)?;
    let resolved_record_iterator = modules
        .iter()
        .any(|(_, program)| crate::iterator_ops::resolved_program_uses_record_iterator(program));
    let expected_compiler = prelude_binding::expected_declaration_facts_for_programs(
        programs,
        resolved_record_iterator,
    )?;
    let mut actual = BTreeMap::new();
    for (module, resolved) in modules {
        let module_uses_record_iterator =
            crate::iterator_ops::resolved_program_uses_record_iterator(resolved);
        let source = programs
            .iter()
            .find(|program| program.module == *module)
            .expect("resolved workspace module belongs to authenticated source");
        let expected_module_compiler = prelude_binding::expected_module_declaration_facts(
            source,
            programs,
            module_uses_record_iterator,
        )?;
        let private = expected_projection::private_layout::authority::invocation::scratch::Current::derive(authority, source, programs, authored)?;
        let synthetic_main = crate::bounded_output::budgeted_format(format_args!(
            "workspace.synthetic.main.{module}"
        ));
        let synthetic_main_is_allowed = !source
            .functions
            .iter()
            .any(|function| function.name == "main")
            && !expected.contains_key(synthetic_main.as_str())
            && !expected_compiler.contains_key(synthetic_main.as_str());
        let mut compiler = BTreeMap::new();
        for declaration in resolved.declarations.workspace_declarations() {
            if declaration.identity_origin == hir::IdentityOrigin::CompilerOwned {
                let fact = WorkspaceDeclarationFact {
                    kind: declaration.kind,
                    origin: declaration.identity_origin,
                    owner: declaration
                        .owner
                        .map(|owner| crate::bounded_output::budgeted_clone(owner.as_str())),
                    path: None,
                    module: None,
                };
                if compiler
                    .insert(
                        crate::bounded_output::budgeted_clone(declaration.id.as_str()),
                        fact,
                    )
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "compiler-owned workspace declaration identity is duplicated",
                    )]);
                }
                continue;
            }
            let top = top_level_declaration(&resolved.declarations, &declaration);
            let Some(top_fact) = expected.get(top.as_str()) else {
                if synthetic_main_is_allowed
                    && declaration.id.as_str() == synthetic_main.as_str()
                    && declaration.kind == hir::DeclarationKind::Function
                    && declaration.identity_origin == hir::IdentityOrigin::Explicit
                    && declaration.owner.is_none()
                {
                    continue;
                }
                return Err(vec![graph_error(
                    "SPX-G173",
                    "resolved workspace declaration has an unauthenticated synthetic or rogue root",
                )]);
            };
            if top_fact.module.as_deref() != Some(module.as_str()) {
                let expected_foreign = expected.get(declaration.id.as_str());
                if (source
                    .module_uses
                    .iter()
                    .any(|u| u.persistent_id == top.as_str())
                    || private.contains(top.as_str(), top_fact.module.as_deref())?)
                    && expected_foreign.is_some_and(|fact| {
                        fact.kind == declaration.kind
                            && fact.origin == declaration.identity_origin
                            && fact.owner.as_deref()
                                == declaration.owner.as_ref().map(hir::DeclarationId::as_str)
                            && fact.path == top_fact.path
                            && fact.module == top_fact.module
                    })
                {
                    continue;
                }
                return Err(vec![graph_error(
                    "SPX-G173",
                    "resolved workspace declaration leaks a non-imported foreign authority",
                )]);
            }
            let fact = WorkspaceDeclarationFact {
                kind: declaration.kind,
                origin: declaration.identity_origin,
                owner: declaration
                    .owner
                    .map(|owner| crate::bounded_output::budgeted_clone(owner.as_str())),
                path: top_fact
                    .path
                    .as_deref()
                    .map(crate::bounded_output::budgeted_clone),
                module: top_fact
                    .module
                    .as_deref()
                    .map(crate::bounded_output::budgeted_clone),
            };
            if actual
                .insert(
                    crate::bounded_output::budgeted_clone(declaration.id.as_str()),
                    fact,
                )
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace declaration identity is retained more than once",
                )]);
            }
        }
        if compiler != expected_module_compiler {
            return Err(vec![graph_error(
                "SPX-G173",
                "compiler-owned prelude declaration facts disagree with the independent prelude map",
            )]);
        }
    }
    if actual != expected {
        let differing = actual
            .keys()
            .chain(expected.keys())
            .find(|id| actual.get(id.as_str()) != expected.get(id.as_str()))
            .expect("unequal declaration maps have a differing identity");
        return Err(vec![graph_error(
            "SPX-G173",
            format!(
                "authored workspace declaration facts disagree with retained HIR at `{differing}`"
            ),
        )]);
    }
    let compiler_ids = expected_compiler
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected_compiler_ids = prelude_binding::ids(programs, resolved_record_iterator);
    if compiler_ids != expected_compiler_ids {
        return Err(vec![graph_error(
            "SPX-G173",
            "compiler-owned workspace declaration facts disagree with the exact shared prelude",
        )]);
    }
    let mut facts = expected;
    for (id, fact) in expected_compiler {
        if facts.insert(id, fact).is_some() {
            return Err(vec![graph_error(
                "SPX-G173",
                "compiler-owned and authored workspace declaration identities overlap",
            )]);
        }
    }
    Ok(facts)
}

