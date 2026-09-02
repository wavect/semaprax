//! Authenticated semantic-workspace change preview, evidence, and application.
//!
//! The read-only public routes own one canonical proposal file while holding
//! the shared semantic-workspace lock, validate the complete replacements-only
//! candidate, and return bounded canonical Preview or Evidence artifacts. Submitted
//! Evidence can be verified by exact replay into a one-invocation receipt.
//! Exact submitted Evidence may authorize this invocation's replacements-only
//! candidate publication through one exclusive lock and sole `ACTIVE` pivot.
//! This module has no reusable token, signature, approval, rollback, cleanup,
//! backend, runtime, or managed-source create, delete, move, or path-set-change
//! authority.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::{Map, Value};

use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::{hir, semantic_workspace, workspace, workspace_graph};

mod artifact;
mod verification;

pub use artifact::SemanticWorkspaceChangeArtifacts;

pub(crate) fn render_prepared_artifacts(
    prepared: &SemanticWorkspacePreparedChange,
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>> {
    artifact::render_artifacts(prepared)
}

pub(crate) const EVIDENCE_SCHEMA: &str = artifact::EVIDENCE_SCHEMA;

pub(crate) fn evidence_artifact_digest(source: &str) -> String {
    artifact::digest_evidence(source)
}

pub(crate) fn validate_evidence_document(source: &str) -> Result<(), Vec<Diagnostic>> {
    verification::parse_evidence(source).map(|_| ())
}

/// Generates the complete authenticated read-only change artifact bundle.
pub fn generate(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>> {
    generate_with_operation_hook(root, proposal_path, |_| {})
}

#[derive(Clone, Copy)]
enum GeneratePoint {
    ProposalOwned,
    ArtifactsRendered,
}

fn generate_with_operation_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(GeneratePoint),
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_proposal(proposal_path).and_then(|source| parse_proposal(&source));
    if proposal.is_ok() {
        hook(GeneratePoint::ProposalOwned);
    }
    let (authority, change_set) = locked.authenticate(proposal)?;
    with_authenticated_change_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        hook(GeneratePoint::ArtifactsRendered);
        Ok(artifacts)
    })
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "test-only boundary hook supports lock/read and final-recheck regressions"
)]
fn generate_with_hook(
    root: &Path,
    proposal_path: &Path,
    hook: impl FnMut(GeneratePoint),
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>> {
    generate_with_operation_hook(root, proposal_path, hook)
}

/// Generates the canonical Change Preview document, including its terminal LF.
pub fn preview(root: &Path, proposal_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate(root, proposal_path).map(artifact::SemanticWorkspaceChangeArtifacts::into_preview)
}

/// Generates the canonical Change Evidence capsule, including its terminal LF.
pub fn evidence(root: &Path, proposal_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate(root, proposal_path).map(artifact::SemanticWorkspaceChangeArtifacts::into_evidence)
}

/// Verifies one submitted Change Evidence capsule by exact authenticated replay.
pub fn verify(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    verify_with_operation_hook(root, proposal_path, evidence_path, |_| {})
}

/// Applies one replacements-only change after exact Evidence replay.
pub fn apply(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    apply_authenticated_with_hook(root, proposal_path, evidence_path, |_, _, _, _| Ok(()))
}

#[derive(Clone, Copy)]
enum VerifyPoint {
    ProposalOwned,
    EvidenceOwned,
    ReceiptRendered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticApplyPoint {
    ProposalOwned,
    EvidenceOwned,
    AfterReplay,
    ReceiptRendered,
    Workspace(workspace::SemanticChangeApplyPoint),
}

fn verify_with_operation_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(VerifyPoint),
) -> Result<String, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(VerifyPoint::ProposalOwned);
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(VerifyPoint::EvidenceOwned);
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) = locked.authenticate(input)?;
    with_authenticated_change_authority(authority, change_set, |prepared| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        let receipt =
            artifact::render_verification_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(VerifyPoint::ReceiptRendered);
        Ok(receipt)
    })
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "test-only boundary hook supports owned-input and final-recheck regressions"
)]
fn verify_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    hook: impl FnMut(VerifyPoint),
) -> Result<String, Vec<Diagnostic>> {
    verify_with_operation_hook(root, proposal_path, evidence_path, hook)
}

pub(crate) fn apply_authenticated_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(SemanticApplyPoint, &Path, Option<&Path>, Option<&Path>) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_apply_lock(root)?;
    let active_path = root.join(".semaprax-workspace/ACTIVE");
    let input = read_proposal(proposal_path).and_then(|proposal_source| {
        hook(SemanticApplyPoint::ProposalOwned, &active_path, None, None)
            .map_err(|error| proposal_io_hook("proposal post-read hook failed", error))?;
        let evidence_source = verification::read_evidence(evidence_path)?;
        let submitted = verification::parse_evidence(&evidence_source)?;
        hook(SemanticApplyPoint::EvidenceOwned, &active_path, None, None)
            .map_err(|error| proposal_io_hook("Evidence post-read hook failed", error))?;
        let change_set = parse_proposal(&proposal_source)?;
        Ok((change_set, evidence_source, submitted))
    });
    let (authority, (change_set, evidence_source, submitted)) = locked.authenticate(input)?;
    let (authority, prepared) = prepare_authenticated_change_authority(authority, change_set)?;
    let prepublication = (|| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        verification::verify_replay(&submitted, &evidence_source, &artifacts)?;
        hook(SemanticApplyPoint::AfterReplay, &active_path, None, None)
            .map_err(|error| proposal_io_hook("exact replay hook failed", error))?;
        let receipt =
            artifact::render_application_receipt(&prepared, &artifacts, evidence_source.len())?;
        hook(
            SemanticApplyPoint::ReceiptRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| proposal_io_hook("application receipt hook failed", error))?;
        Ok(receipt)
    })();
    let receipt = match prepublication {
        Ok(receipt) => receipt,
        Err(diagnostics) => return authority.finish(Err(diagnostics)),
    };
    let (candidate_files, candidate_manifest, candidate_revision) =
        prepared.into_candidate_generation_parts();
    let commit = SemanticWorkspaceChangeCommitAuthority {
        authority,
        candidate_files,
        candidate_manifest,
        candidate_revision,
        receipt,
    };
    workspace::commit_semantic_change_authority_with_hook(
        commit,
        |point, active, staged, candidate| {
            hook(
                SemanticApplyPoint::Workspace(point),
                active,
                staged,
                candidate,
            )
        },
    )
}

/// In-memory Project bridge: shared authority is acquired before replaying the
/// caller's candidate history. It has no publication path and creates no files.
pub(crate) fn with_project_candidate_change<T>(
    root: &Path,
    derive: impl FnOnce(
        &str,
        &[workspace::WorkspaceSemanticSource],
    ) -> Result<SemanticWorkspaceChangeSet, Vec<Diagnostic>>,
    review: impl FnOnce(
        &SemanticWorkspacePreparedChange,
        &SemanticWorkspaceChangeArtifacts,
    ) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let (authority, ()) = locked.authenticate(Ok(()))?;
    let (authority, prepared) = prepare_project_candidate_change(authority, derive)?;
    let result =
        artifact::render_artifacts(&prepared).and_then(|artifacts| review(&prepared, &artifacts));
    authority.finish(result)
}

/// The bridge contributes an exact submitted Change-v1 evidence document plus
/// its outer receipt. Existing evidence replay and the sole Workspace publisher
/// remain mandatory; no reusable authority leaves this invocation.
pub(crate) fn apply_project_candidate_change(
    root: &Path,
    derive: impl FnOnce(
        &str,
        &[workspace::WorkspaceSemanticSource],
    ) -> Result<SemanticWorkspaceChangeSet, Vec<Diagnostic>>,
    verify: impl FnOnce(
        &SemanticWorkspacePreparedChange,
        &SemanticWorkspaceChangeArtifacts,
    ) -> Result<(String, String), Vec<Diagnostic>>,
    hook: impl FnMut(
        workspace::SemanticChangeApplyPoint,
        &Path,
        Option<&Path>,
        Option<&Path>,
    ) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_apply_lock(root)?;
    let (authority, ()) = locked.authenticate(Ok(()))?;
    let (authority, prepared) = prepare_project_candidate_change(authority, derive)?;
    let replayed = (|| {
        let artifacts = artifact::render_artifacts(&prepared)?;
        let (submitted_source, receipt) = verify(&prepared, &artifacts)?;
        let submitted = verification::parse_evidence(&submitted_source)?;
        verification::verify_replay(&submitted, &submitted_source, &artifacts)?;
        Ok(receipt)
    })();
    let receipt = match replayed {
        Ok(receipt) => receipt,
        Err(diagnostics) => return authority.finish(Err(diagnostics)),
    };
    let (candidate_files, candidate_manifest, candidate_revision) =
        prepared.into_candidate_generation_parts();
    workspace::commit_semantic_change_authority_with_hook(
        SemanticWorkspaceChangeCommitAuthority {
            authority,
            candidate_files,
            candidate_manifest,
            candidate_revision,
            receipt,
        },
        hook,
    )
}

fn prepare_project_candidate_change(
    mut authority: workspace::WorkspaceSemanticReadAuthority,
    derive: impl FnOnce(
        &str,
        &[workspace::WorkspaceSemanticSource],
    ) -> Result<SemanticWorkspaceChangeSet, Vec<Diagnostic>>,
) -> Result<
    (
        workspace::WorkspaceSemanticReadAuthority,
        SemanticWorkspacePreparedChange,
    ),
    Vec<Diagnostic>,
> {
    let base_revision = authority.workspace_revision().to_owned();
    let storage = (
        authority.manifest_bytes(),
        authority.retained_generations(),
        authority.staging_attempts(),
    );
    let result = (|| {
        let graph = authority.take_graph()?;
        let sources = authority.take_sources();
        // The source inventory remains authenticated by the held authority and
        // is compared with the independently admitted Project base by derive.
        let changes = derive(&base_revision, &sources)?;
        prepare_owned(base_revision, sources, graph, storage, changes)
    })();
    match result {
        Ok(prepared) => Ok((authority, prepared)),
        Err(diagnostics) => match authority.finish::<()>(Err(diagnostics)) {
            Err(diagnostics) => Err(diagnostics),
            Ok(()) => Err(replay(
                "Project publication preparation failed without diagnostics",
            )),
        },
    }
}

fn proposal_io_hook(label: &'static str, error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I211", format!("{label}: {error}"))]
}

pub(crate) const SCHEMA: &str = "semaprax.workspace-semantic-change.v1";
const MIN_CHANGED_FILES: usize = 2;
const MAX_CHANGED_FILES: usize = semantic_workspace::MAX_MANAGED_FILES;
const MAX_SOURCE_BYTES_PER_CHANGE: usize = 1024 * 1024;
const MAX_TOTAL_REPLACEMENT_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRY_MODULE_BYTES: usize = 16 * 1024 * 1024;
const MAX_PROPOSAL_BYTES: usize = 32 * 1024 * 1024;
const MAX_DELTA_ROOTS: usize = 8192;
const MAX_DELTA_EDGES: usize = 131_072;
const MAX_IMPACT_NODES: usize = 16_384;
const MAX_IMPACT_PROVENANCE: usize = 65_536;
const MAX_IMPACT_DEPTH: usize = 1024;
const EDGE_FAMILY_ORDER: [&str; 6] = [
    "function_import",
    "type_import",
    "call",
    "type_reference",
    "effect_requirement",
    "capability_authority",
];

pub(crate) struct SemanticWorkspaceChangeFile {
    path: String,
    base_source_graph_schema: String,
    base_source_revision: String,
    base_source_digest: String,
    source: String,
}

pub(crate) struct SemanticWorkspaceChangeSet {
    base_workspace_revision: String,
    entry_module: String,
    files: Vec<SemanticWorkspaceChangeFile>,
    proposal_source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceChangedFileFact {
    path: String,
    base_source_graph_schema: String,
    candidate_source_graph_schema: String,
    base_source_revision: String,
    candidate_source_revision: String,
    base_source_digest: String,
    candidate_source_digest: String,
    base_bytes: usize,
    candidate_bytes: usize,
}

pub(crate) struct SemanticWorkspacePreparedChange {
    base_workspace_revision: String,
    candidate_workspace_revision: String,
    entry_module: String,
    proposal_source: String,
    base_workspace_graph_digest: String,
    candidate_workspace_graph_digest: String,
    base_files: Vec<SemanticWorkspaceBaseFileFact>,
    changed_files: Vec<SemanticWorkspaceChangedFileFact>,
    #[allow(
        dead_code,
        reason = "retained for private exact replay and later held verification"
    )]
    base_graph: workspace_graph::WorkspaceGraphChangeView,
    #[allow(
        dead_code,
        reason = "retained for private exact replay and later held verification"
    )]
    candidate_graph: workspace_graph::WorkspaceGraphChangeView,
    candidate_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    candidate_manifest: String,
    roots: Vec<SemanticWorkspaceChangeRoot>,
    delta_edges: Vec<SemanticWorkspaceChangeEdge>,
    context_nodes: Vec<SemanticWorkspaceChangeContextNode>,
    impact: Vec<SemanticWorkspaceChangeImpactFact>,
    impact_edges: Vec<SemanticWorkspaceChangeImpactEdge>,
    used_builder_bytes: usize,
    used_total_replacement_source_bytes: usize,
    retained_generations: usize,
    staging_attempts: usize,
}

pub(crate) struct SemanticWorkspaceChangeCommitAuthority {
    authority: workspace::WorkspaceSemanticReadAuthority,
    candidate_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    candidate_manifest: String,
    candidate_revision: String,
    receipt: String,
}

impl SemanticWorkspaceChangeCommitAuthority {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceChangeRoot {
    state: &'static str,
    kind: &'static str,
    id: String,
    path: Option<String>,
    module: Option<String>,
    change: &'static str,
    identity_origin: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceChangeEdge {
    state: &'static str,
    change: &'static str,
    edge: workspace_graph::WorkspaceEdge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceChangeContextNode {
    state: &'static str,
    kind: &'static str,
    declaration_kind: Option<&'static str>,
    identity_origin: Option<&'static str>,
    id: String,
    path: Option<String>,
    module: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceChangeImpactFact {
    state: &'static str,
    kind: &'static str,
    declaration_kind: Option<&'static str>,
    identity_origin: Option<&'static str>,
    id: String,
    path: Option<String>,
    module: Option<String>,
    minimum_depth: usize,
    impact_role: &'static str,
    reasons: Vec<&'static str>,
    root_provenance: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWorkspaceChangeImpactEdge {
    state: &'static str,
    edge: workspace_graph::WorkspaceEdge,
}

#[allow(dead_code, reason = "private typed artifact and exact-fact seam")]
impl SemanticWorkspaceChangeRoot {
    pub(crate) const fn state(&self) -> &'static str {
        self.state
    }

    pub(crate) const fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub(crate) fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }

    pub(crate) const fn change(&self) -> &'static str {
        self.change
    }

    pub(crate) const fn identity_origin(&self) -> Option<&'static str> {
        self.identity_origin
    }
}

#[allow(dead_code, reason = "private typed artifact and exact-fact seam")]
impl SemanticWorkspaceChangeEdge {
    pub(crate) const fn state(&self) -> &'static str {
        self.state
    }

    pub(crate) const fn change(&self) -> &'static str {
        self.change
    }

    pub(crate) fn edge(&self) -> &workspace_graph::WorkspaceEdge {
        &self.edge
    }
}

#[allow(dead_code, reason = "private typed artifact and exact-fact seam")]
impl SemanticWorkspaceChangeContextNode {
    pub(crate) const fn state(&self) -> &'static str {
        self.state
    }

    pub(crate) const fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) const fn declaration_kind(&self) -> Option<&'static str> {
        self.declaration_kind
    }

    pub(crate) const fn identity_origin(&self) -> Option<&'static str> {
        self.identity_origin
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub(crate) fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }
}

#[allow(dead_code, reason = "private typed artifact and exact-fact seam")]
impl SemanticWorkspaceChangeImpactFact {
    pub(crate) const fn state(&self) -> &'static str {
        self.state
    }

    pub(crate) const fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) const fn declaration_kind(&self) -> Option<&'static str> {
        self.declaration_kind
    }

    pub(crate) const fn identity_origin(&self) -> Option<&'static str> {
        self.identity_origin
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub(crate) fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }

    pub(crate) const fn minimum_depth(&self) -> usize {
        self.minimum_depth
    }

    pub(crate) const fn impact_role(&self) -> &'static str {
        self.impact_role
    }

    pub(crate) fn reasons(&self) -> &[&'static str] {
        &self.reasons
    }

    pub(crate) fn root_provenance(&self) -> &[usize] {
        &self.root_provenance
    }
}

#[allow(dead_code, reason = "private typed artifact and exact-fact seam")]
impl SemanticWorkspaceChangeImpactEdge {
    pub(crate) const fn state(&self) -> &'static str {
        self.state
    }

    pub(crate) fn edge(&self) -> &workspace_graph::WorkspaceEdge {
        &self.edge
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ChangeNodeKey {
    kind: &'static str,
    id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticWorkspaceBaseFileFact {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
}

impl SemanticWorkspaceChangeFile {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn replacement_source(&self) -> &str {
        &self.source
    }

    pub(crate) fn base_source_graph_schema(&self) -> &str {
        &self.base_source_graph_schema
    }

    pub(crate) fn base_source_revision(&self) -> &str {
        &self.base_source_revision
    }

    pub(crate) fn base_source_digest(&self) -> &str {
        &self.base_source_digest
    }

    pub(crate) fn new(
        path: String,
        base_source_graph_schema: String,
        base_source_revision: String,
        base_source_digest: String,
        source: String,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !matches!(
            base_source_graph_schema.as_str(),
            "semaprax.graph.v10"
                | "semaprax.graph.v11"
                | "semaprax.graph.v12"
                | "semaprax.graph.v13"
                | "semaprax.graph.v14"
        ) {
            return Err(grammar(
                "Semantic Workspace change base source Graph schema is unsupported",
            ));
        }
        validate_digest(&base_source_revision, "base source revision")?;
        validate_digest(&base_source_digest, "base source digest")?;
        Ok(Self {
            path,
            base_source_graph_schema,
            base_source_revision,
            base_source_digest,
            source,
        })
    }
}

impl SemanticWorkspaceChangeSet {
    pub(crate) fn new(
        base_workspace_revision: String,
        entry_module: String,
        files: Vec<SemanticWorkspaceChangeFile>,
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_revision(&base_workspace_revision)?;
        validate_entry_module(&entry_module)?;
        if files.len() < MIN_CHANGED_FILES {
            return Err(grammar(
                "Semantic Workspace Change requires 2..16 changed files",
            ));
        }
        if files.len() > MAX_CHANGED_FILES {
            return Err(limit("changed_files", MAX_CHANGED_FILES));
        }
        let mut total_bytes = 0usize;
        for file in &files {
            if file.source.len() > MAX_SOURCE_BYTES_PER_CHANGE {
                return Err(limit(
                    "source_bytes_per_change",
                    MAX_SOURCE_BYTES_PER_CHANGE,
                ));
            }
            total_bytes = total_bytes.checked_add(file.source.len()).ok_or_else(|| {
                limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                )
            })?;
            if total_bytes > MAX_TOTAL_REPLACEMENT_SOURCE_BYTES {
                return Err(limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                ));
            }
            if !workspace::evidence_path_is_valid(&file.path) {
                return Err(grammar(
                    "Semantic Workspace changed path is outside the managed path domain",
                ));
            }
        }
        if files.windows(2).any(|pair| pair[0].path >= pair[1].path) {
            return Err(grammar(
                "Semantic Workspace changed paths must be strictly sorted and unique",
            ));
        }
        let mut change_set = Self {
            base_workspace_revision,
            entry_module,
            files,
            proposal_source: String::new(),
        };
        change_set.proposal_source = render_proposal(&change_set)?;
        Ok(change_set)
    }

    pub(crate) fn source(&self) -> &str {
        &self.proposal_source
    }

    pub(crate) fn entry_module(&self) -> &str {
        &self.entry_module
    }

    pub(crate) fn base_workspace_revision(&self) -> &str {
        &self.base_workspace_revision
    }

    pub(crate) fn files(&self) -> &[SemanticWorkspaceChangeFile] {
        &self.files
    }

    pub(crate) fn changed_file_count(&self) -> usize {
        self.files.len()
    }

    pub(crate) fn total_replacement_source_bytes(&self) -> Option<usize> {
        self.files
            .iter()
            .try_fold(0usize, |total, file| total.checked_add(file.source.len()))
    }
}

pub(crate) fn parse_proposal(source: &str) -> Result<SemanticWorkspaceChangeSet, Vec<Diagnostic>> {
    if source.len() > MAX_PROPOSAL_BYTES {
        return Err(limit("proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    let body = canonical_body(source)?;
    validate_json_depth(body)?;
    let value: Value = serde_json::from_str(body)
        .map_err(|_| grammar("Semantic Workspace Change proposal JSON is not canonical"))?;
    let object = exact_object(
        &value,
        &[
            "schema",
            "base_workspace_revision",
            "entry_module",
            "changes",
        ],
    )?;
    if text(object, "schema")? != SCHEMA {
        return Err(grammar(
            "Semantic Workspace Change proposal schema is unsupported",
        ));
    }
    let changes = array(object, "changes")?;
    if changes.len() < MIN_CHANGED_FILES {
        return Err(grammar(
            "Semantic Workspace Change requires 2..16 changed files",
        ));
    }
    if changes.len() > MAX_CHANGED_FILES {
        return Err(limit("changed_files", MAX_CHANGED_FILES));
    }
    let mut files = Vec::with_capacity(changes.len());
    let mut total_replacement_bytes = 0usize;
    for value in changes {
        let change = exact_object(
            value,
            &[
                "path",
                "base_source_graph_schema",
                "base_source_revision",
                "base_source_digest",
                "replacement_source",
            ],
        )?;
        let replacement_source = text(change, "replacement_source")?;
        if replacement_source.len() > MAX_SOURCE_BYTES_PER_CHANGE {
            return Err(limit(
                "source_bytes_per_change",
                MAX_SOURCE_BYTES_PER_CHANGE,
            ));
        }
        total_replacement_bytes = total_replacement_bytes
            .checked_add(replacement_source.len())
            .ok_or_else(|| {
                limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                )
            })?;
        if total_replacement_bytes > MAX_TOTAL_REPLACEMENT_SOURCE_BYTES {
            return Err(limit(
                "total_replacement_source_bytes",
                MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
            ));
        }
        files.push(SemanticWorkspaceChangeFile::new(
            text(change, "path")?.to_owned(),
            text(change, "base_source_graph_schema")?.to_owned(),
            text(change, "base_source_revision")?.to_owned(),
            text(change, "base_source_digest")?.to_owned(),
            replacement_source.to_owned(),
        )?);
    }
    let change_set = SemanticWorkspaceChangeSet::new(
        text(object, "base_workspace_revision")?.to_owned(),
        text(object, "entry_module")?.to_owned(),
        files,
    )?;
    if change_set.source() != source {
        return Err(grammar(
            "Semantic Workspace Change proposal is not canonical semaprax.workspace-semantic-change.v1",
        ));
    }
    Ok(change_set)
}

fn render_proposal(change_set: &SemanticWorkspaceChangeSet) -> Result<String, Vec<Diagnostic>> {
    let (output, overflowed) = crate::bounded_output::with_limit(MAX_PROPOSAL_BYTES, || {
        let mut output = CappedString::new();
        output.push_str("{\"schema\":");
        push_json(&mut output, SCHEMA);
        output.push_str(",\"base_workspace_revision\":");
        push_json(&mut output, &change_set.base_workspace_revision);
        output.push_str(",\"entry_module\":");
        push_json(&mut output, &change_set.entry_module);
        output.push_str(",\"changes\":[");
        for (index, file) in change_set.files.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str("{\"path\":");
            push_json(&mut output, &file.path);
            output.push_str(",\"base_source_graph_schema\":");
            push_json(&mut output, &file.base_source_graph_schema);
            output.push_str(",\"base_source_revision\":");
            push_json(&mut output, &file.base_source_revision);
            output.push_str(",\"base_source_digest\":");
            push_json(&mut output, &file.base_source_digest);
            output.push_str(",\"replacement_source\":");
            push_json(&mut output, &file.source);
            output.push('}');
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

#[cfg(test)]
#[allow(
    dead_code,
    reason = "private fact getters support focused replay tests"
)]
impl SemanticWorkspaceChangedFileFact {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn base_source_graph_schema(&self) -> &str {
        &self.base_source_graph_schema
    }

    pub(crate) fn candidate_source_graph_schema(&self) -> &str {
        &self.candidate_source_graph_schema
    }

    pub(crate) fn base_source_revision(&self) -> &str {
        &self.base_source_revision
    }

    pub(crate) fn candidate_source_revision(&self) -> &str {
        &self.candidate_source_revision
    }

    pub(crate) fn base_source_digest(&self) -> &str {
        &self.base_source_digest
    }

    pub(crate) fn candidate_source_digest(&self) -> &str {
        &self.candidate_source_digest
    }

    pub(crate) const fn base_bytes(&self) -> usize {
        self.base_bytes
    }

    pub(crate) const fn candidate_bytes(&self) -> usize {
        self.candidate_bytes
    }
}

#[allow(
    dead_code,
    reason = "private typed fact accessors support exact replay tests"
)]
impl SemanticWorkspacePreparedChange {
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

    pub(crate) fn base_workspace_graph_digest(&self) -> &str {
        &self.base_workspace_graph_digest
    }

    pub(crate) fn candidate_workspace_graph_digest(&self) -> &str {
        &self.candidate_workspace_graph_digest
    }

    pub(crate) fn changed_files(&self) -> &[SemanticWorkspaceChangedFileFact] {
        &self.changed_files
    }

    pub(crate) fn candidate_manifest(&self) -> &str {
        &self.candidate_manifest
    }

    pub(crate) fn base_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.base_graph
    }

    pub(crate) fn candidate_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.candidate_graph
    }

    pub(crate) fn roots(&self) -> &[SemanticWorkspaceChangeRoot] {
        &self.roots
    }

    pub(crate) fn delta_edges(&self) -> &[SemanticWorkspaceChangeEdge] {
        &self.delta_edges
    }

    pub(crate) fn context_nodes(&self) -> &[SemanticWorkspaceChangeContextNode] {
        &self.context_nodes
    }

    pub(crate) fn impact(&self) -> &[SemanticWorkspaceChangeImpactFact] {
        &self.impact
    }

    pub(crate) fn impact_edges(&self) -> &[SemanticWorkspaceChangeImpactEdge] {
        &self.impact_edges
    }

    pub(crate) const fn used_builder_bytes(&self) -> usize {
        self.used_builder_bytes
    }

    pub(crate) const fn used_total_replacement_source_bytes(&self) -> usize {
        self.used_total_replacement_source_bytes
    }

    pub(crate) const fn retained_generations(&self) -> usize {
        self.retained_generations
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

    pub(crate) fn into_operations_commit_parts(
        self,
    ) -> (
        Vec<semantic_workspace::SemanticWorkspaceFileFact>,
        String,
        String,
    ) {
        self.into_candidate_generation_parts()
    }
}

#[cfg(test)]
pub(crate) fn with_authenticated_change<T>(
    root: &Path,
    change_set: SemanticWorkspaceChangeSet,
    operation: impl FnOnce(SemanticWorkspacePreparedChange) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let authority = workspace::acquire_semantic_change_read(root).map_err(|diagnostics| {
        map_change_builder_limit(diagnostics, semantic_workspace::MAX_CHANGE_BUILDER_BYTES)
    })?;
    with_authenticated_change_authority(authority, change_set, operation)
}

fn with_authenticated_change_authority<T>(
    authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceChangeSet,
    operation: impl FnOnce(SemanticWorkspacePreparedChange) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let (authority, prepared) = prepare_authenticated_change_authority(authority, change_set)?;
    let result = operation(prepared);
    authority.finish(result)
}

fn prepare_authenticated_change_authority(
    mut authority: workspace::WorkspaceSemanticReadAuthority,
    change_set: SemanticWorkspaceChangeSet,
) -> Result<
    (
        workspace::WorkspaceSemanticReadAuthority,
        SemanticWorkspacePreparedChange,
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
            Ok(()) => Err(replay(
                "Semantic Workspace Change failed preparation without diagnostics",
            )),
        },
    }
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "test-only seam proves consumed graph failures synchronously unlock"
)]
fn build_with_consumed_graph_for_test(
    root: &Path,
    change_set: SemanticWorkspaceChangeSet,
) -> Result<(), Vec<Diagnostic>> {
    let mut authority = workspace::acquire_semantic_change_read(root)?;
    let _consumed = authority.take_graph()?;
    with_authenticated_change_authority(authority, change_set, |_| Ok(()))
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
        "SPX-I214",
        format!("could not read Semantic Workspace Change proposal: {detail}"),
    )]
}

#[cfg(test)]
pub(crate) fn build_authenticated_change(
    root: &Path,
    change_set: SemanticWorkspaceChangeSet,
) -> Result<SemanticWorkspacePreparedChange, Vec<Diagnostic>> {
    with_authenticated_change(root, change_set, Ok)
}

fn prepare_owned(
    authenticated_revision: String,
    sources: Vec<workspace::WorkspaceSemanticSource>,
    base_graph: workspace_graph::WorkspaceGraphBuild,
    storage: (usize, usize, usize),
    change_set: SemanticWorkspaceChangeSet,
) -> Result<SemanticWorkspacePreparedChange, Vec<Diagnostic>> {
    if authenticated_revision != change_set.base_workspace_revision {
        return Err(stale("Semantic Workspace Change base revision is stale"));
    }
    if sources.len() < 2 || sources.len() > semantic_workspace::MAX_MANAGED_FILES {
        return Err(replay(
            "Semantic Workspace authenticated source cardinality disagrees",
        ));
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
        return Err(stale(
            "Semantic Workspace change contains an unmanaged path",
        ));
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
                    return Err(stale(
                        "Semantic Workspace change base source binding is stale",
                    ));
                }
                if change.source == source.source {
                    return Err(replay(
                        "Semantic Workspace change set contains an unchanged file",
                    ));
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
        return Err(replay(
            "Semantic Workspace declared change overlay was not consumed exactly once",
        ));
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
    let candidate = semantic_workspace::preflight_owned_for_change(
        &path_set,
        candidate_sources,
        candidate_builder_limit,
    )
    .map_err(|diagnostics| map_change_builder_limit(diagnostics, candidate_builder_limit))?;
    if candidate.path_set() != paths || candidate.files().len() != base_files.len() {
        return Err(replay(
            "Semantic Workspace candidate path inventory disagrees with its authenticated base",
        ));
    }
    if !base_graph.contains_module(&change_set.entry_module)
        || !candidate.graph().contains_module(&change_set.entry_module)
    {
        return Err(stale(
            "Semantic Workspace Change entry module is absent from the managed workspace",
        ));
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
            return Err(replay(
                "Semantic Workspace changed-file inventory disagrees with candidate facts",
            ));
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
        return Err(replay(
            "Semantic Workspace changed-file cardinality disagrees with its declaration",
        ));
    }
    let candidate_workspace_revision = candidate.workspace_revision().to_owned();
    if candidate_workspace_revision == authenticated_revision {
        return Err(replay(
            "Semantic Workspace changed candidate retained the base revision",
        ));
    }
    let (candidate_files, candidate_manifest, replayed_candidate_revision, candidate_graph) =
        candidate.into_snapshot_parts();
    if replayed_candidate_revision != candidate_workspace_revision {
        return Err(replay(
            "Semantic Workspace candidate revision changed while retaining its typed graph",
        ));
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
    let base_graph = base_graph
        .into_change_view()
        .map_err(|_| replay("Semantic Workspace Change base declaration replay disagrees"))?;
    let candidate_graph = candidate_graph
        .into_change_view()
        .map_err(|_| replay("Semantic Workspace Change candidate declaration replay disagrees"))?;
    let prebound = delta_builder_prebound(&base_graph, &candidate_graph)?;
    let (delta, overflowed, delta_builder_bytes) =
        crate::bounded_output::with_limit_usage(remaining_delta_builder, || {
            if !crate::bounded_output::reserve_active(prebound) {
                return Err(limit(
                    "total_analysis_builder_bytes",
                    semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
                ));
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
        });
    if overflowed {
        return Err(limit(
            "total_analysis_builder_bytes",
            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
        ));
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

/// Builds the unchanged Change-v1 typed artifact input from a single
/// Operations-authenticated base/candidate pair. This bridge performs no
/// parsing, resolution, or candidate construction.
pub(crate) struct OperationsChangeBridge {
    pub(crate) change_set: SemanticWorkspaceChangeSet,
    pub(crate) authenticated_revision: String,
    pub(crate) base_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    pub(crate) candidate_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    pub(crate) candidate_manifest: String,
    pub(crate) candidate_workspace_revision: String,
    pub(crate) base_graph: workspace_graph::WorkspaceGraphChangeView,
    pub(crate) candidate_graph: workspace_graph::WorkspaceGraphChangeView,
    pub(crate) base_builder_bytes: usize,
    pub(crate) candidate_builder_bytes: usize,
    pub(crate) storage: (usize, usize, usize),
}

pub(crate) fn prepare_from_operations_facts(
    bridge: OperationsChangeBridge,
) -> Result<SemanticWorkspacePreparedChange, Vec<Diagnostic>> {
    let OperationsChangeBridge {
        change_set,
        authenticated_revision,
        base_files,
        candidate_files,
        candidate_manifest,
        candidate_workspace_revision,
        base_graph,
        candidate_graph,
        base_builder_bytes,
        candidate_builder_bytes,
        storage,
    } = bridge;
    if change_set.base_workspace_revision != authenticated_revision
        || candidate_workspace_revision == authenticated_revision
        || semantic_workspace::semantic_workspace_revision(&candidate_manifest)
            != candidate_workspace_revision
        || base_files.len() != candidate_files.len()
        || !base_graph
            .modules()
            .iter()
            .any(|module| module.module() == change_set.entry_module)
        || !candidate_graph
            .modules()
            .iter()
            .any(|module| module.module() == change_set.entry_module)
    {
        return Err(replay(
            "Semantic Workspace Operations typed Change bridge disagrees with authenticated facts",
        ));
    }
    let bridge_limit = semantic_workspace::MAX_CHANGE_BUILDER_BYTES
        .checked_sub(base_builder_bytes)
        .and_then(|remaining| remaining.checked_sub(candidate_builder_bytes))
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
    let bridge_strings = base_files
        .iter()
        .try_fold(0usize, |total, file| {
            total
                .checked_add(file.path().len())
                .and_then(|value| value.checked_add(file.source_graph_schema().len()))
                .and_then(|value| value.checked_add(file.source_revision().len()))
                .and_then(|value| value.checked_add(file.source_digest().len()))
        })
        .and_then(|value| {
            change_set.files.iter().try_fold(value, |total, file| {
                total
                    .checked_add(file.path.len())
                    .and_then(|next| next.checked_add(file.base_source_graph_schema.len()))
                    .and_then(|next| next.checked_add(file.base_source_revision.len()))
                    .and_then(|next| next.checked_add(file.base_source_digest.len()))
            })
        })
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
    let bridge_structures = base_files
        .len()
        .checked_mul(
            std::mem::size_of::<SemanticWorkspaceBaseFileFact>()
                + std::mem::size_of::<(&str, &semantic_workspace::SemanticWorkspaceFileFact)>() * 2
                + std::mem::size_of::<String>(),
        )
        .and_then(|value| {
            value.checked_add(
                change_set
                    .files
                    .len()
                    .checked_mul(std::mem::size_of::<SemanticWorkspaceChangedFileFact>())?,
            )
        })
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
    let bridge_prebound = bridge_strings
        .checked_add(bridge_structures)
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
    let (_, bridge_overflowed, bridge_builder_bytes) =
        crate::bounded_output::with_limit_usage(bridge_limit, || {
            crate::bounded_output::reserve_active(bridge_prebound)
        });
    if bridge_overflowed || bridge_builder_bytes != bridge_prebound {
        return Err(limit(
            "total_analysis_builder_bytes",
            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
        ));
    }
    for file in &change_set.files {
        let base = base_files
            .iter()
            .find(|fact| fact.path() == file.path)
            .ok_or_else(|| {
                replay("Semantic Workspace Operations Change row has no authenticated base fact")
            })?;
        let candidate = candidate_files
            .iter()
            .find(|fact| fact.path() == file.path)
            .ok_or_else(|| {
                replay("Semantic Workspace Operations Change row has no candidate fact")
            })?;
        if file.base_source_graph_schema != base.source_graph_schema()
            || file.base_source_revision != base.source_revision()
            || file.base_source_digest != base.source_digest()
            || file.source != candidate.source()
        {
            return Err(replay(
                "Semantic Workspace Operations derived Change row disagrees with authenticated facts",
            ));
        }
    }
    let declared_paths = change_set
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let base_by_path = base_files
        .iter()
        .map(|file| (file.path(), file))
        .collect::<BTreeMap<_, _>>();
    let candidate_by_path = candidate_files
        .iter()
        .map(|file| (file.path(), file))
        .collect::<BTreeMap<_, _>>();
    if base_by_path.len() != base_files.len()
        || candidate_by_path.len() != candidate_files.len()
        || base_by_path.keys().ne(candidate_by_path.keys())
        || !declared_paths
            .iter()
            .all(|path| base_by_path.contains_key(path.as_str()))
    {
        return Err(replay(
            "Semantic Workspace Operations typed Change path inventory disagrees",
        ));
    }
    let mut base_file_facts = Vec::with_capacity(base_files.len());
    let mut changed_files = Vec::with_capacity(declared_paths.len());
    for base in &base_files {
        let candidate = candidate_by_path.get(base.path()).ok_or_else(|| {
            replay("Semantic Workspace Operations candidate path is absent from Change bridge")
        })?;
        let changed = declared_paths.contains(base.path());
        let facts_differ = base.source_graph_schema() != candidate.source_graph_schema()
            || base.source_revision() != candidate.source_revision()
            || base.source_digest() != candidate.source_digest()
            || base.bytes() != candidate.bytes()
            || base.source() != candidate.source();
        if changed != facts_differ {
            return Err(replay(
                "Semantic Workspace Operations changed-file inventory disagrees with Change bridge",
            ));
        }
        base_file_facts.push(SemanticWorkspaceBaseFileFact {
            path: base.path().to_owned(),
            source_graph_schema: base.source_graph_schema().to_owned(),
            source_revision: base.source_revision().to_owned(),
            source_digest: base.source_digest().to_owned(),
            bytes: base.bytes(),
        });
        if changed {
            changed_files.push(SemanticWorkspaceChangedFileFact {
                path: base.path().to_owned(),
                base_source_graph_schema: base.source_graph_schema().to_owned(),
                candidate_source_graph_schema: candidate.source_graph_schema().to_owned(),
                base_source_revision: base.source_revision().to_owned(),
                candidate_source_revision: candidate.source_revision().to_owned(),
                base_source_digest: base.source_digest().to_owned(),
                candidate_source_digest: candidate.source_digest().to_owned(),
                base_bytes: base.bytes(),
                candidate_bytes: candidate.bytes(),
            });
        }
    }
    if changed_files.len() != declared_paths.len() {
        return Err(replay(
            "Semantic Workspace Operations changed-file cardinality disagrees with Change bridge",
        ));
    }
    let remaining_delta_builder =
        bridge_limit
            .checked_sub(bridge_builder_bytes)
            .ok_or_else(|| {
                limit(
                    "total_analysis_builder_bytes",
                    semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
                )
            })?;
    let prebound = delta_builder_prebound(&base_graph, &candidate_graph)?;
    let (delta, overflowed, delta_builder_bytes) =
        crate::bounded_output::with_limit_usage(remaining_delta_builder, || {
            if !crate::bounded_output::reserve_active(prebound) {
                return Err(limit(
                    "total_analysis_builder_bytes",
                    semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
                ));
            }
            let base_sources = base_files
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
        });
    if overflowed {
        return Err(limit(
            "total_analysis_builder_bytes",
            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
        ));
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
        .and_then(|used| used.checked_add(bridge_builder_bytes))
        .and_then(|used| used.checked_add(delta_builder_bytes))
        .ok_or_else(|| {
            limit(
                "total_analysis_builder_bytes",
                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
            )
        })?;
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
    Ok(SemanticWorkspacePreparedChange {
        base_workspace_revision: authenticated_revision,
        candidate_workspace_revision,
        entry_module: change_set.entry_module,
        proposal_source: change_set.proposal_source,
        base_workspace_graph_digest,
        candidate_workspace_graph_digest,
        base_files: base_file_facts,
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

pub(crate) fn build_delta(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
    changed_paths: &BTreeSet<String>,
) -> Result<
    (
        Vec<SemanticWorkspaceChangeRoot>,
        Vec<SemanticWorkspaceChangeEdge>,
    ),
    Vec<Diagnostic>,
> {
    build_delta_inner(base, candidate, changed_paths, false)
}

pub(crate) fn build_structural_delta(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
    changed_paths: &BTreeSet<String>,
) -> Result<
    (
        Vec<SemanticWorkspaceChangeRoot>,
        Vec<SemanticWorkspaceChangeEdge>,
    ),
    Vec<Diagnostic>,
> {
    build_delta_inner(base, candidate, changed_paths, true)
}

fn build_delta_inner(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
    changed_paths: &BTreeSet<String>,
    allow_derived_import_change: bool,
) -> Result<
    (
        Vec<SemanticWorkspaceChangeRoot>,
        Vec<SemanticWorkspaceChangeEdge>,
    ),
    Vec<Diagnostic>,
> {
    let base_edges = exact_edge_set(base.edges())?;
    let candidate_edges = exact_edge_set(candidate.edges())?;
    let mut delta_edges = Vec::new();
    for edge in base_edges.difference(&candidate_edges) {
        push_delta_edge(&mut delta_edges, "base", "removed", (**edge).clone())?;
    }
    for edge in candidate_edges.difference(&base_edges) {
        push_delta_edge(&mut delta_edges, "candidate", "added", (**edge).clone())?;
    }
    delta_edges.sort_by(|left, right| {
        (left.state, left.change, &left.edge).cmp(&(right.state, right.change, &right.edge))
    });

    let mut roots = Vec::new();
    declaration_roots(base, candidate, changed_paths, &mut roots)?;
    module_roots(
        base,
        candidate,
        changed_paths,
        &base_edges,
        &candidate_edges,
        &mut roots,
        allow_derived_import_change,
    )?;
    capability_roots(&base_edges, &candidate_edges, &mut roots)?;
    roots.sort_by(|left, right| root_key(left).cmp(&root_key(right)));
    if roots.len() > MAX_DELTA_ROOTS {
        return Err(limit("delta_roots", MAX_DELTA_ROOTS));
    }
    if roots.is_empty()
        || delta_edges.is_empty() && !roots.iter().any(|root| root.kind == "declaration")
    {
        return Err(replay(
            "Semantic Workspace Change typed delta is empty or disagrees with the candidate",
        ));
    }
    Ok((roots, delta_edges))
}

pub(crate) fn delta_builder_prebound(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
) -> Result<usize, Vec<Diagnostic>> {
    let mut bytes = 0usize;
    for graph in [base, candidate] {
        bytes = checked_builder_add(
            bytes,
            graph
                .edges()
                .len()
                .checked_mul(
                    std::mem::size_of::<workspace_graph::WorkspaceEdge>()
                        + std::mem::size_of::<SemanticWorkspaceChangeEdge>()
                        + std::mem::size_of::<SemanticWorkspaceChangeImpactEdge>()
                        + 8 * std::mem::size_of::<usize>(),
                )
                .ok_or_else(builder_limit)?,
        )?;
        for edge in graph.edges() {
            let dynamic = edge
                .caller_path()
                .len()
                .checked_add(edge.caller().len())
                .and_then(|value| value.checked_add(edge.target_path().len()))
                .and_then(|value| value.checked_add(edge.target().len()))
                .and_then(|value| value.checked_add(edge.expression().len()))
                .and_then(|value| value.checked_add(edge.ast_path().len()))
                .and_then(|value| value.checked_add(edge.alias().len()))
                .and_then(|value| value.checked_mul(4))
                .ok_or_else(builder_limit)?;
            bytes = checked_builder_add(bytes, dynamic)?;
        }
        bytes = checked_builder_add(
            bytes,
            graph
                .declarations()
                .len()
                .checked_mul(
                    std::mem::size_of::<SemanticWorkspaceChangeRoot>()
                        + 8 * std::mem::size_of::<usize>(),
                )
                .ok_or_else(builder_limit)?,
        )?;
        for declaration in graph.declarations() {
            let dynamic = declaration
                .id()
                .len()
                .checked_add(declaration.path().map_or(0, str::len))
                .and_then(|value| value.checked_add(declaration.module().map_or(0, str::len)))
                .and_then(|value| value.checked_mul(4))
                .ok_or_else(builder_limit)?;
            bytes = checked_builder_add(bytes, dynamic)?;
        }
        for module in graph.modules() {
            let dynamic = module
                .path()
                .len()
                .checked_add(module.module().len())
                .and_then(|value| value.checked_mul(4))
                .ok_or_else(builder_limit)?;
            bytes = checked_builder_add(
                bytes,
                dynamic
                    .checked_add(std::mem::size_of::<SemanticWorkspaceChangeRoot>())
                    .ok_or_else(builder_limit)?,
            )?;
        }
    }
    Ok(bytes)
}

pub(crate) fn build_context_nodes(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
    roots: &[SemanticWorkspaceChangeRoot],
    delta_edges: &[SemanticWorkspaceChangeEdge],
) -> Result<Vec<SemanticWorkspaceChangeContextNode>, Vec<Diagnostic>> {
    let mut output = Vec::new();
    for (state, graph) in [("base", base), ("candidate", candidate)] {
        let modules = graph
            .modules()
            .iter()
            .map(|module| (module.module(), module))
            .collect::<BTreeMap<_, _>>();
        let declarations = graph
            .declarations()
            .iter()
            .filter(|declaration| declaration.origin() != hir::IdentityOrigin::CompilerOwned)
            .map(|declaration| (declaration.id(), declaration))
            .collect::<BTreeMap<_, _>>();
        let calls = graph
            .edges()
            .iter()
            .filter(|edge| edge.kind() == "call")
            .map(call_binding_key)
            .collect::<BTreeSet<_>>();
        let mut nodes = roots
            .iter()
            .filter(|root| root.state == state)
            .map(root_node)
            .collect::<BTreeSet<_>>();
        if nodes.len() > MAX_IMPACT_NODES {
            return Err(incomplete(
                "Semantic Workspace Change Context node closure is incomplete",
            ));
        }
        for edge in delta_edges.iter().filter(|edge| edge.state == state) {
            let (source, target) = typed_edge_nodes(&edge.edge, &modules, &declarations, &calls)?;
            nodes.insert(source);
            nodes.insert(target);
            if nodes.len() > MAX_IMPACT_NODES {
                return Err(incomplete(
                    "Semantic Workspace Change Context node closure is incomplete",
                ));
            }
        }
        if output.len().saturating_add(nodes.len()) > MAX_IMPACT_NODES {
            return Err(incomplete(
                "Semantic Workspace Change Context node closure is incomplete",
            ));
        }
        for node in nodes {
            let (declaration_kind, identity_origin, path, module) = match node.kind {
                "module" => {
                    let fact = modules.get(node.id.as_str()).ok_or_else(|| {
                        replay("Semantic Workspace Change Context module is absent")
                    })?;
                    (
                        None,
                        None,
                        Some(fact.path().to_owned()),
                        Some(fact.module().to_owned()),
                    )
                }
                "declaration" => {
                    let fact = declarations.get(node.id.as_str()).ok_or_else(|| {
                        replay("Semantic Workspace Change Context declaration is absent")
                    })?;
                    (
                        Some(declaration_kind_text(fact.kind())),
                        Some(fact.origin().text()),
                        fact.path().map(str::to_owned),
                        fact.module().map(str::to_owned),
                    )
                }
                "capability" => (None, None, None, None),
                _ => {
                    return Err(replay(
                        "Semantic Workspace Change Context node kind disagrees",
                    ));
                }
            };
            output.push(SemanticWorkspaceChangeContextNode {
                state,
                kind: node.kind,
                declaration_kind,
                identity_origin,
                id: node.id,
                path,
                module,
            });
        }
    }
    output.sort_by(|left, right| {
        (left.state, left.kind, &left.id, &left.path, &left.module).cmp(&(
            right.state,
            right.kind,
            &right.id,
            &right.path,
            &right.module,
        ))
    });
    Ok(output)
}

fn checked_builder_add(left: usize, right: usize) -> Result<usize, Vec<Diagnostic>> {
    left.checked_add(right).ok_or_else(builder_limit)
}

fn builder_limit() -> Vec<Diagnostic> {
    limit(
        "total_analysis_builder_bytes",
        semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
    )
}

pub(crate) fn build_impact(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
    roots: &[SemanticWorkspaceChangeRoot],
) -> Result<
    (
        Vec<SemanticWorkspaceChangeImpactFact>,
        Vec<SemanticWorkspaceChangeImpactEdge>,
    ),
    Vec<Diagnostic>,
> {
    let mut facts = Vec::new();
    let mut edges = Vec::new();
    let mut provenance = 0usize;
    build_state_impact("base", base, roots, &mut facts, &mut edges, &mut provenance)?;
    build_state_impact(
        "candidate",
        candidate,
        roots,
        &mut facts,
        &mut edges,
        &mut provenance,
    )?;
    facts.sort_by(|left, right| {
        (
            left.state,
            left.minimum_depth,
            left.kind,
            &left.id,
            &left.path,
            &left.module,
        )
            .cmp(&(
                right.state,
                right.minimum_depth,
                right.kind,
                &right.id,
                &right.path,
                &right.module,
            ))
    });
    edges.sort_by(|left, right| (left.state, &left.edge).cmp(&(right.state, &right.edge)));
    if facts.len() > MAX_IMPACT_NODES
        || edges.len() > MAX_DELTA_EDGES
        || provenance > MAX_IMPACT_PROVENANCE
    {
        return Err(incomplete(
            "Semantic Workspace Change impact closure is incomplete",
        ));
    }
    Ok((facts, edges))
}

fn build_state_impact(
    state: &'static str,
    graph: &workspace_graph::WorkspaceGraphChangeView,
    roots: &[SemanticWorkspaceChangeRoot],
    facts: &mut Vec<SemanticWorkspaceChangeImpactFact>,
    impact_edges: &mut Vec<SemanticWorkspaceChangeImpactEdge>,
    provenance_count: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    let modules = graph
        .modules()
        .iter()
        .map(|module| (module.module(), module))
        .collect::<BTreeMap<_, _>>();
    let declarations = graph
        .declarations()
        .iter()
        .filter(|declaration| declaration.origin() != hir::IdentityOrigin::CompilerOwned)
        .map(|declaration| (declaration.id(), declaration))
        .collect::<BTreeMap<_, _>>();
    let call_occurrences = graph
        .edges()
        .iter()
        .filter(|edge| edge.kind() == "call")
        .map(call_binding_key)
        .collect::<BTreeSet<_>>();
    let mut reverse = BTreeMap::<ChangeNodeKey, Vec<(ChangeNodeKey, usize)>>::new();
    for (index, edge) in graph.edges().iter().enumerate() {
        let (source, target) = typed_edge_nodes(edge, &modules, &declarations, &call_occurrences)?;
        reverse.entry(target).or_default().push((source, index));
    }
    for incoming in reverse.values_mut() {
        incoming.sort();
    }

    let state_roots = roots
        .iter()
        .enumerate()
        .filter(|(_, root)| root.state == state)
        .map(|(index, root)| (index, root_node(root)))
        .collect::<Vec<_>>();
    let mut distances = BTreeMap::<ChangeNodeKey, usize>::new();
    let mut queue = VecDeque::new();
    for (_, root) in &state_roots {
        if distances.insert(root.clone(), 0).is_none() {
            queue.push_back(root.clone());
        }
    }
    while let Some(target) = queue.pop_front() {
        let depth = distances[&target];
        let Some(incoming) = reverse.get(&target) else {
            continue;
        };
        if depth == MAX_IMPACT_DEPTH {
            if incoming
                .iter()
                .any(|(source, _)| !distances.contains_key(source))
            {
                return Err(incomplete(
                    "Semantic Workspace Change impact depth is incomplete",
                ));
            }
            continue;
        }
        for (source, _) in incoming {
            if !distances.contains_key(source) {
                if facts.len().saturating_add(distances.len()) == MAX_IMPACT_NODES {
                    return Err(incomplete(
                        "Semantic Workspace Change impact node closure is incomplete",
                    ));
                }
                distances.insert(source.clone(), depth + 1);
                queue.push_back(source.clone());
            }
        }
    }

    let mut root_provenance = BTreeMap::<ChangeNodeKey, BTreeSet<usize>>::new();
    let mut reasons = BTreeMap::<ChangeNodeKey, BTreeSet<&'static str>>::new();
    for (index, root) in state_roots {
        root_provenance.entry(root).or_default().insert(index);
    }
    let mut ordered = distances
        .iter()
        .map(|(node, depth)| (*depth, node.clone()))
        .collect::<Vec<_>>();
    ordered.sort();
    let mut selected_edges = BTreeSet::new();
    for (depth, target) in &ordered {
        let target_roots = root_provenance.get(target).cloned().unwrap_or_default();
        if let Some(incoming) = reverse.get(target) {
            for (source, edge_index) in incoming {
                if distances.get(source) != Some(&(depth + 1)) {
                    continue;
                }
                selected_edges.insert(*edge_index);
                reasons
                    .entry(source.clone())
                    .or_default()
                    .insert(edge_family(graph.edges()[*edge_index].kind())?);
                root_provenance
                    .entry(source.clone())
                    .or_default()
                    .extend(target_roots.iter().copied());
            }
        }
    }
    for (depth, node) in ordered {
        let node_reasons = reasons.remove(&node).unwrap_or_default();
        let provenance = root_provenance
            .remove(&node)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        *provenance_count = provenance_count
            .checked_add(provenance.len())
            .ok_or_else(|| incomplete("Semantic Workspace Change provenance is incomplete"))?;
        if *provenance_count > MAX_IMPACT_PROVENANCE {
            return Err(incomplete(
                "Semantic Workspace Change provenance is incomplete",
            ));
        }
        facts.push(materialize_impact_fact(
            state,
            node,
            depth,
            provenance,
            node_reasons,
            &modules,
            &declarations,
        )?);
    }
    for index in selected_edges {
        if impact_edges.len() == MAX_DELTA_EDGES {
            return Err(incomplete(
                "Semantic Workspace Change impact edge closure is incomplete",
            ));
        }
        impact_edges.push(SemanticWorkspaceChangeImpactEdge {
            state,
            edge: graph.edges()[index].clone(),
        });
    }
    Ok(())
}

fn typed_edge_nodes(
    edge: &workspace_graph::WorkspaceEdge,
    modules: &BTreeMap<&str, &workspace_graph::WorkspaceGraphChangeModule>,
    declarations: &BTreeMap<&str, &workspace_graph::WorkspaceGraphChangeDeclaration>,
    call_occurrences: &BTreeSet<CallBindingKey<'_>>,
) -> Result<(ChangeNodeKey, ChangeNodeKey), Vec<Diagnostic>> {
    let (source_kind, target_kind) = match edge.kind() {
        "function_import" | "type_import" => ("module", "declaration"),
        "call" | "type_reference" => ("declaration", "declaration"),
        "effect_requirement" => ("declaration", "capability"),
        "capability_authority" => ("module", "capability"),
        _ => return Err(replay("Semantic Workspace Change edge kind is unsupported")),
    };
    if source_kind == "module" {
        let Some(module) = modules.get(edge.caller()) else {
            return Err(replay(
                "Semantic Workspace Change edge module source is absent",
            ));
        };
        if module.path() != edge.caller_path() {
            return Err(replay(
                "Semantic Workspace Change edge source path disagrees",
            ));
        }
    } else {
        let Some(declaration) = declarations.get(edge.caller()) else {
            return Err(replay(
                "Semantic Workspace Change edge declaration source is absent",
            ));
        };
        if declaration.path() != Some(edge.caller_path()) {
            return Err(replay(
                "Semantic Workspace Change edge declaration source path disagrees",
            ));
        }
    }
    if target_kind == "declaration" {
        let Some(declaration) = declarations.get(edge.target()) else {
            return Err(replay(
                "Semantic Workspace Change edge declaration target is absent",
            ));
        };
        if declaration.path() != Some(edge.target_path()) {
            return Err(replay(
                "Semantic Workspace Change edge declaration target path disagrees",
            ));
        }
    } else if edge.kind() == "capability_authority" {
        if edge.target_path() != edge.caller_path() {
            return Err(replay(
                "Semantic Workspace Change capability authority path disagrees",
            ));
        }
    } else if edge.kind() == "effect_requirement"
        && !call_occurrences.contains(&call_binding_key(edge))
    {
        return Err(replay(
            "Semantic Workspace Change effect requirement has no authenticated call occurrence",
        ));
    }
    Ok((
        ChangeNodeKey {
            kind: source_kind,
            id: edge.caller().to_owned(),
        },
        ChangeNodeKey {
            kind: target_kind,
            id: edge.target().to_owned(),
        },
    ))
}

type CallBindingKey<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    usize,
);

fn call_binding_key(edge: &workspace_graph::WorkspaceEdge) -> CallBindingKey<'_> {
    (
        edge.caller_path(),
        edge.caller(),
        edge.target_path(),
        edge.site(),
        edge.expression(),
        edge.ast_path(),
        edge.alias(),
        edge.ordinal(),
    )
}

fn root_node(root: &SemanticWorkspaceChangeRoot) -> ChangeNodeKey {
    ChangeNodeKey {
        kind: root.kind,
        id: root.id.clone(),
    }
}

fn materialize_impact_fact(
    state: &'static str,
    node: ChangeNodeKey,
    minimum_depth: usize,
    root_provenance: Vec<usize>,
    reason_set: BTreeSet<&'static str>,
    modules: &BTreeMap<&str, &workspace_graph::WorkspaceGraphChangeModule>,
    declarations: &BTreeMap<&str, &workspace_graph::WorkspaceGraphChangeDeclaration>,
) -> Result<SemanticWorkspaceChangeImpactFact, Vec<Diagnostic>> {
    let (declaration_kind, identity_origin, path, module) = match node.kind {
        "module" => {
            let fact = modules
                .get(node.id.as_str())
                .ok_or_else(|| replay("Semantic Workspace Change impact module is absent"))?;
            (
                None,
                None,
                Some(fact.path().to_owned()),
                Some(fact.module().to_owned()),
            )
        }
        "declaration" => {
            let fact = declarations
                .get(node.id.as_str())
                .ok_or_else(|| replay("Semantic Workspace Change impact declaration is absent"))?;
            (
                Some(declaration_kind_text(fact.kind())),
                Some(fact.origin().text()),
                fact.path().map(str::to_owned),
                fact.module().map(str::to_owned),
            )
        }
        "capability" => (None, None, None, None),
        _ => {
            return Err(replay(
                "Semantic Workspace Change impact node kind disagrees",
            ))
        }
    };
    let reasons = EDGE_FAMILY_ORDER
        .into_iter()
        .filter(|kind| reason_set.contains(kind))
        .collect();
    Ok(SemanticWorkspaceChangeImpactFact {
        state,
        kind: node.kind,
        declaration_kind,
        identity_origin,
        id: node.id,
        path,
        module,
        minimum_depth,
        impact_role: if minimum_depth == 0 {
            "target"
        } else {
            match node.kind {
                "declaration" => "consumer",
                "module" => "module_consumer",
                "capability" => "dependency",
                _ => unreachable!(),
            }
        },
        reasons,
        root_provenance,
    })
}

fn exact_edge_set(
    edges: &[workspace_graph::WorkspaceEdge],
) -> Result<BTreeSet<&workspace_graph::WorkspaceEdge>, Vec<Diagnostic>> {
    let set = edges.iter().collect::<BTreeSet<_>>();
    if set.len() != edges.len() {
        return Err(replay(
            "Semantic Workspace Change graph edge replay contains duplicates",
        ));
    }
    Ok(set)
}

fn push_delta_edge(
    edges: &mut Vec<SemanticWorkspaceChangeEdge>,
    state: &'static str,
    change: &'static str,
    edge: workspace_graph::WorkspaceEdge,
) -> Result<(), Vec<Diagnostic>> {
    if edges.len() == MAX_DELTA_EDGES {
        return Err(limit("delta_edges", MAX_DELTA_EDGES));
    }
    edges.push(SemanticWorkspaceChangeEdge {
        state,
        change,
        edge,
    });
    Ok(())
}

fn declaration_roots(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
    changed_paths: &BTreeSet<String>,
    roots: &mut Vec<SemanticWorkspaceChangeRoot>,
) -> Result<(), Vec<Diagnostic>> {
    let base_declarations = declaration_map(base)?;
    let candidate_declarations = declaration_map(candidate)?;
    let ids = base_declarations
        .keys()
        .chain(candidate_declarations.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in ids {
        let before = base_declarations.get(&id).copied();
        let after = candidate_declarations.get(&id).copied();
        match (before, after) {
            (Some(before), Some(after))
                if before.origin() == hir::IdentityOrigin::Explicit
                    && after.origin() == hir::IdentityOrigin::Explicit =>
            {
                if declaration_tuple(before) != declaration_tuple(after) {
                    require_changed_declaration_path(before, changed_paths)?;
                    require_changed_declaration_path(after, changed_paths)?;
                    push_declaration_root(roots, "base", "modified_before", before)?;
                    push_declaration_root(roots, "candidate", "modified_after", after)?;
                }
            }
            (Some(before), Some(after)) => {
                let changed = before
                    .path()
                    .is_some_and(|path| changed_paths.contains(path))
                    || after
                        .path()
                        .is_some_and(|path| changed_paths.contains(path));
                if changed {
                    push_declaration_root(roots, "base", "removed", before)?;
                    push_declaration_root(roots, "candidate", "added", after)?;
                } else if declaration_tuple(before) != declaration_tuple(after) {
                    return Err(replay(
                        "Semantic Workspace Change declaration changed outside the replacement inventory",
                    ));
                }
            }
            (Some(before), None) => {
                require_changed_declaration_path(before, changed_paths)?;
                push_declaration_root(roots, "base", "removed", before)?;
            }
            (None, Some(after)) => {
                require_changed_declaration_path(after, changed_paths)?;
                push_declaration_root(roots, "candidate", "added", after)?;
            }
            (None, None) => unreachable!(),
        }
    }
    Ok(())
}

fn require_changed_declaration_path(
    declaration: &workspace_graph::WorkspaceGraphChangeDeclaration,
    changed_paths: &BTreeSet<String>,
) -> Result<(), Vec<Diagnostic>> {
    if declaration
        .path()
        .is_some_and(|path| changed_paths.contains(path))
    {
        Ok(())
    } else {
        Err(replay(
            "Semantic Workspace Change declaration changed outside the replacement inventory",
        ))
    }
}

fn declaration_map(
    graph: &workspace_graph::WorkspaceGraphChangeView,
) -> Result<BTreeMap<&str, &workspace_graph::WorkspaceGraphChangeDeclaration>, Vec<Diagnostic>> {
    let mut declarations = BTreeMap::new();
    for declaration in graph.declarations() {
        if declaration.origin() == hir::IdentityOrigin::CompilerOwned {
            continue;
        }
        if declarations.insert(declaration.id(), declaration).is_some() {
            return Err(replay(
                "Semantic Workspace Change declaration identity replay contains duplicates",
            ));
        }
    }
    Ok(declarations)
}

fn declaration_tuple(
    declaration: &workspace_graph::WorkspaceGraphChangeDeclaration,
) -> (
    hir::DeclarationKind,
    hir::IdentityOrigin,
    Option<&str>,
    Option<&str>,
    Option<&str>,
    &str,
) {
    (
        declaration.kind(),
        declaration.origin(),
        declaration.owner(),
        declaration.path(),
        declaration.module(),
        declaration.semantic_fingerprint(),
    )
}

fn push_declaration_root(
    roots: &mut Vec<SemanticWorkspaceChangeRoot>,
    state: &'static str,
    change: &'static str,
    declaration: &workspace_graph::WorkspaceGraphChangeDeclaration,
) -> Result<(), Vec<Diagnostic>> {
    push_root(
        roots,
        SemanticWorkspaceChangeRoot {
            state,
            kind: "declaration",
            id: declaration.id().to_owned(),
            path: declaration.path().map(str::to_owned),
            module: declaration.module().map(str::to_owned),
            change,
            identity_origin: Some(declaration.origin().text()),
        },
    )
}

fn module_roots(
    base: &workspace_graph::WorkspaceGraphChangeView,
    candidate: &workspace_graph::WorkspaceGraphChangeView,
    changed_paths: &BTreeSet<String>,
    base_edges: &BTreeSet<&workspace_graph::WorkspaceEdge>,
    candidate_edges: &BTreeSet<&workspace_graph::WorkspaceEdge>,
    roots: &mut Vec<SemanticWorkspaceChangeRoot>,
    allow_derived_import_change: bool,
) -> Result<(), Vec<Diagnostic>> {
    let base_modules = module_map(base)?;
    let candidate_modules = module_map(candidate)?;
    let paths = base_modules
        .keys()
        .chain(candidate_modules.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for path in paths {
        let before = base_modules.get(path).copied();
        let after = candidate_modules.get(path).copied();
        let before_imports = module_imports(base_edges, path);
        let after_imports = module_imports(candidate_edges, path);
        match (before, after) {
            (Some(before), Some(after)) => {
                let authored_change =
                    before.module() != after.module() || before.permits() != after.permits();
                let derived_import_change = before_imports != after_imports;
                if authored_change || derived_import_change {
                    if authored_change || !allow_derived_import_change {
                        require_changed_module_path(path, changed_paths)?;
                    }
                    push_module_root(roots, "base", "modified_before", before)?;
                    push_module_root(roots, "candidate", "modified_after", after)?;
                }
            }
            (Some(before), None) => {
                require_changed_module_path(path, changed_paths)?;
                push_module_root(roots, "base", "removed", before)?;
            }
            (None, Some(after)) => {
                require_changed_module_path(path, changed_paths)?;
                push_module_root(roots, "candidate", "added", after)?;
            }
            (None, None) => unreachable!(),
        }
    }
    Ok(())
}

fn require_changed_module_path(
    path: &str,
    changed_paths: &BTreeSet<String>,
) -> Result<(), Vec<Diagnostic>> {
    if changed_paths.contains(path) {
        Ok(())
    } else {
        Err(replay(
            "Semantic Workspace Change module changed outside the replacement inventory",
        ))
    }
}

fn module_map(
    graph: &workspace_graph::WorkspaceGraphChangeView,
) -> Result<BTreeMap<&str, &workspace_graph::WorkspaceGraphChangeModule>, Vec<Diagnostic>> {
    let mut modules = BTreeMap::new();
    for module in graph.modules() {
        if modules.insert(module.path(), module).is_some() {
            return Err(replay(
                "Semantic Workspace Change module path replay contains duplicates",
            ));
        }
    }
    Ok(modules)
}

fn module_imports<'a>(
    edges: &'a BTreeSet<&'a workspace_graph::WorkspaceEdge>,
    path: &str,
) -> BTreeSet<&'a workspace_graph::WorkspaceEdge> {
    edges
        .iter()
        .filter(|edge| {
            edge.caller_path() == path && matches!(edge.kind(), "function_import" | "type_import")
        })
        .copied()
        .collect()
}

fn push_module_root(
    roots: &mut Vec<SemanticWorkspaceChangeRoot>,
    state: &'static str,
    change: &'static str,
    module: &workspace_graph::WorkspaceGraphChangeModule,
) -> Result<(), Vec<Diagnostic>> {
    push_root(
        roots,
        SemanticWorkspaceChangeRoot {
            state,
            kind: "module",
            id: module.module().to_owned(),
            path: Some(module.path().to_owned()),
            module: Some(module.module().to_owned()),
            change,
            identity_origin: None,
        },
    )
}

fn capability_roots(
    base_edges: &BTreeSet<&workspace_graph::WorkspaceEdge>,
    candidate_edges: &BTreeSet<&workspace_graph::WorkspaceEdge>,
    roots: &mut Vec<SemanticWorkspaceChangeRoot>,
) -> Result<(), Vec<Diagnostic>> {
    let base = capability_incidence(base_edges);
    let candidate = capability_incidence(candidate_edges);
    let capabilities = base
        .keys()
        .chain(candidate.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for capability in capabilities {
        let before = base.get(capability);
        let after = candidate.get(capability);
        if before == after {
            continue;
        }
        match (before, after) {
            (Some(_), Some(_)) => {
                push_capability_root(roots, "base", "modified_before", capability)?;
                push_capability_root(roots, "candidate", "modified_after", capability)?;
            }
            (Some(_), None) => {
                push_capability_root(roots, "base", "removed", capability)?;
            }
            (None, Some(_)) => {
                push_capability_root(roots, "candidate", "added", capability)?;
            }
            (None, None) => unreachable!(),
        }
    }
    Ok(())
}

fn capability_incidence<'a>(
    edges: &'a BTreeSet<&'a workspace_graph::WorkspaceEdge>,
) -> BTreeMap<&'a str, BTreeSet<&'a workspace_graph::WorkspaceEdge>> {
    let mut incidence = BTreeMap::<_, BTreeSet<_>>::new();
    for edge in edges {
        if matches!(edge.kind(), "effect_requirement" | "capability_authority") {
            incidence.entry(edge.target()).or_default().insert(*edge);
        }
    }
    incidence
}

fn push_capability_root(
    roots: &mut Vec<SemanticWorkspaceChangeRoot>,
    state: &'static str,
    change: &'static str,
    capability: &str,
) -> Result<(), Vec<Diagnostic>> {
    push_root(
        roots,
        SemanticWorkspaceChangeRoot {
            state,
            kind: "capability",
            id: capability.to_owned(),
            path: None,
            module: None,
            change,
            identity_origin: None,
        },
    )
}

fn push_root(
    roots: &mut Vec<SemanticWorkspaceChangeRoot>,
    root: SemanticWorkspaceChangeRoot,
) -> Result<(), Vec<Diagnostic>> {
    if roots.len() == MAX_DELTA_ROOTS {
        return Err(limit("delta_roots", MAX_DELTA_ROOTS));
    }
    roots.push(root);
    Ok(())
}

fn root_key(
    root: &SemanticWorkspaceChangeRoot,
) -> (
    &str,
    &str,
    &str,
    Option<&str>,
    Option<&str>,
    &str,
    Option<&str>,
) {
    (
        root.state,
        root.kind,
        &root.id,
        root.path.as_deref(),
        root.module.as_deref(),
        root.change,
        root.identity_origin,
    )
}

fn declaration_kind_text(kind: hir::DeclarationKind) -> &'static str {
    match kind {
        hir::DeclarationKind::Resource => "resource",
        hir::DeclarationKind::ResourceDrop => "resource_drop",
        hir::DeclarationKind::Record => "record",
        hir::DeclarationKind::Class => "class",
        hir::DeclarationKind::Field => "field",
        hir::DeclarationKind::Variant => "variant",
        hir::DeclarationKind::VariantCase => "variant_case",
        hir::DeclarationKind::CaseField => "case_field",
        hir::DeclarationKind::Interface => "interface",
        hir::DeclarationKind::Import => "import",
        hir::DeclarationKind::Function => "function",
    }
}

fn edge_family(kind: &str) -> Result<&'static str, Vec<Diagnostic>> {
    EDGE_FAMILY_ORDER
        .into_iter()
        .find(|candidate| *candidate == kind)
        .ok_or_else(|| replay("Semantic Workspace Change edge family is unsupported"))
}

fn validate_revision(value: &str) -> Result<(), Vec<Diagnostic>> {
    validate_digest(value, "base revision")
}

fn validate_entry_module(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() > MAX_ENTRY_MODULE_BYTES {
        return Err(limit("entry_module_bytes", MAX_ENTRY_MODULE_BYTES));
    }
    let canonical = !value.is_empty()
        && value.split('.').all(|segment| {
            let mut bytes = segment.bytes();
            bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        });
    if canonical {
        Ok(())
    } else {
        Err(grammar(
            "Semantic Workspace Change entry module is not canonical",
        ))
    }
}

fn validate_digest(value: &str, field: &'static str) -> Result<(), Vec<Diagnostic>> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(vec![Diagnostic::io(
            "SPX-G181",
            crate::bounded_output::budgeted_format(format_args!(
                "Semantic Workspace change {field} is not canonical"
            )),
        )]);
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(vec![Diagnostic::io(
            "SPX-G181",
            crate::bounded_output::budgeted_format(format_args!(
                "Semantic Workspace change {field} is not canonical"
            )),
        )]);
    }
    Ok(())
}

fn canonical_body(source: &str) -> Result<&str, Vec<Diagnostic>> {
    if source.is_empty()
        || source.starts_with('\u{feff}')
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len().saturating_sub(1)].contains('\n')
    {
        return Err(grammar(
            "Semantic Workspace Change proposal must be one canonical JSON line with one terminal LF",
        ));
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
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| limit("json_depth", semantic_workspace::MAX_JSON_DEPTH))?;
                if depth > semantic_workspace::MAX_JSON_DEPTH {
                    return Err(limit("json_depth", semantic_workspace::MAX_JSON_DEPTH));
                }
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    grammar("Semantic Workspace Change proposal JSON is unbalanced")
                })?;
            }
            _ => {}
        }
    }
    if string || depth != 0 {
        return Err(grammar(
            "Semantic Workspace Change proposal JSON is unbalanced",
        ));
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| grammar("Semantic Workspace Change JSON value must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(grammar(
            "Semantic Workspace Change JSON has missing or extra keys",
        ));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| grammar("Semantic Workspace Change JSON field must be a string"))
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, Vec<Diagnostic>> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| grammar("Semantic Workspace Change JSON field must be an array"))
}

fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G181", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G182", message)]
}

fn replay(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G184", message)]
}

fn incomplete(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G186", message)]
}

fn map_change_builder_limit(
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
            "total_analysis_builder_bytes",
            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,
        )
    } else {
        diagnostics
    }
}

fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G183",
        crate::bounded_output::budgeted_format(format_args!(
            "Semantic Workspace Change `{field}` exceeds {maximum}"
        )),
    )]
}

#[cfg(test)]
#[path = "semantic_workspace_change/tests.rs"]
mod tests;
