//! Source-only checked preflight retaining selected-replay report ownership.
//! Ordinary grammar, directory, frontend and output failures remain legacy
//! reports. This API does not acquire a workspace lock or publish ACTIVE.
use super::*;
use crate::workspace_report::WorkspaceReportFailure;

/// Opaque checked source facts. No mutable HIR, graph or report authority leaks.
pub struct CheckedPreflight(SemanticWorkspacePreflight);
impl CheckedPreflight {
    /// Borrow canonical manifest bytes already owned by this checked preflight.
    pub fn manifest(&self) -> &str {
        &self.0.manifest
    }
    /// Borrow the revision bound to the full checked manifest.
    pub fn workspace_revision(&self) -> &str {
        &self.0.workspace_revision
    }
    /// Independently replay all held sources and manifest facts again.
    pub fn replay(&self) -> Result<(), WorkspaceReportFailure> {
        replay_typed(&self.0, None)
    }
}

/// Authenticate and check a complete canonical path set and its owned sources.
/// Both selected-replay receiving passes preserve their same-invocation report
/// carrier on failure. Initial carrier refusal has no implicit Vec conversion.
/// Surrounding source/directory/output allocation remains explicitly legacy.
pub fn preflight(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
) -> Result<CheckedPreflight, WorkspaceReportFailure> {
    preflight_typed(path_set_source, sources, None, false, None, None).map(CheckedPreflight)
}

// Keep the complete original Vec receiver in the parent as an independent
// oracle. These typed twins preserve its order and every independent check.
fn preflight_typed(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: Option<usize>,
    retain_operations: bool,
    graph_builder_limit: Option<usize>,
    mut frontend: Option<&mut crate::project::incremental::FrontendPass>,
) -> Result<SemanticWorkspacePreflight, WorkspaceReportFailure> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span =
        crate::workflow_profile::span(crate::workflow_profile::Stage::WorkspacePreflight);
    let path_set = parse_path_set(path_set_source)?;
    if sources.len() != path_set.len() {
        return Err(grammar(
            "semantic workspace owned sources disagree with the canonical path set",
        )
        .into());
    }
    let mut seen = BTreeSet::new();
    let mut source_paths = Vec::with_capacity(sources.len());
    let mut total_source_bytes = 0usize;
    for source in &sources {
        if !workspace::evidence_path_is_valid(&source.path) {
            return Err(grammar("semantic workspace source path is not canonical").into());
        }
        if !seen.insert(source.path.clone()) {
            return Err(grammar("semantic workspace source paths are not unique").into());
        }
        total_source_bytes = total_source_bytes
            .checked_add(source.source.len())
            .ok_or_else(|| storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
            return Err(storage_limit("total_source_bytes", MAX_TOTAL_SOURCE_BYTES).into());
        }
        source_paths.push(source.path.clone());
    }
    source_paths.sort();
    if source_paths != path_set {
        return Err(grammar(
            "semantic workspace owned sources disagree with the canonical path set",
        )
        .into());
    }

    let graph_sources = sources
        .into_iter()
        .map(|source| workspace_graph::WorkspaceSource {
            path: source.path,
            source: source.source,
        })
        .collect();
    let (graph, recovered_sources) = if let Some(change_builder_limit) = change_builder_limit {
        workspace_graph::build_owned_retaining_sources_with_frontend_limit::<WorkspaceReportFailure>(
            graph_sources,
            if retain_operations {
                graph_builder_limit.expect("operations preflight supplies Graph limit")
            } else {
                workspace_graph::MAX_BUILDER_BYTES
            },
            Some(change_builder_limit),
            retain_operations,
            None,
        )?
    } else {
        workspace_graph::build_owned_retaining_sources_with_frontend_limit::<WorkspaceReportFailure>(
            graph_sources,
            workspace_graph::MAX_BUILDER_BYTES,
            None,
            false,
            frontend.as_deref_mut(),
        )?
    };
    // The exact sources this build consumed are the receiving authority.
    let mut schemas = graph.source_graph_schemas_receiving_typed(&recovered_sources)?;
    let mut files = Vec::with_capacity(recovered_sources.len());
    for source in recovered_sources {
        let schema = schemas.remove(&source.path).ok_or_else(|| {
            invariant("semantic workspace source Graph schema is absent from the resolved build")
        })?;
        let source_revision = source_revision_with_frontend(
            &source.path,
            &source.source,
            schema,
            frontend.as_deref(),
        )?;
        files.push(SemanticWorkspaceFileFact {
            path: source.path.clone(),
            source_graph_schema: schema.to_owned(),
            source_revision,
            source_digest: review::source_digest(source.source.as_bytes()),
            bytes: source.source.len(),
            source: workspace_graph::current_source::SourceBytes::current(source),
        });
    }
    if !schemas.is_empty() {
        return Err(invariant(
            "semantic workspace resolved module facts contain an unknown source path",
        )
        .into());
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = render_manifest(&files)?;
    let workspace_revision = semantic_workspace_revision(&manifest);
    let preflight = SemanticWorkspacePreflight {
        path_set,
        files,
        manifest,
        workspace_revision,
        graph,
    };
    replay_typed(&preflight, frontend.as_deref())?;
    Ok(preflight)
}

fn replay_typed(
    preflight: &SemanticWorkspacePreflight,
    frontend: Option<&crate::project::incremental::FrontendPass>,
) -> Result<(), WorkspaceReportFailure> {
    let schemas = preflight
        .graph
        .source_graph_schemas_receiving_typed(&preflight.files)?;
    if schemas.len() != preflight.files.len() || preflight.path_set.len() != preflight.files.len() {
        return Err(invariant("Semantic Workspace preflight fact cardinality disagrees").into());
    }
    for (path, file) in preflight.path_set.iter().zip(&preflight.files) {
        if path != &file.path
            || schemas.get(path).copied() != Some(file.source_graph_schema.as_str())
            || file.bytes != file.source.len()
            || source_revision_with_frontend(
                &file.path,
                &file.source,
                &file.source_graph_schema,
                frontend,
            )? != file.source_revision
            || review::source_digest(file.source.as_bytes()) != file.source_digest
        {
            return Err(invariant(
                "Semantic Workspace preflight file facts disagree with independent replay",
            )
            .into());
        }
    }
    let replayed_manifest = parse_manifest(&preflight.manifest)?;
    if replayed_manifest.len() != preflight.files.len()
        || replayed_manifest
            .iter()
            .zip(&preflight.files)
            .any(|(replayed, expected)| !same_manifest_fact(replayed, expected))
    {
        return Err(invariant(
            "Semantic Workspace manifest facts disagree with independent grammar replay",
        )
        .into());
    }
    if semantic_workspace_revision(&preflight.manifest) != preflight.workspace_revision {
        return Err(invariant(
            "Semantic Workspace manifest or revision disagrees with independent replay",
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

pub(crate) fn preflight_owned_for_change(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    change_builder_limit: usize,
) -> Result<SemanticWorkspacePreflight, WorkspaceReportFailure> {
    preflight_typed(
        path_set_source,
        sources,
        Some(change_builder_limit),
        false,
        None,
        None,
    )
}
pub(crate) fn preflight_owned_for_operations(
    path_set_source: &str,
    sources: Vec<SemanticWorkspaceSource>,
    graph_builder_limit: usize,
    operations_builder_limit: usize,
) -> Result<SemanticWorkspacePreflight, WorkspaceReportFailure> {
    assert!(
        operations_builder_limit <= 67_108_864,
        "private Semantic Workspace Operations builder limit cannot exceed the production maximum"
    );
    preflight_typed(
        path_set_source,
        sources,
        Some(operations_builder_limit),
        true,
        Some(graph_builder_limit),
        None,
    )
}

pub(crate) fn authenticated_operations_preflight(
    authenticated_revision: &str,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    graph: workspace_graph::WorkspaceGraphBuild,
) -> Result<SemanticWorkspacePreflight, WorkspaceReportFailure> {
    let mut files = sources
        .into_iter()
        .map(|source| SemanticWorkspaceFileFact {
            bytes: source.source.len(),
            path: source.path,
            source_graph_schema: source.source_graph_schema,
            source_revision: source.source_revision,
            source_digest: source.source_digest,
            source: source.source.into(),
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let path_set = files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let manifest = render_manifest(&files)?;
    let workspace_revision = semantic_workspace_revision(&manifest);
    if workspace_revision != authenticated_revision {
        return Err((invariant(
            "Semantic Workspace Operations authenticated manifest replay changed revision",
        ))
        .into());
    }
    let preflight = SemanticWorkspacePreflight {
        path_set,
        files,
        manifest,
        workspace_revision,
        graph,
    };
    replay_typed(&preflight, None)?;
    Ok(preflight)
}
