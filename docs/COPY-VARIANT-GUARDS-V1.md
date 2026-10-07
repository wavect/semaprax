# Copy Variant Guards v1

Audience: language users and compiler contributors.

Status: Partial — this is a narrow addition to the general loop-match work
in #589. It does not complete arbitrary aggregate or ownership-changing guards.
The executable gate is `tests/language/guarded_copy_variants.rs`.

## Admission and evaluation

A plain value match over a nominal variant admits guards when every payload
field is a Copy scalar after concrete type substitution. This includes
`Option<u8>`, `Option<i64>` and `Result<i64, bool>`. Guarded patterns name one
exact case and bind its payload fields under the ordinary pattern rules.

A guard is a bool expression built from scalar literals, available scalar
bindings, and unary/binary operators. Lazy `&&` and `||` keep their ordinary
short-circuit behavior. Calls, blocks, aggregate projections and owned values
are outside this guard profile (`SPX-T254`); a scalar guard of the wrong result
type is `SPX-T256`. Owned or borrowed payload matches retain their refusals.
The separate scalar-scrutinee guard profile remains unchanged.

```text
match selected {
    Option::Some { value: byte } if byte == 255u8 => 1,
    Option::Some { value: byte } => 0,
    Option::None {} => 0,
}
```

The scrutinee evaluates once. Arms test in authored order; a case mismatch
skips its guard, while a matching case authenticates and binds the payload
before evaluating the guard once. A false guard falls through. Checked guard
failure selects the ordinary sticky status before any arm value is evaluated.

Guards contribute no exhaustiveness coverage. Unguarded cases, or a final
unguarded wildcard, must cover the variant (`SPX-M101`). Several guarded arms
for one case may precede its unguarded fallback; an arm following unguarded
coverage of that case is unreachable (`SPX-M102`). Guarded wildcards and
case-or patterns remain outside this narrow profile.

This rule applies in ordinary bodies and admitted loop bodies. Copy variants
remain available for later matches and later iterations. Arms may yield Copy
scalars or String under the existing match-result and iteration cleanup rules.
String arm values initialize only after guard success, and unconsumed owners
settle on success or selected failure. String values in while conditions retain
the separately specified condition admission rules.

## Authentication and cleanup

`src/source_verify/variant_guards.rs` and `src/variant_guards.rs` own source and
resolved shape predicates. Resolver and both HIR validators visit guards under
exact arm bindings and `.arm.N.guard` expression identities. Membership in the
shape profile grants no authority and skips no ordinary type, identity, scope,
operator, capability or ownership check.

The canonical builder and independent typed-HIR replay use existing
`VariantCase` observations followed by `BooleanResult` decisions. False guards
rejoin the next case decision; terminal guard failures cannot fall through.
Scalar-only guards introduce no owned temporary, cleanup region or transfer;
construction checks that their cleanup state remains exactly the decision-entry
state. Replay uses the same global materialization/path limits with a
conservative guarded-chain work census, without increasing a cap. These units
bound replay materialization work, not peak heap allocation.

Native lowering evaluates guards after payload binding and before arm-value
selection. The Wasm aggregate emitter uses one outer completion block and one
reject block per arm, emitting each guard and value once; it does not duplicate
later arms along false-guard branches. The interpreter's ordinary guard evaluator
uses the same admitted bindings. The change adds no syntax, HIR node, graph
schema, CleanupPlan schema, capability, public ABI or authority route.

## Focused gates and scope

`cargo test --locked -p semaprax --test language guarded_copy_variants::`
checks canonical/graph round trips, repeated Copy-variant matches, true/false
fallbacks, wrong-case skipping, lazy operands, checked guard failure, and String
results across the interpreter, C11 at O0/O2 with allocation/free accounting,
and repeated String-settling Core-Wasm calls. Hostile guards, missing fallback
coverage and modified cleanup decisions fail closed as `SPX-H006`.

`cargo test --locked -p semaprax --lib copy_variant_guards_match_recursive_oracle`
checks source-verifier parity for success and stable refusals. The existing
indexed-byte guarded near miss still lacks unguarded `Some` coverage and now
selects `SPX-M101`; its invalid field/type/effect/ownership controls stay intact.

The complete matched corpus avoids a guard-to-nested-if authoring rewrite for
sentinel and payload classification loops. That saves source scaffolding and a
refusal/repair turn; no measured token percentage or runtime speedup is claimed.
Owning payload guards, general guard calls or blocks, generic-function match
materialization, and broader aggregate/collection loop support remain open.
