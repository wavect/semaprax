# OPT878 borrowed merge keys: source candidate

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

This is the key-storage part of issue #878, based on
`36c6deb98720e243b0daaf81e813fbe3ec3b0a2d`. Execution and application
qualification are pending. It does not close #878 or claim that the reported
width-one or arena application reservations fit.

`merge_keys.rs` retains the two complete grouping algorithms and the borrowed
suffix acquisition route. Instead of cloning owned-source, continuation and
commit payloads into a BTreeMap, a sorted vector stores group indices. Lookup
compares the same fields in the same order by borrowed content. The groups
vector remains in first-appearance order. No reference is stored across group
or directory growth, and indices are discarded before publication consumes the
groups. Directory insertion shifts indices; comparison is logarithmic and
index insertion is linear. This changes CPU work, not the materialization-unit
meaning in RFC 0003, and no CPU improvement is asserted.

The original merge work charge precedes all new scratch. A directory header,
borrowed lookup controls and each actual index backing growth are paid before
construction through the existing replay payer. Old member/group/publication
and union requests remain. The old key clone and BTree node requests disappear
because those payloads and nodes are no longer constructed. Existing caps,
sticky refusal, diagnostic floors and authority selection are unchanged.

The complete input `skeleton_work.before.rs` is retained; `before-inputs.json`
binds its whole length and SHA to the base commit. The three old methods are
independently compiled in `merge_keys_before.rs`. The only changes in that
compiled slice are method identifiers (including the old suffix self-call) and
explicit replay visibility required by the new child module. Its statements,
payment requests and literals are preserved verbatim. The production suffix
route still clones the complete borrowed input before merging; no suffix Drop
order change is hidden in the key change.

The four new lib owners under
`cleanup_plan::replay::skeleton_work::merge_keys_tests` compare full ordered
small languages and multiplicities with the complete prior methods; cover
ordinary/selected, lexical and pending-commit cases; compare every original
work-refusal Diagnostic field; and probe cumulative allocation through success,
exact capacity, every smaller physical allowance and destruction. These tests
have been authored and rustfmt parsed; they have not been executed. The metric
owner prints old/current cumulative reservations, allocations and highwater
without assuming that fewer allocations implies a reservation improvement.

Integration must join `merge_keys.rs`, `merge_keys_before.rs` and
`merge_keys_tests.rs` into the three existing raw source inventories and the
MAIN043 definition domain, retaining every prior source owner. The historical
whole-source bindings need an explicit new finite successor generation for
`skeleton_work.rs` and changed inventories. No source-custody refresh, Cargo,
common gate or public qualification was run in this private lane.

The second source candidate consumes the suffix vector into an explicit
retention frame whose prefix field precedes its original suffix field. Original
suffix observation backing and metadata remain intact through every union and
sequence refusal. Thin merged rows copy only observation backing for independent
unions; they borrow representative metadata by index. A separate borrowed view
array feeds a copy of the current sequence core and its one-successor macros.
This deliberately retains observation copies to preserve original-owner Drop;
it removes the full `ExprSkeletonPath` clone array and owned-source/continuation
copies. It does not claim zero suffix-copy allocation.

The whole original 7,847-line replay input and the original one-successor module
are archived with exact lengths and SHA-256s. The complete original borrowed
sequence function and original one-successor module remain byte-exact in
production. The adapted core retains all original payment literals, work,
materialization events and control branches. Borrowed view field access and
fanout inspection adapt their input representation. The copied macros use the
unchanged original fee constants; no old fee is reduced. A paid query precedes
the new dispatch on every active expression sequence, including ordinary and
one-suffix fallthrough. The larger retention frame is charged only before its
owned branch. This is a real additional ordinary reservation cost, with no
exemption or application-fit claim. Original historical test selectors dispatch
to their complete prior borrowed receiver before the new path.

Four further lib owners under `cleanup_plan::replay::owned_suffix::tests`
compare complete ordered languages, failure/residual/owned-source/continuation
states and exact work on ordinary/selected and lexical cases; compare all six
original work diagnostics and watched original backing free order; sweep every
smaller physical allowance through union and output refusal, requiring original
prefix carrier release before original suffix carrier release; and report
cumulative allocation/reservation through result destruction. These are authored,
not executed. Source audit registration must also join `owned_suffix.rs`,
`owned_suffix/sequence_vector.rs` and `owned_suffix/tests.rs` to each raw3/MAIN043
scope. The fixed native carrier still must remain at most 1,500 lines.

The current actual 452 baseline also has existing regressions to rerun unchanged:

- `cleanup_plan::replay::factored::canonical_shared::memo::tests::chunked_complete_canonical_receiver_preserves_old_work_ids_and_last_drop`
- `cleanup_plan::replay::factored::tests::ordinary_factored_functions_and_contracts_reuse_exact_program_storage_with_old_automata_work_and_drop`

The reported memo failure spends 16 extra ordinary work units in current source;
the factored regression underflows its old-minus-current allocation subtraction.
No assertions or counters in those owners were changed here, and the new key
and suffix paths add no logical work units. All application and regression
qualification, independent fee/ownership review, and finite source binding
integration remain pending. #878 is not claimed closed.

The independent review followup retains a standalone whole base36 sequence in
`owned_suffix/before.rs`. Its only transformations are function visibility,
explicit before-merge method names, and the explicit original sequence-macro
namespace. The owned test baseline calls this body, rather than the unchanged
production wrapper whose merge methods now select borrowed keys.

Owned suffix preparation first reserves its actual observation copy array and
clones every observation payload in original order, then attempts the original
merge-work debit. The cloned observation buffer is consumed in original order;
original suffix owners remain in the outer retention frame. A new mixed-refusal
owner checks all six diagnostic fields against the old suffix route when both
byte and merge-work budgets are exhausted, and when only merge work is exhausted.
New per-input clone argument/result carriers are paid at the dispatch boundary.
The actual absent metadata copies are never charged as historical credit.

Borrowed comparison now uses explicit ordered matches, preserving the old tuple
field order without comparison-chain closures. The search price accounts for
actual/formal path references, field comparison receivers, results, directory
and closure captures, and the Vec-to-slice search boundary. Its per-comparison
price is multiplied by bit-length plus one, a conservative bound on halving
search callbacks. Directory creation separately pays its new header and payment
argument/result transport. All these new prices can increase reservations;
independent census review and real strict application measurements remain pending.
The additional before module must join the same source-audit scopes as every
other new physical module. No source audit refresh has run in this lane.

A later narrow timing correction acquires `SearchComputation` before calling
`search_controls`: path/call mergers pay a per-input array after the directory
acquisition and before constructing that directory; owned suffix dispatch pays
one computation inventory per suffix before moving inputs. Eight usize roles,
four u32 roles and both Vec-length receiver borrows cover the dynamic helper
boundary and arithmetic. Constant base/callback arithmetic is declared as consts.
The returned search price no longer claims to pay for its own earlier computation.
The directory price also acquires the subsequent per-input reservation's argument,
Result and count transport before that call. This remains a pending source census,
with no execution, application-fit or fee-reduction claim.
