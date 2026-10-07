# Loop Copy Variant Construction v1

Audience: language users, agent authors, and compiler contributors.

Status: authored additive admission and focused regression gate; executable
verification is pending. This extends the existing #589 loop-match profile,
not general aggregate execution or ownership-changing matching.

## Admission

`while` and borrowed scalar-Vec `for` bodies may construct a concrete variant
whose every payload field is a direct Copy scalar after type substitution.
This includes `Option<i64>` and `Result<bool, i64>`, and payload-free cases.
A body-local value can be matched repeatedly in that iteration. A constructor
may also be the direct scrutinee of an ordinary value match, including a
nested match with String arm results. Existing scalar-operator case guards and
exhaustive unguarded fallback remain required by [Copy Variant Guards
v1](COPY-VARIANT-GUARDS-V1.md).

```text
while i < 3 {
    let piece = match (Option<i64>::Some { value: i }) {
        Option::Some { value: n } if n == 1 => "ab",
        Option::Some { value: n } => "x",
        Option::None {} => "bad",
    };
    out = string_concat(out, piece);
    i = i + 1;
    0
}
```

A scalar match condition may inspect a directly constructed Copy variant when
its whole condition is otherwise admitted. Allocating String expressions in
conditions retain `SPX-T252`; the existing exact named String Len inspection
has its own [condition profile](STRING-LENGTH-CONDITIONS-V1.md).

The field expressions undergo the ordinary loop scan and ordinary constructor
checks. Source and HIR still authenticate the declaration, concrete arguments,
case identity, field inventory, value types and Value ownership. A variant
with any non-Copy payload remains `SPX-T252`, including an active payload-free
case of `Option<string>`. Records, nested aggregate payloads, general
aggregate-returning user calls, unsafe boundaries and postfix `?` retain their
existing loop refusals. General guard calls/blocks and owned payload guards
remain `SPX-T254`. Missing unguarded variant coverage remains `SPX-M101`.

## Evaluation, cleanup and projections

Construction evaluates field expressions once, left to right in authored
order. Matching evaluates that constructed scrutinee once, then case tests,
guards and arm values in authored order. A skipped case cannot evaluate its
guard or value; a reached false guard falls through. Checked operand, guard or
arm failure preserves its selected status and prevents later evaluation.

The Copy variant adds no owned storage leaf, transfer or finalizer. Existing
owned temporaries within admitted scalar field expressions, such as
`string_len("abc")`, and String arm results keep their ordinary canonical
regions and settlement order. Every successful iteration preserves outer
ownership; selected failure settles the existing live owners. The plan
builder, independent replay, loan validator and backend consumers remain
unchanged. Cleanup-plan vectors are neither sorted nor repaired.

No graph or CleanupPlan schema changes: constructors, match observations,
guard edges, loop nodes and String cleanup facts already have these exact
meanings. Canonical source and graph round trips remain required. Programs
that do not use the newly admitted shape keep their existing projections.

The interpreter and native C11 use existing variant construction/matching.
The explicit [standalone Copy-variant String Wasm
profile](WASM-INTERNAL-STRING-COPY-VARIANTS-V1.md) admits the new non-Vec corpus
under its unchanged private-carrier and quota contract. Its scalar facade
exports no variant or owned String. Vec `for` traversal remains outside that
Wasm profile with exact `SPX-W111`; the old internal-String entry also retains
its nominal `SPX-W111` refusal. No backend profile is selected implicitly.

## Focused gate

`cargo test --locked -p semaprax --test language guarded_copy_variants::`
retains the seven previous expectations and adds repeated construction/reuse,
direct nested Result scrutinees with String arms, String temporaries in a
constructor operand, a scalar constructor-match condition, late guard failure,
constructor-operand failure and borrowed Vec `for` traversal. It compares
interpreter outcomes with native O0/O2, repeated invocations, String/Vec
allocation accounting and empty Vec authority. The non-Vec cases additionally
execute repeatedly on the explicit Wasm copy profile; the Vec case has an
exact profile refusal. Source diagnostic locations and hostile HIR type,
ownership, member and payload forgeries remain checked.

`cargo test --locked -p semaprax --lib loop_copy_construction_matches_recursive_oracle`
checks independent source-verifier parity for successes and refusals. The
ordinary match and owned-liveness hostile cases remain regression obligations.
No provider trial or measured agent-cost improvement is claimed by this gate.
