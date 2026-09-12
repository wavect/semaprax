# Task equivalence: `sequence-digest-v1`

This is the fairness contract every language implementation of this task
must meet, so a future reader can check a comparison rather than take it on
faith (`docs/METHODOLOGY.md` in this suite owns the general contract; this
file owns this one task's specific choices).

## Problem

Four independent, pure, scalar functions over five fixed signed 64-bit
inputs `a, b, c, d, e`:

- `sum`: `a + b + c + d + e`.
- `count_even`: how many of the five are even.
- `count_negative`: how many of the five are negative.
- `max`: the largest of the five.

## Inputs

Exactly five signed 64-bit integers, supplied as literal arguments in test
vectors baked into the source (no file, stdin, environment, or network
input). No implementation may special-case the specific vectors used by the
public tests; the hidden vectors (below) exist precisely to catch that.

## Outputs

Four signed 64-bit values, one per function above. An implementation may
return them as one record/struct or as four separate values; the harness
checks the four values through each language's own test assertions, not a
shared serialization format.

## Boundary of the measured region

The measured region is exactly the four pure functions, invoked in-process
by the language's own official test runner. Process startup, compiler
invocation, and test-harness overhead are outside the boundary and are never
scored in this schema version (which does not measure wall-clock time at
all; see `docs/METHODOLOGY.md`).

## Allowed optimizations and required equivalence

- Each language uses its own idiomatic control flow. The reference
  implementations use straight-line arithmetic and `if`/ternary expressions,
  not `match`/`switch`, because none of the four functions has more than one
  boolean condition to resolve.
- Overflow: the five inputs and the arithmetic used to combine them
  (addition, comparison) stay well inside `i64`/`number` range for both the
  public and hidden vectors, so no implementation needs an overflow policy
  to pass. This is a deliberately narrow first task; a future task in this
  suite that needs to compare overflow behavior across languages must say so
  explicitly, because SEMAPRAX, Rust, and TypeScript disagree about it.
- No implementation may import a language-specific "reduce over five
  numbers" library helper that would substitute for the algorithm; the
  four functions must be authored, not delegated to a stdlib fold. (This is
  a convention this task's own reference implementations follow, not
  something the harness currently enforces mechanically — see
  `docs/METHODOLOGY.md`'s "What the harness cannot yet check" section.)

## Officially supported toolchain and success signal per language

The three implemented adapters use their language's own official invocation,
and each reports pass/fail through the convention that language actually
uses — not a convention imposed on it. `adapters.json` records these as data,
and `run.py`'s `success` predicate reads each one honestly:

| Language | Official invocation | Success signal |
| --- | --- | --- |
| SEMAPRAX | `semaprax run digest.spx` | prints `app.main`'s `i64` result to stdout; `0` means every check passed. Process exit code is `0` whenever interpretation completes, pass or fail — SEMAPRAX's `run` does not use exit status to report test outcome, so the harness reads stdout instead. |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin` then `./test_bin` | process exit code (`0` = all `#[test]` fns passed) |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts` then `node index.js` | process exit code (`0` = no uncaught assertion `Error`) |

Two boundary choices are recorded here so they can be judged, not assumed:

- **Rust uses bare `rustc --test`, not `cargo test`.** The task has zero
  dependencies, so Cargo's build graph and lockfile add nothing this
  snapshot needs, and a nested `Cargo.toml` would risk entangling this
  fixture with the repository's own Cargo workspace. `rustc --test` is an
  officially documented rustc feature (not a home-grown substitute), but it
  is a narrower slice of "the official Rust workflow" than `cargo test`
  would be, and a future task with real dependencies must use `cargo`.
- **TypeScript's assertions are a five-line local helper, not
  `node:assert`.** Typing `import ... from "node:assert"` under `tsc
  --strict` needs the `@types/node` package, and this sandbox performs no
  package installs (no network access at build time). A failed assertion
  throws, and an uncaught throw is Node's own nonzero-exit signal, so the
  success predicate is unaffected; only the spelling of the assertion is
  narrower than a full `@types/node`-backed pipeline would allow.

## Hidden tests

`hidden/<language>/` holds a second copy of each source file that overlays
(replaces, same relative path) the public one for scoring only. It adds two
vectors never shown publicly: the all-zero identity case, and a case where
the true maximum (`100`) appears after three smaller positive values and one
large negative one, to catch an implementation that only tracks the maximum
of the *first* comparison instead of folding over all five inputs. The
harness never copies `hidden/` into the tree used for the public build step
(see `docs/METHODOLOGY.md`'s leak-check description).

## Capacity ceilings this task deliberately stays under

- `SPX-G171` (the workspace-graph builder's byte/identity pre-bound, roughly
  35-40 KB of a project's own source): this task is a single ~2 KB file per
  language; nowhere near the ceiling.
- `SPX-H006` (per-function cleanup replay path/work budget, which an
  isolated function combining roughly ten sequential `if`/`else` branches can
  hit on its own): an earlier draft of this fixture's SEMAPRAX `main`
  chained eight and then sixteen boolean comparisons with `&&` and hit
  `SPX-H006` on exactly that function, on this task's very small source, with
  no other cost contributor in the file. The fix was structural, not a
  reduction in what is checked: replace the `&&`-chain with a sum of
  `eq(...)` calls and a single guarding `if`. This is recorded because it is
  a real, reproducible data point about the ceiling's shape (short-circuit
  branching, not raw comparison count, is what drives the path count up),
  not an assertion about where the ceiling sits in general.
