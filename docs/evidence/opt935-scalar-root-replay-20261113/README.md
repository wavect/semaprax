# OPT #935 secondary byte-traversal replay

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
