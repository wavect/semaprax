# OpenAPI Schema Generation v1

`semaprax openapi <file.spx>` is a deterministic, read-only projection that
turns admitted function signatures of one verified module into a canonical
OpenAPI 3.1 document, wrapped in a `semaprax.openapi.v1` envelope. The
companion command `semaprax openapi-compat <base.json> <candidate.json>`
classifies the difference between two previously generated envelopes into
breaking, non-breaking, and informational findings under a
`semaprax.openapi-compat.v1` report. Together they are an executable tranche
of the completion-matrix row "OpenAPI, Protobuf/gRPC, GraphQL, and SQL". They
import no schema language, run no conformance fixture, host no registry or
server, execute no target, and change no source.

## Commands

```sh
semaprax openapi <file> --function <name|stable-id> ... [--max-bytes N]
semaprax openapi-compat <base.json> <candidate.json> [--max-bytes N]
```

- `--function` (repeatable; 1 to 32 selections): selects functions by public
  stable `@id` or by plain source name. Duplicate selections fail closed.
- `--max-bytes` (default 64 KiB, minimum 1024, bounds follow the Agent Context
  byte limits): whole-output budget for either command. Unlike truncating
  reports elsewhere in this repository, overflow here fails closed with
  `SPX-OA105` and emits nothing.

## Admission model

A selected function is admitted to the document only when it is monomorphic,
declares no effects, has only by-value direct `i64`/`bool` parameters, and
returns direct `i64`/`bool`. Every other selection fails with `SPX-OA103` and
one closed reason: `generic_function`, `declared_effects`,
`unsupported_parameter_mode`, `unsupported_parameter_type`, or
`unsupported_result_type`. Function bodies are never interpreted; the
document describes declared interfaces only.

## Document shape

The envelope binds its inputs exactly:

- `source.path`, `source.sha256` (domain-separated over raw file bytes), and
  `source.revision` (the graph revision of the verified program).
- `limits` recording the applied budget values.
- `operations` counting emitted path items.
- `sha256`: domain-separated (`semaprax.openapi.document.v1\0`, length, bytes)
  digest over the exact embedded document payload.
- `document`: the OpenAPI 3.1 document itself, serialized canonically
  (sorted keys, compact separators) so the payload bytes are replayable from
  any conforming JSON reader.
- `nonclaims`: explicit non-goals.

Inside the document each operation lives at `/` + stable id with method `post`,
an `operationId` derived from the stable id (characters outside
`[A-Za-z0-9_]` map to `_`; derivation collisions across the selection set fail
closed), an `x-stable-id` extension, a request-body `$ref` to a per-operation
`<op>.Request` schema whose required list preserves authored parameter order,
a `200` response referencing `<op>.Result`, and — when the signature contains
any `i64` position or any `requires`/`ensures` clause — a `default` response
referencing the shared `Semaprax.Status.v1` component. That status component
is a static description of the compiler-owned failure domains
(`semaprax.arithmetic.v1` codes 1–8, `semaprax.contract.v1` codes 1–2); it is
emitted only when at least one operation can surface such a failure.

## Compatibility report

Both inputs must be envelopes produced by this tool. Authentication is exact:
each side's `schema`, structural shape, embedded-document digest, and outer
`sha256` are re-verified against the parsed payload before any classification;
any mismatch fails closed with `SPX-OA104`. The report therefore never
speculates about documents it could not authenticate, and because it carries
no filesystem paths it is byte-stable across machines that hold identical
inputs.

Findings use closed codes: breaking `OAC-B001` operation removed, `OAC-B002`
required parameter removed, `OAC-B003` parameter type changed, `OAC-B004`
new required parameter added (every admitted parameter is required, so any
unknown candidate parameter is breaking), `OAC-B005` result type changed;
non-breaking `OAC-N001` operation added; informational `OAC-I001` operation
description changed, `OAC-I002` source revision changed. Findings are ordered
shared-operations first (authored order), then removals, additions, and the
revision note. The verdict is `breaking` exactly when at least one breaking
finding exists, and the migration block states the version-bump consequence.

The input binding `input_sha256` is domain-separated
(`semaprax.openapi-compat.inputs.v1\0`) over both sides' document digests, so
a report can be re-bound to its inputs without retaining them.

## Non-claims

This tranche does not provide Protobuf/gRPC, GraphQL, or SQL projections; it
does not parse or import existing OpenAPI documents as source-of-truth
schemas; it runs no live conformance fixtures; it hosts no registry, server,
or hosting surface; it executes no target; and it performs no source changes.
The compatibility lane is a structural diff of authenticated documents, not a
semantic guarantee that two systems interoperate.

## Evidence

`tests/openapi_generation_v1.rs` pins the exact canonical document payload and
its domain-separated digest for a fixed fixture, proves byte determinism,
exercises every admission exclusion reason, verifies the presence and absence
rules for the default response and status component, pins the compatibility
input binding digest across all six exercised finding families, proves report
determinism, and rejects tampered, foreign, oversized-budget, and malformed
inputs through the stable diagnostic codes above.
