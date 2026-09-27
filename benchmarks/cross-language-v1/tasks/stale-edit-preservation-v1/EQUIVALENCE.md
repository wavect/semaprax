# Task equivalence: `stale-edit-preservation-v1`

This held-out task measures a discipline distinct from every other family in
this suite: whether a repair leaves a previously completed, unrelated piece
of work untouched. Every other task's candidate module contains only the
code the task asks for. This one's candidate module additionally carries a
second, already-correct function — `stale_note` / `staleNote` — introduced
by a "prior session" and explicitly marked as out of scope for the repair
below it. A solver that rewrites the whole file, "cleans up" what looks like
dead code, or otherwise clobbers that function while fixing the real bug has
destroyed a stale edit it was never asked to touch, even though its own
targeted fix may be entirely correct.

## Problem and oracle

`apply_discount(price, pct)` receives a nonnegative price and a percentage
that a corrupted upstream feed may report above 100. It must return
`floor(price - price * pct / 100)`, clamped to a minimum of `0`. The
public candidate computes the unclamped formula correctly for percentages at
or below 100 but the repair itself — clamping at zero — is exactly what the
committed reference already does, matching this suite's convention (see
`booking-window-conflict-v1/EQUIVALENCE.md`): the committed source is the
correct answer, and the harness's own negative controls (in
`tests/documentation/cross_language_benchmark_suite.rs`) mutate a copy of it
to exercise two distinct, independent wrong candidates:

1. Deleting the zero-floor clamp (a plausible incomplete repair): passes
   every public vector because none use a percentage above 100, fails the
   hidden corrupted-percentage vectors.
2. Deleting or rewriting `stale_note`/`staleNote` while leaving
   `apply_discount`'s repair intact (a plausible over-aggressive edit):
   passes every public vector, because the public suite never calls
   `stale_note`, and fails the hidden vectors that call it directly. This is
   the discriminating case for this task family — no other task's hidden
   suite exists specifically to catch damage to code the task never asked
   the solver to change.

`stale_note(tag) = tag * 2 + 7`. It is deliberately trivial: any value would
do, since the point is presence and byte-for-byte behavior, not the
computation.

All inputs are nonnegative literal integers well inside the intersection of
Rust `i64`, SEMAPRAX `i32`, and JavaScript safe integers. The fixture does
not exercise negative prices or negative percentages; `pct` may exceed 100
but is never negative.

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports each keep `stale_note` and
`apply_discount` in the same candidate module. Public and hidden phases
share that module unchanged; the hidden overlay replaces only the assertion
entry point (`main.rs` / `index.ts` / `src/app.spx`).

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

## Vectors

Public vectors cover ordinary discounts at or below 100%. Hidden vectors
add: a percentage above 100 that must floor at zero (twice, at different
magnitudes), and two direct calls into `stale_note` confirming its output is
byte-for-byte what the prior session left behind.

## Split and independent negative controls

This task is `split: "held_out"` in `tasks.json` and shares no fixture,
vector, or task family with any development or validation task in this
suite. Its two executable self-tests —
`a_missing_zero_floor_repair_passes_public_but_fails_hidden_in_all_ports`
and
`a_candidate_that_deletes_the_preexisting_stale_helper_passes_public_but_fails_hidden_in_all_ports`
in `tests/documentation/cross_language_benchmark_suite/stale_edit_preservation.rs`
— are independent negative controls: each mutates a copy of the committed
correct candidate, proves the mutation target text existed before rewriting
it (so the control cannot silently stop testing anything), and asserts the
mutated candidate passes every public vector while failing the hidden
overlay in all three real ports with `leak_check: "ok"`. Neither mutation
touches the hidden files themselves, and the harness's own leak check
proves the public build tree never receives the hidden-only assertion
paths. No trial, correctness score, or repair transcript for this task may
be exported for training or tuning reuse while it remains held out, for the
same reason `bounded-counter-repair-v1/EQUIVALENCE.md` states for its own
held-out declaration.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. C, Python, and Java keep
the same candidate/entry split the Rust and TypeScript ports use
(`candidate.c`/`candidate.py`/`Candidate.java` hold the unchanged
`apply_discount`/`stale_note` functions; the hidden overlay replaces only
`main.c`/`digest.py`/`Main.java`, via `#include`, `import`, and javac's
same-directory auto-discovery respectively). Swift's fixed single-file
`swiftc main.swift` invocation admits no such split, so both functions are
repeated verbatim in both the public and hidden `main.swift`.

Each port was authored independently against this file's floor-at-zero and
unchanged-helper contract and the Rust/TypeScript references, not
transliterated line-by-line, and was independently compiled/run against
the public and hidden vectors above, then checked against a deliberately
un-repaired candidate that omits the `raw < 0` floor entirely (returning
the corrupted negative discount as-is): the mutant passes every public
vector (neither public vector needs clamping) and fails both corrupted-
percentage hidden vectors (`-50`/`-12` instead of the correct `0`/`0`) in
all four languages, while `stale_note`'s own hidden checks still pass
(that helper was never touched), confirming the hidden vectors are
non-vacuous.
