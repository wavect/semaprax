# Current-cutoff OPT838 whole-input adapter

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Basis:2b5cae170e13b16c4a5f57a619b302405e2934a6. Immutable expected source checkpoint:40f77262950e9dc4bb1da6626f441f2f504cfe59. External source-only candidate; no ROOT edits or Cargo/tests/build/Clippy/Node/Claude.

Exactly four distinct source inputs drifted: HIR module boundary inventory, semantic_graph source_inventory, epsilon_alias/tests.rs and ARCHITECTURE.md. Runtime-projections.json binds each full current input to complete immutable40f bytes, with both hashes and full inverse. It is a protected runtime input and MUST NOT be physically recomposed or have its current hash rebound to an earlier domain. Keep local addition history separate from active physical rows.

Only the existing amortized_source.rs changes; no new Rust module or audit include. checkpoint() first calls current40f, then runs the original match and old patch. Its seven-bind macro first preprojects and borrows the resulting full source before the exact two original assertions. All14 old bind literals,28 old patches/archives, all old helper interiors and source.rs remain exact. The immediate complete helper original/inverse is in source-inverses.json (one active Rust row), and manifest.json identifies the final focused-rustfmt candidate bytes.

verify_adapter.py mirrors actual Rust source::apply, authenticates four runtime views, executes all seven amortized forward/inverse pairs and all seven original forward/inverse pairs through the actual two-hop checkpoint, verifies28 old patch/archive files against40f, reconstructs the entire previous helper after removing only the explicit adapter, and verifies the immediate source inverse. All passed. The inspected839 hardcoded inputs (scratch_tests, liveMemo, before_scratch, before_scratch_memo and both immutable originals) equal40f; no speculative839 adapter is added. Runtime Rust assertions remain unexecuted.

Parent applies src/ and docs/ from this candidate, commits the test-only correction, then reruns bounded global metadata composition on that new basis. New packet list needs source-inverses.json as one additional active source packet. runtime-projections.json must enter protected-input preservation/verification, not ordinary global physical composition rows; its complete local addition inverse stays under metadata-history. Existing two838 source test selectors exercise the new projection; no new selector is introduced.
