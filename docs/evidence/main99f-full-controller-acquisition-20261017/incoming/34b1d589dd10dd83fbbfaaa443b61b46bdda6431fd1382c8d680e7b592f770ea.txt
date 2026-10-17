# Separate reference merger helper names

Status: focused compile-collision repair; full current-head hosted qualification pending.

Audience: compiler contributors and CI reviewers.

The f247b8e5b macOS and Windows build jobs and Lean prerequisite reported E0592: both the older tail-move oracle and the isolated-path reference declared `saved_merged_paths` on `SkeletonWork`. Inherent names must be unique even when their definitions are in different test modules. The earlier component omitted the older implementation and therefore missed that collision.

The older helper is now named `tail_merge_control`, and its four callers use that name. Both complete method bodies and every original assertion remain compiled. The identifier has the same length, so no formatting or literal-content changes accompany the rename. Production behavior, counters, work, byte quotas, diagnostics, fixture values and the isolated-path reference are unchanged.

`sources.json` authenticates three complete predecessors. The source controller proves that replacing only the new identifier restores every old implementation and caller byte, retains all prior source-controller bodies as a complete prefix, and rejects source or digest tampering.

`component-checks.json` compiles BOTH complete actual reference merger methods on the SAME Work type under Rust 1.88 and 1.98, then runs the three complete isolated-path assertion bodies independently under the unchanged 200 MiB process-tree cap. Actual budget, BO, Diagnostic and allocator observation are used; thin HIR/constructor and panic-if-reached unrelated union/oracle adapters remain explicit. The older nonisolated tail/DAG assertions and full compiler are hosted requirements. `collision-check.json` records that restoring the duplicate identifier reproduces E0592 in that combined capsule. This is focused evidence, not all-CI or Lean qualification.
