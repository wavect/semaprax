# Task equivalence: `stable-dispatch-order-v1`

This held-out greenfield task models a tiny dispatch queue. Three jobs arrive
in their argument order, each with an integer priority. The implementation
must order the three job identifiers by ascending priority and preserve
arrival order for equal priorities. It is a bounded, multi-step stable
ordering algorithm, distinct from the existing digest, saturating counter,
record validation, invoice, booking-window, and scalar sensor-gate families.

## Problem and oracle

`dispatch_order(a_priority, b_priority, c_priority)` returns a three-digit
integer containing job identifiers in dispatch order. The identifiers are
always `1` for `a`, `2` for `b`, and `3` for `c`; for example result `231`
means `b`, then `c`, then `a`.

The order is ascending priority. Equal priorities retain their original
arrival order: `a` precedes `b`, which precedes `c`. The reference uses a
fixed three-item selection sort: choose the earliest minimum, then choose the
earliest of the two remaining values. Inputs are signed integer literals
inside the common exact range of SEMAPRAX `i32`, Rust `i64`, and JavaScript
`number`.

A plausible wrong candidate uses strict `<` comparisons in that selection
logic. It agrees with the stable implementation whenever all priorities are
distinct, so it passes public vectors. It chooses a later job when priorities
tie, which hidden asymmetric and all-equal vectors expose.

## Ports and overlays

Rust, TypeScript, and SEMAPRAX Project ports keep the candidate in a module
shared by the public and hidden overlays. The hidden phase replaces only the
assertion entry point, so it tests the copied candidate instead of a second
implementation.

| Language | Invocation | Success signal |
| --- | --- | --- |
| Rust | `rustc --edition 2021 --test main.rs -o test_bin`, then `./test_bin` | exit code `0` |
| TypeScript | `tsc --strict --target ES2020 --module commonjs index.ts`, then `node index.js` | exit code `0` |
| SEMAPRAX Project | `semaprax run .` | stdout `0` |

## Boundary and allowed implementation

The compared logic is the fixed three-item ordering calculation and each
language's local assertions. Compilation, process startup, and runner work
are outside the task boundary. Implementations may use ordinary comparisons
and control flow but may not special-case the listed vectors or delegate the
ordering to a library sorting routine.

This fixture is held out. Its hidden vectors and strict-comparison negative
control must not be used to tune an adapter or admitted feature set.

## C, Python, Swift, and Java ports

Added under the `runnable_adapter_v2` extension. C, Python, and Java keep
the same candidate/entry split the Rust and TypeScript ports use
(`candidate.c`/`candidate.py`/`Candidate.java` hold the unchanged
`dispatch_order` function; the hidden overlay replaces only
`main.c`/`digest.py`/`Main.java`, via `#include`, `import`, and javac's
same-directory auto-discovery respectively). Swift's fixed single-file
`swiftc main.swift` invocation admits no such split, so `dispatchOrder` is
repeated verbatim in both the public and hidden `main.swift`.

Each port was authored independently against this file's tie-preserving
contract and the Rust/TypeScript references, not transliterated
line-by-line, and was independently compiled/run against the public and
hidden vectors above, then checked against this file's own
strict-comparison negative control (every `<=` replaced with `<`): the
mutant passes every public vector (all distinct priorities, so `<` and
`<=` agree) and fails all four hidden tie vectors in all four languages,
confirming the hidden vectors are non-vacuous.
