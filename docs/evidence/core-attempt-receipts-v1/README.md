# Core retry attempt receipts

Status: opt-in test instrumentation; current-head runtime diagnosis pending.

Audience: compiler contributors and CI reviewers.

The completed 3e66c0116 macOS heavy job 117406158240 passed its four kernel differential cases and 78 of 81 workspace graph cases. The original identity retry now builds successfully without G410 or a replay-work refusal, but its unchanged assertion observes five actual core attempts where two are required. Catalog and pattern fixtures retain their G171 byte-budget failures. This is prior-head evidence, not qualification of a later commit.

The original identity test now explicitly observes its existing maximum-budget build. Each unchanged retry-loop attempt can emit its mode, prebound, consumed bytes, overflow flag, refusal, site and original errors after the existing bounded attempt ends. All values are borrowed. The observer is off by default, current-thread scoped, nestable and restored on unwind. It does not consume stage traces, change the counter, mutate a budget, retry, publish or grant authority. The production build has no observer call.

`sources.json` authenticates five complete predecessors, including the empty predecessor for the new test helper. Removing the cfg(test) root hook restores every production source byte. Removing the test-module prefix and unwrapping only the observed call restores all original test bytes, including the exact two-attempt assertion. The prior 27 source-controller bodies remain a complete prefix. Hash and source-tamper controls fail closed.

`component-checks.json` covers Rust 1.88 and 1.98 executions of the complete real helper and all three owning assertions, plus the complete actual root receipt expression with explicit scalar/result/site adapters. Nested state, default silence, borrowed argument formatting, result identity and unwind restoration are exercised. This component does not execute the full production core retries. The unchanged original runtime assertion, limits, fixtures, diagnostics and all quality gates remain required.
