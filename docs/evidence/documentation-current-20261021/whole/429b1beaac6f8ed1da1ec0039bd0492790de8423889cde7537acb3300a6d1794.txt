# PLAN test type fixes after actual consolidated compilation

Base: `7827d3687da21e1093d40f8c8e3f7116edba1993`.

The branch probes inherited the historical `before::plan_skeleton_paths` through `use super::*`. Qualifying all three newly added calls as `super::super::plan_skeleton_paths` selects the actual replay producer, including the generic receiving-frontier probe. The independent original producer and complete cross-generation content comparator remain unchanged. The original 204-line test prefix is byte exact. All work, physical allocation, complete Diagnostic, sticky failure, floor and Drop controls remain unchanged.

The PLAN physical probe now accepts the `usize` returned by `reserve_program_skeleton_work`; the returned slot count is intentionally unused. The reservation still completes before producer entry, and failure still short-circuits through the same Result chain.

`whole-inverses.json` retains both complete before/after bodies and hashes; the adjacent before archives are exact Git base bytes. `verify-source.py` checks the four substitutions are the entire source change, preserves the original calls/assertions, checks the actual signature relationships and source-domain intersections, and reports source-only evidence. No Cargo, Rust compiler, build or runtime was executed in this lane. Compilation success is pending the parent consolidated build.

## Custody impact for the parent's cohesive generation

Only two existing test modules change; no new Rust owner or harness is introduced. `src/cleanup_plan/replay/branch_clone/tests.rs` intersects the MAIN090 174 physical domain. Its new bytes require a finite successor/current acquisition stage retaining the old bound manifest and immutable historical packet; this patch does not retarget or rerun the one-shot binder. The historical complete source inverses in the MAIN090 branch-composition and probe-frontier packets remain untouched.

`src/cleanup_plan/replay/ordinary_plan_history/tests.rs` belongs to the nine PLAN owners and does not intersect the MAIN090 physical domain. Parent current owner/common source fingerprints must cover both new test hashes. Neither edited file intersects the six FINAL PLAN literal roots; replay.rs, all three raw source inventories and frozen historical manifests remain untouched. No full source rebind was performed, as requested.
