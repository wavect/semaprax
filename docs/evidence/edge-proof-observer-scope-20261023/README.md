# Explicit heap-observer capture for byte-traversal refusals

Status: focused local verification; hosted CI remains incomplete.
Audience: compiler maintainers and CI reviewers.

The original edge-proof exact/one-short test on main `2e6c80dae96ee1c440420a8c7844d2e331a02df5` reproduced the hosted panic in `current_metrics`: the byte-traversal test-only refusal hook tried to read metrics during an ordinary bounded call with no observation. The complete native compiler test build retains genuine IR and layout; only unrelated test registration attributes are disabled for this focused run.

The hook now reads metrics only while a `FailureCapture` guard is armed inside the existing all-frontier heap observation. Arming still calls the strict observer, records the first failure, and unwinding disarms capture before the observer is dropped. All existing exact/one-short/zero, sticky refusal, live allocation, free balance, and no-allocation-after-failure assertions remain. Added controls exercise ordinary refusals with no observer and with an unrelated observer, plus panic unwinding and rearming under a real observation.

No production compiler, budget, allocation observer API, fixture, or diagnostic is changed. The original edge test body is unchanged. This evidence qualifies focused test instrumentation and source integrity; it does not claim the complete main CI is green.

Focused verification: 19 native tests passed (one original edge reproducer, all 14 nested byte-traversal tests, four original source-custody controls). The earlier focused 24-registration test Clippy check passed with the repository lint levels; private registration-only unused-macro and test-item-order warnings are excluded from that focused lint acquisition. The expanded 39-registration native compile reached a minimum of 3.85 GiB actual available device RAM, with no per-process memory limit and no pauses. Unrelated registered tests were filtered rather than claimed as executed.

Source publication: the unchanged authority verifier and all six original hostile controls passed. The refreshed HIR and projection manifests preserve every old source role, ordered chunk, and registration chain, and are published with the current boundary domains. All 32 overlay tests and both complete predecessor plus both current-entry tests passed.

All 11 original HIR boundary gates, the original full projection source-boundary gate, and HIR metadata Clippy passed.
