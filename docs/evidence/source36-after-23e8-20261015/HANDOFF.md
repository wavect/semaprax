# Source36 current-view adapter after actual 23e8 compiler corrections

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Basis: `2e116e4f243e161dff5a8cbc335ed71418cf74d7`. Source-only work; no Rust compilation or execution, tests, Clippy, registration binary, or global metadata composition.

The initial census evaluated all 34 a/b source comparisons independently and found exactly two drifts: `src/string_ops.rs` and `src/string_ops/conditions.rs`. Their new named views restore complete immutable `23e8f1d0a67cb2657bb4cdc963a2a2828c523c7c` bytes before every existing undo/count/assertion. The combined JSON retains the prior two records exactly and adds only these two records. The actual Rust helper authenticates complete current and original source hashes and the unique complete inverse as before.

The only Rust changes are the two same-line b.rs view prefixes and the current_view.rs manifest literal. All original assertions remain. a.rs is byte-identical and exactly 1500 lines. All three existing source inventories still include the helper exactly once.

`source-inverses.json` contains only the two immediate full Rust inverses to the basis, with full originals under `original/`. The combined JSON was absent at the basis: its complete local addition inverse and the unchanged prior JSON archive live under `metadata-history/`, outside active source rows. No recursive metadata-source row is introduced.

`verify_source36.py` differs from the existing runner by only its explicit combined JSON literal. `verify_adapter.py` checks complete inverses, immutable checkpoints, old records, old runner/proof/archive preservation, and literal Rust adaptations. Both source-only verifiers passed; `source36-result.log` reports all 36 equal, including unchanged paid/key/substitute/storage archive comparisons. This does not qualify any Rust owner or runtime behavior.

Parent should merge this commit before its incremental composition from all 84 prior packet paths. The new packet supplies two active source rows; retain old derived active metadata rows as planned. No global composition was attempted here.
