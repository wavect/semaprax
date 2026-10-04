use super::*;

pub(super) fn preflight_finalizer_bindings(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    scenario: &CleanupScenario,
) -> Result<BTreeMap<DeclarationId, Option<DeclarationId>>, CleanupExecutionError> {
    let known_imports = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .map(|import| import.id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = scenario
        .available_finalizer_imports
        .iter()
        .find(|import| !known_imports.contains(*import))
    {
        return Err(CleanupExecutionError::UnknownFinalizerBinding(
            unknown.clone(),
        ));
    }

    let mut bindings = BTreeMap::new();
    for action in function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
    {
        if bindings.contains_key(&action.lifecycle_id) {
            continue;
        }
        let binding = resolve_lifecycle_binding(program, &action.lifecycle_id)?;
        if let Some(import) = &binding {
            if !scenario.available_finalizer_imports.contains(import) {
                return Err(CleanupExecutionError::MissingFinalizerBinding(
                    import.clone(),
                ));
            }
        }
        bindings.insert(action.lifecycle_id.clone(), binding);
    }
    Ok(bindings)
}

fn resolve_lifecycle_binding(
    program: &ResolvedProgram,
    lifecycle: &DeclarationId,
) -> Result<Option<DeclarationId>, CleanupExecutionError> {
    if matches!(lifecycle.as_str(), "core.bytes.drop" | "core.string.drop") {
        return Ok(None);
    }
    let mut binding = None;
    for declaration in &program.types {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            continue;
        };
        if drop.id != *lifecycle {
            continue;
        }
        if binding.is_some() {
            return Err(invariant(format!(
                "lifecycle `{lifecycle}` resolves more than once"
            )));
        }
        binding = Some(match &drop.kind {
            ResolvedResourceDropKind::Trivial => None,
            ResolvedResourceDropKind::Imported { import, .. } => Some(import.clone()),
        });
    }
    binding.ok_or_else(|| invariant(format!("unknown lifecycle `{lifecycle}`")))
}
