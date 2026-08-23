# String Operations v1

Status: implemented in this tranche. Three prelude-style intrinsic functions
admitted wherever owned `string` values are already admitted.

## Operations

| Source name       | Reserved stable identity   | Signature                     | Argument ownership |
| ----------------- | -------------------------- | ----------------------------- | ------------------ |
| `string_len`      | `core.string.len`          | `(s: string) -> i64`          | borrowed read      |
| `string_concat`   | `core.string.concat`       | `(a: string, b: string) -> string` | both consumed by move |
| `string_is_empty` | `core.string.is_empty`     | `(s: string) -> bool`         | borrowed read      |

Length is the UTF-8 byte length on every backend; `string_is_empty(s)` is
exactly `string_len(s) == 0`.

## Admission shape

Free-function intrinsics with compiler-reserved identities were chosen over
the two alternatives:

- **Method-call syntax** (`s.len()`) would require primitive-receiver dispatch
  in the verifier, resolver, validator, cleanup planner, graph projection, and
  both backends. Method calls currently require class receivers only.
- **Prelude declarations** (like the compiler-owned `Option`/`Result`
  variants) would change every program's revision digest because the Graph
  binds the whole prelude contract unconditionally, breaking pinned digests
  for programs that never use strings.

Instead, a call to one of the three reserved names resolves to an ordinary
monomorphic [`hir::ResolvedExprKind::Call`] whose callee carries the reserved
`core.string.*` identity. Consequences:

- Parser, canonical formatter, and graph JSON need no new syntax or node
  kinds; intrinsic calls project as ordinary `"call"` nodes bound to their
  reserved identity, so projections stay deterministic and programs that
  never name the operations keep byte-identical output (verified against the
  base revision: identical revision digest and identical Graph JSON bytes).
- No new diagnostic codes were needed. Intrinsic calls verify through the
  ordinary monomorphic call machinery with synthetic signatures, so they reuse
  the established families exactly: `SPX-T204` (arity), `SPX-T205`
  (argument type), `SPX-T225` (type arguments), `SPX-S113` (reserved name),
  `SPX-H006` (HIR invariant failures including resolved use-after-move).
- The names are compiler-reserved: declaring a function named `string_len`,
  `string_concat`, or `string_is_empty` is rejected with `SPX-S113`, mirroring
  how prelude type names are protected.

## Ownership

Consumption mirrors existing String move-checking exactly:

- `string_concat` arguments use the same synthetic `own` parameter shape as an
  ordinary declared `string` parameter, so the shared transfer machinery marks
  them moved. A later use of a consumed argument is a compile-time diagnostic
  (`resolved value ... used after it was moved`, `SPX-H006`), never a backend
  accident.
- `string_len` / `string_is_empty` use non-transferring parameters, so reads
  leave the operand available.

## Backends

- Native C11: gated runtime helpers (`spx_string_len`, `spx_string_concat`,
  `spx_string_is_empty`) appended after the string runtime only when a program
  reaches the operations, so existing projections keep their exact committed
  bytes. Consuming operations free their input buffers exactly at the
  operation site, like owned string equality.
- Wasm32: two optional host imports (`spx_string_len`, `spx_string_concat`)
  appended after the base string imports only when used; `is_empty` lowers as
  `len` + `i64.eqz`. Modules for programs without the operations keep their
  exact bytes.
- Interpreter: intrinsic calls evaluate inside the scalar profile with the
  same byte semantics; user functions taking strings remain outside the
  profile exactly as before.

## Evidence

`tests/string_ops_v1.rs` proves canonical round-trip, deterministic graph JSON
with pinned fragments, HIR binding to the reserved identities, stable
diagnostics (type error, arity, shadowing, use-after-move), borrowed-read
non-movement, interpreter agreement, native C11 O0/O2 execution equality, Node
Wasm execution equality, and the byte-gating of helpers/imports for programs
that do not use the operations. `examples/string_ops.spx` is the canonical
committed example exercised by the examples suite.
