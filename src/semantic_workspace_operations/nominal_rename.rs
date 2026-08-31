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
    let (subject, source_name) = source_subject(&program, target).ok_or_else(|| binding(false))?;
    if selected.kind != subject.text() || selected.name != source_name || selected.name == name {
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
    let occurrences = selected_occurrences(&view.sidecar, &operations[0]).ok_or_else(replay)?;
    if occurrences.is_empty() {
        return Err(replay());
    }
    if occurrences.len() > MAX_PLANNED_EDITS {
        return Err(limit("planned_edits", MAX_PLANNED_EDITS));
    }
    reserve_operations(
        occurrences
            .len()
            .checked_mul(
                std::mem::size_of::<PlannedEditFact>()
                    + selected.path.len()
                    + name.len()
                    + std::mem::size_of::<&PlannedEditFact>(),
            )
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut replacement_bytes = 0usize;
    let mut edits = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        let file = files
            .iter()
            .find(|file| file.path() == occurrence.path)
            .ok_or_else(replay)?;
        if file
            .source()
            .get(occurrence.span.start..occurrence.span.end)
            != Some(selected.name.as_str())
        {
            return Err(replay());
        }
        let replacement = if let Some(binding) = &occurrence.shorthand_binding {
            reserve_operations(binding.len() + 2)?;
            crate::bounded_output::budgeted_format(format_args!("{name}: {binding}"))
        } else {
            name.to_owned()
        };
        replacement_bytes = replacement_bytes
            .checked_add(replacement.len())
            .ok_or_else(|| limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES))?;
        if replacement_bytes > MAX_EDIT_REPLACEMENT_BYTES {
            return Err(limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES));
        }
        reserve_operations(occurrence.path.len())?;
        edits.push(PlannedEditFact {
            path: occurrence.path.clone(),
            start: occurrence.span.start,
            end: occurrence.span.end,
            replacement,
            operation_index: 0,
        });
    }
    edits.sort_by(|left, right| {
        (&left.path, left.start, left.end).cmp(&(&right.path, right.start, right.end))
    });
    reserve_operations(
        files.len() * std::mem::size_of::<semantic_workspace::SemanticWorkspaceSource>(),
    )?;
    let mut sources = Vec::with_capacity(files.len());
    let mut changed_bytes = 0usize;
    for file in &files {
        reserve_operations(file.path().len())?;
        let refs = edits
            .iter()
            .filter(|edit| edit.path == file.path())
            .collect::<Vec<_>>();
        let source =
            if !refs.is_empty() {
                let replacement = render_candidate_source(file.source(), &refs)?;
                validate_replacement_source_per_path(&replacement)?;
                // A renamed label can cease to share its binding's spelling (or
                // acquire it). Canonicalize the explicit source transformation so
                // shorthand remains only a projection, never a binding mutation.
                reserve_operations(replacement.len().checked_mul(4).ok_or_else(|| {
                    limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
                })?)?;
                let parsed = crate::parse(&replacement, Path::new(file.path()))
                    .map_err(|error| vec![error])?;
                let replacement = crate::format::canonical(&parsed);
                validate_replacement_source_per_path(&replacement)?;
                changed_bytes = changed_bytes
                    .checked_add(replacement.len())
                    .ok_or_else(|| {
                        limit(
                            "total_replacement_source_bytes",
                            MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                        )
                    })?;
                if changed_bytes > MAX_TOTAL_REPLACEMENT_SOURCE_BYTES {
                    return Err(limit(
                        "total_replacement_source_bytes",
                        MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                    ));
                }
                replacement
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

fn source_subject<'a>(
    program: &'a crate::ast::Program,
    target: &str,
) -> Option<(DeclarationSubject, &'a str)> {
    use crate::ast::TypeDeclarationKind;
    for declaration in &program.types {
        if !declaration.explicit_id {
            continue;
        }
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } => {
                if declaration.stable_id == target {
                    return Some((DeclarationSubject::Record, &declaration.name));
                }
                if let Some(field) = fields
                    .iter()
                    .find(|field| field.explicit_id && field.stable_id == target)
                {
                    return Some((DeclarationSubject::RecordField, &field.name));
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                if declaration.stable_id == target {
                    return Some((DeclarationSubject::Variant, &declaration.name));
                }
                for case in cases.iter().filter(|case| case.explicit_id) {
                    if case.stable_id == target {
                        return Some((DeclarationSubject::VariantCase, &case.name));
                    }
                    if let Some(field) = case
                        .fields
                        .iter()
                        .find(|field| field.explicit_id && field.stable_id == target)
                    {
                        return Some((DeclarationSubject::VariantField, &field.name));
                    }
                }
            }
            _ => {}
        }
    }
    None
}
