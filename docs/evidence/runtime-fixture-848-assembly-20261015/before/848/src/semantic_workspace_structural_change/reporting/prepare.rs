//! Complete original preparation fork with affine selected-failure propagation.
use super::super::*;
use crate::workspace_report::WorkspaceReportFailure;

pub(super) fn prepare_owned_with_limit_typed(
    authenticated_revision: String,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    base_graph: workspace_graph::WorkspaceGraphBuild,
    storage: (usize, usize, usize),
    change_set: SemanticWorkspaceStructuralChangeSet,
    analysis_builder_limit: usize,
) -> Result<SemanticWorkspacePreparedStructuralChange, WorkspaceReportFailure> {
    let overlay = derive_candidate_overlay(&authenticated_revision, sources, &change_set)?;
    let (base_files, mut candidate_sources, changed_paths, supplied_source_bytes) =
        overlay.into_parts();
    candidate_sources.sort_by(|left, right| left.path.cmp(&right.path));
    let candidate_paths = candidate_sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let path_set = semantic_workspace::render_path_set(&candidate_paths)
        .map_err(|diagnostics| map_replay_diagnostics("path-set", diagnostics))?;

    let base_builder_bytes = base_graph.change_builder_bytes().ok_or_else(replay)?;
    let remaining_after_base = analysis_builder_limit
        .checked_sub(base_builder_bytes)
        .ok_or_else(|| limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))?;
    let candidate_builder_limit = remaining_after_base.min(MAX_CANDIDATE_GRAPH_BUILDER_BYTES);
    let candidate = semantic_workspace::reporting::preflight_owned_for_change(
        &path_set,
        candidate_sources,
        candidate_builder_limit,
    )
    .map_err(|failure| {
        failure.map_legacy(|diagnostics| {
            map_candidate_builder_limit(diagnostics, candidate_builder_limit)
        })
    })?;
    if candidate.path_set() != candidate_paths || candidate.files().len() != candidate_paths.len() {
        return Err((replay()).into());
    }

    let candidate_workspace_revision = candidate.workspace_revision().to_owned();
    if candidate_workspace_revision == authenticated_revision {
        return Err((replay()).into());
    }
    let (candidate_files, candidate_manifest, replayed_revision, candidate_graph) =
        candidate.into_snapshot_parts();
    if candidate_manifest.len() > MAX_CANDIDATE_MANIFEST_BYTES {
        return Err((limit("candidate_manifest_bytes", MAX_CANDIDATE_MANIFEST_BYTES)).into());
    }
    if replayed_revision != candidate_workspace_revision {
        return Err((replay()).into());
    }
    let candidate_builder_bytes = candidate_graph.change_builder_bytes().ok_or_else(replay)?;
    if candidate_builder_bytes > MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
        return Err((limit(
            "candidate_graph_builder_bytes",
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
        ))
        .into());
    }
    let remaining_delta_builder = remaining_after_base
        .checked_sub(candidate_builder_bytes)
        .ok_or_else(|| limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))?;
    let base_graph = base_graph.into_change_view_typed(REPLAY)?;
    let candidate_graph = candidate_graph.into_change_view_typed(REPLAY)?;
    let base_entry_count = base_graph
        .modules()
        .iter()
        .filter(|module| module.module() == change_set.entry_module)
        .count();
    let candidate_entry_count = candidate_graph
        .modules()
        .iter()
        .filter(|module| module.module() == change_set.entry_module)
        .count();
    if base_entry_count != 1 || candidate_entry_count != 1 {
        return Err((conflict(
            "Semantic Workspace Structural Change entry module must occur exactly once in the candidate",
        )).into());
    }

    let prebound = semantic_workspace_change::delta_builder_prebound(&base_graph, &candidate_graph)
        .map_err(|_| limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))?;
    let (delta, overflowed, delta_builder_bytes) = crate::bounded_output::with_limit_usage(
        remaining_delta_builder,
        || -> Result<_, WorkspaceReportFailure> {
            if !crate::bounded_output::reserve_active(prebound) {
                return Err((limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES)).into());
            }
            let base_sources = base_files
                .iter()
                .map(|file| workspace_graph::WorkspaceGraphChangeSourceFact {
                    path: crate::bounded_output::budgeted_clone(&file.path),
                    source_graph_schema: crate::bounded_output::budgeted_clone(
                        &file.source_graph_schema,
                    ),
                    source_revision: crate::bounded_output::budgeted_clone(&file.source_revision),
                    source_digest: crate::bounded_output::budgeted_clone(&file.source_digest),
                })
                .collect::<Vec<_>>();
            let candidate_sources = candidate_files
                .iter()
                .map(|file| workspace_graph::WorkspaceGraphChangeSourceFact {
                    path: crate::bounded_output::budgeted_clone(file.path()),
                    source_graph_schema: crate::bounded_output::budgeted_clone(
                        file.source_graph_schema(),
                    ),
                    source_revision: crate::bounded_output::budgeted_clone(file.source_revision()),
                    source_digest: crate::bounded_output::budgeted_clone(file.source_digest()),
                })
                .collect::<Vec<_>>();
            let base_workspace_graph_digest = base_graph
                .projection_digest(
                    &authenticated_revision,
                    &base_sources,
                    storage.0,
                    storage.1,
                    storage.2,
                    &change_set.entry_module,
                )
                .map_err(|diagnostics| map_replay_diagnostics("base-digest", diagnostics))?;
            let candidate_workspace_graph_digest = candidate_graph
                .projection_digest(
                    &candidate_workspace_revision,
                    &candidate_sources,
                    candidate_manifest.len(),
                    storage.1,
                    storage.2,
                    &change_set.entry_module,
                )
                .map_err(|diagnostics| map_replay_diagnostics("candidate-digest", diagnostics))?;
            let (roots, delta_edges) = semantic_workspace_change::build_structural_delta(
                &base_graph,
                &candidate_graph,
                &changed_paths,
            )
            .map_err(|diagnostics| map_replay_diagnostics("delta", diagnostics))?;
            let context_nodes = semantic_workspace_change::build_context_nodes(
                &base_graph,
                &candidate_graph,
                &roots,
                &delta_edges,
            )
            .map_err(|diagnostics| map_replay_diagnostics("context", diagnostics))?;
            let (impact, impact_edges) =
                semantic_workspace_change::build_impact(&base_graph, &candidate_graph, &roots)
                    .map_err(|diagnostics| map_replay_diagnostics("impact", diagnostics))?;
            Ok((
                base_workspace_graph_digest,
                candidate_workspace_graph_digest,
                roots,
                delta_edges,
                context_nodes,
                impact,
                impact_edges,
            ))
        },
    );
    if overflowed {
        return Err((limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES)).into());
    }
    let (
        base_workspace_graph_digest,
        candidate_workspace_graph_digest,
        roots,
        delta_edges,
        context_nodes,
        impact,
        impact_edges,
    ) = delta?;
    let used_analysis_builder_bytes = base_builder_bytes
        .checked_add(candidate_builder_bytes)
        .and_then(|used| used.checked_add(delta_builder_bytes))
        .filter(|used| *used <= analysis_builder_limit)
        .ok_or_else(|| limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))?;

    Ok(SemanticWorkspacePreparedStructuralChange {
        base_workspace_revision: authenticated_revision,
        candidate_workspace_revision,
        entry_module: change_set.entry_module,
        proposal_source: change_set.proposal_source,
        operations: change_set.operations,
        base_workspace_graph_digest,
        candidate_workspace_graph_digest,
        base_files,
        candidate_files,
        candidate_manifest,
        base_graph,
        candidate_graph,
        roots,
        delta_edges,
        context_nodes,
        impact,
        impact_edges,
        used_analysis_builder_bytes,
        used_total_supplied_source_bytes: supplied_source_bytes,
        base_manifest_bytes: storage.0,
        retained_generations: storage.1,
        staging_attempts: storage.2,
    })
}
