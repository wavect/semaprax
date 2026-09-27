# Task equivalence: `concurrent-delta-merge-v1`

This held-out task is this suite's answer to issue #211's "concurrent
change" category (see `docs/METHODOLOGY.md`'s "Taxonomy mapping" section for
how this corpus's eleven named categories map onto its tasks). It measures
whether a candidate merges two independently-arriving updates against a
shared bounded resource *concurrently* — computed from the same starting
`base` and combined once — rather than *sequentially*, folding one delta's
clamp into the input the other delta sees. Sequential folding is the classic
concurrency-safety defect this task is built to catch: two updates that
arrive at the same time and are individually valid should not lose value to
an intermediate clamp neither update alone would have triggered.

## Problem and oracle

`merge_concurrent_deltas(base, delta_a, delta_b)` models two concurrent
updates, `delta_a` and `delta_b`, both computed independently against a
shared counter's current value `base` (for example, two independent
reservation systems each debiting or crediting the same pooled inventory
count in the same instant). The counter is bounded to `[0, 1_000_000]`. The
required merge is:

```
clamp(base + delta_a + delta_b, 0, 1_000_000)
```

A plausible wrong candidate treats the two updates as if one arrived after
the other — applying `delta_a` to `base` and clamping immediately, then
applying `delta_b` to that already-clamped result and clamping again:

```
clamp(clamp(base + delta_a, 0, 1_000_000) + delta_b, 0, 1_000_000)
```

This sequential form agrees with the correct concurrent merge whenever the
intermediate sum `base + delta_a` never itself leaves `[0, 1_000_000]` — which
every public vector arranges. It disagrees exactly when one delta alone would
push the intermediate result to a bound that the *other* delta would have
pulled it back from, had both been considered together: a real value is
silently lost to a clamp that a correct, order-independent merge never
needed to apply.

## Inputs and outputs

- **Inputs**: three signed integers (`base`, `delta_a`, `delta_b`), literal
  fixtures in every vector, within the common exact range of Rust `i64`,
  SEMAPRAX `i64`, and JavaScript's safe-integer range.
- **Output**: one signed integer, the merged and bounded counter value,
  exact equality.
- **Boundary of the measured region**: the pure `merge_concurrent_deltas` /
  `mergeConcurrentDeltas` function, invoked by each language's own test
  runner exactly as every other task in this corpus is. Compiler/interpreter
  startup and the test harness's own overhead are outside the comparison.
- **Allowed optimizations**: ordinary arithmetic and comparisons only. No
  saturating-arithmetic standard-library call, and no reordering that would
  make the merge dependent on which delta is "applied first" — the whole
  point of the task is that neither delta may see the other's clamp.

## Vectors

Public vectors (`merge_concurrent_deltas(100, 50, -30) == 120`,
`merge_concurrent_deltas(500_000, 100, 100) == 500_200`,
`merge_concurrent_deltas(10, -5, -3) == 2`,
`merge_concurrent_deltas(0, 0, 0) == 0`) never push an intermediate
one-delta-at-a-time sum outside `[0, 1_000_000]`, so the sequential-clamp bug
is invisible until the hidden phase.

Hidden vectors deliberately place `base` near a bound with one delta that
would cross it alone and a second delta that pulls the true sum back inside
range:

- `merge_concurrent_deltas(999_990, 20, -50) == 999_960` — the sequential
  form computes `clamp(999_990 + 20) = 1_000_000`, then
  `1_000_000 - 50 = 999_950`, ten short of the correct `999_960`.
- `merge_concurrent_deltas(10, -20, 15) == 5` — the sequential form computes
  `clamp(10 - 20) = 0`, then `0 + 15 = 15`, ten over the correct `5`.

Both were hand-verified against the exact wrong-candidate formula above
before being committed (see the harness evidence cited in this task's commit
message).

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports each keep
`merge_concurrent_deltas` in its own candidate module. Public and hidden
phases share that module unchanged; the hidden overlay replaces only the
assertion entry point (`main.rs` / `index.ts` / `src/app.spx`), the same
overlay style `cold-chain-release-gate-v1` and `stable-dispatch-order-v1`
use.

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

## Split and negative control

This task is `split: "held_out"` in `tasks.json` and shares no fixture,
vector, or task family with any development or validation task in this
suite. Its wrong-candidate control was verified by hand against all three
real toolchains while authoring this task: the sequential-clamp candidate
above passes every public vector and fails both hidden vectors with the
exact divergent values named above, in Rust (`test_bin`, hidden exit `101`
with `left: 999950, right: 999960` and `left: 15, right: 5`), TypeScript
(`node` throws `expected 999960, got 999950`), and SEMAPRAX (`run .` prints
`0` for the public phase and `1` for the hidden phase). No trial,
correctness score, or repair transcript for this task may be exported for
training or tuning reuse while it remains held out, for the same reason
`bounded-counter-repair-v1/EQUIVALENCE.md` states for its own held-out
declaration.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. C, Python, and Java keep
the same candidate/entry split the Rust and TypeScript ports use
(`candidate.c`/`candidate.py`/`Candidate.java` hold the unchanged
`merge_concurrent_deltas` function; the hidden overlay replaces only
`main.c`/`digest.py`/`Main.java`, via `#include`, `import`, and javac's
same-directory auto-discovery respectively). Swift's fixed single-file
`swiftc main.swift` invocation admits no such split, so
`mergeConcurrentDeltas` is repeated verbatim in both the public and hidden
`main.swift`.

Each port was authored independently against this file's clamp-once
concurrent-merge contract and the Rust/TypeScript references, not
transliterated line-by-line, and was independently compiled/run against
the public and hidden vectors above, then checked against this file's own
sequential-clamp negative control (`clamp(clamp(base + delta_a) +
delta_b)` in place of `clamp(base + delta_a + delta_b)`): the mutant
passes every public vector and fails both hidden vectors with the exact
divergent values this file names above (`999950`/`15` instead of the
correct `999960`/`5`) in all four languages, confirming the hidden vectors
are non-vacuous.
