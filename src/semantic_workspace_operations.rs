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
mod nominal_rename;

pub(crate) use nominal_rename::derive_nominal_rename;

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
    // Private Project member route only; deliberately absent from parse().
    RecordField,
    VariantCase,
    VariantField,
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
            Self::RecordField => "record_field",
            Self::VariantCase => "variant_case",
            Self::VariantField => "variant_field",
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
            "record_field" | "variant_field" => 3,
            "variant_case" => 4,
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
        if !names.insert((
            declaration.path.as_str(),
            category,
            declaration.namespace_owner.as_deref(),
            final_name,
        )) {
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
        if !names.insert((import.path.as_str(), category, None, final_name)) {
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
        if occurrence.path != operation.path() || occurrence.shorthand_binding.is_some() {
            return Err(replay());
        }
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
                let mut ancestor = owner.as_str();
                // Cases add one more ownership level below a variant. Walk
                // only the authenticated finite declaration ancestry.
                for _ in 0..base.graph.declarations().len() {
                    let Ok(index) = base
                        .graph
                        .declarations()
                        .binary_search_by(|declaration| declaration.id().cmp(ancestor))
                    else {
                        break;
                    };
                    let Some(parent) = base.graph.declarations()[index].owner() else {
                        break;
                    };
                    if !allowed_fingerprint_owners.insert(parent) {
                        break;
                    }
                    ancestor = parent;
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
            .all(|(left, right)| left.path == right.path && left.owner == right.owner)
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
                    && left.namespace_owner == right.namespace_owner
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
#[path = "semantic_workspace_operations/tests.rs"]
mod tests;
