# OPT839: exact-comparison retained canonical scratch

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only NEXT batch, based on `94c392e9bd8b60edba497263d0267ec54a4dd728`.
No Cargo, build, test, Clippy, C, or Node execution was performed for this lane.
Static formatting and source/inverse checks are separate from qualification.

`canonical_shared::Canonicalizer` retains paid cursor, canonical-frame and open
identity vector backing across closure lookups and both roots of its existing
immutable automaton comparison. The original weighted closure and closed-map
representations remain unchanged here; OPT838 owns their separate migration.
Open identity membership/removal now scans an insertion-ordered vector; it does
not influence canonical label/signature ordering, IDs, arithmetic or work units.
An outer receiver clears all retained references on success, arithmetic overflow,
cycle, work refusal and physical refusal. Capacity survives; final Drop releases
it. Growth uses the existing full-replacement reservation while prior backing
remains charged. There is no refund or allowance transfer.

A closure cache hit authenticates its exact function, ReplayBudget address and
active ledger, then charges the original single cursor work unit. It constructs
no throwaway cursor vector. Misses use retained cursor backing. Canonical frame
and open storage acquire actual capacity before each push; allocation-free
ordinary/no-owner routes retain their original payer behavior.

`memo::RetainedMemo` composes the complete ordinary Memo. Every authentication,
including a warm cache hit, calls the original Result-returning authentication
and pays its per-call `size_of::<Result<(), Diagnostic>>()` authorization receipt.
The unit-returning wrapper does not remove that inner Result construction. It
keeps a held refusal slot, but claims no authentication-carrier saving. The
ordinary implementation checks the original ledger before querying the payer
and performs sticky zero-byte authentication before the positive receiving fee.
Foreign/stale use invalidates the receiver, hides its values, never queries a
foreign ReplayBudget, and cannot be repaired by restoring the owner. A selected
payer Diagnostic is moved intact; the already-terminal sentinel is the original
allocation-free empty SPX-H006.

The source accounting pays the new wrapper's inline controls and failure
receiving once at activation, including the real Result receiver of its added
control reservation. Misses pay their additional inner Result receiving,
prepare-scratch Result, and copied root scalar. Each canonical call pays its
additional inner Result receiving, moved root Vec header and prepare-scratch
Result. The cursor/frame/open headers are paid once at scratch activation.
Original Rc/vector/map constructors retain their charges. The original ordinary
Memo implementation remains present, with only its now-test-only insert gated
and the retained submodule registered. Neither an extant constructor's fee nor a
work debit was deleted to claim savings.

The complete immediate canonical and Memo originals are retained byte-for-byte
in the two `.before.txt` files. `before_scratch.rs` is the complete canonical
receiver with import/module adaptations, closure test visibility and read-only
accessors; its independent Memo is `before_scratch_memo.rs`. The source regression
reconstructs these adaptations from the complete originals. Both existing source
audit joins include every new Rust owner. Prior originals and proof histories
are untouched. External lane-local provenance holds all changed-file originals,
a complete inverse patch and exact current-byte hashes; it does not compose or
rewrite earlier global archives.

Pending focused library selector prefix:

`cleanup_plan::replay::factored::canonical_shared::scratch_tests::`

It owns complete immediate-old/current closures, states, signatures, IDs,
duplicate weights, overflow, full Diagnostics and every work frontier; actual
allocation/peak/free/last Drop; zero/exact/one-short and every actual byte cut;
cache-hit no-allocation/work preservation; foreign/nested/restored owner; and
ordinary/no-owner controls. A warm-authentication boundary control requires the
full per-call Result fee, rejects zero and one-byte-short availability before any
new allocation, and checks the complete ordinary authenticator source against
the immutable original. Existing canonical_shared, memo, epsilon_alias,
receiving_layer_tests and singleton_target_tests selectors remain required.
All assertions are authored and unexecuted.

At the parent batch boundary run only the exact new/affected owners, scoped
Clippy and the unchanged #808 native/project and builder recovery selectors
recorded by the batch. In particular preserve:

- `identity_prebound_production_core_retry_preserves_phase_debit_and_nested_refusal`
- `catalog_normalizer_compact_core_fits_production_builder_cap`
- the original `codec808_frontier` library diagnostic witness
- `string_settlement::codec_allocation::string_vectors::primitive_string_codec_reads_and_short_refusal_allocate_nothing_then_copy_final_string_once`
- `string_settlement::codec_allocation::string_vectors::primitive_string_codec_actual_byte_and_item_plus_one_preflight_never_reserve_output`
- all five unchanged `standard_library::application_json::nested_response::string_vectors` cases.

The user forbids full suites/gates for this batch. Do not substitute this draft
for completed runtime checks. Actual d6bf evidence was 86 pass / 20 fail, including
10 existing builder/codec application-cap failures. Its 48,840,045-byte figure is
aggregate final-HIR-stage debit including a cfg(test) oracle, not production
`document_end` or cleanup-only cost. No application cap-fit, measured saving,
hosted support, or live-agent token/turn improvement is claimed.

Source-review correction after `15ac307443132d8631e8b16b036ba9a689d97139`:
the initial warm unit-returning path still called `reserve_bytes(0)`, whose
Diagnostic-bearing Result was a real repeated construction. Its one-time controls
could not fund that per-call receiver. The corrected draft delegates every call
to the unchanged ordinary authenticator and restores its full fee. The remaining
structural benefit is retained cursor/frame/open backing; all measured benefit
and application recovery remain unqualified.
