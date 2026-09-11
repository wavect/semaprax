//! Conservative owned-data staleness gate and separately approved publication
//! authority for Universal Semantic Transaction v2 workflows.
//!
//! This module invents no new transaction kernel, workflow composition, or
//! publication route of its own. It composes three already-frozen surfaces:
//! `super::super::SemanticTransactionV2Workflow` (#127) for step-by-step
//! revalidation against a moved base, `super::rebase::pending_draft_conflicts`
//! for scoped, targets-only concurrent-change detection (reused unmodified
//! from the v1 candidate rebase core), and `super::publication` for the
//! actual managed-Workspace commit boundary. See
//! `docs/OWNED-WORKFLOW-APPROVAL-V1.md`.
//!
//! Three composed pieces:
//!
//! 1. **Conservative owned-data staleness.** [`require_owned_targets_unchanged`]
//!    refuses -- rather than silently reselecting -- whenever any declaration
//!    a workflow's steps target has a different signature (including each
//!    parameter's ownership mode), effects, or body between the revision the
//!    workflow's transactions were authored against and the current live
//!    revision. A drift that never touches a targeted declaration (a disjoint
//!    sibling edit) passes; [`reselect_owned_workflow`] then replays the
//!    unmodified original transaction content through the frozen v2 workflow
//!    core, which independently reselects every expression identity and
//!    old-source precondition and fully recompiles the resulting program, so
//!    an edit that broke an actual safety property the workflow depended on
//!    is still caught by ordinary compile-time admission rather than a
//!    backend accident.
//! 2. **One publishable, whole-history candidate.** `super::publication`'s
//!    existing commit boundary requires a [`super::ProjectCandidate`] whose
//!    own `base` is the exact on-disk Project revision. A v2 workflow's own
//!    `candidate()` is instead based on its *penultimate* step's revision, so
//!    [`OwnedWorkflowCandidate::derive`] independently replays every step's
//!    already-validated `replace_expression` intention from the workflow's
//!    true original base into one accumulated candidate, and cross-checks the
//!    result against the frozen workflow core's own final revision before
//!    trusting it.
//! 3. **Publication requires a separately approved exact digest.**
//!    [`OwnedWorkflowApproval::approve`] is a distinct act from creating a
//!    workflow: it captures one workflow's and its whole-history candidate's
//!    exact digests as an authority-free evidence value. Approving one digest
//!    never authorizes a different candidate, and a workflow re-derived after
//!    staleness (even over the identical steps) carries a new digest that no
//!    existing approval names. [`prepare_approved_owned_workflow_publication`]
//!    and [`apply_approved_owned_workflow_publication`] require this evidence
//!    and then delegate to the unmodified `super::publication` commit
//!    boundary, which still performs its own independent replay and holds the
//!    live invocation as the only authority that pivots `ACTIVE`.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectRevision, SemanticTransactionReplaceExpression, SemanticTransactionV2,
    SemanticTransactionV2Workflow,
};

use super::publication::{self, ProjectCandidatePublication};
use super::{ProjectCandidate, SemanticChange};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const OWNED_WORKFLOW_APPROVAL_SCHEMA: &str = "semaprax.owned-workflow-approval.v1";
pub const MAX_OWNED_WORKFLOW_APPROVAL_BYTES: usize = 64 * 1024;

const APPROVAL_DOMAIN: &[u8] = b"semaprax.owned-workflow-approval.digest.v1\0";

/// One immutable, publishable [`ProjectCandidate`] spanning every step of a
/// Universal Semantic Transaction v2 workflow, independently reconstructed
/// from the workflow's own true original base.
///
/// [`SemanticTransactionV2Workflow::candidate`] returns only its terminal
/// step's own local candidate, whose `base` is the *intermediate* revision
/// step N-1 produced -- correct for that step's own review artifacts, but not
/// directly publishable through `super::publication`'s existing commit
/// boundary, which requires a candidate whose `base` equals the exact
/// currently-held Project revision. This type replays each already-validated
/// step's `replace_expression` intention (reconstructed from its public
/// `target`/`expression_id`/`replacement`, the exact same shape
/// `SemanticTransactionV2::validate` itself constructs) onto one running
/// [`ProjectCandidate`] rooted at `base`, and cross-checks that the result's
/// final revision matches the frozen workflow core's own -- so a divergent
/// reconstruction is rejected rather than silently published.
pub struct OwnedWorkflowCandidate {
    candidate: ProjectCandidate,
    workflow: SemanticTransactionV2Workflow,
}

impl OwnedWorkflowCandidate {
    pub fn derive(
        base: Arc<ProjectRevision>,
        transactions: &[SemanticTransactionV2],
    ) -> Result<Self> {
        let workflow = SemanticTransactionV2Workflow::derive(Arc::clone(&base), transactions)?;
        let mut accumulated = ProjectCandidate::open(Arc::clone(&base), base.project_revision())?;
        let mut current = base;
        for transaction in transactions {
            let operation = transaction.operation();
            let change = SemanticChange::new(
                current.project_revision(),
                &json!({
                    "expression_id": operation.expression_id(),
                    "kind": "replace_expression",
                    "replacement": operation.replacement().clone(),
                    "target": operation.target(),
                }),
            )?;
            accumulated = accumulated.apply(accumulated.candidate_digest(), &change)?;
            current = Arc::clone(accumulated.revision());
        }
        if accumulated.revision().project_revision()
            != workflow.candidate().revision().project_revision()
        {
            return Err(invalid(
                "owned workflow candidate reconstruction diverged from the workflow core",
            ));
        }
        Ok(Self {
            candidate: accumulated,
            workflow,
        })
    }

    /// The whole-history candidate, rooted at the true original base, ready
    /// for `super::publication::prepare_candidate_publication`/
    /// `apply_candidate_publication`.
    pub fn candidate(&self) -> &ProjectCandidate {
        &self.candidate
    }
    /// The frozen v2 workflow core's own step-by-step review artifact
    /// (impact, structural diff, per-step source paths).
    pub fn workflow(&self) -> &SemanticTransactionV2Workflow {
        &self.workflow
    }
}

/// Authority-free evidence that an independent reviewer approved one exact
/// [`OwnedWorkflowCandidate`] output. Approving is a distinct act from
/// creating the workflow/candidate: this value is minted only by
/// [`Self::approve`], never implied by `derive`/`replay` succeeding.
/// Presenting it never itself commits anything --
/// [`prepare_approved_owned_workflow_publication`] and
/// [`apply_approved_owned_workflow_publication`] still perform their own
/// exact-digest and exact-replay checks before any live invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedWorkflowApproval {
    workflow_digest: String,
    candidate_digest: String,
    base_workspace_revision: String,
    json: String,
    digest: String,
}

impl OwnedWorkflowApproval {
    /// Bind approval to exactly this workflow's and its whole-history
    /// candidate's digests. A later re-derivation of the same steps --
    /// whether because the base moved or anything else changed -- produces a
    /// different workflow digest and is never authorized by this value.
    pub fn approve(owned: &OwnedWorkflowCandidate) -> Result<Self> {
        let workflow_digest = owned.workflow.digest().to_owned();
        let candidate_digest = owned.candidate.candidate_digest().to_owned();
        let base_workspace_revision = owned
            .workflow
            .base_program_root()
            .workspace_revision()
            .to_owned();
        let json = render(
            json!({
                "approval_authority": false,
                "base_workspace_revision": base_workspace_revision,
                "candidate_digest": candidate_digest,
                "nonclaims": [
                    "not_itself_publication_authority",
                    "binds_only_to_the_exact_workflow_and_candidate_digest_named_here",
                    "a_rederived_or_rebased_candidate_with_a_different_digest_is_not_authorized",
                    "creation_and_approval_are_distinct_acts",
                ],
                "schema": OWNED_WORKFLOW_APPROVAL_SCHEMA,
                "workflow_digest": workflow_digest,
            }),
            MAX_OWNED_WORKFLOW_APPROVAL_BYTES,
        )?;
        let digest = digest(APPROVAL_DOMAIN, json.as_bytes());
        Ok(Self {
            workflow_digest,
            candidate_digest,
            base_workspace_revision,
            json,
            digest,
        })
    }

    /// Reparse and require exact canonical bytes and digest, so evidence
    /// crossing a process/session boundary cannot be tampered with in
    /// transit -- mirroring `SemanticTransactionV2Workflow::replay`.
    pub fn replay(expected_digest: &str, bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_OWNED_WORKFLOW_APPROVAL_BYTES {
            return Err(capacity("owned workflow approval exceeds its byte limit"));
        }
        super::wire::validate_digest(expected_digest)?;
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("owned workflow approval is not valid JSON"))?;
        if value.get("schema").and_then(Value::as_str) != Some(OWNED_WORKFLOW_APPROVAL_SCHEMA) {
            return Err(invalid("owned workflow approval has an invalid schema"));
        }
        if render(value.clone(), MAX_OWNED_WORKFLOW_APPROVAL_BYTES)?.as_bytes() != bytes {
            return Err(invalid("owned workflow approval is not canonical JSON"));
        }
        if digest(APPROVAL_DOMAIN, bytes) != expected_digest {
            return Err(stale("owned workflow approval digest is stale"));
        }
        let workflow_digest = value["workflow_digest"]
            .as_str()
            .ok_or_else(|| invalid("owned workflow approval is missing its workflow digest"))?
            .to_owned();
        let candidate_digest = value["candidate_digest"]
            .as_str()
            .ok_or_else(|| invalid("owned workflow approval is missing its candidate digest"))?
            .to_owned();
        let base_workspace_revision = value["base_workspace_revision"]
            .as_str()
            .ok_or_else(|| {
                invalid("owned workflow approval is missing its base workspace revision")
            })?
            .to_owned();
        super::wire::validate_digest(&workflow_digest)?;
        super::wire::validate_digest(&candidate_digest)?;
        Ok(Self {
            workflow_digest,
            candidate_digest,
            base_workspace_revision,
            json: String::from_utf8(bytes.to_vec())
                .map_err(|_| invalid("owned workflow approval is not UTF-8"))?,
            digest: expected_digest.to_owned(),
        })
    }

    pub fn workflow_digest(&self) -> &str {
        &self.workflow_digest
    }
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }
    pub fn base_workspace_revision(&self) -> &str {
        &self.base_workspace_revision
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn matches(&self, owned: &OwnedWorkflowCandidate) -> bool {
        self.workflow_digest == owned.workflow.digest()
            && self.candidate_digest == owned.candidate.candidate_digest()
    }
}

/// Conservative owned-data staleness gate. `original_base` is the exact
/// revision `transactions` were authored/reviewed against; `current_base` is
/// the live revision at reselection or publish time. Returns `Ok(())` only
/// when every declaration named as a step target has an identical signature
/// (including each parameter's ownership mode), effects, and body between the
/// two revisions -- the concurrent drift, if any, is a disjoint sibling edit
/// that never touched a declaration this workflow depends on. A
/// same-declaration, changed-signature/ownership-mode, or changed-body edit on
/// any touched target is refused as an explicit conflict, never silently
/// reselected onto a moved base.
pub fn require_owned_targets_unchanged(
    original_base: &ProjectRevision,
    current_base: &ProjectRevision,
    transactions: &[SemanticTransactionV2],
) -> Result<()> {
    if transactions.is_empty() {
        return Err(invalid(
            "owned workflow staleness check requires at least one operation",
        ));
    }
    if original_base.project_revision() == current_base.project_revision() {
        return Ok(());
    }
    let targets = transactions
        .iter()
        .map(|transaction| transaction.operation().target())
        .collect::<BTreeSet<&str>>();
    super::rebase::pending_draft_conflicts(
        original_base,
        current_base,
        &targets,
        &targets,
        &BTreeSet::new(),
    )
    .map(|_| ())
    .map_err(|diagnostics| annotate(diagnostics, "owned-data workflow staleness"))
}

/// Conservatively reselect a workflow against the current live base. Refuses
/// before attempting any replay when [`require_owned_targets_unchanged`]
/// finds an owned declaration the workflow touches has moved concurrently.
///
/// A Universal Semantic Transaction v2 envelope binds its own precondition to
/// one exact global workspace revision (see
/// `docs/UNIVERSAL-SEMANTIC-TRANSACTION-V2.md`), so the original transaction
/// bytes are stale the instant *anything* in the workspace moves, even a
/// disjoint sibling edit. Reselection therefore rebuilds one fresh
/// [`SemanticTransactionV2`] per original step, carrying over its exact
/// target, expression identity, and expected old-source text unchanged and
/// binding only the wrapper to `current_base`'s current workspace revision.
/// It invents no new identity-selection route: the frozen
/// `SemanticTransactionV2::validate` core underneath still independently
/// re-checks that the carried-over expression identity resolves and its
/// exact old-source text still matches in `current_base`, and
/// `SemanticTransactionV2Workflow::derive` still fully recompiles the
/// resulting program -- so a same-declaration edit is rejected there even if
/// [`require_owned_targets_unchanged`] had somehow let it through, and an
/// edit that broke an actual safety property is still caught by ordinary
/// compile-time admission rather than a backend accident.
///
/// The result's digest differs from the original workflow's whenever the
/// base changed at all (even if every step still applies unchanged), so no
/// existing [`OwnedWorkflowApproval`] for the original ever authorizes
/// publishing it -- a fresh approval is required.
pub fn reselect_owned_workflow(
    original_base: &ProjectRevision,
    current_base: Arc<ProjectRevision>,
    transactions: &[SemanticTransactionV2],
) -> Result<OwnedWorkflowCandidate> {
    require_owned_targets_unchanged(original_base, &current_base, transactions)?;
    let current_workspace_revision = current_base
        .canonical_workspace_revision()?
        .workspace_revision()
        .to_owned();
    let refreshed = transactions
        .iter()
        .map(|transaction| {
            let operation = transaction.operation();
            SemanticTransactionV2::replace_expression(
                &current_workspace_revision,
                SemanticTransactionReplaceExpression::new(
                    operation.target(),
                    operation.expression_id(),
                    operation.expected_old_expression(),
                    operation.replacement().clone(),
                ),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    OwnedWorkflowCandidate::derive(current_base, &refreshed)
}

/// Read-only, host-bound publication proposal for exactly the candidate
/// `approval` names. A candidate that was created but never separately
/// approved has no [`OwnedWorkflowApproval`] to present and cannot reach this
/// function; an approval minted for a different workflow or candidate digest
/// -- including a same-steps re-derivation whose base moved -- is refused
/// before any workspace read is authorized. Delegates the actual host replay
/// and lock/authority acquisition unchanged to
/// `super::publication::prepare_candidate_publication`.
pub fn prepare_approved_owned_workflow_publication(
    approval: &OwnedWorkflowApproval,
    owned: &OwnedWorkflowCandidate,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
) -> Result<ProjectCandidatePublication> {
    if !approval.matches(owned) {
        return Err(stale(
            "owned workflow approval does not name this exact workflow and candidate digest",
        ));
    }
    publication::prepare_candidate_publication(
        owned.candidate(),
        &approval.candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
    )
}

/// Separately authorized host invocation for exactly the candidate `approval`
/// names. Mirrors [`prepare_approved_owned_workflow_publication`]'s exact
/// digest requirement and delegates the managed `ACTIVE` pivot unchanged to
/// `super::publication::apply_candidate_publication`, which independently
/// re-derives and requires exact submitted proof bytes: this function adds no
/// filesystem or lock authority of its own, only the additional requirement
/// that a distinct approval evidence value name this exact digest.
pub fn apply_approved_owned_workflow_publication(
    approval: &OwnedWorkflowApproval,
    owned: &OwnedWorkflowCandidate,
    workspace_root: &Path,
    project_manifest: &Path,
    expected_workspace_revision: &str,
    submitted_publication: &[u8],
) -> Result<String> {
    if !approval.matches(owned) {
        return Err(stale(
            "owned workflow approval does not name this exact workflow and candidate digest",
        ));
    }
    publication::apply_candidate_publication(
        owned.candidate(),
        &approval.candidate_digest,
        workspace_root,
        project_manifest,
        expected_workspace_revision,
        submitted_publication,
    )
}

fn annotate(diagnostics: Vec<Diagnostic>, context: &'static str) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| Diagnostic {
            message: format!("{context}: {}", diagnostic.message),
            ..diagnostic
        })
        .collect()
}

fn render(mut value: Value, limit: usize) -> Result<String> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid("owned workflow approval could not be rendered"))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(capacity("owned workflow approval exceeds its byte limit"));
    }
    String::from_utf8(bytes).map_err(|_| invalid("owned workflow approval is not UTF-8"))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G603", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G604", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G605", message)]
}
