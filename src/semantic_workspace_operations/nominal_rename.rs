//! Source-only reuse of the checked Operations occurrence and replay engine.
//! This private route does not parse or relax the public multi-operation wire.

use super::*;

pub(crate) fn derive_nominal_rename(
    sources: Vec<semantic_workspace::SemanticWorkspaceSource>,
    entry_module: &str,
    target: &str,
    name: &str,
) -> Result<Vec<semantic_workspace::SemanticWorkspaceSource>, Vec<Diagnostic>> {
    bounded(target, "target_id_bytes", MAX_TARGET_ID_BYTES, false)?;
    bounded(name, "name_bytes", MAX_NAME_BYTES, true)?;
    if !valid_qualified_module(entry_module) || entry_module.len() > MAX_ENTRY_MODULE_BYTES {
        return Err(binding(false));
    }
    // Preflight owns canonicalization, source cardinality, and source byte limits.
    if sources.is_empty() || sources.len() > MAX_AFFECTED_PATHS {
        return Err(limit("managed_files", MAX_AFFECTED_PATHS));
    }
    if sources
        .iter()
        .any(|source| source.path.len() > MAX_PATH_BYTES)
    {
        return Err(limit("path_bytes", MAX_PATH_BYTES));
    }
    let mut paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    let path_set = semantic_workspace::render_path_set(&paths)?;
    let base = semantic_workspace::preflight_owned_for_operations(
        &path_set,
        sources,
        MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
        MAX_OPERATIONS_BUILDER_BYTES,
    )
    .map_err(map_base_operations_builder_limit)?;
    let (files, _, _, build) = base.into_snapshot_parts();
    let view = build.into_operation_view()?;
    let remaining = MAX_OPERATIONS_BUILDER_BYTES
        .checked_sub(view.builder_bytes)
        .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    let (result, overflowed, _) = crate::bounded_output::with_limit_usage(remaining, || {
        derive(files, view, &path_set, entry_module, target, name)
    });
    if overflowed {
        return Err(limit(
            "operations_builder_bytes",
            MAX_OPERATIONS_BUILDER_BYTES,
        ));
    }
    result
}

fn derive(
    files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    view: workspace_graph::WorkspaceGraphOperationView,
    path_set: &str,
    entry_module: &str,
    target: &str,
    name: &str,
) -> Result<Vec<semantic_workspace::SemanticWorkspaceSource>, Vec<Diagnostic>> {
    if !view
        .graph
        .modules()
        .iter()
        .any(|module| module.module() == entry_module)
    {
        return Err(binding(false));
    }
    let selected = view
        .sidecar
        .declarations
        .iter()
        .find(|declaration| declaration.explicit && declaration.id == target)
        .ok_or_else(|| binding(false))?;
    let file = files
        .iter()
        .find(|file| file.path() == selected.path)
        .ok_or_else(|| binding(false))?;
    // The historical sidecar groups classes with records. Authenticate the
    // actual source declaration kind rather than widening this private route.
    reserve_operations(
        file.source()
            .len()
            .checked_mul(4)
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let program =
        crate::parse(file.source(), Path::new(file.path())).map_err(|error| vec![error])?;
    let declaration = program
        .types
        .iter()
        .find(|declaration| declaration.explicit_id && declaration.stable_id == target)
        .ok_or_else(|| binding(false))?;
    let subject = match declaration.kind {
        crate::ast::TypeDeclarationKind::Record { .. } => DeclarationSubject::Record,
        crate::ast::TypeDeclarationKind::Variant { .. } => DeclarationSubject::Variant,
        _ => return Err(binding(false)),
    };
    if selected.kind != subject.text() || selected.name != declaration.name || selected.name == name
    {
        return Err(binding(false));
    }
    reserve_operations(
        std::mem::size_of::<Operation>()
            + selected.path.len()
            + target.len()
            + selected.name.len()
            + name.len(),
    )?;
    let operation = Operation::Declaration {
        path: selected.path.clone(),
        subject,
        target_id: target.to_owned(),
        from: selected.name.clone(),
        to: name.to_owned(),
    };
    let operations = [operation];
    validate_candidate_namespaces(&view.sidecar, &operations)?;
    let mut spans = select_occurrences(
        file.source(),
        &operations[0],
        &view.sidecar,
        MAX_PLANNED_EDITS,
    )?;
    if spans.is_empty() {
        return Err(replay());
    }
    spans.sort_unstable();
    let replacement_bytes = spans
        .len()
        .checked_mul(name.len())
        .ok_or_else(|| limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES))?;
    if replacement_bytes > MAX_EDIT_REPLACEMENT_BYTES {
        return Err(limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES));
    }
    reserve_operations(
        spans
            .len()
            .checked_mul(
                std::mem::size_of::<PlannedEditFact>()
                    + selected.path.len()
                    + name.len()
                    + std::mem::size_of::<&PlannedEditFact>(),
            )
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let edits = spans
        .into_iter()
        .map(|(start, end)| PlannedEditFact {
            path: selected.path.clone(),
            start,
            end,
            replacement: name.to_owned(),
            operation_index: 0,
        })
        .collect::<Vec<_>>();
    let refs = edits.iter().collect::<Vec<_>>();
    let replacement = render_candidate_source(file.source(), &refs)?;
    validate_replacement_source_per_path(&replacement)?;
    reserve_operations(
        files.len() * std::mem::size_of::<semantic_workspace::SemanticWorkspaceSource>(),
    )?;
    let mut replacement = Some(replacement);
    let mut sources = Vec::with_capacity(files.len());
    for file in &files {
        reserve_operations(file.path().len())?;
        let source = if file.path() == selected.path {
            replacement.take().ok_or_else(replay)?
        } else {
            reserve_operations(file.source().len())?;
            file.source().to_owned()
        };
        sources.push(semantic_workspace::SemanticWorkspaceSource {
            path: file.path().to_owned(),
            source,
        });
    }
    let remaining = crate::bounded_output::active_remaining()
        .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    let graph_limit = MAX_CANDIDATE_GRAPH_BUILDER_BYTES.min(remaining);
    let candidate = semantic_workspace::preflight_owned_for_operations(
        path_set,
        sources,
        graph_limit,
        remaining,
    )
    .map_err(|diagnostics| map_candidate_diagnostics(diagnostics, graph_limit, remaining))?;
    let (candidate_files, _, _, candidate_build) = candidate.into_snapshot_parts();
    let candidate_view = candidate_build.into_operation_view()?;
    reserve_operations(candidate_view.builder_bytes)?;
    replay_candidate(&view, &candidate_view, &operations)?;
    reserve_operations(
        candidate_files.len() * std::mem::size_of::<semantic_workspace::SemanticWorkspaceSource>(),
    )?;
    candidate_files
        .into_iter()
        .map(|file| {
            reserve_operations(file.path().len() + file.source().len())?;
            Ok(semantic_workspace::SemanticWorkspaceSource {
                path: file.path().to_owned(),
                source: file.source().to_owned(),
            })
        })
        .collect()
}
