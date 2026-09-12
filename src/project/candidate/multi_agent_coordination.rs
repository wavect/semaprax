//! Multi-agent semantic transaction coordination: proof data, not a scheduler.
//!
//! See [`docs/MULTI-AGENT-COORDINATION-V1.md`](../../../docs/MULTI-AGENT-COORDINATION-V1.md)
//! for the full specification. This closes
//! [#207](https://github.com/wavect/semaprax/issues/207). It composes the
//! existing candidate evidence surface
//! ([`crate::project::candidate::impact_navigation`]) and reuses the
//! independent-acceptance-boundary shape
//! [`crate::project::candidate::candidate_assurance`] (#129) established: it
//! never re-implements revision binding or impact analysis, and it never
//! runs an agent, applies a rebase, builds a merged candidate, executes a
//! target, or publishes anything. Every function here returns a bounded,
//! deterministic evidence document -- "a settlement or concurrency model is
//! proof data, not permission to perform a physical finalizer, spawn runtime
//! work, or publish an artifact" (`AGENTS.md`).
//!
//! [`ProjectCandidate::open_coordination_session`] binds a bounded set of
//! agent participants, each to an explicit granted scope of stable
//! declaration `@id`s that must exist in this exact candidate --
//! independently checked against the candidate's own compiler-owned impact
//! artifact via [`ProjectCandidate::impact_summary`], never trusted from
//! caller input -- plus a coordinator identity that must be distinct from
//! every participant. That is the same "cannot approve yourself" shape
//! `candidate_assurance` uses for `reviewer != proposer`, applied here as
//! "the session governor cannot itself be a working agent whose own
//! proposals it must impartially classify".
//!
//! [`ProjectCandidate::evaluate_agent_proposals`] classifies a batch of
//! typed per-agent proposals against that session. It fails closed
//! (`SPX-Z503`) when the session is bound to a different candidate revision
//! than `self` -- the "intervening edit" case -- and treats a single
//! proposal's own stale base revision, unknown agent id, or out-of-scope
//! target as a *recorded rejection* rather than aborting the whole batch:
//! one bad proposal never hides the classification of the others. Two
//! surviving proposals from different agents that name the same stable id
//! are a `same_target` conflict: both intentions, the affected ids, and a
//! closed set of resolution choices are recorded, and neither is silently
//! preferred. The surviving, non-conflicting subset is handed back as one
//! deterministic `compatible_order` -- guidance for a *separate* authorized
//! rebase/merge invocation. This module never performs that rebase, builds a
//! candidate from it, or grants any authority to do so: `merge_authority`,
//! `publication_authority`, and `execution_authority` are always `false` in
//! every record this module renders, and no function here ever applies a
//! change, runs a target, or writes source.
//!
//! Graph-derived data (`graph_signal`, from the existing
//! [`ProjectCandidate::impact_summary`] artifact) is attached to each granted
//! scope id as descriptive guidance only when a session opens; it is never
//! compared, thresholded, or used to decide compatibility there. Evaluation
//! does use the graph for one proven purpose: two surviving proposals from
//! different agents whose target sets are disjoint (no `same_target`
//! overlap) are still a `cross_target` conflict when the *existing*
//! six-edge-family reverse impact artifact
//! ([`ProjectRevision::semantic_impact`], `direction: reverse`) shows one
//! proposal's target id in the other's `affected` set -- i.e. a real,
//! already-computed dependency edge (for example, one target calls the
//! other) rather than a heuristic this module invents.
//! [`tests::disjoint_but_graph_dependent_targets_are_flagged_as_a_cross_target_conflict`]
//! proves this. "Graph independence can miss hidden external/generated/
//! deployment coupling" remains a named failure case this module does not
//! claim to solve: the reverse impact artifact only covers its own six edge
//! families, so an id pair connected only through coupling outside those
//! families -- deployment, generated output, an external consumer -- is
//! still reported compatible, and the nonclaims say so rather than papering
//! over the remaining gap.
//!
//! [`record_scheduling_comparison`] is the bounded "scheduling/economics
//! evidence" the issue's in-scope list names: a pure function over
//! caller-*observed* sequential/parallel run metrics (this module runs
//! nothing itself), producing a comparison record with the same
//! `execution_authority: false` posture.
//!
//! Diagnostics use the previously unused `SPX-Z5xx` family:
//! - `SPX-Z501`: invalid input (empty/oversized/duplicate participant or
//!   proposal field, zero participants/proposals, a coordinator identity
//!   equal to a participant's).
//! - `SPX-Z502`: capacity exceeded (too many participants/proposals/ids, an
//!   out-of-bounds scheduling metric, or a rendered record over its byte
//!   budget).
//! - `SPX-Z503`: stale (a coordination session bound to a different exact
//!   candidate than `self`, or a granted scope id that does not name a
//!   declaration this exact candidate carries).
//! - `SPX-Z504`: refused (the coordinator identity equals a participant's;
//!   a session cannot be governed by one of its own working agents).
//!
//! A per-proposal rejection (`unknown_agent`, `stale_base_revision`,
//! `scope_violation`) is recorded as data in the evaluation record rather
//! than one of the above codes: it is expected batch content, not a
//! malformed call.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::workspace_analysis::{WorkspaceAnalysisTargetKind, WorkspaceImpactOptions};

use super::{wire, ProjectCandidate};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const COORDINATION_SESSION_SCHEMA: &str = "semaprax.multi-agent-coordination-session.v1";
pub const COORDINATION_EVALUATION_SCHEMA: &str = "semaprax.multi-agent-coordination-evaluation.v1";
pub const SCHEDULING_COMPARISON_SCHEMA: &str = "semaprax.multi-agent-scheduling-comparison.v1";

pub const MAX_COORDINATION_SESSION_BYTES: usize = 256 * 1024;
pub const MAX_COORDINATION_EVALUATION_BYTES: usize = 1024 * 1024;
pub const MAX_SCHEDULING_COMPARISON_BYTES: usize = 16 * 1024;
pub const MAX_COORDINATION_PARTICIPANTS: usize = 16;
pub const MAX_SCOPE_IDS_PER_PARTICIPANT: usize = 32;
pub const MAX_COORDINATION_IDENTITY_BYTES: usize = 256;
pub const MAX_PROPOSALS: usize = 64;
pub const MAX_TARGET_IDS_PER_PROPOSAL: usize = 16;
pub const MAX_INTENTION_BYTES: usize = 4096;
/// Caller-observed metrics are opaque counters, not currency or wall-clock
/// units this module measures itself; bounded only to keep a hostile input
/// from inflating a rendered record without limit.
pub const MAX_SCHEDULING_METRIC: u64 = 1_000_000_000;

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-Z501", message.into())]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-Z502", message.into())]
}
fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-Z503", message.into())]
}
fn refused(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-Z504", message.into())]
}

fn validate_identity(identity: &str) -> Result<()> {
    if identity.is_empty() || identity.len() > MAX_COORDINATION_IDENTITY_BYTES {
        return Err(invalid(
            "coordination identity must be non-empty and bounded",
        ));
    }
    Ok(())
}

/// The stable ids the existing six-edge-family reverse impact artifact
/// records as depending on `target` (its `affected` set, `direction:
/// reverse`, the artifact's own default): real, already-computed dependency
/// edges such as "this id calls `target`", never a claim about coupling
/// outside those six edge families. Reuses
/// [`ProjectCandidate::impact_summary`]'s own compiler artifact rather than
/// a parallel graph walk. `target` is required to already exist in `self`
/// (every caller here draws it from a granted scope this exact candidate
/// has already verified), so a failure is treated as an unexpected staleness
/// rather than a caller input error.
fn reverse_dependents(candidate: &ProjectCandidate, target: &str) -> Result<BTreeSet<String>> {
    let report = candidate
        .revision
        .semantic_impact(
            WorkspaceAnalysisTargetKind::Declaration,
            target,
            WorkspaceImpactOptions::default(),
        )
        .map_err(|_| {
            stale(
                "coordination cross-target dependency check could not recompute the reverse \
                 impact artifact for a previously granted scope id",
            )
        })?;
    let value: Value =
        serde_json::from_str(&report).expect("semantic_impact always renders valid JSON");
    let affected = value["affected"]
        .as_array()
        .expect("semantic_impact always renders an affected array");
    Ok(affected
        .iter()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect())
}

/// One caller-declared agent participant granted into a coordination
/// session: an identity plus a bounded, explicit scope of stable
/// declaration ids it may target, and an opaque budget-unit count recorded
/// as evidence only -- this module never enforces it (a real deployment
/// enforces budgets the way [`crate::live_invocation::budget`] does for one
/// invocation).
#[derive(Clone, Copy, Debug)]
pub struct CoordinationParticipant<'a> {
    pub agent_id: &'a str,
    pub granted_scope: &'a [&'a str],
    pub budget_units: u64,
}

/// The typed operation classes this module classifies conflicts over,
/// matching the issue's named conflict-class list (same target plus
/// call/type/contract/effect/ownership/requirement/ABI/architecture/test),
/// with `GeneratedArtifact` covering contracted generated artifacts. Purely
/// a caller-declared label carried through into evidence: this module does
/// not independently verify that a proposal's declared class matches what
/// its target ids actually are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationClass {
    Call,
    Type,
    Test,
    GeneratedArtifact,
    Requirement,
    Architecture,
    PublicAbi,
    Contract,
    Effect,
    Ownership,
}

impl OperationClass {
    pub fn token(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Type => "type",
            Self::Test => "test",
            Self::GeneratedArtifact => "generated_artifact",
            Self::Requirement => "requirement",
            Self::Architecture => "architecture",
            Self::PublicAbi => "public_abi",
            Self::Contract => "contract",
            Self::Effect => "effect",
            Self::Ownership => "ownership",
        }
    }
}

/// One agent's typed transaction proposal: exact preconditions
/// (`declared_base_project_revision`, the exact target ids it intends to
/// touch) plus its declared intention and operation class. Carries no
/// authority of its own; [`ProjectCandidate::evaluate_agent_proposals`]
/// independently rebinds every precondition against the session it is
/// evaluated against.
#[derive(Clone, Copy, Debug)]
pub struct AgentProposal<'a> {
    pub agent_id: &'a str,
    pub declared_base_project_revision: &'a str,
    pub target_ids: &'a [&'a str],
    pub operation_class: OperationClass,
    pub intention: &'a str,
}

impl ProjectCandidate {
    /// Open a coordination session bound to this exact candidate: a bounded
    /// set of agent participants, each granted an explicit, independently
    /// verified scope of stable declaration ids, governed by a coordinator
    /// identity that must differ from every participant.
    ///
    /// Fails closed (`SPX-Z503`) when a granted scope id does not name a
    /// declaration this exact candidate carries: existence is checked via
    /// [`Self::impact_summary`], never trusted from the caller. Refuses
    /// outright (`SPX-Z504`) when `coordinator_id` equals a participant's
    /// `agent_id`: a session cannot be governed by one of its own working
    /// agents, mirroring `candidate_assurance`'s `reviewer != proposer`.
    ///
    /// The granted scope recorded here is a *capability boundary* enforced
    /// later by [`Self::evaluate_agent_proposals`], not a claim that scopes
    /// which do not overlap are semantically independent: `graph_signal`
    /// attaches existing impact-artifact item counts per id as descriptive
    /// guidance only, never compared or thresholded.
    pub fn open_coordination_session(
        &self,
        expected_candidate: &str,
        coordinator_id: &str,
        participants: &[CoordinationParticipant<'_>],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        validate_identity(coordinator_id)?;
        if participants.is_empty() || participants.len() > MAX_COORDINATION_PARTICIPANTS {
            return Err(capacity(
                "coordination session participant count must be 1..=16",
            ));
        }

        let mut seen_agents: BTreeSet<String> = BTreeSet::new();
        let mut participant_records = Vec::with_capacity(participants.len());
        for participant in participants {
            validate_identity(participant.agent_id)?;
            if participant.agent_id == coordinator_id {
                return Err(refused(
                    "coordination session coordinator identity must be distinct from every \
                     participant agent id; a session governor cannot itself be a working agent",
                ));
            }
            if !seen_agents.insert(participant.agent_id.to_owned()) {
                return Err(invalid(
                    "coordination session participant agent id is repeated",
                ));
            }
            if participant.granted_scope.is_empty()
                || participant.granted_scope.len() > MAX_SCOPE_IDS_PER_PARTICIPANT
            {
                return Err(capacity(
                    "coordination session granted scope must have 1..=32 declaration ids",
                ));
            }

            let mut scope_ids: BTreeSet<String> = BTreeSet::new();
            let mut graph_signal = Vec::with_capacity(participant.granted_scope.len());
            for id in participant.granted_scope {
                if id.is_empty() || id.len() > MAX_COORDINATION_IDENTITY_BYTES {
                    return Err(invalid(
                        "coordination session scope id must be non-empty and bounded",
                    ));
                }
                if !scope_ids.insert((*id).to_owned()) {
                    return Err(invalid(
                        "coordination session scope id is repeated for one participant",
                    ));
                }
                // Independent existence check: never trust the caller's claim
                // that this id names a real declaration in this exact
                // candidate. Reuses the existing impact artifact rather than a
                // parallel lookup, and doubles as the real "graph and impact
                // edges" signal the derived partition carries as guidance.
                let summary = self
                    .impact_summary(expected_candidate, id, WorkspaceImpactOptions::default())
                    .map_err(|_| {
                        stale(
                            "coordination session scope id does not name a declaration this \
                             exact candidate carries",
                        )
                    })?;
                let summary: Value = serde_json::from_str(&summary)
                    .expect("impact_summary always renders valid JSON");
                let facets = summary["facets"]
                    .as_array()
                    .expect("impact_summary always renders a facets array");
                let mut counts = serde_json::Map::new();
                for facet in facets {
                    let view = facet["view"]
                        .as_str()
                        .expect("impact_summary facet view is always a string");
                    counts.insert(view.to_owned(), facet["total_items"].clone());
                }
                graph_signal.push(json!({
                    "id": id,
                    "total_items_by_view": Value::Object(counts),
                }));
            }

            participant_records.push(json!({
                "agent_id": participant.agent_id,
                "granted_scope": scope_ids.into_iter().collect::<Vec<_>>(),
                "budget_units": participant.budget_units,
                "graph_signal": graph_signal,
            }));
        }

        let value = json!({
            "schema": COORDINATION_SESSION_SCHEMA,
            "candidate_revision": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "project_revision": self.revision.project_revision(),
            "coordinator_id": coordinator_id,
            "participants": participant_records,
            "coordination_authority": false,
            "publication_authority": false,
            "execution_authority": false,
            "candidate_retained": false,
            "nonclaims": [
                "no_agent_is_spawned_scheduled_or_invoked_by_this_session",
                "granted_scope_is_a_capability_boundary_not_proof_of_independence",
                "graph_signal_is_descriptive_impact_artifact_guidance_never_thresholded",
                "graph_independence_can_miss_hidden_external_or_generated_coupling",
                "not_publication_or_execution_authority",
            ],
        });
        wire::render(value, MAX_COORDINATION_SESSION_BYTES)
    }

    /// Classify a batch of typed agent proposals against a session this
    /// exact candidate previously opened.
    ///
    /// Fails closed (`SPX-Z503`) when `session`'s `candidate_revision` does
    /// not match `self.candidate_digest()`: a session opened against a
    /// candidate that has since changed (an intervening edit) can never be
    /// reused to evaluate proposals against the new candidate. Within a
    /// structurally valid session, each proposal is independently checked
    /// and, if it fails, *recorded as a rejection* rather than aborting the
    /// batch: `unknown_agent` (not a session participant),
    /// `stale_base_revision` (does not match the session's exact bound
    /// base), or `scope_violation` (a target id outside that agent's
    /// granted scope -- enforced from the session's own record, never from
    /// what the proposal itself claims). Two surviving proposals from
    /// different agents that share a target id are a `same_target`
    /// conflict; two surviving proposals from different agents with
    /// disjoint target ids are a `cross_target` conflict when the existing
    /// reverse impact artifact shows a real dependency edge between one
    /// proposal's target and the other's (see the module documentation).
    /// Both classes are recorded with both intentions, the affected ids, and
    /// a closed set of resolution choices; the remaining, non-conflicting
    /// proposals are returned as one deterministic `compatible_order`, which
    /// is guidance for a separate authorized rebase/merge invocation and
    /// never itself a merge, rebase, or candidate build.
    pub fn evaluate_agent_proposals(
        &self,
        expected_candidate: &str,
        session: &str,
        proposals: &[AgentProposal<'_>],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if session.is_empty() || session.len() > MAX_COORDINATION_SESSION_BYTES {
            return Err(capacity(
                "coordination session record must be non-empty and bounded",
            ));
        }
        let session_value: Value = serde_json::from_str(session)
            .map_err(|_| invalid("coordination session record is not valid JSON"))?;
        if session_value["schema"].as_str() != Some(COORDINATION_SESSION_SCHEMA) {
            return Err(invalid(
                "coordination session record has an unexpected schema",
            ));
        }
        // The independent rebind: a session is only ever evaluated against
        // the exact candidate it was opened against, never a different or
        // later one.
        if session_value["candidate_revision"].as_str() != Some(self.candidate_digest()) {
            return Err(stale(
                "coordination session is bound to a different candidate revision than this \
                 exact candidate; open a fresh session before evaluating proposals against it",
            ));
        }
        let base_project_revision = session_value["base_project_revision"]
            .as_str()
            .ok_or_else(|| invalid("coordination session record has no base project revision"))?
            .to_owned();

        let mut granted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let session_participants = session_value["participants"]
            .as_array()
            .ok_or_else(|| invalid("coordination session record has no participants array"))?;
        for participant in session_participants {
            let agent_id = participant["agent_id"]
                .as_str()
                .ok_or_else(|| invalid("coordination session participant has no agent id"))?;
            let scope = participant["granted_scope"]
                .as_array()
                .ok_or_else(|| invalid("coordination session participant has no granted scope"))?
                .iter()
                .map(|id| id.as_str().map(str::to_owned))
                .collect::<Option<BTreeSet<_>>>()
                .ok_or_else(|| invalid("coordination session granted scope id is not a string"))?;
            granted.insert(agent_id.to_owned(), scope);
        }

        if proposals.is_empty() || proposals.len() > MAX_PROPOSALS {
            return Err(capacity("agent proposal batch must have 1..=64 proposals"));
        }

        struct Accepted<'a> {
            index: usize,
            agent_id: &'a str,
            target_ids: BTreeSet<&'a str>,
            operation_class: OperationClass,
            intention: &'a str,
        }

        let mut rejected: Vec<Value> = Vec::new();
        let mut accepted: Vec<Accepted<'_>> = Vec::new();

        for (index, proposal) in proposals.iter().enumerate() {
            validate_identity(proposal.agent_id)?;
            if proposal.intention.is_empty() || proposal.intention.len() > MAX_INTENTION_BYTES {
                return Err(invalid(
                    "agent proposal intention must be non-empty and bounded",
                ));
            }
            if proposal.target_ids.is_empty()
                || proposal.target_ids.len() > MAX_TARGET_IDS_PER_PROPOSAL
            {
                return Err(capacity("agent proposal target id count must be 1..=16"));
            }
            let mut target_set: BTreeSet<&str> = BTreeSet::new();
            for id in proposal.target_ids {
                if id.is_empty() || id.len() > MAX_COORDINATION_IDENTITY_BYTES {
                    return Err(invalid(
                        "agent proposal target id must be non-empty and bounded",
                    ));
                }
                if !target_set.insert(id) {
                    return Err(invalid("agent proposal target id is repeated"));
                }
            }

            let Some(scope) = granted.get(proposal.agent_id) else {
                rejected.push(json!({
                    "index": index,
                    "agent_id": proposal.agent_id,
                    "reason": "unknown_agent",
                }));
                continue;
            };
            if proposal.declared_base_project_revision != base_project_revision {
                rejected.push(json!({
                    "index": index,
                    "agent_id": proposal.agent_id,
                    "reason": "stale_base_revision",
                }));
                continue;
            }
            if !target_set.iter().all(|id| scope.contains(*id)) {
                rejected.push(json!({
                    "index": index,
                    "agent_id": proposal.agent_id,
                    "reason": "scope_violation",
                }));
                continue;
            }
            accepted.push(Accepted {
                index,
                agent_id: proposal.agent_id,
                target_ids: target_set,
                operation_class: proposal.operation_class,
                intention: proposal.intention,
            });
        }

        // Every distinct target id named by a surviving proposal has its
        // existing reverse impact artifact (the same six-edge-family
        // artifact `open_coordination_session` already uses for scope
        // existence) computed once and memoized here, so an id shared by
        // several proposals is never recomputed.
        let mut reverse_cache: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        for entry in &accepted {
            for id in entry.target_ids.iter().copied() {
                if let std::collections::btree_map::Entry::Vacant(slot) = reverse_cache.entry(id) {
                    slot.insert(reverse_dependents(self, id)?);
                }
            }
        }

        // Pairwise conflict detection among survivors from different
        // agents; two proposals from the same agent sharing a target are
        // that agent's own sequencing choice, not a coordination conflict
        // between agents.
        let mut conflicting_indices: BTreeSet<usize> = BTreeSet::new();
        let mut conflicts: Vec<Value> = Vec::new();
        for i in 0..accepted.len() {
            for j in (i + 1)..accepted.len() {
                let (a, b) = (&accepted[i], &accepted[j]);
                if a.agent_id == b.agent_id {
                    continue;
                }
                let overlap: Vec<&str> =
                    a.target_ids.intersection(&b.target_ids).copied().collect();
                if !overlap.is_empty() {
                    conflicting_indices.insert(a.index);
                    conflicting_indices.insert(b.index);
                    conflicts.push(json!({
                        "conflict_class": "same_target",
                        "affected_ids": overlap,
                        "proposals": [
                            {
                                "index": a.index, "agent_id": a.agent_id,
                                "intention": a.intention, "operation_class": a.operation_class.token(),
                            },
                            {
                                "index": b.index, "agent_id": b.agent_id,
                                "intention": b.intention, "operation_class": b.operation_class.token(),
                            },
                        ],
                        "allowed_resolution_choices": [
                            "prefer_first_proposal", "prefer_second_proposal",
                            "manual_reconciliation", "abandon_both",
                        ],
                    }));
                    continue;
                }

                // Disjoint targets can still be a `cross_target` conflict:
                // the existing reverse impact artifact for one proposal's
                // target names the other proposal's target in its
                // `affected` set, i.e. a real, already-computed dependency
                // edge (for example, one target calls the other) rather
                // than a heuristic this module invents.
                let mut affected_ids: BTreeSet<&str> = BTreeSet::new();
                let mut dependency_witnesses: Vec<Value> = Vec::new();
                for &upstream in &a.target_ids {
                    for &downstream in &b.target_ids {
                        if reverse_cache[upstream].contains(downstream) {
                            affected_ids.insert(upstream);
                            affected_ids.insert(downstream);
                            dependency_witnesses
                                .push(json!({"upstream": upstream, "downstream": downstream}));
                        }
                        if reverse_cache[downstream].contains(upstream) {
                            affected_ids.insert(upstream);
                            affected_ids.insert(downstream);
                            dependency_witnesses
                                .push(json!({"upstream": downstream, "downstream": upstream}));
                        }
                    }
                }
                if dependency_witnesses.is_empty() {
                    continue;
                }
                conflicting_indices.insert(a.index);
                conflicting_indices.insert(b.index);
                conflicts.push(json!({
                    "conflict_class": "cross_target",
                    "affected_ids": affected_ids.into_iter().collect::<Vec<_>>(),
                    "dependency_witnesses": dependency_witnesses,
                    "proposals": [
                        {
                            "index": a.index, "agent_id": a.agent_id,
                            "intention": a.intention, "operation_class": a.operation_class.token(),
                        },
                        {
                            "index": b.index, "agent_id": b.agent_id,
                            "intention": b.intention, "operation_class": b.operation_class.token(),
                        },
                    ],
                    "allowed_resolution_choices": [
                        "prefer_first_proposal", "prefer_second_proposal",
                        "manual_reconciliation", "abandon_both",
                    ],
                }));
            }
        }

        let mut compatible: Vec<&Accepted<'_>> = accepted
            .iter()
            .filter(|entry| !conflicting_indices.contains(&entry.index))
            .collect();
        compatible.sort_by(|a, b| a.agent_id.cmp(b.agent_id).then(a.index.cmp(&b.index)));
        let compatible_order: Vec<Value> = compatible
            .iter()
            .map(|entry| {
                json!({
                    "index": entry.index,
                    "agent_id": entry.agent_id,
                    "target_ids": entry.target_ids.iter().collect::<Vec<_>>(),
                    "operation_class": entry.operation_class.token(),
                })
            })
            .collect();

        let value = json!({
            "schema": COORDINATION_EVALUATION_SCHEMA,
            "candidate_revision": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "session_base_project_revision": base_project_revision,
            "proposals_total": proposals.len(),
            "rejected": rejected,
            "conflicts": conflicts,
            "compatible_order": compatible_order,
            "merge_authority": false,
            "publication_authority": false,
            "execution_authority": false,
            "candidate_retained": false,
            "nonclaims": [
                "does_not_rebase_merge_or_build_any_candidate",
                "does_not_execute_or_schedule_any_agent",
                "compatible_order_is_guidance_for_a_separate_authorized_invocation",
                "same_target_and_cross_target_via_the_existing_reverse_impact_artifact_are_the_only_proven_automatic_conflict_detection",
                "cross_target_detection_covers_only_the_reverse_impact_artifacts_own_six_edge_families",
                "operation_class_is_caller_declared_not_independently_verified",
                "graph_independence_can_miss_hidden_external_or_generated_coupling",
            ],
        });
        wire::render(value, MAX_COORDINATION_EVALUATION_BYTES)
    }
}

/// One side's caller-*observed* run metrics (not measured by this function)
/// over an actual sequential or actual parallel multi-agent run.
#[derive(Clone, Copy, Debug, Default)]
pub struct SchedulingObservation {
    pub agent_count: u32,
    pub wall_clock_units: u64,
    pub retries: u32,
    pub cost_units: u64,
    pub review_items_opened: u32,
}

fn validate_observation(observation: SchedulingObservation) -> Result<()> {
    if observation.agent_count == 0 || observation.agent_count > 1024 {
        return Err(capacity(
            "scheduling observation agent count must be 1..=1024",
        ));
    }
    if observation.wall_clock_units == 0 || observation.wall_clock_units > MAX_SCHEDULING_METRIC {
        return Err(capacity(
            "scheduling observation wall clock units must be nonzero and bounded",
        ));
    }
    if observation.cost_units > MAX_SCHEDULING_METRIC
        || u64::from(observation.retries) > MAX_SCHEDULING_METRIC
        || u64::from(observation.review_items_opened) > MAX_SCHEDULING_METRIC
    {
        return Err(capacity("scheduling observation metric exceeds its bound"));
    }
    Ok(())
}

fn observation_value(observation: SchedulingObservation) -> Value {
    json!({
        "agent_count": observation.agent_count,
        "wall_clock_units": observation.wall_clock_units,
        "retries": observation.retries,
        "cost_units": observation.cost_units,
        "review_items_opened": observation.review_items_opened,
    })
}

/// Record one comparison between a caller-*observed* sequential run and a
/// caller-*observed* parallel run: the "scheduling/economics evidence" the
/// issue's in-scope list names. This function performs no execution,
/// scheduling, or agent invocation itself -- it only computes ratios and
/// deltas over the two supplied observations, which the caller must have
/// obtained from actually running (and actually reviewing) both.
pub fn record_scheduling_comparison(
    sequential: SchedulingObservation,
    parallel: SchedulingObservation,
) -> Result<String> {
    validate_observation(sequential)?;
    validate_observation(parallel)?;

    let wall_clock_speedup = sequential.wall_clock_units as f64 / parallel.wall_clock_units as f64;
    let cost_ratio = parallel.cost_units as f64 / sequential.cost_units.max(1) as f64;
    let review_burden_delta =
        i64::from(parallel.review_items_opened) - i64::from(sequential.review_items_opened);
    let retries_delta = i64::from(parallel.retries) - i64::from(sequential.retries);

    let value = json!({
        "schema": SCHEDULING_COMPARISON_SCHEMA,
        "sequential": observation_value(sequential),
        "parallel": observation_value(parallel),
        "wall_clock_speedup": wall_clock_speedup,
        "cost_ratio": cost_ratio,
        "review_burden_delta": review_burden_delta,
        "retries_delta": retries_delta,
        "execution_authority": false,
        "publication_authority": false,
        "nonclaims": [
            "caller_observed_local_counts_only_not_measured_or_verified_by_this_function",
            "does_not_run_execute_or_schedule_any_agent",
            "not_a_cost_or_capacity_prediction_for_a_future_run",
        ],
    });
    wire::render(value, MAX_SCHEDULING_COMPARISON_BYTES)
}

#[cfg(test)]
mod tests;
