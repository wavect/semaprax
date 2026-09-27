# Task equivalence: `telemetry-overflow-diagnosis-v1`

This held-out task is this suite's answer to issue #106's requested
"backend discrepancy diagnosis" family, and it is honest about a limit
rather than manufacturing a fake one to fill the checkbox.

## Why a literal interpreter/native-C11/Core-Wasm split is not in this file

AGENTS.md's non-negotiable invariants require "equivalent checked behavior
on every backend that claims to implement the admitted feature." A real
result divergence between SEMAPRAX's interpreter, its native C11 backend,
and its Core Wasm backend for one admitted program would therefore itself
be a compiler defect — the same class of bug tracked as issue #21's Wasm
scratch-local miscompile — not a legitimate axis to design a task around.
Deliberately authoring a candidate that diverges across those three
backends would either (a) rely on an unadmitted feature, which is not a
representative "backend discrepancy" a real user would hit, or (b) require
fabricating a bug in this repository's own backends to make the task work,
which is not owned by, or scoped to, a benchmark fixture. Neither is
available honestly, and this file says so rather than pretending otherwise.
Rust and TypeScript, in turn, have no directly analogous three-way
same-language backend split for this corpus to mirror in the first place.

## What is expressed instead: the diagnosis skill, encoded portably

Every real per-runtime divergence this corpus's three ports can actually
produce agrees on one root cause: **fixed-width arithmetic that is not
widened or saturated before a sum can leave its declared range**, and the
one *symptom* that divergence takes is different in every runtime for
reasons intrinsic to that runtime, not to this task's authoring:

- **Rust**: `rustc --edition 2021 --test main.rs` (this suite's own
  official invocation, deliberately without `-O`) is a debug build, so
  `i32::MAX + 1` panics at runtime ("attempt to add with overflow") — Rust's
  own well-known debug/release split, reliably reproduced by staying on the
  debug path this harness already uses everywhere else.
- **TypeScript**: JavaScript numbers are IEEE-754 doubles with no native
  32-bit integer type. The same raw `deltaA + deltaB` neither wraps nor
  traps at this magnitude (`2147483647 + 1 = 2147483648` is exactly
  representable, far inside the 2^53 safe-integer range) — it silently
  returns a value outside the declared 32-bit domain instead.
- **SEMAPRAX**: `i32` arithmetic is checked identically on every backend
  this project admits (`src/interpreter.rs`'s `StatusCase::AddOverflow`);
  the naive candidate raises a defined runtime fault
  (`semaprax.status.v1/semaprax.arithmetic.v1`) instead of producing any
  answer at all, on every backend, per the invariant quoted above.

A solver that only ever sees one of these three symptoms and "fixes" it
locally (e.g. wrapping the Rust call in `catch_unwind`, or special-casing
the one failing TypeScript input) has not diagnosed the actual defect. The
one fix that resolves all three symptoms at once is the same in every
port: guard the addition so it saturates at the boundary before the
underlying representation's own behavior (trap, silent range violation, or
checked fault) ever triggers. That diagnosis — not any single runtime's
failure mode — is the skill this task measures.

## Problem and oracle

`combine_telemetry(delta_a, delta_b)` models a running telemetry register
that must equal `delta_a + delta_b`, saturated to `[i32::MIN, i32::MAX]`
when the exact sum would leave that range — never wrap, never trap, never
raise a checked-arithmetic fault. The committed candidate implements this
correctly in all three ports with an explicit overflow guard performed
*before* the addition:

```
if delta_b > 0 && delta_a > I32_MAX - delta_b { I32_MAX }
else if delta_b < 0 && delta_a < I32_MIN - delta_b { I32_MIN }
else { delta_a + delta_b }
```

This is the required implementation. Rust's `i32::saturating_add` /
`checked_add` would perform exactly the computation this task measures and
must not be used in a solver's answer — that is the "delegating the actual
algorithm to a library call" case METHODOLOGY.md's equivalence contract
rules out. SEMAPRAX has no built-in saturating-arithmetic function to
delegate to in the first place.

## Inputs and outputs

- **Inputs**: two independent signed 32-bit deltas (`i32` in Rust and
  SEMAPRAX; `number` in TypeScript, restricted by this task to integers in
  `[-2147483648, 2147483647]`, which is always exactly representable as a
  JavaScript double).
- **Output**: one signed 32-bit result (same representation), an exact
  equality check.
- **Boundary of the measured region**: the pure `combine_telemetry` /
  `combineTelemetry` function, invoked by each language's own test runner
  exactly as every other task in this corpus is. Compiler/interpreter
  startup and the test harness's own overhead are outside the comparison.
- **Allowed optimizations**: ordinary control flow and comparisons only. No
  saturating-arithmetic standard-library call in any language (see above).

## Vectors

Public vectors (`combine_telemetry(10, 20) == 30`,
`combine_telemetry(-5, 5) == 0`) never approach the 32-bit boundary, so an
unguarded `delta_a + delta_b` coincidentally matches the correct answer on
every public example — the bug is invisible until the hidden phase. Hidden
vectors deliberately sit exactly at the boundary
(`combine_telemetry(i32::MAX, 1) == i32::MAX`,
`combine_telemetry(i32::MIN, -1) == i32::MIN`), which is where the three
runtimes' three different unguarded behaviors (panic, silent range
violation, checked fault) all diverge from the one correct saturated
answer.

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports each keep `combine_telemetry`
in the same candidate module. Public and hidden phases share that module
unchanged; the hidden overlay replaces only the assertion entry point
(`main.rs` / `index.ts` / `src/app.spx`), the same overlay style
`stale-edit-preservation-v1` and `booking-window-conflict-v1` use.

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

## Split and negative control

This task is `split: "held_out"` in `tasks.json` and shares no fixture,
vector, or task family with any development or validation task in this
suite. Its executable negative control,
`an_unguarded_addition_passes_public_but_fails_hidden_in_all_three_ports_for_three_different_reasons`
in
`tests/documentation/cross_language_benchmark_suite/telemetry_overflow_diagnosis.rs`,
mutates a copy of the committed correct candidate to the plain, unguarded
`delta_a + delta_b` in all three ports, proves the mutation target text
existed before rewriting it, and asserts the mutated candidate passes every
public vector while failing the hidden overlay in all three real ports with
`leak_check: "ok"` — confirming, against real `rustc`/`tsc`/`node`/
`semaprax`, the exact three distinct failure mechanisms this file
describes above (a Rust panic, a wrong TypeScript number, and a SEMAPRAX
checked-arithmetic fault). No trial, correctness score, or repair
transcript for this task may be exported for training or tuning reuse
while it remains held out, for the same reason
`bounded-counter-repair-v1/EQUIVALENCE.md` states for its own held-out
declaration.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. C, Python, and Java keep
the same candidate/entry split the Rust and TypeScript ports use
(`candidate.c`/`candidate.py`/`Candidate.java` hold the unchanged
`combine_telemetry` function; the hidden overlay replaces only
`main.c`/`digest.py`/`Main.java`, via `#include`, `import`, and javac's
same-directory auto-discovery respectively). C uses `int32_t`/`INT32_MAX`/
`INT32_MIN` from `<stdint.h>`; Java's `int` is already exactly 32-bit;
Python has no native 32-bit width, so its port carries explicit
`I32_MAX`/`I32_MIN` constants. In all three, the guard is evaluated before
any addition that could overflow, so the guarded form never invokes C's
signed-overflow undefined behavior or (for Swift, see below) Swift's
trapping integer overflow. Swift's fixed single-file `swiftc main.swift`
invocation admits no candidate/entry split, so `combineTelemetry` is
repeated verbatim in both the public and hidden `main.swift`; it uses
`Int32`/`Int32.max`/`Int32.min` for the same genuine 32-bit width.

Each port was authored independently against this file's saturate-not-
wrap-or-trap contract and the Rust/TypeScript references, not
transliterated line-by-line, and was independently compiled/run against
the public and hidden vectors above, then checked against this file's own
plain-`delta_a + delta_b` negative control (the identical mutation the
Rust/TypeScript/SEMAPRAX self-test already uses): the mutant passes every
public vector and fails both hidden boundary vectors in all four
languages, each failing a different way exactly as this file's "Why a
literal interpreter/native-C11/Core-Wasm split is not in this file"
section predicts for an unguarded combiner — C, Java, and Swift's
wrapping variant (`&+`) silently flip to the opposite boundary
(`-2147483648`/`2147483647` instead of the correct saturated values), while
Python's arbitrary-precision integers overflow the declared range entirely
(`2147483648`/`-2147483649`) — confirming the hidden vectors are
non-vacuous.
