# Typed workspace request clients v1

Status: implementation plus Python, Rust, and provisioned TypeScript request-admission evidence authored; expanded gate unrun.

Audience: agent client authors, editor integrators and compiler contributors.

The v5 client generator provides additive structural types for complete selected
request parameters, including recursively nested intentions, expressions and
recovery objects. These types come from the existing compiler-owned schema
bundle. They do not introduce another constructor grammar or replace compiler
admission.

## Additive API

Each selected method receives `<Method>TypedParams` and
`request_<method>_typed`. For example, `candidate/apply-intent` exposes
`CandidateApplyIntentTypedParams` and `request_candidate_apply_intent_typed`.
The existing `CandidateApplyIntentParams`, request builder, generic result
decoder and [typed response decoder](IMAGE-TYPED-RESPONSE-CLIENTS-V1.md) remain
available. Disabled methods receive no typed request helper.

The new builder serializes the typed parameters through the same selected-method
request function as the old builder. Request IDs, outer parameter checks and
the 64 KiB request limit keep their existing behavior. Builders perform no I/O,
choose no host policy and cannot enable a disabled operation.

```typescript
const params: CandidateApplyIntentTypedParams = {
  image_revision: imageRevision,
  candidate_revision: candidateRevision,
  intent: {
    kind: "replace_function_body",
    target: "calculator.add",
    body: {
      kind: "let",
      name: "answer",
      value: { kind: "i64", value: 7 },
      body: { kind: "place", name: "answer" }
    }
  }
};
const frame = request_candidate_apply_intent_typed("edit-1", params);
```

This constructs a request frame; it does not apply, verify or publish the change.
The caller must use revisions returned by its live session and a target admitted
by that candidate. A described constructor can still fail scope, type, ownership,
effect, contract, cleanup or target checks.

## Schema ownership and recursion

The request model follows the selected methods' parameter schemas and resolves
absolute schema identifiers and document-local `$defs` references within the
provided bundle. Local definitions retain their document scope. Unselected
documents do not introduce extra method helpers or constructor authority.

Named shapes are reserved before following their dependencies, allowing finite
recursive values such as nested calls, bindings and conditional expressions.
Unguarded cycles through only aliases and unions fail generation; recursion
must pass through a value structure such as an object or array.
Missing references and unsupported shapes also fail closed. Deterministic
`RequestType` names belong to the generated artifact, not to semantic identities;
the method aliases are the public entry points.

Object fields, required versus optional properties, nullable alternatives,
arrays, literal choices and unions retain their structural meaning. Types do
not prove numeric or byte bounds, name exclusions, uniqueness, expression budgets
or `oneOf` exclusivity. The existing builder validates the outer request only;
the compiler still validates the complete nested constructor. Directly creating
or deserializing a generated type is not an admission API.

## Language boundaries

TypeScript uses recursive structural types and literal discriminants. Its number
type does not represent every exact JSON integer, and static types cannot prove
integer ranges. Callers remain responsible for avoiding values already rounded
by JavaScript before serialization.

Python 3.11 uses functional typed dictionaries and forward references for recursive
fields and alternatives. Required and optional fields remain distinct. These are
annotations, not runtime constructor validation; the compiler still decides
whether the supplied dictionary describes an admitted change.

Rust uses request-specific integer and optional-presence helpers. Boxed named
edges and transparent wrappers permit recursive types without changing their
JSON representation. Closed object types reject unknown fields on direct serde
deserialization, but this is only part of the compiler's grammar and admission
contract. Signed and unsigned JSON integers retain their representable range.

## Bounds and evidence

The model bounds schema traversal and generated type source: 4,096 definitions,
65,536 visits, depth 128, 16 MiB of retained schema keys and 900 KiB of generated
type source. The existing complete discovery payload bound still applies to the
combined generated client. These are generation limits, not a guarantee of total
process memory, runtime recursion depth or latency.

Focused model/emitter regressions and `tests/image_typed_request_clients_v5.rs`
author recursion, local-scope resolution, deterministic selected profiles,
legacy helper preservation and nested request submission. Python, Rust, and provisioned TypeScript harnesses are authored to compile or resolve their generated public types, emit exact request frames, submit them to ordinary compiler admission, and require hostile unbound places to reject. The expanded exact-subject gate remains unrun. These selected request paths do not establish a complete SDK, every method, external package ergonomics, hosted/cross-platform behavior, or completion-matrix promotion.
