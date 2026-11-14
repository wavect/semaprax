# OPT #935 secondary byte-traversal replay

Status: Focused validator and cross-backend traversal checks passed locally. Full qualification is pending.

Audience: compiler contributors and performance reviewers.

The focused Rust library tests ran on macOS arm64 with Rust 1.97.1 from compiler source `0a68f5420260999ef7e57a1a3641e41a69a95a4a`. The secondary receiver and its test source are byte-identical at `b2e7b5aaf`. All ten tests under `hir::validation::byte_traversal::tests` passed, including the complete predecessor source inverse, hostile region and receiver cases, depth and late error order, and exact, short, zero, positive-floor, sticky, and Drop controls.

The mixed fixture resolves and validates a program containing a partial byte receiver, scalar and fallback roots, contracts, and a generic instance. The allocation observer then runs the saved and current secondary receivers over the same resolved functions and instance.

| Observation | Saved receiver | Current receiver |
| --- | ---: | ---: |
| Walk setups | 13 | 8 |
| Scalar selector invocations | 0 | 11 |
| Cumulative reservation in the observed window (bytes) | 0 | 4,455 |
| Heap requested in the observed window (bytes) | 0 | 0 |
| Heap live on return in the observed window (bytes) | 0 | 0 |

The skipped setups cover two contracts and three scalar function bodies. The selector consumes 405 reservation bytes per reached root, including roots that still use the walk. This fixture measured setup counts and reservations.

The nine `useful_data::byte_slice_for_runtime` tests also passed on the retained `c28cc443292fb5afb4e29f8db468d1b8b7336049` build. They exercise the interpreter, native C11, and Core Wasm, including nested traversal, failure cleanup, and the no-buffer check. The traversal receiver, its children, and those integration test sources match the recorded main checkout byte for byte. Full current-head compiler quality, CPU time, and application admission remain unqualified for [#935](https://github.com/wavect/semaprax/issues/935).

## Full local quality attempt

The full quality route on macOS arm64 used source `6f1db7ec4979ebeeaca194aa2f7e51ec0ded9117`, Rust 1.97.1, one Cargo build job, four test threads and a 16 MiB Rust test stack. Formatting, workspace checking, all 117 executed advisory tests and workspace Clippy passed. The 11,624-test workspace library run reported failures before completion. In a separate exact single-test run of `native_multi_owner_cleanup_order_is_physical_and_hostile_mutants_reject`, the native process reached its two-second deadline and returned `SPX-G570` at `native_executor.process.deadline`. This reproduces one failure independently of the parallel test runner. Other reported failures require their own diagnosis. The full gate remains outstanding.

Two earlier full runs, sampled after 15 and 18 hours respectively, were both executing `latest_main_and_afa_complete_views_preserve_all_prior_controls_and_refuse_wrong_stages`. The samples placed active work in the source audit's packet composition, compact digest preflight, held reads and JSON teardown. The current checked Python controller composes 1,088 rows with 5,209 prior views. The Rust audit's per-view body makes three file and seven value requests, for 52,092 requests including the two trailing refusal probes. Its public wrappers may construct a new projection invocation on each request after checking declared history. That count describes the audit loop, while the samples identify operations actually reached. A timing comparison would need to preserve the separate file, compiled, archive and hostile-input checks.

### Historical audit decoder measurement

A standalone Rust 1.97.1 O0 probe extracted the exact compact digest validation and decoding loops, replacing the JSON accessor with its borrowed text result. It compared the original alphabet searches with a one-pass range decoder over all 791 digest texts from the checked V22 page (SHA-256 `02ae69914ffee2ec8a02349e260962bda9d0cadba920a10df1139030c78fc59a`). Both arms returned identical arrays and preserved the digest-spelling error for every ASCII substitution at each of the 43 positions, including nonzero pad bits, plus selected malformed lengths and Unicode.

Each timed arm decoded 949,200 texts in the same process. The original loop took 7.995 and 8.319 seconds. The range decoder took 0.860 and 1.104 seconds. This measures one operation sampled in the audit. The full audit also authenticates live files, checks bounds and composes metadata. Its end-to-end time and refusal order still need to be checked before adopting a change.
