# Ordinary first-sibling prefix fusion candidate (source only)

Base: `fade6f4df00808a4f9b763fff22cd5f3898683f4`. Private source candidate;
no Cargo, compiler, test, benchmark, application replay, graft indexing, or full
quality gate was run. The parent integration checkout was not edited.

## Eligibility funding revision

The initial WIP paid a new probe before checking work/shape, which could change
small/work-short fallback admission. It is superseded by a first-sibling-only
receiver. `pending.next_back()` and consuming the existing pending iterator
replace the old tuple/slice split and new earlier-slice iterator. The unchanged
`FINAL_SPLIT_CONTROLS` bound now covers that smaller receiving frame plus ONE
eligibility probe. `NEW_SPLIT + PROBE <= REMOVED_SPLIT` is a compile-time layout
assertion: on a 64-bit host the roles are 40 + 116 <= 168 bytes.

This does not amortize a fixed fee over repeated per-sibling probes: only the
first earlier sibling can fuse. Subsequent earlier siblings retain the complete
original clone/extension receiver. The last suffix leaves the iterator only
after the original first-output capacity receiver, which still sees the exact
same remaining count. No extra fee is charged on small, empty-suffix, DAG,
selected, oracle or work-short decline. The already successful original clone
work authenticates the actual account before this gate.

This is a source argument awaiting parent review and execution, not a passed
physical-accounting result. The conservative new fusion charge still requires
removed clone backing to cover the whole charge; there is no claim that the
exact row03 shape qualifies.

## Candidate's physical boundary

Only the first ordinary ActiveBuilder flat-prefix/nonempty-flat-suffix earlier sibling
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
(not its sentinel assertion), `tests/workspace/semantic_graph/source_inventory.rs`,
and `crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs`.
The existing nested skeleton-work join is a natural local context in the HIR
carrier; preserve every existing occurrence and the sentinel exactly.
All preexisting raw bytes and assertions must remain present. New files are
below 1500 lines; replay.rs shrinks and its recorded budget is not raised.

`preimages/` contains complete frozen changed-source bytes.
`source-inverses.json` records full preimage/current hashes and exact inverse
fragments. Parent source-binding/role manifests and current-to-historical joins
must be refreshed in the common union, without weakening any original class.

## Pending selectors

After parent review and completing union registration, compile and execute:

- `ordinary_prefix_fusion_complete_previous_fields_capacity_work_and_materializations`
- `ordinary_prefix_fusion_whole_owner_removes_actual_backing_and_pays_controls`
- `ordinary_prefix_fusion_original_work_and_sticky_failure_remain_first`
- `ordinary_prefix_fusion_frozen_receiver_is_complete_and_original_calls_survive`
- `ordinary_prefix_fusion_expired_active_builder_and_dag_suffix_keep_old_route`
- `ordinary_prefix_fusion_independent_oracle_keeps_complete_original_account`

Keep all final_output, sequence_vector, ordinary_dag/frontier, physical growth,
clone, complete historical oracle and source-custody controls. Broader hostile
metadata/allocator boundary injection and independently opted-account controls
remain required. Replay the parent's unchanged owning application rows under
64 MiB with exact final receipts; row03's ordinary flat growth site is not a
measurement of the full HIR stage. Row07 is a distinct constructor site.
Only affected exact owners and scoped production Clippy remain pending the parent's grouped qualification; no full suite or full quality gate is authorized.
