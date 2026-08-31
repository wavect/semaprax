# Image Agent Discovery v5

Status: Partial; implementation and focused regressions authored, unrun.

Audience: agent builders, typed-client authors, and compiler contributors.

Image Protocol v5 derives capabilities, request descriptors, schemas, and client
builders from the exact host-selected method registry. No request, generated
helper, or schema can grant preparation, diagnostics, tests, builds, or source
commit authority. Existing v1–v4 discovery and client helpers remain unchanged.

## Schema bundle

`protocol/schemas` returns `semaprax.image-agent-schemas.v5`. Its `methods`
contain the actual method name, selected capability, query flag, and closed
request, success-envelope, and error-envelope schemas. Parameter names, required
fields, enum choices, integer bounds, digest syntax, UTF-8 limits, and control
character restrictions come from the same descriptors used by dispatch.
Optional request fields are omitted; they do not accept null unless explicitly
nullable. Success and error IDs follow the ordinary JSON-RPC grammar.

`documents` provides independently identified request/response schemas, existing
compiler constructor documents, and concrete transport payload documents. All
external references in selected request schemas resolve within this bundle;
constructor-local `$defs` references keep their original document scope.

Read-only `image/dependencies` adds a closed dependency-chunk wrapper and typed
request builders. Its heterogeneous source-HIR report is explicitly listed as
unbundled, so the wrapper schema does not claim to validate every relationship
interior. See [Declaration Dependencies](SEMANTIC-IMAGE-DEPENDENCIES-V1.md).

Compact `image/dependency-summary` and `image/dependency-page` methods add
structured summary/page wrappers and typed view/reference/page-option requests.
The heterogeneous dependency-item schema remains explicitly unbundled. These
selectors grant no authority and remain bound to their exact immutable image.
See [Dependency Navigation](SEMANTIC-IMAGE-DEPENDENCY-NAVIGATION-V1.md).

Host-selected draft recovery adds closed capsule and chunk-envelope schemas plus
`hole/recovery-export` / `hole/recovery-restore` builders to v5 clients. Capsule
shape validation does not replace nested candidate replay, source-base checks,
hole overlap checks or final reconstructed draft identity. Both methods require
candidate preparation; the same clients and schemas cannot enable that grant.

Candidate preparation also selects contract-expression catalogue and hole-open
methods. Their request schemas and generated helpers bind candidate, target and
actual HIR expression identities; no phase, source span or AST path is accepted.
Recovery describes the closed `contract_expression` selector row. Contract
catalogue/context interiors remain explicitly unbundled; schema shape alone
cannot establish predicate purity, type/ownership or exact source replay. See
[Contract Expression Holes](PROJECT-CANDIDATE-CONTRACT-HOLES-V1.md).

Bundled payloads include ordinary workspace state, refresh preview and refresh,
candidate/draft handles, attempt outcomes and summaries, validation receipts,
discard outcomes, common report chunks, target/artifact chunks, source-commit
status/handle/chunks, validation catalogues v1/v2, candidate comparisons,
rebase/merge reconciliation, all current change-catalogue operation shapes,
test relevance plans, the semantic-delta root catalogue, and v5 discovery
results. Opted-in refresh/preview responses may include a concrete
`frontend_work` report; that property is optional, never nullable, and absent
from the unchanged cold response. A discriminated schema union admits the
unchanged AST-only `semaprax.project-frontend-cache-work.v1` or explicitly
selected `semaprax.project-semantic-cache-work.v1`; the latter counts checked
module hits while the former still requires zero. Its work counters describe
frontend or checked-module reuse,
not incremental semantic verification. Fields that are required but
nullable, including chunk continuation and optional candidate selectors in
results, remain distinct from omitted fields.

`unbundled_payload_schemas` explicitly lists compiler reports whose complete
shape is not bundled. This includes owning report schemas carried inside chunk
strings, not only unresolved JSON Schema `$ref` values. A fully described chunk
envelope does not describe its encoded semantic report. Consumers must use the
owning specification or a separate supplied schema for those payloads. The
bundle does not substitute permissive empty schemas and claim full coverage.
Selected semantic-delta facets and candidate query report interiors remain
unbundled because they include heterogeneous HIR/impact facts. Source diffs are
strings inside those owning candidate reports; no new standalone source-diff
payload is invented. These limitations do not make their chunk envelope opaque.

The candidate ownership-delta, contract-delta, interface-delta and symbol-diagnostics queries
likewise have concrete chunk envelopes and explicit owning report-schema listings.
Ownership, contract and interface deltas require candidate preparation; symbol diagnostics
require the diagnostic grant. The latter's optional `expected_report_revision` parameter
must be supplied for nonzero offsets; its conditional requirement is enforced
by the handler and documented in the owning report contract. Client builders
validate the ordinary optional digest shape without claiming to enforce that
cross-field condition.

## Generated clients

The additive `candidate/artifact-delta` method requires `candidate_build` and
describes a closed artifact-delta chunk envelope. Its selected Web/npm report
remains explicitly unbundled. Generated request builders preserve the closed
kind choice and cannot select a build limit or widen host authority.

`protocol/client` accepts `language: "typescript" | "python" | "rust"` and
returns deterministic source for the selected profile. It provides one typed
Params interface/TypedDict/struct, request builder, and method-specific response
decoder per selected method. Enum choices become literal choices or Rust enums;
optional fields become optional properties, `NotRequired`, or omitted `Option`
fields. Required digests and integer bounds are checked before serialization.
Calls always include an explicit request ID and end with one LF.

[Typed Response Clients v1](IMAGE-TYPED-RESPONSE-CLIENTS-V1.md) adds concrete
payload/result aliases and `decode_*_typed` helpers for selected methods. These
helpers call the existing method-bound decoder first; generic decoders retain
their signatures. Types derive only from the bundled response documents, while
explicitly unbundled reports and JSON inside chunk strings remain opaque.

The helpers validate closed outer parameter shapes, enum values, digest format,
integer and UTF-8 bounds, control characters, and request byte limits. Nested
constructor objects remain JSON objects and are checked by the compiler; their
full schemas are available in `protocol/schemas`. These helpers are not general
JSON Schema validators and do not duplicate semantic admission. Constructor-only
schema documents and unrelated inner reports are omitted from generated runtime
metadata: only the transitive documents reached by the selected response payloads
are embedded. All documents remain available in the complete schema bundle.

Client generation audits every consumed schema against the common implemented
subset: closed objects, typed scalar bounds, arrays, constants/enums, alternatives,
absolute document references, and the exact digest/control patterns. Unsupported
keywords, local references, reference siblings requiring additional assertions,
schema-valued additional properties, or assertions without their matching type
fail generation with `SPX-G288`. They are never silently discarded. This is a
deliberately bounded validator, not a general Draft 2020-12 implementation.

Decoders match request IDs, protocol/result version, exact envelope fields,
digest fields, and bundled transport payload shapes. Unbundled results remain
opaque JSON values after their schema discriminator is checked. Response payload
values are not advertised as complete typed semantic reports; additive helpers
type the already bundled transport payloads. Python
also rejects duplicate JSON object keys. TypeScript's standard JSON parser and
Rust's `Value` parser do not preserve duplicate-key lexical evidence; hosts that
need the compiler codec's lexical guarantees must apply that codec separately.

Python uses its 3.11 standard library. Rust requires `serde` with derive and
`serde_json`, supplied by the host; the generator installs nothing. TypeScript
requires ES2022, `TextEncoder`, and `structuredClone`. TypeScript rejects integers
outside JavaScript's safe integer range instead of silently approximating them;
use string request IDs. This also means a response containing large numeric
schema bounds may require a host-provided lossless JSON reader. Python and Rust
retain full unsigned 64-bit request IDs and integer bounds.

All helpers are I/O-free: they construct or decode strings and do not open files,
spawn tools, execute tests, publish commits, or make network requests. The host
supplies transport, runtime dependencies, and every authority selection.

## Bounds and evidence

Discovery payloads are bounded to 900 KiB before the ordinary 1 MiB response
envelope ceiling. Requests retain the 64 KiB framing bound. Generated response
shape traversal is bounded to depth 128. `SPX-G288` identifies internal discovery
or selection inconsistencies; `SPX-G289` rejects oversized discovery payloads.
Ordinary protocol grammar, authority, stale revision, and overflow diagnostics
remain unchanged.

Focused module regressions author selected-profile method exclusions, resolved
constructor references, explicit opaque-report listings, optional/null shape
differences, digest/control patterns, typed builder names, integer checks,
literal LF source escapes, concrete candidate schemas, optional frontend work,
unsupported-assertion rejection, transitive metadata selection, and bounded
generated source. They were not run.
No compiler, interpreter, generated client, target, or local quality gate was
executed for this change. Full SDK conformance and exhaustive report schemas
remain open work.
