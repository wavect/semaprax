# Generic Argument Inference v2

Status: local, partial. Thirteen source/HIR/graph checks, two independent
inference checks, two private ProgramRoot replays and both owned runtime corpora
pass. Interpreter, native O0/O2 and Core Wasm agree, including evaluation-once
probes and contract-failure cleanup. Hosted and public support are not claimed.

Audience: compiler contributors and reviewers.

This additive profile extends [v1](GENERIC-ARGUMENT-INFERENCE-V1.md) to complete
ordered vectors for already admitted generic functions and additional expression
type facts. A monomorphic caller may omit the whole vector; every declared type
parameter must be determined by the argument types. Positions follow declaration
order, never discovery order. Repeated observations must agree exactly.

Inference unifies whole formal and actual types. Nominal declaration identities
and ordered nominal arguments must agree. Only parameters owned by the callee
are unification variables. The existing generic-function admission independently
checks the resulting candidate vector; inference neither broadens a template's
allowed substitutions nor adds new generic body forms.

Evidence includes v1 scalar literals, checked local and parameter bindings,
and explicitly typed record and variant constructors. The extension also uses
unary and binary expression types, equal conditional branch result types, and
declared ordinary function result types with complete authored generic vectors
and exact substitution. Empty statement blocks expose their tail type. Nested
calls whose own generic vectors are omitted do not yet supply evidence. Evidence
traversal permits at most 4,096 visited evidence expressions across a call and
128 levels including each argument root. First-over-bound evidence rejects
with SPX-T225. It never executes expressions, performs ownership moves,
activates loans, or checks a body speculatively. The ordinary verifier and HIR
resolver still visit arguments once, left to right, with the same ownership
commit boundary and failure selection as an explicit call.

Unary evidence supports signed numeric negation and boolean not. Arithmetic
`+`, `-`, `*`, `/` requires equal numeric operands; `%` evidence is i64-only.
Ordering uses equal ordered scalar types; equality uses equal type facts and
still defers legal equality admission to the ordinary checker. Boolean operators
require boolean facts. A conditional inspects both branch types without running
either branch, and requires a boolean condition fact.

A type fact is not an authority grant. Invalid operand types, moved locals,
unauthorized effects, failing contracts, and invalid constructor payloads remain
subject to ordinary verification and execution. In particular, knowing a called
function's result type neither executes it early nor skips its normal checking.

Source verification and HIR resolution independently derive the complete vector.
HIR compares resolved declaration identities. Calls retain the same concrete
instance identity, ownership and cleanup facts as their explicit counterparts.
Canonical formatting preserves authored omission. Each graph and ProgramRoot
remains bound to its own exact source; an explicit source root cannot authenticate
an inferred-source revision merely because their instance closures agree.

Conflicting, missing, partial, surplus or unsupported inference remains a stable
SPX-T225 rejection. Generic-caller omission remains outside this version because
template cycle and forwarding prechecks must derive the same symbolic evidence
before that can be admitted. Return-context inference, constraints, overloads,
generic methods, intrinsic inference, field projections, scoped block bindings
and match evidence remain open. Public generic signatures are unchanged.

## Focused evidence

The existing named Linux `GEN-06 exact argument inference` selector owns source,
HIR and graph replay, generic-call hostility, private workspace ProgramRoot
replay and owned runtime settlement. Its local commands are:

```sh
cargo test --locked --offline -p semaprax --lib generic_inference
cargo test --locked --offline -p semaprax --test language generic_argument_inference
cargo test --locked --offline -p semaprax --test language generic_function_hostiles_are_stable_and_fail_closed
cargo test --locked --offline -p semaprax --test workspace inferred_generic_instance
cargo test --locked --offline -p semaprax --test owned_data generic_owned_function_runtime::inference
```

Local results do not establish hosted or public support. Earlier graph, cleanup,
prelude and public descriptor schemas retain their existing meaning.
