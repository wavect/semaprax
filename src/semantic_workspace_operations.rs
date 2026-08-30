//! Authenticated stable-identity operation derivation for Semantic Workspace Change v1.
//!
//! Read-only derivation, Evidence generation, and verification hold one shared
//! semantic-workspace lock and return only after resolver-free held-workspace
//! reauthentication and checked unlock. Application instead holds one exclusive
//! lock, freshly replays the complete Operations intent and unchanged Change-v1
//! child Evidence, and may publish only the exact immutable candidate through
//! the existing sole `ACTIVE` pivot. Evidence and receipts provide no reusable
//! authorization, approval, signature, provenance, rollback, or cleanup authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::bounded_output::CappedString;
use crate::diagnostic::Diagnostic;
use crate::{semantic_workspace, semantic_workspace_change, workspace, workspace_graph};

mod evidence_artifact;
mod evidence_verification;

pub use evidence_artifact::SemanticWorkspaceOperationsEvidenceArtifacts;

const SCHEMA: &str = "semaprax.semantic-workspace-operations.v1";
const DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-workspace-operations.proposal-digest.v1\0";
pub(crate) const MAX_PROPOSAL_BYTES: usize = 1_048_576;
const MAX_OPERATIONS: usize = 64;
const MAX_AFFECTED_PATHS: usize = 16;
const MAX_PATH_BYTES: usize = 240;
const MAX_TARGET_ID_BYTES: usize = 4096;
const MAX_TARGET_MODULE_BYTES: usize = 240;
const MAX_ENTRY_MODULE_BYTES: usize = 16_777_216;
const MAX_NAME_BYTES: usize = 128;
const MAX_PLANNED_EDITS: usize = 131_072;
const MAX_EDIT_REPLACEMENT_BYTES: usize = 16_777_216;
const MAX_TOTAL_SOURCE_BYTES: usize = 16_777_216;
const MAX_TOTAL_REPLACEMENT_SOURCE_BYTES: usize = 4_194_304;
const MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH: usize = 1_048_576;
pub(crate) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16_777_216;
pub(crate) const MAX_OPERATIONS_BUILDER_BYTES: usize = 67_108_864;
const MAX_DERIVED_CHANGE_PROPOSAL_BYTES: usize = 33_554_432;
const DERIVATION_SCHEMA: &str = "semaprax.semantic-workspace-operations-derivation.v1";
const DERIVATION_DOMAIN: &[u8] =
    b"semaprax.semantic-workspace-operations-derivation.artifact-digest.v1\0";
const WORKSPACE_MANIFEST_SCHEMA: &str = "semaprax.workspace-semantic-manifest.v1";
const CHANGE_SCHEMA: &str = "semaprax.workspace-semantic-change.v1";
const MAX_DERIVATION_BYTES: usize = 33_554_432;
const MAX_TOTAL_DERIVATION_BYTES: usize = 67_108_864;
const MAX_JSON_DEPTH: usize = 8;

#[cfg(test)]
thread_local! {
    static CANDIDATE_PREFLIGHT_ENTRY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BASE_PREFLIGHT_ENTRY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_candidate_preflight_entry_count() {
    CANDIDATE_PREFLIGHT_ENTRY_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn candidate_preflight_entry_count() -> usize {
    CANDIDATE_PREFLIGHT_ENTRY_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
fn reset_base_operations_preflight_entry_count() {
    BASE_PREFLIGHT_ENTRY_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn base_operations_preflight_entry_count() -> usize {
    BASE_PREFLIGHT_ENTRY_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn mark_base_operations_preflight_entry() {
    BASE_PREFLIGHT_ENTRY_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn mark_candidate_preflight_entry() {
    CANDIDATE_PREFLIGHT_ENTRY_COUNT.with(|count| count.set(count.get() + 1));
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DeclarationSubject {
    Function,
    FunctionTemplate,
    Resource,
    Record,
    Variant,
    Interface,
}

impl DeclarationSubject {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "function" => Self::Function,
            "function_template" => Self::FunctionTemplate,
            "resource" => Self::Resource,
            "record" => Self::Record,
            "variant" => Self::Variant,
            "interface" => Self::Interface,
            _ => return None,
        })
    }

    const fn text(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::FunctionTemplate => "function_template",
            Self::Resource => "resource",
            Self::Record => "record",
            Self::Variant => "variant",
            Self::Interface => "interface",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ImportSubject {
    Function,
    Type,
}

impl ImportSubject {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "function" => Self::Function,
            "type" => Self::Type,
            _ => return None,
        })
    }

    const fn text(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Type => "type",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Operation {
    Declaration {
        path: String,
        subject: DeclarationSubject,
        target_id: String,
        from: String,
        to: String,
    },
    ImportAlias {
        path: String,
        subject: ImportSubject,
        target_id: String,
        target_module: String,
        from: String,
        to: String,
    },
}

impl Operation {
    fn sort_key(&self) -> (&str, u8, u8, &str, &str, &str, &str) {
        match self {
            Self::Declaration {
                path,
                subject,
                target_id,
                from,
                to,
            } => (path, 0, *subject as u8, target_id, "", from, to),
            Self::ImportAlias {
                path,
                subject,
                target_id,
                target_module,
                from,
                to,
            } => (path, 1, *subject as u8, target_id, target_module, from, to),
        }
    }

    fn selector(&self) -> (&str, u8, u8, &str, &str) {
        let (path, rank, subject, id, module, _, _) = self.sort_key();
        (path, rank, subject, id, module)
    }

    fn path(&self) -> &str {
        self.sort_key().0
    }
    fn from(&self) -> &str {
        self.sort_key().5
    }
    fn to(&self) -> &str {
        self.sort_key().6
    }
}

struct OperationsProposal {
    base_workspace_revision: String,
    entry_module: String,
    operations: Vec<Operation>,
    source: String,
    digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlannedEditFact {
    pub(crate) path: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
    pub(crate) operation_index: usize,
}

pub(crate) struct PreparedSemanticWorkspaceOperations {
    proposal_source: String,
    proposal_digest: String,
    operations: Vec<Operation>,
    edits: Vec<PlannedEditFact>,
    base_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    candidate_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    derived_change: semantic_workspace_change::SemanticWorkspaceChangeSet,
    base_graph: workspace_graph::WorkspaceGraphChangeView,
    candidate_graph: workspace_graph::WorkspaceGraphChangeView,
    base_change_builder_bytes: usize,
    candidate_change_builder_bytes: usize,
    base_workspace_revision: String,
    candidate_workspace_revision: String,
    candidate_manifest: String,
    entry_module: String,
    usage: OperationsUsageFacts,
    used_operations_builder_bytes: usize,
}

struct PreparedOperationsEvidenceInput {
    operations_proposal: String,
    operations_proposal_digest: String,
    derivation: SemanticWorkspaceOperationsDerivation,
    change: semantic_workspace_change::SemanticWorkspacePreparedChange,
}

pub(crate) struct SemanticWorkspaceOperationsCommitAuthority {
    authority: workspace::WorkspaceSemanticReadAuthority,
    candidate_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    candidate_manifest: String,
    candidate_revision: String,
    receipt: String,
    operations_proposal_digest: String,
    derivation_digest: String,
    derived_change_proposal_digest: String,
    workspace_change_evidence_digest: String,
    operations_evidence_digest: String,
}

impl SemanticWorkspaceOperationsCommitAuthority {
    pub(crate) fn into_parts(
        self,
    ) -> (
        workspace::WorkspaceSemanticReadAuthority,
        Vec<semantic_workspace::SemanticWorkspaceFileFact>,
        String,
        String,
        String,
    ) {
        assert!(
            valid_digest(&self.operations_proposal_digest)
                && valid_digest(&self.derivation_digest)
                && valid_digest(&self.derived_change_proposal_digest)
                && valid_digest(&self.workspace_change_evidence_digest)
                && valid_digest(&self.operations_evidence_digest),
            "sealed Operations commit authority requires exact replay digests"
        );
        (
            self.authority,
            self.candidate_files,
            self.candidate_manifest,
            self.candidate_revision,
            self.receipt,
        )
    }
}

impl PreparedSemanticWorkspaceOperations {
    fn into_evidence_input(
        self,
        derivation: SemanticWorkspaceOperationsDerivation,
        storage: (usize, usize, usize),
    ) -> Result<PreparedOperationsEvidenceInput, Vec<Diagnostic>> {
        if derivation.operations_proposal_digest != self.proposal_digest
            || derivation.derived_change_proposal != self.derived_change.source()
        {
            return Err(replay());
        }
        let operations_proposal = self.proposal_source;
        let operations_proposal_digest = self.proposal_digest;
        let change = semantic_workspace_change::prepare_from_operations_facts(
            semantic_workspace_change::OperationsChangeBridge {
                change_set: self.derived_change,
                authenticated_revision: self.base_workspace_revision,
                base_files: self.base_files,
                candidate_files: self.candidate_files,
                candidate_manifest: self.candidate_manifest,
                candidate_workspace_revision: self.candidate_workspace_revision,
                base_graph: self.base_graph,
                candidate_graph: self.candidate_graph,
                base_builder_bytes: self.base_change_builder_bytes,
                candidate_builder_bytes: self.candidate_change_builder_bytes,
                storage,
            },
        )?;
        Ok(PreparedOperationsEvidenceInput {
            operations_proposal,
            operations_proposal_digest,
            derivation,
            change,
        })
    }
}

/// Opaque read-only Operations derivation bundle.
pub struct SemanticWorkspaceOperationsDerivation {
    operations_proposal_digest: String,
    derived_change_proposal: String,
    derived_change_proposal_digest: String,
    derivation: String,
    derivation_digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationsDerivePoint {
    ProposalOwned,
    DerivationRendered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationsEvidencePoint {
    ProposalOwned,
    EvidenceOwned,
    AfterOperationsReplay,
    ChangeArtifactsRendered,
    EvidenceRendered,
    OperationsEvidenceReplayed,
    ReceiptRendered,
    Workspace(workspace::SemanticChangeApplyPoint),
}

#[derive(Clone, Copy)]
struct OperationsUsageFacts {
    managed_files: usize,
    operations: usize,
    affected_paths: usize,
    planned_edits: usize,
    edit_replacement_bytes: usize,
    total_base_source_bytes: usize,
    total_candidate_source_bytes: usize,
    total_replacement_source_bytes: usize,
    entry_module_bytes: usize,
    operations_proposal_bytes: usize,
    candidate_graph_builder_bytes: usize,
    derived_changed_files: usize,
    derived_change_proposal_bytes: usize,
}

#[cfg(test)]
impl PreparedSemanticWorkspaceOperations {
    pub(crate) fn proposal_source(&self) -> &str {
        &self.proposal_source
    }
    pub(crate) fn proposal_digest(&self) -> &str {
        &self.proposal_digest
    }
    pub(crate) fn edits(&self) -> &[PlannedEditFact] {
        &self.edits
    }
    pub(crate) fn derived_change_proposal(&self) -> &str {
        self.derived_change.source()
    }
    pub(crate) fn derived_change(&self) -> &semantic_workspace_change::SemanticWorkspaceChangeSet {
        &self.derived_change
    }
    pub(crate) fn operations_len(&self) -> usize {
        self.operations.len()
    }
    pub(crate) fn candidate_sources(&self) -> &[semantic_workspace::SemanticWorkspaceFileFact] {
        &self.candidate_files
    }
    pub(crate) fn base_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.base_graph
    }
    pub(crate) fn candidate_graph(&self) -> &workspace_graph::WorkspaceGraphChangeView {
        &self.candidate_graph
    }
    pub(crate) fn used_operations_builder_bytes(&self) -> usize {
        self.used_operations_builder_bytes
    }
}

impl SemanticWorkspaceOperationsDerivation {
    /// Returns the domain-separated digest of the exact Operations proposal.
    pub fn operations_proposal_digest(&self) -> &str {
        &self.operations_proposal_digest
    }

    /// Returns the canonical derived Change-v1 proposal, including its LF.
    pub fn derived_change_proposal(&self) -> &str {
        &self.derived_change_proposal
    }

    /// Returns the unchanged Change-v1 proposal digest.
    pub fn derived_change_proposal_digest(&self) -> &str {
        &self.derived_change_proposal_digest
    }

    /// Returns the canonical Operations derivation wrapper, including its LF.
    pub fn derivation(&self) -> &str {
        &self.derivation
    }

    /// Returns the domain-separated digest of the derivation wrapper.
    pub fn derivation_digest(&self) -> &str {
        &self.derivation_digest
    }

    fn into_derived_change_proposal(self) -> String {
        self.derived_change_proposal
    }

    fn into_derivation(self) -> String {
        self.derivation
    }
}

/// Derives one exact Change-v1 proposal and its canonical Operations wrapper.
pub fn derive(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceOperationsDerivation, Vec<Diagnostic>> {
    derive_with_hook(root, proposal_path, |_| {})
}

/// Returns the exact derived Change-v1 proposal, including its terminal LF.
pub fn derived_change_proposal(
    root: &Path,
    proposal_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    derive(root, proposal_path)
        .map(SemanticWorkspaceOperationsDerivation::into_derived_change_proposal)
}

/// Returns the canonical Operations derivation wrapper, including its terminal LF.
pub fn derivation(root: &Path, proposal_path: &Path) -> Result<String, Vec<Diagnostic>> {
    derive(root, proposal_path).map(SemanticWorkspaceOperationsDerivation::into_derivation)
}

/// Generates the exact Operations-intent Evidence bundle.
pub fn generate_evidence(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceOperationsEvidenceArtifacts, Vec<Diagnostic>> {
    generate_evidence_with_hook(root, proposal_path, |_| {})
}

/// Returns the canonical Operations-intent Evidence document.
pub fn evidence(root: &Path, proposal_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate_evidence(root, proposal_path)
        .map(SemanticWorkspaceOperationsEvidenceArtifacts::into_operations_evidence)
}

fn generate_evidence_with_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(OperationsEvidencePoint),
) -> Result<SemanticWorkspaceOperationsEvidenceArtifacts, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_operations_proposal(proposal_path).and_then(|source| {
        hook(OperationsEvidencePoint::ProposalOwned);
        parse_proposal(&source)
    });
    let (mut authority, proposal) = locked
        .authenticate_operations(proposal)
        .map_err(map_base_operations_builder_limit)?;
    let result = (|| {
        let storage = (
            authority.manifest_bytes(),
            authority.retained_generations(),
            authority.staging_attempts(),
        );
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources();
        let base = semantic_workspace::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        hook(OperationsEvidencePoint::AfterOperationsReplay);
        let derivation = render_derivation(&prepared)?;
        let prepared = prepared.into_evidence_input(derivation, storage)?;
        let change_artifacts =
            semantic_workspace_change::render_prepared_artifacts(&prepared.change)?;
        hook(OperationsEvidencePoint::ChangeArtifactsRendered);
        let artifacts = evidence_artifact::render_evidence(&prepared, &change_artifacts)?;
        hook(OperationsEvidencePoint::EvidenceRendered);
        Ok(artifacts)
    })();
    authority.finish(result)
}

/// Verifies one submitted Operations-intent Evidence document by exact replay.
pub fn verify(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    verify_with_hook(root, proposal_path, evidence_path, |_| {})
}

/// Applies one exact Operations intent after fresh whole Evidence replay.
pub fn apply(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    apply_with_hook(root, proposal_path, evidence_path, |_, _, _, _| Ok(()))
}

pub(crate) fn apply_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(
        OperationsEvidencePoint,
        &Path,
        Option<&Path>,
        Option<&Path>,
    ) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_apply_lock(root)?;
    let active_path = root.join(".semaprax-workspace/ACTIVE");
    let input = read_operations_proposal(proposal_path).and_then(|proposal_source| {
        hook(
            OperationsEvidencePoint::ProposalOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("proposal post-read hook failed", error))?;
        let evidence_source = evidence_verification::read_evidence(evidence_path)?;
        let submitted = evidence_verification::parse_evidence(&evidence_source)?;
        hook(
            OperationsEvidencePoint::EvidenceOwned,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Evidence post-read hook failed", error))?;
        let proposal = parse_proposal(&proposal_source)?;
        Ok((proposal, evidence_source, submitted))
    });
    let (mut authority, (proposal, evidence_source, submitted)) = locked
        .authenticate_operations(input)
        .map_err(map_base_operations_builder_limit)?;
    let prepublication = (|| {
        let storage = (
            authority.manifest_bytes(),
            authority.retained_generations(),
            authority.staging_attempts(),
        );
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources();
        let base = semantic_workspace::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        hook(
            OperationsEvidencePoint::AfterOperationsReplay,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Operations replay hook failed", error))?;
        let derivation = render_derivation(&prepared)?;
        let prepared = prepared.into_evidence_input(derivation, storage)?;
        let change_artifacts =
            semantic_workspace_change::render_prepared_artifacts(&prepared.change)?;
        hook(
            OperationsEvidencePoint::ChangeArtifactsRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Change artifact hook failed", error))?;
        let artifacts = evidence_artifact::render_evidence(&prepared, &change_artifacts)?;
        evidence_verification::verify_replay(
            &submitted,
            &evidence_source,
            artifacts.operations_evidence(),
            change_artifacts.evidence_bytes(),
        )?;
        let replay_token = evidence_artifact::exact_replay_token(
            &prepared,
            &change_artifacts,
            &artifacts,
            &evidence_source,
        )?;
        hook(
            OperationsEvidencePoint::OperationsEvidenceReplayed,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("Operations Evidence replay hook failed", error))?;
        let receipt =
            evidence_artifact::render_receipt(&prepared, &artifacts, &replay_token, true)?;
        hook(
            OperationsEvidencePoint::ReceiptRendered,
            &active_path,
            None,
            None,
        )
        .map_err(|error| operations_apply_hook("application receipt hook failed", error))?;
        Ok((prepared, receipt, replay_token))
    })();
    let (prepared, receipt, replay_token) = match prepublication {
        Ok(value) => value,
        Err(diagnostics) => return authority.finish(Err(diagnostics)),
    };
    let (candidate_files, candidate_manifest, candidate_revision) =
        prepared.change.into_operations_commit_parts();
    let commit = SemanticWorkspaceOperationsCommitAuthority {
        authority,
        candidate_files,
        candidate_manifest,
        candidate_revision,
        receipt,
        operations_proposal_digest: replay_token.operations_proposal_digest,
        derivation_digest: replay_token.derivation_digest,
        derived_change_proposal_digest: replay_token.derived_change_proposal_digest,
        workspace_change_evidence_digest: replay_token.workspace_change_evidence_digest,
        operations_evidence_digest: replay_token.operations_evidence_digest,
    };
    workspace::commit_semantic_operations_authority_with_hook(
        commit,
        |point, active, staged, candidate| {
            hook(
                OperationsEvidencePoint::Workspace(point),
                active,
                staged,
                candidate,
            )
        },
    )
}

fn operations_apply_hook(label: &'static str, error: std::io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I211", format!("{label}: {error}"))]
}

fn verify_with_hook(
    root: &Path,
    proposal_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(OperationsEvidencePoint),
) -> Result<String, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let input = read_operations_proposal(proposal_path).and_then(|proposal_source| {
        hook(OperationsEvidencePoint::ProposalOwned);
        let evidence_source = evidence_verification::read_evidence(evidence_path)?;
        let submitted = evidence_verification::parse_evidence(&evidence_source)?;
        hook(OperationsEvidencePoint::EvidenceOwned);
        let proposal = parse_proposal(&proposal_source)?;
        Ok((proposal, evidence_source, submitted))
    });
    let (mut authority, (proposal, evidence_source, submitted)) = locked
        .authenticate_operations(input)
        .map_err(map_base_operations_builder_limit)?;
    let result = (|| {
        let storage = (
            authority.manifest_bytes(),
            authority.retained_generations(),
            authority.staging_attempts(),
        );
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources();
        let base = semantic_workspace::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        hook(OperationsEvidencePoint::AfterOperationsReplay);
        let derivation = render_derivation(&prepared)?;
        let prepared = prepared.into_evidence_input(derivation, storage)?;
        let change_artifacts =
            semantic_workspace_change::render_prepared_artifacts(&prepared.change)?;
        hook(OperationsEvidencePoint::ChangeArtifactsRendered);
        let artifacts = evidence_artifact::render_evidence(&prepared, &change_artifacts)?;
        evidence_verification::verify_replay(
            &submitted,
            &evidence_source,
            artifacts.operations_evidence(),
            change_artifacts.evidence_bytes(),
        )?;
        let replay_token = evidence_artifact::exact_replay_token(
            &prepared,
            &change_artifacts,
            &artifacts,
            &evidence_source,
        )?;
        hook(OperationsEvidencePoint::OperationsEvidenceReplayed);
        let receipt =
            evidence_artifact::render_receipt(&prepared, &artifacts, &replay_token, false)?;
        hook(OperationsEvidencePoint::ReceiptRendered);
        Ok(receipt)
    })();
    authority.finish(result)
}

pub(crate) fn derive_with_hook(
    root: &Path,
    proposal_path: &Path,
    mut hook: impl FnMut(OperationsDerivePoint),
) -> Result<SemanticWorkspaceOperationsDerivation, Vec<Diagnostic>> {
    let locked = workspace::acquire_semantic_change_lock(root)?;
    let proposal = read_operations_proposal(proposal_path).and_then(|source| {
        hook(OperationsDerivePoint::ProposalOwned);
        parse_proposal(&source)
    });
    let (mut authority, proposal) = locked
        .authenticate_operations(proposal)
        .map_err(map_base_operations_builder_limit)?;
    let result = (|| {
        let authenticated_revision = authority.workspace_revision().to_owned();
        let graph = authority.take_graph()?;
        let sources = authority.take_sources();
        let base = semantic_workspace::authenticated_operations_preflight(
            &authenticated_revision,
            sources,
            graph,
        )?;
        let prepared = prepare_parsed_with_limit(proposal, base, MAX_OPERATIONS_BUILDER_BYTES)?;
        let derivation = render_derivation(&prepared)?;
        hook(OperationsDerivePoint::DerivationRendered);
        Ok(derivation)
    })();
    authority.finish(result)
}

fn read_operations_proposal(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut file = File::open(path).map_err(|_| proposal_io("open failed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| proposal_io("metadata inspection failed"))?;
    if !metadata.is_file() {
        return Err(proposal_io("input is not a regular file"));
    }
    if metadata.len() > MAX_PROPOSAL_BYTES as u64 {
        return Err(limit("operations_proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    let mut bytes = Vec::with_capacity((metadata.len() as usize).saturating_add(1));
    file.by_ref()
        .take((MAX_PROPOSAL_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| proposal_io("read failed"))?;
    if bytes.len() > MAX_PROPOSAL_BYTES {
        return Err(limit("operations_proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    String::from_utf8(bytes).map_err(|_| proposal_io("input is not UTF-8"))
}

fn proposal_io(detail: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-I216",
        format!("could not read Semantic Workspace Operations proposal: {detail}"),
    )]
}

fn map_base_operations_builder_limit(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    if diagnostics.len() == 1
        && diagnostics[0].code == "SPX-G171"
        && diagnostics[0].message
            == format!(
                "Workspace Semantic Graph `change_builder_bytes` exceeds {MAX_OPERATIONS_BUILDER_BYTES}"
            )
    {
        limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
    } else {
        diagnostics
    }
}

fn render_derivation(
    prepared: &PreparedSemanticWorkspaceOperations,
) -> Result<SemanticWorkspaceOperationsDerivation, Vec<Diagnostic>> {
    render_derivation_with_limits(prepared, MAX_DERIVATION_BYTES, MAX_TOTAL_DERIVATION_BYTES)
}

#[cfg(test)]
fn render_derivation_with_test_limits(
    prepared: &PreparedSemanticWorkspaceOperations,
    derivation_limit: usize,
    total_limit: usize,
) -> Result<SemanticWorkspaceOperationsDerivation, Vec<Diagnostic>> {
    assert!(derivation_limit <= MAX_DERIVATION_BYTES);
    assert!(total_limit <= MAX_TOTAL_DERIVATION_BYTES);
    render_derivation_with_limits(prepared, derivation_limit, total_limit)
}

fn render_derivation_with_limits(
    prepared: &PreparedSemanticWorkspaceOperations,
    derivation_limit: usize,
    total_limit: usize,
) -> Result<SemanticWorkspaceOperationsDerivation, Vec<Diagnostic>> {
    validate_derivation_usage(prepared)?;
    let derived_change_proposal = prepared.derived_change.source().to_owned();
    let derived_change_proposal_digest = digest_without_length(
        b"semaprax.workspace-semantic-change.proposal-digest.v1\0",
        derived_change_proposal.as_bytes(),
    );
    let input_bytes = prepared.proposal_source.len();
    let derived_bytes = derived_change_proposal.len();
    let fixed_bytes = input_bytes
        .checked_add(derived_bytes)
        .ok_or_else(|| limit("total_derivation_bytes", MAX_TOTAL_DERIVATION_BYTES))?;
    if fixed_bytes > total_limit {
        return Err(limit("total_derivation_bytes", MAX_TOTAL_DERIVATION_BYTES));
    }
    let aggregate_remaining = total_limit - fixed_bytes;
    let output_limit = derivation_limit.min(aggregate_remaining);
    let mut used_derivation_bytes = 0usize;
    let derivation = loop {
        let total = fixed_bytes
            .checked_add(used_derivation_bytes)
            .ok_or_else(|| limit("total_derivation_bytes", MAX_TOTAL_DERIVATION_BYTES))?;
        let (document, overflowed) = crate::bounded_output::with_limit(output_limit, || {
            render_derivation_document(
                prepared,
                &derived_change_proposal_digest,
                used_derivation_bytes,
                total,
            )
        });
        if overflowed {
            return Err(if aggregate_remaining < derivation_limit {
                limit("total_derivation_bytes", MAX_TOTAL_DERIVATION_BYTES)
            } else {
                limit("derivation_bytes", MAX_DERIVATION_BYTES)
            });
        }
        if document.len() > derivation_limit {
            return Err(limit("derivation_bytes", MAX_DERIVATION_BYTES));
        }
        if document.len() > aggregate_remaining {
            return Err(limit("total_derivation_bytes", MAX_TOTAL_DERIVATION_BYTES));
        }
        if document.len() == used_derivation_bytes {
            break document;
        }
        used_derivation_bytes = document.len();
    };
    if prepared.proposal_digest != proposal_digest(&prepared.proposal_source)
        || derived_change_proposal != prepared.derived_change.source()
    {
        return Err(replay());
    }
    let derivation_digest = digest_without_length(DERIVATION_DOMAIN, derivation.as_bytes());
    Ok(SemanticWorkspaceOperationsDerivation {
        operations_proposal_digest: prepared.proposal_digest.clone(),
        derived_change_proposal,
        derived_change_proposal_digest,
        derivation,
        derivation_digest,
    })
}

fn validate_derivation_usage(
    prepared: &PreparedSemanticWorkspaceOperations,
) -> Result<(), Vec<Diagnostic>> {
    let usage = prepared.usage;
    let replayed_operations = parse_proposal(&prepared.proposal_source).map_err(|_| replay())?;
    if replayed_operations.source != prepared.proposal_source
        || replayed_operations.digest != prepared.proposal_digest
        || replayed_operations.base_workspace_revision != prepared.base_workspace_revision
        || replayed_operations.entry_module != prepared.entry_module
        || replayed_operations.operations != prepared.operations
        || prepared.candidate_workspace_revision == prepared.base_workspace_revision
    {
        return Err(replay());
    }
    let candidate_manifest_facts =
        semantic_workspace::parse_manifest(&prepared.candidate_manifest).map_err(|_| replay())?;
    if semantic_workspace::render_manifest(&candidate_manifest_facts).map_err(|_| replay())?
        != prepared.candidate_manifest
        || candidate_manifest_facts.len() != prepared.candidate_files.len()
        || candidate_manifest_facts
            .iter()
            .zip(&prepared.candidate_files)
            .any(|(fact, source)| {
                fact.path() != source.path()
                    || prepared
                        .candidate_graph
                        .modules()
                        .iter()
                        .find(|module| module.path() == fact.path())
                        .is_none_or(|module| {
                            module.source_graph_schema() != fact.source_graph_schema()
                        })
                    || fact.bytes() != source.source().len()
                    || fact.source_revision()
                        != crate::graph::revision_from_canonical_source(source.source())
                    || fact.source_digest()
                        != crate::review::source_digest(source.source().as_bytes())
            })
    {
        return Err(replay());
    }
    let affected_paths = prepared
        .operations
        .iter()
        .map(Operation::path)
        .collect::<BTreeSet<_>>()
        .len();
    let edit_replacement_bytes = prepared
        .edits
        .iter()
        .try_fold(0usize, |total, edit| {
            total.checked_add(edit.replacement.len())
        })
        .ok_or_else(replay)?;
    let total_candidate_source_bytes = prepared
        .candidate_files
        .iter()
        .try_fold(0usize, |total, source| {
            total.checked_add(source.source().len())
        })
        .ok_or_else(replay)?;
    let total_replacement_source_bytes = prepared
        .derived_change
        .total_replacement_source_bytes()
        .ok_or_else(replay)?;
    let replayed_change =
        semantic_workspace_change::parse_proposal(prepared.derived_change.source())
            .map_err(|_| replay())?;
    if replayed_change.source() != prepared.derived_change.source()
        || replayed_change.base_workspace_revision() != prepared.base_workspace_revision
        || replayed_change.entry_module() != prepared.entry_module
        || replayed_change.changed_file_count() != affected_paths
        || replayed_change.files().len() != prepared.derived_change.files().len()
        || replayed_change
            .files()
            .iter()
            .zip(prepared.derived_change.files())
            .any(|(left, right)| {
                left.path() != right.path()
                    || left.base_source_graph_schema() != right.base_source_graph_schema()
                    || left.base_source_revision() != right.base_source_revision()
                    || left.base_source_digest() != right.base_source_digest()
                    || left.replacement_source() != right.replacement_source()
            })
        || prepared.base_graph.used_managed_files() != prepared.candidate_graph.used_managed_files()
        || prepared.base_workspace_revision != prepared.derived_change.base_workspace_revision()
        || semantic_workspace::semantic_workspace_revision(&prepared.candidate_manifest)
            != prepared.candidate_workspace_revision
    {
        return Err(replay());
    }
    if usage.managed_files != prepared.base_graph.used_managed_files()
        || usage.operations != prepared.operations.len()
        || usage.affected_paths != affected_paths
        || usage.planned_edits != prepared.edits.len()
        || usage.edit_replacement_bytes != edit_replacement_bytes
        || usage.total_base_source_bytes != prepared.base_graph.used_total_source_bytes()
        || usage.total_candidate_source_bytes != total_candidate_source_bytes
        || usage.total_candidate_source_bytes != prepared.candidate_graph.used_total_source_bytes()
        || usage.total_replacement_source_bytes != total_replacement_source_bytes
        || usage.entry_module_bytes != prepared.entry_module.len()
        || usage.entry_module_bytes != prepared.derived_change.entry_module().len()
        || usage.operations_proposal_bytes != prepared.proposal_source.len()
        || usage.candidate_graph_builder_bytes != prepared.candidate_graph.used_builder_bytes()
        || usage.derived_changed_files != prepared.derived_change.changed_file_count()
        || usage.derived_change_proposal_bytes != prepared.derived_change.source().len()
    {
        return Err(replay());
    }
    for (field, used, maximum) in [
        ("managed_files", usage.managed_files, 16),
        ("operations", usage.operations, MAX_OPERATIONS),
        ("affected_paths", usage.affected_paths, MAX_AFFECTED_PATHS),
        ("planned_edits", usage.planned_edits, MAX_PLANNED_EDITS),
        (
            "edit_replacement_bytes",
            usage.edit_replacement_bytes,
            MAX_EDIT_REPLACEMENT_BYTES,
        ),
        (
            "total_base_source_bytes",
            usage.total_base_source_bytes,
            MAX_TOTAL_SOURCE_BYTES,
        ),
        (
            "total_candidate_source_bytes",
            usage.total_candidate_source_bytes,
            MAX_TOTAL_SOURCE_BYTES,
        ),
        (
            "total_replacement_source_bytes",
            usage.total_replacement_source_bytes,
            MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
        ),
        (
            "entry_module_bytes",
            usage.entry_module_bytes,
            MAX_ENTRY_MODULE_BYTES,
        ),
        (
            "operations_proposal_bytes",
            usage.operations_proposal_bytes,
            MAX_PROPOSAL_BYTES,
        ),
        (
            "candidate_graph_builder_bytes",
            usage.candidate_graph_builder_bytes,
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
        ),
        (
            "operations_builder_bytes",
            prepared.used_operations_builder_bytes,
            MAX_OPERATIONS_BUILDER_BYTES,
        ),
        (
            "derived_change_proposal_bytes",
            usage.derived_change_proposal_bytes,
            MAX_DERIVED_CHANGE_PROPOSAL_BYTES,
        ),
    ] {
        if used > maximum {
            return Err(limit(field, maximum));
        }
    }
    if usage.derived_changed_files != usage.affected_paths {
        return Err(replay());
    }
    Ok(())
}

fn parse_proposal(source: &str) -> Result<OperationsProposal, Vec<Diagnostic>> {
    if source.len() > MAX_PROPOSAL_BYTES {
        return Err(limit("operations_proposal_bytes", MAX_PROPOSAL_BYTES));
    }
    if !source.ends_with('\n') || source[..source.len().saturating_sub(1)].contains('\n') {
        return Err(grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"));
    }
    let body = &source[..source.len() - 1];
    if json_depth(body)? > MAX_JSON_DEPTH {
        return Err(limit("json_depth", MAX_JSON_DEPTH));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"))?;
    let object = value.as_object().ok_or_else(|| {
        grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
    })?;
    if object.len() != 4
        || !object.contains_key("schema")
        || !object.contains_key("base_workspace_revision")
        || !object.contains_key("entry_module")
        || !object.contains_key("operations")
    {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    if text(object.get("schema"))? != SCHEMA {
        return Err(grammar(
            "Semantic Workspace Operations proposal schema is unsupported",
        ));
    }
    let base_workspace_revision = text(object.get("base_workspace_revision"))?.to_owned();
    let entry_module = text(object.get("entry_module"))?.to_owned();
    if !valid_digest(&base_workspace_revision) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    if entry_module.len() > MAX_ENTRY_MODULE_BYTES {
        return Err(limit("entry_module_bytes", MAX_ENTRY_MODULE_BYTES));
    }
    if !valid_qualified_module(&entry_module) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    let rows = object
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
        })?;
    if rows.len() < 2 {
        return Err(grammar(
            "Semantic Workspace Operations requires 2..64 operations",
        ));
    }
    if rows.len() > MAX_OPERATIONS {
        return Err(limit("operations", MAX_OPERATIONS));
    }
    let mut operations = Vec::with_capacity(rows.len());
    for row in rows {
        operations.push(parse_operation(row)?);
    }
    if operations
        .windows(2)
        .any(|pair| pair[0].sort_key() >= pair[1].sort_key())
    {
        return Err(grammar(
            "Semantic Workspace Operations operations must be strictly sorted",
        ));
    }
    let selectors = operations
        .iter()
        .map(Operation::selector)
        .collect::<BTreeSet<_>>();
    if selectors.len() != operations.len() {
        return Err(conflict(
            "Semantic Workspace Operations operation selector is duplicated",
        ));
    }
    let paths = operations
        .iter()
        .map(Operation::path)
        .collect::<BTreeSet<_>>();
    if paths.len() < 2 {
        return Err(conflict(
            "Semantic Workspace Operations require 2..16 distinct affected managed paths",
        ));
    }
    if paths.len() > MAX_AFFECTED_PATHS {
        return Err(limit("affected_paths", MAX_AFFECTED_PATHS));
    }
    let proposal = OperationsProposal {
        base_workspace_revision,
        entry_module,
        operations,
        source: source.to_owned(),
        digest: proposal_digest(source),
    };
    if render_proposal(&proposal)? != source {
        return Err(grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"));
    }
    Ok(proposal)
}

fn parse_operation(value: &Value) -> Result<Operation, Vec<Diagnostic>> {
    let object = value.as_object().ok_or_else(|| {
        grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
    })?;
    let kind = text(object.get("kind"))?;
    let operation = match kind {
        "rename_declaration" => {
            exact_keys(
                object,
                &[
                    "kind",
                    "path",
                    "declaration_kind",
                    "target_id",
                    "from",
                    "to",
                ],
            )?;
            Operation::Declaration {
                path: bounded(
                    text(object.get("path"))?,
                    "path_bytes",
                    MAX_PATH_BYTES,
                    false,
                )?,
                subject: DeclarationSubject::parse(text(object.get("declaration_kind"))?)
                    .ok_or_else(|| {
                        grammar("Semantic Workspace Operations operation kind is unsupported")
                    })?,
                target_id: bounded(
                    text(object.get("target_id"))?,
                    "target_id_bytes",
                    MAX_TARGET_ID_BYTES,
                    false,
                )?,
                from: bounded(
                    text(object.get("from"))?,
                    "name_bytes",
                    MAX_NAME_BYTES,
                    true,
                )?,
                to: bounded(text(object.get("to"))?, "name_bytes", MAX_NAME_BYTES, true)?,
            }
        }
        "rename_import_alias" => {
            exact_keys(
                object,
                &[
                    "kind",
                    "path",
                    "import_kind",
                    "target_id",
                    "target_module",
                    "from",
                    "to",
                ],
            )?;
            Operation::ImportAlias {
                path: bounded(
                    text(object.get("path"))?,
                    "path_bytes",
                    MAX_PATH_BYTES,
                    false,
                )?,
                subject: ImportSubject::parse(text(object.get("import_kind"))?).ok_or_else(
                    || grammar("Semantic Workspace Operations operation kind is unsupported"),
                )?,
                target_id: bounded(
                    text(object.get("target_id"))?,
                    "target_id_bytes",
                    MAX_TARGET_ID_BYTES,
                    false,
                )?,
                target_module: bounded(
                    text(object.get("target_module"))?,
                    "target_module_bytes",
                    MAX_TARGET_MODULE_BYTES,
                    false,
                )?,
                from: bounded(
                    text(object.get("from"))?,
                    "name_bytes",
                    MAX_NAME_BYTES,
                    true,
                )?,
                to: bounded(text(object.get("to"))?, "name_bytes", MAX_NAME_BYTES, true)?,
            }
        }
        _ => {
            return Err(grammar(
                "Semantic Workspace Operations operation kind is unsupported",
            ))
        }
    };
    if operation.from() == operation.to() {
        return Err(conflict(
            "Semantic Workspace Operations rename must change the source name",
        ));
    }
    if !crate::workspace::evidence_path_is_valid(operation.path()) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    if let Operation::ImportAlias { target_module, .. } = &operation {
        if !valid_qualified_module(target_module) {
            return Err(grammar(
                "Semantic Workspace Operations proposal object keys or value types are invalid",
            ));
        }
    }
    Ok(operation)
}

#[cfg(test)]
pub(crate) fn prepare_owned(
    proposal_source: &str,
    base: semantic_workspace::SemanticWorkspacePreflight,
) -> Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>> {
    prepare_owned_with_limit(proposal_source, base, MAX_OPERATIONS_BUILDER_BYTES)
}

#[cfg(test)]
fn prepare_owned_with_limit(
    proposal_source: &str,
    base: semantic_workspace::SemanticWorkspacePreflight,
    operations_builder_limit: usize,
) -> Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>> {
    let proposal = parse_proposal(proposal_source)?;
    prepare_parsed_with_limit(proposal, base, operations_builder_limit)
}

fn prepare_parsed_with_limit(
    proposal: OperationsProposal,
    base: semantic_workspace::SemanticWorkspacePreflight,
    operations_builder_limit: usize,
) -> Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>> {
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
        return Err(limit("total_base_source_bytes", MAX_TOTAL_SOURCE_BYTES));
    }
    let (base_files, _, base_workspace_revision, base_build) = base.into_snapshot_parts();
    let operation_view = base_build.into_operation_view().map_err(|_| replay())?;
    if operation_view.builder_bytes > operations_builder_limit {
        return Err(limit(
            "operations_builder_bytes",
            MAX_OPERATIONS_BUILDER_BYTES,
        ));
    }
    let remaining = operations_builder_limit - operation_view.builder_bytes;
    let base_builder_bytes = operation_view.builder_bytes;
    let base_change_builder_bytes = operation_view.change_builder_bytes;
    let (result, overflowed, replay_builder_bytes) =
        crate::bounded_output::with_limit_usage(remaining, || {
            prepare_owned_inner(
                proposal,
                &base_workspace_revision,
                base_files,
                operation_view,
                base_change_builder_bytes,
            )
        });
    if overflowed {
        return Err(limit(
            "operations_builder_bytes",
            MAX_OPERATIONS_BUILDER_BYTES,
        ));
    }
    let mut prepared = result?;
    prepared.used_operations_builder_bytes =
        base_builder_bytes
            .checked_add(replay_builder_bytes)
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    Ok(prepared)
}

fn prepare_owned_inner(
    proposal: OperationsProposal,
    base_workspace_revision: &str,
    base_files: Vec<semantic_workspace::SemanticWorkspaceFileFact>,
    operation_view: workspace_graph::WorkspaceGraphOperationView,
    base_change_builder_bytes: usize,
) -> Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>> {
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
        return Err(binding(false));
    }
    if !operation_view
        .graph
        .modules()
        .iter()
        .any(|module| module.module() == proposal.entry_module)
    {
        return Err(binding(false));
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
            return Err(conflict(
                "Semantic Workspace Operations source occurrence proof is incomplete",
            ));
        }
        let new_edit_count = edits
            .len()
            .checked_add(selected.len())
            .ok_or_else(|| limit("planned_edits", MAX_PLANNED_EDITS))?;
        if new_edit_count > MAX_PLANNED_EDITS {
            return Err(limit("planned_edits", MAX_PLANNED_EDITS));
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
        return Err(limit("planned_edits", MAX_PLANNED_EDITS));
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
        return Err(conflict(
            "Semantic Workspace Operations derived source edit ranges overlap",
        ));
    }
    let replacement_bytes = edits
        .iter()
        .try_fold(0usize, |n, e| n.checked_add(e.replacement.len()))
        .ok_or_else(|| limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES))?;
    if replacement_bytes > MAX_EDIT_REPLACEMENT_BYTES {
        return Err(limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES));
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
                return Err(limit(
                    "total_replacement_source_bytes",
                    MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
                ));
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
        return Err(limit(
            "total_candidate_source_bytes",
            MAX_TOTAL_SOURCE_BYTES,
        ));
    }
    let path_set = semantic_workspace::render_path_set(&base_paths)?;
    let remaining_operations = crate::bounded_output::active_remaining()
        .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?;
    let candidate_graph_limit = MAX_CANDIDATE_GRAPH_BUILDER_BYTES.min(remaining_operations);
    #[cfg(test)]
    mark_candidate_preflight_entry();
    let candidate = semantic_workspace::preflight_owned_for_operations(
        &path_set,
        candidate_sources,
        candidate_graph_limit,
        remaining_operations,
    )
    .map_err(|diagnostics| {
        map_candidate_diagnostics(diagnostics, candidate_graph_limit, remaining_operations)
    })?;
    let (candidate_files, candidate_manifest, candidate_revision, candidate_build) =
        candidate.into_snapshot_parts();
    let candidate_view = candidate_build
        .into_operation_view()
        .map_err(|_| replay())?;
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
        return Err(replay());
    }
    if candidate_revision == proposal.base_workspace_revision {
        return Err(replay());
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

fn reserve_operations(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(limit(
            "operations_builder_bytes",
            MAX_OPERATIONS_BUILDER_BYTES,
        ))
    }
}

fn render_candidate_source(
    original: &str,
    edits: &[&PlannedEditFact],
) -> Result<String, Vec<Diagnostic>> {
    let mut removed_bytes = 0usize;
    let mut replacement_bytes = 0usize;
    let mut cursor = 0usize;
    for edit in edits {
        if edit.start < cursor
            || edit.end < edit.start
            || edit.end > original.len()
            || !original.is_char_boundary(edit.start)
            || !original.is_char_boundary(edit.end)
        {
            return Err(replay());
        }
        removed_bytes = removed_bytes
            .checked_add(edit.end - edit.start)
            .ok_or_else(|| limit("total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
        replacement_bytes = replacement_bytes
            .checked_add(edit.replacement.len())
            .ok_or_else(|| limit("edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES))?;
        cursor = edit.end;
    }
    let final_len = original
        .len()
        .checked_sub(removed_bytes)
        .and_then(|bytes| bytes.checked_add(replacement_bytes))
        .ok_or_else(|| limit("total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES))?;
    if final_len > MAX_TOTAL_SOURCE_BYTES {
        return Err(limit(
            "total_candidate_source_bytes",
            MAX_TOTAL_SOURCE_BYTES,
        ));
    }
    reserve_operations(final_len)?;
    let mut rendered = String::with_capacity(final_len);
    cursor = 0;
    for edit in edits {
        rendered.push_str(&original[cursor..edit.start]);
        rendered.push_str(&edit.replacement);
        cursor = edit.end;
    }
    rendered.push_str(&original[cursor..]);
    if rendered.len() != final_len {
        return Err(replay());
    }
    Ok(rendered)
}

fn validate_replacement_source_per_path(source: &str) -> Result<(), Vec<Diagnostic>> {
    if source.len() > MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH {
        Err(limit(
            "replacement_source_bytes_per_path",
            MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH,
        ))
    } else {
        Ok(())
    }
}

fn map_candidate_diagnostics(
    diagnostics: Vec<Diagnostic>,
    candidate_graph_limit: usize,
    operations_builder_limit: usize,
) -> Vec<Diagnostic> {
    if diagnostics.len() != 1 || diagnostics[0].code != "SPX-G171" {
        return diagnostics;
    }
    let builder_message =
        format!("Workspace Semantic Graph `builder_bytes` exceeds {candidate_graph_limit}");
    if diagnostics[0].message == builder_message {
        return if candidate_graph_limit == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {
            limit(
                "candidate_graph_builder_bytes",
                MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            )
        } else {
            limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES)
        };
    }
    let change_message = format!(
        "Workspace Semantic Graph `change_builder_bytes` exceeds {}",
        operations_builder_limit
    );
    if diagnostics[0].message == change_message {
        return limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES);
    }
    diagnostics
}

fn validate_candidate_namespaces(
    sidecar: &workspace_graph::WorkspaceOperationSidecar,
    operations: &[Operation],
) -> Result<(), Vec<Diagnostic>> {
    let mut names = BTreeSet::new();
    for declaration in &sidecar.declarations {
        let category = match declaration.kind {
            "function" | "function_template" => 0_u8,
            "resource" | "record" | "variant" => 1,
            "interface" => 2,
            _ => return Err(replay()),
        };
        let final_name = operations
            .iter()
            .find_map(|operation| match operation {
                Operation::Declaration {
                    path,
                    subject,
                    target_id,
                    to,
                    ..
                } if path == &declaration.path
                    && subject.text() == declaration.kind
                    && target_id == &declaration.id =>
                {
                    Some(to.as_str())
                }
                _ => None,
            })
            .unwrap_or(&declaration.name);
        if !names.insert((declaration.path.as_str(), category, final_name)) {
            return Err(conflict(
                "Semantic Workspace Operations candidate name namespace conflicts",
            ));
        }
    }
    for import in &sidecar.imports {
        let category = match import.kind {
            "function" => 0_u8,
            "type" => 1,
            _ => return Err(replay()),
        };
        let final_name = operations
            .iter()
            .find_map(|operation| match operation {
                Operation::ImportAlias {
                    path,
                    subject,
                    target_id,
                    target_module,
                    to,
                    ..
                } if path == &import.path
                    && subject.text() == import.kind
                    && target_id == &import.target_id
                    && target_module == &import.target_module =>
                {
                    Some(to.as_str())
                }
                _ => None,
            })
            .unwrap_or(&import.alias);
        if !names.insert((import.path.as_str(), category, final_name)) {
            return Err(conflict(
                "Semantic Workspace Operations candidate name namespace conflicts",
            ));
        }
    }
    Ok(())
}

fn select_occurrences(
    source: &str,
    operation: &Operation,
    sidecar: &workspace_graph::WorkspaceOperationSidecar,
    remaining_edits: usize,
) -> Result<Vec<(usize, usize)>, Vec<Diagnostic>> {
    let occurrences = match operation {
        Operation::Declaration {
            path,
            subject,
            target_id,
            from,
            ..
        } => sidecar
            .declarations
            .iter()
            .find(|item| {
                item.path == *path
                    && item.explicit
                    && item.kind == subject.text()
                    && item.id == *target_id
                    && item.name == *from
            })
            .map(|item| item.occurrences.as_slice())
            .ok_or_else(|| binding(false))?,
        Operation::ImportAlias {
            path,
            subject,
            target_id,
            target_module,
            from,
            ..
        } => sidecar
            .imports
            .iter()
            .find(|item| {
                item.path == *path
                    && item.kind == subject.text()
                    && item.target_id == *target_id
                    && item.target_module == *target_module
                    && item.alias == *from
            })
            .map(|item| item.occurrences.as_slice())
            .ok_or_else(|| binding(true))?,
    };
    if occurrences.len() > remaining_edits {
        return Err(limit("planned_edits", MAX_PLANNED_EDITS));
    }
    reserve_operations(
        occurrences
            .len()
            .checked_mul(std::mem::size_of::<(usize, usize)>())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut spans = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        if source.get(occurrence.span.start..occurrence.span.end) != Some(operation.from()) {
            return Err(conflict(
                "Semantic Workspace Operations source occurrence proof is incomplete",
            ));
        }
        spans.push((occurrence.span.start, occurrence.span.end));
    }
    Ok(spans)
}

fn replay_candidate(
    base: &workspace_graph::WorkspaceGraphOperationView,
    candidate: &workspace_graph::WorkspaceGraphOperationView,
    operations: &[Operation],
) -> Result<(), Vec<Diagnostic>> {
    if base.graph.modules() != candidate.graph.modules() {
        return Err(replay());
    }
    for operation in operations {
        match operation {
            Operation::Declaration {
                path,
                subject,
                target_id,
                from,
                to,
            } => {
                let before = base
                    .sidecar
                    .declarations
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.explicit
                            && item.kind == subject.text()
                            && item.id == *target_id
                            && item.name == *from
                    })
                    .ok_or_else(replay)?;
                let after = candidate
                    .sidecar
                    .declarations
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.explicit
                            && item.kind == subject.text()
                            && item.id == *target_id
                            && item.name == *to
                    })
                    .ok_or_else(replay)?;
                if !same_occurrence_owners(&before.occurrences, &after.occurrences) {
                    return Err(replay());
                }
            }
            Operation::ImportAlias {
                path,
                subject,
                target_id,
                target_module,
                from,
                to,
            } => {
                let before = base
                    .sidecar
                    .imports
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.kind == subject.text()
                            && item.target_id == *target_id
                            && item.target_module == *target_module
                            && item.alias == *from
                    })
                    .ok_or_else(replay)?;
                let after = candidate
                    .sidecar
                    .imports
                    .iter()
                    .find(|item| {
                        item.path == *path
                            && item.kind == subject.text()
                            && item.target_id == *target_id
                            && item.target_module == *target_module
                            && item.alias == *to
                    })
                    .ok_or_else(replay)?;
                if !same_occurrence_owners(&before.occurrences, &after.occurrences) {
                    return Err(replay());
                }
            }
        }
    }
    if !same_normalized_sidecar(&base.sidecar, &candidate.sidecar, operations) {
        return Err(replay());
    }
    reserve_operations(
        base.graph
            .declarations()
            .len()
            .checked_mul(std::mem::size_of::<String>() + std::mem::size_of::<usize>())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut allowed_fingerprint_owners = BTreeSet::new();
    for operation in operations {
        let occurrences = selected_occurrences(&base.sidecar, operation).ok_or_else(replay)?;
        for occurrence in occurrences {
            if let Some(owner) = &occurrence.owner {
                allowed_fingerprint_owners.insert(owner.as_str());
                if let Ok(index) = base
                    .graph
                    .declarations()
                    .binary_search_by(|declaration| declaration.id().cmp(owner))
                {
                    let declaration = &base.graph.declarations()[index];
                    if let Some(parent) = declaration.owner() {
                        allowed_fingerprint_owners.insert(parent);
                    }
                }
            }
        }
    }
    let before_declarations = base.graph.declarations();
    let after_declarations = candidate.graph.declarations();
    if before_declarations.len() != after_declarations.len() {
        return Err(replay());
    }
    for (before, after) in before_declarations.iter().zip(after_declarations) {
        let fingerprint_may_change = allowed_fingerprint_owners.contains(before.id())
            || before
                .owner()
                .is_some_and(|owner| allowed_fingerprint_owners.contains(owner));
        if before.id() != after.id()
            || before.kind() != after.kind()
            || before.origin() != after.origin()
            || before.owner() != after.owner()
            || before.path() != after.path()
            || before.module() != after.module()
            || (!fingerprint_may_change
                && before.semantic_fingerprint() != after.semantic_fingerprint())
        {
            return Err(replay());
        }
    }
    if !same_normalized_edges(base.graph.edges(), candidate.graph.edges(), operations)? {
        return Err(replay());
    }
    Ok(())
}

fn selected_occurrences<'a>(
    sidecar: &'a workspace_graph::WorkspaceOperationSidecar,
    operation: &Operation,
) -> Option<&'a [workspace_graph::WorkspaceOperationOccurrence]> {
    match operation {
        Operation::Declaration {
            path,
            subject,
            target_id,
            ..
        } => sidecar
            .declarations
            .iter()
            .find(|item| item.path == *path && item.kind == subject.text() && item.id == *target_id)
            .map(|item| item.occurrences.as_slice()),
        Operation::ImportAlias {
            path,
            subject,
            target_id,
            target_module,
            ..
        } => sidecar
            .imports
            .iter()
            .find(|item| {
                item.path == *path
                    && item.kind == subject.text()
                    && item.target_id == *target_id
                    && item.target_module == *target_module
            })
            .map(|item| item.occurrences.as_slice()),
    }
}

fn same_occurrence_owners(
    before: &[workspace_graph::WorkspaceOperationOccurrence],
    after: &[workspace_graph::WorkspaceOperationOccurrence],
) -> bool {
    before.len() == after.len()
        && before
            .iter()
            .zip(after)
            .all(|(left, right)| left.owner == right.owner)
}

fn same_normalized_sidecar(
    before: &workspace_graph::WorkspaceOperationSidecar,
    after: &workspace_graph::WorkspaceOperationSidecar,
    operations: &[Operation],
) -> bool {
    before.declarations.len() == after.declarations.len()
        && before
            .declarations
            .iter()
            .zip(&after.declarations)
            .all(|(left, right)| {
                let selected = operations.iter().any(|operation| {
                    matches!(operation,
                Operation::Declaration { path, subject, target_id, .. }
                    if *path == left.path && subject.text() == left.kind && *target_id == left.id)
                });
                left.path == right.path
                    && left.kind == right.kind
                    && left.explicit == right.explicit
                    && left.id == right.id
                    && left.normalized_fingerprint == right.normalized_fingerprint
                    && (selected || left.name == right.name)
                    && same_occurrence_owners(&left.occurrences, &right.occurrences)
            })
        && before.imports.len() == after.imports.len()
        && before
            .imports
            .iter()
            .zip(&after.imports)
            .all(|(left, right)| {
                let selected = operations.iter().any(|operation| {
                    matches!(operation,
                Operation::ImportAlias { path, subject, target_id, target_module, .. }
                    if *path == left.path && subject.text() == left.kind
                    && *target_id == left.target_id && *target_module == left.target_module)
                });
                left.path == right.path
                    && left.kind == right.kind
                    && left.target_id == right.target_id
                    && left.target_module == right.target_module
                    && (selected || left.alias == right.alias)
                    && same_occurrence_owners(&left.occurrences, &right.occurrences)
            })
}

fn same_normalized_edges(
    before: &[workspace_graph::WorkspaceEdge],
    after: &[workspace_graph::WorkspaceEdge],
    operations: &[Operation],
) -> Result<bool, Vec<Diagnostic>> {
    if before.len() != after.len() {
        return Ok(false);
    }
    type OccurrenceKey<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str, usize);
    reserve_operations(
        before
            .len()
            .checked_mul(std::mem::size_of::<OccurrenceKey<'_>>())
            .ok_or_else(|| limit("operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES))?,
    )?;
    let mut selected_calls = Vec::with_capacity(before.len());
    for edge in before.iter().filter(|edge| edge.kind() == "call") {
        if operations.iter().any(|operation| {
            matches!(operation,
            Operation::ImportAlias { path, target_id, from, .. }
                if edge.caller_path() == path && edge.target() == target_id && edge.alias() == from)
        }) {
            selected_calls.push((
                edge.caller_path(),
                edge.caller(),
                edge.site(),
                edge.expression(),
                edge.ast_path(),
                edge.ordinal(),
            ));
        }
    }
    selected_calls.sort_unstable();
    selected_calls.dedup();
    Ok(before.iter().zip(after).all(|(left, right)| {
        let alias_renamed = operations.iter().any(|operation| {
            let Operation::ImportAlias {
                path,
                target_id,
                from,
                to,
                ..
            } = operation
            else {
                return false;
            };
            if left.caller_path() != path || left.alias() != from || right.alias() != to {
                return false;
            }
            left.target() == target_id
                || (left.kind() == "effect_requirement"
                    && selected_calls
                        .binary_search(&(
                            left.caller_path(),
                            left.caller(),
                            left.site(),
                            left.expression(),
                            left.ast_path(),
                            left.ordinal(),
                        ))
                        .is_ok())
        });
        let same = left.caller_path() == right.caller_path()
            && left.caller() == right.caller()
            && left.target_path() == right.target_path()
            && left.target() == right.target()
            && left.kind() == right.kind()
            && left.site() == right.site()
            && left.expression() == right.expression()
            && left.ast_path() == right.ast_path()
            && (alias_renamed || left.alias() == right.alias())
            && left.ordinal() == right.ordinal();
        same
    }))
}

fn render_proposal(p: &OperationsProposal) -> Result<String, Vec<Diagnostic>> {
    let (s, o) = crate::bounded_output::with_limit(MAX_PROPOSAL_BYTES, || {
        let mut x = CappedString::new();
        x.push_str("{\"schema\":");
        json(&mut x, SCHEMA);
        x.push_str(",\"base_workspace_revision\":");
        json(&mut x, &p.base_workspace_revision);
        x.push_str(",\"entry_module\":");
        json(&mut x, &p.entry_module);
        x.push_str(",\"operations\":[");
        for (i, op) in p.operations.iter().enumerate() {
            if i > 0 {
                x.push(',')
            }
            render_op(&mut x, op)
        }
        x.push_str("]}\n");
        x.into_string()
    });
    if o {
        Err(limit("operations_proposal_bytes", MAX_PROPOSAL_BYTES))
    } else {
        Ok(s)
    }
}

fn render_derivation_document(
    prepared: &PreparedSemanticWorkspaceOperations,
    derived_change_digest: &str,
    used_derivation_bytes: usize,
    used_total_derivation_bytes: usize,
) -> String {
    let mut out = CappedString::new();
    out.push_str("{\"schema\":");
    json(&mut out, DERIVATION_SCHEMA);
    out.push_str(",\"workspace_manifest_schema\":");
    json(&mut out, WORKSPACE_MANIFEST_SCHEMA);
    out.push_str(",\"base_workspace_revision\":");
    json(&mut out, &prepared.base_workspace_revision);
    out.push_str(",\"candidate_workspace_revision\":");
    json(&mut out, &prepared.candidate_workspace_revision);
    out.push_str(",\"entry_module\":");
    json(&mut out, &prepared.entry_module);
    out.push_str(",\"operations_proposal\":{\"schema\":");
    json(&mut out, SCHEMA);
    out.push_str(",\"digest\":");
    json(&mut out, &prepared.proposal_digest);
    out.push_str(",\"bytes\":");
    number(&mut out, prepared.proposal_source.len());
    out.push_str("},\"derived_workspace_change_proposal\":{\"schema\":");
    json(&mut out, CHANGE_SCHEMA);
    out.push_str(",\"digest\":");
    json(&mut out, derived_change_digest);
    out.push_str(",\"bytes\":");
    number(&mut out, prepared.derived_change.source().len());
    out.push_str("},\"limits\":");
    render_derivation_limits(&mut out);
    out.push_str(",\"budget\":");
    render_derivation_budget(
        &mut out,
        prepared,
        used_derivation_bytes,
        used_total_derivation_bytes,
    );
    out.push_str(",\"nonclaims\":[");
    for (index, claim) in DERIVATION_NONCLAIMS.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        json(&mut out, claim);
    }
    out.push_str("]}\n");
    out.into_string()
}

fn render_derivation_limits(out: &mut CappedString) {
    out.push('{');
    for (index, (field, value)) in DERIVATION_LIMITS.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        json(out, field);
        out.push(':');
        number(out, *value);
    }
    out.push('}');
}

fn render_derivation_budget(
    out: &mut CappedString,
    prepared: &PreparedSemanticWorkspaceOperations,
    used_derivation_bytes: usize,
    used_total_derivation_bytes: usize,
) {
    let usage = prepared.usage;
    let values = [
        usage.managed_files,
        usage.operations,
        usage.affected_paths,
        usage.planned_edits,
        usage.edit_replacement_bytes,
        usage.total_base_source_bytes,
        usage.total_candidate_source_bytes,
        usage.total_replacement_source_bytes,
        usage.entry_module_bytes,
        usage.operations_proposal_bytes,
        usage.candidate_graph_builder_bytes,
        prepared.used_operations_builder_bytes,
        usage.derived_changed_files,
        usage.derived_change_proposal_bytes,
        used_derivation_bytes,
        used_total_derivation_bytes,
    ];
    out.push('{');
    for (index, (field, value)) in DERIVATION_BUDGET_FIELDS.iter().zip(values).enumerate() {
        if index > 0 {
            out.push(',');
        }
        json(out, field);
        out.push(':');
        number(out, value);
    }
    out.push('}');
}

fn number(out: &mut CappedString, value: usize) {
    let _ = write!(out, "{value}");
}

fn digest_without_length(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

const DERIVATION_LIMITS: [(&str, usize); 21] = [
    ("max_managed_files", 16),
    ("max_operations_proposal_bytes", MAX_PROPOSAL_BYTES),
    ("max_operations", MAX_OPERATIONS),
    ("max_affected_paths", MAX_AFFECTED_PATHS),
    ("max_path_bytes", MAX_PATH_BYTES),
    ("max_target_id_bytes", MAX_TARGET_ID_BYTES),
    ("max_target_module_bytes", MAX_TARGET_MODULE_BYTES),
    ("max_entry_module_bytes", MAX_ENTRY_MODULE_BYTES),
    ("max_name_bytes", MAX_NAME_BYTES),
    ("max_planned_edits", MAX_PLANNED_EDITS),
    ("max_edit_replacement_bytes", MAX_EDIT_REPLACEMENT_BYTES),
    ("max_total_base_source_bytes", MAX_TOTAL_SOURCE_BYTES),
    ("max_total_candidate_source_bytes", MAX_TOTAL_SOURCE_BYTES),
    (
        "max_total_replacement_source_bytes",
        MAX_TOTAL_REPLACEMENT_SOURCE_BYTES,
    ),
    (
        "max_replacement_source_bytes_per_path",
        MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH,
    ),
    (
        "max_candidate_graph_builder_bytes",
        MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
    ),
    ("max_operations_builder_bytes", MAX_OPERATIONS_BUILDER_BYTES),
    (
        "max_derived_change_proposal_bytes",
        MAX_DERIVED_CHANGE_PROPOSAL_BYTES,
    ),
    ("max_derivation_bytes", MAX_DERIVATION_BYTES),
    ("max_total_derivation_bytes", MAX_TOTAL_DERIVATION_BYTES),
    ("max_json_depth", MAX_JSON_DEPTH),
];

const DERIVATION_BUDGET_FIELDS: [&str; 16] = [
    "used_managed_files",
    "used_operations",
    "used_affected_paths",
    "used_planned_edits",
    "used_edit_replacement_bytes",
    "used_total_base_source_bytes",
    "used_total_candidate_source_bytes",
    "used_total_replacement_source_bytes",
    "used_entry_module_bytes",
    "used_operations_proposal_bytes",
    "used_candidate_graph_builder_bytes",
    "used_operations_builder_bytes",
    "used_derived_changed_files",
    "used_derived_change_proposal_bytes",
    "used_derivation_bytes",
    "used_total_derivation_bytes",
];

const DERIVATION_NONCLAIMS: [&str; 24] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_target_evidence_or_machine_code_claim",
    "no_context_impact_review_or_evidence",
    "no_operations_evidence_verification_receipt_or_apply_authority",
    "no_commit_or_publication_authority_in_derivation",
    "no_existing_change_v1_evidence_binding_to_operations_intent",
    "no_raw_path_create_delete_move_or_write",
    "no_path_set_change",
    "no_automatic_or_compiler_identity_targeting",
    "no_unmanaged_path_or_raw_tree_authority",
    "no_raw_tree_git_or_editor_atomic_visibility",
    "no_automatic_rollback_cleanup_or_gc",
    "no_power_loss_durability_guarantee",
    "no_network_distributed_nfs_or_overlay_guarantee",
    "no_acl_xattr_ads_preservation",
    "no_general_proof_system",
    "no_persistence_or_incrementality",
    "no_external_consumer_compatibility",
    "no_new_language_graph_cleanup_backend_or_runtime_semantics",
    "no_change_v1_schema_api_or_kat_modification",
];
fn render_op(x: &mut CappedString, op: &Operation) {
    match op {
        Operation::Declaration {
            path,
            subject,
            target_id,
            from,
            to,
        } => {
            x.push_str("{\"kind\":\"rename_declaration\",\"path\":");
            json(x, path);
            x.push_str(",\"declaration_kind\":");
            json(x, subject.text());
            x.push_str(",\"target_id\":");
            json(x, target_id);
            x.push_str(",\"from\":");
            json(x, from);
            x.push_str(",\"to\":");
            json(x, to)
        }
        Operation::ImportAlias {
            path,
            subject,
            target_id,
            target_module,
            from,
            to,
        } => {
            x.push_str("{\"kind\":\"rename_import_alias\",\"path\":");
            json(x, path);
            x.push_str(",\"import_kind\":");
            json(x, subject.text());
            x.push_str(",\"target_id\":");
            json(x, target_id);
            x.push_str(",\"target_module\":");
            json(x, target_module);
            x.push_str(",\"from\":");
            json(x, from);
            x.push_str(",\"to\":");
            json(x, to)
        }
    }
    x.push('}')
}
fn json(x: &mut CappedString, s: &str) {
    x.push('"');
    for c in s.chars() {
        match c {
            '"' => x.push_str("\\\""),
            '\\' => x.push_str("\\\\"),
            '\n' => x.push_str("\\n"),
            '\r' => x.push_str("\\r"),
            '\t' => x.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(x, "\\u{:04x}", c as u32);
            }
            c => x.push(c),
        }
    }
    x.push('"')
}
fn proposal_digest(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(DIGEST_DOMAIN);
    h.update((s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(h.finalize()))
}
fn exact_keys(o: &serde_json::Map<String, Value>, keys: &[&str]) -> Result<(), Vec<Diagnostic>> {
    if o.len() != keys.len() || keys.iter().any(|k| !o.contains_key(*k)) {
        Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ))
    } else {
        Ok(())
    }
}
fn text(v: Option<&Value>) -> Result<&str, Vec<Diagnostic>> {
    v.and_then(Value::as_str).ok_or_else(|| {
        grammar("Semantic Workspace Operations proposal object keys or value types are invalid")
    })
}
fn bounded(v: &str, field: &str, max: usize, identifier: bool) -> Result<String, Vec<Diagnostic>> {
    if v.len() > max {
        return Err(limit(field, max));
    }
    if v.is_empty() || (identifier && !valid_ident(v)) {
        return Err(grammar(
            "Semantic Workspace Operations proposal object keys or value types are invalid",
        ));
    }
    Ok(v.to_owned())
}
fn valid_ident(v: &str) -> bool {
    let mut c = v.chars();
    c.next()
        .is_some_and(|x| x == '_' || x.is_ascii_alphabetic())
        && c.all(|x| x == '_' || x.is_ascii_alphanumeric())
}
pub(super) fn valid_qualified_module(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(valid_ident)
}
pub(super) fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn json_depth(s: &str) -> Result<usize, Vec<Diagnostic>> {
    let mut d = 0usize;
    let mut m = 0usize;
    let mut q = false;
    let mut e = false;
    for c in s.chars() {
        if q {
            if e {
                e = false
            } else if c == '\\' {
                e = true
            } else if c == '"' {
                q = false
            }
        } else {
            match c{'"'=>q=true,'{'|'['=>{d+=1;m=m.max(d)},'}'|']'=>d=d.checked_sub(1).ok_or_else(||grammar("Semantic Workspace Operations proposal must be one canonical JSON line with one terminal LF"))?,_=>{}}
        }
    }
    Ok(m)
}
fn grammar(m: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G196", m)]
}
fn binding(import: bool) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G197",
        if import {
            "Semantic Workspace Operations import alias does not match one direct explicit pre-state import binding"
        } else {
            "Semantic Workspace Operations target does not match one explicit user-owned pre-state declaration"
        },
    )]
}
fn conflict(m: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G198", m)]
}
fn limit(f: &str, m: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G199",
        format!("Semantic Workspace Operations exceeds {f} maximum {m}"),
    )]
}
fn replay() -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G200","Semantic Workspace Operations derivation disagrees with authenticated base or candidate semantics")]
}

fn operations_evidence_replay() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G203",
        "submitted Semantic Workspace Operations Evidence does not exactly replay the authenticated Operations proposal and derived Change Evidence",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs2::FileExt;
    use std::fs::OpenOptions;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    static MANAGED_SERIAL: AtomicU64 = AtomicU64::new(0);

    struct ManagedOperationsFixture {
        root: PathBuf,
        proposal_path: PathBuf,
        proposal_source: String,
    }

    impl ManagedOperationsFixture {
        fn new(label: &str) -> Self {
            let serial = MANAGED_SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-semantic-workspace-operations-{label}-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&root).unwrap();
            let provider = canonical(
                "a/provider.spx",
                "module ops.provider; @id(\"ops.answer\") fn answer()->i64{1}",
            );
            let consumer = canonical(
                "b/consumer.spx",
                "module ops.consumer; use function @id(\"ops.answer\") from ops.provider as answer; @id(\"ops.main\") fn main()->i64{answer()}",
            );
            for (path, source) in [("a/provider.spx", provider), ("b/consumer.spx", consumer)] {
                let destination = root.join(path);
                std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
                std::fs::write(destination, source).unwrap();
            }
            let path_set = root.join("paths.json");
            std::fs::write(
                &path_set,
                semantic_workspace::render_path_set(&[
                    "a/provider.spx".to_owned(),
                    "b/consumer.spx".to_owned(),
                ])
                .unwrap(),
            )
            .unwrap();
            let revision = semantic_workspace::initialize(&root, &path_set).unwrap();
            let proposal_source = format!(
                "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.answer\",\"from\":\"answer\",\"to\":\"response\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.answer\",\"target_module\":\"ops.provider\",\"from\":\"answer\",\"to\":\"response\"}}]}}\n"
            );
            let proposal_path = root.join("operations.json");
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
                        facts.push((relative, false, std::fs::read(path).unwrap()));
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

        fn managed_source_path(&self, relative: &str) -> PathBuf {
            let generations = self.root.join(".semaprax-workspace/generations");
            let entries = std::fs::read_dir(&generations)
                .unwrap()
                .map(|entry| entry.unwrap())
                .filter(|entry| entry.file_type().unwrap().is_dir())
                .collect::<Vec<_>>();
            assert_eq!(entries.len(), 1);
            entries[0].path().join("files").join(relative)
        }
    }

    impl Drop for ManagedOperationsFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn canonical(path: &str, source: &str) -> String {
        let program = crate::parse(source, Path::new(path)).unwrap();
        crate::format::canonical(&program)
    }

    fn fixture() -> (semantic_workspace::SemanticWorkspacePreflight, String) {
        let provider = canonical(
            "a/provider.spx",
            "module ops.provider; @id(\"ops.answer\") fn answer()->i64{1}",
        );
        let consumer = canonical(
            "b/consumer.spx",
            "module ops.consumer; use function @id(\"ops.answer\") from ops.provider as answer; @id(\"ops.main\") fn main()->i64{answer()}",
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let preflight = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.answer\",\"from\":\"answer\",\"to\":\"response\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.answer\",\"target_module\":\"ops.provider\",\"from\":\"answer\",\"to\":\"response\"}}]}}\n",
            preflight.workspace_revision(),
        );
        (preflight, proposal)
    }

    fn evidence_render_fixture() -> (
        PreparedOperationsEvidenceInput,
        semantic_workspace_change::SemanticWorkspaceChangeArtifacts,
    ) {
        let (base, proposal) = fixture();
        let manifest_bytes = base.manifest().len();
        let prepared = prepare_owned(&proposal, base).unwrap();
        let derivation = render_derivation(&prepared).unwrap();
        let prepared = prepared
            .into_evidence_input(derivation, (manifest_bytes, 1, 0))
            .unwrap();
        let change =
            semantic_workspace_change::render_prepared_artifacts(&prepared.change).unwrap();
        (prepared, change)
    }

    fn broad_fixture() -> (semantic_workspace::SemanticWorkspacePreflight, String) {
        let provider = canonical(
            "a/provider.spx",
            r#"module ops.provider;
permit { audit.read }
@id("ops.token") resource Token { @id("ops.token.drop") drop trivial; }
@id("ops.point") record Point { @id("ops.point.value") value: i64, }
@id("ops.choice") variant Choice { @id("ops.choice.some") Some { @id("ops.choice.some.value") value: i64, }, @id("ops.choice.none") None, }
@id("ops.host") interface Host permits {} {
    @id("ops.host.release") import fn release(token: own Token) -> unit effects {} failure infallible consumes token always;
}
@id("ops.work") fn work(value: Point) -> i64 uses { audit.read } { value.value }
@id("ops.echo") fn echo<T>(value: T) -> T { value }
@id("ops.provider.main") fn main() -> i64 uses { audit.read } { match Choice::Some { value: 1 } { Choice::Some { value } => work(Point { value: echo<i64>(value) }), Choice::None {} => 0, } }
"#,
        );
        let consumer = canonical(
            "b/consumer.spx",
            r#"module ops.consumer;
use function @id("ops.work") from ops.provider as provider;
use type @id("ops.point") from ops.provider as Point;
permit { audit.read }
@id("ops.consumer.box") record Box { @id("ops.consumer.box.point") Point: i64, @id("ops.consumer.box.value") value: Point, }
@id("ops.consumer.wrapper") record Wrapper { @id("ops.consumer.wrapper.point") point: Point, }
@id("ops.consumer.identity") fn identity<T>(value: T) -> T { value }
@id("ops.consumer.shadow") fn shadow<Point>(Point: Point) -> Point { Point }
@id("ops.consumer.local") fn local(provider: i64) -> i64 { let Point = provider; Point }
@id("ops.consumer.preserve") fn preserve(value: Point) -> Point { value }
@id("ops.consumer.run") fn run(value: Point) -> i64 uses { audit.read } {
    let wrapped = Wrapper { point: value };
    match wrapped { Wrapper { point: nested } => provider(nested), }
}
@id("ops.consumer.main") fn main() -> i64 uses { audit.read } { run(Point { value: 0 }) }
"#,
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let preflight = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.work\",\"from\":\"work\",\"to\":\"compute\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function_template\",\"target_id\":\"ops.echo\",\"from\":\"echo\",\"to\":\"identity\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"resource\",\"target_id\":\"ops.token\",\"from\":\"Token\",\"to\":\"Handle\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.point\",\"from\":\"Point\",\"to\":\"Metric\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"variant\",\"target_id\":\"ops.choice\",\"from\":\"Choice\",\"to\":\"Outcome\"}},{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"interface\",\"target_id\":\"ops.host\",\"from\":\"Host\",\"to\":\"Runtime\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.work\",\"target_module\":\"ops.provider\",\"from\":\"provider\",\"to\":\"compute\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"type\",\"target_id\":\"ops.point\",\"target_module\":\"ops.provider\",\"from\":\"Point\",\"to\":\"Metric\"}}]}}\n",
            preflight.workspace_revision(),
        );
        (preflight, proposal)
    }

    fn code(result: Result<PreparedSemanticWorkspaceOperations, Vec<Diagnostic>>) -> String {
        result.err().expect("expected failure")[0].code.to_owned()
    }

    #[test]
    fn compiles_two_prestate_operations_into_exact_change_v1() {
        let (base, proposal) = fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        assert_eq!(prepared.proposal_source(), proposal);
        assert!(prepared.proposal_digest().starts_with("sha256:"));
        assert_eq!(prepared.operations_len(), 2);
        assert_eq!(
            prepared
                .edits()
                .iter()
                .map(|edit| edit.path.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["a/provider.spx", "b/consumer.spx"])
        );
        assert_eq!(prepared.candidate_sources().len(), 2);
        assert_eq!(prepared.base_graph().modules().len(), 2);
        assert_eq!(prepared.candidate_graph().modules().len(), 2);
        assert!(prepared.used_operations_builder_bytes() <= MAX_OPERATIONS_BUILDER_BYTES);
        assert_eq!(
            prepared.derived_change().source(),
            prepared.derived_change_proposal()
        );
        assert_eq!(
            semantic_workspace_change::parse_proposal(prepared.derived_change_proposal())
                .unwrap()
                .source(),
            prepared.derived_change_proposal()
        );
    }

    #[test]
    fn derivation_wrapper_binds_exact_retained_proposals_and_fixed_point_usage() {
        let (base, proposal) = fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        let output = render_derivation(&prepared).unwrap();
        let document: Value = serde_json::from_str(output.derivation().trim_end()).unwrap();
        assert_eq!(document["schema"], DERIVATION_SCHEMA);
        assert_eq!(
            document["operations_proposal"]["digest"],
            output.operations_proposal_digest()
        );
        assert_eq!(
            document["derived_workspace_change_proposal"]["digest"],
            output.derived_change_proposal_digest()
        );
        assert_eq!(
            document["derived_workspace_change_proposal"]["bytes"],
            output.derived_change_proposal().len()
        );
        assert_eq!(
            document["budget"]["used_derivation_bytes"],
            output.derivation().len()
        );
        assert_eq!(
            document["budget"]["used_total_derivation_bytes"],
            proposal.len() + output.derived_change_proposal().len() + output.derivation().len()
        );
        assert_eq!(
            output.derivation_digest(),
            digest_without_length(DERIVATION_DOMAIN, output.derivation().as_bytes())
        );
        assert!(output.derivation().ends_with('\n'));
    }

    #[test]
    fn authenticated_derivation_kat_binds_refs_budget_nonclaims_and_build_counts() {
        let fixture = ManagedOperationsFixture::new("derivation-kat");
        reset_base_operations_preflight_entry_count();
        reset_candidate_preflight_entry_count();
        let before = fixture.inventory();
        let output = derive_with_hook(&fixture.root, &fixture.proposal_path, |_| {}).unwrap();
        assert_eq!(base_operations_preflight_entry_count(), 1);
        assert_eq!(candidate_preflight_entry_count(), 1);
        assert_eq!(fixture.inventory(), before);
        fixture.assert_exclusive_reacquire();

        let document: Value = serde_json::from_str(output.derivation().trim_end()).unwrap();
        assert_eq!(document["schema"], DERIVATION_SCHEMA);
        assert_eq!(document["operations_proposal"]["schema"], SCHEMA);
        assert_eq!(
            document["operations_proposal"]["digest"],
            proposal_digest(&fixture.proposal_source)
        );
        assert_eq!(
            document["operations_proposal"]["bytes"],
            fixture.proposal_source.len()
        );
        assert_eq!(
            document["derived_workspace_change_proposal"]["schema"],
            CHANGE_SCHEMA
        );
        assert_eq!(
            document["derived_workspace_change_proposal"]["digest"],
            output.derived_change_proposal_digest()
        );
        assert_eq!(
            document["derived_workspace_change_proposal"]["bytes"],
            output.derived_change_proposal().len()
        );
        assert_eq!(
            document["nonclaims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<Vec<_>>(),
            DERIVATION_NONCLAIMS
        );
        assert_eq!(
            document["budget"]["used_derivation_bytes"],
            output.derivation().len()
        );
        assert_eq!(
            document["budget"]["used_total_derivation_bytes"],
            fixture.proposal_source.len()
                + output.derived_change_proposal().len()
                + output.derivation().len()
        );
        assert_eq!(
            output.operations_proposal_digest(),
            "sha256:3c7bf340a5313907edcec41748063e8666793ee76b903bc4e691871a843544b5"
        );
        assert_eq!(
            output.derived_change_proposal_digest(),
            "sha256:5c7a67d42ef76b3a241c0dc98f3d8919a799d3745bb6ae54a1d0289a51ee3e86"
        );
        assert_eq!(
            output.derivation_digest(),
            "sha256:7a4836b4ab443313792b3c4e7c05cc4557f0ef4fcdd456c18d24aba49a73b8ef"
        );
    }

    #[test]
    fn derivation_refs_usage_and_candidate_facts_fail_closed_on_mutation() {
        fn expect_replay(mutator: impl FnOnce(&mut PreparedSemanticWorkspaceOperations)) {
            let (base, proposal) = fixture();
            let mut prepared = prepare_owned(&proposal, base).unwrap();
            mutator(&mut prepared);
            let diagnostics = render_derivation(&prepared).err().unwrap();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, "SPX-G200");
        }

        expect_replay(|prepared| prepared.proposal_digest.push('0'));
        expect_replay(|prepared| prepared.base_workspace_revision.push('0'));
        expect_replay(|prepared| prepared.candidate_workspace_revision.push('0'));
        expect_replay(|prepared| prepared.entry_module.push_str(".other"));
        expect_replay(|prepared| prepared.candidate_manifest.push(' '));
        expect_replay(|prepared| prepared.usage.operations += 1);
        expect_replay(|prepared| prepared.usage.affected_paths += 1);
        expect_replay(|prepared| prepared.usage.planned_edits += 1);
        expect_replay(|prepared| prepared.usage.total_candidate_source_bytes += 1);
        expect_replay(|prepared| prepared.usage.candidate_graph_builder_bytes += 1);
        expect_replay(|prepared| prepared.candidate_files[0].source_mut().push('\n'));
    }

    #[test]
    fn derivation_individual_and_aggregate_caps_are_exact_and_cannot_expand() {
        let (base, proposal) = fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        let output = render_derivation(&prepared).unwrap();
        let derivation_bytes = output.derivation().len();
        let total_bytes =
            proposal.len() + output.derived_change_proposal().len() + derivation_bytes;
        assert!(render_derivation_with_test_limits(
            &prepared,
            derivation_bytes,
            MAX_TOTAL_DERIVATION_BYTES
        )
        .is_ok());
        assert_eq!(
            render_derivation_with_test_limits(
                &prepared,
                derivation_bytes - 1,
                MAX_TOTAL_DERIVATION_BYTES
            )
            .err()
            .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds derivation_bytes maximum 33554432"
        );
        assert!(
            render_derivation_with_test_limits(&prepared, MAX_DERIVATION_BYTES, total_bytes)
                .is_ok()
        );
        assert_eq!(
            render_derivation_with_test_limits(&prepared, MAX_DERIVATION_BYTES, total_bytes - 1)
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds total_derivation_bytes maximum 67108864"
        );
    }

    #[test]
    #[should_panic]
    fn derivation_limit_test_seam_cannot_expand_production_authority() {
        let (base, proposal) = fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        let _ = render_derivation_with_test_limits(
            &prepared,
            MAX_DERIVATION_BYTES + 1,
            MAX_TOTAL_DERIVATION_BYTES,
        );
    }

    #[test]
    fn operations_input_ownership_lock_precedence_and_limits_are_fail_closed() {
        let fixture = ManagedOperationsFixture::new("input-hostiles");
        let baseline = fixture.inventory();
        let missing = fixture.root.join("missing.json");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(fixture.root.join(".semaprax-workspace/LOCK"))
            .unwrap();
        FileExt::try_lock_exclusive(&lock).unwrap();
        let diagnostics = derive_with_hook(&fixture.root, &missing, |_| {})
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I210");
        FileExt::unlock(&lock).unwrap();
        assert_eq!(fixture.inventory(), baseline);

        let directory = fixture.root.join("proposal-dir");
        std::fs::create_dir(&directory).unwrap();
        let diagnostics = derive_with_hook(&fixture.root, &directory, |_| {})
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I216");
        #[cfg(windows)]
        assert_eq!(
            diagnostics[0].message,
            "could not read Semantic Workspace Operations proposal: open failed"
        );
        #[cfg(not(windows))]
        assert_eq!(
            diagnostics[0].message,
            "could not read Semantic Workspace Operations proposal: input is not a regular file"
        );
        fixture.assert_exclusive_reacquire();

        let invalid = fixture.root.join("invalid-utf8.json");
        std::fs::write(&invalid, [0xff]).unwrap();
        let diagnostics = derive_with_hook(&fixture.root, &invalid, |_| {})
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I216");
        assert_eq!(
            diagnostics[0].message,
            "could not read Semantic Workspace Operations proposal: input is not UTF-8"
        );
        fixture.assert_exclusive_reacquire();

        let exact = fixture.root.join("exact-limit.json");
        let exact_file = std::fs::File::create(&exact).unwrap();
        exact_file.set_len(MAX_PROPOSAL_BYTES as u64).unwrap();
        let diagnostics = derive_with_hook(&fixture.root, &exact, |_| {})
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G196");
        fixture.assert_exclusive_reacquire();

        let over = fixture.root.join("over-limit.json");
        let over_file = std::fs::File::create(&over).unwrap();
        over_file.set_len(MAX_PROPOSAL_BYTES as u64 + 1).unwrap();
        let diagnostics = derive_with_hook(&fixture.root, &over, |_| {})
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G199");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations exceeds operations_proposal_bytes maximum 1048576"
        );
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn operations_proposal_is_owned_once_and_final_drift_returns_no_derivation() {
        let baseline_fixture = ManagedOperationsFixture::new("owned-baseline");
        let baseline = derive_with_hook(
            &baseline_fixture.root,
            &baseline_fixture.proposal_path,
            |_| {},
        )
        .unwrap();

        let owned = ManagedOperationsFixture::new("owned-replacement");
        let output = derive_with_hook(&owned.root, &owned.proposal_path, |point| {
            if point == OperationsDerivePoint::ProposalOwned {
                std::fs::write(&owned.proposal_path, "{}\n").unwrap();
            }
        })
        .unwrap();
        assert_eq!(output.derivation(), baseline.derivation());
        assert_eq!(
            output.derived_change_proposal(),
            baseline.derived_change_proposal()
        );
        assert_eq!(
            std::fs::read_to_string(&owned.proposal_path).unwrap(),
            "{}\n"
        );
        owned.assert_exclusive_reacquire();

        let drift = ManagedOperationsFixture::new("final-drift");
        let before_control = drift
            .inventory()
            .into_iter()
            .filter(|(path, _, _)| {
                path.starts_with(".semaprax-workspace")
                    && !path.ends_with("/ACTIVE")
                    && !path.ends_with("\\ACTIVE")
            })
            .collect::<Vec<_>>();
        let result = derive_with_hook(&drift.root, &drift.proposal_path, |point| {
            if point == OperationsDerivePoint::DerivationRendered {
                use std::io::Write as _;
                OpenOptions::new()
                    .append(true)
                    .open(drift.root.join(".semaprax-workspace/ACTIVE"))
                    .unwrap()
                    .write_all(b"x")
                    .unwrap();
            }
        });
        assert!(result.is_err());
        assert_eq!(result.err().unwrap()[0].code, "SPX-G153");
        let after_control = drift
            .inventory()
            .into_iter()
            .filter(|(path, _, _)| {
                path.starts_with(".semaprax-workspace")
                    && !path.ends_with("/ACTIVE")
                    && !path.ends_with("\\ACTIVE")
            })
            .collect::<Vec<_>>();
        assert_eq!(after_control, before_control);
        drift.assert_exclusive_reacquire();

        let same_identity = ManagedOperationsFixture::new("same-byte-identity-drift");
        let managed = same_identity.managed_source_path("a/provider.spx");
        let held = managed.with_extension("held");
        let original = std::fs::read(&managed).unwrap();
        let result = derive_with_hook(&same_identity.root, &same_identity.proposal_path, |point| {
            if point == OperationsDerivePoint::DerivationRendered {
                std::fs::rename(&managed, &held).unwrap();
                std::fs::write(&managed, &original).unwrap();
            }
        });
        assert_eq!(result.err().unwrap()[0].code, "SPX-G153");
        assert_eq!(std::fs::read(&managed).unwrap(), original);
        assert_eq!(std::fs::read(&held).unwrap(), original);
        same_identity.assert_exclusive_reacquire();

        let content_drift = ManagedOperationsFixture::new("managed-content-drift");
        let managed = content_drift.managed_source_path("a/provider.spx");
        let result = derive_with_hook(&content_drift.root, &content_drift.proposal_path, |point| {
            if point == OperationsDerivePoint::DerivationRendered {
                std::fs::write(&managed, b"module changed.managed;\n").unwrap();
            }
        });
        assert_eq!(result.err().unwrap()[0].code, "SPX-G153");
        content_drift.assert_exclusive_reacquire();

        let staging_drift = ManagedOperationsFixture::new("staging-inventory-drift");
        let foreign = staging_drift
            .root
            .join(".semaprax-workspace/staging/foreign");
        let result = derive_with_hook(&staging_drift.root, &staging_drift.proposal_path, |point| {
            if point == OperationsDerivePoint::DerivationRendered {
                std::fs::create_dir(&foreign).unwrap();
            }
        });
        assert_eq!(result.err().unwrap()[0].code, "SPX-G153");
        assert!(foreign.is_dir());
        staging_drift.assert_exclusive_reacquire();
    }

    #[test]
    fn canonical_order_binding_and_path_cardinality_fail_closed() {
        let (base, proposal) = fixture();
        let reversed = proposal.replace(
            "{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\"",
            "{\"kind\":\"rename_declaration\",\"path\":\"z/provider.spx\"",
        );
        assert_eq!(parse_proposal(&reversed).err().unwrap()[0].code, "SPX-G196");
        let stale = proposal.replace(
            "\"target_id\":\"ops.answer\"",
            "\"target_id\":\"ops.missing\"",
        );
        assert_eq!(
            prepare_owned(&stale, base).err().unwrap()[0].code,
            "SPX-G197"
        );
    }

    #[test]
    fn typed_occurrence_sidecar_covers_constructor_arguments_and_nested_patterns() {
        let first = canonical(
            "a/first.spx",
            r#"module ops.first;
@id("ops.first.inner") record Inner { @id("ops.first.inner.value") value: i64, }
@id("ops.first.outer") record Outer { @id("ops.first.outer.inner") inner: Inner, }
@id("ops.first.holder") record Holder<T> { @id("ops.first.holder.value") value: T, }
@id("ops.first.use") fn use(input: Outer)->i64{let wrapped=Holder<i64>{value:1};match input{Outer{inner:Inner{value}}=>value+wrapped.value,}}"#,
        );
        let second = canonical(
            "b/second.spx",
            r#"module ops.second;
@id("ops.second.leaf") record Leaf { @id("ops.second.leaf.value") value: i64, }
@id("ops.second.outer") record Outer { @id("ops.second.outer.leaf") leaf: Leaf, }
@id("ops.second.holder") record Holder<T> { @id("ops.second.holder.value") value: T, }
@id("ops.second.use") fn use(input: Outer)->i64{let wrapped=Holder<i64>{value:1};match input{Outer{leaf:Leaf{value}}=>value+wrapped.value,}}"#,
        );
        let paths = vec!["a/first.spx".to_owned(), "b/second.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: first,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: second,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.first\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/first.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.first.inner\",\"from\":\"Inner\",\"to\":\"Core\"}},{{\"kind\":\"rename_declaration\",\"path\":\"b/second.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.second.leaf\",\"from\":\"Leaf\",\"to\":\"Node\"}}]}}\n",
            base.workspace_revision(),
        );
        let prepared = prepare_owned(&proposal, base).unwrap();
        let first = prepared.candidate_sources()[0].source();
        assert!(first.contains("record Core"));
        assert!(first.contains("Holder<i64>"));
        assert!(first.contains("inner: Core"));
        assert!(first.contains("inner: Core {"));
        let second = prepared.candidate_sources()[1].source();
        assert!(second.contains("record Node"));
        assert!(second.contains("Holder<i64>"));
        assert!(second.contains("leaf: Node"));
        assert!(second.contains("leaf: Node {"));
    }

    #[test]
    fn all_admitted_subjects_and_alias_occurrences_are_exact_without_textual_capture() {
        let (base, proposal) = broad_fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        for (caller, expected) in [("ops.consumer.preserve", 2), ("ops.consumer.shadow", 0)] {
            assert_eq!(
                prepared
                    .base_graph()
                    .edges()
                    .iter()
                    .filter(|edge| {
                        edge.kind() == "type_reference"
                            && edge.caller() == caller
                            && edge.target() == "ops.point"
                    })
                    .count(),
                expected
            );
        }
        assert_eq!(prepared.operations_len(), 8);
        assert_eq!(prepared.proposal_source(), proposal);
        assert_eq!(parse_proposal(&proposal).unwrap().source, proposal);
        assert_eq!(
            semantic_workspace_change::parse_proposal(prepared.derived_change_proposal())
                .unwrap()
                .source(),
            prepared.derived_change_proposal()
        );
        assert_eq!(
            prepared
                .edits()
                .iter()
                .map(|edit| edit.operation_index)
                .collect::<BTreeSet<_>>(),
            (0..8).collect::<BTreeSet<_>>()
        );
        assert!(prepared.edits().windows(2).all(|pair| {
            (
                &pair[0].path,
                pair[0].start,
                pair[0].end,
                pair[0].operation_index,
            ) < (
                &pair[1].path,
                pair[1].start,
                pair[1].end,
                pair[1].operation_index,
            )
        }));

        let provider = &prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path() == "a/provider.spx")
            .unwrap()
            .source();
        for expected in [
            "resource Handle",
            "record Metric",
            "variant Outcome",
            "interface Runtime",
            "fn compute",
            "fn identity<T>",
            "identity<i64>(value)",
            "token: own Handle",
            "value: Metric",
            "Outcome::Some",
            "compute(Metric",
        ] {
            assert!(
                provider.contains(expected),
                "missing `{expected}` in {provider}"
            );
        }
        assert!(provider.contains("permit { audit.read }"));
        assert!(provider.contains("uses { audit.read }"));

        let consumer = &prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path() == "b/consumer.spx")
            .unwrap()
            .source();
        for expected in [
            "from ops.provider as Metric",
            "from ops.provider as compute",
            "fn run(value: Metric)",
            "fn preserve(value: Metric) -> Metric",
            "Wrapper { point: value }",
            "compute(nested)",
            "run(Metric { value: 0 })",
        ] {
            assert!(
                consumer.contains(expected),
                "missing `{expected}` in {consumer}"
            );
        }
        for unrelated in [
            "Point: i64",
            "fn shadow<Point>(Point: Point) -> Point",
            "fn local(provider: i64)",
            "let Point = provider",
            "module ops.consumer",
        ] {
            assert!(
                consumer.contains(unrelated),
                "textual capture changed `{unrelated}`"
            );
        }
    }

    #[test]
    fn one_of_two_effectful_imports_changes_only_its_exact_call_family() {
        let provider = canonical(
            "a/provider.spx",
            r#"module selective.provider;
permit { audit.one, audit.two }
@id("selective.first") fn first()->i64 uses { audit.one } { 1 }
@id("selective.second") fn second()->i64 uses { audit.two } { 2 }
@id("selective.provider.main") fn main()->i64 { 0 }
"#,
        );
        let consumer = canonical(
            "b/consumer.spx",
            r#"module selective.consumer;
use function @id("selective.first") from selective.provider as first;
use function @id("selective.second") from selective.provider as second;
permit { audit.one, audit.two }
@id("selective.consumer.main") fn main()->i64 uses { audit.one, audit.two } { first() + second() }
"#,
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"selective.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"selective.first\",\"from\":\"first\",\"to\":\"chosen\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"selective.first\",\"target_module\":\"selective.provider\",\"from\":\"first\",\"to\":\"chosen\"}}]}}\n",
            base.workspace_revision(),
        );
        let prepared = prepare_owned(&proposal, base).unwrap();
        let selected = |edge: &&workspace_graph::WorkspaceEdge| {
            matches!(edge.kind(), "call" | "effect_requirement")
                && matches!(edge.target(), "selective.first" | "audit.one")
        };
        let untouched = |edge: &&workspace_graph::WorkspaceEdge| {
            matches!(edge.kind(), "call" | "effect_requirement")
                && matches!(edge.target(), "selective.second" | "audit.two")
        };
        let base_selected = prepared
            .base_graph()
            .edges()
            .iter()
            .filter(selected)
            .collect::<Vec<_>>();
        let candidate_selected = prepared
            .candidate_graph()
            .edges()
            .iter()
            .filter(selected)
            .collect::<Vec<_>>();
        assert_eq!(base_selected.len(), 2);
        assert_eq!(candidate_selected.len(), 2);
        assert!(base_selected.iter().all(|edge| edge.alias() == "first"));
        assert!(candidate_selected
            .iter()
            .all(|edge| edge.alias() == "chosen"));
        assert_eq!(
            prepared
                .base_graph()
                .edges()
                .iter()
                .filter(untouched)
                .collect::<Vec<_>>(),
            prepared
                .candidate_graph()
                .edges()
                .iter()
                .filter(untouched)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn trailing_as_keyword_and_alias_do_not_capture_the_module_segment() {
        let provider = canonical(
            "a/provider.spx",
            "module ops.as; @id(\"ops.as.answer\") fn answer()->i64{1}",
        );
        let consumer = canonical(
            "b/consumer.spx",
            "module ops.consumer; use function @id(\"ops.as.answer\") from ops.as as as; @id(\"ops.consumer.main\") fn main()->i64{as()}",
        );
        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: provider,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: consumer,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.as.answer\",\"from\":\"answer\",\"to\":\"response\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.as.answer\",\"target_module\":\"ops.as\",\"from\":\"as\",\"to\":\"invoke\"}}]}}\n",
            base.workspace_revision(),
        );
        let prepared = prepare_owned(&proposal, base).unwrap();
        let consumer = &prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path() == "b/consumer.spx")
            .unwrap()
            .source();
        assert!(consumer.contains("from ops.as as invoke"));
        assert!(consumer.contains("invoke()"));
        assert!(prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path() == "a/provider.spx")
            .unwrap()
            .source()
            .contains("module ops.as"));
        assert_eq!(
            prepared
                .edits()
                .iter()
                .filter(|edit| edit.operation_index == 1)
                .count(),
            2
        );
    }

    #[test]
    fn grammar_binding_conflict_and_limit_matrix_is_exact() {
        let (_base, proposal) = fixture();
        assert_eq!(parse_proposal(&proposal).unwrap().operations.len(), 2);
        for hostile in [
            proposal.trim_end_matches('\n').to_owned(),
            proposal.replace('\n', "\r\n"),
            format!("\u{feff}{proposal}"),
            proposal.replacen("{\"schema\":", "{\"extra\":0,\"schema\":", 1),
            proposal.replacen(
                "{\"kind\":\"rename_declaration\"",
                "{\"to\":\"response\",\"kind\":\"rename_declaration\"",
                1,
            ),
            proposal.replacen("a/provider.spx", "../provider.spx", 1),
            proposal.replacen("ops.provider", "ops..provider", 1),
            proposal.replacen("ops.consumer", "ops/consumer", 1),
            proposal.replacen("sha256:", "SHA256:", 1),
        ] {
            assert_eq!(parse_proposal(&hostile).err().unwrap()[0].code, "SPX-G196");
        }
        let one = proposal.replacen(
            ",{\"kind\":\"rename_import_alias\"",
            "]}\n__cut__{\"kind\":\"rename_import_alias\"",
            1,
        );
        let one = format!("{}\n", one.split("\n__cut__").next().unwrap());
        let diagnostics = parse_proposal(&one).err().unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G196");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations requires 2..64 operations"
        );
        let one_path =
            proposal.replace("\"path\":\"b/consumer.spx\"", "\"path\":\"a/provider.spx\"");
        let diagnostics = parse_proposal(&one_path).err().unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G198");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations require 2..16 distinct affected managed paths"
        );

        for (needle, replacement, expected) in [
            (
                "\"target_id\":\"ops.answer\"",
                "\"target_id\":\"ops.missing\"",
                "SPX-G197",
            ),
            ("\"from\":\"answer\"", "\"from\":\"missing\"", "SPX-G197"),
            (
                "\"target_module\":\"ops.provider\"",
                "\"target_module\":\"ops.other\"",
                "SPX-G197",
            ),
            ("\"to\":\"response\"", "\"to\":\"answer\"", "SPX-G198"),
        ] {
            let (fresh, _) = fixture();
            assert_eq!(
                code(prepare_owned(&proposal.replace(needle, replacement), fresh)),
                expected
            );
        }
        let (fresh, _) = fixture();
        let collision = proposal.replace("\"to\":\"response\"", "\"to\":\"main\"");
        assert_eq!(code(prepare_owned(&collision, fresh)), "SPX-G198");

        let long_id = "x".repeat(MAX_TARGET_ID_BYTES + 1);
        assert_eq!(
            parse_proposal(&proposal.replacen("ops.answer", &long_id, 1))
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds target_id_bytes maximum 4096"
        );
        let long_name = "x".repeat(MAX_NAME_BYTES + 1);
        assert_eq!(
            parse_proposal(&proposal.replacen("response", &long_name, 1))
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds name_bytes maximum 128"
        );

        let revision = parse_proposal(&proposal).unwrap().base_workspace_revision;
        let rows = (0..65)
            .map(|index| {
                format!(
                    "{{\"kind\":\"rename_declaration\",\"path\":\"{index:02}/module.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.limit.{index:02}\",\"from\":\"f{index:02}\",\"to\":\"g{index:02}\"}}"
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let over_operations = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"ops.limit\",\"operations\":[{rows}]}}\n"
        );
        assert_eq!(
            parse_proposal(&over_operations).err().unwrap()[0].message,
            "Semantic Workspace Operations exceeds operations maximum 64"
        );
    }

    #[test]
    fn affected_path_cardinality_accepts_sixteen_and_rejects_seventeen() {
        let mut paths = Vec::new();
        let mut sources = Vec::new();
        let mut operations = Vec::new();
        for index in 0..MAX_AFFECTED_PATHS {
            let path = format!("{index:02}/module.spx");
            paths.push(path.clone());
            sources.push(semantic_workspace::SemanticWorkspaceSource {
                path: path.clone(),
                source: canonical(
                    &path,
                    &format!(
                        "module exact.path{index:02}; @id(\"exact.path{index:02}.target\") fn target()->i64{{{index}}} @id(\"exact.path{index:02}.main\") fn main()->i64{{target()}}"
                    ),
                ),
            });
            operations.push(format!(
                "{{\"kind\":\"rename_declaration\",\"path\":\"{path}\",\"declaration_kind\":\"function\",\"target_id\":\"exact.path{index:02}.target\",\"from\":\"target\",\"to\":\"renamed\"}}"
            ));
        }
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            sources,
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"exact.path00\",\"operations\":[{}]}}\n",
            base.workspace_revision(),
            operations.join(",")
        );
        let revision = base.workspace_revision().to_owned();
        let prepared = prepare_owned(&proposal, base).unwrap();
        assert_eq!(prepared.operations_len(), MAX_AFFECTED_PATHS);
        assert_eq!(prepared.candidate_sources().len(), MAX_AFFECTED_PATHS);

        operations.push(String::from(
            "{\"kind\":\"rename_declaration\",\"path\":\"16/module.spx\",\"declaration_kind\":\"function\",\"target_id\":\"exact.path16.target\",\"from\":\"target\",\"to\":\"renamed\"}",
        ));
        let over_paths = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"exact.path00\",\"operations\":[{}]}}\n",
            operations.join(",")
        );
        assert_eq!(
            parse_proposal(&over_paths).err().unwrap()[0].message,
            "Semantic Workspace Operations exceeds affected_paths maximum 16"
        );
    }

    #[test]
    fn automatic_compiler_and_unsupported_targets_are_not_addressable() {
        let first = canonical(
            "a/first.spx",
            "module ops.first; fn automatic()->i64{1} @id(\"ops.first.main\") fn main()->i64{automatic()}",
        );
        let second = canonical(
            "b/second.spx",
            "module ops.second; @id(\"ops.second.record\") record Record { @id(\"ops.second.field\") field:i64, } @id(\"ops.second.main\") fn main()->i64{0}",
        );
        let paths = vec!["a/first.spx".to_owned(), "b/second.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let make = || {
            semantic_workspace::preflight_owned_for_operations(
                &path_set,
                vec![
                    semantic_workspace::SemanticWorkspaceSource {
                        path: paths[0].clone(),
                        source: first.clone(),
                    },
                    semantic_workspace::SemanticWorkspaceSource {
                        path: paths[1].clone(),
                        source: second.clone(),
                    },
                ],
                MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
                MAX_OPERATIONS_BUILDER_BYTES,
            )
            .unwrap()
        };
        let revision = make().workspace_revision().to_owned();
        for (kind, id, from) in [
            ("function", "auto:ops.first.automatic", "automatic"),
            ("record", "ops.second.field", "field"),
            ("record", crate::prelude::OPTION_ID, "Option"),
        ] {
            let proposal = format!(
                "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"ops.first\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/first.spx\",\"declaration_kind\":\"{kind}\",\"target_id\":\"{id}\",\"from\":\"{from}\",\"to\":\"changed\"}},{{\"kind\":\"rename_declaration\",\"path\":\"b/second.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.second.main\",\"from\":\"main\",\"to\":\"entry\"}}]}}\n"
            );
            assert_eq!(code(prepare_owned(&proposal, make())), "SPX-G197");
        }
    }

    #[test]
    fn candidate_namespace_checks_include_automatic_declarations() {
        let first = canonical(
            "a/first.spx",
            "module ops.first; fn occupied()->i64{1} @id(\"ops.first.selected\") fn selected()->i64{2}",
        );
        let second = canonical(
            "b/second.spx",
            "module ops.second; record Occupied { value:i64, } @id(\"ops.second.selected\") record Selected { @id(\"ops.second.selected.value\") value:i64, } @id(\"ops.second.main\") fn main()->i64{0}",
        );
        let paths = vec!["a/first.spx".to_owned(), "b/second.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let base = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: first,
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: second,
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.second\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/first.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.first.selected\",\"from\":\"selected\",\"to\":\"occupied\"}},{{\"kind\":\"rename_declaration\",\"path\":\"b/second.spx\",\"declaration_kind\":\"record\",\"target_id\":\"ops.second.selected\",\"from\":\"Selected\",\"to\":\"Occupied\"}}]}}\n",
            base.workspace_revision(),
        );
        let diagnostics = prepare_owned(&proposal, base).err().unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G198");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations candidate name namespace conflicts"
        );
    }

    fn dense_fixture() -> (semantic_workspace::SemanticWorkspacePreflight, String) {
        let mut provider = String::from("module ops.dense;\n");
        let mut consumer = String::from("module ops.late;\n");
        let mut operations = Vec::new();
        for index in 0..32 {
            provider.push_str(&format!(
                "@id(\"ops.a{index:02}\") fn f{index:02}()->i64{{{index}}}\n"
            ));
            consumer.push_str(&format!(
                "use function @id(\"ops.a{index:02}\") from ops.dense as f{index:02};\n"
            ));
            operations.push(format!(
                "{{\"kind\":\"rename_declaration\",\"path\":\"a/dense.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.a{index:02}\",\"from\":\"f{index:02}\",\"to\":\"g{index:02}\"}}"
            ));
        }
        provider.push_str("@id(\"ops.dense.main\") fn main()->i64{0}\n");
        for index in 0..32 {
            consumer.push_str(&format!(
                "@id(\"ops.b.caller{index:02}\") fn caller{index:02}()->i64{{f{index:02}()}}\n"
            ));
            operations.push(format!(
                "{{\"kind\":\"rename_import_alias\",\"path\":\"b/late.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.a{index:02}\",\"target_module\":\"ops.dense\",\"from\":\"f{index:02}\",\"to\":\"g{index:02}\"}}"
            ));
        }
        consumer.push_str("@id(\"ops.late.main\") fn main()->i64{0}\n");
        let paths = vec!["a/dense.spx".to_owned(), "b/late.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let preflight = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            vec![
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[0].clone(),
                    source: canonical(&paths[0], &provider),
                },
                semantic_workspace::SemanticWorkspaceSource {
                    path: paths[1].clone(),
                    source: canonical(&paths[1], &consumer),
                },
            ],
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let proposal = format!(
            "{{\"schema\":\"{SCHEMA}\",\"base_workspace_revision\":\"{}\",\"entry_module\":\"ops.late\",\"operations\":[{}]}}\n",
            preflight.workspace_revision(),
            operations.join(",")
        );
        (preflight, proposal)
    }

    #[test]
    fn dense_sixty_four_operation_late_edits_are_one_pass_and_exactly_bounded() {
        let (base, proposal) = dense_fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        assert_eq!(prepared.operations_len(), MAX_OPERATIONS);
        assert!(prepared.edits().len() > MAX_OPERATIONS);
        let late = prepared
            .candidate_sources()
            .iter()
            .find(|source| source.path() == "b/late.spx")
            .unwrap();
        assert!(late.source().contains("as g31"));
        assert!(late.source().contains("g31()"));
        assert!((0..32).all(|index| late.source().contains(&format!("g{index:02}()"))));
        assert_eq!(
            prepared
                .base_graph()
                .edges()
                .iter()
                .filter(|edge| edge.kind() == "call")
                .count(),
            32
        );
        assert_eq!(
            prepared
                .candidate_graph()
                .edges()
                .iter()
                .filter(|edge| edge.kind() == "call")
                .count(),
            32
        );
        let used = prepared.used_operations_builder_bytes();
        let (base, proposal) = dense_fixture();
        assert_eq!(
            prepare_owned_with_limit(&proposal, base, used - 1)
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds operations_builder_bytes maximum 67108864"
        );
        let (base, proposal) = dense_fixture();
        assert!(prepare_owned_with_limit(&proposal, base, used).is_ok());
    }

    #[test]
    fn derived_changed_source_cap_helper_is_exact_and_rejects_one_over() {
        reset_candidate_preflight_entry_count();
        let (base, proposal) = fixture();
        assert!(prepare_owned(&proposal, base).is_ok());
        assert_eq!(candidate_preflight_entry_count(), 1);

        reset_candidate_preflight_entry_count();
        let original = "a".repeat(MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH);
        let exact_edit = PlannedEditFact {
            path: "a/large.spx".to_owned(),
            start: original.len() - 1,
            end: original.len(),
            replacement: "z".to_owned(),
            operation_index: 0,
        };
        let rendered = render_candidate_source(&original, &[&exact_edit]).unwrap();
        assert_eq!(rendered.len(), MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH);
        assert!(validate_replacement_source_per_path(&rendered).is_ok());
        assert_eq!(candidate_preflight_entry_count(), 0);

        let one_over_edit = PlannedEditFact {
            path: "a/large.spx".to_owned(),
            start: original.len() - 1,
            end: original.len(),
            replacement: "zz".to_owned(),
            operation_index: 0,
        };
        let rendered = render_candidate_source(&original, &[&one_over_edit]).unwrap();
        assert_eq!(rendered.len(), MAX_REPLACEMENT_SOURCE_BYTES_PER_PATH + 1);
        let diagnostics = validate_replacement_source_per_path(&rendered)
            .err()
            .unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G199");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations exceeds replacement_source_bytes_per_path maximum 1048576"
        );
        assert_eq!(candidate_preflight_entry_count(), 0);
    }

    #[test]
    fn candidate_replay_rejects_unrelated_semantic_mutation() {
        let (base, proposal) = fixture();
        let prepared = prepare_owned(&proposal, base).unwrap();
        let mut candidate_sources = prepared
            .candidate_sources()
            .iter()
            .map(|source| semantic_workspace::SemanticWorkspaceSource {
                path: source.path().to_owned(),
                source: source.source().to_owned(),
            })
            .collect::<Vec<_>>();
        let consumer = candidate_sources
            .iter_mut()
            .find(|source| source.path == "b/consumer.spx")
            .unwrap();
        let prior = consumer.source.clone();
        assert!(consumer.source.contains("@id(\"ops.main\")"));
        consumer.source =
            consumer
                .source
                .replacen("@id(\"ops.main\")", "@id(\"ops.tampered.main\")", 1);
        assert_ne!(consumer.source, prior);

        let paths = vec!["a/provider.spx".to_owned(), "b/consumer.spx".to_owned()];
        let path_set = semantic_workspace::render_path_set(&paths).unwrap();
        let candidate = semantic_workspace::preflight_owned_for_operations(
            &path_set,
            candidate_sources,
            MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
            MAX_OPERATIONS_BUILDER_BYTES,
        )
        .unwrap();
        let (_, _, _, candidate_build) = candidate.into_snapshot_parts();
        let candidate_view = candidate_build.into_operation_view().unwrap();

        let (fresh, _) = fixture();
        let (_, _, _, base_build) = fresh.into_snapshot_parts();
        let base_view = base_build.into_operation_view().unwrap();
        let operations = parse_proposal(&proposal).unwrap().operations;
        assert_eq!(
            replay_candidate(&base_view, &candidate_view, &operations)
                .err()
                .unwrap()[0]
                .code,
            "SPX-G200"
        );
    }

    #[test]
    fn operations_builder_limit_is_exact_and_cannot_exceed_production() {
        let (base, proposal) = fixture();
        let used = prepare_owned(&proposal, base)
            .unwrap()
            .used_operations_builder_bytes();
        let (base, proposal) = fixture();
        assert_eq!(
            prepare_owned_with_limit(&proposal, base, used - 1)
                .err()
                .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds operations_builder_bytes maximum 67108864"
        );
        let (base, proposal) = fixture();
        assert!(prepare_owned_with_limit(&proposal, base, used).is_ok());
    }

    #[test]
    #[should_panic(expected = "cannot exceed the production maximum")]
    fn operations_builder_test_seam_cannot_expand_authority() {
        let (base, proposal) = fixture();
        let _ = prepare_owned_with_limit(&proposal, base, MAX_OPERATIONS_BUILDER_BYTES + 1);
    }

    #[test]
    fn dense_late_edits_stream_once_and_fail_before_tiny_capacity() {
        let original = "a".repeat(20_000);
        let facts = (0..10_000)
            .map(|index| PlannedEditFact {
                path: "a/dense.spx".to_owned(),
                start: index * 2 + 1,
                end: index * 2 + 2,
                replacement: "expanded".to_owned(),
                operation_index: 0,
            })
            .collect::<Vec<_>>();
        let edits = facts.iter().collect::<Vec<_>>();
        let expected_len = 20_000 + 10_000 * 7;
        let (rendered, overflowed, used) =
            crate::bounded_output::with_limit_usage(expected_len, || {
                render_candidate_source(&original, &edits)
            });
        assert!(!overflowed);
        assert_eq!(used, expected_len);
        assert_eq!(rendered.unwrap().len(), expected_len);
        let (rejected, overflowed, _) =
            crate::bounded_output::with_limit_usage(expected_len - 1, || {
                render_candidate_source(&original, &edits)
            });
        assert!(overflowed);
        assert_eq!(rejected.err().unwrap()[0].code, "SPX-G199");
    }

    fn raw_sha256(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(hasher.finalize())
        )
    }

    fn directory_names(path: &Path) -> Vec<String> {
        let mut names = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn spawn_operations_apply_process(
        fixture: &ManagedOperationsFixture,
        evidence_path: &Path,
        boundary: &str,
    ) -> (Child, PathBuf) {
        let ready = fixture
            .root
            .join(format!("operations-apply-{boundary}.ready"));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "semantic_workspace_operations::tests::operations_apply_process_child",
                "--nocapture",
            ])
            .env("SEMAPRAX_OPERATIONS_APPLY_CHILD", "1")
            .env("SEMAPRAX_OPERATIONS_APPLY_ROOT", &fixture.root)
            .env("SEMAPRAX_OPERATIONS_APPLY_PROPOSAL", &fixture.proposal_path)
            .env("SEMAPRAX_OPERATIONS_APPLY_EVIDENCE", evidence_path)
            .env("SEMAPRAX_OPERATIONS_APPLY_BOUNDARY", boundary)
            .env("SEMAPRAX_OPERATIONS_APPLY_READY", &ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !matches!(std::fs::read(&ready), Ok(bytes) if bytes == b"ready\n") {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("Operations apply child exited before {boundary}: {status}");
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("Operations apply child did not reach {boundary}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        (child, ready)
    }

    #[test]
    fn operations_evidence_and_verification_are_exact_one_build_kats() {
        let fixture = ManagedOperationsFixture::new("evidence-kat");
        reset_base_operations_preflight_entry_count();
        reset_candidate_preflight_entry_count();
        let before = fixture.inventory();
        let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
        assert_eq!(base_operations_preflight_entry_count(), 1);
        assert_eq!(candidate_preflight_entry_count(), 1);
        assert_eq!(fixture.inventory(), before);
        assert!(artifacts.operations_evidence().ends_with('\n'));
        assert!(
            !artifacts.operations_evidence()[..artifacts.operations_evidence().len() - 1]
                .contains('\n')
        );
        assert!(artifacts
            .operations_evidence_digest()
            .starts_with("sha256:"));
        assert_eq!(
            raw_sha256(artifacts.workspace_change_evidence().as_bytes()),
            "sha256:a597228f058936733af2fc8a813f4a07e6aa5e19decdb65c68f6d229dd5f5768"
        );
        assert_eq!(
            raw_sha256(artifacts.operations_evidence().as_bytes()),
            "sha256:d26cd43db24e551b122a972d2b67ce4a797b8b6ced6726d4d1d3533638659056"
        );
        assert_eq!(
            artifacts.operations_proposal_digest(),
            proposal_digest(&fixture.proposal_source)
        );
        assert!(artifacts
            .derived_change_proposal_digest()
            .starts_with("sha256:"));

        let evidence_path = fixture.root.join("operations-evidence.json");
        std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
        reset_base_operations_preflight_entry_count();
        reset_candidate_preflight_entry_count();
        let receipt = verify(&fixture.root, &fixture.proposal_path, &evidence_path).unwrap();
        assert_eq!(base_operations_preflight_entry_count(), 1);
        assert_eq!(candidate_preflight_entry_count(), 1);
        assert_eq!(fixture.inventory(), {
            let mut expected = before;
            expected.push((
                "operations-evidence.json".to_owned(),
                false,
                artifacts.operations_evidence().as_bytes().to_vec(),
            ));
            expected.sort_by(|left, right| left.0.cmp(&right.0));
            expected
        });
        assert_eq!(
            raw_sha256(receipt.as_bytes()),
            "sha256:ca90733e0cb36c8f489418abfd8eebf6582b2066a019e4002817a378abc44a7c"
        );
        let value: Value = serde_json::from_str(receipt.trim_end()).unwrap();
        assert_eq!(
            value["schema"],
            evidence_artifact::VERIFICATION_RECEIPT_SCHEMA
        );
        assert_eq!(value["result"], "exact_replay");
        assert_eq!(
            value["budget"]["used_receipt_bytes"].as_u64().unwrap() as usize,
            receipt.len()
        );
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn operations_evidence_and_receipt_individual_and_aggregate_caps_are_exact() {
        let (prepared, change) = evidence_render_fixture();
        let full = evidence_artifact::render_evidence(&prepared, &change).unwrap();
        let evidence_bytes = full.operations_evidence().len();
        assert!(evidence_artifact::render_evidence_with_test_limits(
            &prepared,
            &change,
            evidence_bytes,
            evidence_artifact::MAX_TOTAL_BYTES,
        )
        .is_ok());
        assert_eq!(
            evidence_artifact::render_evidence_with_test_limits(
                &prepared,
                &change,
                evidence_bytes - 1,
                evidence_artifact::MAX_TOTAL_BYTES,
            )
            .err()
            .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds operations_evidence_bytes maximum 4194304"
        );

        let mut low = 0usize;
        let mut high = evidence_artifact::MAX_TOTAL_BYTES;
        while low < high {
            let middle = low + (high - low) / 2;
            if evidence_artifact::render_evidence_with_test_limits(
                &prepared,
                &change,
                evidence_artifact::MAX_OPERATIONS_EVIDENCE_BYTES,
                middle,
            )
            .is_ok()
            {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert!(evidence_artifact::render_evidence_with_test_limits(
            &prepared,
            &change,
            evidence_artifact::MAX_OPERATIONS_EVIDENCE_BYTES,
            low,
        )
        .is_ok());
        assert_eq!(
            evidence_artifact::render_evidence_with_test_limits(
                &prepared,
                &change,
                evidence_artifact::MAX_OPERATIONS_EVIDENCE_BYTES,
                low - 1,
            )
            .err()
            .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds total_operations_artifact_bytes maximum 150994944"
        );

        let replay = evidence_artifact::exact_replay_token(
            &prepared,
            &change,
            &full,
            full.operations_evidence(),
        )
        .unwrap();
        let receipt = evidence_artifact::render_receipt(&prepared, &full, &replay, false).unwrap();
        assert!(evidence_artifact::render_receipt_with_test_limits(
            &prepared,
            &full,
            &replay,
            false,
            receipt.len(),
            evidence_artifact::MAX_TOTAL_BYTES,
        )
        .is_ok());
        assert_eq!(
            evidence_artifact::render_receipt_with_test_limits(
                &prepared,
                &full,
                &replay,
                false,
                receipt.len() - 1,
                evidence_artifact::MAX_TOTAL_BYTES,
            )
            .err()
            .unwrap()[0]
                .message,
            "Semantic Workspace Operations exceeds receipt_bytes maximum 65536"
        );
    }

    #[test]
    #[should_panic]
    fn operations_evidence_cap_seam_cannot_expand_production_authority() {
        let (prepared, change) = evidence_render_fixture();
        let _ = evidence_artifact::render_evidence_with_test_limits(
            &prepared,
            &change,
            evidence_artifact::MAX_OPERATIONS_EVIDENCE_BYTES + 1,
            evidence_artifact::MAX_TOTAL_BYTES,
        );
    }

    #[test]
    #[should_panic]
    fn operations_receipt_cap_seam_cannot_expand_production_authority() {
        let (prepared, change) = evidence_render_fixture();
        let full = evidence_artifact::render_evidence(&prepared, &change).unwrap();
        let replay = evidence_artifact::exact_replay_token(
            &prepared,
            &change,
            &full,
            full.operations_evidence(),
        )
        .unwrap();
        let _ = evidence_artifact::render_receipt_with_test_limits(
            &prepared,
            &full,
            &replay,
            false,
            evidence_artifact::MAX_RECEIPT_BYTES + 1,
            evidence_artifact::MAX_TOTAL_BYTES,
        );
    }

    #[test]
    fn operations_evidence_parser_and_exact_replay_fail_closed() {
        let fixture = ManagedOperationsFixture::new("evidence-hostile");
        let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
        let canonical = artifacts.operations_evidence();
        let depth8: Value = serde_json::from_str("[[[[[[[0]]]]]]]").unwrap();
        let depth9: Value = serde_json::from_str("[[[[[[[[0]]]]]]]]").unwrap();
        assert_eq!(evidence_verification::json_depth(&depth8), 8);
        assert_eq!(evidence_verification::json_depth(&depth9), 9);
        let parsed = evidence_verification::parse_evidence(canonical).unwrap();
        evidence_verification::verify_replay(
            &parsed,
            canonical,
            canonical,
            artifacts.workspace_change_evidence(),
        )
        .unwrap();

        for (hostile, expected) in [
            (canonical.trim_end().to_owned(), "SPX-G201"),
            (format!("\u{feff}{canonical}"), "SPX-G201"),
            (canonical.replace("\n", "\r\n"), "SPX-G201"),
            (
                canonical.replacen("\"schema\":", "\"extra\":0,\"schema\":", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen("\"schema\":", "\"schema\":0,\"schema_copy\":", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"schema\":",
                    "\"schema\":\"duplicate\",\"schema\":",
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    evidence_artifact::EVIDENCE_SCHEMA,
                    evidence_artifact::VERIFICATION_RECEIPT_SCHEMA,
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen("\"entry_module\":\"ops.consumer\",", "", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen("\"bytes\":509}", "\"bytes\":509,\"extra\":0}", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"schema\":\"semaprax.semantic-workspace-operations.v1\",\"digest\":",
                    "\"digest\":\"sha256:3c7bf340a5313907edcec41748063e8666793ee76b903bc4e691871a843544b5\",\"schema\":\"semaprax.semantic-workspace-operations.v1\",\"bytes\":509,\"digest\":",
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen("\"bytes\":509", "\"bytes\":\"509\"", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"schema\":\"semaprax.semantic-workspace-operations.v1\",\"digest\":",
                    "\"digest\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"schema\":\"semaprax.semantic-workspace-operations.v1\",\"digest\":",
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen(artifacts.operations_proposal_digest(), "invalid-digest", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"document\":\"",
                    "\"extra\":0,\"document\":\"",
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen("\"max_json_depth\":8,", "", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"max_json_depth\":8,",
                    "\"max_json_depth\":\"8\",",
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen("\"used_staging_attempts\":0,", "", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"used_staging_attempts\":0,",
                    "\"used_staging_attempts\":\"0\",",
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen("\"nonclaims\":[", "\"nonclaims\":{", 1),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"entry_module\":\"ops.consumer\"",
                    "\"entry_module\":[]",
                    1,
                ),
                "SPX-G201",
            ),
            (
                canonical.replacen(
                    "\"operations_proposal\":{",
                    "\"operations_proposal\":[]",
                    1,
                ),
                "SPX-G201",
            ),
            (
                format!(
                    "{{\"schema\":\"{}\",\"result\":\"exact_replay\"}}\n",
                    evidence_artifact::VERIFICATION_RECEIPT_SCHEMA
                ),
                "SPX-G201",
            ),
            (artifacts.workspace_change_evidence().to_owned(), "SPX-G201"),
            ("[[[[[[[[[]]]]]]]]]\n".to_owned(), "SPX-G199"),
        ] {
            assert_eq!(
                evidence_verification::parse_evidence(&hostile)
                    .err()
                    .unwrap()[0]
                    .code,
                expected
            );
        }
        assert_eq!(
            evidence_verification::parse_evidence("[[[[[[[]]]]]]]\n")
                .err()
                .unwrap()[0]
                .code,
            "SPX-G201"
        );

        for hostile in [
            canonical.replacen(
                artifacts.workspace_change_evidence_digest(),
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                1,
            ),
            canonical.replacen(
                &format!("\"bytes\":{}", artifacts.workspace_change_evidence().len()),
                &format!(
                    "\"bytes\":{}",
                    artifacts.workspace_change_evidence().len() + 1
                ),
                1,
            ),
            canonical.replacen(
                "\"used_unexpected_inventory_entries\":0",
                "\"used_unexpected_inventory_entries\":1",
                1,
            ),
        ] {
            assert_eq!(
                evidence_verification::parse_evidence(&hostile)
                    .err()
                    .unwrap()[0]
                    .code,
                "SPX-G202"
            );
        }

        let mut nonclaim_hostile = canonical.to_owned();
        let nonclaim_index = nonclaim_hostile
            .rfind(evidence_artifact::NONCLAIMS[0])
            .unwrap();
        nonclaim_hostile.replace_range(
            nonclaim_index..nonclaim_index + evidence_artifact::NONCLAIMS[0].len(),
            "xot_signature_or_authenticated_provenance",
        );
        for hostile in [
            canonical.replacen(
                "\"max_receipt_bytes\":65536",
                "\"max_receipt_bytes\":65535",
                1,
            ),
            nonclaim_hostile,
        ] {
            let submitted = evidence_verification::parse_evidence(&hostile).unwrap();
            assert_eq!(
                evidence_verification::verify_replay(
                    &submitted,
                    &hostile,
                    canonical,
                    artifacts.workspace_change_evidence(),
                )
                .err()
                .unwrap()[0]
                    .code,
                "SPX-G203"
            );
        }

        let submitted = evidence_verification::parse_evidence(canonical).unwrap();
        let regenerated = canonical.replacen(
            artifacts.operations_proposal_digest(),
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            1,
        );
        assert_eq!(
            evidence_verification::verify_replay(
                &submitted,
                canonical,
                &regenerated,
                artifacts.workspace_change_evidence(),
            )
            .err()
            .unwrap()[0]
                .code,
            "SPX-G203"
        );
    }

    #[test]
    fn operations_verify_io_precedence_is_read_only_and_unlocks() {
        let fixture = ManagedOperationsFixture::new("evidence-io");
        let before = fixture.inventory();
        let missing_proposal = fixture.root.join("missing-operations.json");
        let missing_evidence = fixture.root.join("missing-evidence.json");
        let diagnostics = verify(&fixture.root, &missing_proposal, &missing_evidence)
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I216");
        assert_eq!(fixture.inventory(), before);
        fixture.assert_exclusive_reacquire();

        let malformed_proposal = fixture.root.join("malformed-operations.json");
        std::fs::write(&malformed_proposal, "{}\n").unwrap();
        let diagnostics = verify(&fixture.root, &malformed_proposal, &missing_evidence)
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I217");
        fixture.assert_exclusive_reacquire();

        let malformed_evidence = fixture.root.join("malformed-evidence.json");
        std::fs::write(&malformed_evidence, "{}\n").unwrap();
        let diagnostics = verify(&fixture.root, &malformed_proposal, &malformed_evidence)
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G201");
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn operations_evidence_io_owned_once_and_final_drift_are_exact() {
        let fixture = ManagedOperationsFixture::new("evidence-io-hostiles");
        let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();

        let directory = fixture.root.join("evidence-directory");
        std::fs::create_dir(&directory).unwrap();
        let diagnostics = verify(&fixture.root, &fixture.proposal_path, &directory)
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I217");
        #[cfg(unix)]
        assert_eq!(
            diagnostics[0].message,
            "could not read Semantic Workspace Operations Evidence: input is not a regular file"
        );
        #[cfg(windows)]
        assert_eq!(
            diagnostics[0].message,
            "could not read Semantic Workspace Operations Evidence: open failed"
        );
        fixture.assert_exclusive_reacquire();

        let invalid = fixture.root.join("invalid-evidence.json");
        std::fs::write(&invalid, [0xff]).unwrap();
        let diagnostics = verify(&fixture.root, &fixture.proposal_path, &invalid)
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I217");
        assert_eq!(
            diagnostics[0].message,
            "could not read Semantic Workspace Operations Evidence: input is not UTF-8"
        );
        fixture.assert_exclusive_reacquire();

        let over = fixture.root.join("over-evidence.json");
        let over_file = File::create(&over).unwrap();
        over_file
            .set_len((evidence_artifact::MAX_OPERATIONS_EVIDENCE_BYTES + 1) as u64)
            .unwrap();
        let diagnostics = verify(&fixture.root, &fixture.proposal_path, &over)
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G199");
        assert_eq!(
            diagnostics[0].message,
            "Semantic Workspace Operations exceeds operations_evidence_bytes maximum 4194304"
        );
        fixture.assert_exclusive_reacquire();

        let evidence_path = fixture.root.join("owned-evidence.json");
        std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
        let baseline = verify(&fixture.root, &fixture.proposal_path, &evidence_path).unwrap();
        let owned = verify_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point| {
                if point == OperationsEvidencePoint::EvidenceOwned {
                    std::fs::write(&evidence_path, "{}\n").unwrap();
                    std::fs::write(&fixture.proposal_path, "{}\n").unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(owned, baseline);
        fixture.assert_exclusive_reacquire();

        let drift = ManagedOperationsFixture::new("verify-final-drift");
        let artifacts = generate_evidence(&drift.root, &drift.proposal_path).unwrap();
        let evidence_path = drift.root.join("operations-evidence.json");
        std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
        let managed = drift.managed_source_path("a/provider.spx");
        let diagnostics =
            verify_with_hook(&drift.root, &drift.proposal_path, &evidence_path, |point| {
                if point == OperationsEvidencePoint::ReceiptRendered {
                    OpenOptions::new()
                        .append(true)
                        .open(&managed)
                        .unwrap()
                        .write_all(b"\n")
                        .unwrap();
                }
            })
            .err()
            .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G153");
        drift.assert_exclusive_reacquire();

        let generate_drift = ManagedOperationsFixture::new("generate-final-drift");
        let managed = generate_drift.managed_source_path("a/provider.spx");
        let diagnostics = generate_evidence_with_hook(
            &generate_drift.root,
            &generate_drift.proposal_path,
            |point| {
                if point == OperationsEvidencePoint::EvidenceRendered {
                    OpenOptions::new()
                        .append(true)
                        .open(&managed)
                        .unwrap()
                        .write_all(b"\n")
                        .unwrap();
                }
            },
        )
        .err()
        .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G153");
        generate_drift.assert_exclusive_reacquire();

        let apply_drift = ManagedOperationsFixture::new("apply-receipt-final-drift");
        let artifacts = generate_evidence(&apply_drift.root, &apply_drift.proposal_path).unwrap();
        let evidence_path = apply_drift.root.join("operations-evidence.json");
        std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
        let active_path = apply_drift.root.join(".semaprax-workspace/ACTIVE");
        let old_active = std::fs::read(&active_path).unwrap();
        let managed = apply_drift.managed_source_path("a/provider.spx");
        let diagnostics = apply_with_hook(
            &apply_drift.root,
            &apply_drift.proposal_path,
            &evidence_path,
            |point, _, _, _| {
                if point == OperationsEvidencePoint::ReceiptRendered {
                    OpenOptions::new()
                        .append(true)
                        .open(&managed)?
                        .write_all(b"\n")?;
                }
                Ok(())
            },
        )
        .err()
        .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G153");
        assert_eq!(std::fs::read(active_path).unwrap(), old_active);
        assert_eq!(
            directory_names(&apply_drift.root.join(".semaprax-workspace/generations")).len(),
            1
        );
        assert!(directory_names(&apply_drift.root.join(".semaprax-workspace/staging")).is_empty());
        apply_drift.assert_exclusive_reacquire();
    }

    #[test]
    fn every_operations_public_route_owns_bounded_inputs_and_fails_closed() {
        for route in ["generate", "evidence", "verify", "apply"] {
            for case in ["directory", "utf8", "exact", "over"] {
                let fixture = ManagedOperationsFixture::new(&format!("proposal-io-{route}-{case}"));
                let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
                let evidence_path = fixture.root.join("valid-evidence.json");
                std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
                let hostile = fixture.root.join(format!("hostile-proposal-{case}"));
                match case {
                    "directory" => std::fs::create_dir(&hostile).unwrap(),
                    "utf8" => std::fs::write(&hostile, [0xff]).unwrap(),
                    "exact" | "over" => {
                        let file = File::create(&hostile).unwrap();
                        file.set_len((MAX_PROPOSAL_BYTES + usize::from(case == "over")) as u64)
                            .unwrap();
                    }
                    _ => unreachable!(),
                }
                let before = fixture.inventory();
                let diagnostics = match route {
                    "generate" => generate_evidence(&fixture.root, &hostile).map(|_| ()),
                    "evidence" => evidence(&fixture.root, &hostile).map(|_| ()),
                    "verify" => verify(&fixture.root, &hostile, &evidence_path).map(|_| ()),
                    "apply" => apply(&fixture.root, &hostile, &evidence_path).map(|_| ()),
                    _ => unreachable!(),
                }
                .err()
                .unwrap();
                assert_eq!(
                    diagnostics[0].code,
                    match case {
                        "directory" | "utf8" => "SPX-I216",
                        "exact" => "SPX-G196",
                        "over" => "SPX-G199",
                        _ => unreachable!(),
                    },
                    "proposal route={route} case={case}"
                );
                assert_eq!(fixture.inventory(), before);
                fixture.assert_exclusive_reacquire();
            }
        }

        for route in ["verify", "apply"] {
            for case in ["directory", "utf8", "exact", "over"] {
                let fixture = ManagedOperationsFixture::new(&format!("evidence-io-{route}-{case}"));
                let hostile = fixture.root.join(format!("hostile-evidence-{case}"));
                match case {
                    "directory" => std::fs::create_dir(&hostile).unwrap(),
                    "utf8" => std::fs::write(&hostile, [0xff]).unwrap(),
                    "exact" | "over" => {
                        let file = File::create(&hostile).unwrap();
                        file.set_len(
                            (evidence_artifact::MAX_OPERATIONS_EVIDENCE_BYTES
                                + usize::from(case == "over")) as u64,
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
                let before = fixture.inventory();
                let diagnostics = match route {
                    "verify" => verify(&fixture.root, &fixture.proposal_path, &hostile),
                    "apply" => apply(&fixture.root, &fixture.proposal_path, &hostile),
                    _ => unreachable!(),
                }
                .err()
                .unwrap();
                assert_eq!(
                    diagnostics[0].code,
                    match case {
                        "directory" | "utf8" => "SPX-I217",
                        "exact" => "SPX-G201",
                        "over" => "SPX-G199",
                        _ => unreachable!(),
                    },
                    "Evidence route={route} case={case}"
                );
                assert_eq!(fixture.inventory(), before);
                fixture.assert_exclusive_reacquire();
            }
        }

        let generated_owned = ManagedOperationsFixture::new("generate-owned-once");
        let expected = generate_evidence(&generated_owned.root, &generated_owned.proposal_path)
            .unwrap()
            .operations_evidence()
            .to_owned();
        let output = generate_evidence_with_hook(
            &generated_owned.root,
            &generated_owned.proposal_path,
            |point| {
                if point == OperationsEvidencePoint::ProposalOwned {
                    std::fs::write(&generated_owned.proposal_path, "{}\n").unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(output.operations_evidence(), expected);
        generated_owned.assert_exclusive_reacquire();

        let applied_owned = ManagedOperationsFixture::new("apply-owned-once");
        let artifacts =
            generate_evidence(&applied_owned.root, &applied_owned.proposal_path).unwrap();
        let evidence_path = applied_owned.root.join("owned-evidence.json");
        std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
        let receipt = apply_with_hook(
            &applied_owned.root,
            &applied_owned.proposal_path,
            &evidence_path,
            |point, _, _, _| {
                if point == OperationsEvidencePoint::EvidenceOwned {
                    std::fs::write(&applied_owned.proposal_path, "{}\n")?;
                    std::fs::write(&evidence_path, "{}\n")?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(receipt.trim_end()).unwrap()["result"],
            "applied"
        );
        applied_owned.assert_exclusive_reacquire();
    }

    #[test]
    fn operations_apply_is_exact_stale_and_zero_write_before_replay() {
        let fixture = ManagedOperationsFixture::new("evidence-apply");
        let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
        let evidence_path = fixture.root.join("operations-evidence.json");
        std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
        let raw_provider = std::fs::read(fixture.root.join("a/provider.spx")).unwrap();
        let raw_consumer = std::fs::read(fixture.root.join("b/consumer.spx")).unwrap();
        let receipt = apply(&fixture.root, &fixture.proposal_path, &evidence_path).unwrap();
        assert_eq!(
            raw_sha256(receipt.as_bytes()),
            "sha256:b5994154b42314aa622111f1a7ce3457a5b65ec33de78eff4dfbfb9ca0875e22"
        );
        let value: Value = serde_json::from_str(receipt.trim_end()).unwrap();
        assert_eq!(
            value["schema"],
            evidence_artifact::APPLICATION_RECEIPT_SCHEMA
        );
        assert_eq!(value["result"], "applied");
        assert_eq!(
            std::fs::read(fixture.root.join("a/provider.spx")).unwrap(),
            raw_provider
        );
        assert_eq!(
            std::fs::read(fixture.root.join("b/consumer.spx")).unwrap(),
            raw_consumer
        );
        fixture.assert_exclusive_reacquire();

        let committed = fixture.inventory();
        let stale = apply(&fixture.root, &fixture.proposal_path, &evidence_path)
            .err()
            .unwrap();
        assert_eq!(stale[0].code, "SPX-G197");
        assert_eq!(
            stale[0].message,
            "Semantic Workspace Operations target does not match one explicit user-owned pre-state declaration"
        );
        assert_eq!(fixture.inventory(), committed);
        fixture.assert_exclusive_reacquire();

        let replay_fixture = ManagedOperationsFixture::new("evidence-apply-replay");
        let artifacts =
            generate_evidence(&replay_fixture.root, &replay_fixture.proposal_path).unwrap();
        let evidence_path = replay_fixture.root.join("operations-evidence.json");
        let hostile = artifacts.operations_evidence().replacen(
            artifacts.operations_proposal_digest(),
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            1,
        );
        std::fs::write(&evidence_path, hostile).unwrap();
        let before = replay_fixture.inventory();
        let diagnostics = apply(
            &replay_fixture.root,
            &replay_fixture.proposal_path,
            &evidence_path,
        )
        .err()
        .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-G203");
        assert_eq!(replay_fixture.inventory(), before);
        replay_fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn every_operations_apply_boundary_fails_closed_and_unlocks_exactly() {
        let points = [
            OperationsEvidencePoint::ProposalOwned,
            OperationsEvidencePoint::EvidenceOwned,
            OperationsEvidencePoint::AfterOperationsReplay,
            OperationsEvidencePoint::ChangeArtifactsRendered,
            OperationsEvidencePoint::OperationsEvidenceReplayed,
            OperationsEvidencePoint::ReceiptRendered,
            OperationsEvidencePoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterSlotCreate,
            )),
            OperationsEvidencePoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterManifestWrite,
            )),
            OperationsEvidencePoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterFilesWrite,
            )),
            OperationsEvidencePoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::BeforeStageValidation,
            )),
            OperationsEvidencePoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::BeforeGenerationPublish,
            )),
            OperationsEvidencePoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::DestinationChecked,
            )),
            OperationsEvidencePoint::Workspace(workspace::SemanticChangeApplyPoint::Generation(
                workspace::GenerationPoint::AfterGenerationPublish,
            )),
            OperationsEvidencePoint::Workspace(
                workspace::SemanticChangeApplyPoint::AfterCandidatePrepared,
            ),
            OperationsEvidencePoint::Workspace(
                workspace::SemanticChangeApplyPoint::AfterActiveStaged,
            ),
            OperationsEvidencePoint::Workspace(
                workspace::SemanticChangeApplyPoint::BeforeFirstFinalCheck,
            ),
            OperationsEvidencePoint::Workspace(
                workspace::SemanticChangeApplyPoint::BeforeSecondFinalCheck,
            ),
            OperationsEvidencePoint::Workspace(
                workspace::SemanticChangeApplyPoint::BeforeActiveReplace,
            ),
            OperationsEvidencePoint::Workspace(
                workspace::SemanticChangeApplyPoint::AfterActiveReplace,
            ),
        ];
        for (index, target) in points.into_iter().enumerate() {
            let fixture = ManagedOperationsFixture::new(&format!("apply-boundary-{index}"));
            let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
            let evidence_path = fixture.root.join("operations-evidence.json");
            std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
            let old_revision = crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
                .unwrap()
                .workspace_revision()
                .to_owned();
            let candidate_revision = serde_json::from_str::<Value>(artifacts.operations_evidence())
                .unwrap()["candidate_workspace_revision"]
                .as_str()
                .unwrap()
                .to_owned();
            let raw_provider = std::fs::read(fixture.root.join("a/provider.spx")).unwrap();
            let raw_consumer = std::fs::read(fixture.root.join("b/consumer.spx")).unwrap();
            let mut reached = false;
            let result = apply_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, _, _, _| {
                    if point == target {
                        reached = true;
                        return Err(std::io::Error::other("injected boundary failure"));
                    }
                    Ok(())
                },
            );
            assert!(reached, "boundary {target:?} was not reached");
            let diagnostics = result.err().unwrap();
            assert_eq!(
                diagnostics[0].code,
                if target
                    == OperationsEvidencePoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterActiveReplace,
                    )
                {
                    "SPX-I212"
                } else {
                    "SPX-I211"
                },
                "boundary {target:?}"
            );
            let current = crate::workspace_graph::snapshot(&fixture.root, "ops.consumer").unwrap();
            assert_eq!(
                current.workspace_revision(),
                if target
                    == OperationsEvidencePoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterActiveReplace,
                    )
                {
                    &candidate_revision
                } else {
                    &old_revision
                },
                "boundary {target:?}"
            );
            let generations =
                directory_names(&fixture.root.join(".semaprax-workspace/generations"));
            assert!(!generations.is_empty());
            assert!(generations.len() <= 2);
            assert!(directory_names(&fixture.root.join(".semaprax-workspace/staging")).len() <= 1);
            assert_eq!(
                std::fs::read(fixture.root.join("a/provider.spx")).unwrap(),
                raw_provider
            );
            assert_eq!(
                std::fs::read(fixture.root.join("b/consumer.spx")).unwrap(),
                raw_consumer
            );
            fixture.assert_exclusive_reacquire();
        }
    }

    #[test]
    fn operations_candidate_residue_requires_regenerated_evidence_and_is_reused() {
        let fixture = ManagedOperationsFixture::new("candidate-residue-reuse");
        let original = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
        let evidence_path = fixture.root.join("operations-evidence.json");
        std::fs::write(&evidence_path, original.operations_evidence()).unwrap();
        let old_revision = crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
            .unwrap()
            .workspace_revision()
            .to_owned();
        let candidate_revision = serde_json::from_str::<Value>(original.operations_evidence())
            .unwrap()["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut published_candidate = None;
        let diagnostics = apply_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                if point
                    == OperationsEvidencePoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterCandidatePrepared,
                    )
                {
                    published_candidate = candidate.map(Path::to_path_buf);
                    return Err(std::io::Error::other("stop before pivot"));
                }
                Ok(())
            },
        )
        .err()
        .unwrap();
        assert_eq!(diagnostics[0].code, "SPX-I211");
        assert_eq!(
            crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
                .unwrap()
                .workspace_revision(),
            old_revision
        );
        let published_candidate = published_candidate.unwrap();
        assert!(published_candidate.is_dir());
        let retained_before =
            directory_names(&fixture.root.join(".semaprax-workspace/generations"));
        assert_eq!(retained_before.len(), 2);

        let stale = apply(&fixture.root, &fixture.proposal_path, &evidence_path)
            .err()
            .unwrap();
        assert_eq!(stale[0].code, "SPX-G203");
        assert_eq!(
            crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
                .unwrap()
                .workspace_revision(),
            old_revision
        );

        let regenerated = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
        assert_ne!(
            regenerated.operations_evidence(),
            original.operations_evidence()
        );
        std::fs::write(&evidence_path, regenerated.operations_evidence()).unwrap();
        let mut reused_candidate = None;
        let receipt = apply_with_hook(
            &fixture.root,
            &fixture.proposal_path,
            &evidence_path,
            |point, _, _, candidate| {
                if point
                    == OperationsEvidencePoint::Workspace(
                        workspace::SemanticChangeApplyPoint::AfterCandidatePrepared,
                    )
                {
                    reused_candidate = candidate.map(Path::to_path_buf);
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            reused_candidate.as_deref(),
            Some(published_candidate.as_path())
        );
        assert_eq!(
            directory_names(&fixture.root.join(".semaprax-workspace/generations")),
            retained_before
        );
        assert_eq!(
            crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
                .unwrap()
                .workspace_revision(),
            candidate_revision
        );
        assert_eq!(
            serde_json::from_str::<Value>(receipt.trim_end()).unwrap()["result"],
            "applied"
        );
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn operations_destination_races_never_clobber_foreign_objects() {
        for raced_at in [
            workspace::GenerationPoint::BeforeGenerationPublish,
            workspace::GenerationPoint::DestinationChecked,
        ] {
            let fixture = ManagedOperationsFixture::new(&format!("destination-race-{raced_at:?}"));
            let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
            let evidence_path = fixture.root.join("operations-evidence.json");
            std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
            let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
            let old_active = std::fs::read(&active_path).unwrap();
            let foreign = std::cell::RefCell::new(None::<PathBuf>);
            let diagnostics = apply_with_hook(
                &fixture.root,
                &fixture.proposal_path,
                &evidence_path,
                |point, _, _, candidate| {
                    if point
                        == OperationsEvidencePoint::Workspace(
                            workspace::SemanticChangeApplyPoint::Generation(raced_at),
                        )
                    {
                        let destination = candidate.unwrap();
                        std::fs::write(destination, b"foreign-operations-candidate\n")?;
                        *foreign.borrow_mut() = Some(destination.to_path_buf());
                    }
                    Ok(())
                },
            )
            .err()
            .unwrap();
            assert_eq!(diagnostics[0].code, "SPX-I211", "race {raced_at:?}");
            let foreign = foreign.into_inner().unwrap();
            assert_eq!(
                std::fs::read(foreign).unwrap(),
                b"foreign-operations-candidate\n"
            );
            assert_eq!(std::fs::read(active_path).unwrap(), old_active);
            fixture.assert_exclusive_reacquire();
        }
    }

    #[test]
    fn operations_cooperative_reader_observes_old_then_new_generation() {
        let fixture = ManagedOperationsFixture::new("cooperative-reader");
        let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
        let evidence_path = fixture.root.join("operations-evidence.json");
        std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
        let expected_revision = serde_json::from_str::<Value>(artifacts.operations_evidence())
            .unwrap()["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let active_path = fixture.root.join(".semaprax-workspace/ACTIVE");
        let old_active = std::fs::read(&active_path).unwrap();
        std::thread::scope(|scope| {
            let (arrived_tx, arrived_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let root = &fixture.root;
            let proposal = &fixture.proposal_path;
            let evidence = &evidence_path;
            let writer = scope.spawn(move || {
                apply_with_hook(root, proposal, evidence, |point, _, _, _| {
                    if point
                        == OperationsEvidencePoint::Workspace(
                            workspace::SemanticChangeApplyPoint::BeforeActiveReplace,
                        )
                    {
                        arrived_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                    }
                    Ok(())
                })
            });
            arrived_rx.recv().unwrap();
            let diagnostics = crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
                .err()
                .unwrap();
            assert_eq!(diagnostics[0].code, "SPX-I210");
            assert_eq!(std::fs::read(&active_path).unwrap(), old_active);
            release_tx.send(()).unwrap();
            writer.join().unwrap().unwrap();
        });
        assert_eq!(
            crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
                .unwrap()
                .workspace_revision(),
            expected_revision
        );
        fixture.assert_exclusive_reacquire();
    }

    #[test]
    fn operations_apply_process_child() {
        if std::env::var_os("SEMAPRAX_OPERATIONS_APPLY_CHILD").is_none() {
            return;
        }
        let root = PathBuf::from(std::env::var_os("SEMAPRAX_OPERATIONS_APPLY_ROOT").unwrap());
        let proposal =
            PathBuf::from(std::env::var_os("SEMAPRAX_OPERATIONS_APPLY_PROPOSAL").unwrap());
        let evidence =
            PathBuf::from(std::env::var_os("SEMAPRAX_OPERATIONS_APPLY_EVIDENCE").unwrap());
        let boundary = std::env::var("SEMAPRAX_OPERATIONS_APPLY_BOUNDARY").unwrap();
        let ready = PathBuf::from(std::env::var_os("SEMAPRAX_OPERATIONS_APPLY_READY").unwrap());
        apply_with_hook(&root, &proposal, &evidence, |point, _, _, _| {
            let selected = matches!(
                (boundary.as_str(), point),
                (
                    "pre",
                    OperationsEvidencePoint::Workspace(
                        workspace::SemanticChangeApplyPoint::BeforeActiveReplace
                    )
                ) | (
                    "post",
                    OperationsEvidencePoint::Workspace(
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
    fn operations_apply_killed_process_boundaries_preserve_exact_old_or_new() {
        for boundary in ["pre", "post"] {
            let fixture = ManagedOperationsFixture::new(&format!("process-kill-{boundary}"));
            let artifacts = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
            let evidence_path = fixture.root.join("operations-evidence.json");
            std::fs::write(&evidence_path, artifacts.operations_evidence()).unwrap();
            let old_revision = crate::workspace_graph::snapshot(&fixture.root, "ops.consumer")
                .unwrap()
                .workspace_revision()
                .to_owned();
            let candidate_revision = serde_json::from_str::<Value>(artifacts.operations_evidence())
                .unwrap()["candidate_workspace_revision"]
                .as_str()
                .unwrap()
                .to_owned();
            let raw_paths = ["a/provider.spx", "b/consumer.spx"];
            let raw_bytes = raw_paths.map(|path| std::fs::read(fixture.root.join(path)).unwrap());

            let (mut child, ready) =
                spawn_operations_apply_process(&fixture, &evidence_path, boundary);
            let held_lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(fixture.root.join(".semaprax-workspace/LOCK"))
                .unwrap();
            assert!(FileExt::try_lock_exclusive(&held_lock).is_err());
            child.kill().unwrap();
            assert!(!child.wait().unwrap().success());
            std::fs::remove_file(ready).unwrap();
            fixture.assert_exclusive_reacquire();

            let current = crate::workspace_graph::snapshot(&fixture.root, "ops.consumer").unwrap();
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
            assert_eq!(
                directory_names(&fixture.root.join(".semaprax-workspace/generations")),
                expected_generations
            );
            for generation in &expected_generations {
                let metadata = std::fs::symlink_metadata(
                    fixture
                        .root
                        .join(".semaprax-workspace/generations")
                        .join(generation),
                )
                .unwrap();
                assert!(metadata.is_dir());
                assert!(!metadata.file_type().is_symlink());
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt as _;
                    assert_eq!(metadata.file_attributes() & 0x400, 0);
                }
            }
            let staging = directory_names(&fixture.root.join(".semaprax-workspace/staging"));
            if boundary == "pre" {
                assert_eq!(staging, ["0"]);
                let metadata =
                    std::fs::symlink_metadata(fixture.root.join(".semaprax-workspace/staging/0"))
                        .unwrap();
                assert!(metadata.is_file());
                assert!(!metadata.file_type().is_symlink());
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt as _;
                    assert_eq!(metadata.file_attributes() & 0x400, 0);
                }
            } else {
                assert!(staging.is_empty());
            }
            for (path, bytes) in raw_paths.into_iter().zip(raw_bytes) {
                assert_eq!(std::fs::read(fixture.root.join(path)).unwrap(), bytes);
            }
        }
    }
}
