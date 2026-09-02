# Typed-draft semantic merge v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: agent builders, embedding hosts and compiler contributors.

Two unfinished sibling drafts can combine checked intention histories and
compatible pending holes without releasing a completed candidate. The result
retains the common original source base, so an eventual canonical source diff
includes both histories. No placeholder source, unchecked HIR or inferred
hole-completion state is introduced.

## API and checked history

`ProjectCandidateDraft::merge(expected_draft, other, expected_other)` takes
another borrowed draft and returns `ProjectCandidateDraftMerge`. Exact parent
draft identities are required. The result exposes `draft()`, `into_draft()` and
`to_json()` for a separate merge report; it has no candidate or source accessor.

The private last-valid candidates first use the existing
[candidate history merge](PROJECT-CANDIDATE-REBASE-V1.md). Both must share their
original Project base and canonical manifest. Their exact common history prefix
is replayed once; the right suffix precedes the left suffix. Existing history
conflicts, identity/dependency checks and complete canonical source replay remain
unchanged. The combined history must fit the existing 32-intention limit.

The merged checked candidate is used only as the internal source revision for
pending-hole reconciliation. Each parent's selections are independently rebound
onto it through the same private machinery as
[draft rebase](PROJECT-CANDIDATE-DRAFT-REBASE-V1.md). History is not reapplied onto
the already merged source, and neither parent's history is discarded by making
its final source an unrelated new base.

## Opposing writes and pending selections

Final source equality alone cannot show that a pending intention is untouched.
For each parent's pending holes, merge also examines the other parent's actual
history after the exact common prefix. A fill that produces the old body, or an
edit followed by restoration, is still an opposing write.

| Pending region | Opposing suffix intentions that conflict |
| --- | --- |
| Body or body expression | Body replacement, expression replacement, extraction or signature change on that owner. |
| Contract expression | Contract replacement, contract addition or signature change on that owner. |

This is conservative: it does not infer that a fill on one branch fulfills a
hole on another. Draft archives contain pending selectors and checked history,
not a tombstone or shared filled-hole event log. Merge never removes or completes
a pending hole based on the absence of a corresponding hole in the other draft.
Any future completion-lineage semantics require a separate explicit contract.

After opposing-write checks, each parent still passes all existing draft rebase
guards against the merged last-valid revision. Protected source regions,
signatures, effects, contract callees, checked nominal owner descriptors and
selected lexical scope must remain compatible. Expression identities are
rejoined through authenticated canonical source origins and checked type and
ownership. These joins are compiler-derived; callers supply no AST paths or
spans. An unrelated contract change can coexist with a body hole, and an
unrelated body change can coexist with a contract hole, subject to these checks.

## Union, overlap and completion

The pending union covers body, body-expression and contract-expression holes.
An identical hole ID coalesces only when kind, stable target and newly mapped
expression identity also agree. For body holes, the expression identity is null.
The report records both contributing parents rather than losing one ancestry.
An identical ID naming different selections conflicts.

Distinct IDs do not authorize overlapping selections. The union is reconstructed
through ordinary hole APIs, preserving unique body targets, expression subtree
overlap checks, contract phase/predicate boundaries and the shared sixteen-hole
limit. A body and contract hole may coexist on the same owner; a body and body
expression hole may not. Coalescing does not evade the union capacity bound.

Every final hole context is generated against the final merged draft before
success. Context bounds, current scope/call facts, contracts and prior
loan/cleanup facts retain their ordinary owners. No parent context is imported
as proof for another revision.

The result remains incomplete while any hole is pending. Empty and ready drafts
can merge, but even a hole-free result requires an explicit `complete` call.
That completed candidate still requires separate review and publication
authority. Both input drafts, source files and session approvals remain unchanged.

## V5 transport

Candidate-enabled sessions add `hole/merge` with exact `image_revision`,
`draft_revision` and `other_draft_revision`. Both drafts must already be retained.
The closed `semaprax.image-draft-merge.v1` response binds `left_draft_revision`,
`right_draft_revision`, the ordinary `draft` handle and the merge `report`.
Only the final draft is retained; the internal merged candidate is not installed
in the candidate registry. Existing identical drafts keep their prior session
association.

The request performs ordinary before/after live-source authentication. Registry
admission and bounded response preparation precede installation. The inline
report cap is 64 KiB, followed by the existing final response bound; failures
install no result rather than truncating its report. This is a mutation and is
excluded from parallel image reads. V1–v4 methods and existing rebase report
schemas remain unchanged. V5 discovery and generated clients expose this method
only under the host-selected candidate grant.

Recovered historical drafts may merge if they retain the same original base.
That does not make historical source current. Explicitly rebase the resulting
draft onto an admitted current revision before current-source publication, or
complete and rebase through the existing candidate route. Startup archive
admission, refresh clearing of drafts and independent Git approvals are unchanged.

## Report and evidence

The canonical library report uses `semaprax.project-candidate-draft-merge.v1`.
It binds `left_parent_draft_digest`, `right_parent_draft_digest`,
`original_base_revision`, `result_base_revision` and `result_draft_digest`.
`last_valid_merge` contains the existing checked-history
merge report. `left_holes` and `right_holes` preserve both parents' old/new
expression mappings and concurrent-region classifications. Final `holes` rows
bind the merged selection and its contributing parents. Materialization and
source authority remain false.

Final rows contain `hole_id`, `kind`, `target`, `expression_id`, `parents` and
`context_refreshed:true`. Parent arrays are exactly `["left"]`, `["right"]` or
`["left","right"]`; final rows and each parent's mappings use hole-ID order.
The root `validation` is `checked_history_merge_and_pending_selector_readmission`,
and `nonclaims` identifies unproved behavior, lineage and authority. Per-parent
mapping rows retain the eight-field shape described by draft rebase. None of
these fields is a proof that an unfilled replacement would typecheck or execute.

The library report is bounded to 1 MiB. Each parent uses the existing bounded
pending-selector traversal; merge performs at most two such traversals, with
the existing final per-context bounds and sixteen-hole cap. These are not total
heap, latency, incremental-reuse or productivity claims.

`SPX-G346` owns malformed internal history/report data; `SPX-G347` owns the
pending-union and merge-report bounds; `SPX-G348` owns conflicting hole-ID
selections and opposing protected writes. Existing draft selector, overlap and
context diagnostics propagate, as do candidate history/replay conflicts and
draft rebase's region/dependency diagnostics. The inline transport cap remains
`SPX-G234`. Failure changes neither parent and grants no retry or commit authority.

`tests/project_candidate_draft_merge_v1.rs` and
`tests/image_transport_v5/draft_merge.rs` author compatible checked histories,
mixed holes, coalescing, conflicting selections, opposing writes, recovery and
authority-preserving protocol behavior. Tests, compiler checks and long local
gates were not run. No completion-matrix row is promoted.

General semantic compatibility, filled-hole lineage, arbitrary disjoint edits
inside one protected region, cross-manifest merging, runtime verification and
measured multi-agent performance remain open.
