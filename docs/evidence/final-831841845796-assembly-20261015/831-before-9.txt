//! Complete original preparation fork with affine selected-failure propagation.
use super::super::*;
use crate::workspace_report::WorkspaceReportFailure;

pub(super) fn prepare_parsed_with_limit_typed(
    proposal: OperationsProposal,
    base: semantic_workspace::SemanticWorkspacePreflight,
    operations_builder_limit: usize,
) -> Result<PreparedSemanticWorkspaceOperations, WorkspaceReportFailure> {
    assert!(
        operations_builder_limit <= MAX_OPERATIONS_BUILDER_BYTES,
        "private Semantic Workspace Operations builder limit cannot exceed the production maximum"
    );
    let base_total = base
        .files()
        .iter()
        .try_fold(0usize, |n, f| n.checked_add(f.bytes()))
        .ok_or_else(|| limit("total_base_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
    if base_total > MAX_TOTAL_SOURCE_BYTES {
        return Err((limit("total_base_source_bytes", MAX_TOTAL_SOURCE_BYTES)).into());
    }
    let (base_files, _, base_workspace_revision, base_build) = base.into_snapshot_parts();
    let operation_view = base_build.into_operation_view_typed(REPLAY)?;
    if operation_view.builder_bytes > operations_builder_limit {
        return Err((limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)).into());
    }
    let remaining = operations_builder_limit - operation_view.builder_bytes;
    let base_builder_bytes = operation_view.builder_bytes;
    let base_change_builder_bytes = operation_view.change_builder_bytes;
    let (result, overflowed, replay_builder_bytes) =
        crate::bounded_output::with_limit_usage(remaining, || {
            prepare_owned_inner_typed(
                proposal,
                &base_workspace_revision,
                base_files,
                operation_view,
                base_change_builder_bytes,
            )
        });
    if matches!(&result, Err(failure) if failure.is_selected_refusal()) {
        return result;
    }
    if overflowed {
        return Err((limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)).into());
    }
    let mut prepared = result?;
    prepared.used_operations_builder_bytes =
        base_builder_bytes
            .checked_add(replay_builder_bytes)
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    Ok(prepared)
}

pub(super) fn prepare_owned_inner_typed(
    proposal: OperationsProposal,
    base_workspace_revision: &str,
    base_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    operation_view: workspace_graph::WorkspaceGraphOperationView,
    base_change_builder_bytes: usize,
) -> Result<PreparedSemanticWorkspaceOperations, WorkspaceReportFailure> {
    // Account for the transient serde tree, retained typed operation objects,
    // and their strings before parsing allocates any of them. Four payloads is
    // a conservative bound for this shallow, capped canonical grammar.
    reserve_operations(
        proposal
            .source
            .len()
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(MAX_OPERATIONS * std::mem::size_of::<Operation>()))
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    if base_workspace_revision != proposal.base_workspace_revision {
        return Err((binding(false)).into());
    }
    if !operation_view
        .graph
        .modules()
        .iter()
        .any(|module| module.module() == proposal.entry_module)
    {
        return Err((binding(false)).into());
    }
    validate_candidate_namespaces(&operation_view.sidecar, &proposal.operations)?;
    reserve_operations(
        base_files
            .len()
            .checked_mul(std::mem::size_of::<(
                String,
                semantic_workspace::SemanticWorkspaceFileFact,
            )>())
            .and_then(|bytes| {
                bytes.checked_add(
                    base_files
                        .iter()
                        .map(|file| file.path().len())
                        .sum::<usize>(),
                )
            })
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let base_paths = base_files
        .iter()
        .map(|file| crate::bounded_output::budgeted_clone(file.path()))
        .collect::<Vec<_>>();
    let mut sources = base_files
        .into_iter()
        .map(|file| (file.path().to_owned(), file))
        .collect::<BTreeMap<_, _>>();
    let mut edits = Vec::new();
    for (index, operation) in proposal.operations.iter().enumerate() {
        let file = sources
            .get(operation.path())
            .ok_or_else(|| binding(matches!(operation, Operation::ImportAlias { .. })))?;
        let remaining_edits = MAX_PLANNED_EDITS
            .checked_sub(edits.len())
            .ok_or_else(|| limit("planned_edits", MAX_PLANNED_EDITS))?;
        let mut selected = select_occurrences(
            file.source(),
            operation,
            &operation_view.sidecar,
            remaining_edits,
        )?;
        if selected.is_empty() {
            return Err((conflict(
                "Semantic Workspace Operations source occurrence proof is incomplete",
            ))
            .into());
        }
        let new_edit_count = edits
            .len()
            .checked_add(selected.len())
            .ok_or_else(|| limit("planned_edits", MAX_PLANNED_EDITS))?;
        if new_edit_count > MAX_PLANNED_EDITS {
            return Err((limit("planned_edits", MAX_PLANNED_EDITS)).into());
        }
        for (start, end) in selected.drain(..) {
            reserve_operations(
                std::mem::size_of::<PlannedEditFact>()
                    .checked_add(operation.path().len())
                    .and_then(|bytes| bytes.checked_add(operation.to().len()))
                    .ok_or_else(|| {
                        limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
                    })?,
            )?;
            edits.push(PlannedEditFact {
                path: operation.path().to_owned(),
                start,
                end,
                replacement: operation.to().to_owned(),
                operation_index: index,
            });
        }
    }
    if edits.len() > MAX_PLANNED_EDITS {
        return Err((limit("planned_edits", MAX_PLANNED_EDITS)).into());
    }
    edits.sort_by(|a, b| {
        (&a.path, a.start, a.end, a.operation_index).cmp(&(
            &b.path,
            b.start,
            b.end,
            b.operation_index,
        ))
    });
    if edits
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path && pair[0].end > pair[1].start)
    {
        return Err(
            (conflict("Semantic Workspace Operations derived source edit ranges overlap")).into(),
        );
    }
    let replacement_bytes = edits
        .iter()
        .try_fold(0usize, |n, e| n.checked_add(e.replacement.len()))
        .ok_or_else(|| limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES))?;
    if replacement_bytes > MAX_EDIT_REPLACEMENT_BYTES {
        return Err((limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES)).into());
    }
    let mut by_path = BTreeMap::<String, Vec<&PlannedEditFact>>::new();
    for edit in &edits {
        reserve_operations(
            std::mem::size_of::<(String, Vec<&PlannedEditFact>)>()
                .checked_add(std::mem::size_of::<&PlannedEditFact>())
                .and_then(|bytes| bytes.checked_add(edit.path.len()))
                .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
        )?;
        by_path.entry(edit.path.clone()).or_default().push(edit);
    }
    let mut candidate_sources = Vec::with_capacity(sources.len());
    let mut retained_base_files = Vec::with_capacity(sources.len());
    reserve_operations(
        sources
            .len()
            .checked_mul(std::mem::size_of::<
                semantic_workspace::SemanticWorkspaceSource,
            >())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut replacement_total = 0usize;
    let mut change_files = Vec::new();
    for path in &base_paths {
        let file = sources.remove(path).ok_or_else(replay)?;
        let source =
            if let Some(path_edits) = by_path.get(path) {
                render_candidate_source(file.source(), path_edits)?
            } else {
                reserve_operations(path.len().checked_add(file.source().len()).ok_or_else(
                    || limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES),
                )?)?;
                file.source().to_owned()
            };
        if by_path.contains_key(path) {
            validate_replacement_source_per_path(&source)?;
            replacement_total = replacement_total.checked_add(source.len()).ok_or_else(|| {
                limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                )
            })?;
            if replacement_total > MAX_TOTAL_REPLACEMENT_SOURCE_BYTES {
                return Err((limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                ))
                .into());
            }
            reserve_operations(
                std::mem::size_of::<semantic_workspace_change::SemanticWorkspaceChangeFile>()
                    .checked_add(path.len())
                    .and_then(|bytes| bytes.checked_add(file.source_graph_schema().len()))
                    .and_then(|bytes| bytes.checked_add(file.source_revision().len()))
                    .and_then(|bytes| bytes.checked_add(file.source_digest().len()))
                    .and_then(|bytes| bytes.checked_add(source.len()))
                    .ok_or_else(|| {
                        limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
                    })?,
            )?;
            change_files.push(semantic_workspace_change::SemanticWorkspaceChangeFile::new(
                path.clone(),
                file.source_graph_schema().to_owned(),
                file.source_revision().to_owned(),
                file.source_digest().to_owned(),
                source.clone(),
            )?);
        }
        candidate_sources.push(semantic_workspace::SemanticWorkspaceSource {
            path: path.clone(),
            source,
        });
        retained_base_files.push(file);
    }
    let candidate_total = candidate_sources
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.source.len()))
        .ok_or_else(|| limit("total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
    if candidate_total > MAX_TOTAL_SOURCE_BYTES {
        return Err((limit("total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES)).into());
    }
    let path_set = semantic_workspace::render_path_set(&base_paths)?;
    let remaining_operations = crate::bounded_output::active_remaining()
        .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    let candidate_graph_limit = MAX_CANDIDATE_GRAPH_BUILDER_BYTES.min(remaining_operations);
    #[cfg(test)]
    mark_candidate_preflight_entry();
    let candidate = semantic_workspace::reporting::preflight_owned_for_operations(
        &path_set,
        candidate_sources,
        candidate_graph_limit,
        remaining_operations,
    )
    .map_err(|failure| {
        failure.map_legacy(|diagnostics| {
            map_candidate_diagnostics(diagnostics, candidate_graph_limit, remaining_operations)
        })
    })?;
    let (candidate_files, candidate_manifest, candidate_revision, candidate_build) =
        candidate.into_snapshot_parts();
    let candidate_view = candidate_build.into_operation_view_typed(REPLAY)?;
    let candidate_change_builder_bytes = candidate_view.change_builder_bytes;
    reserve_operations(candidate_view.builder_bytes)?;
    replay_candidate(&operation_view, &candidate_view, &proposal.operations)?;
    let derived_change = semantic_workspace_change::SemanticWorkspaceChangeSet::new(
        proposal.base_workspace_revision.clone(),
        proposal.entry_module.clone(),
        change_files,
    )?;
    if derived_change.source().len() > MAX_DERIVED_CHANGE_PROPOSAL_BYTES
        || semantic_workspace_change::parse_proposal(derived_change.source())?.source()
            != derived_change.source()
    {
        return Err((replay()).into());
    }
    if candidate_revision == proposal.base_workspace_revision {
        return Err((replay()).into());
    }
    // Preserve the frozen Operations builder accounting for the retained
    // candidate source projection. The Evidence vertical retains the richer
    // file facts in the same allocation envelope instead of constructing a
    // duplicate full source vector.
    reserve_operations(
        candidate_files
            .len()
            .checked_mul(std::mem::size_of::<
                semantic_workspace::SemanticWorkspaceSource,
            >())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let affected_paths = by_path.len();
    let usage = OperationsUsageFacts {
        managed_files: operation_view.graph.used_managed_files(),
        operations: proposal.operations.len(),
        affected_paths,
        planned_edits: edits.len(),
        edit_replacement_bytes: replacement_bytes,
        total_base_source_bytes: operation_view.graph.used_total_source_bytes(),
        total_candidate_source_bytes: candidate_total,
        total_replacement_source_bytes: replacement_total,
        entry_module_bytes: proposal.entry_module.len(),
        operations_proposal_bytes: proposal.source.len(),
        candidate_graph_builder_bytes: candidate_view.graph.used_builder_bytes(),
        derived_changed_files: affected_paths,
        derived_change_proposal_bytes: derived_change.source().len(),
    };
    Ok(PreparedSemanticWorkspaceOperations {
        proposal_source: proposal.source,
        proposal_digest: proposal.digest,
        operations: proposal.operations,
        edits,
        base_files: retained_base_files,
        candidate_files,
        derived_change,
        base_graph: operation_view.graph,
        candidate_graph: candidate_view.graph,
        base_change_builder_bytes,
        candidate_change_builder_bytes,
        base_workspace_revision: base_workspace_revision.to_owned(),
        candidate_workspace_revision: candidate_revision,
        candidate_manifest,
        entry_module: proposal.entry_module,
        usage,
        used_operations_builder_bytes: 0,
    })
}
