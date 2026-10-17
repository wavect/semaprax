# Ordinary prefix fusion candidate (source only, not ready)

Base: `fade6f4df00808a4f9b763fff22cd5f3898683f4`. Private source candidate;
no Cargo, compiler, test, benchmark, application replay, graft indexing, or full
quality gate was run. The parent integration checkout was not edited.

## Blocking accounting issue

The candidate currently pays `prefix_fusion::CONTROLS` after the original first
clone-work debit and before its eligibility probe. This correctly prices the
probe itself, but means small, non-saving, DAG, selected, oracle, expired, and
work-short receivers can encounter an additional byte refusal before entering
the complete old fallback. The requested final design requires those receivers
to decline before any new fee. Do not integrate this candidate as satisfying
that requirement.

`FINAL_SPLIT_CONTROLS` currently prices one usize (its payment price), split
references, Options and iterators, with no explicit new bool/work/length/growth
probe roles. I have not claimed those roles are already paid. Moving the new
fee after an unpriced probe would disguise this issue; increasing the split fee
would still add a fee to receivers that must decline without one. A concrete
funded eligibility frame must resolve that boundary before qualification.

## Candidate's physical boundary

Only ordinary ActiveBuilder flat-prefix/nonempty-flat-suffix earlier siblings
can fuse. It requires enough actual existing skeleton work for the extension,
nonoverflowing clone bytes and final geometric capacity, and removed clone
backing at least as large as all new controls. Sole suffix and final sibling
retain their old move routes. Selected and leased oracle accounts never fuse.

The fused arm preserves both work debits and materialization events, then
acquires the one final Vec backing at the old extension-growth frontier. It
clones prefix and suffix Rc handles left to right, retains the same final
capacity, and constructs no intermediate Vec. This removes a real allocation;
it does not discount one that still exists. The caps, floors, word layout, DAG
semantics, cleanup ordering and ABI are untouched.

Byte admission can legitimately differ after removal. The authored whole-owner
control compares actual requested allocation count/bytes/peak, paid cumulative
bytes, work, materialization, capacity, fields and full Drop against a separately
compiled complete frozen sequencing function. It explicitly checks that a
current exact budget can reject the old receiver. This is authored source, not
passed evidence, and it makes no application-fit or stage-share claim.

## Parent union registration

Do not edit the shared inventory carriers in this lane. Register these three
new Rust paths in the union raw inventories:

- `src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs`
- `src/cleanup_plan/replay/skeleton_work/prefix_fusion/before.rs`
- `src/cleanup_plan/replay/skeleton_work/prefix_fusion/tests.rs`

Actual carriers are `tests/hir_module_boundaries.rs`'s `validation_raw` join
(not its sentinel assertion), the nested skeleton-work raw join in that same
file, `tests/workspace/semantic_graph/source_inventory.rs`, and the parent-owned
interop-builder ledger capacity inventory as appropriate to its complete join.
All preexisting raw bytes and assertions must remain present. New files are
below 1500 lines; replay.rs shrinks and its recorded budget is not raised.

`preimages/` contains complete frozen changed-source bytes.
`source-inverses.json` records full preimage/current hashes and exact inverse
fragments. Parent source-binding/role manifests and current-to-historical joins
must be refreshed in the common union, without weakening any original class.

## Pending selectors

After fixing funding and completing union registration, compile and execute:

- `ordinary_prefix_fusion_complete_previous_fields_capacity_work_and_materializations`
- `ordinary_prefix_fusion_whole_owner_removes_actual_backing_and_pays_controls`
- `ordinary_prefix_fusion_original_work_and_sticky_failure_remain_first`
- `ordinary_prefix_fusion_frozen_receiver_is_complete_and_original_calls_survive`
- `ordinary_prefix_fusion_expired_active_builder_and_dag_suffix_keep_old_route`

Keep all final_output, sequence_vector, ordinary_dag/frontier, physical growth,
clone, complete historical oracle and source-custody controls. Broader hostile
metadata/allocator boundary injection and independently opted-account controls
remain required. Replay the parent's unchanged owning application rows under
64 MiB with exact final receipts; row03's ordinary flat growth site is not a
measurement of the full HIR stage. Row07 is a distinct constructor site.
`scripts/quality.sh full` remains pending the parent's authorized grouped gates.
