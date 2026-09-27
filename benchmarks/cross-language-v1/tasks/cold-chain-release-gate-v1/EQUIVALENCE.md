# Task equivalence: `cold-chain-release-gate-v1`

This held-out validation task models a warehouse cold-chain release gate. A
shipment may leave cold storage only if both integer readings lie within their
inclusive operating bands. It is distinct from the suite's digest,
bounded-state, structured-record, imported-invoice, and booking-window tasks:
it tests an independent conjunction of two scalar range predicates, with no
accumulation, monetary calculation, interval-overlap rule, input envelope, or
module-import algorithm.

## Problem and oracle

`release_allowed(core_temperature, seal_pressure)` returns `1` exactly when:

```
2 <= core_temperature <= 8 && 95 <= seal_pressure <= 105
```

It returns `0` otherwise. Readings are signed integer literals. The fixture
uses only values inside Rust `i64`, SEMAPRAX `i32`, and JavaScript's exact
integer range.

A plausible wrong candidate joins the temperature and pressure predicates
with `||`. It accepts a shipment when either reading is acceptable. Public
vectors contain one case where both readings are acceptable and cases where
both are unacceptable, so that candidate passes public. Hidden vectors hold
one reading inside its band while the other fails, so the wrong candidate
fails in every port.

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports place the candidate predicate in
its own module. Public and hidden phases share that candidate source; the
hidden overlay replaces only the assertion entry point. Therefore hidden
scoring executes the solver-authored candidate rather than a second oracle.

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

## Boundary and allowed implementation

The measured logic is the pure release predicate and the language-native test
entry point. Process startup, compilation, and harness work remain outside
that logical boundary. Implementations may use ordinary comparisons and
control flow, but may not replace the predicate with lookup data tailored to
the listed vectors.

This fixture is held out. Its vectors and wrong-candidate control must not be
used to tune a language adapter or admitted feature set.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. C, Python, and Java keep
the same candidate/entry split the Rust and TypeScript ports use
(`candidate.c`/`candidate.py`/`Candidate.java` hold the unchanged
`release_allowed` predicate; the hidden overlay replaces only
`main.c`/`digest.py`/`Main.java`, via `#include`, `import`, and javac's
same-directory auto-discovery respectively). Swift's fixed single-file
`swiftc main.swift` invocation admits no such split, so `releaseAllowed` is
repeated verbatim in both the public and hidden `main.swift`.

Each port was authored independently against this file's two-independent-
band contract and the Rust/TypeScript references, not transliterated
line-by-line, and was independently compiled/run against the public and
hidden vectors above, then checked against a deliberately mutated candidate
that ORs the two band checks instead of ANDing them (so a good reading in
either sensor alone releases the shipment): the mutant passes every public
vector (both bands happen to agree there) and fails both
one-good-one-bad hidden vectors (`1` instead of the correct `0`) in all
four languages, confirming the hidden vectors are non-vacuous.
