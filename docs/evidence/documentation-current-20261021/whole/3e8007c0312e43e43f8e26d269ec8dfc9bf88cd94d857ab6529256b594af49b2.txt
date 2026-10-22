# Actual focused qualification at 2e92f64f6

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

These are completed executions of source `2e92f64f640fe7f4495b3fffe6a17c5575e9f965`, tree `a80b60c7350b8a83e82253175503d4fdd926aedd`, on aarch64 macOS with Rust 1.97.1. They are prior-source evidence after subsequent changes, not current-head qualification.

All six selected build commands passed and produced nine new artifacts. Exactly 128 registered tests ran: 94 passed, 34 failed. No full test suite or quality gate ran. Source-bound receipts retain exact commands, artifact hashes, log hashes, resource observations and settled process groups. The single separate native-disposal test and the single missing #887 helper control passed.

Both scoped Clippy commands failed solely on the unused `validate_status_sources_certified` import in `expression_facts/receiving.rs`. The next repair batch must resolve that failure and qualify the changed source. Tests must not be represented as an overall passing gate. `summary.json` lists all 34 failures.

All eight #890 validation/resource owners (indices 73–80) passed: ordinary emission equivalence, hostile-HIR ordering, H006 pre-ABI refusal, duplicate-debit removal, receipt validation, complete resource scaffold, mutation invalidation, and the separately selected resource route. Their full logs are retained here. Clippy remains outstanding before issue closure.

The actual DeclarationId layout changed from 24 to 16 bytes, while Option<DeclarationId> remained 24 bytes. The complete core validator frame changed from 248 to 240 bytes; its existing 248-byte tariff remains intact. These are target-specific compiled layouts, not aggregate allocation or CPU savings.

The same twelve input files, in the same emitted order and with exact byte hashes, produced complete mode-6 static forecast pass 1 totals of 51,988,568 bytes at source 530073a5 and 50,515,936 bytes at 2e92f64f6. Both executions failed application admission at the unchanged 64 MiB cumulative live ledger. This static-forecast comparison cannot establish application fit, physical peak memory, billed agent savings, or sole causality from #891. Full forecast logs, completeness flags and matched pass records are retained.

These executions are compiler qualification, not live agent benchmarks. Turns, net input tokens, authored tokens, estimated billed cost per accepted task, wall time per accepted application, and a SEMAPRAX/TypeScript advantage have not been established by this round.
