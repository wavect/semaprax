# Conformance trace v1

Status: the public Rust data model, deterministic JSON projection, independent
attached-plan replay, scenario-driven single-frame reference executor, and
native scalar status/out execution are implemented. The narrow public
`semaprax.wasm-owned.v1` path emits compiler-generated semantic ordinals from
real Node/Wasm control flow. The private generated-C lane now executes the same
authoritative 14-case corpus through an exact loader lease, authority, ledger,
and generated callable at O0/O2. Its host authenticates the event dictionary
and compiler-owned trace-path certificate before materializing events. Native
host and Wasm outcomes, publication, logical liveness, and complete traces
match the reference exactly. Recursive reference-callee execution,
callable-import execution, public callable-native resource execution, and
general-shape backend conformance remain unimplemented. The public Linux
[dynamic-provider ASan/UBSan job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 cases through the Rust host. It does not
sanitizer-instrument the Rust host code; unrelated Clippy/GCC failures kept the
overall workflow run red. The separate pinned-nightly [Rust-host ASan
job](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065)
later passed with the Rust host itself instrumented. Rust-host UBSan and general
backend conformance are not inferred.

This document fixes the current public wire projection defined by `src/conformance.rs`. It complements [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md). The native scalar backend executes the status/out portion. For the admitted owned-resource slice, generated native C and Wasm emit dictionary ordinals that the host materializes into this protocol; the 14-case equality is executable evidence for that slice, not a claim of general or production native resource conformance.

## Normalized status

`NormalizedStatus::to_json` emits one `semaprax.status.v1` object in this canonical field order:

```json
{"schema":"semaprax.status.v1","domain_id":"semaprax.contract.v1","code":1,"class":"contract","retryable":false}
```

The fields are:

- `schema`: exactly `semaprax.status.v1`.
- `domain_id`: a stable semantic domain identity encoded as 1–255 UTF-8 bytes with no embedded NUL. The bound is measured in bytes, not Unicode scalar values, and is shared by source/HIR validation and every target adapter.
- `code`: a nonzero unsigned 32-bit domain code. This is not the context-local ABI status token; token zero means success, while no `NormalizedStatus` represents success.
- `class`: one of `contract`, `arithmetic`, `import`, `explicit_close`, or `adapter`.
- `retryable`: JSON `true`, JSON `false`, or the string `"unknown"`.

Opaque diagnostic detail, localized text, backtraces, exceptions, and context-local tokens are not serialized.

The narrow Wasm owned adapter returns a frozen JavaScript object with this same
field spelling and order, including `domain_id`; its nonzero numeric status
token remains private to the instance arena. Its separate local
commit/drop/publish audit events contain physical handles and are intentionally
excluded from this protocol. Semantic evidence comes instead from the returned
compiler-generated ordinal sequence and its exact authenticated dictionary.

Compiler-owned mappings are fixed:

| Domain | Code | Meaning | Class | Retryable |
| --- | ---: | --- | --- | --- |
| `semaprax.contract.v1` | 1 | `requires_false` | `contract` | `false` |
| `semaprax.contract.v1` | 2 | `ensures_false` | `contract` | `false` |
| `semaprax.arithmetic.v1` | 1 | `add_overflow` | `arithmetic` | `false` |
| `semaprax.arithmetic.v1` | 2 | `sub_overflow` | `arithmetic` | `false` |
| `semaprax.arithmetic.v1` | 3 | `mul_overflow` | `arithmetic` | `false` |
| `semaprax.arithmetic.v1` | 4 | `division_by_zero` | `arithmetic` | `false` |
| `semaprax.arithmetic.v1` | 5 | `division_overflow` | `arithmetic` | `false` |
| `semaprax.arithmetic.v1` | 6 | `remainder_by_zero` | `arithmetic` | `false` |
| `semaprax.arithmetic.v1` | 7 | `remainder_overflow` | `arithmetic` | `false` |
| `semaprax.arithmetic.v1` | 8 | `negation_overflow` | `arithmetic` | `false` |

Import, explicit-close, and adapter statuses retain their declared domain and adapter-normalized nonzero code. A consumer must validate that their class, domain, code, and retryability agree with the resolved interface and adapter contract.

The public adapter/import constructor rejects the compiler-owned `contract` and `arithmetic` classes and both `semaprax.contract.v1` and `semaprax.arithmetic.v1` domains. Only the exact compiler constructors can create those mappings. This prevents untrusted adapter data from impersonating a compiler failure; binding-contract validation is still required for non-reserved domains.

## Trace envelope and identities

`ConformanceTrace::to_json` emits:

```text
{
  "schema": "semaprax.conformance-trace.v1",
  "scenario_id": string,
  "root_function": DeclarationId,
  "events": [TraceEvent...],
  "outcome": TraceOutcome
}
```

Canonical field order is `schema`, `scenario_id`, `root_function`, `events`, then `outcome`. The schema is exactly `semaprax.conformance-trace.v1`.

`scenario_id` identifies the conformance fixture. `root_function` and every event `function` are resolved declaration IDs, never display names. Each event has an `invocation` array containing the revision-scoped call-expression IDs from the root frame to that frame. The root path is `[]`; a nested call appends its call expression ID. Repeated IDs may occur when future recursive execution revisits the same call site.

Expression and value IDs are revision-scoped HIR identities. Lifecycle, import, field, callee, function, and owned-result type IDs are `DeclarationId` values. An owned result currently identifies its nominal resource declaration directly; it does not contain a payload or instantiated layout.

### Status sources

A selected cleanup status source is encoded as:

```json
{"kind":"status_source_id","expression":"declaration:8:app.main:expression:4:body","lane":"operation_failure"}
```

`lane` is `operation_failure` or `contract_false`. Contract phase and ordinal are represented by the validated cleanup plan's status producer, while the trace retains the exact expression/lane source selected in each frame.

### Storage and places

All ownership events reuse Graph-v6 cleanup storage and place identities:

```json
{"kind":"value","value":"resolved-value-id"}
{"kind":"temporary","expression":"resolved-expression-id"}
{"kind":"call_argument","call":"call-expression-id","parameter_index":0,"value_expression":"argument-expression-id"}
{"kind":"provisional_result"}
```

A place wraps one storage identity plus an ordered field-declaration path:

```json
{"kind":"cleanup_place","storage":{"kind":"provisional_result"},"projections":["outer.inner","inner.value"]}
```

Projection arrays are semantic path order and must not be sorted.

## Events

Every event begins with `kind`, `function`, and `invocation` in that order. Variant fields follow in the order shown below. The `events` vector is execution order and must never be sorted, deduplicated, grouped, or repaired.

### `initialize`

Marks an owned result or temporary live only after its producing operation succeeds.

```json
{"kind":"initialize","function":"app.make","invocation":[],"at":"expression-id","destination":{"kind":"cleanup_place","storage":{"kind":"temporary","expression":"expression-id"},"projections":[]}}
```

### `transfer`

Atomically clears the source ownership epoch and initializes the destination epoch.

```text
{"kind":"transfer","function":DeclarationId,"invocation":[ExpressionId...],"at":ExpressionId,"source":CleanupPlace,"destination":CleanupPlace}
```

### `call_commit`

Records the single atomic caller-to-callee ownership boundary after all arguments have evaluated.

```text
{
  "kind":"call_commit",
  "function":DeclarationId,
  "invocation":[ExpressionId...],
  "call":ExpressionId,
  "callee":DeclarationId,
  "arguments":[
    {"kind":"call_argument_transfer","parameter_index":u32,"source":CleanupPlace}
  ]
}
```

Arguments remain in semantic parameter order. The array contains every and only transferred `own` argument epoch.

### `import_begin`

Begins either an ordinary callable import or an imported automatic finalizer:

```json
{"kind":"import_begin","function":"app.read","invocation":[],"site":{"kind":"call","expression":"call-expression-id"},"import_id":"io.read"}
```

```text
{"kind":"import_begin","function":DeclarationId,"invocation":[ExpressionId...],"site":{"kind":"finalizer","source":CleanupPlace,"lifecycle_id":DeclarationId},"import_id":DeclarationId}
```

### `import_end`

The wire kind is `import_end`, but the Rust model deliberately has two completion variants.

An ordinary callable import carries an operation outcome:

```text
{"kind":"import_end","function":DeclarationId,"invocation":[ExpressionId...],"site":{"kind":"call","expression":ExpressionId},"import_id":DeclarationId,"outcome":{"kind":"success"}}
```

or:

```text
{"kind":"import_end","function":DeclarationId,"invocation":[ExpressionId...],"site":{"kind":"call","expression":ExpressionId},"import_id":DeclarationId,"outcome":{"kind":"failure","status":NormalizedStatus}}
```

An imported automatic finalizer is success-only:

```text
{"kind":"import_end","function":DeclarationId,"invocation":[ExpressionId...],"site":{"kind":"finalizer","source":CleanupPlace,"lifecycle_id":DeclarationId},"import_id":DeclarationId,"outcome":{"kind":"success"}}
```

The public Rust type cannot attach a failure outcome to finalizer completion. A finalizer unwind, throw, or trap is an adapter-conformance failure outside the language status protocol; it must not replace a selected failure.

### `select_failure`

Records the frame-local, write-once failure selection:

```text
{"kind":"select_failure","function":DeclarationId,"invocation":[ExpressionId...],"source":StatusSourceId,"status":NormalizedStatus}
```

Nested frames emit their own selection before cleanup. If a caller propagates that failure, the caller emits a separate `select_failure` naming its propagated-call source while retaining the same normalized semantic status.

### `finalize_begin` and `finalize_end`

Finalization uses the exact semantic place, lifecycle, and single guard flag from the cleanup plan:

```text
{"kind":"finalize_begin","function":DeclarationId,"invocation":[ExpressionId...],"source":CleanupPlace,"lifecycle_id":DeclarationId,"guard_flag":u32,"binding_import":DeclarationId|null}
```

```text
{"kind":"finalize_end","function":DeclarationId,"invocation":[ExpressionId...],"source":CleanupPlace,"lifecycle_id":DeclarationId,"guard_flag":u32,"binding_import":DeclarationId|null,"outcome":{"kind":"success"}}
```

The runtime must clear the guard before `finalize_begin`. A false guard emits neither event. A trivial lifecycle emits `finalize_begin`, then `finalize_end`. An imported lifecycle emits `finalize_begin`, `import_begin`, success-only `import_end`, then `finalize_end`. Automatic finalization has no failure representation.

### `result_commit`

Publishes the result only after postconditions and all live non-result cleanup:

```json
{"kind":"result_commit","function":"app.main","invocation":[],"source":{"kind":"scalar","expression":"declaration:8:app.main:expression:4:body"}}
```

An owned result source is:

```text
{"kind":"owned","storage":CleanupPlace}
```

The event contains semantic source identity, not the result payload or caller address.

## Final outcome

A successful trace has exactly one published result:

```text
{"kind":"success","selected_source":null,"status":null,"result_published":true,"result":TraceResult}
```

`TraceResult` is one of:

```json
{"kind":"i64","value":"42"}
{"kind":"bool","value":true}
{"kind":"unit"}
{"kind":"owned","type_id":"platform.token"}
```

`i64` is a decimal JSON string so JavaScript consumers preserve the complete range. In the current scalar-resource executor, an owned result exposes only its resource declaration ID. Record results are rejected with `UnsupportedResultType`; a later trace schema must add an aggregate semantic value projection before record-result conformance can be claimed.

A failed trace publishes no result:

```text
{"kind":"failure","selected_source":StatusSourceId,"status":NormalizedStatus,"result_published":false,"result":null}
```

The final failure status must equal the root frame's selected status. A failed trace must contain no `result_commit` event.

## Canonical ordering and excluded data

Canonical JSON uses the field order documented here, escapes strings with the repository JSON encoder, renders no insignificant whitespace, and preserves all input vectors exactly. Calling `to_json` repeatedly on the same value produces byte-identical output. Consumers compare the complete canonical event sequence and final outcome; they do not sort events or treat JSON object-key reordering as the canonical byte projection.

The protocol must not contain:

- native pointers, addresses, `uintptr_t` values, or stack offsets;
- Wasm resource handles, linear-memory offsets, context pointers, or nonces;
- context-local status tokens or arena indices;
- raw resource payloads;
- host exception objects, class names, localized messages, backtraces, or opaque diagnostic detail.

Target harnesses may retain those values privately, but they cannot influence semantic trace equality.

## Required validation before a conformance claim

The public Rust types and canonical serializer are a data protocol, not by themselves an executor or proof. `ConformanceTrace::to_json` preserves authored order; it does not validate or repair a trace. The current reference lane independently checks inventory/HIR coverage, exhaustively replays the current acyclic plan CFG, and executes one deterministic single-frame scenario. Before any backend claims conformance, the reference lane and backend-trace validator must reject at least:

- an unknown schema, function, expression, value, field, lifecycle, import, callee, type, status source, slot, or guard flag;
- an invocation path that does not match the nested call sequence;
- an event whose function or semantic IDs do not belong to that frame;
- initialization of a live place, transfer from a dead place, incompatible transfer shapes, duplicate ownership, or a non-atomic/incomplete call commit;
- a callable import end that does not pair with its begin, or a finalizer import end whose source, lifecycle, binding, order, or success-only contract disagrees;
- `select_failure` occurring twice in one frame, selecting a success outcome, changing the selected status during cleanup, or disagreeing with the cleanup plan's source/producer;
- finalization with a mismatched lifecycle/guard, finalization while the guard is false, failure to clear before invocation, missing live cleanup, duplicate cleanup, or incorrect reverse order;
- a result commit before postconditions/non-result cleanup, multiple result commits, owned publication from an incomplete/dead provisional result, any publication after failure, or a terminal outcome inconsistent with emitted events;
- a compiler-owned status with the wrong domain, code, class, or retryability, or an import/adapter status outside its resolved binding contract;
- missing, extra, reordered, or unused fixture/import outcomes, and any physical target data in the semantic projection.

Validation must independently replay the attached `CleanupPlan`; it must not call the canonical plan builder or accept equality with another backend as its only oracle. Native and Wasm traces must each match the same independently validated expected trace and then match each other. Backend execution additionally requires the relevant sanitizer, hostile-adapter, context-isolation, and target-matrix evidence from RFC 0003.

The current reference executor satisfies the target-neutral single-frame subset of these gates and models caller result publication explicitly, but it stubs internal-call outcomes and rejects callable imports. Replay is exhaustive only for the current acyclic CFG surface and rejects a statically estimated path bound above 65,536 or more than 1,000,000 charged work units with `SPX-H006`; loops require a later symbolic/fixpoint design. For the narrow 14-case shape, generated native C and Wasm each emit actual dictionary ordinals and match the independently produced trace and outcome exactly. The private physical native host separately proves ownership plumbing without generated loaded-code execution. That narrow equality is not a substitute for a production native-host trace validator or general-shape conformance; until those remaining gates pass, the protocols and reference oracle do not establish full backend resource conformance.
