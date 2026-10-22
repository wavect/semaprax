# Final source-only composition on e30e017

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Immutable source basis: `e30e017e503f8e0362a186ada126c48e6199d4d7`.

The metadata-only composition covers all previous 68 packets and 666 rows, plus sixteen packets containing 108 rows: declaration rewrites, registry census scratch, OPT836 frontend/interpreter/backends/controls, precise lint and receiver corrections, the four region inverses, scalar guard views, source36 views/immediate originals, formatter correction, and the two documentation inverse owners. Total: 84 packets, 774 rows. Existing hooks and complete original fields are retained; 195 complete current-to-immutable-checkpoint inverses are additive. Independent joined verification authenticates 191 added checkpoints in existing packet files; the remaining four are in newly derived manifest packets.

The new compositor uses a persistent Git batch process with an LRU bounded to 16 MiB and 128 entries, stages one packet at a time on the same SSD, and retains source/archive hash tables rather than all source bodies. Measured cache peak: 16,776,186 bytes. All Rust, production and test source stays byte-exact to the source basis. No old scripts, proofs, original archives or original hooks were rewritten. The proof records 1,530 preserved historical proof/script/archive identities and 418 current source identities.

The complete 36-owner source-only reconstruction uses the explicitly adapted current-view runner. It executes every original undo and assertion and restores every complete original archive. The two actual Rust owners use the same authenticated view. The old source36 runner and its old evidence remain untouched.

Separate joined verification preserves 2,951 old hooks, verifies 53 independent receivers unchanged, retains both actual #834 capacity bounds plus all three independent capacity/registry modules, and proves both full #837 patch scopes independently. The archived files called `immediate-source-inverse.patch` actually contain forward diffs; applying those exact archived bytes in reverse restores all 32 and 6 patch source paths, including the newly added file deletions. Complete current-to-authored source joins for all 38 paths are authenticated separately. This is source custody, not runtime evidence.

All 30 declaration merge inputs retain their exact compressed/raw SHA256 and original Git blob IDs; the existing bounded streaming verifier was rerun without changing its archived proof. Raw payload size: 22,952,096 bytes.

Run these exact source-only checks from repository root:

```
python3 docs/evidence/final-source-composition-e30e017-20261015/compose.py --verify-only
python3 docs/evidence/final-source-composition-e30e017-20261015/verify_joined.py
python3 docs/evidence/declaration-825-integration-inputs-20261015/verify_archives.py
```

All passed. `compose.py` without `--verify-only` regenerates metadata from the immutable basis; it does not run Rust. No Cargo/build/Rust test/Clippy/Claude execution occurred. All runtime, physical measurement, pin acceptance, push and issue closure qualification remains with the parent. #781 remains open for its separate residual children.

Minimum additional affected runtime owners, beyond the parent's retained selection:

- `semaprax --lib`: `cleanup_plan::replay::live_oracle::source_custody::a::complete_original_sources_and_compiled_owning_receivers_remain_exact`
- `semaprax --lib`: `cleanup_plan::replay::live_oracle::source_custody::b::complete_original_sources_and_compiled_owning_receivers_remain_exact`
- `semaprax --test hir_module_boundaries`: `core_validation_has_no_resolution_cleanup_build_or_physical_authority`
- `semaprax-native-rust-interop --lib`: `implementation::tests::ledger_capacity::hir_capacity_layout_constants_are_bound_to_root_const_assertions`
- `semaprax --test workspace`: `semantic_graph::expected_projection_source_boundary_is_pure_and_keeps_shared_helpers_in_root`

The native helper joins `loan_cfg_source_inventory!` → `hir_resolver_source_inventory!` → that single layout-pin consumer. The workspace helper joins the root inventory, preserving the private projection's closed facade inventory.
