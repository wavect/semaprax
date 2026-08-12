//! Private structural-change proposal and candidate preparation core.
//!
//! This module has no filesystem, public API, artifact, CLI, or publication
//! authority. It owns canonical structural proposals and derives a complete
//! candidate source set from already-authenticated semantic-workspace facts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::{Map, Value};

use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::{semantic_workspace, semantic_workspace_change, workspace, workspace_graph};

mod artifact;

pub(crate) const SCHEMA: &str = "semaprax.workspace-semantic-structural-change.v1";
const MAX_PROPOSAL_BYTES: usize = 32 * 1024 * 1024;
const MAX_OPERATIONS: usize = 16;
const MAX_PATH_BYTES: usize = 240;
const MAX_SOURCE_BYTES_PER_OPERATION: usize = 1024 * 1024;
const MAX_TOTAL_SUPPLIED_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRY_MODULE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CANDIDATE_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16 * 1024 * 1024;
const MAX_ANALYSIS_BUILDER_BYTES: usize = 32 * 1024 * 1024;
const MIN_MANAGED_FILES: usize = 2;
const MAX_MANAGED_FILES: usize = semantic_workspace::MAX_MANAGED_FILES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceStructuralChangeSet {
    base_workspace_revision: String,
    entry_module: String,
    operations: Vec<SemanticWorkspaceStructuralOperation>,
    proposal_source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SemanticWorkspaceStructuralOperation {
    Create {
        path: String,
        source: String,
    },
    Delete {
        path: String,
        base: BaseSourceBinding,
    },
    Move {
        from_path: String,
        to_path: String,
        base: BaseSourceBinding,
    },
    Replace {
        path: String,
        base: BaseSourceBinding,
        replacement_source: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BaseSourceBinding {
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
}

pub(crate) struct StructuralCandidateOverlay {
    candidate_sources: Vec<semantic_workspace::SemanticWorkspaceSource>,
    base_files: Vec<StructuralBaseFileFact>,
    changed_paths: BTreeSet<String>,
    supplied_source_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralBaseFileFact {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
}

pub(crate) struct SemanticWorkspacePreparedStructuralChange {
    base_workspace_revision: String,
    candidate_workspace_revision: String,
    entry_module: String,
    proposal_source: String,
    operations: Vec<SemanticWorkspaceStructuralOperation>,
    base_workspace_graph_digest: String,
    candidate_workspace_graph_digest: String,
    base_files: Vec<StructuralBaseFileFact>,
    candidate_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    candidate_manifest: String,
    base_graph: workspace_graph::WorkspaceGraphChangeView,
    candidate_graph: workspace_graph::WorkspaceGraphChangeView,
    roots: Vec<semantic_workspace_change::SemanticWorkspaceChangeRoot>,
    delta_edges: Vec<semantic_workspace_change::SemanticWorkspaceChangeEdge>,
    context_nodes: Vec<semantic_workspace_change::SemanticWorkspaceChangeContextNode>,
    impact: Vec<semantic_workspace_change::SemanticWorkspaceChangeImpactFact>,
    impact_edges: Vec<semantic_workspace_change::SemanticWorkspaceChangeImpactEdge>,
    used_analysis_builder_bytes: usize,
    used_total_supplied_source_bytes: usize,
    base_manifest_bytes: usize,
    retained_generations: usize,
    staging_attempts: usize,
}

impl StructuralBaseFileFact {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn source_graph_schema(&self) -> &str {
        &self.source_graph_schema
    }

    pub(crate) fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub(crate) fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }
}

impl SemanticWorkspaceStructuralChangeSet {
    pub(crate) fn source(&self) -> &str {
        &self.proposal_source
    }

    pub(crate) fn base_workspace_revision(&self) -> &str {
        &self.base_workspace_revision
    }

    pub(crate) fn entry_module(&self) -> &str {
        &self.entry_module
    }

    pub(crate) fn operations(&self) -> &[SemanticWorkspaceStructuralOperation] {
        &self.operations
    }
}

impl StructuralCandidateOverlay {
    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<StructuralBaseFileFact>,
        Vec<semantic_workspace::SemanticWorkspaceSource>,
        BTreeSet<String>,
        usize,
    ) {
        (
            self.base_files,
            self.candidate_sources,
            self.changed_paths,
            self.supplied_source_bytes,
        )
    }
}

impl SemanticWorkspacePreparedStructuralChange {
    pub(crate) fn base_workspace_revision(&self) -> &str {
        &self.base_workspace_revision
    }

    pub(crate) fn candidate_workspace_revision(&self) -> &str {
        &self.candidate_workspace_revision
    }

    pub(crate) fn entry_module(&self) -> &str {
        &self.entry_module
    }

    pub(crate) fn proposal_source(&self) -> &str {
        &self.proposal_source
    }

    pub(crate) fn operations(&self) -> &[SemanticWorkspaceStructuralOperation] {
        &self.operations
    }

    pub(crate) fn candidate_manifest(&self) -> &str {
        &self.candidate_manifest
    }

    pub(crate) fn base_workspace_graph_digest(&self) -> &str {
        &self.base_workspace_graph_digest
    }

    pub(crate) fn candidate_workspace_graph_digest(&self) -> &str {
        &self.candidate_workspace_graph_digest
    }

    pub(crate) fn base_files(&self) -> &[StructuralBaseFileFact] {
        &self.base_files
    }

    pub(crate) fn candidate_files(&self) -> &[semantic_workspace::SemanticWorkspaceFileFact] {
        &self.candidate_files
    }

    pub(crate) fn base_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.base_graph
    }

    pub(crate) fn candidate_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.candidate_graph
    }

    pub(crate) fn roots(&self) -> &[semantic_workspace_change::SemanticWorkspaceChangeRoot] {
        &self.roots
    }

    pub(crate) fn delta_edges(&self) -> &[semantic_workspace_change::SemanticWorkspaceChangeEdge] {
        &self.delta_edges
    }

    pub(crate) fn context_nodes(
        &self,
    ) -> &[semantic_workspace_change::SemanticWorkspaceChangeContextNode] {
        &self.context_nodes
    }

    pub(crate) fn impact(&self) -> &[semantic_workspace_change::SemanticWorkspaceChangeImpactFact] {
        &self.impact
    }

    pub(crate) fn impact_edges(
        &self,
    ) -> &[semantic_workspace_change::SemanticWorkspaceChangeImpactEdge] {
        &self.impact_edges
    }

    pub(crate) const fn used_analysis_builder_bytes(&self) -> usize {
        self.used_analysis_builder_bytes
    }

    pub(crate) const fn used_total_supplied_source_bytes(&self) -> usize {
        self.used_total_supplied_source_bytes
    }

    pub(crate) const fn retained_generations(&self) -> usize {
        self.retained_generations
    }

    pub(crate) const fn base_manifest_bytes(&self) -> usize {
        self.base_manifest_bytes
    }

    pub(crate) const fn staging_attempts(&self) -> usize {
        self.staging_attempts
    }

    pub(crate) fn into_candidate_generation_parts(
        self,
    ) -> (
        Vec<semantic_workspace::SemanticWorkspaceFileFact>,
        String,
        String,
    ) {
        (
            self.candidate_files,
            self.candidate_manifest,
            self.candidate_workspace_revision,
        )
    }
}

pub(crate) fn parse_proposal(
    source: &str,
) -> Result<SemanticWorkspaceStructuralChangeSet, Vec<Diagnostic>> {
    if source.len() > MAX_PROPOSAL_BYTES {
        return Err(limit("proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    let body = canonical_body(source)?;
    validate_json_depth(body)?;
    let value: Value = serde_json::from_str(body).map_err(|_| canonical())?;
    let object = exact_object(
        &value,
        &[
            "schema",
            "base_workspace_revision",
            "entry_module",
            "operations",
        ],
    )?;
    if text(object, "schema")? != SCHEMA {
        return Err(canonical());
    }
    let base_workspace_revision = text(object, "base_workspace_revision")?.to_owned();
    validate_digest(&base_workspace_revision)?;
    let entry_module = text(object, "entry_module")?.to_owned();
    validate_entry_module(&entry_module)?;
    let values = array(object, "operations")?;
    if values.is_empty() {
        return Err(canonical());
    }
    if values.len() > MAX_OPERATIONS {
        return Err(limit("operations", MAX_OPERATIONS));
    }

    let mut operations = Vec::with_capacity(values.len());
    let mut total_supplied_source_bytes = 0usize;
    for value in values {
        let object = value.as_object().ok_or_else(canonical)?;
        let kind = text(object, "kind")?;
        let operation = match kind {
            "create" => {
                require_keys(object, &["kind", "path", "source"])?;
                let path = checked_path(text(object, "path")?)?;
                let source = checked_source(text(object, "source")?)?;
                add_supplied(&mut total_supplied_source_bytes, source.len())?;
                SemanticWorkspaceStructuralOperation::Create { path, source }
            }
            "delete" => {
                require_keys(
                    object,
                    &[
                        "kind",
                        "path",
                        "base_source_graph_schema",
                        "base_source_revision",
                        "base_source_digest",
                    ],
                )?;
                SemanticWorkspaceStructuralOperation::Delete {
                    path: checked_path(text(object, "path")?)?,
                    base: parse_base_binding(object)?,
                }
            }
            "move" => {
                require_keys(
                    object,
                    &[
                        "kind",
                        "from_path",
                        "to_path",
                        "base_source_graph_schema",
                        "base_source_revision",
                        "base_source_digest",
                    ],
                )?;
                SemanticWorkspaceStructuralOperation::Move {
                    from_path: checked_path(text(object, "from_path")?)?,
                    to_path: checked_path(text(object, "to_path")?)?,
                    base: parse_base_binding(object)?,
                }
            }
            "replace" => {
                require_keys(
                    object,
                    &[
                        "kind",
                        "path",
                        "base_source_graph_schema",
                        "base_source_revision",
                        "base_source_digest",
                        "replacement_source",
                    ],
                )?;
                let replacement_source = checked_source(text(object, "replacement_source")?)?;
                add_supplied(&mut total_supplied_source_bytes, replacement_source.len())?;
                SemanticWorkspaceStructuralOperation::Replace {
                    path: checked_path(text(object, "path")?)?,
                    base: parse_base_binding(object)?,
                    replacement_source,
                }
            }
            _ => return Err(canonical()),
        };
        operations.push(operation);
    }
    if operations
        .windows(2)
        .any(|pair| operation_key(&pair[0]) >= operation_key(&pair[1]))
    {
        return Err(canonical());
    }
    if !operations.iter().any(|operation| {
        matches!(
            operation,
            SemanticWorkspaceStructuralOperation::Create { .. }
                | SemanticWorkspaceStructuralOperation::Delete { .. }
                | SemanticWorkspaceStructuralOperation::Move { .. }
        )
    }) {
        return Err(conflict(
            "Semantic Workspace Structural Change requires at least one create, delete, or move operation",
        ));
    }

    let mut change_set = SemanticWorkspaceStructuralChangeSet {
        base_workspace_revision,
        entry_module,
        operations,
        proposal_source: String::new(),
    };
    change_set.proposal_source = render_proposal(&change_set)?;
    if change_set.proposal_source != source {
        return Err(canonical());
    }
    Ok(change_set)
}

pub(crate) fn derive_candidate_overlay(
    authenticated_revision: &str,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    change_set: &SemanticWorkspaceStructuralChangeSet,
) -> Result<StructuralCandidateOverlay, Vec<Diagnostic>> {
    if authenticated_revision != change_set.base_workspace_revision {
        return Err(stale(
            "Semantic Workspace Structural Change base workspace revision is stale",
        ));
    }
    if sources.len() > MAX_MANAGED_FILES {
        return Err(limit("base_managed_files", MAX_MANAGED_FILES));
    }
    if sources.len() < MIN_MANAGED_FILES {
        return Err(replay());
    }
    let total_base_source_bytes = sources.iter().try_fold(0usize, |total, source| {
        total.checked_add(source.source.len())
    });
    if total_base_source_bytes.is_none_or(|bytes| bytes > MAX_TOTAL_SOURCE_BYTES) {
        return Err(limit("total_base_source_bytes", MAX_TOTAL_SOURCE_BYTES));
    }

    let mut base = BTreeMap::new();
    for source in &sources {
        if base.insert(source.path.clone(), source).is_some() {
            return Err(replay());
        }
    }
    let mut consumed = BTreeSet::new();
    let mut produced = BTreeSet::new();
    let mut changed_paths = BTreeSet::new();
    let mut supplied_source_bytes = 0usize;

    validate_move_relationships(&change_set.operations)?;

    for operation in &change_set.operations {
        match operation {
            SemanticWorkspaceStructuralOperation::Create { path, source } => {
                require_destination_absent(&base, path)?;
                require_produced(&mut produced, path)?;
                changed_paths.insert(path.clone());
                supplied_source_bytes = checked_supplied(supplied_source_bytes, source.len())?;
            }
            SemanticWorkspaceStructuralOperation::Delete {
                path,
                base: binding,
            } => {
                let source = require_base(&base, path, binding)?;
                require_consumed(&mut consumed, source.path.as_str())?;
                changed_paths.insert(path.clone());
            }
            SemanticWorkspaceStructuralOperation::Move {
                from_path,
                to_path,
                base: binding,
            } => {
                if from_path == to_path {
                    return Err(operation_conflict());
                }
                let source = require_base(&base, from_path, binding)?;
                require_destination_absent(&base, to_path)?;
                require_consumed(&mut consumed, source.path.as_str())?;
                require_produced(&mut produced, to_path)?;
                changed_paths.insert(from_path.clone());
                changed_paths.insert(to_path.clone());
            }
            SemanticWorkspaceStructuralOperation::Replace {
                path,
                base: binding,
                replacement_source,
            } => {
                let source = require_base(&base, path, binding)?;
                require_consumed(&mut consumed, source.path.as_str())?;
                if replacement_source == &source.source {
                    return Err(operation_conflict());
                }
                changed_paths.insert(path.clone());
                supplied_source_bytes =
                    checked_supplied(supplied_source_bytes, replacement_source.len())?;
            }
        }
    }

    let mut created = BTreeMap::new();
    let mut moved = BTreeMap::new();
    let mut replacements = BTreeMap::new();
    for operation in &change_set.operations {
        match operation {
            SemanticWorkspaceStructuralOperation::Create { path, source } => {
                created.insert(path.as_str(), source.as_str());
            }
            SemanticWorkspaceStructuralOperation::Move {
                from_path, to_path, ..
            } => {
                moved.insert(from_path.as_str(), to_path.as_str());
            }
            SemanticWorkspaceStructuralOperation::Replace {
                path,
                replacement_source,
                ..
            } => {
                replacements.insert(path.as_str(), replacement_source.as_str());
            }
            SemanticWorkspaceStructuralOperation::Delete { .. } => {}
        }
    }
    let mut base_files = Vec::with_capacity(sources.len());
    let mut candidate = BTreeMap::new();
    for source in sources {
        base_files.push(StructuralBaseFileFact {
            path: source.path.clone(),
            source_graph_schema: source.source_graph_schema,
            source_revision: source.source_revision,
            source_digest: source.source_digest,
            bytes: source.source.len(),
        });
        if let Some(to_path) = moved.get(source.path.as_str()) {
            insert_candidate(&mut candidate, (*to_path).to_owned(), source.source)?;
        } else if let Some(replacement) = replacements.get(source.path.as_str()) {
            insert_candidate(&mut candidate, source.path, (*replacement).to_owned())?;
        } else if !consumed.contains(source.path.as_str()) {
            insert_candidate(&mut candidate, source.path, source.source)?;
        }
    }
    base_files.sort_by(|left, right| left.path.cmp(&right.path));
    for (path, source) in created {
        insert_candidate(&mut candidate, path.to_owned(), source.to_owned())?;
    }
    if candidate.len() < MIN_MANAGED_FILES || candidate.len() > MAX_MANAGED_FILES {
        return Err(conflict(
            "Semantic Workspace Structural Change candidate path set must contain 2..16 files",
        ));
    }
    let total_candidate_source_bytes = candidate.values().try_fold(0usize, |total, source| {
        total.checked_add(source.source.len())
    });
    if total_candidate_source_bytes.is_none_or(|bytes| bytes > MAX_TOTAL_SOURCE_BYTES) {
        return Err(limit(
            "total_candidate_source_bytes",
            MAX_TOTAL_SOURCE_BYTES,
        ));
    }

    Ok(StructuralCandidateOverlay {
        candidate_sources: candidate.into_values().collect(),
        base_files,
        changed_paths,
        supplied_source_bytes,
    })
}

fn validate_move_relationships(
    operations: &[SemanticWorkspaceStructuralOperation],
) -> Result<(), Vec<Diagnostic>> {
    let mut sources = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    for operation in operations {
        if let SemanticWorkspaceStructuralOperation::Move {
            from_path, to_path, ..
        } = operation
        {
            if from_path == to_path
                || !sources.insert(from_path.as_str())
                || !destinations.insert(to_path.as_str())
            {
                return Err(operation_conflict());
            }
        }
    }
    if destinations.iter().any(|path| sources.contains(path)) {
        return Err(operation_conflict());
    }
    Ok(())
}

pub(crate) fn prepare_owned(
    authenticated_revision: String,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    base_graph: workspace_graph::WorkspaceGraphBuild,
    storage: (usize, usize, usize),
    change_set: SemanticWorkspaceStructuralChangeSet,
) -> Result<SemanticWorkspacePreparedStructuralChange, Vec<Diagnostic>> {
    prepare_owned_with_limit(
        authenticated_revision,
        sources,
        base_graph,
        storage,
        change_set,
        MAX_ANALYSIS_BUILDER_BYTES,
    )
}

#[cfg(test)]
pub(crate) fn prepare_owned_with_analysis_limit(
    authenticated_revision: String,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    base_graph: workspace_graph::WorkspaceGraphBuild,
    storage: (usize, usize, usize),
    change_set: SemanticWorkspaceStructuralChangeSet,
    analysis_builder_limit: usize,
) -> Result<SemanticWorkspacePreparedStructuralChange, Vec<Diagnostic>> {
    prepare_owned_with_limit(
        authenticated_revision,
        sources,
        base_graph,
        storage,
        change_set,
        analysis_builder_limit.min(MAX_ANALYSIS_BUILDER_BYTES),
    )
}

fn prepare_owned_with_limit(
    authenticated_revision: String,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    base_graph: workspace_graph::WorkspaceGraphBuild,
    storage: (usize, usize, usize),
    change_set: SemanticWorkspaceStructuralChangeSet,
    analysis_builder_limit: usize,
) -> Result<SemanticWorkspacePreparedStructuralChange, Vec<Diagnostic>> {
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
    let candidate = semantic_workspace::preflight_owned_for_change(
        &path_set,
        candidate_sources,
        candidate_builder_limit,
    )
    .map_err(|diagnostics| map_candidate_builder_limit(diagnostics, candidate_builder_limit))?;
    if candidate.path_set() != candidate_paths || candidate.files().len() != candidate_paths.len() {
        return Err(replay());
    }

    let candidate_workspace_revision = candidate.workspace_revision().to_owned();
    if candidate_workspace_revision == authenticated_revision {
        return Err(replay());
    }
    let (candidate_files, candidate_manifest, replayed_revision, candidate_graph) =
        candidate.into_snapshot_parts();
    if candidate_manifest.len() > MAX_CANDIDATE_MANIFEST_BYTES {
        return Err(limit(
            "candidate_manifest_bytes",
            MAX_CANDIDATE_MANIFEST_BYTES,
        ));
    }
    if replayed_revision != candidate_workspace_revision {
        return Err(replay());
    }
    let candidate_builder_bytes = candidate_graph.change_builder_bytes().ok_or_else(replay)?;
    if candidate_builder_bytes > MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
        return Err(limit(
            "candidate_graph_builder_bytes",
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
        ));
    }
    let remaining_delta_builder = remaining_after_base
        .checked_sub(candidate_builder_bytes)
        .ok_or_else(|| limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))?;
    let base_graph = base_graph
        .into_change_view()
        .map_err(|diagnostics| map_replay_diagnostics("base-view", diagnostics))?;
    let candidate_graph = candidate_graph
        .into_change_view()
        .map_err(|diagnostics| map_replay_diagnostics("candidate-view", diagnostics))?;
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
        return Err(conflict(
            "Semantic Workspace Structural Change entry module must occur exactly once in the candidate",
        ));
    }

    let prebound = semantic_workspace_change::delta_builder_prebound(&base_graph, &candidate_graph)
        .map_err(|_| limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES))?;
    let (delta, overflowed, delta_builder_bytes) =
        crate::bounded_output::with_limit_usage(remaining_delta_builder, || {
            if !crate::bounded_output::reserve_active(prebound) {
                return Err(limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES));
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
        });
    if overflowed {
        return Err(limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES));
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

pub(crate) fn render_proposal(
    change_set: &SemanticWorkspaceStructuralChangeSet,
) -> Result<String, Vec<Diagnostic>> {
    render_proposal_facts(
        &change_set.base_workspace_revision,
        &change_set.entry_module,
        &change_set.operations,
    )
}

pub(crate) fn render_proposal_facts(
    base_workspace_revision: &str,
    entry_module: &str,
    operations: &[SemanticWorkspaceStructuralOperation],
) -> Result<String, Vec<Diagnostic>> {
    let (output, overflowed) = crate::bounded_output::with_limit(MAX_PROPOSAL_BYTES, || {
        let mut output = CappedString::new();
        output.push_str("{\"schema\":");
        push_json(&mut output, SCHEMA);
        output.push_str(",\"base_workspace_revision\":");
        push_json(&mut output, base_workspace_revision);
        output.push_str(",\"entry_module\":");
        push_json(&mut output, entry_module);
        output.push_str(",\"operations\":[");
        for (index, operation) in operations.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            render_operation(&mut output, operation);
        }
        output.push_str("]}\n");
        output.into_string()
    });
    if overflowed {
        Err(limit("proposal_bytes", MAX_PROPOSAL_BYTES))
    } else {
        Ok(output)
    }
}

fn render_operation(output: &mut CappedString, operation: &SemanticWorkspaceStructuralOperation) {
    match operation {
        SemanticWorkspaceStructuralOperation::Create { path, source } => {
            output.push_str("{\"kind\":\"create\",\"path\":");
            push_json(output, path);
            output.push_str(",\"source\":");
            push_json(output, source);
        }
        SemanticWorkspaceStructuralOperation::Delete { path, base } => {
            output.push_str("{\"kind\":\"delete\",\"path\":");
            push_json(output, path);
            render_base_binding(output, base);
        }
        SemanticWorkspaceStructuralOperation::Move {
            from_path,
            to_path,
            base,
        } => {
            output.push_str("{\"kind\":\"move\",\"from_path\":");
            push_json(output, from_path);
            output.push_str(",\"to_path\":");
            push_json(output, to_path);
            render_base_binding(output, base);
        }
        SemanticWorkspaceStructuralOperation::Replace {
            path,
            base,
            replacement_source,
        } => {
            output.push_str("{\"kind\":\"replace\",\"path\":");
            push_json(output, path);
            render_base_binding(output, base);
            output.push_str(",\"replacement_source\":");
            push_json(output, replacement_source);
        }
    }
    output.push('}');
}

fn render_base_binding(output: &mut CappedString, base: &BaseSourceBinding) {
    output.push_str(",\"base_source_graph_schema\":");
    push_json(output, &base.source_graph_schema);
    output.push_str(",\"base_source_revision\":");
    push_json(output, &base.source_revision);
    output.push_str(",\"base_source_digest\":");
    push_json(output, &base.source_digest);
}

fn push_json(output: &mut CappedString, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value.is_control() => {
                let _ = write!(output, "\\u{:04x}", value as u32);
            }
            value => output.push(value),
        }
    }
    output.push('"');
}

fn parse_base_binding(object: &Map<String, Value>) -> Result<BaseSourceBinding, Vec<Diagnostic>> {
    let source_graph_schema = text(object, "base_source_graph_schema")?.to_owned();
    if !matches!(
        source_graph_schema.as_str(),
        "semaprax.graph.v10"
            | "semaprax.graph.v11"
            | "semaprax.graph.v12"
            | "semaprax.graph.v13"
            | "semaprax.graph.v14"
    ) {
        return Err(canonical());
    }
    let source_revision = text(object, "base_source_revision")?.to_owned();
    let source_digest = text(object, "base_source_digest")?.to_owned();
    validate_digest(&source_revision)?;
    validate_digest(&source_digest)?;
    Ok(BaseSourceBinding {
        source_graph_schema,
        source_revision,
        source_digest,
    })
}

fn checked_path(value: &str) -> Result<String, Vec<Diagnostic>> {
    if value.len() > MAX_PATH_BYTES {
        return Err(limit("path_bytes", MAX_PATH_BYTES));
    }
    if !workspace::evidence_path_is_valid(value) {
        return Err(canonical());
    }
    Ok(value.to_owned())
}

fn checked_source(value: &str) -> Result<String, Vec<Diagnostic>> {
    if value.len() > MAX_SOURCE_BYTES_PER_OPERATION {
        return Err(limit(
            "source_bytes_per_operation",
            MAX_SOURCE_BYTES_PER_OPERATION,
        ));
    }
    Ok(value.to_owned())
}

fn add_supplied(total: &mut usize, bytes: usize) -> Result<(), Vec<Diagnostic>> {
    *total = checked_supplied(*total, bytes)?;
    Ok(())
}

fn checked_supplied(total: usize, bytes: usize) -> Result<usize, Vec<Diagnostic>> {
    let total = total.checked_add(bytes).ok_or_else(|| {
        limit(
            "total_supplied_source_bytes",
            MAX_TOTAL_SUPPLIED_SOURCE_BYTES,
        )
    })?;
    if total > MAX_TOTAL_SUPPLIED_SOURCE_BYTES {
        Err(limit(
            "total_supplied_source_bytes",
            MAX_TOTAL_SUPPLIED_SOURCE_BYTES,
        ))
    } else {
        Ok(total)
    }
}

fn require_base<'a>(
    base: &'a BTreeMap<String, &workspace::WorkspaceSemanticSource>,
    path: &str,
    binding: &BaseSourceBinding,
) -> Result<&'a workspace::WorkspaceSemanticSource, Vec<Diagnostic>> {
    let source = base.get(path).copied().ok_or_else(|| {
        stale("Semantic Workspace Structural Change source path is not managed in the base")
    })?;
    if binding.source_graph_schema != source.source_graph_schema
        || binding.source_revision != source.source_revision
        || binding.source_digest != source.source_digest
    {
        return Err(stale(
            "Semantic Workspace Structural Change base source tuple is stale",
        ));
    }
    Ok(source)
}

fn require_destination_absent(
    base: &BTreeMap<String, &workspace::WorkspaceSemanticSource>,
    path: &str,
) -> Result<(), Vec<Diagnostic>> {
    if base.contains_key(path) {
        Err(stale(
            "Semantic Workspace Structural Change destination path already exists in the base",
        ))
    } else {
        Ok(())
    }
}

fn require_consumed(consumed: &mut BTreeSet<String>, path: &str) -> Result<(), Vec<Diagnostic>> {
    if consumed.insert(path.to_owned()) {
        Ok(())
    } else {
        Err(operation_conflict())
    }
}

fn require_produced(produced: &mut BTreeSet<String>, path: &str) -> Result<(), Vec<Diagnostic>> {
    if produced.insert(path.to_owned()) {
        Ok(())
    } else {
        Err(operation_conflict())
    }
}

fn insert_candidate(
    candidate: &mut BTreeMap<String, semantic_workspace::SemanticWorkspaceSource>,
    path: String,
    source: String,
) -> Result<(), Vec<Diagnostic>> {
    if candidate
        .insert(
            path.clone(),
            semantic_workspace::SemanticWorkspaceSource { path, source },
        )
        .is_some()
    {
        Err(operation_conflict())
    } else {
        Ok(())
    }
}

fn operation_key(operation: &SemanticWorkspaceStructuralOperation) -> (u8, &str, &str) {
    match operation {
        SemanticWorkspaceStructuralOperation::Create { path, .. } => (0, path, ""),
        SemanticWorkspaceStructuralOperation::Delete { path, .. } => (1, path, ""),
        SemanticWorkspaceStructuralOperation::Move {
            from_path, to_path, ..
        } => (2, from_path, to_path),
        SemanticWorkspaceStructuralOperation::Replace { path, .. } => (3, path, ""),
    }
}

fn validate_entry_module(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() > MAX_ENTRY_MODULE_BYTES {
        return Err(limit("entry_module_bytes", MAX_ENTRY_MODULE_BYTES));
    }
    let is_canonical = !value.is_empty()
        && value.split('.').all(|segment| {
            let mut bytes = segment.bytes();
            bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        });
    if is_canonical {
        Ok(())
    } else {
        Err(canonical())
    }
}

fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    let valid = value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    if valid {
        Ok(())
    } else {
        Err(canonical())
    }
}

fn canonical_body(source: &str) -> Result<&str, Vec<Diagnostic>> {
    if source.is_empty()
        || source.starts_with('\u{feff}')
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len().saturating_sub(1)].contains('\n')
    {
        return Err(canonical());
    }
    Ok(&source[..source.len() - 1])
}

fn validate_json_depth(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut depth = 0usize;
    let mut string = false;
    let mut escape = false;
    for byte in source.bytes() {
        if string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'"' => string = true,
            b'{' | b'[' => {
                depth = depth.checked_add(1).ok_or_else(canonical)?;
                if depth > semantic_workspace::MAX_JSON_DEPTH {
                    return Err(canonical());
                }
            }
            b'}' | b']' => depth = depth.checked_sub(1).ok_or_else(canonical)?,
            _ => {}
        }
    }
    if string || depth != 0 {
        Err(canonical())
    } else {
        Ok(())
    }
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, Vec<Diagnostic>> {
    let object = value.as_object().ok_or_else(canonical)?;
    require_keys(object, keys)?;
    Ok(object)
}

fn require_keys(object: &Map<String, Value>, keys: &[&str]) -> Result<(), Vec<Diagnostic>> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        Err(canonical())
    } else {
        Ok(())
    }
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(canonical)
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(canonical)
}

fn canonical() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G188",
        "Semantic Workspace Structural Change proposal is not canonical",
    )]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G189", message)]
}

fn conflict(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G190", message)]
}

fn operation_conflict() -> Vec<Diagnostic> {
    conflict("Semantic Workspace Structural Change operation set conflicts")
}

fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G191",
        crate::bounded_output::budgeted_format(format_args!(
            "Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}"
        )),
    )]
}

fn replay() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G192",
        "Semantic Workspace Structural Change candidate replay disagrees with authenticated facts",
    )]
}

fn map_replay_diagnostics(_label: &'static str, _diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    replay()
}

fn map_candidate_builder_limit(
    diagnostics: Vec<Diagnostic>,
    internal_maximum: usize,
) -> Vec<Diagnostic> {
    let expected = crate::bounded_output::budgeted_format(format_args!(
        "Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}"
    ));
    if diagnostics.len() == 1
        && diagnostics[0].code == "SPX-G171"
        && diagnostics[0].message == expected
    {
        limit(
            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
                "candidate_graph_builder_bytes"
            } else {
                "analysis_builder_bytes"
            },
            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
                MAX_CANDIDATE_GRAPH_BUILDER_BYTES
            } else {
                MAX_ANALYSIS_BUILDER_BYTES
            },
        )
    } else {
        diagnostics
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    pub(super) struct BaseFixture {
        pub(super) revision: String,
        pub(super) manifest_bytes: usize,
        pub(super) sources: Vec<workspace::WorkspaceSemanticSource>,
        pub(super) graph: workspace_graph::WorkspaceGraphBuild,
    }

    fn canonical(source: &str, path: &str) -> String {
        crate::format::canonical(&crate::parse(source, path).unwrap())
    }

    fn provider() -> String {
        canonical(
            r#"
module structural.provider;
permit { audit.old }

@id("structural.point")
record Point { @id("structural.point.value") value: i64, }

@id("structural.work")
fn work(value: Point) -> i64 uses { audit.old } { value.value }

fn helper() -> i64 { 1 }

@id("structural.provider.main")
fn main() -> i64 { helper() }
"#,
            "a/provider.spx",
        )
    }

    fn consumer() -> String {
        canonical(
            r#"
module structural.consumer;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
permit { audit.old, audit.new }

@id("structural.consume")
fn consume() -> i64 uses { audit.old, audit.new } { work(Point { value: 3 }) }

@id("structural.consumer.main")
fn main() -> i64 uses { audit.old, audit.new } { consume() }
"#,
            "m/consumer.spx",
        )
    }

    fn island() -> String {
        canonical(
            r#"
module structural.island;
permit { island.old }

@id("structural.island.value")
fn value() -> i64 { 1 }

@id("structural.island.main")
fn main() -> i64 { value() }
"#,
            "n/island.spx",
        )
    }

    fn entry() -> String {
        canonical(
            r#"
module structural.entry;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
use function @id("structural.consume") from structural.consumer as consume;
permit { audit.old, audit.new }

@id("structural.entry.main")
fn main() -> i64 uses { audit.old, audit.new } { work(Point { value: 1 }) }
"#,
            "z/entry.spx",
        )
    }

    fn entry_replacement() -> String {
        canonical(
            r#"
module structural.entry;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
use function @id("structural.consume") from structural.consumer as consume;
permit { audit.old, audit.new }

@id("structural.entry.main")
fn main() -> i64 uses { audit.old, audit.new } { work(Point { value: 2 }) + consume() }
"#,
            "z/entry.spx",
        )
    }

    fn created() -> String {
        canonical(
            r#"
module structural.created;
permit { created.capability }

fn helper() -> i64 { 7 }

@id("structural.created.main")
fn main() -> i64 uses { created.capability } { helper() }
"#,
            "b/created.spx",
        )
    }

    pub(super) fn base_fixture() -> BaseFixture {
        base_fixture_with_order(false)
    }

    fn base_fixture_with_order(reverse_sources: bool) -> BaseFixture {
        let mut sources = vec![
            semantic_workspace::SemanticWorkspaceSource {
                path: "a/provider.spx".to_owned(),
                source: provider(),
            },
            semantic_workspace::SemanticWorkspaceSource {
                path: "m/consumer.spx".to_owned(),
                source: consumer(),
            },
            semantic_workspace::SemanticWorkspaceSource {
                path: "n/island.spx".to_owned(),
                source: island(),
            },
            semantic_workspace::SemanticWorkspaceSource {
                path: "z/entry.spx".to_owned(),
                source: entry(),
            },
        ];
        let mut paths = sources
            .iter()
            .map(|source| source.path.clone())
            .collect::<Vec<_>>();
        paths.sort();
        if reverse_sources {
            sources.reverse();
        }
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let preflight = semantic_workspace::preflight_owned_for_change(
            &path_set,
            sources,
            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
        )
        .unwrap();
        let (files, manifest, revision, graph) = preflight.into_snapshot_parts();
        let sources = files
            .into_iter()
            .map(|file| {
                let (path, source_graph_schema, source_revision, source_digest, source) =
                    file.into_parts();
                workspace::WorkspaceSemanticSource {
                    path,
                    source_graph_schema,
                    source_revision,
                    source_digest,
                    source,
                }
            })
            .collect();
        BaseFixture {
            revision,
            manifest_bytes: manifest.len(),
            sources,
            graph,
        }
    }

    fn quoted(value: &str) -> String {
        serde_json::to_string(value).unwrap()
    }

    fn binding(source: &workspace::WorkspaceSemanticSource) -> String {
        format!(
            ",\"base_source_graph_schema\":{},\"base_source_revision\":{},\"base_source_digest\":{}",
            quoted(&source.source_graph_schema),
            quoted(&source.source_revision),
            quoted(&source.source_digest)
        )
    }

    fn create_operation(path: &str, source: &str) -> String {
        format!(
            "{{\"kind\":\"create\",\"path\":{},\"source\":{}}}",
            quoted(path),
            quoted(source)
        )
    }

    fn delete_operation(source: &workspace::WorkspaceSemanticSource) -> String {
        format!(
            "{{\"kind\":\"delete\",\"path\":{}{}}}",
            quoted(&source.path),
            binding(source)
        )
    }

    fn move_operation(source: &workspace::WorkspaceSemanticSource, to_path: &str) -> String {
        format!(
            "{{\"kind\":\"move\",\"from_path\":{},\"to_path\":{}{}}}",
            quoted(&source.path),
            quoted(to_path),
            binding(source)
        )
    }

    fn replace_operation(source: &workspace::WorkspaceSemanticSource, replacement: &str) -> String {
        format!(
            "{{\"kind\":\"replace\",\"path\":{}{},\"replacement_source\":{}}}",
            quoted(&source.path),
            binding(source),
            quoted(replacement)
        )
    }

    fn proposal(revision: &str, entry_module: &str, operations: &[String]) -> String {
        format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":{},\"entry_module\":{},\"operations\":[{}]}}\n",
            quoted(revision),
            quoted(entry_module),
            operations.join(",")
        )
    }

    fn source<'a>(base: &'a BaseFixture, path: &str) -> &'a workspace::WorkspaceSemanticSource {
        base.sources
            .iter()
            .find(|source| source.path == path)
            .unwrap()
    }

    pub(super) fn mixed_proposal(base: &BaseFixture) -> String {
        proposal(
            &base.revision,
            "structural.entry",
            &[
                create_operation("b/created.spx", &created()),
                delete_operation(source(base, "n/island.spx")),
                move_operation(source(base, "a/provider.spx"), "c/provider.spx"),
                replace_operation(source(base, "z/entry.spx"), &entry_replacement()),
            ],
        )
    }

    fn error_code<T>(result: Result<T, Vec<Diagnostic>>) -> String {
        let diagnostics = match result {
            Ok(_) => panic!("expected failure"),
            Err(diagnostics) => diagnostics,
        };
        assert_eq!(diagnostics.len(), 1);
        diagnostics[0].code.to_owned()
    }

    #[test]
    fn proposal_kat_has_all_four_operations_and_frozen_order() {
        let base = base_fixture();
        let proposal_source = mixed_proposal(&base);
        let parsed = parse_proposal(&proposal_source).unwrap();
        assert_eq!(parsed.source(), proposal_source);
        assert_eq!(parsed.base_workspace_revision(), base.revision);
        assert_eq!(parsed.entry_module(), "structural.entry");
        assert!(matches!(
            parsed.operations(),
            [
                SemanticWorkspaceStructuralOperation::Create { path, .. },
                SemanticWorkspaceStructuralOperation::Delete { .. },
                SemanticWorkspaceStructuralOperation::Move { .. },
                SemanticWorkspaceStructuralOperation::Replace { .. }
            ] if path == "b/created.spx"
        ));
        let digest = format!("sha256:{:x}", Sha256::digest(proposal_source.as_bytes()));
        assert_eq!(
            digest,
            "sha256:b13dcbf801bdb0fe1cd05a5cff26b58085bc32a576d9a5b8fc7264755c5548f8"
        );

        let mut reordered = parsed.source().to_owned();
        reordered = reordered.replace("{\"kind\":\"create\",\"path\"", "{\"path\"");
        assert_eq!(error_code(parse_proposal(&reordered)), "SPX-G188");
        let reversed = proposal(
            &base.revision,
            "structural.entry",
            &[
                move_operation(source(&base, "a/provider.spx"), "c/provider.spx"),
                create_operation("b/created.spx", &created()),
            ],
        );
        assert_eq!(error_code(parse_proposal(&reversed)), "SPX-G188");
    }

    #[test]
    fn endpoint_conflicts_stale_bindings_and_structural_premise_fail_closed() {
        let base = base_fixture();
        let provider = source(&base, "a/provider.spx");
        let island = source(&base, "n/island.spx");
        let entry_source = source(&base, "z/entry.spx");
        let replacement = entry_replacement();
        let cases = [
            (
                vec![create_operation("a/provider.spx", &created())],
                "SPX-G189",
            ),
            (vec![move_operation(provider, "m/consumer.spx")], "SPX-G189"),
            (
                vec![
                    delete_operation(provider),
                    move_operation(provider, "c/provider.spx"),
                ],
                "SPX-G190",
            ),
            (
                vec![
                    create_operation("c/provider.spx", &created()),
                    move_operation(provider, "c/provider.spx"),
                ],
                "SPX-G190",
            ),
            (vec![move_operation(provider, "a/provider.spx")], "SPX-G190"),
            (
                vec![
                    move_operation(provider, "n/island.spx"),
                    move_operation(island, "a/provider.spx"),
                ],
                "SPX-G190",
            ),
            (
                vec![
                    delete_operation(entry_source),
                    replace_operation(entry_source, &replacement),
                ],
                "SPX-G190",
            ),
            (
                vec![
                    move_operation(provider, "c/provider.spx"),
                    replace_operation(provider, &provider.source),
                ],
                "SPX-G190",
            ),
            (
                vec![
                    move_operation(provider, "n/island.spx"),
                    move_operation(island, "c/island.spx"),
                ],
                "SPX-G190",
            ),
            (
                vec![
                    create_operation("b/created.spx", &created()),
                    replace_operation(entry_source, &entry_source.source),
                ],
                "SPX-G190",
            ),
        ];
        for (operations, expected) in cases {
            let parsed =
                parse_proposal(&proposal(&base.revision, "structural.entry", &operations)).unwrap();
            assert_eq!(
                error_code(derive_candidate_overlay(
                    &base.revision,
                    base_fixture().sources,
                    &parsed,
                )),
                expected
            );
        }

        let replace_only = proposal(
            &base.revision,
            "structural.entry",
            &[replace_operation(
                source(&base, "z/entry.spx"),
                &replacement,
            )],
        );
        assert_eq!(error_code(parse_proposal(&replace_only)), "SPX-G190");

        let duplicate_create = proposal(
            &base.revision,
            "structural.entry",
            &[
                create_operation("b/created.spx", &created()),
                create_operation("b/created.spx", &created()),
            ],
        );
        assert_eq!(error_code(parse_proposal(&duplicate_create)), "SPX-G188");

        let mut stale = move_operation(provider, "c/provider.spx");
        stale = stale.replace(
            &provider.source_digest,
            &format!("sha256:{}", "0".repeat(64)),
        );
        let parsed =
            parse_proposal(&proposal(&base.revision, "structural.entry", &[stale])).unwrap();
        assert_eq!(
            error_code(derive_candidate_overlay(
                &base.revision,
                base_fixture().sources,
                &parsed,
            )),
            "SPX-G189"
        );

        let parsed = parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &[move_operation(provider, "c/provider.spx")],
        ))
        .unwrap();
        assert_eq!(
            error_code(derive_candidate_overlay(
                &format!("sha256:{}", "f".repeat(64)),
                base_fixture().sources,
                &parsed,
            )),
            "SPX-G189"
        );
    }

    #[test]
    fn overlay_preserves_exact_move_bytes_and_enforces_final_cardinality() {
        let base = base_fixture();
        let provider_bytes = source(&base, "a/provider.spx").source.clone();
        let parsed = parse_proposal(&mixed_proposal(&base)).unwrap();
        let overlay = derive_candidate_overlay(&base.revision, base.sources, &parsed).unwrap();
        let (base_files, candidate, changed_paths, supplied_bytes) = overlay.into_parts();
        assert_eq!(base_files.len(), 4);
        assert_eq!(
            candidate
                .iter()
                .map(|source| source.path.as_str())
                .collect::<Vec<_>>(),
            [
                "b/created.spx",
                "c/provider.spx",
                "m/consumer.spx",
                "z/entry.spx"
            ]
        );
        assert_eq!(
            candidate
                .iter()
                .find(|source| source.path == "c/provider.spx")
                .unwrap()
                .source,
            provider_bytes
        );
        assert_eq!(
            changed_paths.into_iter().collect::<Vec<_>>(),
            [
                "a/provider.spx",
                "b/created.spx",
                "c/provider.spx",
                "n/island.spx",
                "z/entry.spx"
            ]
        );
        assert_eq!(supplied_bytes, created().len() + entry_replacement().len());

        let base = base_fixture();
        let exact_operations = (0..12)
            .map(|index| create_operation(&format!("x/{index:02}.spx"), ""))
            .collect::<Vec<_>>();
        let exact = parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &exact_operations,
        ))
        .unwrap();
        assert_eq!(
            derive_candidate_overlay(&base.revision, base.sources, &exact)
                .unwrap()
                .into_parts()
                .1
                .len(),
            16
        );
        let base = base_fixture();
        let over_operations = (0..13)
            .map(|index| create_operation(&format!("x/{index:02}.spx"), ""))
            .collect::<Vec<_>>();
        let over = parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &over_operations,
        ))
        .unwrap();
        assert_eq!(
            error_code(derive_candidate_overlay(
                &base.revision,
                base.sources,
                &over
            )),
            "SPX-G190"
        );

        let base = base_fixture();
        let exact_min = parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &[
                delete_operation(source(&base, "a/provider.spx")),
                delete_operation(source(&base, "n/island.spx")),
            ],
        ))
        .unwrap();
        assert_eq!(
            derive_candidate_overlay(&base.revision, base.sources, &exact_min)
                .unwrap()
                .into_parts()
                .1
                .len(),
            2
        );
        let base = base_fixture();
        let under_min = parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &[
                delete_operation(source(&base, "a/provider.spx")),
                delete_operation(source(&base, "m/consumer.spx")),
                delete_operation(source(&base, "n/island.spx")),
            ],
        ))
        .unwrap();
        assert_eq!(
            error_code(derive_candidate_overlay(
                &base.revision,
                base.sources,
                &under_min,
            )),
            "SPX-G190"
        );
    }

    #[test]
    fn parser_limits_are_exact_and_one_over_is_named() {
        let base = base_fixture();
        let exact_path = format!(
            "{}/{}/{}/{}.spx",
            "a".repeat(59),
            "b".repeat(59),
            "c".repeat(59),
            "d".repeat(56)
        );
        assert_eq!(exact_path.len(), MAX_PATH_BYTES);
        let exact = proposal(
            &base.revision,
            "structural.entry",
            &[create_operation(&exact_path, "")],
        );
        parse_proposal(&exact).unwrap();
        let over_path = format!("{exact_path}a");
        assert_eq!(
            error_code(parse_proposal(&proposal(
                &base.revision,
                "structural.entry",
                &[create_operation(&over_path, "")],
            ))),
            "SPX-G191"
        );

        let exact_source = "x".repeat(MAX_SOURCE_BYTES_PER_OPERATION);
        parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &[create_operation("x/exact.spx", &exact_source)],
        ))
        .unwrap();
        assert_eq!(
            error_code(parse_proposal(&proposal(
                &base.revision,
                "structural.entry",
                &[create_operation("x/over.spx", &format!("{exact_source}x"))],
            ))),
            "SPX-G191"
        );

        let exact_operations = (0..4)
            .map(|index| create_operation(&format!("x/{index}.spx"), &exact_source))
            .collect::<Vec<_>>();
        parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &exact_operations,
        ))
        .unwrap();
        let mut over_operations = exact_operations;
        over_operations.push(create_operation("x/4.spx", "x"));
        assert_eq!(
            error_code(parse_proposal(&proposal(
                &base.revision,
                "structural.entry",
                &over_operations,
            ))),
            "SPX-G191"
        );

        let exact_entry = "a".repeat(MAX_ENTRY_MODULE_BYTES);
        parse_proposal(&proposal(
            &base.revision,
            &exact_entry,
            &[create_operation("x/entry-exact.spx", "")],
        ))
        .unwrap();
        assert_eq!(
            error_code(parse_proposal(&proposal(
                &base.revision,
                &format!("{exact_entry}a"),
                &[create_operation("x/entry-over.spx", "")],
            ))),
            "SPX-G191"
        );

        let operations = (0..=MAX_OPERATIONS)
            .map(|index| create_operation(&format!("x/{index:02}.spx"), ""))
            .collect::<Vec<_>>();
        assert_eq!(
            error_code(parse_proposal(&proposal(
                &base.revision,
                "structural.entry",
                &operations,
            ))),
            "SPX-G191"
        );
    }

    #[test]
    fn mixed_full_graph_candidate_is_deterministic_and_entry_removal_fails() {
        let base = base_fixture();
        let parsed = parse_proposal(&mixed_proposal(&base)).unwrap();
        let expected_operations = parsed.operations().to_vec();
        let prepared = prepare_owned(
            base.revision,
            base.sources,
            base.graph,
            (base.manifest_bytes, 1, 0),
            parsed,
        )
        .unwrap();
        assert_ne!(
            prepared.base_workspace_revision(),
            prepared.candidate_workspace_revision()
        );
        assert_eq!(prepared.entry_module(), "structural.entry");
        assert_eq!(prepared.operations(), expected_operations);
        assert_eq!(
            prepared.used_total_supplied_source_bytes(),
            created().len() + entry_replacement().len()
        );
        assert!(!prepared.roots().is_empty());
        assert!(!prepared.delta_edges().is_empty());
        assert!(!prepared.context_nodes().is_empty());
        assert!(!prepared.impact().is_empty());
        assert!(!prepared.impact_edges().is_empty());
        assert!(prepared.used_analysis_builder_bytes() > 0);
        assert_eq!(
            prepared
                .delta_edges()
                .iter()
                .map(|edge| edge.edge().kind())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "call",
                "capability_authority",
                "effect_requirement",
                "function_import",
                "type_import",
                "type_reference",
            ])
        );
        assert!(prepared.roots().iter().any(|root| {
            root.id() == "structural.point"
                && root.identity_origin() == Some("explicit")
                && root.path() == Some("a/provider.spx")
                && root.change() == "modified_before"
        }));
        assert!(prepared.roots().iter().any(|root| {
            root.id() == "structural.point"
                && root.identity_origin() == Some("explicit")
                && root.path() == Some("c/provider.spx")
                && root.change() == "modified_after"
        }));
        assert!(prepared.roots().iter().any(|root| {
            root.identity_origin() == Some("automatic")
                && root.path() == Some("a/provider.spx")
                && root.change() == "removed"
        }));
        assert!(prepared.roots().iter().any(|root| {
            root.identity_origin() == Some("automatic")
                && root.path() == Some("c/provider.spx")
                && root.change() == "added"
        }));
        assert!(prepared
            .roots()
            .iter()
            .any(|root| root.path() == Some("n/island.spx") && root.state() == "base"));
        assert!(prepared
            .roots()
            .iter()
            .any(|root| root.path() == Some("b/created.spx") && root.state() == "candidate"));
        assert!(prepared.roots().iter().any(|root| {
            root.state() == "base"
                && root.kind() == "module"
                && root.id() == "structural.consumer"
                && root.path() == Some("m/consumer.spx")
                && root.change() == "modified_before"
        }));
        assert!(prepared.roots().iter().any(|root| {
            root.state() == "candidate"
                && root.kind() == "module"
                && root.id() == "structural.consumer"
                && root.path() == Some("m/consumer.spx")
                && root.change() == "modified_after"
        }));

        let replay = base_fixture_with_order(true);
        let parsed = parse_proposal(&mixed_proposal(&replay)).unwrap();
        let replayed = prepare_owned(
            replay.revision,
            replay.sources,
            replay.graph,
            (replay.manifest_bytes, 1, 0),
            parsed,
        )
        .unwrap();
        assert_eq!(prepared.proposal_source(), replayed.proposal_source());
        assert_eq!(
            prepared.candidate_workspace_revision(),
            replayed.candidate_workspace_revision()
        );
        assert_eq!(prepared.candidate_manifest(), replayed.candidate_manifest());
        assert_eq!(
            prepared.candidate_workspace_graph_digest(),
            replayed.candidate_workspace_graph_digest()
        );
        assert_eq!(prepared.roots(), replayed.roots());
        assert_eq!(prepared.delta_edges(), replayed.delta_edges());

        let base = base_fixture();
        let delete_entry = parse_proposal(&proposal(
            &base.revision,
            "structural.entry",
            &[delete_operation(source(&base, "z/entry.spx"))],
        ))
        .unwrap();
        assert_eq!(
            error_code(prepare_owned(
                base.revision,
                base.sources,
                base.graph,
                (base.manifest_bytes, 1, 0),
                delete_entry,
            )),
            "SPX-G190"
        );
    }

    fn build_with_analysis_limit(limit: usize) -> Result<usize, Vec<Diagnostic>> {
        let base = base_fixture();
        let parsed = parse_proposal(&mixed_proposal(&base)).unwrap();
        prepare_owned_with_analysis_limit(
            base.revision,
            base.sources,
            base.graph,
            (base.manifest_bytes, 1, 0),
            parsed,
            limit,
        )
        .map(|prepared| prepared.used_analysis_builder_bytes())
    }

    #[test]
    fn each_structural_operation_builds_a_complete_candidate_independently() {
        for case in [
            "create",
            "delete",
            "move",
            "create-replace",
            "structural-three",
        ] {
            let base = base_fixture();
            let operations = match case {
                "create" => vec![create_operation("b/created.spx", &created())],
                "delete" => vec![delete_operation(source(&base, "n/island.spx"))],
                "move" => vec![move_operation(
                    source(&base, "a/provider.spx"),
                    "c/provider.spx",
                )],
                "create-replace" => vec![
                    create_operation("b/created.spx", &created()),
                    replace_operation(source(&base, "z/entry.spx"), &entry_replacement()),
                ],
                "structural-three" => vec![
                    create_operation("b/created.spx", &created()),
                    delete_operation(source(&base, "n/island.spx")),
                    move_operation(source(&base, "a/provider.spx"), "c/provider.spx"),
                ],
                _ => unreachable!(),
            };
            let parsed =
                parse_proposal(&proposal(&base.revision, "structural.entry", &operations)).unwrap();
            if let Err(diagnostics) = prepare_owned(
                base.revision,
                base.sources,
                base.graph,
                (base.manifest_bytes, 1, 0),
                parsed,
            ) {
                panic!("{case} failed: {diagnostics:?}");
            }
        }
    }

    #[test]
    fn analysis_builder_limit_has_an_exact_minimum_successful_boundary() {
        let mut low = 0usize;
        let mut high = MAX_ANALYSIS_BUILDER_BYTES;
        while low < high {
            let middle = low + (high - low) / 2;
            if build_with_analysis_limit(middle).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert!(low > 0);
        assert_eq!(build_with_analysis_limit(low).unwrap(), low);
        let diagnostics = build_with_analysis_limit(low - 1).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G191");
        assert_eq!(
            diagnostics[0].message,
            format!(
                "Semantic Workspace Structural Change limit exceeded: analysis_builder_bytes maximum {MAX_ANALYSIS_BUILDER_BYTES}"
            )
        );
    }
}
