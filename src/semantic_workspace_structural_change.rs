//! Authenticated semantic-workspace structural-change evidence and application.
//!
//! The read-only public routes own one canonical proposal while holding the
//! shared semantic-workspace lock, derive one complete candidate generation,
//! and return bounded canonical artifacts or an exact-replay receipt. The apply
//! route holds the exclusive lock and publishes a new immutable generation only
//! after exact Evidence replay. No route provides a reusable token, signature,
//! approval, or rollback authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use serde_json::{Map, Value};

use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::{semantic_workspace, semantic_workspace_change, workspace, workspace_graph};

mod artifact;
mod verification;

pub use artifact::SemanticWorkspaceStructuralChangeArtifacts;

/// Generates the complete authenticated read-only structural-change bundle.
pub fn generate(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceStructuralChangeArtifacts, Vec<Diagnostic>> {
    generate_with_hook(root, proposal_path, |_| {})
}

/// Generates the canonical Structural Change Preview, including its terminal LF.
pub fn preview(root: &Path, proposal_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate(root, proposal_path).map(SemanticWorkspaceStructuralChangeArtifacts::into_preview)
}

/// Generates the canonical Structural Change Evidence, including its terminal LF.
pub fn evidence(root: &Path, proposal_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate(root, proposal_path).map(SemanticWorkspaceStructuralChangeArtifacts::into_evidence)
}

/// Verifies submitted Structural Change Evidence by exact authenticated replay.
pub fn verify(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    verify_with_hook(root, proposal_path, evidence_path, |_| {})
}

/// Applies an authenticated structural change after exact Evidence replay.
///
/// The returned canonical application receipt includes its terminal LF.
pub fn apply(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    apply_authenticated_with_hook(root, proposal_path, evidence_path, |_, _, _, _| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralGeneratePoint {
    ProposalOwned,
    ArtifactsRendered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralVerifyPoint {
    ProposalOwned,
    EvidenceOwned,
    ReceiptRendered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuralApplyPoint {
    ProposalOwned,
    EvidenceOwned,
    AfterReplay,
    ReceiptRendered,
    Workspace(workspace::SemanticChangeApplyPoint),
}

pub(crate) struct SemanticWorkspaceStructuralChangeCommitAuthority {
    authority: workspace::WorkspaceSemanticReadAuthority,
    candidate_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    candidate_manifest: String,
    candidate_revision: String,
    receipt: String,
}

impl SemanticWorkspaceStructuralChangeCommitAuthority {
    pub(crate) fn into_parts(
        self,
    ) -> (
        workspace::WorkspaceSemanticReadAuthority,
        Vec<semantic_workspace::SemanticWorkspaceFileFact>,
        String,
        String,
        String,
    ) {
        (
            self.authority,
            self.candidate_files,
            self.candidate_manifest,
            self.candidate_revision,
            self.receipt,
        )
    }
}

pub(crate) fn generate_with_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(StructuralGeneratePoint),
) -> Result<artifact::SemanticWorkspaceStructuralChangeArtifacts, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_proposal(proposal_path).and_then(|source| {
        let change_set = parse_proposal(&source)?;
        hook(StructuralGeneratePoint::ProposalOwned);
        Ok(change_set)
    });
    let (authority, change_set) = locked
        .authenticate(proposal)
        .map_err(map_base_builder_limit)?;
    with_authenticated_structural_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        hook(StructuralGeneratePoint::ArtifactsRendered);
        Ok(artifacts)
    })
}

pub(crate) fn verify_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(StructuralVerifyPoint),
) -> Result<String, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(StructuralVerifyPoint::ProposalOwned);
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(StructuralVerifyPoint::EvidenceOwned);
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) =
        locked.authenticate(input).map_err(map_base_builder_limit)?;
    with_authenticated_structural_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        let receipt =
            artifact::render_verification_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(StructuralVerifyPoint::ReceiptRendered);
        Ok(receipt)
    })
}

pub(crate) fn apply_authenticated_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(
        StructuralApplyPoint,
        &Path,
        Option<&Path>,
        Option<&Path>,
    ) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_apply_lock(root)?;
    let active_path = root.join(".semaprax-workspace/ACTIVE");
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(
            StructuralApplyPoint::ProposalOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| apply_hook_error("proposal post-read hook failed", error))?;
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(
            StructuralApplyPoint::EvidenceOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| apply_hook_error("Evidence post-read hook failed", error))?;
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) =
        locked.authenticate(input).map_err(map_base_builder_limit)?;
    let (authority, prepared) = prepare_authenticated_structural_authority(authority, change_set)?;
    let prepublication = (|| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        hook(StructuralApplyPoint::AfterReplay, &active_path, None, None)
            .map_err(|error| apply_hook_error("exact replay hook failed", error))?;
        let receipt =
            artifact::render_application_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(
            StructuralApplyPoint::ReceiptRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| apply_hook_error("application receipt hook failed", error))?;
        Ok(receipt)
    })();
    let receipt = match prepublication {
        Ok(receipt) => receipt,
        Err(diagnostics) => return authority.finish(Err(diagnostics)),
    };
    let (candidate_files, candidate_manifest, candidate_revision) =
        prepared.into_candidate_generation_parts();
    let commit = SemanticWorkspaceStructuralChangeCommitAuthority {
        authority,
        candidate_files,
        candidate_manifest,
        candidate_revision,
        receipt,
    };
    workspace::commit_semantic_structural_change_authority_with_hook(
        commit,
        |point, active, staged, candidate| {
            hook(
                StructuralApplyPoint::Workspace(point),
                active,
                staged,
                candidate,
            )
        },
    )
}

fn apply_hook_error(label: &'static str, error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I211", format!("{label}: {error}"))]
}

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

#[cfg(test)]
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

    fn into_candidate_generation_parts(
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

fn with_authenticated_structural_authority<T>(
    authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceStructuralChangeSet,
    operation: impl FnOnce(SemanticWorkspacePreparedStructuralChange) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let (authority, prepared) = prepare_authenticated_structural_authority(authority, change_set)?;
    let result = operation(prepared);
    authority.finish(result)
}

fn prepare_authenticated_structural_authority(
    mut authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceStructuralChangeSet,
) -> Result<
    (
        workspace::WorkspaceSemanticReadAuthority,
        SemanticWorkspacePreparedStructuralChange,
    ),
    Vec<Diagnostic>,
> {
    let base_workspace_revision = authority.workspace_revision().to_owned();
    let storage = (
        authority.manifest_bytes(),
        authority.retained_generations(),
        authority.staging_attempts(),
    );
    let result = (|| {
        let base_graph = authority.take_graph()?;
        let sources = authority.take_sources();
        prepare_owned(
            base_workspace_revision,
            sources,
            base_graph,
            storage,
            change_set,
        )
    })();
    match result {
        Ok(prepared) => Ok((authority, prepared)),
        Err(diagnostics) => match authority.finish::<()>(Err(diagnostics)) {
            Err(diagnostics) => Err(diagnostics),
            Ok(()) => Err(replay()),
        },
    }
}

fn read_proposal(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut file = File::open(path).map_err(|_| proposal_io("open failed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| proposal_io("metadata inspection failed"))?;
    if !metadata.is_file() {
        return Err(proposal_io("input is not a regular file"));
    }
    if metadata.len() > MAX_PROPOSAL_BYTES as u64 {
        return Err(limit("proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    let mut bytes = Vec::with_capacity((metadata.len() as usize).saturating_add(1));
    file.by_ref()
        .take((MAX_PROPOSAL_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| proposal_io("read failed"))?;
    if bytes.len() > MAX_PROPOSAL_BYTES {
        return Err(limit("proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    String::from_utf8(bytes).map_err(|_| proposal_io("input is not UTF-8"))
}

fn proposal_io(detail: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-I215",
        format!("could not read Semantic Workspace Structural Change proposal: {detail}"),
    )]
}

fn map_base_builder_limit(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let expected = crate::bounded_output::budgeted_format(format_args!(
        "Workspace Semantic Graph `change_builder_bytes` exceeds {}",
        semantic_workspace::MAX_CHANGE_BUILDER_BYTES
    ));
    if diagnostics.len() == 1
        && diagnostics[0].code == "SPX-G171"
        && diagnostics[0].message == expected
    {
        limit("analysis_builder_bytes", MAX_ANALYSIS_BUILDER_BYTES)
    } else {
        diagnostics
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
        crate::graph::reject_while_loop_evidence_schema(&source.source_graph_schema)
            .map_err(|error| vec![error])?;
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
    use std::fs::{File, OpenOptions};
    use std::io::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use super::*;
    use fs2::FileExt;
    use sha2::{Digest, Sha256};

    static MANAGED_SERIAL: AtomicU64 = AtomicU64::new(0);
    const TEST_MAX_EVIDENCE_BYTES: usize = 1_048_576;

    pub(super) struct BaseFixture {
        pub(super) revision: String,
        pub(super) manifest_bytes: usize,
        pub(super) sources: Vec<workspace::WorkspaceSemanticSource>,
        pub(super) graph: workspace_graph::WorkspaceGraphBuild,
    }

    struct ManagedFixture {
        root: PathBuf,
        proposal_path: PathBuf,
        proposal_source: String,
    }

    impl ManagedFixture {
        fn new(label: &str) -> Self {
            let base = base_fixture();
            let proposal_source = mixed_proposal(&base);
            let serial = MANAGED_SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-semantic-workspace-structural-change-{label}-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&root).unwrap();
            let mut paths = Vec::new();
            for source in &base.sources {
                let destination = root.join(&source.path);
                std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
                std::fs::write(&destination, &source.source).unwrap();
                paths.push(source.path.clone());
            }
            paths.sort();
            let path_set = root.join("paths.json");
            std::fs::write(
                &path_set,
                semantic_workspace::render_path_set(&paths).unwrap(),
            )
            .unwrap();
            assert_eq!(
                semantic_workspace::initialize(&root, &path_set).unwrap(),
                base.revision
            );
            let proposal_path = root.join("structural-change.json");
            std::fs::write(&proposal_path, &proposal_source).unwrap();
            Self {
                root,
                proposal_path,
                proposal_source,
            }
        }

        fn inventory(&self) -> Vec<(String, bool, Vec<u8>)> {
            fn walk(root: &Path, path: &Path, facts: &mut Vec<(String, bool, Vec<u8>)>) {
                let mut entries = std::fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap())
                    .collect::<Vec<_>>();
                entries.sort_by_key(std::fs::DirEntry::file_name);
                for entry in entries {
                    let path = entry.path();
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned();
                    let metadata = std::fs::symlink_metadata(&path).unwrap();
                    if metadata.is_dir() {
                        facts.push((relative, true, Vec::new()));
                        walk(root, &path, facts);
                    } else {
                        facts.push((relative, false, std::fs::read(&path).unwrap()));
                    }
                }
            }

            let mut facts = Vec::new();
            walk(&self.root, &self.root, &mut facts);
            facts
        }

        fn assert_exclusive_reacquire(&self) {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(self.root.join(".semaprax-workspace/LOCK"))
                .unwrap();
            FileExt::try_lock_exclusive(&lock).unwrap();
            FileExt::unlock(&lock).unwrap();
        }

        fn raw_inventory(&self) -> Vec<(String, bool, Vec<u8>)> {
            self.inventory()
                .into_iter()
                .filter(|(path, _, _)| !path.starts_with(".semaprax-workspace"))
                .collect()
        }

        fn authenticated_paths_and_storage(&self) -> (Vec<String>, usize, usize) {
            let mut authority = workspace::acquire_semantic_change_read(&self.root).unwrap();
            let retained = authority.retained_generations();
            let staging = authority.staging_attempts();
            let mut paths = authority
                .take_sources()
                .into_iter()
                .map(|source| source.path)
                .collect::<Vec<_>>();
            paths.sort();
            let _graph = authority.take_graph().unwrap();
            authority.finish(Ok(())).unwrap();
            (paths, retained, staging)
        }
    }

    impl Drop for ManagedFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn raw_sha(source: &str) -> String {
        format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
        )
    }

    fn diagnostic(result: Result<impl Sized, Vec<Diagnostic>>) -> Diagnostic {
        let diagnostics = match result {
            Ok(_) => panic!("expected failure"),
            Err(diagnostics) => diagnostics,
        };
        assert_eq!(diagnostics.len(), 1);
        diagnostics.into_iter().next().unwrap()
    }

    fn read_only_failure<T>(
        fixture: &ManagedFixture,
        operation: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
    ) -> Diagnostic {
        let before = fixture.inventory();
        let error = diagnostic(operation());
        assert_eq!(fixture.inventory(), before);
        fixture.assert_exclusive_reacquire();
        error
    }

    fn application_fixture(label: &str) -> (ManagedFixture, PathBuf) {
        let fixture = ManagedFixture::new(label);
        let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
        let evidence_path = fixture.root.join("evidence.json");
        std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
        (fixture, evidence_path)
    }

    fn spawn_structural_apply_process(
        fixture: &ManagedFixture,
        evidence_path: &Path,
        boundary: &str,
    ) -> (Child, PathBuf) {
        let ready = fixture
            .root
            .join(format!("structural-apply-{boundary}.ready"));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "semantic_workspace_structural_change::tests::structural_apply_process_child",
                "--nocapture",
            ])
            .env("SEMAPRAX_STRUCTURAL_APPLY_CHILD", "1")
            .env("SEMAPRAX_STRUCTURAL_APPLY_ROOT", &fixture.root)
            .env("SEMAPRAX_STRUCTURAL_APPLY_PROPOSAL", &fixture.proposal_path)
            .env("SEMAPRAX_STRUCTURAL_APPLY_EVIDENCE", evidence_path)
            .env("SEMAPRAX_STRUCTURAL_APPLY_BOUNDARY", boundary)
            .env("SEMAPRAX_STRUCTURAL_APPLY_READY", &ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !matches!(std::fs::read(&ready), Ok(bytes) if bytes == b"ready\n") {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("structural apply child exited before {boundary}: {status}");
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("structural apply child did not reach {boundary}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        (child, ready)
    }

    fn directory_names(path: &Path) -> Vec<String> {
        let mut names = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn apply_point_name(point: StructuralApplyPoint) -> &'static str {
        match point {
            StructuralApplyPoint::ProposalOwned => "proposal_owned",
            StructuralApplyPoint::EvidenceOwned => "evidence_owned",
            StructuralApplyPoint::AfterReplay => "after_replay",
            StructuralApplyPoint::ReceiptRendered => "receipt_rendered",
            StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterSlotCreate,
            )) => "generation_after_slot_create",
            StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterManifestWrite,
            )) => "generation_after_manifest_write",
            StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterFilesWrite,
            )) => "generation_after_files_write",
            StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::BeforeStageValidation,
            )) => "generation_before_stage_validation",
            StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::BeforeGenerationPublish,
            )) => "generation_before_publish",
            StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::DestinationChecked,
            )) => "generation_destination_checked",
            StructuralApplyPoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterGenerationPublish,
            )) => "generation_after_publish",
            StructuralApplyPoint::Workspace(
                workspace::SemanticChangeApplyPoint::AfterCandidatePrepared,
            ) => "after_candidate_prepared",
            StructuralApplyPoint::Workspace(
                workspace::SemanticChangeApplyPoint::AfterActiveStaged,
            ) => "after_active_staged",
            StructuralApplyPoint::Workspace(
                workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck,
            ) => "before_first_final_check",
            StructuralApplyPoint::Workspace(
                workspace::SemanticChangeApplyPoint::BeforeSecondFinalCheck,
            ) => "before_second_final_check",
            StructuralApplyPoint::Workspace(
                workspace::SemanticChangeApplyPoint::BeforeActiveReplace,
            ) => "before_active_replace",
            StructuralApplyPoint::Workspace(
                workspace::SemanticChangeApplyPoint::AfterActiveReplace,
            ) => "after_active_replace",
        }
    }

    fn replace_owned_path(path: &Path, replacement: &Path) {
        std::fs::remove_file(path).unwrap();
        std::fs::rename(replacement, path).unwrap();
    }

    fn object_after<'a>(source: &'a str, marker: &str) -> &'a str {
        let marker = source.find(marker).unwrap() + marker.len();
        let start = marker + source[marker..].find('{').unwrap();
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (offset, byte) in source[start..].bytes().enumerate() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[start..=start + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("object after marker is unterminated")
    }

    fn replace_scalar_field(object: &str, field: &str, replacement: &str) -> String {
        let needle = format!("\"{field}\":");
        let start = object.find(&needle).unwrap() + needle.len();
        let bytes = object.as_bytes();
        let end = if bytes[start] == b'"' {
            let mut index = start + 1;
            let mut escaped = false;
            loop {
                if escaped {
                    escaped = false;
                } else if bytes[index] == b'\\' {
                    escaped = true;
                } else if bytes[index] == b'"' {
                    break index + 1;
                }
                index += 1;
            }
        } else {
            let mut index = start;
            while !matches!(bytes[index], b',' | b'}') {
                index += 1;
            }
            index
        };
        format!("{}{}{}", &object[..start], replacement, &object[end..])
    }

    fn remove_nonfirst_scalar_field(object: &str, field: &str) -> String {
        let needle = format!(",\"{field}\":");
        let start = object.find(&needle).unwrap();
        let value_start = start + needle.len();
        let bytes = object.as_bytes();
        let end = if bytes[value_start] == b'"' {
            let mut index = value_start + 1;
            let mut escaped = false;
            loop {
                if escaped {
                    escaped = false;
                } else if bytes[index] == b'\\' {
                    escaped = true;
                } else if bytes[index] == b'"' {
                    break index + 1;
                }
                index += 1;
            }
        } else {
            let mut index = value_start;
            while !matches!(bytes[index], b',' | b'}') {
                index += 1;
            }
            index
        };
        format!("{}{}", &object[..start], &object[end..])
    }

    fn duplicate_first_field(object: &str) -> String {
        let comma = object.find(',').unwrap();
        format!("{{{},{}", &object[1..comma], &object[1..])
    }

    fn reorder_first_two_fields(object: &str) -> String {
        let first_comma = object.find(',').unwrap();
        let second_end = object[first_comma + 1..]
            .find(',')
            .map_or(object.len() - 1, |offset| first_comma + 1 + offset);
        format!(
            "{{{},{}{}",
            &object[first_comma + 1..second_end],
            &object[1..first_comma],
            &object[second_end..]
        )
    }

    fn nested_shape_mutations(
        source: &str,
        marker: &str,
        first_field: &str,
        second_field: &str,
    ) -> Vec<String> {
        let object = object_after(source, marker);
        [
            remove_nonfirst_scalar_field(object, second_field),
            object.replacen('{', "{\"extra\":0,", 1),
            duplicate_first_field(object),
            reorder_first_two_fields(object),
            replace_scalar_field(object, first_field, "[]"),
        ]
        .into_iter()
        .map(|mutation| source.replacen(object, &mutation, 1))
        .collect()
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
        let digest = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(proposal_source.as_bytes()))
        );
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

    #[test]
    fn managed_generate_and_verify_are_exact_read_only_kats_under_one_shared_lock() {
        let fixture = ManagedFixture::new("generate-verify-kat");
        let before_generate = fixture.inventory();
        let generate_points = std::cell::RefCell::new(Vec::new());
        let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |point| {
            generate_points.borrow_mut().push(point);
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(fixture.root.join(".semaprax-workspace/LOCK"))
                .unwrap();
            assert!(FileExt::try_lock_exclusive(&lock).is_err());
        })
        .unwrap();
        assert_eq!(
            *generate_points.borrow(),
            [
                StructuralGeneratePoint::ProposalOwned,
                StructuralGeneratePoint::ArtifactsRendered,
            ]
        );
        assert_eq!(fixture.inventory(), before_generate);
        assert_eq!(
            [
                raw_sha(artifacts.preview()),
                raw_sha(artifacts.context()),
                raw_sha(artifacts.impact()),
                raw_sha(artifacts.review()),
                raw_sha(artifacts.evidence()),
            ],
            [
                "sha256:abd5d9cad5472e695e9f580e8fdaa7468b160268c21e98432805248618133d8b",
                "sha256:213ae99e3169084640627930c5f5cd8fe61042de57c0b786814a53dfb44a835c",
                "sha256:ccff6f8136442c1ce1d925b683dcc8d5db0bfec7169f635db9f767e1732df255",
                "sha256:156a61f236c21041918c6d545a9a2422ab1fdd59eefb2c98bb138f6b9916e44c",
                "sha256:5ebd115a8a6de760600f2cdd2e644d9574947f0db0b92779b9ce7f6ea99f51e8",
            ]
        );

        let evidence_path = fixture.root.join("evidence.json");
        std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
        let before_verify = fixture.inventory();
        let verify_points = std::cell::RefCell::new(Vec::new());
        let receipt = verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point| {
                verify_points.borrow_mut().push(point);
                let lock = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(fixture.root.join(".semaprax-workspace/LOCK"))
                    .unwrap();
                assert!(FileExt::try_lock_exclusive(&lock).is_err());
            },
        )
        .unwrap();
        assert_eq!(
            *verify_points.borrow(),
            [
                StructuralVerifyPoint::ProposalOwned,
                StructuralVerifyPoint::EvidenceOwned,
                StructuralVerifyPoint::ReceiptRendered,
            ]
        );
        assert_eq!(fixture.inventory(), before_verify);
        assert!(receipt.ends_with('\n'));
        assert!(!receipt[..receipt.len() - 1].contains('\n'));
        let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        assert_eq!(
            value["schema"],
            "semaprax.workspace-semantic-structural-change-evidence-verification.v1"
        );
        assert_eq!(value["result"], "exact_replay");
        assert_eq!(
            value["workspace_structural_change_evidence"]["bytes"],
            artifacts.evidence().len()
        );
        assert_eq!(value["budget"]["used_receipt_bytes"], receipt.len());
        assert_eq!(
            raw_sha(&receipt),
            "sha256:37ccda3e8c1ceeda269a357feab264ae06f5fab0dd1111b5847d1a6b0628f306"
        );
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn input_ownership_precedence_and_exact_read_limits_are_fail_closed() {
        let fixture = ManagedFixture::new("input-precedence");
        let missing_proposal = fixture.root.join("missing-proposal.json");
        let missing_evidence = fixture.root.join("missing-evidence.json");
        let error = read_only_failure(&fixture, || {
            verify_with_hook(&fixture.root, &missing_proposal, &missing_evidence, |_| {})
        });
        assert_eq!(error.code, "SPX-I215");
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change proposal: open failed"
        );

        let malformed_proposal = fixture.root.join("malformed-proposal.json");
        std::fs::write(&malformed_proposal, "{}\n").unwrap();
        let error = read_only_failure(&fixture, || {
            verify_with_hook(
                &fixture.root,
                &malformed_proposal,
                &missing_evidence,
                |_| {},
            )
        });
        assert_eq!(error.code, "SPX-I215");
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change Evidence: open failed"
        );
        let malformed_evidence = fixture.root.join("malformed-evidence.json");
        std::fs::write(&malformed_evidence, "{}\n").unwrap();
        assert_eq!(
            read_only_failure(&fixture, || verify_with_hook(
                &fixture.root,
                &malformed_proposal,
                &malformed_evidence,
                |_| {},
            ))
            .code,
            "SPX-G193"
        );

        let proposal_dir = fixture.root.join("proposal-dir");
        std::fs::create_dir(&proposal_dir).unwrap();
        let error = read_only_failure(&fixture, || {
            generate_with_hook(&fixture.root, &proposal_dir, |_| {})
        });
        assert_eq!(error.code, "SPX-I215");
        #[cfg(windows)]
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change proposal: open failed"
        );
        #[cfg(not(windows))]
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change proposal: input is not a regular file"
        );

        let invalid_proposal = fixture.root.join("invalid-proposal.json");
        std::fs::write(&invalid_proposal, [0xff]).unwrap();
        let error = read_only_failure(&fixture, || {
            generate_with_hook(&fixture.root, &invalid_proposal, |_| {})
        });
        assert_eq!(error.code, "SPX-I215");
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change proposal: input is not UTF-8"
        );

        let exact_proposal = fixture.root.join("exact-proposal.json");
        File::create(&exact_proposal)
            .unwrap()
            .set_len(MAX_PROPOSAL_BYTES as u64)
            .unwrap();
        assert_eq!(
            read_only_failure(&fixture, || {
                generate_with_hook(&fixture.root, &exact_proposal, |_| {})
            })
            .code,
            "SPX-G188"
        );
        let oversized_proposal = fixture.root.join("oversized-proposal.json");
        File::create(&oversized_proposal)
            .unwrap()
            .set_len(MAX_PROPOSAL_BYTES as u64 + 1)
            .unwrap();
        assert_eq!(
            read_only_failure(&fixture, || generate_with_hook(
                &fixture.root,
                &oversized_proposal,
                |_| {}
            ))
            .code,
            "SPX-G191"
        );

        let evidence_dir = fixture.root.join("evidence-dir");
        std::fs::create_dir(&evidence_dir).unwrap();
        let error = read_only_failure(&fixture, || {
            verify_with_hook(&fixture.root, &fixture.proposal_path, &evidence_dir, |_| {})
        });
        assert_eq!(error.code, "SPX-I215");
        #[cfg(windows)]
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change Evidence: open failed"
        );
        #[cfg(not(windows))]
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change Evidence: input is not a regular file"
        );
        let invalid_evidence = fixture.root.join("invalid-evidence.json");
        std::fs::write(&invalid_evidence, [0xff]).unwrap();
        let error = read_only_failure(&fixture, || {
            verify_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &invalid_evidence,
                |_| {},
            )
        });
        assert_eq!(error.code, "SPX-I215");
        assert_eq!(
            error.message,
            "could not read Semantic Workspace Structural Change Evidence: input is not UTF-8"
        );
        let exact_evidence = fixture.root.join("exact-evidence.json");
        File::create(&exact_evidence)
            .unwrap()
            .set_len(TEST_MAX_EVIDENCE_BYTES as u64)
            .unwrap();
        assert_eq!(
            read_only_failure(&fixture, || verify_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &exact_evidence,
                |_| {},
            ))
            .code,
            "SPX-G193"
        );
        let oversized_evidence = fixture.root.join("oversized-evidence.json");
        File::create(&oversized_evidence)
            .unwrap()
            .set_len(TEST_MAX_EVIDENCE_BYTES as u64 + 1)
            .unwrap();
        assert_eq!(
            read_only_failure(&fixture, || verify_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &oversized_evidence,
                |_| {},
            ))
            .code,
            "SPX-G191"
        );
    }

    #[test]
    fn owned_inputs_are_never_reopened_and_final_drift_discards_outputs() {
        let fixture = ManagedFixture::new("owned-inputs");
        let baseline = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
        let evidence_path = fixture.root.join("owned-evidence.json");
        std::fs::write(&evidence_path, baseline.evidence()).unwrap();
        let baseline_receipt = verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |_| {},
        )
        .unwrap();

        for replacement in [fixture.proposal_source.as_bytes(), b"{}\n".as_slice()] {
            std::fs::write(&fixture.proposal_path, &fixture.proposal_source).unwrap();
            let replacement_path = fixture.root.join("generate-replacement.json");
            std::fs::write(&replacement_path, replacement).unwrap();
            let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |point| {
                if matches!(point, StructuralGeneratePoint::ProposalOwned) {
                    replace_owned_path(&fixture.proposal_path, &replacement_path);
                }
            })
            .unwrap();
            assert_eq!(
                [
                    artifacts.proposal_digest(),
                    artifacts.candidate_manifest_digest(),
                    artifacts.preview(),
                    artifacts.preview_digest(),
                    artifacts.context(),
                    artifacts.context_digest(),
                    artifacts.impact(),
                    artifacts.impact_digest(),
                    artifacts.review(),
                    artifacts.review_digest(),
                    artifacts.evidence(),
                    artifacts.evidence_digest(),
                ],
                [
                    baseline.proposal_digest(),
                    baseline.candidate_manifest_digest(),
                    baseline.preview(),
                    baseline.preview_digest(),
                    baseline.context(),
                    baseline.context_digest(),
                    baseline.impact(),
                    baseline.impact_digest(),
                    baseline.review(),
                    baseline.review_digest(),
                    baseline.evidence(),
                    baseline.evidence_digest(),
                ]
            );
        }

        for replace_at in ["proposal", "evidence"] {
            std::fs::write(&fixture.proposal_path, &fixture.proposal_source).unwrap();
            std::fs::write(&evidence_path, baseline.evidence()).unwrap();
            let replacement_path = fixture.root.join(format!("{replace_at}-replacement.json"));
            std::fs::write(&replacement_path, "{}\n").unwrap();
            let receipt = verify_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point| match (replace_at, point) {
                    ("proposal", StructuralVerifyPoint::ProposalOwned) => {
                        replace_owned_path(&fixture.proposal_path, &replacement_path);
                    }
                    ("evidence", StructuralVerifyPoint::EvidenceOwned) => {
                        replace_owned_path(&evidence_path, &replacement_path);
                    }
                    _ => {}
                },
            )
            .unwrap();
            assert_eq!(receipt, baseline_receipt);
        }
        fixture.assert_exclusive_reacquire();

        let generate_drift = ManagedFixture::new("generate-final-drift");
        let error = diagnostic(generate_with_hook(
            &generate_drift.root,
            &generate_drift.proposal_path,
            |point| {
                if matches!(point, StructuralGeneratePoint::ArtifactsRendered) {
                    OpenOptions::new()
                        .append(true)
                        .open(generate_drift.root.join(".semaprax-workspace/ACTIVE"))
                        .unwrap()
                        .write_all(b"x")
                        .unwrap();
                }
            },
        ));
        assert_eq!(error.code, "SPX-G153");
        generate_drift.assert_exclusive_reacquire();

        let verify_drift = ManagedFixture::new("verify-final-drift");
        let artifacts =
            generate_with_hook(&verify_drift.root, &verify_drift.proposal_path, |_| {}).unwrap();
        let evidence_path = verify_drift.root.join("evidence.json");
        std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
        let error = diagnostic(verify_with_hook(
            &verify_drift.root,
            &verify_drift.proposal_path,
            &evidence_path,
            |point| {
                if matches!(point, StructuralVerifyPoint::ReceiptRendered) {
                    OpenOptions::new()
                        .append(true)
                        .open(verify_drift.root.join(".semaprax-workspace/ACTIVE"))
                        .unwrap()
                        .write_all(b"x")
                        .unwrap();
                }
            },
        ));
        assert_eq!(error.code, "SPX-G153");
        verify_drift.assert_exclusive_reacquire();
    }

    #[test]
    fn evidence_format_confusion_and_exact_replay_mutations_fail_closed() {
        let fixture = ManagedFixture::new("evidence-hostile");
        let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
        let evidence = artifacts.evidence();
        let schema_prefix = concat!(
            "{\"schema\":\"semaprax.workspace-semantic-structural-change-evidence.v1\",",
            "\"workspace_manifest_schema\":\"semaprax.workspace-semantic-manifest.v1\""
        );
        let reordered_prefix = concat!(
            "{\"workspace_manifest_schema\":\"semaprax.workspace-semantic-manifest.v1\",",
            "\"schema\":\"semaprax.workspace-semantic-structural-change-evidence.v1\""
        );
        let mut missing = evidence.to_owned();
        missing = missing.replace("\"entry_module\":\"structural.entry\",", "");
        let extra = evidence.replacen("{\"schema\":", "{\"extra\":0,\"schema\":", 1);
        let duplicate =
            evidence.replacen("{\"schema\":", "{\"schema\":\"duplicate\",\"schema\":", 1);
        let reordered = evidence.replacen(schema_prefix, reordered_prefix, 1);
        let wrong_type = evidence.replace(
            "\"entry_module\":\"structural.entry\"",
            "\"entry_module\":0",
        );
        let no_lf = evidence.trim_end_matches('\n').to_owned();
        let crlf = format!("{}\r\n", evidence.trim_end_matches('\n'));
        let bom = format!("\u{feff}{evidence}");
        let two_lines = format!("{evidence}\n");
        let proposal_ref = object_after(evidence, "\"proposal\":");
        let missing_proposal_ref_field = evidence.replacen(
            proposal_ref,
            &remove_nonfirst_scalar_field(proposal_ref, "bytes"),
            1,
        );
        let extra_proposal_ref_field = evidence.replacen(
            proposal_ref,
            &proposal_ref.replacen('{', "{\"extra\":0,", 1),
            1,
        );
        let wrong_proposal_ref_type = evidence.replacen(
            proposal_ref,
            &replace_scalar_field(proposal_ref, "bytes", "\"invalid\""),
            1,
        );
        let graph_ref = object_after(evidence, "\"base_workspace_graph\":");
        let missing_graph_digest = evidence.replacen(
            graph_ref,
            &remove_nonfirst_scalar_field(graph_ref, "digest"),
            1,
        );
        let path_row = object_after(evidence, "\"paths\":[");
        let missing_path_peer = evidence.replacen(
            path_row,
            &remove_nonfirst_scalar_field(path_row, "peer_path"),
            1,
        );
        let limits = object_after(evidence, "\"limits\":");
        let missing_limit = evidence.replacen(
            limits,
            &remove_nonfirst_scalar_field(limits, "max_operations"),
            1,
        );
        let budget = object_after(evidence, "\"budget\":");
        let wrong_budget_type = evidence.replacen(
            budget,
            &replace_scalar_field(budget, "used_operations", "\"four\""),
            1,
        );
        let wrong_nonclaim_type =
            evidence.replacen("\"not_signature_or_authenticated_provenance\"", "0", 1);
        let evidence_path = fixture.root.join("evidence.json");
        let mut format_hostiles = vec![
            missing,
            extra,
            duplicate,
            reordered,
            wrong_type,
            no_lf,
            crlf,
            bom,
            two_lines,
            "[[[[[[[[[[]]]]]]]]]\n".to_owned(),
            missing_proposal_ref_field,
            extra_proposal_ref_field,
            wrong_proposal_ref_type,
            missing_graph_digest,
            missing_path_peer,
            missing_limit,
            wrong_budget_type,
            wrong_nonclaim_type,
        ];
        for (marker, first, second) in [
            ("\"proposal\":", "schema", "digest"),
            ("\"base_workspace_graph\":", "schema", "digest"),
            ("\"paths\":[", "path", "change"),
            ("\"limits\":", "max_managed_files", "max_operations"),
            (
                "\"budget\":",
                "used_base_managed_files",
                "used_candidate_managed_files",
            ),
        ] {
            format_hostiles.extend(nested_shape_mutations(evidence, marker, first, second));
        }
        for hostile in format_hostiles {
            assert_eq!(
                diagnostic(verification::parse_evidence(&hostile)).code,
                "SPX-G193"
            );
            std::fs::write(&evidence_path, hostile).unwrap();
            assert_eq!(
                read_only_failure(&fixture, || verify_with_hook(
                    &fixture.root,
                    &fixture.proposal_path,
                    &evidence_path,
                    |_| {},
                ))
                .code,
                "SPX-G193"
            );
        }

        std::fs::write(&evidence_path, evidence).unwrap();
        let receipt = verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |_| {},
        )
        .unwrap();
        assert_eq!(
            diagnostic(verification::parse_evidence(&receipt)).code,
            "SPX-G193"
        );
        std::fs::write(&evidence_path, &receipt).unwrap();
        let malformed_proposal = fixture.root.join("receipt-confusion-proposal.json");
        std::fs::write(&malformed_proposal, "{}\n").unwrap();
        assert_eq!(
            read_only_failure(&fixture, || verify_with_hook(
                &fixture.root,
                &malformed_proposal,
                &evidence_path,
                |_| {},
            ))
            .code,
            "SPX-G193"
        );

        let path_row = object_after(evidence, "\"paths\":[");
        let proposal_ref = object_after(evidence, "\"proposal\":");
        let graph_ref = object_after(evidence, "\"base_workspace_graph\":");
        let limits = object_after(evidence, "\"limits\":");
        let budget = object_after(evidence, "\"budget\":");
        let replay_mutations = [
            evidence.replace(
                "\"entry_module\":\"structural.entry\"",
                "\"entry_module\":\"structural.entri\"",
            ),
            evidence.replacen(
                proposal_ref,
                &replace_scalar_field(
                    proposal_ref,
                    "digest",
                    &format!("\"sha256:{}\"", "0".repeat(64)),
                ),
                1,
            ),
            evidence.replacen(
                graph_ref,
                &replace_scalar_field(
                    graph_ref,
                    "digest",
                    &format!("\"sha256:{}\"", "0".repeat(64)),
                ),
                1,
            ),
            evidence.replacen(
                path_row,
                &replace_scalar_field(path_row, "change", "\"mutated\""),
                1,
            ),
            evidence.replacen(
                limits,
                &replace_scalar_field(limits, "max_managed_files", "15"),
                1,
            ),
            evidence.replacen(
                budget,
                &replace_scalar_field(budget, "used_operations", "3"),
                1,
            ),
            evidence.replacen(
                "\"not_signature_or_authenticated_provenance\"",
                "\"mutated_nonclaim\"",
                1,
            ),
        ];
        for mutated in replay_mutations {
            if let Err(diagnostics) = verification::parse_evidence(&mutated) {
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, "SPX-G195");
            }
            std::fs::write(&evidence_path, mutated).unwrap();
            let error = read_only_failure(&fixture, || {
                verify_with_hook(
                    &fixture.root,
                    &fixture.proposal_path,
                    &evidence_path,
                    |_| {},
                )
            });
            assert_eq!(error.code, "SPX-G195");
            assert_eq!(
                error.message,
                "Semantic Workspace Structural Change Evidence does not exactly replay the authenticated proposal and candidate"
            );
        }
    }

    #[test]
    fn structural_apply_process_child() {
        if std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_CHILD").is_none() {
            return;
        }
        let root = PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_ROOT").unwrap());
        let proposal =
            PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_PROPOSAL").unwrap());
        let evidence =
            PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_EVIDENCE").unwrap());
        let boundary = std::env::var("SEMAPRAX_STRUCTURAL_APPLY_BOUNDARY").unwrap();
        let ready = PathBuf::from(std::env::var_os("SEMAPRAX_STRUCTURAL_APPLY_READY").unwrap());
        apply_authenticated_with_hook(&root, &proposal, &evidence, |point, _, _, _| {
            let selected = matches!(
                (boundary.as_str(), point),
                (
                    "pre",
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::BeforeActiveReplace
                    )
                ) | (
                    "post",
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterActiveReplace
                    )
                )
            );
            if selected {
                let mut marker = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&ready)?;
                marker.write_all(b"ready\n")?;
                marker.sync_all()?;
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn structural_apply_killed_process_boundaries_preserve_exact_old_or_new() {
        for boundary in ["pre", "post"] {
            let (fixture, evidence_path) = application_fixture(&format!("process-kill-{boundary}"));
            let old_revision = workspace_graph::snapshot(&fixture.root, "structural.entry")
                .unwrap()
                .workspace_revision()
                .to_owned();
            let evidence = std::fs::read_to_string(&evidence_path).unwrap();
            let candidate_revision = serde_json::from_str::<Value>(&evidence).unwrap()
                ["candidate_workspace_revision"]
                .as_str()
                .unwrap()
                .to_owned();
            let raw_before = fixture.raw_inventory();

            let (mut child, ready) =
                spawn_structural_apply_process(&fixture, &evidence_path, boundary);
            assert_eq!(std::fs::read(&ready).unwrap(), b"ready\n");
            let held_lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(fixture.root.join(".semaprax-workspace/LOCK"))
                .unwrap();
            assert!(FileExt::try_lock_exclusive(&held_lock).is_err());
            child.kill().unwrap();
            assert!(!child.wait().unwrap().success());
            std::fs::remove_file(&ready).unwrap();

            fixture.assert_exclusive_reacquire();
            let current = workspace_graph::snapshot(&fixture.root, "structural.entry").unwrap();
            assert_eq!(
                current.workspace_revision(),
                if boundary == "pre" {
                    &old_revision
                } else {
                    &candidate_revision
                }
            );
            let mut expected_generations = [old_revision.as_str(), candidate_revision.as_str()]
                .map(|revision| revision.strip_prefix("sha256:").unwrap().to_owned())
                .to_vec();
            expected_generations.sort();
            let generations_path = fixture.root.join(".semaprax-workspace/generations");
            assert_eq!(directory_names(&generations_path), expected_generations);
            for generation in &expected_generations {
                let metadata =
                    std::fs::symlink_metadata(generations_path.join(generation)).unwrap();
                assert!(metadata.is_dir());
                assert!(!metadata.file_type().is_symlink());
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt as _;
                    assert_eq!(metadata.file_attributes() & 0x400, 0);
                }
            }
            let staging_path = fixture.root.join(".semaprax-workspace/staging");
            let staging_names = directory_names(&staging_path);
            if boundary == "pre" {
                assert_eq!(staging_names, ["0"]);
                let metadata = std::fs::symlink_metadata(staging_path.join("0")).unwrap();
                assert!(metadata.is_file());
                assert!(!metadata.file_type().is_symlink());
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt as _;
                    assert_eq!(metadata.file_attributes() & 0x400, 0);
                }
            } else {
                assert!(staging_names.is_empty());
            }
            assert_eq!(fixture.raw_inventory(), raw_before);
        }
    }

    #[test]
    fn structural_apply_publishes_exact_candidate_once_without_raw_writes() {
        let (fixture, evidence_path) = application_fixture("apply-success");
        let raw_before = fixture.raw_inventory();
        let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
        let points = std::cell::RefCell::new(Vec::new());
        let candidate_path = std::cell::RefCell::new(None::<PathBuf>);
        let receipt = apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                points.borrow_mut().push(apply_point_name(point));
                if let Some(candidate) = candidate {
                    *candidate_path.borrow_mut() = Some(candidate.to_owned());
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            *points.borrow(),
            [
                "proposal_owned",
                "evidence_owned",
                "after_replay",
                "receipt_rendered",
                "generation_after_slot_create",
                "generation_after_manifest_write",
                "generation_after_files_write",
                "generation_before_stage_validation",
                "generation_before_publish",
                "generation_destination_checked",
                "generation_after_publish",
                "after_candidate_prepared",
                "after_active_staged",
                "before_first_final_check",
                "before_second_final_check",
                "before_active_replace",
                "after_active_replace",
            ]
        );
        let receipt_value: Value = serde_json::from_str(&receipt).unwrap();
        assert_eq!(
            receipt_value["schema"],
            "semaprax.workspace-semantic-structural-change-evidence-application.v1"
        );
        assert_eq!(receipt_value["result"], "applied");
        assert_eq!(
            raw_sha(&receipt),
            "sha256:6bb4ae6e865e1e112af85ffdf243229a374813034ad47544907880781caeba47"
        );
        assert_eq!(fixture.raw_inventory(), raw_before);
        assert_ne!(
            std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
            active_before
        );
        assert!(candidate_path.borrow().as_ref().unwrap().is_dir());
        assert_eq!(
            fixture.authenticated_paths_and_storage(),
            (
                vec![
                    "b/created.spx".to_owned(),
                    "c/provider.spx".to_owned(),
                    "m/consumer.spx".to_owned(),
                    "z/entry.spx".to_owned(),
                ],
                2,
                0,
            )
        );
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn every_structural_apply_hook_maps_pre_and_post_pivot_failures_exactly() {
        for target in [
            "proposal_owned",
            "evidence_owned",
            "after_replay",
            "receipt_rendered",
            "generation_after_slot_create",
            "generation_after_manifest_write",
            "generation_after_files_write",
            "generation_before_stage_validation",
            "generation_before_publish",
            "generation_destination_checked",
            "generation_after_publish",
            "after_candidate_prepared",
            "after_active_staged",
            "before_first_final_check",
            "before_second_final_check",
            "before_active_replace",
            "after_active_replace",
        ] {
            let (fixture, evidence_path) = application_fixture(target);
            let active_before =
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
            let reached = std::cell::Cell::new(false);
            let error = diagnostic(apply_authenticated_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, _, _, _| {
                    if apply_point_name(point) == target {
                        reached.set(true);
                        return Err(std::io::Error::other("injected boundary failure"));
                    }
                    Ok(())
                },
            ));
            assert!(reached.get(), "hook was not reached: {target}");
            assert_eq!(
                error.code,
                if target == "after_active_replace" {
                    "SPX-I212"
                } else {
                    "SPX-I211"
                },
                "unexpected diagnostic at {target}: {error:?}"
            );
            let active_after =
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
            if target == "after_active_replace" {
                assert_ne!(active_after, active_before);
            } else {
                assert_eq!(active_after, active_before);
            }
            fixture.assert_exclusive_reacquire();
        }
    }

    #[test]
    fn published_candidate_residue_requires_new_evidence_then_reuses_exact_path() {
        let (fixture, evidence_path) = application_fixture("candidate-residue");
        let active_before = std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
        let first_candidate = std::cell::RefCell::new(None::<PathBuf>);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterCandidatePrepared
                    )
                ) {
                    *first_candidate.borrow_mut() = candidate.map(Path::to_owned);
                    return Err(std::io::Error::other("stop after candidate publication"));
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-I211");
        let first_candidate = first_candidate.into_inner().unwrap();
        let candidate_inventory = {
            let fixture_inventory = fixture.inventory();
            fixture_inventory
                .into_iter()
                .filter(|(path, _, _)| {
                    first_candidate
                        .strip_prefix(&fixture.root)
                        .is_ok_and(|prefix| path.starts_with(prefix.to_string_lossy().as_ref()))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
            active_before
        );
        assert_eq!(fixture.authenticated_paths_and_storage().1, 2);

        let stale = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |_, _, _, _| Ok(()),
        ));
        assert_eq!(stale.code, "SPX-G195");
        let regenerated =
            generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
        std::fs::write(&evidence_path, regenerated.evidence()).unwrap();
        let reused_candidate = std::cell::RefCell::new(None::<PathBuf>);
        apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterCandidatePrepared
                    )
                ) {
                    *reused_candidate.borrow_mut() = candidate.map(Path::to_owned);
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(reused_candidate.into_inner().unwrap(), first_candidate);
        assert_eq!(fixture.authenticated_paths_and_storage().1, 2);
        let after_inventory = fixture.inventory();
        for fact in candidate_inventory {
            assert!(after_inventory.contains(&fact));
        }
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn structural_final_rechecks_detect_identity_and_post_pivot_candidate_drift() {
        let (identity_fixture, identity_evidence) = application_fixture("identity-drift");
        let active_path = identity_fixture.root.join(".semaprax-workspace/ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let error = diagnostic(apply_authenticated_with_hook(
            &identity_fixture.root,
            &identity_fixture.proposal_path,
            &identity_evidence,
            |point, active, _, _| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                    )
                ) {
                    let replacement = active.with_extension("replacement");
                    std::fs::write(&replacement, std::fs::read(active).unwrap())?;
                    std::fs::remove_file(active)?;
                    std::fs::rename(replacement, active)?;
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-G153");
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        identity_fixture.assert_exclusive_reacquire();

        let (post_fixture, post_evidence) = application_fixture("post-pivot-drift");
        let error = diagnostic(apply_authenticated_with_hook(
            &post_fixture.root,
            &post_fixture.proposal_path,
            &post_evidence,
            |point, _, _, candidate| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterActiveReplace
                    )
                ) {
                    let candidate = candidate.unwrap();
                    OpenOptions::new()
                        .append(true)
                        .open(candidate.join("files/z/entry.spx"))?
                        .write_all(b"x")?;
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-I212");
        post_fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn structural_apply_replay_failure_is_zero_write_and_destination_races_never_clobber() {
        let (fixture, evidence_path) = application_fixture("apply-replay-zero-write");
        let evidence = std::fs::read_to_string(&evidence_path).unwrap().replace(
            "\"entry_module\":\"structural.entry\"",
            "\"entry_module\":\"structural.entri\"",
        );
        verification::parse_evidence(&evidence).unwrap();
        std::fs::write(&evidence_path, evidence).unwrap();
        let before = fixture.inventory();
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |_, _, _, _| Ok(()),
        ));
        assert_eq!(error.code, "SPX-G195");
        assert_eq!(fixture.inventory(), before);
        fixture.assert_exclusive_reacquire();

        for kind in ["file", "directory"] {
            let (fixture, evidence_path) = application_fixture(&format!("destination-{kind}"));
            let active_before =
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
            let foreign = std::cell::RefCell::new(None::<PathBuf>);
            let error = diagnostic(apply_authenticated_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, _, _, candidate| {
                    if matches!(
                        point,
                        StructuralApplyPoint::Workspace(
                            workspace::SemanticChangeApplyPoint::Generation(
                                workspace::GenerationPoint::DestinationChecked
                            )
                        )
                    ) {
                        let candidate = candidate.unwrap();
                        if kind == "file" {
                            std::fs::write(candidate, "foreign structural generation\n")?;
                        } else {
                            std::fs::create_dir(candidate)?;
                        }
                        *foreign.borrow_mut() = Some(candidate.to_owned());
                    }
                    Ok(())
                },
            ));
            assert_eq!(error.code, "SPX-I211");
            let foreign = foreign.into_inner().unwrap();
            assert_eq!(foreign.is_file(), kind == "file");
            assert_eq!(foreign.is_dir(), kind == "directory");
            assert_eq!(
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
                active_before
            );
            fixture.assert_exclusive_reacquire();
        }
    }

    #[test]
    fn structural_generation_rechecks_reject_same_byte_manifest_and_source_substitution() {
        for (label, point, relative) in [
            (
                "manifest",
                workspace::GenerationPoint::AfterManifestWrite,
                "manifest.json",
            ),
            (
                "source",
                workspace::GenerationPoint::AfterFilesWrite,
                "files/z/entry.spx",
            ),
        ] {
            let (fixture, evidence_path) = application_fixture(&format!("generation-{label}"));
            let active_before =
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
            let substituted = std::cell::Cell::new(false);
            let error = diagnostic(apply_authenticated_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |current, _, staged, _| {
                    if matches!(
                        current,
                        StructuralApplyPoint::Workspace(
                            workspace::SemanticChangeApplyPoint::Generation(observed)
                        ) if observed == point
                    ) {
                        let path = staged.unwrap().join(relative);
                        let bytes = std::fs::read(&path)?;
                        std::fs::remove_file(&path)?;
                        std::fs::write(path, bytes)?;
                        substituted.set(true);
                    }
                    Ok(())
                },
            ));
            assert!(substituted.get());
            assert_eq!(error.code, "SPX-G153");
            assert_eq!(
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
                active_before
            );
            fixture.assert_exclusive_reacquire();
        }
    }

    #[cfg(unix)]
    #[test]
    fn structural_generation_rejects_staged_symlink_and_hardlink_aliases() {
        use std::os::unix::fs::symlink;

        for kind in ["symlink", "hardlink"] {
            let (fixture, evidence_path) = application_fixture(&format!("alias-{kind}"));
            let active_before =
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap();
            let error = diagnostic(apply_authenticated_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, _, staged, _| {
                    if matches!(
                        point,
                        StructuralApplyPoint::Workspace(
                            workspace::SemanticChangeApplyPoint::Generation(
                                workspace::GenerationPoint::AfterFilesWrite
                            )
                        )
                    ) {
                        let staged = staged.unwrap();
                        let target = staged.join("files/z/entry.spx");
                        std::fs::remove_file(&target)?;
                        if kind == "symlink" {
                            symlink(&fixture.proposal_path, target)?;
                        } else {
                            std::fs::hard_link(staged.join("files/m/consumer.spx"), target)?;
                        }
                    }
                    Ok(())
                },
            ));
            assert_eq!(error.code, "SPX-G153");
            assert_eq!(
                std::fs::read(fixture.root.join(".semaprax-workspace/ACTIVE")).unwrap(),
                active_before
            );
            fixture.assert_exclusive_reacquire();
        }
    }

    #[cfg(unix)]
    #[test]
    fn structural_candidate_destination_aliases_preserve_foreign_targets() {
        use std::os::unix::fs::symlink;

        for kind in ["symlink", "hardlink"] {
            let (fixture, evidence_path) = application_fixture(&format!("destination-{kind}"));
            let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
            let active_before = std::fs::read(&active_path).unwrap();
            let foreign = fixture.root.join(format!("foreign-{kind}-target"));
            if kind == "symlink" {
                std::fs::create_dir(&foreign).unwrap();
                std::fs::write(foreign.join("sentinel.txt"), b"foreign-directory-target\n")
                    .unwrap();
            } else {
                std::fs::write(&foreign, b"foreign-file-target\n").unwrap();
            }
            let alias = std::cell::RefCell::new(None::<PathBuf>);
            let error = diagnostic(apply_authenticated_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, _, _, candidate| {
                    if matches!(
                        point,
                        StructuralApplyPoint::Workspace(
                            workspace::SemanticChangeApplyPoint::Generation(
                                workspace::GenerationPoint::DestinationChecked
                            )
                        )
                    ) {
                        let destination = candidate.unwrap();
                        if kind == "symlink" {
                            symlink(&foreign, destination)?;
                        } else {
                            std::fs::hard_link(&foreign, destination)?;
                        }
                        *alias.borrow_mut() = Some(destination.to_owned());
                    }
                    Ok(())
                },
            ));
            assert_eq!(error.code, "SPX-I211");
            assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
            let alias = alias.into_inner().unwrap();
            if kind == "symlink" {
                assert!(std::fs::symlink_metadata(&alias)
                    .unwrap()
                    .file_type()
                    .is_symlink());
                std::fs::remove_file(alias).unwrap();
                assert_eq!(
                    std::fs::read(foreign.join("sentinel.txt")).unwrap(),
                    b"foreign-directory-target\n"
                );
            } else {
                assert!(alias.is_file());
                assert_eq!(std::fs::read(&alias).unwrap(), b"foreign-file-target\n");
                std::fs::remove_file(alias).unwrap();
                assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign-file-target\n");
            }
            fixture.assert_exclusive_reacquire();
        }
    }

    #[cfg(unix)]
    #[test]
    fn structural_apply_rejects_permission_drift_without_pivot() {
        use std::os::unix::fs::PermissionsExt;

        for case in ["lock", "active", "candidate"] {
            let (fixture, evidence_path) = application_fixture(&format!("permission-{case}"));
            let control = fixture.root.join(".semaprax-workspace");
            let active_path = control.join("ACTIVE");
            let active_before = std::fs::read(&active_path).unwrap();
            let changed = std::cell::RefCell::new(None::<(PathBuf, std::fs::Permissions)>);
            let error = diagnostic(apply_authenticated_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, active, _, candidate| {
                    if !matches!(
                        point,
                        StructuralApplyPoint::Workspace(
                            workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                        )
                    ) {
                        return Ok(());
                    }
                    let path = match case {
                        "lock" => control.join("LOCK"),
                        "active" => active.to_owned(),
                        "candidate" => candidate.unwrap().join("manifest.json"),
                        _ => unreachable!(),
                    };
                    let original = std::fs::metadata(&path)?.permissions();
                    let mut altered = original.clone();
                    altered.set_mode(altered.mode() ^ 0o100);
                    std::fs::set_permissions(&path, altered)?;
                    *changed.borrow_mut() = Some((path, original));
                    Ok(())
                },
            ));
            assert_eq!(error.code, "SPX-G153");
            assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
            let (path, permissions) = changed.into_inner().unwrap();
            std::fs::set_permissions(path, permissions).unwrap();
            fixture.assert_exclusive_reacquire();
        }
    }

    #[test]
    fn cooperative_reader_observes_only_locked_old_then_complete_new_structural_state() {
        let (fixture, evidence_path) = application_fixture("cooperative-reader");
        let artifacts = generate_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
        let expected_revision = serde_json::from_str::<Value>(artifacts.evidence()).unwrap()
            ["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let (arrived_tx, arrived_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        std::thread::scope(|scope| {
            let root = fixture.root.as_path();
            let proposal_path = fixture.proposal_path.as_path();
            let evidence_path = evidence_path.as_path();
            let writer = scope.spawn(move || {
                apply_authenticated_with_hook(
                    root,
                    proposal_path,
                    evidence_path,
                    |point, _, _, _| {
                        if matches!(
                            point,
                            StructuralApplyPoint::Workspace(
                                workspace::SemanticChangeApplyPoint::BeforeActiveReplace
                            )
                        ) {
                            arrived_tx.send(()).unwrap();
                            release_rx.recv().unwrap();
                        }
                        Ok(())
                    },
                )
            });
            arrived_rx.recv().unwrap();
            let diagnostics = match workspace_graph::snapshot(&fixture.root, "structural.entry") {
                Ok(_) => panic!("reader must not observe an in-progress structural generation"),
                Err(diagnostics) => diagnostics,
            };
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, "SPX-I210");
            assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
            release_tx.send(()).unwrap();
            writer.join().unwrap().unwrap();
        });
        assert_eq!(
            workspace_graph::snapshot(&fixture.root, "structural.entry")
                .unwrap()
                .workspace_revision(),
            expected_revision
        );
        fixture.assert_exclusive_reacquire();
    }

    #[cfg(windows)]
    #[test]
    fn structural_apply_rejects_windows_junction_and_casefold_destination_without_clobber() {
        use std::process::Command;

        let (junction_fixture, junction_evidence) = application_fixture("windows-junction");
        let active_path = junction_fixture.root.join(".semaprax-workspace/ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let foreign = junction_fixture.root.join("foreign-junction-target");
        std::fs::create_dir(&foreign).unwrap();
        let sentinel = foreign.join("sentinel.txt");
        std::fs::write(&sentinel, b"structural-foreign-junction\n").unwrap();
        let junction = std::cell::RefCell::new(None::<PathBuf>);
        let error = diagnostic(apply_authenticated_with_hook(
            &junction_fixture.root,
            &junction_fixture.proposal_path,
            &junction_evidence,
            |point, _, _, candidate| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::Generation(
                            workspace::GenerationPoint::BeforeGenerationPublish
                        )
                    )
                ) {
                    let destination = candidate.unwrap();
                    let status = Command::new("cmd")
                        .args(["/C", "mklink", "/J"])
                        .arg(destination)
                        .arg(&foreign)
                        .status()?;
                    assert!(status.success(), "mklink /J failed");
                    *junction.borrow_mut() = Some(destination.to_owned());
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-I211");
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        let junction = junction.into_inner().unwrap();
        {
            use std::os::windows::fs::MetadataExt as _;
            assert!(
                std::fs::symlink_metadata(&junction)
                    .unwrap()
                    .file_attributes()
                    & 0x400
                    != 0
            );
        }
        std::fs::remove_dir(junction).unwrap();
        assert_eq!(
            std::fs::read(&sentinel).unwrap(),
            b"structural-foreign-junction\n"
        );
        junction_fixture.assert_exclusive_reacquire();

        let (case_fixture, case_evidence) = application_fixture("windows-casefold");
        let active_path = case_fixture.root.join(".semaprax-workspace/ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let alias = std::cell::RefCell::new(None::<PathBuf>);
        let error = diagnostic(apply_authenticated_with_hook(
            &case_fixture.root,
            &case_fixture.proposal_path,
            &case_evidence,
            |point, _, _, candidate| {
                if matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::Generation(
                            workspace::GenerationPoint::DestinationChecked
                        )
                    )
                ) {
                    let candidate = candidate.unwrap();
                    let name = candidate.file_name().unwrap().to_string_lossy();
                    let upper = name.to_ascii_uppercase();
                    assert_ne!(upper, name);
                    let path = candidate.with_file_name(upper);
                    std::fs::create_dir(&path)?;
                    *alias.borrow_mut() = Some(path);
                }
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-I211");
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        assert!(alias.into_inner().unwrap().is_dir());
        case_fixture.assert_exclusive_reacquire();
    }

    #[cfg(windows)]
    #[test]
    fn structural_apply_rejects_windows_readonly_permission_drift() {
        for case in ["lock", "active", "candidate"] {
            let (fixture, evidence_path) = application_fixture(&format!("windows-readonly-{case}"));
            let control = fixture.root.join(".semaprax-workspace");
            let active_path = control.join("ACTIVE");
            let active_before = std::fs::read(&active_path).unwrap();
            let changed = std::cell::RefCell::new(None::<(PathBuf, std::fs::Permissions)>);
            let error = diagnostic(apply_authenticated_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, active, _, candidate| {
                    if !matches!(
                        point,
                        StructuralApplyPoint::Workspace(
                            workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                        )
                    ) {
                        return Ok(());
                    }
                    let path = match case {
                        "lock" => control.join("LOCK"),
                        "active" => active.to_owned(),
                        "candidate" => candidate.unwrap().join("manifest.json"),
                        _ => unreachable!(),
                    };
                    let original = std::fs::metadata(&path)?.permissions();
                    let mut altered = original.clone();
                    altered.set_readonly(!altered.readonly());
                    std::fs::set_permissions(&path, altered)?;
                    *changed.borrow_mut() = Some((path, original));
                    Ok(())
                },
            ));
            assert_eq!(error.code, "SPX-G153");
            assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
            let (path, permissions) = changed.into_inner().unwrap();
            std::fs::set_permissions(path, permissions).unwrap();
            fixture.assert_exclusive_reacquire();
        }
    }

    #[cfg(windows)]
    #[test]
    fn structural_apply_rejects_windows_same_byte_file_index_substitution() {
        let (fixture, evidence_path) = application_fixture("windows-file-index");
        let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
        let active_before = std::fs::read(&active_path).unwrap();
        let identities = std::cell::RefCell::new(None::<(u64, u64)>);
        let error = diagnostic(apply_authenticated_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                if !matches!(
                    point,
                    StructuralApplyPoint::Workspace(
                        workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck
                    )
                ) {
                    return Ok(());
                }
                let path = candidate.unwrap().join("files/z/entry.spx");
                let before = winapi_util::Handle::from_path_any(&path)
                    .and_then(winapi_util::file::information)?
                    .file_index();
                let bytes = std::fs::read(&path)?;
                std::fs::remove_file(&path)?;
                std::fs::write(&path, bytes)?;
                let after = winapi_util::Handle::from_path_any(&path)
                    .and_then(winapi_util::file::information)?
                    .file_index();
                assert_ne!(before, after);
                *identities.borrow_mut() = Some((before, after));
                Ok(())
            },
        ));
        assert_eq!(error.code, "SPX-G153");
        assert!(identities.into_inner().is_some());
        assert_eq!(std::fs::read(&active_path).unwrap(), active_before);
        fixture.assert_exclusive_reacquire();
    }
}
