# Generic Owned Result v1

Status: locally implemented for owned-Bytes success with all eight Copy error
substitutions and Bytes errors. Focused source/HIR, graph, workspace replay,
interpreter, native O0/O2 and Core-Wasm evidence is recorded with the change;
no hosted support claim.

## Semantic scope

This GEN-06 tranche admits explicit generic functions whose owning parameter
and result are the same authenticated compiler-owned `Result<Bytes, E>`.
`E` is one declaration-owned type parameter, explicitly instantiated as any
of the eight Copy scalars (`i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`,
`bool`) or `Bytes`. This is a real substitution in the residual carrier, not
an unused generic marker. Direct relay, exact acyclic generic forwarding,
and postfix `?` followed by `Ok` reconstruction compose in the function body.

```semaprax
@id("example.propagate")
fn propagate<E>(value: own Result<Bytes, E>) -> Result<Bytes, E> {
  let payload = value?;
  Result<Bytes, E>::Ok { value: payload }
}
```

The result type of `?` is owned `Bytes`; the operand and residual are the
same concrete Result type. On `Ok`, the operation moves the selected payload
into the success expression. On `Err`, it transfers the complete selected
carrier into the provisional function result. A Copy-scalar error has no
owned payload, but still has an authenticated case tag and residual value.
The absence of owned flags must never erase the error or authorize an `Ok`
payload. `Bytes` errors retain their guarded owned payload.

Requires and ensures retain the ordinary checked contract semantics. Failure
selection remains sticky; cleanup settles before result publication. Repeated
invocation must not retain allocations from success, residual return, or
contract failure. Live unrelated owners across `?` remain subject to the
existing ownership rule until a separately implemented extension admits them.

## Independent validation

Source admission authenticates the prelude Result identity and the parameter
owner/index. HIR rederives the exact substitution, signature, expression types,
case identities, moves, residual type and call closure. Materialization retains
both the `Try` and constructor nodes; it does not replace them with backend
layout facts. Unused templates are checked over the admitted substitutions.

Cleanup Inventory v2 and CleanupPlan v6 already represent a conditional case
with an empty owned-leaf list. Construction and independent replay preserve
that meaning and the existing canonical transition order. The profile uses
that contract rather than changing old plan bytes. A forged success ownership,
residual type, case field, selected case or cleanup transition rejects before
execution. The interpreter validates the selected payload against that case's
concrete type; native and Wasm authenticate the tag before payload access.

Graph v34 projects the concrete parameter/result ownership, ordered type
arguments, case-qualified leaves, conditional cleanup state, `Try` expression,
forwarding closure and selected v6 plan. ProgramRoot binds those facts through
its existing additive semantic-program node. Graph and root evidence remains
descriptive and is rederived from retained checked input.

## Required focused evidence

The regression corpus covers all nine error substitutions on both cases,
direct and transitive forwarding, repeated invocation, postcondition failure
on normal and residual returns, failure after success extraction, exact
cleanup, and graph/canonical source round-trip. Interpreter, C11 at O0/O2 and
Core Wasm must agree. Hostile HIR and cleanup mutations cover substituted
error types, foreign prelude case/field IDs, ownership, residual mismatch,
omitted transitions and invalid carrier tags or payloads.

Historical scalar and two-owned-Bytes Result checks retain their known answers.
The extension changes no public Project, package, C, C++, Rust, WIT, Component,
or registry signature. Copy success payloads,
nested Result payloads, ownership-equivalent reconstruction, multiple owners,
generic variants and collection payloads remain subsequent implementation
work in the full goal. This tranche alone does not complete GEN-06.
