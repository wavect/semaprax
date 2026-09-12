# Multi-Agent Coordination v1

Status: implemented bounded profile; local evidence only (see "Known
limitations" below). Reuses the independent-acceptance-boundary shape
[Project Candidate Assurance and Acceptance v1](PROJECT-CANDIDATE-ASSURANCE-ACCEPTANCE-V1.md)
(#129) established and composes the existing
[Project Candidate Impact Navigation v1](PROJECT-CANDIDATE-IMPACT-NAVIGATION-V1.md)
artifact; neither module is modified.

Audience: agents and reviewers coordinating more than one working agent
against one exact candidate, and compiler contributors extending candidate
evidence.

This closes [#207](https://github.com/wavect/semaprax/issues/207). It adds
exactly three things, all implemented by
`src/project/candidate/multi_agent_coordination.rs`:
`ProjectCandidate::open_coordination_session`,
`ProjectCandidate::evaluate_agent_proposals`, and the free function
`record_scheduling_comparison`. It does not implement automatic rebase,
automatic merge, a task scheduler, or an agent runner: this is proof data
that a *separate*, already-authorized invocation (an ordinary
`ProjectCandidate` rebase/build/test-selection/publication path) can consume,
never a path that performs any of those itself.

## Bounded scope and the trap this issue names

> "A settlement or concurrency model is proof data, not permission to perform
> a physical finalizer, spawn runtime work, or publish an artifact."
> (`AGENTS.md`)

A "multi-agent coordination" feature invites building a scheduler that
actually runs concurrent agents, actually rebases their changes, and actually
publishes the result. This module deliberately does none of that. Every
function here:

- never spawns, invokes, or schedules an agent process or model call;
- never applies a semantic change, builds a rebase, or constructs a merged
  candidate from the proposals it classifies;
- never executes a target or runs project tests;
- never grants `merge_authority`, `publication_authority`, or
  `execution_authority` -- every rendered record sets all three `false`, by
  construction, in every code path.

What it *does* do: given a bounded, caller-supplied batch of typed agent
proposals against one exact candidate, it independently classifies which
proposals are structurally safe to hand to a separate authorized
rebase/merge step, which conflict with each other, and which are stale or
out of scope -- and it renders that classification as one deterministic
evidence document. Nothing more.

## Library API and binding

```rust
pub struct CoordinationParticipant<'a> {
    pub agent_id: &'a str,
    pub granted_scope: &'a [&'a str],
    pub budget_units: u64,
}

pub enum OperationClass {
    Call, Type, Test, GeneratedArtifact, Requirement,
    Architecture, PublicAbi, Contract, Effect, Ownership,
}

pub struct AgentProposal<'a> {
    pub agent_id: &'a str,
    pub declared_base_project_revision: &'a str,
    pub target_ids: &'a [&'a str],
    pub operation_class: OperationClass,
    pub intention: &'a str,
}

impl ProjectCandidate {
    pub fn open_coordination_session(
        &self,
        expected_candidate: &str,
        coordinator_id: &str,
        participants: &[CoordinationParticipant<'_>],
    ) -> Result<String, Vec<Diagnostic>>;

    pub fn evaluate_agent_proposals(
        &self,
        expected_candidate: &str,
        session: &str,
        proposals: &[AgentProposal<'_>],
    ) -> Result<String, Vec<Diagnostic>>;
}

pub struct SchedulingObservation {
    pub agent_count: u32,
    pub wall_clock_units: u64,
    pub retries: u32,
    pub cost_units: u64,
    pub review_items_opened: u32,
}

pub fn record_scheduling_comparison(
    sequential: SchedulingObservation,
    parallel: SchedulingObservation,
) -> Result<String, Vec<Diagnostic>>;
```

Both `ProjectCandidate` methods, and the free function, are bound at
`semaprax::project::{...}` (flattened, matching every other candidate
evidence module) and at `semaprax::project::candidate::{...}`.

### Opening a session

`open_coordination_session` binds a bounded set of agent participants
(`1..=16`), each granted an explicit, bounded (`1..=32`) scope of stable
declaration `@id`s. Every granted-scope id is independently checked against
this exact candidate's own compiler-owned impact artifact via
`ProjectCandidate::impact_summary` -- never trusted from the caller -- and
fails closed (`SPX-Z503`) if the id does not name a declaration this exact
candidate carries. Each granted id's impact-artifact view counts
(`affected`/`dependency_edges`/`frontier` totals) are attached to the
rendered session as `graph_signal`: **descriptive guidance only**, never
compared, thresholded, or used to decide compatibility.

`coordinator_id` must differ from every participant's `agent_id`
(`SPX-Z504` otherwise): a session cannot be governed by one of its own
working agents, the same "cannot approve yourself" shape
`candidate_assurance::grant_candidate_acceptance` uses for
`reviewer != proposer`.

### Evaluating proposals

`evaluate_agent_proposals` classifies a batch (`1..=64`) of typed
`AgentProposal` values against a previously opened session. It fails closed
(`SPX-Z503`) when the session's `candidate_revision` does not match
`self.candidate_digest()` -- a session opened before an intervening edit can
never be reused to evaluate proposals against the changed candidate.

Within a structurally valid session, each proposal is checked independently.
A failure is *recorded as a rejection* in the output rather than aborting the
whole batch, because one bad proposal must never hide the classification of
the others:

| Rejection reason      | Trigger                                                          |
|------------------------|-------------------------------------------------------------------|
| `unknown_agent`        | `agent_id` is not one of the session's participants               |
| `stale_base_revision`  | `declared_base_project_revision` != the session's bound base       |
| `scope_violation`      | a `target_ids` entry is outside that agent's `granted_scope`        |

Two surviving proposals from *different* agents that name the same target id
are a `same_target` conflict: both intentions, the affected ids, and a closed
set of `allowed_resolution_choices` (`prefer_first_proposal`,
`prefer_second_proposal`, `manual_reconciliation`, `abandon_both`) are
recorded, and neither is ever silently chosen. Two proposals from the *same*
agent sharing a target are that agent's own sequencing choice, not a
coordination conflict.

The remaining, non-conflicting proposals are returned as one deterministic
`compatible_order` (sorted by agent id, then proposal index) -- guidance for
a separate authorized rebase/merge invocation. `merge_authority`,
`publication_authority`, and `execution_authority` are always `false`.

### Scheduling and economics evidence

`record_scheduling_comparison` takes two caller-*observed*
`SchedulingObservation` values (from an actual sequential run and an actual
parallel run the caller performed) and computes `wall_clock_speedup`,
`cost_ratio`, `review_burden_delta`, and `retries_delta`. It runs nothing
itself; it is a pure function over bounded, caller-supplied counters.

## Diagnostics (`SPX-Z5xx`, previously unused)

- `SPX-Z501`: invalid input (empty/oversized/duplicate field, a coordinator
  identity equal to a participant's, zero participants/proposals).
- `SPX-Z502`: capacity exceeded (too many participants/proposals/ids, an
  out-of-bounds scheduling metric, or a rendered record over its byte
  budget).
- `SPX-Z503`: stale (a session bound to a different exact candidate, or a
  granted scope id that does not name a declaration this exact candidate
  carries).
- `SPX-Z504`: refused (the coordinator identity equals a participant's).

A per-proposal rejection (`unknown_agent`, `stale_base_revision`,
`scope_violation`) is expected batch content, recorded as data, not one of
the codes above.

## Failure cases this module names rather than silently mishandles

- **Graph independence can miss hidden coupling.** `graph_signal` is
  descriptive only. This module's *only* proven, automatically-computed
  conflict is exact stable-`@id` overlap (`same_target`); it does not claim
  to detect that two disjoint target ids are semantically coupled (for
  example, one calls the other). The unit test
  `disjoint_but_graph_dependent_targets_are_not_flagged_as_a_cross_target_conflict`
  demonstrates this honestly-declared gap using a real dependency
  (`coordination.main` calls `coordination.divide`): both proposals come back
  `compatible`, and the rendered record's `nonclaims` say so
  (`graph_independence_can_miss_hidden_external_or_generated_coupling`). A
  caller that needs real cross-target semantic coupling must independently
  run the existing impact/dependency analysis
  (`ProjectCandidate::impact_summary`/`impact_page`,
  `dependency_summary`/`dependency_page`) over the candidate proposals before
  trusting a "compatible" verdict for anything beyond stable-id disjointness.
- **A rebase can preserve syntax but change intention under intervening
  edits.** Handled at the session level: `evaluate_agent_proposals` fails
  closed the moment `self` is a different exact candidate than the session
  was opened against.
- **Agent scope/capability enforcement can be confused with mere task
  assignment.** `granted_scope` is enforced from the session's own record
  (independently validated at session-open time), never from what a
  proposal itself claims its scope or impact to be.
- **Parallel work can amplify cost without benefit.** `record_scheduling_comparison`
  is the bounded evidence surface for a caller to record and compare that,
  without this module ever running the comparison itself.
- **No agent/coordinator path publishes without a separate authorized
  session.** Every record this module renders carries
  `publication_authority: false`; the module calls no publication, rebase,
  or execution API.

## Known limitations

- Local evidence only: every test in this suite runs the fixture harness
  in-process; no hosted, multi-process, or genuinely concurrent agent run is
  exercised or claimed.
- `evaluate_agent_proposals` proves only *same-target* conflicts
  automatically; it does not attempt cross-target semantic-coupling
  detection (see above) -- a deliberate, tested, and declared limitation
  rather than an unverified heuristic.
- This module never builds a merged `ProjectCandidate`, never reruns
  assurance or requirement traceability over a merged result, and never
  selects or runs cross-target tests. "Full candidate rebuild/assurance/test
  selection" for an accepted `compatible_order` remains the responsibility of
  a separate, already-existing authorized invocation (the same rebase/build/
  test-selection/publication machinery every other `ProjectCandidate` route
  uses); adding that step here would be exactly the trap this issue names.
- `OperationClass` is a caller-declared label carried through into evidence;
  this module does not independently verify that a proposal's declared class
  matches what its target ids actually are.
- "Crash/recovery preserves proposals and does not adopt partial merges" is
  satisfied by construction and tested as determinism: this module holds no
  mutable state across calls, so two identical `evaluate_agent_proposals`
  calls render byte-identical output
  (`evaluation_is_deterministic_so_a_repeated_call_can_never_adopt_a_partial_merge`).
  There is no partial merge state for a crash between calls to ever leave
  behind, because no call ever merges anything.
