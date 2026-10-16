# Selected replay recovering constructor v1

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

This internal ownership adapter changes no source admission, graph family,
physical/work ceiling, or authority proof. Source qualification is pending.

`Session::try_from_preparation` borrows the exact existing preparation budget
while reserving the Session header and constructing its one borrowed callee
index, in the existing order. Success moves that budget into Session. Failure
returns the same failed Preparation and the first owned Diagnostic; temporary
index storage has dropped before return. The caller can observe and drop both
without rebuilding the index or restarting either allowance.

The existing `from_preparation` owned-Diagnostic wrapper preserves its prior
already-failed error path. Otherwise it delegates and drops the recovered owner
on refusal. Callee-index structural failures use the selected budget's already
paid refusal String with the same SPX-H006 code and first-report text.

The new test-only zero probe observes sticky state on the same owner. It grants
no authority or credit and does not claim that the unchanged owned-Diagnostic
retry APIs allocate nothing. First-report and recovered-owner Drop remain in
allocation observations; fixed two-member work and exact carrier requests are
independent expectations, rather than receipts reused as formulas.

This does not establish funding for an outward `Vec<Diagnostic>` whose first
carrier reservation fails. The legacy initial Preparation construction error
channel remains owned and unfunded by an outer report carrier. No ordinary scalar lexical authority proposal is applied here.

Owning lib selectors under `cleanup_plan::replay::selected_entry::tests::recovering`:

- `recovering_constructor_exact_work_backing_and_first_report_drop_keep_one_owner`
- `recovering_constructor_structural_refusal_keeps_paid_first_diagnostic_and_temporary_drop`

## Inline initial setup

`Preparation::try_new` now returns either a preparation or an affine failure
containing the exact failed `PhysicalReservation` and a Copy setup reason. It
allocates no Diagnostic, report Vec, or failure text on a refused reservation.
Successful setup preserves the existing reservation order: ledger Box, clone
cursor, then one bounded refusal String. Construction occurs only after those
requests succeed. A later first refusal moves that actual paid String into its
Diagnostic; it does not fund an outward report Vec.

The compatibility `Preparation::new` (now test-only) and
`ReplayBudget::from_reservation` wrappers still convert the inline reason into
an allocating owned Diagnostic.
They are not zero-allocation or caller-funded report boundaries. This neutral
constructor adds no ordinary lexical admission, G410 bypass, new ceiling or
allowance. The caller-owned report that it does not fund is described below.

Additional owning lib selectors under
`cleanup_plan::replay::selected_entry::tests::setup`:

- `inline_setup_zero_and_each_one_short_return_the_exact_sticky_ledger_without_allocating`
- `inline_setup_exact_preparation_preserves_first_prepaid_text_pointer_receipt_and_drop`

These controls observe setup and refusal through Drop, with independent literal
carrier requests, zero/one-short setup, first receipt and same text/ledger
pointers. They do not themselves fund an outer report owner.

## Caller-owned paid report (#781, authored, not yet executed)

Status: source-only. Neither the build, Clippy nor the selectors below have
run against this protocol; no allocation fit, performance or benchmark claim
follows from it.

`selected_entry/paid.rs` owns the crate-private protocol types. `ReportForm`
is one fixed outward code and literal message chosen by the actual caller.
`Acquired` is the affine carrier-and-ledger stage: `try_acquire` debits exactly one
`Diagnostic` slot and the form's literal bytes on the caller's selected ledger
before allocating a capacity-one Vec and an exact-capacity String. No function
id or formatted detail is published, so no further text is needed. A refused
debit returns the same rejected ledger and a Copy `SetupReason`
(`ReportCarrier` or `ReportText`) without allocating. `ReportCapacity` is a
post-allocation invariant refusal: the requested layouts were paid, both
buffers are dropped, and their debit stays spent. It is not an allocation-free
initial refusal and does not authorize an unexpected allocator capacity.

`PaidRefusal` is sealed; only `paid.rs` constructs it, so no caller can
present an unpaid or foreign Vec as paid. Its unpaid form retains the failed
ledger and reason. Its paid form retains the published carrier, a Copy
`Refused`, and exactly one ledger owner: the `SetupFailure` from
`Acquired::prepare` (`Preparation::try_new` on the ledger that paid the
carrier), the rejected bare ledger, or the failed `Preparation` recovered from
authentication, a constructor, `Session::try_from_preparation` or a Session
refusal (`Session::into_failed_preparation`, `Session::finish_paid`).
Publication writes into the acquired backing without reallocation. Transfer
marks the preparation failed and rejects its ledger without replacing a first
receipt. Neither form restores work, physical allowance or admission.

### Affine stage pairing

Acquisition returns one `Acquired`, never a `(report, ledger)` pair. Its
`prepare(function)` consumes both and returns the existing `Preparation`
with the carrier held internally. Authentication borrows that combined
preparation; it cannot take the report. `Preparation::into_paid_session`
consumes the entire stage and transfers its carrier with the cumulative
budget into the existing `Session`. Constructor refusal retains that same
combined preparation. `refuse`, `refuse_session`, `refuse_unprepared`, and
`finish_paid` accept no external report, preparation or physical ledger.
Moving a whole stage preserves pairing; no identity token, extra allocation,
new allowance, or report-to-ledger pointer lookup is introduced.

The neutral raw setup constructor is private to `selected_entry`. Only legacy
test constructors produce a preparation/session without a report. Production
cannot construct a detached carrier or split a paid stage; the report's fields
and publication method remain private. The actual local and imported callers
use these consuming transitions, rather than reconstructing a pair at finish.

### Inline refusal on the qualified route

`Refused` is a Copy, heap-free value: a `Cause` (`Mismatch`, `Budget`,
`Constructor`, `Failed`, `Setup`, `Session`), a static detail literal, and the
first static boundary stage. It has no `From` conversion into a `Diagnostic`
or `Vec`. Only the test-only `into_legacy_diagnostic`, and the test-only
`static_text::primitive::legacy` (G410) helper, render it.

The held `Preparation` refuses inline through `try_charge`,
`try_reserve_array`, `try_reserve_vec` and `construct`. A budget refusal from
`ReplayBudget` is the first paid Diagnostic: `budget_refusal` moves the
prepaid refusal text exactly once. The preparation retains it by move as its
`receipt`. A later refusal returns `Refused::FAILED`, or an empty, unallocated
String from a spent budget that is dropped, and never replaces that receipt.
`construct` lends a Copy-refusing debit. Work exhaustion spends the same
prepaid receipt and fails closed, even if a constructor ignores its refused
debit. The selected declaration constructors (`primitive_replay_metered`,
`append_flat_record_replay_metered`, `append_vec_replay_metered`,
`append_scalar_helper_replay_metered`, `helper_signature::admitted`, and the
test-only Option factory) now refuse with `Refused::constructor(literal)`
instead of allocating a `"... reconstruction refused"` Diagnostic.

Every authenticator on the route returns `Result<_, Refused>`:
`primitive::{members, current_module}`, `helpers::{authenticate,
authenticate_target, fact, calls, same_type}`, `helpers::local`,
`helpers::graph`, `helpers::types`, and `records::prepared`. None constructs a
Diagnostic, Vec or String before returning, and the caller transfers the
carrier with `Preparation::refuse(refused)`. The closure boundary
records its stage with `Refused::at` (one static pointer) instead of
formatting test help.

The test-only legacy owners `Preparation::{new, charge, reserve_array,
reserve_vec}` and `Session::{new, from_preparation, finish}` still allocate
`replay_error` on refusal or retry. Production no longer reaches them.

### Session refusal and fixed literal remapping

`Session::try_from_preparation` retains its original Diagnostic contract.
The actual paid callers use `validate_paid` and `validate_scalar_paid`.
Their shared expression-fact producer now has the private typed protocol below;
unmigrated child failures still return the Session's own Diagnostic.
`Preparation::refuse_engine` retains exactly one first receipt, moving an owned
Diagnostic unchanged or retaining typed fact data. Neither is formatted or
published as the outward report. The route stops at the first member refusal.
The paid siblings refuse an already failed Session inline; they never create
the legacy `already failed` retry Diagnostic.

The outward report is a fixed literal remapping. The public `SPX-G410` form,
or the remapping caller's literal, is the carrier's prepaid exact owned text.
The Session text pointer is not transferred into that report. It stays with
the failed ledger owner and is dropped with it when the refusal is published.
`finish_paid` on a failed Session refuses inline with `Refused::FAILED` and
allocates nothing. On success it drops the unused carrier and returns the same
boxed ledger, with no refund or reset.

### Actual caller

The actual caller is `workspace_graph/source_schema/static_text/primitive.rs`
`replay_paid` and its imported `helpers::replay` twin. It creates the module's
ledger, acquires the carrier, then prepares. It transfers the carrier and the
exact failed owner on every later refusal. This contract is on the actual
production route, and no test-only envelope stands in for it.

Typed twins carry the refusal outward without remapping:
`source_schema::schema_with_proofs_paid` and `source_schema::all`, then
`WorkspaceGraphBuild::{into_change_view_paid, into_operation_view_paid,
source_graph_schemas_paid}` in `workspace_graph/paid_view.rs` through
`ViewRefusal`. Remapping callers instead pass the literal they publish:
Change base and candidate (`SPX-G184`), Structural Change (`SPX-G192`) and
Operations (`SPX-G200`) call `into_*_view_reporting(form)`. A paid carrier for
that exact form moves out unchanged, and no replay Vec is formatted after the
refusal.

`PaidRefusal::into_report(expected)` and `into_published` return
`Result<Vec<Diagnostic>, Unpaid>`. Only a paid carrier for exactly `expected`
moves out. An unpaid carrier (`UnpaidReason::Carrier`), a carrier paid for a
different form (`FormMismatch`, dropped and never relabelled) or a non-replay
view refusal (`Legacy`) reaches the Copy `Unpaid` boundary without
allocating. `Unpaid::into_unfunded` is the only operation that then allocates
the requested literal. It is a named compatibility edge, not a funded report.

These compatibility boundaries remain allocating and unfunded, outside the
qualified contract:

- `Unpaid::into_unfunded`, at the view wrappers' outward edge.
- `ViewRefusal::Legacy`: fingerprint and sidecar `SPX-G173`, builder
  structure including `reserve_builder_structure`'s own limit Vec,
  literal-site, helper and schema-selection refusals. A remapping caller drops
  it and publishes its literal through `Unpaid::into_unfunded`.
- The pass-through wrappers `into_change_view` (test-only),
  `into_operation_view` (nominal rename), `source_graph_schemas` (test-only)
  and `source_graph_schemas_receiving` (Semantic Workspace preflight/replay,
  with that preflight's own held sources). These use the public `SPX-G410` form.
  A paid G410 carrier passes through them. An unpaid one is allocated there,
  only through `Unpaid::into_unfunded`.
- Callers above `semantic_workspace` preflight and the Change, Structural
  Change and Operations entry points still return owned `Vec<Diagnostic>`.
  No public type changes in this batch. A v0.10 typed outward refusal would be
  a separate, specified API change.

The qualified imported-helper caller uses `pure_records_checked`: its local
limit is an inline Copy refusal and its charge error moves out unchanged. The
legacy purity wrappers retain their original owned-Vec contract for other
callers. The actual paid acquisition-to-purity-refusal-to-report-to-Drop owner
covers the carrier and ledger together; this removes the classifier's own
refusal allocation, without claiming whole-engine funding.

Remaining concrete gaps inside the route (not claimed as funded):

- Replay engine refusals inside the Session that are not budget refusals
  (`replay_error` in `require_entry`, `selected_callees`, `selected_context`
  and lexical/scalar replay) still allocate their own formatted Diagnostic.
  The route moves that Diagnostic as the receipt and never publishes it, but
  its allocation is unfunded.
- This protocol funds only the outward report carrier and reuses the setup's
  prepaid refusal text as the receipt. It makes no whole-allocation,
  application-fit or authority qualification claim for the replay itself.

Every public literal and code is unchanged. Each selected module ledger now
also carries one report carrier debit (`size_of::<Diagnostic>()` plus the form
length) inside the existing `MAX_CHANGE_BUILDER_BYTES` and the enclosing
active budget. No ceiling is raised. The ordinary lexical proposal stays
unapplied, and no admission, schema or authority is widened.

Owning lib selectors under `cleanup_plan::replay::selected_entry::tests::paid`:

- `paid_report_zero_and_one_short_return_the_exact_rejected_ledger_without_allocating`
- `paid_report_setup_refusal_publishes_original_backing_and_keeps_setup_ledger`
- `paid_report_constructor_refusal_keeps_the_exact_ledger_and_first_receipt`
- `paid_report_budget_refusal_moves_the_prepaid_text_once_without_allocating`
- `paid_report_constructor_exhaustion_spends_the_prepaid_receipt_and_fails_closed`
- `paid_report_session_refusal_transfers_carrier_and_retains_first_receipt_through_drop`
- `paid_finish_returns_the_same_ledger_and_a_failed_finish_allocates_nothing`
- `paid_report_remap_publishes_only_its_prepaid_form`
- `paid_stage_moves_keep_each_report_with_its_original_ledger_without_pairing_arguments`

Actual-caller selectors under
`workspace_graph::source_schema::static_text::tests::paid_report`:

- `actual_caller_zero_and_report_one_short_return_the_unpaid_ledger_without_allocating`
- `actual_caller_paid_setup_refusal_keeps_original_carrier_ledger_and_drop`
- `actual_caller_authentication_refusal_allocates_and_discards_nothing`
- `actual_caller_authentication_and_session_refusals_transfer_one_paid_report`
- `actual_caller_one_short_budget_refusal_retains_the_prepaid_receipt`
- `reporting_views_publish_the_callers_prepaid_form_and_keep_success_unchanged`

Updated owning callers of the typed constructors and authenticators:
`hir::declaration_index::primitive_replay::{flat_record, option_u8}::tests`,
`hir::borrowed_scalar::option_u8::tests`,
`cleanup_plan::replay::selected_entry::borrowed::tests`,
`loop_calls::byte_writer::resolved::selected` (census control), and the
`static_text::primitive::{helpers, records}` test modules (closure-graph
oracle, entry and prepared controls). None has been executed.

### Closed scalar lexical receiver (OPT766, authored, not yet executed)

A second actual caller, `workspace_graph/source_schema/scalar_lexical.rs`
`receive_paid`, receives only the separate inventory captured by
`workspace_graph/static_text/scalar_lexical.rs` in both retention routes. It is
a finite profile: a no-import module whose every retained authored function,
contract and dead branch passes the closed census in
`static_text/scalar_lexical/census.rs` (Value scalar signatures, scalar
literals and unprojected places, unary/binary, block, `if`, condition-latched
loops, scalar `let`/assignment and typed exits; `Never` only on an exit and
the block/`if` nodes carrying it), with at least one authored exit. Every
other shape, and every view without receiving sources (Change, Structural
Change, Operations), keeps its existing route and refusal; the frozen
from-parts selectors still produce `SPX-G410`.

Capture is metered on its local work before it reads. After an O(1) shape gate
(which also requires equal authored and resolved function counts), it debits
each member's selector fields, one exact identity key comparison against the
paired resolved function and the census signature. Only then does it reserve
the one census cursor. Each body walk charges its `DEPTH` slot initialization
before the array exists. No `BTreeMap` lookup is performed. Before demand,
exhaustion declines (`None`); it does not fail. A module that is not selected
pays no builder bytes beyond that cursor. If the capture member `Vec` or the
receiver's member census `Vec` is allocated with a capacity other than the one
requested, the allocator's actual excess is charged afterwards to the same
ledger. That charge is never treated as prepaid. Capture then poisons the
active builder budget, and the receiver fails its preparation. Either way the
first refusal stays sticky.

The inventory retains the exact path, module, ordered member identities,
captured identity origins and typed member digests. It retains no source
revision, plan or schema. At reception the Semantic Workspace preflight passes
its own held normalized sources (`ReceivingSources`); the receiver recomputes
the primitive source revision from those bytes and requires every member's
typed digest to replay under it. One existing paid owner then runs exact live
membership, inventory binding, namespace/origin facts, digests, the closed
census, the declaration index, the Session constructor, every member replay
(`validate_paid` for lexical members, `validate_scalar_paid` for the others)
and `finish_paid`, in that order, with one carrier acquired before setup.
Authentication refuses with the Copy `SelectedReplayRefused`.

Only the private consumed `Received` receipt can select this module's Graph
schema. It holds a shared borrow of the exact replayed live module, so `select`
needs only pointer equality and does no per-member work after the ledger is
released. It maps census-derived
flags through `graph::scalar_base_lattice`, the shared ordered v17..v10 tail of
the base classifier; the excluded families' predicates are fixed `false`. A
receipt presented for any other module is the existing unfunded `SPX-G410`
literal. The Session's unmigrated `EngineError::Owned` Diagnostics remain the
open #781 gap above, so this route is not end-to-end funded.

Owning selectors under `workspace_graph::source_schema::scalar_lexical::tests`:

- `original_terminated_guard_is_received_from_current_source_with_full_program_parity`
- `canonical_semicolon_control_is_the_same_generation_and_body_drift_is_a_new_receipt`
- `mixed_unrelated_scalar_member_is_replayed_and_selects_the_oracle_usize_family`
- `mixed_exit_free_looping_member_is_replayed_by_the_paid_scalar_validator`
- `current_source_namespace_member_and_layout_attacks_refuse_with_one_paid_report`
- `reordered_missing_transplanted_detached_and_inserted_inventories_refuse`
- `certificate_and_plan_mutations_refuse_without_repair`
- `closed_family_exclusions_keep_their_existing_route`
- `receipt_selects_only_its_own_module_without_post_release_work`
- `census_charges_cursor_initialization_before_the_walk_with_exact_one_short_and_zero_work`
- `capture_selection_is_metered_and_unaffected_modules_keep_exact_builder_usage`
- `capture_capacity_excess_is_debited_after_the_fact_and_stays_refused`
- `source_less_change_view_keeps_the_original_refusal`
- `zero_and_report_one_short_return_the_unpaid_ledger_without_allocating`
- `exact_success_one_short_refusal_and_drop_share_one_observed_owner`


## Typed expression-fact receipt v1 (private, source-only)

This bounded OPT781 tranche is active on the local/imported paid callers but
has not been compiled or executed. It adds no source admission, authority,
physical allowance, work allowance, cache wire change or general engine funding
claim. Legacy lexical replay and test compatibility drivers keep byte-identical
owned errors via an explicit legacy policy.

`expression_facts/errors.rs` defines the private policy. Semantic fact failures
retain a static exact reason, the held body's shared `ExpressionId`, and, for a
duplicate, the exact repeated shared identity. Cloning these existing Arc handles
allocates no identity payload; the immutable source remains borrowed by Session.
These handles are observation data, never lookup permission. They are not a
formatted Diagnostic and do not promise the old inner Diagnostic text on the
new paid API. The fixed outward report is unchanged. The owned compatibility
policy renders the original complete function prefix and duplicate identity,
without truncation, against the saved independent producer/capacity oracle.

Physical fact-cursor, tree, array or clone-backing refusal returns an inline
reason from the physical sub-borrow. After that borrow ends, the original
ReplayBudget consumes its already prepaid text and moves that first Diagnostic
into the same failed Preparation. A single receipt sum prevents semantic data
and a later budget error from replacing one another. No new error identity
allocation, guessed text capacity, quota reset or formatted-error remapping is
introduced. Type work/depth and expression depth remain the original limits;
requires/body/ensures preorder, fact insertion/duplicate timing and all
preclone reservations are unchanged. Capacity and continuation carrier layouts
are still the actual `size_of` layouts at the existing reservation sites.

The paid Arena collector has a separate `ArenaCapacity` owner for its expression
continuations. Its original `FactCapacity` base still owns and funds the complete
514-entry type cursor. The new owner acquires its actual inline layout before
construction and owns one growing expression-frame vector across the function's
requires/body/ensures sequence. No caller supplies spare backing. Every growth
pays complete replacement backing and receiving controls while the previous
allocation is live, and charges every copied or newly initialized slot to the
existing work allowance. Reuse checks the held payer's sticky state, including
an Oracle epoch that has ended. There is no refund, quota increase, lookup
authority, or allocator credit. Both depth ceilings, all payload preclone charges,
fact insertion timing and complete original errors remain unchanged. The legacy
factory and a complete independent fixed-array Arena entry/producer remain
compiled, with full-source byte controls. Deep walks can pay more cumulative
replacement backing than the old fixed array; lower cost is claimed only for
measured shallow owners. Full-HIR and application qualification is pending CI.

The ordinary structural call lookup separately owns a growing stack of lexical
child cursors. Its original 514-slot traversal ceiling is independent of current
backing capacity. The first lookup acquires two slots; later descents acquire
complete replacement buffers up to that same ceiling, charging initialized and
moved slots to the existing work allowance. Requires/body/ensures order, early
lookup return, missing-call diagnostics and downstream signature/transfer checks
are unchanged. Reuse authenticates the existing payer before touching a slot.
The complete fixed-capacity owner remains independently compiled, and byte
controls reconstruct the full previous call module and original lookup body.
Deep traversals can reserve more cumulative bytes; no quota or refund changes.
`structural_control::calls::cursor::tests` owns real-HIR lookup parity, the raw
514/515 boundary, allocator overlap and release, exact/short/zero funding and
work-before-allocation controls. Full-HIR and application qualification remains
pending CI; isolated carrier probes do not establish application fit.

Supplemental receiving has its own independent 1,028-frame ceiling. Its paid
scratch owner acquires two initial frames and keeps that backing across complete
root walks. Deeper pending continuations acquire a full replacement, two vector
controls and a frame control while old backing remains live. Only moved live
frames consume relocation work; unused capacity is never initialized as a
semantic frame. The receiver's dispatch, parameter views, shape derivation,
flag order, callback order and original depth errors remain unchanged. Its
complete fixed-capacity receiver remains independently compiled, with a complete
source inverse and real-HIR callback-parity controls. Deep walks can pay more
cumulative backing than the original eager buffer; application qualification
requires the full owning gates.

Physical reservation wrappers propagate the compiler's caller coordinate to the
existing first-refusal receipt. This preserves all requests, debits, refusal
ordering and messages while identifying the requesting receiver rather than the
shared wrapper. It grants no source or replay authority.

Owning lib controls (authored, unexecuted):

- `cleanup_plan::replay::expression_facts::tests::errors::typed_fact_policy_matches_saved_legacy_text_order_and_full_facts`
- `cleanup_plan::replay::selected_entry::tests::fact_errors::paid_fact_cursor_and_growth_exact_short_keep_first_receipt_and_drop_one_owner`
- `cleanup_plan::replay::selected_entry::tests::fact_errors::paid_fact_semantic_receipt_keeps_exact_shared_ids_and_driver_verdicts`

The affine owner controls observe acquisition, Preparation, Session, the shared
fact producer, receipt/report transfer and Drop in one allocator scope. Source,
enclosing test-ledger setup and fixed observer metadata are excluded explicitly.
Constructor baseline debits are measured separately; the added two-node fact
cursor/tree census is independently literal. Exact/one-short constructor and
later growth, original report/ledger/prepaid text pointers and zero-allocation
retry/publication remain mandatory. Separate success comparisons run the actual
complete lexical and scalar drivers against their legacy verdict/work/backing.
The forged census inputs are not treated as HIR admission or successful replay.

Still open: profile/context/renewal errors, Lookup's structural unknown-ID
errors, inventory/status/structural/path/factored children and their dynamic
Diagnostic producers. The selected test-only borrowed-fact route retains its
existing error contract. This tranche does not establish full transitive
Session funding, ordinary V6 authority, or application fit/performance.


## Ordinary admission and allocation payment

The internal replay budget now separates `AdmissionMode` from
`AllocationPayer`. Ordinary replay remains ordinary when an existing active
builder pays for it. That payer never grants selected type, schema, provider,
renewal or profile authority. Selected reservations already debit the enclosing
builder, so they never receive a second active-builder debit. Legacy standalone
replay outside a bounded builder remains unobserved.

The actual ordinary fact map, slot tree, arbitrary record/variant shape walk,
leaf trees and storage directory reserve their known carrier/backing requests
before construction. Queue/vector growth, clone payment and shared certificate,
path and factored storage guards can consume the active builder. No new limit,
refund or separate ledger is introduced. The original arbitrary-shape algorithm
remains inline; a saved uncharged test oracle preserves its old verdict/order.
The first ordinary refusal String is charged before allocation; initial short
setup returns an inline H006 and retries allocate nothing. Longer current
function IDs acquire replacement refusal backing while the old buffer is held.

Authored controls (not executed):

- `cleanup_plan::replay::allocation::tests::ordinary_builder_payment_never_grants_selected_admission_and_covers_index_drop`
- `cleanup_plan::replay::allocation::tests::active_builder_zero_exact_short_refusal_is_sticky_and_moves_paid_text`

The first control encloses the actual paid indexes and Drop in one allocator
observation, with source, reference indexes, active ledger setup and fixed
observer metadata outside it. The second uses the independent literal request
of one refusal String plus three u64 elements for exact/short/sticky boundaries.
The complete ordinary entry and malformed slot still compare their old verdicts.
The same body checks all three early work-refusal APIs before facts or slots,
including complete legacy first text, and independently prices the ordinary
514-entry clone census cursor for exact/short/reuse boundaries. That cursor is
paid separately from the expression-fact collector's own type cursor.

This is not whole-validator or whole-replay funding evidence. Ordinary schema,
status, transition/exit and observation materialization children still contain
owned constructors, and full HIR validation independently constructs function,
origin, endpoint, scope/type, permit/effect and call indexes. Those remaining
allocations and semantic diagnostic producers require their own actual payment
joins before claiming complete final-validation allocation coverage. Existing
whole-lifecycle owners retain their inequalities and detect the remaining gaps.
No application fit, runtime result, cap increase or ordinary V6 admission is
claimed.

## Retaining typed outward owner (source-only followup)

[Workspace report owner v1](WORKSPACE-REPORT-OWNER-V1.md) adds an affine outward
receiver preserving the exact initial failure or failed Preparation and first
receipt through the typed source-preflight caller. Existing paid carrier
acquisition and physical equations are unchanged. Exact paid publication moves
the original slot/text; initial or mismatched paid publication returns the same
owner. All original legacy publication and Preparation/Session tests remain.
The filesystem view wrappers and full dynamic directory/TLS-help frontier are
explicit legacy residuals, so this followup does not complete OPT781.


## Selected-local static refusal receipts (source-only successor)

The closed local entry, member and profile guards in
`selected_context/paid.rs`, `selected_owner/paid.rs` and
`selected_profile/paid.rs` acquire their actual refusal controls on the current
ReplayBudget before constructing `Atom::SelectedSession`. The acquisition
covers Receipt, EngineError, the Context/function-ID/unit receiving Results
and the retained Option<EngineError>; it performs no diagnostic formatting.
The original guard order and detailed Diagnostic remain the independent legacy
reference. Publication derives its stage from the retained first receipt.

Error-only acquisition does not justify an uncharged increase in successful
receiving layouts. The owning control compares the complete original and current
sum/Option/Result sizes and alignments before that equality can be claimed.
The focused campaign must also exercise the actual public paid-report receiver
from current supported source, including complete Drop and fixed outward fields.
This does not bypass the outer source-graph authenticator to manufacture a
Session refusal. Execution qualification is pending.

Arbitrary renewal and child-helper Diagnostics remain their explicit Owned
boundary. This successor neither funds those producers nor completes whole
transitive OPT781 or application admission; allowances and caps are unchanged.


## Finite selected child refusal cut (source-only successor)

The actual paid Context entry now uses the exact bounded lexical scan and the
selected renewal/helper census/proof twins with closed inline failure data.
Lexical-depth, renewal/helper census and collection-probe failures keep their
original raw H006 rendering; renewal-work and lost-reservation failures keep
the original complete function prefix. No already-allocated Diagnostic is
translated into a paid receipt. Complete old receivers remain compiled, including
the full previous paid Context method, and all diagnostic fields/order remain
owning controls.

Before child entry, the same budget acquires an affine subject-borrowed permit
covering its complete receipt, error, optional and receiving/result controls,
on successful entry too. The typed census/proof receiving and scratch controls
are charged in addition to original backing/node requests; no old-header credit
is taken. Added initializer work is charged explicitly, with all original walk,
lookup and proof units unchanged. The permit owns only the one refusal slot
permission; it lends no lookup, source, member or provider authority. No report
or physical allowance is added. A work failure still zeros remaining before
return; a physical failure keeps the original first ledger receipt. Returning
the exact finite semantic receipt after either failure spends the permit paid
before entry, not a later debit or retroactive diagnostic allocation.

The permit carries the original borrowed subject and consumes itself to retain
the existing shared body identity. It accepts neither a replacement subject nor
a new payer at refusal. Raw detailed rendering remains explicit compatibility;
publication still transfers the original fixed outward report and ledger.
The previous complete sum/Option/Result layout controls remain mandatory: no
portable layout equality, runtime success, byte savings or application recovery
is claimed by this source-only successor. Actual added control bytes/work may
increase cumulative reservations.

Remaining explicit inner boundaries include selected_vector_element's old
ignored classification errors, the selected profile's collection/append and
other dynamic child Diagnostic producers, and arbitrary owned helper/renewal
routes outside this finite cut. They are not certified paid by the new receipt
or the outward report. This batch does not claim whole transitive OPT781 closure.

### Finite paid Context child refusal successor (#781, pending qualification)

The selected paid Context uses closed Copy data for the exact forty-two finite
child labels in its collection, direct-record, row-loan, string-transfer,
literal, latch and owner checks. Their original raw/function-prefixed six-field
diagnostics and first-error order remain the compatibility contract. A borrowed
subject permit acquires complete receiving/error/result/optional controls on the
existing invocation budget before child entry, including successful calls.
The additional Deferred/result controls and their four initializer units are
paid separately from the full eleven permit controls and any nested producer
controls. No old carrier is credited against a new acquisition.

Only real ReplayBudget methods construct the sealed budget-diagnostic arm.
Arbitrary owned child diagnostics cannot be classified as paid through this
seam. Nonselected or absent-owner compatibility wrappers remain explicitly
allocating legacy receivers. Renewal eligibility still ignores failed Vec
classification exactly where the original query did; replacing the discarded
Diagnostic with Copy data grants no new source or payer authority.

The complete former paid Context, profile and owner receivers remain executable
controls, with explicit namespace adapters and immutable whole-source inverses.
This finite adoption does not complete arbitrary later Session/replay/renewal
owned diagnostic propagation. Those residuals, unsupported admission profiles,
and outward legacy rendering remain explicit; issue #781 is not closed by the
source-only successor or by observing its tests before qualification.

## Duplicated driver-local receiving cut (#781, source-only)

The actual paid lexical and scalar driver bodies call the existing paid profile
and member-owner twins at their original positions. Their complete generic
Diagnostic-policy bodies remain independent legacy references. Specializing the
paid bodies removes the previous fact-builder callback/forwarding arguments; no
new helper Result, aggregate allowance, cap change or old-header credit is added.
The paid twins retain their actual receiving permits and initializer work, so
this repeated validation can reserve more cumulative controls than the former
legacy calls. No savings or admission claim follows.

The direct receiving owner compares both complete old driver entries with
profile and identity-by-address failures, checks all six legacy diagnostic fields
through the explicit renderer, and transfers the actual prepaid semantic receipt
into the same report/ledger owner through sticky rejection and complete Drop.
These are internal hostile receiving controls, not a bypass of public source
authentication. Source-only qualification is pending. Initial report acquisition,
inline zero/short setup, fixed outward publication and named unfunded legacy
boundaries are unchanged.

This is not transitive Session funding. Lexical-exit validation, structure bounds,
inventory/status construction, region/entry/exit/certificate/CFG validation, path
replay, factored comparison, and the scalar signature/body/schema/trace families
still include Diagnostic-returning children converted to EngineError::Owned.
Budget failures also retain their genuinely prepaid original Diagnostic. An
already-owned semantic Diagnostic is not retroactively paid by report transfer;
#781 remains open.

## Reference-coverage receiving cut (#781, source-only)

Paid lexical and scalar Session drivers use the complete reference-coverage
algorithm with closed edge-owner, exit-owner and status-declaration refusals.
The original Diagnostic producer remains independent. The two actual vector
wrapper calls each acquire a nine-slot receiving bank and nine initializer work
units on the same ledger before their arguments. Each genuinely new status-set
node separately acquires its eight-slot bank and eight units before its wrapper;
duplicates acquire neither. There is no guessed per-function loop allowance.
The existing profile permit covers entry/refusal composition. BudgetDiagnostic
remains sealed: only genuine ReplayBudget producers construct that arm.

The original status membership and traversal order, then edge/exit/status error
priority, stay exact. New receiving reservations and initializer work may refuse
before the former backing or semantic frontier; no old allowance is credited.
The node helper forwarding Result is eliminated lexically; the two real budget
wrapper Results and their Diagnostic/Deferred receivers remain explicitly paid.
Initial report acquisition, report carrier, inline refusal and all legacy
wrappers stay unchanged. This finite family and the prior profile/owner tranche
do not complete the remaining dynamic Session validator families: #781 stays
open. Focused semantic, exact/short/zero, duplicate, ledger and Drop controls are
authored; no execution or savings claim follows from this source checkpoint.

## Structural leaf-family receiving cut (#781, source-only)

Paid lexical and scalar drivers use complete slot, region and leaf algorithms
with closed static labels and the existing shared-identity dynamic atoms. The
attached flat-record leaf collector and storage-index constructor preserve all
membership, insertion, field, flag and error ordering. Every actual clone and
vector forwarding call acquires its nine/ten-slot receiving layout on the SAME
budget before arguments; conditional nodes retain their eight-slot bank only
when the original membership test admits insertion. The lexical expansion adds
no extra forwarding function or Result. Original nested budget Results remain.
Each helper acquires its existing profile permit for its actual return type;
slot dynamic Atom initialization/receiving has a separate two-Atom bank.

These payments and initializer work are genuine earlier refusal frontiers, not
old-fee credit or a refund. Genuine budget errors alone use sealed
BudgetDiagnostic; static and shared-identity semantic failures remain closed
receipts for the already-funded report. Compatibility Diagnostic renderers and
full immediate-old drivers remain independent. The initial receiving boundary
retains the existing report/ledger contract; no arbitrary child Diagnostic is
certified after construction. The inventory builder still returns an owned
Diagnostic through its raw physical callback; status, certificate, path and
other unmigrated families remain residual, so #781 stays OPEN. No runtime,
allocation reduction, application-fit or end-to-end funding claim follows.

## Status and scalar-local receiving cut (#781, source-only)

Paid lexical/scalar drivers use complete status reconstruction with seven closed
labels and one shared-identity unknown-callee atom. Requires, body, ensures,
ordinal conversion, case normalization and all vector/clone ordering remain.
Each actual twelve-site array/Vec/clone call acquires the existing exact
same-ledger receiving layout before arguments. The dynamic Atom initializer and
receiving pair is paid only when the unchanged callee lookup actually fails,
before its identity clone or receipt construction. Successful collectors do
not acquire that error-only bank. The original complete producer is unchanged.

Selected scalar record/body checks retain the full signature, rooted parameter,
projected field, mode/type and cursor algorithms. The original pure type-key
math is lexical, removing extra helper Results rather than introducing nested
mutable-budget forwarding. Error Option conversions are lexical returning
branches so the affine permit survives successful iterations. Each actual
new helper Result/receipt bank is acquired for its return type before entry.
The generic borrowed-scalar helper remains unchanged and has only its original
real ReplayBudget charge callback; its new outer Error mapping is prepaid and
cannot convert an arbitrary semantic Diagnostic. Existing typed inline record
classification and its exact original refusal labels remain source-bound.

Complete immediate-prior paid drivers, original scalar traversal and record
helpers remain independently executable. Four owning controls cover complete
status/static/dynamic diagnostics, actual exact/short/work/Drop, full record
shape answers, and source-authenticated whole old/current drivers. They are
unexecuted source gates. New entry/control payments may refuse before an old
semantic or backing frontier; old fees, forecasts and caps are unchanged.

This cut does not establish transitive #781 funding. Inventory-builder raw
callbacks, lexical certificates, scalar schema/borrowed-test adapters, CFG,
path and factored descendants still include original owned Diagnostics.
Those genuine boundaries require separate typed cuts; no late relabeling or
whole-Session closure is claimed. Ordinary and legacy paths remain allocating.

## Certificate receiving and dead Deferred slots (#781, source-only)

The selected lexical certificate entry and its dictionary/DFS children acquire
the existing per-call same-ledger receiving banks before real array, clone,
Vec or conditional node arguments. Fifty-two static semantic failures carry
closed certificate data. Original traversal, branch order, normal-end checks,
work and error text remain; original certificate and complete immediate-prior
paid drivers remain independently executable. Borrowed chain, type/drop and
lexical-boundary children still forward their original owned Diagnostics and
are explicit residuals. No arbitrary Diagnostic is certified after creation.

Five scalar-record helpers, the scalar driver entry and the borrowed-scalar
bool receiver use the existing acquire bank: their own scopes never construct
or resolve Deferred, including genuine old budget-error forwarding. Real
child Deferred producers retain their own profile banks. This removes exactly
the dead four-slot profile tuple and its initializer work, without a refund or
new protocol. Initial receiving payments remain genuine earlier refusal
frontiers. Four unexecuted owners cover full certificate fields/hostile order,
actual bill/work/exact-short-zero/Drop, complete old/current drivers and exact
read-only slot removal. #781 remains OPEN for unmigrated descendants; there is
no measured saving, application-fit or closure claim.
