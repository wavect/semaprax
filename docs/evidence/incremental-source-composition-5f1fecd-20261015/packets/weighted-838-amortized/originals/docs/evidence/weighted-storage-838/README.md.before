# OPT838: paid ordered numeric canonical storage

Status: source-only NEXT batch; executable qualification pending.
Audience: compiler contributors and reviewers.

This successor follows OPT839 including its per-call authentication-fee
correction. No Cargo, build, test, Clippy, C, Node, or model execution was
performed. Formatting is source preparation, not compiler qualification.

The production closure and weighted closed-accumulator maps now use a private
AVL arena. Each node stores its numeric key, route weight, left/right/parent
indices and height. Dense geometric Vec growth acquires its complete replacement
backing before allocation while prior backing remains charged; keys do not index
capacity, including `usize::MAX`. Lookup and insertion remain logarithmic;
rotations and final Drop are iterative. Ascending iteration follows parent links
with no traversal allocation. It replaces the old ordered iterator at the same
loop sites. Comparator/rebalance internals remain outside the existing semantic
work counter, just as std BTree internals were; exposed work requests are exact.
No CPU-time or allocation improvement has been measured.

The arena initializer/receiving owner, new insertion controls (all three Result receivers) and node
initializer, closed-view initializer/receiver and borrowed reference are paid
separately from arena allocation. Existing memo, Rc/header, weighted vector,
label/target, state/signature and refusal constructors retain their payments.
A unit-weight singleton moves its authenticated closure Rc into a borrowed
closed view. It executes each original checked multiply/add and weighted-route
work request, and creates no second numeric map. Other singleton weights and
multi-input states use a paid accumulator. Duplicate entries, zero weights,
route multiplicity and the first checked overflow keep their original meaning.
No reserved floor, sticky refusal, ledger identity, cap or forecast changes.
The enclosing memo authenticates the function, budget address and active ledger
before any closure view; restoring a foreign/stale owner cannot revive it.

`before_ordered.rs` contains the complete immediate-old BTree receiver, with
only imports/module registration and test visibility/accessors adapted. It
shares the unchanged OPT839 Memo and the unchanged preceding BTree epsilon
helper; neither is an optimization target of this lane. All earlier complete
receivers and tests remain. Two older closure-test comparisons now compare the
entire ordered iterator/outcome directly; they allocate no projected BTree map
inside an allocation observer. Their complete original test bodies are archived
and the full forward/inverse source tests bind every changed file byte.
The additional audit joins include the complete helper, controls and old body.
Parent batch metadata must compose the new joins through its authenticated
current-source adapters before invoking older source-history proofs; no old proof
body or historical archive was edited to exempt the new owners.

The `.before.txt` files are complete immediate originals, including unchanged
interiors. Matching `.patch.txt` files provide exact forward and inverse edits.
`ordered_weights::source` checks both directions and the complete receiver
adaptation. These are source-authored assertions, not execution evidence.

Pending focused library selectors under
`cleanup_plan::replay::factored::canonical_shared::`:

- `ordered_weights::tests::ordered_arena_all_rotations_numeric_extremes_and_dense_permutations_match_btree`
- `ordered_weights::tests::ordered_arena_each_growth_zero_exact_short_keeps_actual_allocation_separate_and_drops`
- `ordered_weights::tests::ordered_insertion_all_three_receiving_results_are_paid_even_without_growth`
- `ordered_weights::receiver_tests::ordered_complete_old_closures_successors_ids_signatures_and_weighted_overflow_match`
- `ordered_weights::receiver_tests::ordered_complete_old_each_exposed_work_frontier_preserves_first_diagnostic_and_overflow`
- `ordered_weights::receiver_tests::ordered_full_receiver_actual_reservation_allocation_peak_and_every_exit_drop`
- `ordered_weights::source::ordered_complete_immediate_sources_round_trip_in_both_directions`
- `ordered_weights::source::ordered_full_old_receiver_and_alias_adaptation_are_exact`

Retain the complete existing `tests`, `epsilon_alias::tests`, `memo::tests`,
`scratch_tests`, `singleton_target_tests` and `receiving_layer_tests` owning
selectors, including their foreign/stale/restored/floor/sticky and physical
refusal controls. Parent qualification also owns the unchanged #808 production
native/project codec selectors, original library `codec808_frontier` witness,
`identity_prebound_production_core_retry_preserves_phase_debit_and_nested_refusal`,
`catalog_normalizer_compact_core_fits_production_builder_cap`, original
106-selector failure-recovery denominator and scoped Clippy. Only the large
batch boundary runs them; no full suite/gate is authorized for this lane.

State/signature dictionaries and persistent target storage remain follow-ups
requiring attribution. Actual d6bf production codec cap failure remains the last
execution evidence supplied to this lane. The 48,840,045-byte library trace is
aggregate final-HIR validation spend including its cfg(test) oracle; it is not
cleanup-only or a document_end allocation. This source change establishes no
cap fit, byte gain, speedup, hosted/public support or agent token/turn saving.
