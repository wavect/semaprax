# Named String Length Conditions v1

Audience: language users and compiler contributors.

Status: bounded additive source profile, gated by the tests below. This is
the first narrow slice of #592; general String evaluation in loop conditions
remains open. Existing interpreter entry points, native ABI, Wasm profile
selection, schemas, and capabilities remain unchanged.

This versioned profile remains limited to `string_len`. The separate additive
[Borrowed String Predicate Conditions v1](BORROWED-STRING-PREDICATE-CONDITIONS-V1.md)
admits three exact named-owner Boolean reads without widening this profile's
length rule or enabling general String evaluation in conditions.

## Admitted shape

`string_len(text)` may occur in a `while` condition when `text` is an
available whole named binding of type `string`, including an owned String parameter
where the selected backend already admits that signature. It returns the
current UTF-8 byte length. Scalar arithmetic, comparisons, branches, and lazy
boolean expressions retain their ordinary evaluation order and failures.

```text
let mut log = "";
while string_len(log) < 1000 {
    log = string_concat(log, "a");
    0
}
```

Each condition reads the current owner, including the generation published
by the previous body's same-owner append. The inspection creates no String,
consumes no owner, retains no view, and initializes no cleanup leaf. A false
condition does not enter the body; a checked scalar failure settles the live
owner through its ordinary failure exit before returning the selected status.

## Authentication and cleanup

Source admission recognizes only the reserved `string_len` operation with
one direct name and no type arguments. Ordinary source verification checks
its type and availability. Resolver condition admission shares this exact
source predicate; the recursive source oracle shares it too.

HIR retains the ordinary `core.string.len` call and the binding's Own Place
operand. A derived condition-read set requires the exact intrinsic identity,
one unprojected String Place, exact ownership, no generic instance/type
arguments, and an i64 Value result. Full HIR replay independently checks the
binding's authenticated type, scope, availability, and expression identities.
The set comes only from actual while-condition trees; body reads and ordinary
straight-line calls retain their existing clone behavior.
If a condition block contains a nested while statement, its condition may use
this inspection, but its body is outside the outer condition's derived read
set. String operations in that nested body, including further nested loop
conditions, remain source `SPX-T252` even when the body would be skipped.

The cleanup builder and independent replay derive the same inspected reads
from typed HIR, rather than accepting attached plan claims. Such a read has
neither clone initialization nor an owned transfer source. Its inactive
inventory temporary retains an authenticated lexical region, like the moving
operand of a same-owner append, and never becomes live. The existing acyclic one-iteration loop plan remains sufficient; no condition cleanup
region, back-edge, or schema extension is introduced.

The derived identity sets are metadata, like the existing append index. Replay
materialization units count path-evidence operations, not these sets' bytes,
peak heap, or traversal time; they grant no new cleanup authority. Builder
capacity telemetry includes the retained inspected-read and append identities.

Native C11 and admitted Core Wasm read the existing physical carrier directly.
The interpreter reads the available environment owner directly while retaining
the operand expression's fuel and trace charge. UTF-8 materialization limits
are not charged for this inspection. Dynamic semantic-work charges for loop
body entry remain unchanged.

## Still refused

String literals, produced strings, consuming String operations, projected
owners, blocks yielding a String, other String readers such as
`string_is_empty` under this length-only profile, and String-signature user
calls in conditions remain `SPX-T252`. Wrong intrinsic argument types and arity retain
ordinary type diagnostics. A moved owner is still a compile-time ownership
error; malformed HIR remains `SPX-H006` before backend evaluation.

Permitting allocating condition expressions would require a separate cleanup
region that settles before both Boolean outcomes and before the next condition
evaluation. This profile does not provide that broader #592 behavior.

## Focused gates

`tests/language/owned_string_loops_v1.rs` covers canonical/graph round trips,
absence of clone initialization, repeated length reads, UTF-8 byte length,
owner growth through append, an initially false condition, and checked failure
in the condition. The existing interpreter/native C11 O0/O2/Core-Wasm cases
run the added corpus. Native allocation accounting requires the repeated
read-only case to allocate only its initial owner. Hostile HIR operands and
a forged clone initialization fail validation independently; negative source
cases retain allocating, consuming, and guard-contained String refusals.

`source_verify::iterative_verifier_tests` compares the narrow profile with the
recursive oracle for success, wrong types/arity, allocation, and match guards.
`interpreter::string_conditions` additionally checks exact logical UTF-8
materialization usage and exhausted-fuel behavior for repeated inspection.
Wasm remains limited to the selected backend profile, with repeated invocations
and its normal String settlement checks.
