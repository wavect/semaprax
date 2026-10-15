//! Complete original preparation fork with affine selected-failure propagation.
use super::super::*;
use crate::workspace_report::WorkspaceReportFailure;

pub(super) fn prepare_owned_typed(
    authenticated_revision: String,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    base_graph: workspace_graph::WorkspaceGraphBuild,
    storage: (usize, usize, usize),
    change_set: SemanticWorkspaceChangeSet,
) -> Result<SemanticWorkspacePreparedChange, WorkspaceReportFailure> {
    if authenticated_revision != change_set.base_workspace_revision {
        return Err((stale("Semantic Workspace Change base revision is stale")).into());
    }
    if sources.len() < 2 || sources.len() > semantic_workspace::MAX_MANAGED_FILES {
        return Err(
            (replay("Semantic Workspace authenticated source cardinality disagrees")).into(),
        );
    }

    let used_total_replacement_source_bytes = change_set
        .files
        .iter()
        .try_fold(0usize, |total, file| total.checked_add(file.source.len()))
        .ok_or_else(|| {
            limit(
                "total_replacement_source_bytes",
                MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
            )
        })?;
    let declared_paths = change_set
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let authenticated_paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<BTreeSet<_>>();
    if !declared_paths.is_subset(&authenticated_paths) {
        return Err((stale("Semantic Workspace change contains an unmanaged path")).into());
    }

    let mut changes = change_set
        .files
        .into_iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    let mut base_files = Vec::with_capacity(sources.len());
    let mut candidate_sources = Vec::with_capacity(sources.len());
    for source in sources {
        let base_bytes = source.source.len();
        let candidate_source = match changes.remove(&source.path) {
            Some(change) => {
                if change.base_source_graph_schema != source.source_graph_schema
                    || change.base_source_revision != source.source_revision
                    || change.base_source_digest != source.source_digest
                {
                    return Err(
                        (stale("Semantic Workspace change base source binding is stale")).into(),
                    );
                }
                if change.source == source.source {
                    return Err((replay(
                        "Semantic Workspace change set contains an unchanged file",
                    ))
                    .into());
                }
                change.source
            }
            None => source.source,
        };
        base_files.push(SemanticWorkspaceBaseFileFact {
            path: source.path.clone(),
            source_graph_schema: source.source_graph_schema,
            source_revision: source.source_revision,
            source_digest: source.source_digest,
            bytes: base_bytes,
        });
        candidate_sources.push(semantic_workspace::SemanticWorkspaceSource {
            path: source.path,
            source: candidate_source,
        });
    }
    if !changes.is_empty() {
        return Err((replay(
            "Semantic Workspace declared change overlay was not consumed exactly once",
        ))
        .into());
    }
    base_files.sort_by(|left, right| left.path.cmp(&right.path));
    candidate_sources.sort_by(|left, right| left.path.cmp(&right.path));
    let paths = base_files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let path_set = semantic_workspace::render_path_set(&paths)?;
    let base_builder_bytes = base_graph
        .change_builder_bytes()
        .ok_or_else(|| replay("Semantic Workspace Change base fingerprint accounting is absent"))?;
    let candidate_builder_limit = semantic_workspace::MAX_CHANGE_BUILDER_BYTES
        .checked_sub(base_builder_bytes)
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
    let candidate = semantic_workspace::reporting::preflight_owned_for_change(
        &path_set,
        candidate_sources,
        candidate_builder_limit,
    )
    .map_err(|failure| {
        failure.map_legacy(|diagnostics| {
            map_change_builder_limit(diagnostics, candidate_builder_limit)
        })
    })?;
    if candidate.path_set() != paths || candidate.files().len() != base_files.len() {
        return Err((replay(
            "Semantic Workspace candidate path inventory disagrees with its authenticated base",
        ))
        .into());
    }
    if !base_graph.contains_module(&change_set.entry_module)
        || !candidate.graph().contains_module(&change_set.entry_module)
    {
        return Err((stale(
            "Semantic Workspace Change entry module is absent from the managed workspace",
        ))
        .into());
    }

    let base_by_path = base_files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let mut changed_files = Vec::with_capacity(declared_paths.len());
    for candidate_file in candidate.files() {
        let base = base_by_path.get(candidate_file.path()).ok_or_else(|| {
            replay("Semantic Workspace candidate contains an unknown managed path")
        })?;
        let changed = declared_paths.contains(candidate_file.path());
        let facts_differ = base.source_graph_schema != candidate_file.source_graph_schema()
            || base.source_revision != candidate_file.source_revision()
            || base.source_digest != candidate_file.source_digest()
            || base.bytes != candidate_file.bytes();
        if changed != facts_differ {
            return Err((replay(
                "Semantic Workspace changed-file inventory disagrees with candidate facts",
            ))
            .into());
        }
        if changed {
            changed_files.push(SemanticWorkspaceChangedFileFact {
                path: candidate_file.path().to_owned(),
                base_source_graph_schema: base.source_graph_schema.clone(),
                candidate_source_graph_schema: candidate_file.source_graph_schema().to_owned(),
                base_source_revision: base.source_revision.clone(),
                candidate_source_revision: candidate_file.source_revision().to_owned(),
                base_source_digest: base.source_digest.clone(),
                candidate_source_digest: candidate_file.source_digest().to_owned(),
                base_bytes: base.bytes,
                candidate_bytes: candidate_file.bytes(),
            });
        }
    }
    if changed_files.len() != declared_paths.len() {
        return Err((replay(
            "Semantic Workspace changed-file cardinality disagrees with its declaration",
        ))
        .into());
    }
    let candidate_workspace_revision = candidate.workspace_revision().to_owned();
    if candidate_workspace_revision == authenticated_revision {
        return Err(
            (replay("Semantic Workspace changed candidate retained the base revision")).into(),
        );
    }
    let (candidate_files, candidate_manifest, replayed_candidate_revision, candidate_graph) =
        candidate.into_snapshot_parts();
    if replayed_candidate_revision != candidate_workspace_revision {
        return Err((replay(
            "Semantic Workspace candidate revision changed while retaining its typed graph",
        ))
        .into());
    }
    let candidate_builder_bytes = candidate_graph.change_builder_bytes().ok_or_else(|| {
        replay("Semantic Workspace Change candidate fingerprint accounting is absent")
    })?;
    let remaining_delta_builder = candidate_builder_limit
        .checked_sub(candidate_builder_bytes)
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
    // Each view prepays its exact remapped literal; no replay Vec is formatted
    // after a selected refusal. Unpaid and non-replay refusals stay legacy.
    let base_graph = base_graph.into_change_view_typed(BASE_VIEW_REPLAY)?;
    let candidate_graph = candidate_graph.into_change_view_typed(CANDIDATE_VIEW_REPLAY)?;
    let prebound = delta_builder_prebound(&base_graph, &candidate_graph)?;
    let (delta, overflowed, delta_builder_bytes) = crate::bounded_output::with_limit_usage(
        remaining_delta_builder,
        || -> Result<_, WorkspaceReportFailure> {
            if !crate::bounded_output::reserve_active(prebound) {
                return Err((limit(
                    "total_analysis_builder_bytes",
                    semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
                ))
                .into());
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
            let base_workspace_graph_digest = base_graph.projection_digest(
                &authenticated_revision,
                &base_sources,
                storage.0,
                storage.1,
                storage.2,
                &change_set.entry_module,
            )?;
            let candidate_workspace_graph_digest = candidate_graph.projection_digest(
                &candidate_workspace_revision,
                &candidate_sources,
                candidate_manifest.len(),
                storage.1,
                storage.2,
                &change_set.entry_module,
            )?;
            let (roots, delta_edges) = build_delta(&base_graph, &candidate_graph, &declared_paths)?;
            let context_nodes =
                build_context_nodes(&base_graph, &candidate_graph, &roots, &delta_edges)?;
            let (impact, impact_edges) = build_impact(&base_graph, &candidate_graph, &roots)?;
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
        return Err((limit(
            "total_analysis_builder_bytes",
            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
        ))
        .into());
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
    let used_builder_bytes = base_builder_bytes
        .checked_add(candidate_builder_bytes)
        .and_then(|used| used.checked_add(delta_builder_bytes))
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
    Ok(SemanticWorkspacePreparedChange {
        base_workspace_revision: authenticated_revision,
        candidate_workspace_revision,
        entry_module: change_set.entry_module,
        proposal_source: change_set.proposal_source,
        base_workspace_graph_digest,
        candidate_workspace_graph_digest,
        base_files,
        changed_files,
        base_graph,
        candidate_graph,
        candidate_files,
        candidate_manifest,
        roots,
        delta_edges,
        context_nodes,
        impact,
        impact_edges,
        used_builder_bytes,
        used_total_replacement_source_bytes,
        retained_generations: storage.1,
        staging_attempts: storage.2,
    })
}
