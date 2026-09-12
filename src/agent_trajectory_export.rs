//! Bounded, offline, opt-in exporter for privacy-safe semantic-action
//! learning trajectories (issue #146 / SPX-AI-047).
//!
//! # Two independent failure modes, and why one module owns both
//!
//! A trajectory export can satisfy either invariant while quietly failing
//! the other:
//!
//! 1. Secret leakage -- a raw credential, private path, or real user
//!    payload reaching an exported record. `src/model_call_receipt/`'s
//!    [`crate::model_call_receipt::audit_view`] already solved the shape of
//!    this problem for one call's receipt (commitments never reveal a
//!    withheld field; a redacted view still carries a verifiable
//!    commitment rather than silently dropping the field). This module
//!    reuses that exact commitment/domain-separation idiom -- see
//!    [`commit_context_bytes`], [`commit_action_bytes`], and friends --
//!    rather than inventing a second one, and its own tests mirror
//!    `model_call_receipt::audit_view::tests::redaction_hides_each_of_six_secret_bearing_fields_individually`:
//!    every marker is asserted present in the raw candidate before the
//!    sanitized/refused case is checked, so a fixture that never carried a
//!    marker cannot pass this test by accident.
//! 2. Held-out contamination -- a record derived from a held-out
//!    evaluation task silently entering the pool a future model trains on.
//!    Unlike a secret leak, nothing fails loudly downstream when this
//!    happens: the record round-trips, decodes, and looks exactly like a
//!    legitimate synthetic-dev trajectory. The only test that actually
//!    proves this invariant is one that constructs a held-out-labelled (or
//!    denylisted, or family-contaminated) candidate and asserts
//!    [`export_trajectory`] refuses it with a specific [`ExportRefusal`]
//!    variant -- never a test that merely observes a held-out record
//!    absent from some default-constructed empty output, which would also
//!    pass if the refusal path were deleted entirely.
//!
//! # Scope
//!
//! This module is the export mechanism and its refusals. It does not: run
//! a real training job (SPX-AI-048, a separate, human-gated issue), decide
//! whether a given real corpus is legally reusable (a human policy
//! decision this module can only gate on, via
//! [`EligibilityDeclaration::explicitly_selected_for_learning_reuse`]), or
//! compute solution-similarity clustering over a real fixture corpus (out
//! of scope here; see [`HeldOutDenylist`]'s family-based proxy for what is
//! implemented instead, and the "known limitations" note on
//! [`export_trajectory_batch`]).
//!
//! # No ambient authority
//!
//! Every function in this module is `fn(&_, &_, ...) -> Result<_, _>` over
//! caller-supplied bytes and closed-vocabulary flags. There is no
//! [`crate::agent_runtime::AgentHost`], no filesystem path, and no network
//! type anywhere in this module's signatures, so it structurally cannot
//! perform a network call, a filesystem write, or a process spawn; see
//! `tests::export_trajectory_is_pure_and_deterministic_across_repeated_calls`
//! for how that is exercised (repeated calls compared for byte-identical
//! output, rather than a mocked transport -- there is no transport here to
//! mock).
//!
//! # Low-entropy payloads are not automatically private
//!
//! A commitment digest over a value drawn from a small closed set (a task
//! family name, a one-line repair delta, a short prompt) is reversible by
//! brute-force enumeration: hashing alone does not make it safe to publish
//! as "we only export the hash". This module reuses
//! [`crate::model_call_receipt::PayloadPrivacyClaim`] rather than
//! redefining the same threshold a second time: every sanitized record
//! carries an explicit `context_privacy_claim`/`action_privacy_claim`, and
//! a payload under
//! [`crate::model_call_receipt::LOW_ENTROPY_BYTE_THRESHOLD`] bytes is
//! always classified `DigestOnlyLowEntropyCaveat`, never `Withheld` or
//! silently treated as safe -- see
//! `tests::low_entropy_context_never_earns_a_withheld_or_safe_claim`.

use std::collections::BTreeSet;

use sha2::{Digest as _, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::digest_hex::LowerHex;
use crate::model_call_receipt::PayloadPrivacyClaim;

/// Schema of one exported sanitized trajectory record.
pub const TRAJECTORY_SCHEMA_V1: &str = "semaprax.agent-trajectory.v1";

const TRAJECTORY_DOMAIN: &[u8] = b"semaprax.agent-trajectory-export.trajectory.v1\0";
const CONTEXT_DOMAIN: &[u8] = b"semaprax.agent-trajectory-export.context-bytes.v1\0";
const ACTION_DOMAIN: &[u8] = b"semaprax.agent-trajectory-export.action-bytes.v1\0";
const VALIDATION_DOMAIN: &[u8] = b"semaprax.agent-trajectory-export.validation-bytes.v1\0";
const OUTCOME_DOMAIN: &[u8] = b"semaprax.agent-trajectory-export.outcome-bytes.v1\0";
const REPAIR_DOMAIN: &[u8] = b"semaprax.agent-trajectory-export.repair-bytes.v1\0";

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", LowerHex(hash.finalize()))
}

/// Commits to one turn's context bytes. Not reversible from the digest
/// alone unless the input is short -- see the module doc's low-entropy
/// caveat.
#[must_use]
pub fn commit_context_bytes(bytes: &[u8]) -> String {
    digest(CONTEXT_DOMAIN, bytes)
}

/// Commits to the proposed semantic action's bytes.
#[must_use]
pub fn commit_action_bytes(bytes: &[u8]) -> String {
    digest(ACTION_DOMAIN, bytes)
}

/// Commits to the canonical (compiler/verifier) validation outcome bytes.
#[must_use]
pub fn commit_validation_bytes(bytes: &[u8]) -> String {
    digest(VALIDATION_DOMAIN, bytes)
}

/// Commits to the observed execution outcome bytes.
#[must_use]
pub fn commit_outcome_bytes(bytes: &[u8]) -> String {
    digest(OUTCOME_DOMAIN, bytes)
}

/// Commits to the subsequent repair action's bytes, if any.
#[must_use]
pub fn commit_repair_bytes(bytes: &[u8]) -> String {
    digest(REPAIR_DOMAIN, bytes)
}

/// The closed task-split vocabulary. `HeldOut` exists so a caller can name
/// the fact truthfully and be refused by [`export_trajectory`]; it is
/// never a value this module exports.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TaskSplit {
    SyntheticDev,
    HeldOut,
}

impl TaskSplit {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SyntheticDev => "synthetic_dev",
            Self::HeldOut => "held_out",
        }
    }
}

/// Who assigned [`IndependentAcceptance::decision`]. `ModelSelfReport` is a
/// named, closed member of this vocabulary specifically so
/// [`export_trajectory`] can refuse it: a model's own claim about its own
/// output is never the oracle a training signal is built on (tracking
/// issue step 5: never label a model self-evaluation as the oracle).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AcceptanceSource {
    /// The compiler, verifier, or type checker admitted (or refused) the
    /// action: a canonical, deterministic ground truth.
    CanonicalVerifier,
    /// A human or a separate, independently run reviewer/judge process.
    IndependentReviewer,
    /// An independently executed hostile-input/golden/regression test
    /// suite outcome.
    IndependentTestSuite,
    /// The model's own self-report about its own action. Never accepted by
    /// [`export_trajectory`].
    ModelSelfReport,
}

impl AcceptanceSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalVerifier => "canonical_verifier",
            Self::IndependentReviewer => "independent_reviewer",
            Self::IndependentTestSuite => "independent_test_suite",
            Self::ModelSelfReport => "model_self_report",
        }
    }
}

/// The independently assigned acceptance verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AcceptanceDecision {
    Accepted,
    Rejected,
}

impl AcceptanceDecision {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

/// One independently assigned acceptance verdict, kept separate from the
/// canonical validation and observed execution outcomes so a rejected
/// action's rejection label is never silently overwritten by a later
/// acceptance, and vice versa.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndependentAcceptance {
    pub source: AcceptanceSource,
    pub decision: AcceptanceDecision,
}

/// One raw candidate a caller proposes for export. Its raw byte fields
/// (`context_bytes`, `proposed_action_bytes`, ...) are never themselves
/// exported: [`export_trajectory`] only ever emits a [`SanitizedTrajectory`]
/// built from commitments over these bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct TrajectoryCandidate {
    pub task_family: String,
    pub split: TaskSplit,
    /// Identity of the originating fixture/task, checked against
    /// [`HeldOutDenylist`]. Caller-supplied, not derived here, so a
    /// denylist built from a separate held-out task registry can be
    /// checked without this module ever seeing the held-out task's actual
    /// content.
    pub source_task_digest: String,
    pub language_version: String,
    pub schema_version: String,
    pub toolchain_version: String,
    pub model_version: String,
    pub context_revision: String,
    pub context_bytes: Vec<u8>,
    pub proposed_action_bytes: Vec<u8>,
    pub canonical_validation_bytes: Vec<u8>,
    pub canonical_validation_accepted: bool,
    pub observed_execution_bytes: Vec<u8>,
    pub observed_execution_succeeded: bool,
    pub repair_bytes: Option<Vec<u8>>,
    pub acceptance: IndependentAcceptance,
    /// A reference to (never a copy of) the digest of the original trusted
    /// journal/receipt evidence this trajectory derives from, e.g.
    /// [`crate::model_call_receipt::ModelCallReceipt::digest`] or an
    /// [`crate::agent_transcript::ScriptedRun`]'s evidence digest. Kept
    /// distinct from [`SanitizedTrajectory::digest`] by construction --
    /// see `tests::sanitized_digest_never_equals_any_digest_it_references`.
    pub provenance_original_evidence_digest: String,
}

/// The closed, exhaustive eligibility declaration a caller must assert
/// about a [`TrajectoryCandidate`]'s raw bytes before
/// [`export_trajectory`] will consider it. Mirrors
/// `std.log.redact.event_is_safe`'s shape (named boolean flags, one per
/// secret-bearing family, closed over a fixed list) and its own caveat:
/// this module does not scan `context_bytes`/`proposed_action_bytes` for
/// secret-shaped content itself. A caller lies about these flags at its own
/// risk, exactly as `std.log.redact`'s docs already state for its own
/// callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EligibilityDeclaration {
    /// A provider API key, bearer token, session token, webhook signing
    /// secret, or SMTP/API credential -- the same family
    /// `std.log.redact.event_is_safe` and
    /// [`crate::model_call_receipt::ReceiptPrivateExtras`] close over.
    pub carries_provider_secret: bool,
    /// A live runtime credential (not a provider secret): a local process
    /// token, a signing key, a wallet, or similar ambient authority.
    pub carries_runtime_credential: bool,
    /// An absolute or otherwise identifying private filesystem path.
    pub carries_private_filesystem_path: bool,
    /// Real user data rather than synthetic/development fixture content.
    pub carries_real_user_data: bool,
    /// A hidden evaluator/oracle input that a training consumer should
    /// never see (would let a trained model "peek" at grading internals).
    pub carries_hidden_oracle_input: bool,
    /// The caller's affirmative, explicit opt-in. Every field, including
    /// this one, must be named at the call site (this struct has no
    /// `Default` impl), so "eligible for export" can never be a value
    /// nobody actually chose.
    pub explicitly_selected_for_learning_reuse: bool,
}

impl EligibilityDeclaration {
    #[must_use]
    fn carries_any_disqualifying_content(self) -> bool {
        self.carries_provider_secret
            || self.carries_runtime_credential
            || self.carries_private_filesystem_path
            || self.carries_real_user_data
            || self.carries_hidden_oracle_input
    }
}

/// A denylist of held-out task/fixture identities. Checked two ways: exact
/// `source_task_digest` membership, and `task_family` membership. The
/// family check is what keeps a near-duplicate fixture variant that
/// individually claims `SyntheticDev` from entering the training split
/// merely because that one row was not the held-out row itself (tracking
/// issue step 6: keep related fixture variants in one split).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HeldOutDenylist {
    task_digests: BTreeSet<String>,
    task_families: BTreeSet<String>,
}

impl HeldOutDenylist {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deny_task_digest(&mut self, digest: impl Into<String>) {
        self.task_digests.insert(digest.into());
    }

    pub fn deny_task_family(&mut self, family: impl Into<String>) {
        self.task_families.insert(family.into());
    }

    #[must_use]
    pub fn denies(&self, candidate: &TrajectoryCandidate) -> bool {
        self.task_digests.contains(&candidate.source_task_digest)
            || self.task_families.contains(&candidate.task_family)
    }
}

/// Why [`export_trajectory`] or [`export_trajectory_batch`] refused a
/// candidate. Every variant is a distinct, attributable diagnostic code
/// (see [`ExportRefusal::diagnostic`]), never a single generic "refused".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportRefusal {
    /// The candidate is itself labelled a held-out evaluation task.
    HeldOutSplit,
    /// The candidate's task/fixture digest or task family is on the
    /// caller-supplied held-out denylist.
    HeldOutDenylisted,
    /// The candidate claims `SyntheticDev`, but another candidate in the
    /// same export batch and the same `task_family` is labelled `HeldOut`;
    /// the whole family is refused, not only the labelled row.
    HeldOutFamilyContamination,
    /// The caller did not affirmatively opt this candidate in.
    NotExplicitlySelected,
    /// The caller's own eligibility declaration names a secret-bearing,
    /// credential-bearing, private-path-bearing, real-user-data-bearing,
    /// or hidden-oracle-bearing source.
    SecretBearingSource,
    /// The independent acceptance verdict's source is the model's own
    /// self-report, which this module never treats as an oracle.
    ModelSelfReportedAcceptance,
}

impl ExportRefusal {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::HeldOutSplit => "SPX-G610",
            Self::HeldOutDenylisted => "SPX-G611",
            Self::HeldOutFamilyContamination => "SPX-G612",
            Self::NotExplicitlySelected => "SPX-G613",
            Self::SecretBearingSource => "SPX-G614",
            Self::ModelSelfReportedAcceptance => "SPX-G615",
        }
    }

    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::HeldOutSplit => {
                "trajectory export refused: candidate is labelled a held-out evaluation task"
            }
            Self::HeldOutDenylisted => {
                "trajectory export refused: candidate's task digest or task family is on the held-out denylist"
            }
            Self::HeldOutFamilyContamination => {
                "trajectory export refused: another candidate in this task family is labelled held-out"
            }
            Self::NotExplicitlySelected => {
                "trajectory export refused: candidate was not explicitly selected for learning reuse"
            }
            Self::SecretBearingSource => {
                "trajectory export refused: candidate's eligibility declaration names a secret-, credential-, private-path-, real-user-data-, or hidden-oracle-bearing source"
            }
            Self::ModelSelfReportedAcceptance => {
                "trajectory export refused: independent acceptance cannot be sourced from the model's own self-report"
            }
        }
    }

    /// A `Diagnostic` naming the exact refusal reason. Never carries any
    /// raw candidate byte -- only this closed reason's fixed text.
    #[must_use]
    pub fn diagnostic(self) -> Diagnostic {
        Diagnostic::io(self.code(), self.message())
    }
}

/// One privacy-safe, provenance-referenced sanitized trajectory. Never
/// constructed except by [`export_trajectory`], and never carries any raw
/// byte field from [`TrajectoryCandidate`]: only commitments, closed tags,
/// and version identities.
#[derive(Clone, Debug, PartialEq)]
pub struct SanitizedTrajectory {
    pub task_family: String,
    pub split: TaskSplit,
    pub language_version: String,
    pub schema_version: String,
    pub toolchain_version: String,
    pub model_version: String,
    pub context_revision: String,
    pub context_digest: String,
    pub context_privacy_claim: PayloadPrivacyClaim,
    pub action_digest: String,
    pub action_privacy_claim: PayloadPrivacyClaim,
    pub canonical_validation_digest: String,
    pub canonical_validation_accepted: bool,
    pub execution_outcome_digest: String,
    pub observed_execution_succeeded: bool,
    pub repair_digest: Option<String>,
    pub acceptance_source: AcceptanceSource,
    pub acceptance_decision: AcceptanceDecision,
    /// A reference to the original trusted evidence this trajectory
    /// derives from. Never reused as though the sanitized bytes were
    /// identical to it: see [`Self::digest`].
    pub provenance_original_evidence_digest: String,
}

impl SanitizedTrajectory {
    /// The canonical, deterministic rendering: fixed field order, explicit
    /// `null` for absent optionals. Two sanitized trajectories with
    /// identical fields render (and therefore digest) identically; any
    /// differing field changes the rendering.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{{\"schema\":{},\"task_family\":{},\"split\":{},\"language_version\":{},\"schema_version\":{},\"toolchain_version\":{},\"model_version\":{},\"context_revision\":{},\"context_digest\":{},\"context_privacy_claim\":{},\"action_digest\":{},\"action_privacy_claim\":{},\"canonical_validation_digest\":{},\"canonical_validation_accepted\":{},\"execution_outcome_digest\":{},\"observed_execution_succeeded\":{},\"repair_digest\":{},\"acceptance_source\":{},\"acceptance_decision\":{},\"provenance_original_evidence_digest\":{}}}",
            quote_json(TRAJECTORY_SCHEMA_V1),
            quote_json(&self.task_family),
            quote_json(self.split.as_str()),
            quote_json(&self.language_version),
            quote_json(&self.schema_version),
            quote_json(&self.toolchain_version),
            quote_json(&self.model_version),
            quote_json(&self.context_revision),
            quote_json(&self.context_digest),
            quote_json(self.context_privacy_claim.as_str()),
            quote_json(&self.action_digest),
            quote_json(self.action_privacy_claim.as_str()),
            quote_json(&self.canonical_validation_digest),
            self.canonical_validation_accepted,
            quote_json(&self.execution_outcome_digest),
            self.observed_execution_succeeded,
            match &self.repair_digest {
                Some(value) => quote_json(value),
                None => "null".to_owned(),
            },
            quote_json(self.acceptance_source.as_str()),
            quote_json(self.acceptance_decision.as_str()),
            quote_json(&self.provenance_original_evidence_digest),
        )
    }

    /// The canonical digest of this sanitized record, domain-separated
    /// from every commitment/evidence digest it references. It cannot
    /// equal `provenance_original_evidence_digest`, `context_digest`,
    /// `action_digest`, or any other referenced digest by construction:
    /// they are computed under different domain-separation prefixes -- see
    /// `tests::sanitized_digest_never_equals_any_digest_it_references`.
    #[must_use]
    pub fn digest(&self) -> String {
        digest(TRAJECTORY_DOMAIN, self.render().as_bytes())
    }
}

/// Exports exactly one candidate, or refuses it with an attributable
/// reason. Pure: no filesystem, network, process, or clock access anywhere
/// in this call graph.
pub fn export_trajectory(
    candidate: &TrajectoryCandidate,
    eligibility: &EligibilityDeclaration,
    denylist: &HeldOutDenylist,
) -> Result<SanitizedTrajectory, ExportRefusal> {
    if candidate.split == TaskSplit::HeldOut {
        return Err(ExportRefusal::HeldOutSplit);
    }
    if denylist.denies(candidate) {
        return Err(ExportRefusal::HeldOutDenylisted);
    }
    if !eligibility.explicitly_selected_for_learning_reuse {
        return Err(ExportRefusal::NotExplicitlySelected);
    }
    if eligibility.carries_any_disqualifying_content() {
        return Err(ExportRefusal::SecretBearingSource);
    }
    if candidate.acceptance.source == AcceptanceSource::ModelSelfReport {
        return Err(ExportRefusal::ModelSelfReportedAcceptance);
    }
    Ok(SanitizedTrajectory {
        task_family: candidate.task_family.clone(),
        split: candidate.split,
        language_version: candidate.language_version.clone(),
        schema_version: candidate.schema_version.clone(),
        toolchain_version: candidate.toolchain_version.clone(),
        model_version: candidate.model_version.clone(),
        context_revision: candidate.context_revision.clone(),
        context_digest: commit_context_bytes(&candidate.context_bytes),
        context_privacy_claim: PayloadPrivacyClaim::classify(candidate.context_bytes.len(), false),
        action_digest: commit_action_bytes(&candidate.proposed_action_bytes),
        action_privacy_claim: PayloadPrivacyClaim::classify(
            candidate.proposed_action_bytes.len(),
            false,
        ),
        canonical_validation_digest: commit_validation_bytes(&candidate.canonical_validation_bytes),
        canonical_validation_accepted: candidate.canonical_validation_accepted,
        execution_outcome_digest: commit_outcome_bytes(&candidate.observed_execution_bytes),
        observed_execution_succeeded: candidate.observed_execution_succeeded,
        repair_digest: candidate.repair_bytes.as_deref().map(commit_repair_bytes),
        acceptance_source: candidate.acceptance.source,
        acceptance_decision: candidate.acceptance.decision,
        provenance_original_evidence_digest: candidate.provenance_original_evidence_digest.clone(),
    })
}

/// One refusal attributed to its position in the input batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchRefusal {
    pub index: usize,
    pub refusal: ExportRefusal,
}

/// Exports a batch, first poisoning every candidate whose `task_family` is
/// shared with any `HeldOut`-labelled candidate in the same batch (tracking
/// issue step 6's family/ancestry partitioning), then applying
/// [`export_trajectory`] to every remaining candidate. Any refusal in the
/// batch fails the whole batch: a partial, silently-filtered export is
/// exactly the failure mode that lets contamination in "by construction"
/// rather than by an explicit, auditable refusal.
///
/// Known limitation: family membership is the only ancestry/similarity
/// signal this module implements. Solution-similarity clustering over
/// real fixture content (the issue's "near-duplicate ... solution
/// similarity" language beyond a shared family label) needs a real corpus
/// and a similarity metric this module does not have; a caller with a
/// finer-grained ancestry signal should fold it into `task_family` (or a
/// denied [`HeldOutDenylist`] entry) before calling this function.
pub fn export_trajectory_batch(
    candidates: &[TrajectoryCandidate],
    eligibility: &[EligibilityDeclaration],
    denylist: &HeldOutDenylist,
) -> Result<Vec<SanitizedTrajectory>, Vec<BatchRefusal>> {
    assert_eq!(
        candidates.len(),
        eligibility.len(),
        "export_trajectory_batch requires exactly one eligibility declaration per candidate"
    );
    let mut held_out_families: BTreeSet<&str> = BTreeSet::new();
    for candidate in candidates {
        if candidate.split == TaskSplit::HeldOut {
            held_out_families.insert(candidate.task_family.as_str());
        }
    }
    let mut sanitized = Vec::with_capacity(candidates.len());
    let mut refusals = Vec::new();
    for (index, (candidate, elig)) in candidates.iter().zip(eligibility).enumerate() {
        let outcome = if candidate.split == TaskSplit::SyntheticDev
            && held_out_families.contains(candidate.task_family.as_str())
        {
            Err(ExportRefusal::HeldOutFamilyContamination)
        } else {
            export_trajectory(candidate, elig, denylist)
        };
        match outcome {
            Ok(trajectory) => sanitized.push(trajectory),
            Err(refusal) => refusals.push(BatchRefusal { index, refusal }),
        }
    }
    if refusals.is_empty() {
        Ok(sanitized)
    } else {
        Err(refusals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_candidate() -> TrajectoryCandidate {
        TrajectoryCandidate {
            task_family: "family-a".to_owned(),
            split: TaskSplit::SyntheticDev,
            source_task_digest: "sha256:task-a".to_owned(),
            language_version: "spx-0.1".to_owned(),
            schema_version: "semaprax.agent-trajectory.v1".to_owned(),
            toolchain_version: "toolchain-fixture-1".to_owned(),
            model_version: "model-fixture-1".to_owned(),
            context_revision: "sha256:revision-1".to_owned(),
            context_bytes: b"ordinary non-secret context".to_vec(),
            proposed_action_bytes: b"ordinary non-secret action".to_vec(),
            canonical_validation_bytes: b"ordinary non-secret validation".to_vec(),
            canonical_validation_accepted: true,
            observed_execution_bytes: b"ordinary non-secret execution".to_vec(),
            observed_execution_succeeded: true,
            repair_bytes: None,
            acceptance: IndependentAcceptance {
                source: AcceptanceSource::CanonicalVerifier,
                decision: AcceptanceDecision::Accepted,
            },
            provenance_original_evidence_digest: "sha256:original-evidence".to_owned(),
        }
    }

    fn eligible() -> EligibilityDeclaration {
        EligibilityDeclaration {
            carries_provider_secret: false,
            carries_runtime_credential: false,
            carries_private_filesystem_path: false,
            carries_real_user_data: false,
            carries_hidden_oracle_input: false,
            explicitly_selected_for_learning_reuse: true,
        }
    }

    #[test]
    fn baseline_candidate_exports_successfully() {
        let candidate = base_candidate();
        let result = export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new());
        assert!(
            result.is_ok(),
            "a clean, opted-in, synthetic-dev candidate must export: {result:?}"
        );
    }

    // ---- Held-out contamination: the refusal, not mere absence ----

    #[test]
    fn export_refuses_a_held_out_split_candidate_outright() {
        let mut candidate = base_candidate();
        candidate.split = TaskSplit::HeldOut;
        let result = export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new());
        assert_eq!(result, Err(ExportRefusal::HeldOutSplit));
        assert_eq!(ExportRefusal::HeldOutSplit.diagnostic().code, "SPX-G610");

        // Positive control: the identical candidate, only the split
        // flipped back to SyntheticDev, must export -- proving the
        // refusal above is caused by the split label, not something else
        // that would refuse unconditionally.
        let mut synthetic = candidate;
        synthetic.split = TaskSplit::SyntheticDev;
        assert!(export_trajectory(&synthetic, &eligible(), &HeldOutDenylist::new()).is_ok());
    }

    #[test]
    fn export_refuses_a_denylisted_task_digest() {
        let candidate = base_candidate();
        let mut denylist = HeldOutDenylist::new();
        denylist.deny_task_digest(candidate.source_task_digest.clone());
        assert_eq!(
            export_trajectory(&candidate, &eligible(), &denylist),
            Err(ExportRefusal::HeldOutDenylisted)
        );
        // Positive control: an empty denylist lets the same candidate
        // through.
        assert!(export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).is_ok());
    }

    #[test]
    fn export_refuses_a_denylisted_task_family() {
        let candidate = base_candidate();
        let mut denylist = HeldOutDenylist::new();
        denylist.deny_task_family(candidate.task_family.clone());
        assert_eq!(
            export_trajectory(&candidate, &eligible(), &denylist),
            Err(ExportRefusal::HeldOutDenylisted)
        );
        assert!(export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).is_ok());
    }

    #[test]
    fn batch_refuses_every_synthetic_dev_record_sharing_a_family_with_a_held_out_record() {
        let mut held_out = base_candidate();
        held_out.split = TaskSplit::HeldOut;
        held_out.source_task_digest = "sha256:task-held-out".to_owned();

        let mut near_duplicate = base_candidate();
        near_duplicate.source_task_digest = "sha256:task-near-duplicate".to_owned();
        // Same task_family as `held_out` (both default to "family-a"):
        // this is exactly the "near-duplicate fixture variant that
        // individually claims SyntheticDev" case the issue calls out.
        assert_eq!(near_duplicate.task_family, held_out.task_family);

        let candidates = vec![held_out, near_duplicate];
        let eligibility = vec![eligible(), eligible()];
        let result = export_trajectory_batch(&candidates, &eligibility, &HeldOutDenylist::new());
        let Err(refusals) = result else {
            panic!("a batch containing a held-out record and its same-family sibling must be refused wholesale, got {result:?}");
        };
        assert_eq!(refusals.len(), 2, "both rows must be individually refused");
        assert_eq!(
            refusals[0],
            BatchRefusal {
                index: 0,
                refusal: ExportRefusal::HeldOutSplit
            }
        );
        assert_eq!(
            refusals[1],
            BatchRefusal {
                index: 1,
                refusal: ExportRefusal::HeldOutFamilyContamination
            },
            "the sibling row must be refused for family contamination, not silently dropped or silently exported"
        );

        // Positive control: the same near-duplicate candidate, exported
        // alone (no held-out sibling in the batch), succeeds -- proving
        // the refusal above is caused by the shared family with a
        // held-out row, not something inherent to the candidate itself.
        let alone = export_trajectory_batch(
            &[candidates[1].clone()],
            &[eligible()],
            &HeldOutDenylist::new(),
        );
        assert!(alone.is_ok(), "{alone:?}");
    }

    #[test]
    fn batch_exports_unrelated_families_independently() {
        let mut held_out = base_candidate();
        held_out.split = TaskSplit::HeldOut;
        held_out.task_family = "family-held-out".to_owned();
        held_out.source_task_digest = "sha256:task-held-out".to_owned();

        let mut unrelated = base_candidate();
        unrelated.task_family = "family-unrelated".to_owned();
        unrelated.source_task_digest = "sha256:task-unrelated".to_owned();

        let candidates = vec![held_out, unrelated];
        let eligibility = vec![eligible(), eligible()];
        let result = export_trajectory_batch(&candidates, &eligibility, &HeldOutDenylist::new());
        let Err(refusals) = result else {
            panic!("expected exactly the held-out row to be refused, got {result:?}");
        };
        assert_eq!(
            refusals,
            vec![BatchRefusal {
                index: 0,
                refusal: ExportRefusal::HeldOutSplit
            }],
            "an unrelated family must not be contaminated by a held-out row in a different family"
        );
    }

    // ---- Explicit opt-in ----

    #[test]
    fn export_requires_explicit_opt_in_selection() {
        let candidate = base_candidate();
        let mut not_selected = eligible();
        not_selected.explicitly_selected_for_learning_reuse = false;
        assert_eq!(
            export_trajectory(&candidate, &not_selected, &HeldOutDenylist::new()),
            Err(ExportRefusal::NotExplicitlySelected)
        );
        assert!(export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).is_ok());
    }

    // ---- Secret/credential/private-data eligibility flags ----

    /// One disqualifying eligibility flag: its name, and the mutation that
    /// sets it. Named so each flag is refused on its own rather than in a
    /// bundle -- a single test setting all five at once would pass even if
    /// four of them were never checked.
    type DisqualifyingFlag = (&'static str, fn(&mut EligibilityDeclaration));

    #[test]
    fn export_refuses_each_disqualifying_eligibility_flag_individually() {
        let candidate = base_candidate();

        let flags: [DisqualifyingFlag; 5] = [
            ("provider_secret", |e| e.carries_provider_secret = true),
            ("runtime_credential", |e| {
                e.carries_runtime_credential = true
            }),
            ("private_filesystem_path", |e| {
                e.carries_private_filesystem_path = true
            }),
            ("real_user_data", |e| e.carries_real_user_data = true),
            ("hidden_oracle_input", |e| {
                e.carries_hidden_oracle_input = true
            }),
        ];

        for (name, set_flag) in flags {
            let mut declaration = eligible();
            set_flag(&mut declaration);
            assert_eq!(
                export_trajectory(&candidate, &declaration, &HeldOutDenylist::new()),
                Err(ExportRefusal::SecretBearingSource),
                "flag {name} alone must refuse export"
            );
        }

        // Positive control: the fully clean declaration exports.
        assert!(export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).is_ok());
    }

    // ---- Independent acceptance: never the model grading itself ----

    #[test]
    fn export_refuses_model_self_reported_acceptance_but_accepts_every_independent_source() {
        let mut self_reported = base_candidate();
        self_reported.acceptance = IndependentAcceptance {
            source: AcceptanceSource::ModelSelfReport,
            decision: AcceptanceDecision::Accepted,
        };
        assert_eq!(
            export_trajectory(&self_reported, &eligible(), &HeldOutDenylist::new()),
            Err(ExportRefusal::ModelSelfReportedAcceptance)
        );

        for source in [
            AcceptanceSource::CanonicalVerifier,
            AcceptanceSource::IndependentReviewer,
            AcceptanceSource::IndependentTestSuite,
        ] {
            let mut candidate = base_candidate();
            candidate.acceptance = IndependentAcceptance {
                source,
                decision: AcceptanceDecision::Accepted,
            };
            assert!(
                export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).is_ok(),
                "independent source {source:?} must not be refused"
            );
        }
    }

    // ---- Rejected actions retain their labels; acceptance is attributable ----

    #[test]
    fn rejected_actions_retain_rejection_labels_through_export() {
        let mut candidate = base_candidate();
        candidate.canonical_validation_accepted = false;
        candidate.observed_execution_succeeded = false;
        candidate.acceptance = IndependentAcceptance {
            source: AcceptanceSource::IndependentReviewer,
            decision: AcceptanceDecision::Rejected,
        };
        let sanitized =
            export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        assert!(!sanitized.canonical_validation_accepted);
        assert!(!sanitized.observed_execution_succeeded);
        assert_eq!(sanitized.acceptance_decision, AcceptanceDecision::Rejected);
        assert_eq!(
            sanitized.acceptance_source,
            AcceptanceSource::IndependentReviewer
        );
    }

    #[test]
    fn canonical_validation_success_is_never_conflated_with_independent_acceptance() {
        // Compiles/validates fine, but an independent reviewer rejected it
        // anyway (e.g. it solved the wrong problem). Source compilation
        // must never be read back as acceptance.
        let mut candidate = base_candidate();
        candidate.canonical_validation_accepted = true;
        candidate.observed_execution_succeeded = true;
        candidate.acceptance = IndependentAcceptance {
            source: AcceptanceSource::IndependentReviewer,
            decision: AcceptanceDecision::Rejected,
        };
        let sanitized =
            export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        assert!(sanitized.canonical_validation_accepted);
        assert!(sanitized.observed_execution_succeeded);
        assert_eq!(
            sanitized.acceptance_decision,
            AcceptanceDecision::Rejected,
            "validation success must not be silently read back as acceptance"
        );
    }

    // ---- No raw secret content anywhere in exported content or diagnostics ----

    #[test]
    fn sanitized_content_and_refusal_diagnostics_never_contain_a_raw_marker() {
        const CONTEXT_MARKER: &[u8] = b"SECRET-CONTEXT-MARKER-do-not-leak";
        const ACTION_MARKER: &[u8] = b"SECRET-ACTION-MARKER-do-not-leak";
        const VALIDATION_MARKER: &[u8] = b"SECRET-VALIDATION-MARKER-do-not-leak";
        const OUTCOME_MARKER: &[u8] = b"SECRET-OUTCOME-MARKER-do-not-leak";
        const REPAIR_MARKER: &[u8] = b"SECRET-REPAIR-MARKER-do-not-leak";
        const PATH_MARKER: &str = "/Users/SECRET-USER/do-not-leak";

        let mut candidate = base_candidate();
        candidate.context_bytes = CONTEXT_MARKER.to_vec();
        candidate.proposed_action_bytes = ACTION_MARKER.to_vec();
        candidate.canonical_validation_bytes = VALIDATION_MARKER.to_vec();
        candidate.observed_execution_bytes = OUTCOME_MARKER.to_vec();
        candidate.repair_bytes = Some(REPAIR_MARKER.to_vec());

        // Positive control: the markers really are present in the raw
        // candidate, so their absence below is not vacuous.
        assert!(candidate
            .context_bytes
            .windows(CONTEXT_MARKER.len())
            .any(|w| w == CONTEXT_MARKER));

        let sanitized =
            export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        let rendered = sanitized.render();
        for marker in [
            std::str::from_utf8(CONTEXT_MARKER).unwrap(),
            std::str::from_utf8(ACTION_MARKER).unwrap(),
            std::str::from_utf8(VALIDATION_MARKER).unwrap(),
            std::str::from_utf8(OUTCOME_MARKER).unwrap(),
            std::str::from_utf8(REPAIR_MARKER).unwrap(),
        ] {
            assert!(
                !rendered.contains(marker),
                "sanitized rendering must never contain marker {marker}"
            );
        }

        // A refusal built from a candidate carrying the same markers must
        // also never surface them: its diagnostic text is a fixed,
        // closed-vocabulary string, never a template over candidate bytes.
        let mut held_out_with_secret = candidate;
        held_out_with_secret.split = TaskSplit::HeldOut;
        held_out_with_secret.context_bytes =
            format!("{}{}", String::from_utf8_lossy(CONTEXT_MARKER), PATH_MARKER).into_bytes();
        let refusal =
            export_trajectory(&held_out_with_secret, &eligible(), &HeldOutDenylist::new())
                .unwrap_err();
        let diagnostic = refusal.diagnostic();
        assert!(!diagnostic.message.contains(PATH_MARKER));
        assert!(!diagnostic
            .message
            .contains(std::str::from_utf8(CONTEXT_MARKER).unwrap()));
    }

    // ---- Sanitized derivatives cannot masquerade as original evidence ----

    #[test]
    fn sanitized_digest_never_equals_any_digest_it_references() {
        // Deliberately construct a candidate whose provenance evidence
        // digest is the caller's own commitment over the exact same bytes
        // used for context/action, to stress-test that domain separation
        // (not merely differing input bytes) is what keeps the digests
        // apart.
        let mut candidate = base_candidate();
        candidate.provenance_original_evidence_digest =
            commit_context_bytes(&candidate.context_bytes);
        let sanitized =
            export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();

        assert_ne!(
            sanitized.digest(),
            sanitized.provenance_original_evidence_digest
        );
        assert_ne!(sanitized.digest(), sanitized.context_digest);
        assert_ne!(sanitized.digest(), sanitized.action_digest);
        assert_ne!(sanitized.digest(), sanitized.canonical_validation_digest);
        assert_ne!(sanitized.digest(), sanitized.execution_outcome_digest);
    }

    #[test]
    fn single_field_mutation_changes_the_sanitized_digest() {
        let candidate = base_candidate();
        let sanitized =
            export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        let original_digest = sanitized.digest();

        let mut mutated = sanitized.clone();
        mutated.canonical_validation_accepted = !mutated.canonical_validation_accepted;
        assert_ne!(
            mutated.digest(),
            original_digest,
            "flipping a single boolean field must change the sanitized digest"
        );
    }

    // ---- Low-entropy payloads ----

    #[test]
    fn low_entropy_context_never_earns_a_withheld_or_safe_claim() {
        let mut candidate = base_candidate();
        candidate.context_bytes = b"hi".to_vec();
        let sanitized =
            export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        assert_eq!(
            sanitized.context_privacy_claim,
            PayloadPrivacyClaim::DigestOnlyLowEntropyCaveat
        );
        assert!(sanitized
            .render()
            .contains("digest_only_low_entropy_caveat"));

        let mut long_candidate = base_candidate();
        long_candidate.context_bytes = vec![b'x'; 4096];
        let long_sanitized =
            export_trajectory(&long_candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        assert_eq!(
            long_sanitized.context_privacy_claim,
            PayloadPrivacyClaim::DigestOnly
        );
    }

    // ---- Purity / no ambient host access ----

    #[test]
    fn export_trajectory_is_pure_and_deterministic_across_repeated_calls() {
        let candidate = base_candidate();
        let first = export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        let second = export_trajectory(&candidate, &eligible(), &HeldOutDenylist::new()).unwrap();
        assert_eq!(first.digest(), second.digest());
        assert_eq!(first, second);
    }

    #[test]
    fn commit_helpers_are_deterministic_and_domain_separated_from_each_other() {
        let bytes = b"same content";
        assert_eq!(commit_context_bytes(bytes), commit_context_bytes(bytes));
        assert_ne!(commit_context_bytes(bytes), commit_action_bytes(bytes));
        assert_ne!(commit_context_bytes(bytes), commit_validation_bytes(bytes));
        assert_ne!(commit_context_bytes(bytes), commit_outcome_bytes(bytes));
        assert_ne!(commit_context_bytes(bytes), commit_repair_bytes(bytes));
    }
}
