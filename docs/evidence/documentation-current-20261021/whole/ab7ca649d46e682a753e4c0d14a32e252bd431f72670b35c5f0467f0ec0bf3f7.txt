# Canonical merge resolution candidates

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only external candidates for the in-progress 6e549d1ae +
88884d148d9c148eed61bc3265eed955edf19346 merge. No ROOT source, index,
refs or commits were changed. No Rust/Cargo/test/Clippy/Node execution.

- canonical_shared.rs: exact stage-2 source. This is a reviewed result, not a
  blanket ours selection: main changes only registration order and continuation
  formatting of the old insertion call. Current registrations already include
  that order plus all complete old receivers and838/839 helpers. Keep test-only
  BTree epsilon_alias, production ordered_alias/ordered_weights, RetainedMemo,
  insert_retained, explicit imports and all retained scratch cleanup/payment.
- before_alias.rs: incorporate all main formatting, including the full old
  producer insertion and adapter formatting. Keep same_closure_state because the
  integrated epsilon caller uses it. The main method spelling same_snapshot was
  already present in stage1; it is not an additional semantic fix. Method body
  compares the same entire ID/state maps. No extra wrapper or duplicate method.
- epsilon_alias/tests.rs: incorporate all main body/import formatting. Retain the
  explicit Automaton/Label/SkeletonTerminal/work-limit imports because the current
  parent epsilon helper no longer provides those through its old wildcard. Keep
  the allocation-free ordered/BTree result/view comparisons and the matching
  same_closure_state call, plus every work/physical/foreign/Drop assertion.

Static assertions checked conflict counts, full-source stage hashes, exact
canonical stage2 identity, the full explicit before_alias formatting transform,
and unchanged epsilon body tokens versus stage2. Per-file diffs and SHA bindings
are adjacent. This is not compiler or runtime qualification.

Nonconflicted interactions observed in the same subtree:

- before_index.rs loses one trailing blank line from main.
- memo/tests.rs receives only assert expression formatting from main.
- Main epsilon caller uses same_snapshot; merged caller must keep the integrated
  same_closure_state pair, as these candidates do.
- Complete historical source archives and old proof bodies must stay immutable.
  The before_alias/before_index/epsilon-test formatting changes need authenticated
  complete current-to-prior joins in the next metadata composition; do not label
  their current bytes unchanged or rewrite old hashes to conceal the drift.
- No main algorithmic/CI fix in these three files is erased: stage1-to-stage3
  changes are formatting/registration only. Other main CI fixes outside this
  assigned subtree were not resolved or overwritten by this work.
