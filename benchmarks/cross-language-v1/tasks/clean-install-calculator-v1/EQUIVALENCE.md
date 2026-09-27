# Task equivalence: `clean-install-calculator-v1`

This held-out task is this suite's answer to issue #106's requested
"clean-install use" family: the workflow a user hits immediately after
installing the toolchain, before any of their own code exists, rather than
a hand-authored fixture that never went through the tool's own scaffolding.

## What "clean install" means for each port

`benchmarks/cross-language-v1/adapters.json` is shared across every task in
this corpus and, deliberately, never invokes Cargo or npm for any of them
(`METHODOLOGY.md`'s equivalence contract; also why every other Rust/
TypeScript task here is a bare `rustc --test` / `tsc --strict` file, not a
`cargo new`/`npm init` project): this sandbox forbids build-time network
access, and a package manager's build graph and lockfile would add nothing
any existing task needs. That constraint is unrelated to this task
specifically and is not being reopened here. So "clean install" in this
corpus is anchored at the **source-text level**, not the **tool-invocation
level**:

- **SEMAPRAX**: the public candidate's `AGENTS.md`, `README.md`,
  `semaprax.toml`, and `src/tests.spx` are byte-for-byte the output of
  `semaprax new <dest> --name clean-install-calculator --template
  calculator` run against this checkout's own `target/debug/semaprax` — the
  literal command a fresh install's `docs/QUICKSTART.md` workflow leads to,
  captured verbatim rather than approximated. `src/core.spx` starts from
  that same scaffold's own `add` function, unchanged. The only files this
  task's public candidate does not leave untouched are `src/core.spx`
  (which gains the `subtract` function this task asks for) and `src/app.spx`
  (whose `main` is rewritten from the scaffold's plain demo computation,
  `add(19, 23)`, to this suite's own pass/fail sentinel convention — 0 when
  every assertion passes — exactly as every other task in this corpus
  already does for its entry point; this is a structural requirement of
  `run.py`'s `stdout_equals` success predicate documented in
  METHODOLOGY.md, not something specific to this task).
- **Rust and TypeScript**: there is no official scaffolding command for
  either language exercised by this benchmark's adapters (see above), so
  each port's public candidate instead hand-reproduces the identical
  minimal "one already-working operation, ready for a second" shape the
  real SEMAPRAX scaffold ships — a two-function calculator module with a
  pre-existing `add` and a `subtract` the task asks the solver to add. The
  task each port solves is the same small feature-addition problem; the
  tool-invocation ceremony that unavoidably differs across ecosystems is
  explicitly out of scope for this comparison, as METHODOLOGY.md's
  "Boundary of the measured region" requires stating rather than leaving
  implicit.

The distinguishing skill this task measures is different from this
corpus's other feature-addition task
(`module-import-refactor-v1`, a hand-designed fixture): here the solver
must work *within* a project whose surrounding files (a generated
`AGENTS.md`/`README.md`, an existing `@id` naming convention, an existing
`sources`/`web`-exports manifest section) were produced by the tool itself,
not authored for the exercise, and add the new function and its manifest
entry without disturbing any of it.

## Problem and oracle

`add(left, right) = left + right` is the pre-existing scaffold operation,
unchanged. `subtract(left, right) = left - right` is the operation this
task adds; unlike a saturating/bounded counter elsewhere in this corpus, a
calculator's subtraction is an ordinary signed operation and must be
allowed to go negative. All inputs are integers well inside the
intersection of Rust `i64`, SEMAPRAX `i64`, and JavaScript safe integers.

## Inputs and outputs

- **Inputs**: two signed integers per call, literal fixtures in every
  vector.
- **Outputs**: one signed integer, exact equality.
- **Boundary of the measured region**: the pure `add`/`subtract` functions,
  invoked by each language's own test runner exactly as every other task in
  this corpus is. Project/file scaffolding, compiler/interpreter startup,
  and test-harness overhead are outside the comparison.
- **Allowed optimizations**: ordinary arithmetic only; no clamping, no
  reuse of a bounded-counter-style floor.

## Vectors

Public vectors (`add(19, 23) == 42`, `subtract(50, 8) == 42`) never produce
a negative result, so a candidate that mistakenly reuses this corpus's
own "floor negative results at zero" idiom from
`stale-edit-preservation-v1`/`bounded-counter-repair-v1` — a plausible
cross-contamination of a pattern learned from a neighboring task in the
same corpus — passes every public vector undetected. Hidden vectors
(`subtract(8, 50) == -42`, `subtract(-5, -5) == 0`, plus a repeated
`add(19, 23) == 42` confirming the pre-existing scaffold operation is
unchanged) catch exactly that mistake.

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports each keep `add` and `subtract`
in the same candidate module (`core.spx` for SEMAPRAX). Public and hidden
phases share that module unchanged; the hidden overlay replaces only the
assertion entry point (`main.rs` / `index.ts` / `src/app.spx`), the same
overlay style `stale-edit-preservation-v1` and `booking-window-conflict-v1`
use. The SEMAPRAX public candidate additionally carries the scaffold's
`AGENTS.md`, `README.md`, and `src/tests.spx`, none of which the hidden
overlay touches or the harness's build/run step reads; they exist only to
make the public candidate a genuine, complete scaffold rather than a
trimmed-down excerpt of one.

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

## Split and negative control

This task is `split: "held_out"` in `tasks.json` and shares no fixture,
vector, or task family with any development or validation task in this
suite. Its executable negative control,
`a_bounded_counter_floor_habit_borrowed_from_a_neighboring_task_passes_public_but_fails_hidden_in_all_ports`
in
`tests/documentation/cross_language_benchmark_suite/clean_install_calculator.rs`,
mutates a copy of the committed correct candidate's `subtract` to clamp
negative results at zero in all three ports, proves the mutation target
text existed before rewriting it, and asserts the mutated candidate passes
every public vector while failing the hidden overlay in all three real
ports with `leak_check: "ok"`. No trial, correctness score, or repair
transcript for this task may be exported for training or tuning reuse
while it remains held out, for the same reason
`bounded-counter-repair-v1/EQUIVALENCE.md` states for its own held-out
declaration.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. This task's own
"clean install" framing above is specific to SEMAPRAX's scaffolding
command and to Rust/TypeScript having no analogous official scaffold in
this suite's adapters; C, Python, Swift, and Java are new lanes with no
such scaffolding claim to make, so each port is instead a hand-authored
minimal two-function calculator module in that language's own idiom,
solving the identical `add`/`subtract` problem and reusing the identical
public/hidden vectors. C, Python, and Java keep the same candidate/entry
split the Rust and TypeScript ports use (`candidate.c`/`candidate.py`/
`Candidate.java` hold the unchanged functions; the hidden overlay replaces
only `main.c`/`digest.py`/`Main.java`, via `#include`, `import`, and
javac's same-directory auto-discovery respectively). Swift's fixed
single-file `swiftc main.swift` invocation admits no such split, so both
functions are repeated verbatim in both the public and hidden `main.swift`.

Each port was independently compiled/run against the public and hidden
vectors above, then checked against this file's own negative control (a
`subtract` that clamps negative results at zero, the "bounded counter
floor habit borrowed from a neighboring task" this file names): the mutant
passes every public vector (neither needs clamping) and fails the
`subtract(8, 50) == -42` hidden vector (returning `0` instead) in all four
languages, confirming the hidden vectors are non-vacuous.
