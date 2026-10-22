# Authenticated current views for the original 36 source guards

Source basis: `94c392e9bd8b60edba497263d0267ec54a4dd728`.

The replay input acquired two formatting changes and the authenticated vector-parameter receiver in OPT836. Its original source guard still expected the complete pre-change spelling. The new test-only helper authenticates the complete current file and projects it to the immutable complete e3eb checkpoint before every existing original undo/assertion executes. Adding the helper to the HIR source inventory also requires the same complete view from the amended inventory to its immutable 94c checkpoint. These are two named views, with no fallback or ignored mismatch.

Both actual Rust source-custody owners invoke the helper. All old undo, count, assertion and archive bytes remain intact. `a.rs` stays exactly 1500 lines. The helper joins the HIR, native and semantic-graph root inventories. Seven immediate whole-source inverses preserve the full additional test-only edit; the helper's immediate original is empty and authenticated as absent at the source basis.

Run from repository root:

```
python3 docs/evidence/source36-current-view-adapter-94c392-20261015/verify_adapter.py
python3 docs/evidence/source36-current-view-adapter-94c392-20261015/verify_source36.py
```

Both ran successfully as source-only checks. The latter reconstructs all 36 complete original input sources and the unchanged paid/key/substitute/storage archives. The old source36 runner and proof remain byte-exact. No Rust compilation or execution occurred. The actual Rust source-custody owning selector still requires parent qualification. Global composition must retain the old script and use this explicitly adapted runner.
