# Owned Workflow Approval v1

Status: implemented bounded profile; local evidence only (see below). Extends
[Universal Semantic Transaction v2 Workflow](UNIVERSAL-SEMANTIC-TRANSACTION-V2-WORKFLOW.md)
by composition; neither that module nor the frozen v2 transaction kernel is
modified.

Audience: compiler contributors, agent-tool authors, and reviewers of
concurrent multi-file semantic change and managed-Workspace publication.

Owned Workflow Approval v1 answers two questions a Universal Semantic
Transaction v2 Workflow leaves open when the underlying source moves between
review and commit, or when a candidate is prepared but never independently
signed off: is a workflow authored against one base still safe to apply once
the live source has drifted, and can a candidate be published without a
distinct approval act naming its exact digest.

## Bounded scope and non-goal

This module implements a conservative gate over the *selected owned-data
change plan* a v2 workflow already describes, not a general concurrent
version-control merge. Automatic reselection is granted only for a drift that
never touches a workflow's targeted declarations; any signature, ownership
mode, contract, or body change on a targeted declaration is an explicit
conflict, refused rather than guessed at.

## Conservative owned-data staleness

`require_owned_targets_unchanged(original_base, current_base, transactions)`
compares every step's target declaration between the revision the
transactions were authored against and the current live revision, reusing
`ProjectCandidate::rebase`'s own target-scoped conflict facts
(`pending_draft_conflicts`) unmodified. It returns `Ok(())` only when each
target's signature (including every parameter's ownership mode), effects, and
body are byte-identical across the two revisions -- the drift, if any, never
touched a declaration this workflow depends on.

`reselect_owned_workflow(original_base, current_base, transactions)` calls
that gate first. A Universal Semantic Transaction v2 envelope binds its own
precondition to one exact global workspace revision, so the original
transaction bytes are stale the instant *anything* in the workspace moves,
even a fully disjoint sibling edit. Reselection therefore rebuilds one fresh
transaction per step, carrying over its exact target, expression identity, and
expected old-source text unchanged, and binding only the wrapper to the
current workspace revision, before handing the refreshed steps to the frozen
`SemanticTransactionV2Workflow::derive` core -- which still independently
re-checks that each carried-over expression identity resolves and its exact
old-source text still matches, and still fully recompiles the resulting
program. A same-declaration edit that the staleness gate somehow missed is
still rejected there; an edit that broke an actual safety property the
workflow depended on is still caught by ordinary compile-time admission,
never a backend accident.

## One publishable, whole-history candidate

`SemanticTransactionV2Workflow::candidate()` returns only its terminal step's
own local `ProjectCandidate`, whose `base` is the *intermediate* revision step
N-1 produced -- correct for that step's own review artifacts, but not directly
publishable through the existing managed-Workspace commit boundary
(`prepare_candidate_publication`/`apply_candidate_publication`), which
requires a candidate whose `base` equals the exact currently held Project
revision.

`OwnedWorkflowCandidate::derive(base, transactions)` independently replays
each already-validated step's `replace_expression` intention (reconstructed
from its public target/expression-identity/replacement fields, the exact
intent shape `SemanticTransactionV2::validate` itself constructs) onto one
running candidate rooted at `base`, and cross-checks the result's final
revision against the frozen workflow core's own before trusting it. A
divergent reconstruction is rejected, never silently published.

## Publication requires a separately approved exact digest

`OwnedWorkflowApproval::approve(&owned_workflow_candidate)` is a distinct act
from creating the candidate: it captures the workflow's and the whole-history
candidate's exact digests as an authority-free evidence value (`"approval_authority":
false`, matching every other evidence artifact in this codebase). It is never
implied by `derive`/`reselect_owned_workflow` succeeding, and
`OwnedWorkflowApproval::replay` requires exact canonical bytes and digest so
the evidence cannot be tampered with in transit across a process or session
boundary.

`prepare_approved_owned_workflow_publication` and
`apply_approved_owned_workflow_publication` both require this evidence and
refuse before any workspace read or write when it does not name the exact
candidate presented -- approving one candidate never authorizes publishing a
different one, and a workflow re-derived after staleness (even over the
identical steps) carries a new digest that no existing approval names. Both
functions delegate the actual host replay, lock/authority acquisition, and
`ACTIVE` pivot unchanged to the existing
`super::publication::prepare_candidate_publication`/`apply_candidate_publication`
route: this module adds no filesystem, lock, or commit authority of its own,
only the additional requirement that a distinct approval evidence value name
the exact digest being published.

## Implementation and tests

The implementation is owned by
`src/project/candidate/owned_workflow_approval.rs` and exports:

```rust
pub struct OwnedWorkflowCandidate { /* opaque */ }
pub struct OwnedWorkflowApproval { /* opaque */ }

pub const OWNED_WORKFLOW_APPROVAL_SCHEMA: &str = "semaprax.owned-workflow-approval.v1";
pub const MAX_OWNED_WORKFLOW_APPROVAL_BYTES: usize = 65_536;

pub fn require_owned_targets_unchanged(...) -> Result<(), Vec<Diagnostic>>;
pub fn reselect_owned_workflow(...) -> Result<OwnedWorkflowCandidate, Vec<Diagnostic>>;
pub fn prepare_approved_owned_workflow_publication(...) -> Result<ProjectCandidatePublication, Vec<Diagnostic>>;
pub fn apply_approved_owned_workflow_publication(...) -> Result<String, Vec<Diagnostic>>;
```

Regressions live as the `owned_workflow_approval` module of the
`project_candidate` integration harness
(`tests/project_candidate/owned_workflow_approval.rs`), covering: a disjoint
sibling edit surviving the staleness gate, reselection, and needing a fresh
approval; a same-declaration signature/ownership-mode conflict refused by the
gate even though a naive reselection that only refreshes the workspace
wrapper would silently admit it; a same-declaration contract conflict refused
the same way; approving one candidate never authorizing publication of a
different one, including through the real managed-Workspace `prepare`/`apply`
route with the original raw source files verified byte-identical afterward;
and exact canonical-bytes/digest replay of the approval evidence itself.

## Nonclaims

- Not a general concurrent version-control merge: only a drift that never
  touches a workflow's own targeted declarations is treated as safe to
  reselect automatically.
- Not itself publication authority: an `OwnedWorkflowApproval` is evidence: it
  presents no lock, no Workspace authority, and no capability. The live
  invocation calling `apply_approved_owned_workflow_publication` still
  performs the ordinary managed-Workspace replay and `ACTIVE` pivot.
- Does not cover transitive callee drift beyond a targeted declaration's own
  signature, ownership modes, effects, contracts, and body; wider transitive
  conflict classification remains `ProjectCandidate::rebase`'s existing,
  separately gated surface.
