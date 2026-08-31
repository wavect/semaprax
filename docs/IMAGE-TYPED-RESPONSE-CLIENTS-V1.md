# Typed workspace response clients v1

Status: implementation and regression evidence authored, unrun.

Audience: agent client authors, editor integrators and compiler contributors.

The v5 `protocol/client` generator adds concrete response types for the schemas
already owned by [Agent Discovery v5](IMAGE-AGENT-DISCOVERY-V5.md). This extends
the same host-selected, I/O-free TypeScript, Python and Rust clients. It does
not change a semantic method, grant a capability or make an opaque compiler
report fully described.

## Additive client API

Each selected method gains a payload alias, a result alias and a typed decoder.
For example, `candidate/open` produces `CandidateOpenPayload`,
`CandidateOpenResult` and `decode_request_candidate_open_typed`. The existing
request builder, generic decoder and `ResultEnvelope` remain available with
their previous signatures. Disabled methods have no typed helpers either.

The typed decoder takes the same response line and expected request ID as the
existing decoder. It first runs that method's ordinary validation of the request
ID, envelope fields, protocol/schema versions, revision digest syntax and
bundled payload shape. TypeScript and Python then expose the validated value
through its generated static type. Rust converts the validated payload to the
generated deserializable type and preserves the other envelope fields.

```typescript
const result = decode_request_candidate_open_typed(line, "open-1");
const candidateRevision = result.payload.candidate_revision;
```

This is a response-decoding example, not a transport or source-publication API.
The host still supplies the response line and every authority selection.
Clients must use the exact revisions returned by the live session; a typed
string does not authenticate source or turn an old handle into a current one.

## Schema ownership and type generation

The generator uses only the selected methods and their transitively reachable
response documents. A common type model feeds all three emitters. It describes
object fields, required and optional properties, arrays, scalar values, literal
choices, alternatives and bundled references. Shared shapes receive deterministic
internal names; the method aliases are the public entry points. Internal names
belong to that generated artifact and are not stable semantic identities.

The schema audit remains authoritative for which assertions the runtime
validators understand. Static types do not prove numeric bounds, digest formats,
array lengths, `oneOf` exclusivity, contracts, ownership or semantic admission.
The existing runtime checks remain in front of every typed decoder. Unsupported
schema shapes or unresolved, unclassified references fail generation rather
than silently falling back to a permissive payload type.

Explicitly unbundled compiler reports remain opaque. A fully described chunk
contains a typed string, offset and continuation metadata; it does not make the
JSON encoded inside that string a typed semantic report. Constructor request
interiors remain compiler-validated JSON objects, as before. Neither this change
nor the generated type names claim complete HIR or candidate-report schemas.

## Language boundaries

TypeScript retains the ES2022 client and its safe-integer checks. It exposes
literal choices and unions, optional properties and explicit `null` branches.
It does not approximate integers that its existing decoder rejects.

Python retains the 3.11 standard-library client. Typed dictionaries and aliases
describe payloads, with required/optional keys distinct from nullable values.
Open object schemas use `dict[str, Any]`, since this client does not add a newer
typing dependency for typed extra keys. Constant JSON arrays remain lists with
typed elements; exact constant order and length remain runtime checks.
Type annotations are not a replacement for the existing runtime shape and
duplicate-key checks.

Rust retains the host-supplied `serde` and `serde_json` dependencies. Generated
response types preserve the admitted signed and unsigned JSON integer range.
`ResponseInteger` represents that range without narrowing to `i64` or `u64`.
Optional fields distinguish absence from a present nullable value; required
nullable properties still require their key through the ordinary validator.
Deserializing a generated type directly, without the method decoder, does not
perform the protocol's complete validation and is not an admission API.

No generated client reads files, opens a socket, starts a process, applies a
source edit or approves a commit. A decoded publication result retains its
ordinary meaning; its Rust/Python/TypeScript type cannot authorize a retry.

## Bounds and evidence

The generator preserves the existing 900 KiB discovery-payload limit, 1 MiB
response envelope and 64 KiB request bound. Type generation has its own bounded
shape traversal: at most 4,096 definitions, 65,536 visits, depth 128 and 16 MiB
of retained schema keys. Unsupported reference cycles fail generation. These
limits are not an overall heap, CPU or latency guarantee. Unsupported shapes
retain `SPX-G288`; generation-capacity and discovery-size failures use `SPX-G289`.

`tests/image_typed_response_clients_v5.rs` and focused generator regressions
author selected-profile, deterministic generation, concrete shape, nullable and
hostile-input evidence. They have not been executed. Tests, compiler checks,
generated clients and long local quality gates were not run for this change.

Complete heterogeneous compiler-report schemas, typed constructor requests,
editor integration, independent cross-language conformance and measured workflow
improvements remain open. No completion-matrix row is promoted.
