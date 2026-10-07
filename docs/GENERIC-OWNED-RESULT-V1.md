# Generic Owned Result v1

Status: implemented private profile; **HOSTED GREEN** under the
[accepted v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
Admission covers owned-Bytes success with all eight Copy error substitutions
and Bytes errors, and all eight Copy success substitutions with owned-Bytes
errors. Public generic ABI and support remain separately gated.

Audience: compiler contributors and language reviewers.

Historical local source/HIR, graph, workspace replay, interpreter, native O0/O2
and Core-Wasm witnesses remain attached to their original execution; they are
not a new test run or the current evidence ceiling.

## Semantic scope

GEN-06 admits explicit generic functions whose owning parameter and result
share the authenticated compiler-owned `Result<Bytes, E>` type.
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

Here, `?` produces owned `Bytes`. Its operand and residual have the same
concrete Result type. On `Ok`, it moves the selected payload into the success
expression. On `Err`, it transfers the complete selected carrier into the
provisional function result.

A Copy-scalar error has no owned payload, but it still has an authenticated
case tag and residual value. Missing owned flags must never erase that error
or authorize access to an `Ok` payload. A `Bytes` error keeps its guarded
owned payload.

Requires and ensures use the ordinary checked contract rules. Once selected,
a failure cannot be replaced; cleanup finishes before result publication.
Repeated calls must retain no allocations from success, residual return, or
contract failure. The existing ownership rule still applies to unrelated
owners live across `?`; widening it requires a separate implemented extension.

## Copy success and owned error

The complementary explicit `Result<T, Bytes>` profile admits all eight Copy
substitutions for `T`. Its generic declaration is instantiated only with those
Copy types. The owning operand and the success payload have separate modes:
`?` consumes the Result, but produces a Copy `T` on `Ok`. An owning operand
must not cause its Copy success value to acquire a cleanup slot or move flag.

Cleanup v6 authenticates the selected Ok case even though its owned-leaf list
is empty. It consumes the conditional owner and exposes the scalar payload
without an owned transfer. The Err edge retains the whole-carrier transfer
into provisional result storage. Native and Wasm route this operation by
operand ownership, then extract the Copy scalar after tag authentication.
Interpreter payload validation independently checks either selected field's
concrete type. Both normal and residual postcondition failures settle the
correct guarded case before returning the sticky failure.

The complementary runtime matrix covers eight success types and five
settlement profiles on interpreter, native O0/O2 and Core-Wasm. Hostile
invented Ok flags and missing residual transitions reject before execution.

## Changed success type with an identical error

A bounded local extension admits owned `Result<T, E>` propagation into
`Result<U, E>` when both carriers belong to the existing scalar/Bytes profile
and retain the exact same error type and compiler-owned case/field identities.
The normal expression has type T; the residual constructs the enclosing
Result's Err case with the moved E payload. It does not reinterpret the entire
operand carrier as the destination carrier. Error-type changes remain refused.

For example, an owned `Result<Bytes, Bytes>` may return
`Result<bool, Bytes>` after observing and settling the extracted success Bytes.
Unrelated normal-path owners settle in canonical order before joining the
residual epilogue. Err bypasses every later normal expression. Postconditions
and non-result cleanup precede result publication, and the selected failure
remains sticky. Typed residual reconstruction is independently replayed by HIR
and cleanup validation and implemented by interpreter, native C11 and Core Wasm.

The extension changes no public generic ABI and does not promote prior hosted
support. Its focused local gates are
`language::owned_result_variants::owned_result_try_reconstructs_identical_error_with_changed_success_type`
and `owned_data::generic_owned_function_runtime::changed_success_result`.

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
or registry signature. Nested Result payloads and broader Result/collection
payload composition remain outside this profile.

The released implementation separately includes
[record reconstruction](GENERIC-OWNED-RECORD-COMPOSITION-V2.md),
[multi-owner records](GENERIC-MULTI-OWNER-RECORDS-V1.md),
[authored generic variants](GENERIC-AUTHORED-VARIANTS-V1.md), and
[compiler collections](GENERIC-COMPILER-COLLECTIONS-V1.md). These are no longer
wholly future implementation tasks, but their owning contracts do not widen
this Result profile. This tranche alone does not complete GEN-06.
