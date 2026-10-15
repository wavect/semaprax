# Corrections found in the RAM-interrupted 1ef5 compilation

The report move test keeps both returned Vec owners alive through their original pointer-identity assertions. This removes two dangling temporary pointer warnings while retaining every prior assertion and both complete receivers. The test-only scope helper is now used, so its unfulfilled dead-code expectation is removed; its body, calls and checks remain exact. Full immediate sources and exact inverses are retained.

Source-only correction; no build, test, Clippy or benchmark executed. The original 1ef5 warning log and resource-interruption receipt remain unchanged. Independent legacy receiver dead-code warnings are not blanket-suppressed.
