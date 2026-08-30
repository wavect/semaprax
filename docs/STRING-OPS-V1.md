# String Operations v1

Audience: language users, tool authors, and compiler contributors.

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

The later [native inline String settlement correction](NATIVE-INLINE-STRING-SETTLEMENT-V1.md)
adds authored, unrun failure-path allocation evidence for ordinary C11 and
stdout-transcript execution. It intentionally changes String-bearing native
function bodies while preserving intrinsic signatures and diagnostics. It
is complemented by the authored [native String contents correction](NATIVE-STRING-CONTENTS-V1.md),
which preserves embedded NUL through all ordinary/native stdout operations.
Both corrections remain unrun; ordinary Wasm String drop remains open, and
the value fixtures below are not physical settlement evidence.

`tests/string_ops_v1.rs` proves canonical round-trip, deterministic graph JSON
with pinned fragments, HIR binding to the reserved identities, stable
diagnostics (type error, arity, shadowing, use-after-move), borrowed-read
non-movement, interpreter agreement, native C11 O0/O2 execution equality, Node
Wasm execution equality, and the byte-gating of helpers/imports for programs
that do not use the operations. `examples/string_ops.spx` is the canonical
committed example exercised by the examples suite.

---

## String operations breadth v2 (2026-08-24)

Status: implemented in the `feat/string-ops-breadth-v2` tranche. Four more
prelude-style intrinsics extend the same admission shape; everything above is
unchanged.

### Operations

| Source name           | Reserved stable identity         | Signature                              | Argument ownership        |
| --------------------- | -------------------------------- | -------------------------------------- | ------------------------- |
| `string_starts_with`  | `core.string.starts_with`        | `(s: string, prefix: string) -> bool`  | both borrowed reads       |
| `string_contains`     | `core.string.contains`           | `(s: string, needle: string) -> bool`  | both borrowed reads       |
| `string_len_chars`    | `core.string.len_chars`          | `(s: string) -> i64`                   | borrowed read             |
| `string_from_char`    | `core.string.from_char`          | `(c: char) -> string`                  | copied scalar, no transfer|

`string_len_chars` counts Unicode scalar values, so `"héllo"` has a char
length of 5 while its UTF-8 byte length (`string_len`) is 6 on every backend.
An empty needle/prefix follows the ordinary substring convention: every string
starts with and contains the empty string.

### Why `string_char_at` did not land

A character-indexing operation (`string_char_at(s, index) -> char`) was part
of the planned wave but is not admitted. Its negative/out-of-bounds story has
no home: the compiler's normalized runtime failure lattice carries exactly two
classes today (`semaprax.contract.v1`, `semaprax.arithmetic.v1`; see the
OpenAPI status schema and the native status runtime), with no range/bounds
class, and inventing one would require editing the shared failure machinery
far beyond the additive intrinsic-table seams this wave committed to. The
contingency named in the plan applies: `Option<char>` is not an alternative
because `char` payloads are not admitted inside `Option`. `string_len_chars`
lands instead as the third borrowed read, and indexing stays out of scope
along with slicing and mutation.

### Admission and gating

Same architecture as v1: reserved names resolve through the synthetic
signatures into ordinary monomorphic calls bound to their `core.string.*`
identities; parser, canonical formatter, resolver/HIR, verifier, semantic
graph, cleanup planning/replay, interpreter, and both backends consume the
extended table without new node kinds or diagnostic codes. The only table
extension beyond name/id/arity bookkeeping is per-parameter expected types:
`resolved_params`, `ast_params`, and the two HIR argument checks now consult
`StringOp::param_types()` so `string_from_char` admits exactly one `char`.

Backends gate breadth-v2 lowering as one separate group:

- Native C11 appends `NATIVE_STRING_OPS_V2_RUNTIME_C` (helpers
  `spx_string_starts_with`, `spx_string_contains`, `spx_string_len_chars`,
  `spx_string_from_char`) only when a program reaches a v2 call. Borrowed
  operations free their staged input buffers at the operation site exactly
  like the first-wave reads; `string_from_char` allocates one fresh owned
  buffer from the scalar's UTF-8 encoding and consumes nothing.
- Wasm32 emits four host imports as one group directly after any first-wave
  imports (deterministic gap-free indexes computed from which groups are
  present). First-wave-only programs keep byte-identical modules, and
  programs without any operations keep byte-identical output on both
  backends.
- Interpreter evaluates all four inside the scalar profile with identical
  semantics (`chars().count()` for scalar counting).

### Evidence

`tests/string_ops_v2.rs` proves canonical round-trip, deterministic graph JSON
with pinned fragments for the four new identities (and their absence for
first-wave-only and operation-free programs), HIR binding with ownership
modes, stable diagnostics (argument type including the `char` parameter,
arity for one- and two-argument forms, reserved-name shadowing,
use-after-move behind a borrowed read), borrow non-movement, interpreter
agreement, native C11 O0/O2 execution equality over ASCII, empty strings,
whole-value prefixes, and 1–4-byte scalar content, Node/Wasm execution
equality, and the group-gating byte guarantees. `examples/string_ops_v2.spx`
is the canonical committed example exercised by the examples suite.
