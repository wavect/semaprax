# Typed-draft semantic rebase v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: agent builders, embedding hosts and compiler contributors.

An unfinished draft can move its valid history and pending typed holes onto an
independently admitted source revision without completing those holes first.
The compiler replays the checked history, rejects incompatible changes to
pending regions, authenticates new expression identities and regenerates hole
contexts. It never creates placeholder source or treats the prior valid body as
the unfinished draft's intended replacement.

## Library and history

`ProjectCandidateDraft::rebase(expected_draft, new_base, expected_new_base)`
accepts an `Arc<ProjectRevision>` and returns `ProjectCandidateDraftRebase`.
Both expectations bind exact identities; canonical old/new Project manifests
must match. The result exposes `draft()`, `into_draft()` and `to_json()` for its
separate report. It exposes no candidate or source accessor.

The existing [candidate rebase](PROJECT-CANDIDATE-REBASE-V1.md) first replays the
private last-valid candidate's complete intention history on the selected new
base. This includes earlier successful fills. Its original conflict rules,
intermediate source reconstruction and independent Project admission remain
unchanged. A history conflict rejects the entire draft rebase before any result
is installed. The new draft's eventual source diff is based on the selected
new revision, not the abandoned original revision.

Every successful rebase appends one bounded ancestry row naming the exact
parent draft and destination Project revision. Filled-hole events survive with
their checked history ordinals and intention bindings. The resulting draft and
report use additive v2 schemas; lineage is metadata over replayed valid history,
not unfinished meaning, approval, or authority.

Pending selections are compared between the original last-valid revision and
the fully rebased last-valid revision, after every checked history step. A hole
in a declaration introduced by that history is therefore selected against real
rebuilt source, not assumed to exist in the original base.

## Pending region conflicts and remapping

The compiler shares candidate rebase's source-derived function fingerprints,
including stable-ID call normalization, authenticated nominal/member occurrence
normalization and checked nominal signature facts. Function, callee and proven
nominal/member display renames do not by themselves create region conflicts.

| Pending kind | Concurrent owner changes that reject |
| --- | --- |
| Function body | Signature, body or effects/module permits. |
| Body expression | Signature, body or effects/module permits. |
| Contract expression | Signature, contracts or effects/module permits. |

An independently changed contract may coexist with a body or body-expression
hole, while an independently changed body may coexist with a contract hole.
The resulting context describes the new checked revision. It does not preserve
stale contextual facts or prove that an eventual fill satisfies newly changed
contracts. Filling still performs ordinary complete source and target admission.

These are conservative region checks: disjoint edits within one function body
or contract inventory are not automatically merged. Missing targets or lost
explicit identity reject. Selection remapping alone is insufficient to establish
that the old region survived; conflict checks run before remapping.

Calls in a selected contract predicate retain the existing authenticated callee
signature, effect and contract guards. Protected regions and signatures also
contribute their actual checked nominal type identities, including inferred
intermediates. Their existing owner descriptors must agree, including ordered
member identities and types. This does not introduce a transitive nominal-type
equivalence proof or silently broaden constructor admission.
Known owner, case and field display names are excluded from these descriptor
comparisons; IDs, ancestry, types, order, generic parameter names and provenance
remain exact. The shared guard also applies during draft merge. A same-type
sibling-field substitution is a semantic change, not a display rename.

For expression and contract holes, existing source-join helpers derive the old
AST position from the actual HIR identity and locate the new unique HIR origin.
Resolved type and ownership must agree. Contract phase and predicate position
remain source-derived. Callers supply no spans, AST paths, replacement strings
or guessed identities. All pending holes are reconstructed through ordinary
`with_body_hole`, `with_expression_hole` or `with_contract_expression_hole`,
retaining the shared sixteen-hole, global-ID and overlap rules.

Selected lexical scope must retain its binding names, types, ownership modes
and mutability; revision-scoped binding IDs are not assumed stable. Full contexts
for every final pending hole are generated under the ordinary context bounds
before rebase succeeds. Old scope, contracts, available-call and prior cleanup
facts are never copied into a new revision as evidence.

Only the resulting draft is returned. Each hole remains unresolved, with the
same hole ID and newly bound context. An empty or fully filled draft remains
ready to complete; `complete` is still the only release to a valid candidate.
Neither parent draft, source base, sibling candidate nor filesystem is mutated.

The additive [Draft Merge](PROJECT-CANDIDATE-DRAFT-MERGE-V1.md) shares these
pending-selection guards while combining two checked histories with one common
original base. It separately checks opposing history writes and reconciles the
pending union; these rebase APIs, report bytes and destination-base semantics
remain unchanged.

## V5 protocol and restart workflow

Candidate-enabled v5 sessions add `hole/rebase` with exact `image_revision`,
`draft_revision` and `new_base_candidate_revision` parameters. The selected
retained candidate supplies its checked source revision as the destination
base. The method cannot open a path, select a new manifest or widen authority.

Its response contains the selected candidate identity, a normal draft handle
and the source-bound rebase report. Registry admission and bounded response
preparation precede installation, and ordinary live-source checks surround the
operation. Only the resulting draft is retained; its internal last-valid
candidate is not inserted into the candidate registry. Failure installs nothing.
Existing identical draft entries retain their prior session association.

The rebase report is inline and limited by the existing 64 KiB reconciliation
response bound; larger library reports are not silently truncated. The ordinary
frame and final response limits also apply. This mutation is excluded from
parallel read batches. V1–v4 method sets remain unchanged; v5 discovery and
generated clients advertise the method only under the host candidate grant.

For manual source edits, persist the draft explicitly through
[draft archives](PROJECT-CANDIDATE-DRAFT-ARCHIVE-V1.md) or
[durable draft storage](DRAFT-ARCHIVE-PERSISTENCE-V1.md), open a new session on
the changed source and recover the historical draft at startup. Open a current
candidate, then select it as the rebase destination. `workspace/refresh` still
clears drafts; this change does not silently preserve them across refresh or
relax historical archive startup admission.

After rebasing, query the new contexts, fill the remaining holes and explicitly
complete. Review the resulting canonical source diff before invoking any
separate publication authority. Rebase reports, draft handles and stored
archives carry no approval or commit authority.

## Evidence and limits

The canonical LF-terminated lineage report uses
`semaprax.project-candidate-draft-rebase.v2`; the v1 schema remains a published
compatibility identity for prior artifacts. It binds `parent_draft_digest`,
`original_base_revision`, `onto_revision`, `result_base_revision` and
`result_draft_digest`, plus the existing checked-history report nested under
`last_valid_rebase`. Pending rows bind hole ID, kind, stable target, old/new
expression identities (null for body holes), concurrent body/contract change
flags and regenerated-context status. `filled_hole_lineage` and
`branch_ancestry` bind the preserved event and branch history.
`materializable` and `source_authority`
remain false; validation describes history replay and pending-selector
readmission, not validation of an unfilled replacement.

The v5 wrapper schema is `semaprax.image-draft-rebase.v1`, with
`selected_candidate_revision`, `draft` and `report`. It binds the selected
candidate separately from the report's destination Project revision. Nested
history facts describe the last valid candidate, not the incomplete draft.

`tests/project_candidate/draft_rebase.rs` and
`tests/image_transport_v5/draft_rebase.rs` author mixed pending kinds, partial
history, context/identity remapping, compatible and conflicting source changes,
historical recovery, stale rejection and authority preservation. Tests, compiler
checks and long local gates were not run. No completion-matrix row is promoted.

The existing Project, candidate-history, source-traversal and expression limits
remain in force. The library report is bounded to 1 MiB; protected HIR/type
traversal shares a 1,048,576-visit bound and depth 256. Context generation retains
the ordinary per-context bounds and at most sixteen pending holes. These bounds
do not claim a total process-memory or wall-clock limit.

`SPX-G343` owns malformed internal rebase-report data, `SPX-G344` new traversal
and report capacity, and `SPX-G345` pending-region, dependency or scope conflict.
Existing draft selectors retain `SPX-G232`; candidate rebase's new-base selector
and history conflict diagnostics retain `SPX-G235`, and its manifest failures
retain `SPX-G233`. Source admission, fingerprint and expression-origin failures
propagate their owning diagnostics. The inline transport report cap remains
`SPX-G234`.

Rebase is explicit
reconstruction, not an incremental-performance claim, logical contract proof,
runtime execution, arbitrary semantic merge or complete session recovery.
