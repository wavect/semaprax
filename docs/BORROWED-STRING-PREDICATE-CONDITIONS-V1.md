# Borrowed String Predicate Conditions v1

Audience: language users and compiler contributors.

Status: bounded additive source profile; current-head verification pending.
This is an additional narrow slice of #592. The existing named length profile,
loop syntax, cleanup-plan schema, interpreter entry points, native ABI, Wasm
profile selection, and capabilities remain unchanged.

## Admitted shape

A `while` condition may call these existing compiler-owned operations when
every String operand is an available, whole named binding:

| Operation | Exact source shape | Result |
| --- | --- | --- |
| `string_is_empty` | `string_is_empty(text)` | `bool` |
| `string_starts_with` | `string_starts_with(text, prefix)` | `bool` |
| `string_contains` | `string_contains(text, needle)` | `bool` |

The call has no type arguments. Each argument is a direct name, not a literal,
computed value, projected place, or block. These operations use borrowed String
parameters and return scalar booleans. Conditions retain ordinary left-to-right
and lazy Boolean evaluation, so an operand is read only when its call executes.

```text
let text = "abc";
let prefix = "a";
let needle = "b";
let mut i = 0;
while i < 2 && string_starts_with(text, prefix) && string_contains(text, needle) {
    i = i + 1;
    0
}
```

Each condition reads the current owner generation. A same-owner append in the
body is visible to the next condition. The read is synchronous: it creates no
String clone, retains no borrow across the condition, and initializes no
cleanup leaf.

## Authentication and backend behavior

Source admission recognizes only the three reserved operations with exact
arity and direct named operands. Typed HIR replay independently requires each
exact intrinsic identity, no generic instance or type arguments, a scalar Bool
Value result, and one unprojected Own String Place per Borrow parameter. The
condition-read identities are derived only from authenticated while-condition
trees; cleanup-plan construction and independent replay derive the same set.
The operation and ordinary String Place remain in HIR and the semantic graph.

The interpreter preserves the call and operand fuel charges and reads the
available environment owners directly, without UTF-8 materialization. Native
C11 and the existing String-settling Core Wasm backend already lower these
borrowed operations over the current String carriers. No loop back-edge,
condition cleanup region, ownership transition, public ABI, or schema change is
introduced.

## Still refused

String literals, computed or projected String operands, String-producing or
consuming operations, arbitrary user functions with String parameters, and
other String operations in conditions remain `SPX-T252`. In particular, this
profile does not permit `string_concat`, `string_slice`, or `string_trim` in a
condition. Such expressions need a distinct cleanup region that settles
before both Boolean outcomes and before the next condition evaluation. The
existing named String length profile remains the only admitted length form:
`string_len` must likewise take one direct named String Place.

Wrong intrinsic types and arities retain their ordinary source diagnostics;
malformed typed HIR fails independently with `SPX-H006`. An owner may be read
again after a same-owner append, but moving or replacing an outer owner in a
loop remains refused by the existing loop ownership rules.

## Focused gate

`tests/language/owned_string_loops_v1.rs` owns source admission and refusal,
canonical/graph preservation, interpreter/native C11 O0/O2/Core Wasm parity,
current-owner reinspection after append, allocation settlement, and hostile
HIR identity/type/mode/arity controls. `src/source_verify/iterative_verifier_tests.rs`
compares the added accepted and refused shapes against the recursive oracle.
`src/interpreter/string_conditions.rs` checks repeated predicate reads preserve
fuel behavior without per-condition UTF-8 materialization.
