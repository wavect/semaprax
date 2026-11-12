# Whole-subtree owned-record pattern binding v2

Audience: compiler contributors and reviewers.

Status: focused OPT779 source, HIR, cleanup, cache and three-backend
qualification passed, including the Catalog command adoption.

This is an additive internal language rule over the existing bounded nested
owned-record profiles. It does not change Project/public aggregate ABIs,
generics, templates, variants, resources, stored views or collection families.
The v1 borrowed-pattern rule remains exact recursive destructuring.

An exact irrefutable `match own` of a named, unprojected owned record may bind
an entire monomorphic record field. For example `Outer { report, valid }`
moves `report` as one owned child record. Other siblings may use recursive
patterns, and authored fields may be reordered. Every declared field must
still occur exactly once; hiding an owned subtree with `_` is rejected.

Source verification and independent HIR validation authenticate the current
record, field, field type, ownership mode and exact generated `field.binding`
identity. The inherited depth64, transitive-owner256 and field4096 limits
count the complete root, including descendants hidden by the child binding.
A live descendant loan blocks the root move. Borrowed whole-owned-subtree
bindings, projected scrutinees and repeated moves remain refused.

Cleanup construction and replay derive independent declaration-order inventories.
One existing `Transfer` from the exact field prefix to the new binding root maps
all descendant leaf paths by their relative stable field-ID suffix. It preserves
actual completion history; it neither normalizes nor repairs canonical vectors.
The existing cleanup-plan.v8/graph.v28 vocabulary suffices: no field, transition
or wire representation changes. Higher lexical/loan schemas retain their
ordinary precedence. A v7 downgrade of a subtree destructure is refused.

The interpreter moves the existing child Record value. Native and Wasm create
ordinary aggregate carrier storage and move its fields, clearing staged owned
carriers; canonical leaf transfers and liveness flags still govern cleanup.
No owned payload or record tree is cloned, rebuilt or reacquired after failure.
Copy fields remain available in the destination. Arm failure and lexical exits
retain sticky status, canonical settlement and late result publication.

Owning controls include the independent recursive unpack/rebuild runtime oracle
in `nested_collection_record::subtree`, exact cleanup/graph prefix and mutation
checks in `nested_record_destructure_tests::subtree`, and existing nested-record
limit, loan, cache, source-identity and backend-hostile gates. The exact and
one-short builder checks covered actual resolution and independent replay
through sticky refusal and Drop.

Catalog binds `CatalogResult.report` and forwards that owner to the existing
encoder after the validity check. The adopted command and the frozen recursive
unpack/rebuild command passed all 23 original cases at native O0 and O2 with
identical status, stdout and stderr. The ordinary Project native build and
the existing differential routes passed the same corpus.

The additional `nested_collection_record::subtree::lifetime` owners keep a
transferred child live on actual Break/Continue paths, and instrument the native
forward-entry to first-consumer boundary for allocator requests and memcpy/
memmove payload work and every realloc request, alongside the old recursive control. The observer also
requires three exercised boundaries and complete canonical settlement at O0/O2;
it excludes compiler preparation. The probes passed on both native builds.
The owning three-backend cases also passed with live subtree cleanup across
Break and Continue and with late field, call and contract failures.

Independent ordinary replay now compares inventory entry places and supplemental
slots directly from current immutable HIR instead of collecting duplicate owned
expected directories. A paid sorted borrowed index retains the old last duplicate
storage-ID selection; liveness lookup still retains first duplicate flag selection.
A mismatch is selected only after the complete corresponding old fallible source
collection frontier. Supplemental traversal reuses one actual paid 1028-frame
cursor without changing its depth/capacity rule or replay Work equations.
Direct reconstructed shape vectors, lifecycle backing, type substitution vectors
and callable boxes acquire the current invocation's payer before construction.
The complete old coverage, supplemental and shape receivers remain independent
oracles. New original-corpus verdict/work, duplicate/late-error, whole allocation,
debit, exact/short/sticky and receiving Drop controls are authored source only.
Declaration fact-cache misses on the actual replay classifier route now acquire
existing caller payment before owned type/key/layout, vector, callable box and
ordered memo/visiting backing. Exact borrowed admission checks remove discarded
native-result and authored byte-variant substitution trees; Finish borrows the
existing child facts instead of collecting another discarded vector. Current
cached facts retain their complete original precedence, including hostile flags
and the owned-key fallback when the bounded borrowed census declines. No new
classification depth, work allowance, cache authority or quota is introduced.

The paid string receiver preserves the original logical append order, reserved
floor suppression, partial CappedString output and strict formatter behavior.
A logical sink decline performs no physical growth and is not converted into a
sticky physical refusal. Actual additional physical construction debits may
refuse earlier than the original unfunded constructor; rejected intermediate
backing stays charged through Drop. All accepted full classifiers compare full
layout keys against the complete original classifier. The ordinary droppability
twin changes only its fact callback and retains source/call/error order; its
pending/visited/substitution and semantic error-String backing, plus the fifteen
synthetic owning parameter-provider families, remain explicitly unpaid residuals.
No whole-replay funding, savings, OPT closure or execution is claimed.

Additional authored owning controls have prefixes
`cleanup_plan::replay::paid_type_facts::tests::` and
`bounded_output::classifier_string::tests::`. They compare complete original
nested/resource replay verdicts and Work, full uncached layouts, hostile cache
precedence, full identity/substitution/admission oracles, actual same-payer
constructor/receiving/Drop allocation versus debit, zero/exact/short/sticky
refusal, cross-invocation refusal, and independent floor/overflow formatter
behavior. The TLS invocation factory is outside the classifier payer contract;
the cross-invocation control observes no allocation by the refused classifier,
without claiming the separate factory is paid by that classifier.

The new owning selector prefix is
`cleanup_plan::replay::ordinary_inventory::tests::`, with controls
`complete_original_nested_and_resource_variant_receivers_preserve_verdicts_and_work`,
`late_unknown_entry_storage_precedes_retained_earlier_entry_mismatch`,
`one_paid_fixed_cursor_reuses_original_capacity_without_per_expression_requests`,
`whole_current_inventory_exact_short_and_sticky_refusal_settle_through_receiving_drop`,
`paid_substitution_preserves_complete_original_callable_and_nominal_payloads`,
`duplicate_inventory_ids_keep_the_complete_old_last_insertion_receiver`,
`complete_original_inventory_construction_receiving_and_drop_reports_both_real_costs`,
and `complete_paid_shape_receivers_keep_original_flags_exact_short_sticky_and_drop`.
