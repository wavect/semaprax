# Owned String Loops v2

Audience: language users, agent authors, and compiler contributors.

Status: Partial — an additive bounded profile over
[Owned String Loops v1](OWNED-STRING-LOOPS-V1.md). The focused gates below
own local evidence; they do not establish hosted or public ABI support.

## User calls in loop bodies

A `while` or `for` body may call a monomorphic user function
whose result is a Copy scalar or `string`, and whose parameters are Copy
scalars, named borrowed byte slices or `str` views, or consumed strings. The
closed read-only effects `process.args.read`, `fs.read`, and
`process.environment.read` are admitted when ordinarily declared and permitted;
other effectful user calls stay refused. Body-local Strings stage
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
match may also form a `while` condition under the separate additive
[String Condition Lifetimes v1](STRING-CONDITION-LIFETIMES-V1.md) rules;
condition temporaries settle before the Boolean selection.

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

[Copy Variant Guards v1](COPY-VARIANT-GUARDS-V1.md) adds scalar-operator guards
on exact cases of these Copy-payload variants. Guarded cases contribute no
coverage: exhaustive unguarded fallback remains required (`SPX-M101`). Calls,
blocks, guarded wildcards/or-patterns and owned payload guards retain `SPX-T254`. Owned or borrowed non-Copy
scrutinees, including `Option<string>`, remain
`SPX-T252`. [Loop Copy Variant Construction v1](LOOP-COPY-VARIANT-CONSTRUCTION-V1.md)
adds direct concrete Copy-scalar variant construction in bodies and otherwise
admitted conditions. Other variant construction, records, postfix `?`,
generic calls outside an existing admitted intrinsic, and write-effect user calls
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
guard-only nonexhaustive variants and malformed patterns, effect/allocation
refusals, and hostile
HIR identity, field, type, and ownership controls. Wrong fields in this corpus
and `tests/language/text_toolkit_v1.rs` use the ordinary `SPX-M104` pattern
diagnostic instead of the former exact-shape admission message.

## Immutable input and borrowed views (#590)

Loop bodies admit repeated `args_len()` and dynamic `arg_utf8(index)` lookups
through the existing immutable invocation snapshot. The caller still declares
and receives `process.args.read`; out-of-range lookups select the existing
`semaprax.command-input.v1` failure, and reads do not mint or recharge roots.

Named borrowed `str` parameters and aliases may be passed to read-only user
helpers. `string_as_str(owner)` and `str_as_bytes(view)` may create local loop
views over exact unprojected named places; ordinary root provenance and loan
replay reject moved owners, temporary borrows, forged operations, and views
used after ownership changes. `file_read_text` and helpers declaring `fs.read`
retain the existing per-invocation file operation and cumulative byte budgets.
No new filesystem or process authority is granted, and `stdin_read` remains
outside loops. Existing single-write restrictions remain unchanged.

`tests/language/loop_command_input_v1.rs` owns canonical/graph and HIR replay,
interpreter command execution with explicit arguments and files, exact native
C11 `-O0`/`-O2` allocation settlement after two successful iterations and late
argument/file failures, source effect/ownership controls, and hostile HIR view
identity/projection/ownership controls. These additions do not establish
ordinary Core Wasm filesystem support or broaden the opaque internal-String
profile's borrowed-carrier support; that profile retains the exact `SPX-W111`
closed-signature refusal for the effectful helper corpus.
