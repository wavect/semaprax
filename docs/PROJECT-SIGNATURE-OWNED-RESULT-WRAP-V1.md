# Project signature owned-result wrapping v1

Status: **Partial; library implementation and regressions are authored and
unrun.** This stage is a closed local-candidate migration. It is not an ABI,
package-consumer, deployment, runtime-equivalence, or compatibility claim.

Audience: compiler contributors, SDK integrators, and candidate reviewers.

The ordered `parameters` form of `change_function_signature` may carry one
additional closed member:

```json
{
  "kind": "change_function_signature",
  "target": "provider.function.id",
  "parameters": [{"from": "input"}],
  "wrap_return": {
    "record": "wrapper.record.id",
    "field": "wrapper.record.field.id"
  }
}
```

The target must be an explicit monomorphic function whose complete result is
an owning `Bytes` or `string`. It may have no requires or ensures clause and
must not be either Project entrypoint or a manifest web export. The selected
wrapper must be an existing, visible, explicit, monomorphic record with exactly
one explicit field. Retained checked HIR must prove that field has the exact
original result type and that the record is sized, resource-free, non-Copy,
and cleanup-owning. Record and field display spellings come from authenticated
source bindings; request bytes select stable identities only.

Admission authenticates the provider source and checked signature, the whole
record and field shape, and every stable-ID-bound local call inventory before
mutation. Every provider parameter must match retained HIR elementwise in
name, span, ownership, and recursively resolved type. Bare source `string`
normalizes only to checked owning `String`; all legacy scalars/views and
ordered nominal owners keep their exact ordinary ownership and type identity.
Calls from contracts are rejected. At least one local body call is required,
and the authenticated count must equal the rewritten count.

The provider wraps its former body exactly once in the selected record
constructor at the existing result position. Each local caller keeps its
existing left-to-right argument staging and call commit, then immediately
projects and moves the sole field. The call is evaluated once. The provider
record owns the result until publication; after projection, ordinary caller
cleanup owns the moved `Bytes` or `string` and cleans the empty record according
to the compiler's existing structural plan. Full Project reconstruction
rechecks HIR, ownership, cleanup, interpreter admission, native emission, Wasm
emission, manifests, and target admission before the candidate is observable.

The lane rejects borrowed results, projected provider results, generic or
resource-bearing wrappers, multi-field or mismatched wrappers, implicit
identities, contract occurrences, exported providers, zero-local-caller
changes, unknown mapping fields, and caller-inventory drift. It does not search
or rewrite external source, generated SDKs, packages, deployed consumers,
reflection, network providers, or runtime data. Package-consumer analysis and
replay are separate explicit APIs; this lane neither invokes them nor claims a
package conflict gate, compatibility result, or automatic consumer migration.

`SPX-G494` owns unsupported wrapper shape and scope, `SPX-G495` owns retained
source/HIR authentication failures, and `SPX-G496` owns caller inventory and
migration failures. The closed intent schema and candidate catalogue expose
the same two stable selectors and exclusions.

Authored, unrun regressions in
[`tests/project_candidate/signature_ownership.rs`](../tests/project_candidate/signature_ownership.rs)
cover Bytes and String wrappers, legacy borrowed parameters, bare owning String
parameter normalization, provider construction, caller projection,
exact candidate replay, cleanup-bearing Project admission, wrong-type and
multi-field rejection, and unchanged candidate state after rejection. Full
Project replay is the authored interpreter/native/Wasm admission oracle; no
test target, backend artifact, benchmark, or local runtime was executed for
this change.
