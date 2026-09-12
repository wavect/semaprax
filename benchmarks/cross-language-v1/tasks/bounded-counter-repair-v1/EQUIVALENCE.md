# Task equivalence: `bounded-counter-repair-v1`

This is the fairness contract every language implementation of this task
must meet, so a future reader can check a comparison rather than take it on
faith (`docs/METHODOLOGY.md` in this suite owns the general contract; this
file owns this one task's specific choices).

This is a **held-out** task (`tasks.json`'s `split: "held_out"`): it exists
to measure realistic bug repair rather than greenfield authoring, and it is
excluded from any development or tuning use — see "Held-out discipline"
below.

## Problem

A saturating counter, bounded to `[0, 100]`, that applies five signed
64-bit deltas to a starting value **in sequence**, clamping the running
value back into bounds after every individual step rather than only once at
the end of the sequence. The task is named for the repair bug it is built
to catch: an implementation that clamps the *final summed delta* instead of
clamping the *running counter after each step* silently disagrees with the
correct implementation whenever the sequence would push the counter out of
bounds and then move it back the other way. Both strategies agree on every
sequence that never leaves bounds, or leaves bounds only at the very end —
which is exactly why the public tests below cannot catch it and the hidden
tests exist.

Three pure functions:

- `clamp(value)`: `value` if `0 <= value <= 100`, else the nearer of `0` or
  `100`.
- `step(counter, delta)`: `clamp(counter + delta)`.
- `apply5(c0, d1, d2, d3, d4, d5)`: `step` applied five times in order,
  starting from `c0`.

## Inputs

Exactly six signed 64-bit integers per test vector (`c0` and five deltas),
supplied as literal arguments in test vectors baked into the source (no
file, stdin, environment, or network input) — the same input discipline as
`sequence-digest-v1`.

## Outputs

One signed 64-bit value: the counter after all five steps. The harness
checks it through each language's own test assertions, not a shared
serialization format.

## Boundary of the measured region

The measured region is exactly `apply5` (and the `clamp`/`step` helpers it
calls), invoked in-process by the language's own official test runner.
Process startup, compiler invocation, and test-harness overhead are outside
the boundary and are never scored in this schema version (which does not
measure wall-clock time at all; see `docs/METHODOLOGY.md`).

## Allowed optimizations and required equivalence

- Each language uses its own idiomatic control flow for `clamp`: the Rust
  and TypeScript references use `if`/`else if`/`else`; the SEMAPRAX
  reference nests two single-condition `if`/`else` expressions instead,
  because only that shape is demonstrated working in this suite's existing
  `sequence-digest-v1` fixture (see that task's `EQUIVALENCE.md` for the
  `SPX-H006` cleanup-replay-budget note this suite has already hit once on
  branching shape). This is a per-language idiom difference, not a
  difference in what is computed.
- No implementation may special-case the specific vectors used by the
  public tests, and no implementation may import a language-specific
  "clamp"/"saturating arithmetic" library helper that would substitute for
  the algorithm; `clamp` must be authored, not delegated (the same
  convention `sequence-digest-v1` follows, enforced by author discipline and
  review, not mechanically — see `docs/METHODOLOGY.md`'s "What the harness
  cannot yet check").
- Overflow: every intermediate `counter + delta` in every vector (public and
  hidden) stays well inside `i64`/`number` range before clamping, so no
  implementation needs an integer-overflow policy to pass.

## Officially supported toolchain and success signal per language

Identical toolchain invocations and success signals to `sequence-digest-v1`
(see that task's `EQUIVALENCE.md` for the two recorded boundary choices:
bare `rustc --test` over `cargo test`, and a local `assertEqual` helper over
`node:assert`):

| Language | Official invocation | Success signal |
| --- | --- | --- |
| SEMAPRAX | `semaprax run digest.spx` | prints `app.main`'s `i64` result to stdout; `0` means every check passed |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin` then `./test_bin` | process exit code (`0` = all `#[test]` fns passed) |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts` then `node index.js` | process exit code (`0` = no uncaught assertion `Error`) |

## Public vectors (do not expose the repair bug)

| `c0` | deltas | correct `apply5` |
| --- | --- | --- |
| `0` | `10,10,10,10,10` | `50` (never leaves bounds) |
| `50` | `10,-5,10,-5,10` | `70` (never leaves bounds) |
| `95` | `10,0,0,0,0` | `100` (clamps once, at the very end of the useful deltas; a naive end-only clamp agrees) |
| `5` | `-10,0,0,0,0` | `0` (clamps once, at the very end of the useful deltas; a naive end-only clamp agrees) |

## Hidden vectors (only a correctly-stepwise implementation passes both)

`hidden/<language>/` holds a second copy of each source file that overlays
(replaces, same relative path) the public one for scoring only. It adds two
vectors that push the counter out of bounds and then move it back the other
way — the case a naive "clamp the final sum once" repair gets wrong:

- `c0=90`, deltas `50,-30,0,0,0`: stepwise clamps `90+50` to `100`, then
  applies `-30` to reach `70`. A single end clamp instead sums
  `90+50-30=110` and clamps once to `100` — wrong.
- `c0=5`, deltas `-20,50,0,0,0`: stepwise clamps `5-20` to `0`, then applies
  `+50` to reach `50`. A single end clamp instead sums `5-20+50=35`, which
  never needs clamping at all — also wrong, in the opposite direction.

The harness never copies `hidden/` into the tree used for the public build
step (see `docs/METHODOLOGY.md`'s leak-check description); the leak check
and the hidden-isolation self-test in
`tests/documentation/cross_language_benchmark_suite.rs` cover this task the
same way they cover `sequence-digest-v1`.

## Held-out discipline

This task's `tasks.json` entry declares `"split": "held_out"`. That
declaration means, concretely:

- Its hidden vectors (above) are not to be quoted, paraphrased, or used as a
  worked example anywhere a future contributor tunes SEMAPRAX's own
  admitted-feature set or a benchmark adapter against — the same
  discipline `src/agent_trajectory_export.rs` enforces mechanically for
  trajectory export (`SPX-G610`–`SPX-G612`: a held-out task, its exact
  denylisted digest, or anything sharing its `task_family`, is refused
  regardless of how the individual record is labelled).
- It carries its own `task_family` distinct from `sequence-digest-v1`'s
  (`bounded-counter-repair-v1` vs. `sequence-digest-v1` at the task-id
  level, since this suite's task inventory has no separate family field yet
  — see `docs/METHODOLOGY.md` for what a future schema version would need
  to add a real many-tasks-per-family axis), so a future near-duplicate
  variant of this exact fixture must be added to the same split, not
  independently labelled development.
- No trial record, correctness score, or repair transcript for this task
  may be exported for training or tuning reuse while it remains held out.
  Nothing in this repository currently pipes cross-language-v1 records
  through `agent_trajectory_export`; this note exists so a future
  integration inherits the constraint rather than rediscovering it.

## Capacity ceilings this task deliberately stays under

Same as `sequence-digest-v1`: single-condition nested `if`/`else` (not a
`&&`-chain) per assertion group, and one ~2 KB file per language, well
inside `SPX-G171`'s workspace-graph pre-bound and `SPX-H006`'s per-function
cleanup replay path budget.
