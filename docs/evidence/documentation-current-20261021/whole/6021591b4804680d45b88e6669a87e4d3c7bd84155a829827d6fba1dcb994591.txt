# CI corrections from actual compiler head 1c87c0f125

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The actual completed Chromium job 117666506981 in run 39124274980 captured both complete compiler graphs successfully before its exact fixture pins failed. The capture includes the exact head, binary SHA-256 and Rust 1.88 Linux provenance. The retained reader validates envelope lengths, canonical base64, SHA-256, the complete recursive graph, key and vector order, and exact whole-wire equality after replacing only graph_digest and budget.used_builder_bytes. The graph digests are independently recomputed using the compiler domain and little-endian payload length. Every other field and byte is identical. The observed calculator bill falls from 12316396 to 10622764 bytes. This is a measured graph-record bill; it does not establish native application fit or a browser runtime pass.

The production help correction restores every qualified usage in the exhaustive catalog while keeping the existing standalone new scoped-help assertion satisfied. The complete catalog matches the unchanged frozen current snapshot exactly. Three tests using the actual full command metadata, real catalog/scoped renderers and actual usage module pass under Clippy 1.97.1 -Dwarnings, peaking at 181472 KiB. The unexecuted JSON-codec supplemental-help dependency is opaque; this is not a full CLI executable qualification.

The stdlib conformance additions directly call five existing helpers that the public-surface audit had not exercised: bounded_valid, quoted_len, line_start, line_end and basic_line_fault. Existing tests remain intact. Cases include invalid/truncated UTF8, exact/short bounds, the existing 65-byte policy refusal, escape quoting, record cuts, empty and invalid lines, and exact 512/513-byte line limits. Actual canonical/compiler/interpreter/native/Wasm execution remains a CI gate.

Three successive Component jobs ended with runner shutdown 143 during compilation, without a runtime verdict. The workflow now serializes Cargo compilation and disables incremental compilation for that job. Its original debug profile, symbols, assertions, tools, locked/offline commands, runtime tests and failure diagnostics remain unchanged. The cause of the runner shutdown is not proven, and the configuration change requires a real CI rerun.

All prior snapshots, exact fixture assertions, hostile controls, quotas and source-authority rules remain. Current full CI, native 64MiB, STD-08, browser runtime and genuine Lean --require-kernel gates are still required. Source-only custody and isolated renderer checks do not substitute for those results.
