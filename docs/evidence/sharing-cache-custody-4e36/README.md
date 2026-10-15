# Cache child source custody composition

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The parent cold_source_tests.rs source audit reconstructs its original inline function from the current extracted child before applying its existing hooks. The cache corrective changed that child. One additive exact undo now restores the complete unchanged archived child in the reconstructed parent, before all original parent hooks. Every prior hook retains its literal bytes and relative order; the final complete original parent comparison passes. No archived source changed.

The existing static checker proves all 36 full-source inverses equal. All 65 current nominal live bindings and unchanged original hashes were revalidated; neither cache child is in those 65 rows, so no nominal metadata edits were needed. Both cache corrective source/current/archive/patch SHAs match their existing packet. No Cargo or Rust execution.
