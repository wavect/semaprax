# Workspace report owner v1

Audience: compiler contributors and reviewers.

Status: source-authored additive v0.10 typed Rust facade API; executable
qualification pending. Dynamic legacy diagnostics and application funding remain
separate from the selected replay report protocol.

## Same-invocation ownership

`workspace_report::WorkspaceReportFailure` is sealed and non-Clone. Selected
replay still acquires its original caller-owned report before preparation on
that invocation's original physical ledger: one actual Diagnostic slot and the
exact fixed form's owned text. This API adds no allowance, factory budget,
allocation, Box, reset or refund. It moves the complete original refusal owner,
including initial setup failure or failed Preparation, first receipt and ledger.
An unacquired report stays inline through propagation and Drop.

`reported()` borrows existing backing. `has_paid_report()` certifies only the
original selected carrier for its exact form. `into_paid_report()` moves that
Vec/String, preserving pointer and capacity; it otherwise returns the same
affine failure without releasing the retained owner. An ordinary existing Vec
moves unchanged but does not become certified paid storage. Mismatched forms
cannot relabel an existing paid report. No public operation exposes mutable HIR,
ledger access, a replay index or authority to restore a failed invocation.

Display streams the existing diagnostics or exact fixed refusal form to a
caller-owned formatter. It allocates no intermediate message/report. Output
backing is the renderer's obligation; calling Display does not fund output.
`into_legacy_unfunded()` explicitly materializes the old outward report when
necessary. There is no allocating Deref or implicit typed-failure-to-Vec/String
conversion. The legacy Vec APIs and all their original bodies/owners remain.

## Actual typed source receiver

`SemanticWorkspaceSource::new(String, String)` moves caller-owned input storage.
`semantic_workspace::reporting::preflight` authenticates the complete canonical
path set and owned sources, then returns an opaque `CheckedPreflight`. Its
`manifest()` and `workspace_revision()` borrow checked stored bytes; `replay()`
independently rechecks every held source, schema, revision, digest and manifest.
Both full schema-receiving passes propagate the affine selected-replay failure.
The original complete Vec preflight and independent replay remain an oracle.
Canonical ordering, all original source/storage limits and diagnostic priority
are unchanged. This source-only API grants no filesystem, lock, transaction,
staging, evidence, managed generation or ACTIVE authority. The original initializer, CLI and Vec-returning filesystem APIs
retain their compatibility behavior; the additive typed filesystem routes below
preserve their original authority and execution order.

## Explicit residual frontiers

Ordinary grammar, source resolution, fingerprints, directory backing, dynamic
limit/help reports and output still use the existing allocating legacy boundary.
In particular `source_schema::all` first charges its original directory before
replay. Its failure preserves the exact dynamic maximum and full TLS module,
pending/resolving/retained attribution; no max-only substitute or post-refusal
clone claims funding. Migrating that seam needs a separately acquired attribution
owner that preserves nested/repeated invocation behavior.

The original Vec-returning filesystem view wrappers retain their allocating
compatibility boundary. The v0.10 `into_change_view_reporting` and
`into_operation_view_reporting` routes instead move the selected failure through
the authority-held preparer to the public typed outcome. They preserve the full
original lock, evidence replay, source binding and ACTIVE order; replay remains
before staging. Their owning controls await the consolidated qualification.

## Source-authored owners

All previous paid carrier, Preparation/Session, workspace preflight and filesystem
oracles remain complete. New library selectors:

- `workspace_report::tests::initial_zero_exact_short_sticky_typed_transfer_display_and_drop_allocate_nothing`
- `workspace_report::tests::exact_paid_setup_refusal_transfers_original_slot_text_and_ledger_through_drop`
- `workspace_report::tests::constructor_receipt_and_same_ledger_survive_typed_owner_and_no_paid_form_remap`
- `workspace_report::tests::explicit_legacy_materialization_preserves_full_owned_report_and_original_pass_through`
- `workspace_report::tests::actual_schema_receiving_caller_keeps_zero_short_exact_paid_owner_until_final_drop`
- `workspace_report::tests::actual_source_drift_keeps_full_prior_literal_and_whole_receiving_debit_drop`
- `workspace_report::tests::prepaid_first_budget_receipt_moves_with_affine_typed_owner_without_reformatting`
- `semantic_workspace::reporting::tests::complete_original_schema_corpus_matches_legacy_manifest_revision_and_independent_replay`
- `semantic_workspace::reporting::tests::full_path_frontier_and_late_source_errors_keep_original_diagnostics_and_order`
- `semantic_workspace::reporting::tests::typed_independent_replay_retains_manifest_source_revision_and_digest_priority`

The controls observe actual construction, receiving, streaming and final Drop;
zero, exact and one-short acquisition; original report pointer/capacity and
sticky first ledger refusal; full old literal, schema corpus and hostile error
parity. They are authored only. No compile, runtime, efficiency, end-to-end
funding or issue-closure result is claimed.


## Affine filesystem caller routes (OPT781)

`semantic_workspace_change::reporting`,
`semantic_workspace_structural_change::reporting`, and
`semantic_workspace_operations::reporting` expose authority-held typed generation,
verification and application. Operations also exposes typed derivation. They
retain the original locks, source acquisition, complete preparation, exact replay,
and commit order. Candidate preflight and graph-view setup pass the same selected
report owner through `WorkspaceReportFailure`, without converting an inline
refusal into a new diagnostic vector. Initial report acquisition remains before
selected replay preparation, under the existing invocation ledger.

`finish_reporting` moves an initial failure through the original unlock call.
If unlock fails, the original legacy SPX-I210 report has the same outward priority
as `finish`; the primary report and first receipt remain privately retained until
Drop. Unlock reports are ordinary existing diagnostics, not selected-funded
reports. No error mapper may consume a selected owner to rebuild an ordinary
report. Legacy graph-view failures preserve their complete existing diagnostics
on the typed route instead of relabeling them with the selected fixed form.

The original Vec-returning APIs and CLI callers remain explicit compatibility
routes. Dynamic proposal, filesystem, authentication, rendering and commit errors
remain ordinary legacy reports. This addition does not establish funding for
those errors, activate new source profiles, or establish complete OPT781 closure.

### Selected-family receiving candidate (#781)

The source-only selected schema/inventory candidate is specified in
`SELECTED-REPLAY-RECOVERING-CONSTRUCTOR-V1.md`. It changes the child receipt
transport and receiving funding on the existing paid replay path, while keeping
the original acquired report carrier and affine ledger. A real selected
inventory mismatch is observed in the existing selected-entry paid-report
harness: the original Diagnostic slot/text pointer and shared source identity
survive publication, a later receipt cannot replace the first, and full owner
Drop releases report, scratch and ledger. Detailed legacy diagnostics are
compared outside the paid no-allocation cut with complete independent original
owners. Runtime qualification is still pending; neither this candidate nor its
typed finite receipt grants source, replay or report authority. Unmigrated
internal Session semantic Diagnostic descendants remain explicit unpaid
production residuals. Named external compatibility exclusions are unchanged.


### Selected structural-control child candidate (#781, unqualified)

The subsequent selected structural-control source batch keeps the same acquired
report carrier and affine ledger through the actual paid Session unknown-entry
refusal. Semantic details stay in a typed finite/shared-ID receipt; publishing
that receipt must retain the original Diagnostic slot, fixed text backing and
source identity pointer without allocating. An already failed Session refuses a
second validation, and a separately prepaid later receipt cannot replace the
first. Full legacy diagnostics are observed outside this paid publication cut
and compared against complete immediate original owners, including all six
fields and first-refusal order. The owner selector is
`selected_entry::tests::paid::selected_control`.

These controls and their structural-family controls are source-only and unrun.
They do not grant AuthSourceBorrow, source authentication, replay permission or
new report authority; existing authority controls remain unchanged. Remaining
lexical-HIR/certificate, path-state/transition and factored semantic descendants
retain their explicit unpaid production status. No claim of complete #781
closure, current-head executable support or measured improvement is made.

The isolated #781 selected path-state candidate extends the same preacquired
report/ledger route through path transitions and renewal provenance. A finite
nested control reason replaces semantic owned-Diagnostic transport for that
selected family; genuine budget diagnostics retain their existing prepaid
provenance. No new report or payer is supplied, and later refusals retain the
first receipt and original report slot/text pointers. Complete immediate old
owners and ordinary APIs remain independent. Owning source-only controls cover
live duplicate Initialize, current paid Session recovery, full six-field legacy
observation and Drop. This candidate remains unqualified and does not establish
whole-engine funding, net cost benefit or closure of #781.
