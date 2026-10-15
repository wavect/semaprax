--- checkpoint/docs/evidence/weighted-storage-838/README.md
+++ amortized/docs/evidence/weighted-storage-838/README.md
@@ -7,21 +7,25 @@
 correction. No Cargo, build, test, Clippy, C, Node, or model execution was
 performed. Formatting is source preparation, not compiler qualification.
 
 The production closure and weighted closed-accumulator maps now use a private
-AVL arena. Each node stores its numeric key, route weight, left/right/parent
-indices and height. Dense geometric Vec growth acquires its complete replacement
+AVL arena. Each node stores its numeric key, route weight, private 32-bit left/right/parent
+indices and height; numeric keys and canonical IDs remain `usize`. Dense geometric Vec growth acquires its complete replacement
 backing before allocation while prior backing remains charged; keys do not index
 capacity, including `usize::MAX`. Lookup and insertion remain logarithmic;
 rotations and final Drop are iterative. Ascending iteration follows parent links
 with no traversal allocation. It replaces the old ordered iterator at the same
 loop sites. Comparator/rebalance internals remain outside the existing semantic
 work counter, just as std BTree internals were; exposed work requests are exact.
 No CPU-time or allocation improvement has been measured.
 
-The arena initializer/receiving owner, new insertion controls (all three Result receivers) and node
-initializer, closed-view initializer/receiver and borrowed reference are paid
-separately from arena allocation. Existing memo, Rc/header, weighted vector,
+The arena initializer/receiving owner, growth controls (all four Result
+receivers), actual empty-node initializers, closed-view initializer/receiver
+and borrowed reference are paid separately from arena allocation. Growth pays
+for and initializes every real spare slot. Insertion mutates existing node
+fields and returns unit; a first entry uses already-owned inline scalar fields.
+Only a necessary growth calls the fallible helper. No repeated insertion Result
+constructor survives behind a unit-returning wrapper. Existing memo, Rc/header, weighted vector,
 label/target, state/signature and refusal constructors retain their payments.
 A unit-weight singleton moves its authenticated closure Rc into a borrowed
 closed view. It executes each original checked multiply/add and weighted-route
 work request, and creates no second numeric map. Other singleton weights and
@@ -43,8 +47,19 @@
 Parent batch metadata must compose the new joins through its authenticated
 current-source adapters before invoking older source-history proofs; no old proof
 body or historical archive was edited to exempt the new owners.
 
+The original paid-insertion draft is checkpoint `41b6e19ad`; the `amortized/`
+subdirectory preserves its complete immediate originals and exact inverses.
+The new source checks authenticate current bytes back through that checkpoint
+before invoking its original full-source assertions. No prior inverse or
+historical archive is rewritten. Every production growth guard bounds a map by
+the original 32,000,000 work ceiling plus its one optional own/sink entry; the
+next power of two bounds all spare slots below the private u32 sentinel. Growth
+rejects before arithmetic/allocation beyond that bound. Sparse numeric IDs do
+not participate in the conversion. There is no new public source limit or work
+debit; current-source/private hostile metadata tests own the unreachable guard.
+
 The `.before.txt` files are complete immediate originals, including unchanged
 interiors. Matching `.patch.txt` files provide exact forward and inverse edits.
 `ordered_weights::source` checks both directions and the complete receiver
 adaptation. These are source-authored assertions, not execution evidence.
@@ -53,9 +68,11 @@
 `cleanup_plan::replay::factored::canonical_shared::`:
 
 - `ordered_weights::tests::ordered_arena_all_rotations_numeric_extremes_and_dense_permutations_match_btree`
 - `ordered_weights::tests::ordered_arena_each_growth_zero_exact_short_keeps_actual_allocation_separate_and_drops`
-- `ordered_weights::tests::ordered_insertion_all_three_receiving_results_are_paid_even_without_growth`
+- `ordered_weights::tests::ordered_growth_receiving_and_initializers_are_paid_then_warm_insertion_constructs_no_result`
+- `ordered_weights::tests::ordered_private_index_growth_refuses_before_allocation_or_mutation`
+- `ordered_weights::amortized_source::ordered_amortized_checkpoint_full_sources_and_inverses_are_exact`
 - `ordered_weights::receiver_tests::ordered_complete_old_closures_successors_ids_signatures_and_weighted_overflow_match`
 - `ordered_weights::receiver_tests::ordered_complete_old_each_exposed_work_frontier_preserves_first_diagnostic_and_overflow`
 - `ordered_weights::receiver_tests::ordered_full_receiver_actual_reservation_allocation_peak_and_every_exit_drop`
 - `ordered_weights::source::ordered_complete_immediate_sources_round_trip_in_both_directions`
