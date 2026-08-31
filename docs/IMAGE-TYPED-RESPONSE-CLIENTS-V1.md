# Typed workspace response clients v1

Status: implementation and regression evidence authored, unrun.

Audience: agent client authors, editor integrators and compiler contributors.

The v5 `protocol/client` generator adds concrete response types for the schemas
already owned by [Agent Discovery v5](IMAGE-AGENT-DISCOVERY-V5.md). This extends
the same host-selected, I/O-free TypeScript, Python and Rust clients. It does
not change a semantic method or grant a capability. The repair catalogue is
now structurally described; other explicitly unbundled reports remain opaque.

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

Response-reachable document-local definitions are resolved in their owning
document and normalized into a bounded internal absolute-reference registry.
This is an in-memory schema transformation, not a fetch or a new schema source.
Assertions remain intact; only recognized descriptive constructor annotations
are omitted from runtime metadata. Missing definitions, unexpected nested
resource identities and unsupported assertion keywords fail generation.

Named shapes are reserved before their dependencies are followed. Recursion
through objects and arrays is supported; cycles consisting only of aliases or
unions are rejected. This permits the repair catalogue's nested typed expression
and semantic change payloads without replacing them with arbitrary JSON types.
Both literal-retag and direct byte-field borrow repairs retain closed fields,
their exact typed request bodies, selectors and no-authority facts. An empty
repair list remains an explicitly described result.

The schema audit remains authoritative for which assertions the runtime
validators understand. Static types do not prove numeric bounds, digest formats,
array lengths, `oneOf` exclusivity, contracts, ownership or semantic admission.
The existing runtime checks remain in front of every typed decoder. Unsupported
schema shapes or unresolved, unclassified references fail generation rather
than silently falling back to a permissive payload type.

Explicitly unbundled compiler reports remain opaque. A fully described chunk
contains a typed string, offset and continuation metadata; it does not make the
JSON encoded inside that string a typed semantic report. Constructor request
interiors remain compiler-validated JSON objects. The additive
[typed request helpers](IMAGE-TYPED-REQUEST-CLIENTS-V1.md) describe their recursive
structural shapes without changing runtime admission. Neither this change
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
Forward references describe recursive fields without requiring a newer Python
version or an external typing package.

Rust retains the host-supplied `serde` and `serde_json` dependencies. Generated
response types preserve the admitted signed and unsigned JSON integer range.
`ResponseInteger` represents that range without narrowing to `i64` or `u64`.
Optional fields distinguish absence from a present nullable value; required
nullable properties still require their key through the ordinary validator.
Deserializing a generated type directly, without the method decoder, does not
perform the protocol's complete validation and is not an admission API.
Recursive Rust structures use boxed edges and transparent wrappers where needed
for finite layout; those wrappers do not alter the JSON representation.
Recursive union deserialization checks schema-proven scalar discriminants before
attempting nested branch conversion. A conversion scope shares 65,536 work units
and a depth-128 recursive-union limit. Failed branch attempts consume the same
budget, and capacity failure remains sticky until that scope ends. A new
independent conversion starts with a fresh budget. This protects the typed
conversion step as well as the preceding runtime schema validation; it is not
an alternative source-admission API.

Literal unit types share generated macro implementations with the same serde
checks and integer widths. Unchanged field names omit redundant rename
attributes; escaped or remapped identifiers retain them. This reduces source
and JSON-string escaping overhead without erasing types or assertions.

No generated client reads files, opens a socket, starts a process, applies a
source edit or approves a commit. A decoded publication result retains its
ordinary meaning; its Rust/Python/TypeScript type cannot authorize a retry.

## Bounds and evidence

The generator preserves the existing 900 KiB discovery-payload limit, 1 MiB
response envelope and 64 KiB request bound. Type generation has its own bounded
shape traversal: at most 4,096 definitions, 65,536 visits, depth 128 and 16 MiB
of retained schema keys. Unguarded reference cycles fail generation. These
limits are not an overall heap, CPU or latency guarantee. Unsupported shapes
retain `SPX-G288`; generation-capacity and discovery-size failures use `SPX-G289`.
The client-size regression exercises the actual serialized discovery payload,
including metadata and escaped source, rather than only the raw source length.

Response normalization separately bounds input schema bytes to 16 MiB, traversal
to 65,536 visits and depth 128, and the combined retained/lifted document registry
to 4,096 entries. Original schema IDs are exact `urn:` registry names of at most
4,096 bytes. Local definition names are at most 128 ASCII alphanumeric,
underscore or hyphen characters. Lifted names cannot collide with original or
already retained document identities; references never fall back to a URL fetch.

Runtime validation retains its depth-128 bound and shares a 1,048,576-work
budget across the complete validation, including failed alternative branches.
Each schema visit and each uniqueness item consumes budget. Depth or work
exhaustion fails the whole validation; an alternative cannot catch it and turn
it into success. Constant object discriminants are checked before recursive
child values, so a wrong constructor alternative does not repeatedly traverse
the same nested body. Supported additional assertions include the compiler's exact
lexical-name, protocol-binding and stable-ID patterns, finite string exclusions,
and uniqueness of bounded string arrays. Unsupported `not` forms, regular
expressions and schema intersections still fail generation.

`tests/image_typed_response_clients_v5.rs` and focused generator regressions
author selected-profile, deterministic generation, concrete shape, nullable and
hostile-input evidence. They have not been executed. Tests, compiler checks,
generated clients and long local quality gates were not run for this change.

`tests/image_recursive_repair_response_clients_v5.rs` adds selected-only repair
schemas, actual literal/byte-field/empty reports, recursive language types,
hostile nested payloads, assertion preservation and shared validation-budget
cases. Synthetic changed reports in client tests establish shape handling only;
they are not reminted compiler receipts. These cases are also authored and unrun.

Complete heterogeneous compiler-report schemas, independent cross-language
conformance and measured workflow improvements remain open. Typed constructor
requests and the optional saved-source editor have separate authored, unrun
contracts. No completion-matrix row is promoted.
