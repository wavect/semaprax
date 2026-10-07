# Owned String Loops v2

Audience: language users, agent authors, and compiler contributors.

Status: Partial — an additive bounded profile over
[Owned String Loops v1](OWNED-STRING-LOOPS-V1.md). The focused gates below
own local evidence; they do not establish hosted or public ABI support.

## User calls in loop bodies

A `while` or `for` body may call a monomorphic, effect-free user function
whose result is a Copy scalar or `string`, and whose parameters are Copy
scalars, borrowed byte slices, or consumed strings. Body-local Strings stage
left to right and transfer together at the call's commit boundary. Unused
results settle with the iteration. Consuming an outer String without a
recognized same-owner reopen changes loop-entry ownership and is refused
with `SPX-T252`.

The source verifier, recursive oracle, HIR resolver, and independent HIR
validator share the signature admission predicates in `src/loop_calls.rs`.
Each `while` body undergoes ordinary type and ownership checking; failures
retain source locations and their ordinary stable diagnostic codes.

## Matches in loop bodies

A match scrutinee may be a Copy scalar or a variant whose every payload field
is a Copy scalar after concrete type substitution. Payload-free variants and
`Option<i64>` or `Option<u8>` qualify. A preexisting Copy variant binding can
be matched on every iteration and after the loop. Scrutinees evaluate once;
scalar guards and arms follow authored order, with the ordinary match exhaustiveness,
unreachable-arm, field-type, identity, and ownership checks.

Arms may return a Copy scalar or `string`. A String result joins and settles
in the per-iteration region, including on a failing guard or arm. A scalar
match may also form a `while` condition if the entire condition satisfies the
existing restriction against String values.

```text
let selected = Option<i64>::Some { value: 7 };
let mut i = 0;
let mut total = 0;
while i < 4 {
    total = total + match selected {
        Option::Some { value: n } => n,
        Option::None {} => 1000,
    };
    i = i + 1;
    0
}
```

Guards over variant scrutinees retain `SPX-T254`; guards are admitted only
for the existing Copy-scalar match profile. Owned or borrowed non-Copy
scrutinees, including `Option<string>`, remain
`SPX-T252`. Variant construction inside an iteration, records, postfix `?`,
generic calls outside an existing admitted intrinsic, and effectful user calls
retain their refusals. This widening changes no graph or CleanupPlan schema:
ordinary match decisions and per-iteration cleanup facts retain their existing
meaning. Loop-entry ownership must still equal successful body-exit ownership.

## Focused gates

`tests/language/owned_string_loops_v2.rs` owns canonical and graph round trips,
user calls, scalar guarded String matches, a repeatedly matched Copy variant,
source ownership refusals, and failing-arm settlement. It compares interpreter
results with native C11 at `-O0`/`-O2` under allocation/free accounting;
literal-only scalar cases additionally execute on the String-settling Core
Wasm profile with repeated calls. Numeric text and nominal variants retain
their backend-specific profiles.

`tests/language/indexed_byte_loops_v2.rs` retains the exact byte-read corpus,
guarded-variant and malformed patterns, effect/allocation refusals, and hostile
HIR identity, field, type, and ownership controls. Wrong fields in this corpus
and `tests/language/text_toolkit_v1.rs` use the ordinary `SPX-M104` pattern
diagnostic instead of the former exact-shape admission message.
