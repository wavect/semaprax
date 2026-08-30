# Image Diagnostic Protocol v4

Audience: agent builders, embedding hosts, and compiler contributors.

Status: additive implementation with focused authored, unrun regressions.
No executed validation, hosted-completion, or general repair claim.

V4 adds a host-selected diagnostic lifecycle over validated candidates. It does
not require runtime-test authority and does not widen the existing read-only v1,
candidate-only v2, or test-enabled v3 method sets, descriptors, or result bytes.

## Host selection and authority

`ImageHostCapability::CandidateDiagnostics` selects
`semaprax.image-agent-protocol.v4` with `semantic_read`, `candidate_prepare`, and
`candidate_diagnostics`. `ImageHostCapability::DiagnosticTests` adds
`candidate_test` with the existing fixed default interpreter policy:
100,000 steps, 65,536 execution bytes, and 262,144 report bytes.

`ImageSession::open_diagnostics(manifest, Option<CandidateTestPolicy>)` lets the
trusted embedding host choose an optional fixed policy before accepting any
request. `None` grants no tests. Requests cannot enable tests, override policy,
change the manifest, or switch profiles. Host CLI selectors are
`semaprax serve-diagnostics <manifest>` and
`semaprax serve-diagnostics-tested <manifest>` respectively.

The capabilities report and the method catalogue describe the selected host
profile. Test methods are absent when the host supplies no test policy. The
same catalogue generates request/response-envelope descriptors, query
catalogues, instructions, and TypeScript/Python/Rust client method lists. Payload
schema URNs identify owning versioned reports; these descriptors do not claim
complete bundled JSON Schemas for every semantic result payload.

No v4 method grants filesystem-store, source-write, build, subprocess, network,
approval, or publication authority. Compiler candidate admission and typed repair
are not runtime tests. Explicit optional tests remain the bounded Project
interpreter contract from [v3](IMAGE-CANDIDATE-TEST-PROTOCOL-V3.md).

## Attempt lifecycle

All methods below require the startup `image_revision`; additional arguments
are listed in the table. Unknown arguments and malformed revision handles are
rejected by the closed request validator.

| Method | Additional arguments | Result |
| --- | --- | --- |
| `candidate/attempt` | `candidate_revision`, structured `intent` | Accepted candidate handle or rejected attempt summary |
| `attempt/summary` | `attempt_revision` | Compact rejection metadata, byte count, and base bindings |
| `attempt/query` | `attempt_revision`; optional `offset`, `chunk_bytes` | Exact canonical rejected-attempt report chunks |
| `attempt/repair-catalog` | `attempt_revision` | Only compiler-derived, fully admitted proposals, or explicit empty availability |
| `attempt/repair-apply` | `attempt_revision`, `repair_id` | New validated candidate handle |
| `attempt/discard` | `attempt_revision` | Confirmation that only the named attempt was removed |
| `candidate/semantic-delta` | `candidate_revision`, `target`; optional `offset`, `chunk_bytes` | Exact declaration-delta report chunks |
| `candidate/semantic-delta-catalog` | `candidate_revision`; optional `offset`, `chunk_bytes` | Exact declaration-selection catalogue chunks |
| `hole/open-expression` | `candidate_revision`, `target`, `expression_id`, `hole_id`; optional `draft_revision` | Immutable typed expression-hole draft |
| `protocol/conformance` | Optional `candidate_revision`, `offset`, `chunk_bytes` | Source-backed static protocol conformance chunks for the base image or a retained candidate |
| `candidate/interface-catalog` | `candidate_revision`, `target`; optional `offset`, `chunk_bytes` | Required local protocol members and eligible implementation functions |

The two conformance queries use existing `semantic_read` authority. They expose
source-derived declaration tables over admitted images without adding dynamic
dispatch or protocol nodes to the runtime Graph. See
[Image Protocol Conformance v1](IMAGE-PROTOCOL-CONFORMANCE-V1.md).

Expression holes use the existing fill/complete/discard lifecycle. V4 `hole/query`
discovery admits either the body-hole or expression-hole context schema; the
returned schema identifies which was selected. Opening an expression hole uses
the already granted `candidate_prepare` authority, not test or source authority.
See [Expression Holes v1](PROJECT-CANDIDATE-EXPRESSION-HOLES-V1.md).

`candidate/apply-intent` keeps its existing fail-fast behavior, even in v4.
Only `candidate/attempt` requests diagnostic retention. Its success payload is
`semaprax.image-candidate-attempt-outcome.v1`, with `status: accepted` and a
candidate handle, or `status: rejected` and an attempt summary; the opposite
field is null. Semantic rejection here is a successful diagnostic-record
operation. Ordinary RPC failures still leave registries unchanged.

Attempts use the [Candidate Diagnostics v1](PROJECT-CANDIDATE-DIAGNOSTICS-V1.md)
library and preserve exact bounded intentions, compiler diagnostics, base
candidate identity, and verified predecessor source/target provenance. They are
not checked images, materializable source, or candidates. Candidate and hole
methods cannot substitute an attempt handle for a candidate or draft handle.
Diagnostic spans remain explicitly distinct from verified predecessor expression
locations. No invalid source or HIR is returned or persisted.

Repair discovery offers only the library's bounded, fully admitted integer
literal retagging class. Selection re-derives and fully validates the chosen
repair. It returns a new ordinary candidate; the attempt and predecessor remain
unchanged. The additive [Project Diagnostic Change v1](PROJECT-DIAGNOSTIC-CHANGE-V1.md)
also exposes a `repair_diagnostic` SemanticChange through ordinary candidate
application; it retains the selected repair in history and independently
regenerates it. No guessed repair, automatic selection, or implicit execution
is introduced.

## Registries and authentication

The invocation retains at most 16 candidates, 16 drafts, and 16 rejected
attempts. A shared conservative 256 MiB serialized-report budget counts public
candidate reports, each draft's report plus private last-valid candidate report,
and each rejected attempt's report plus private predecessor report. Shared Arc
reports may be counted more than once; this is not a total HIR-memory bound.
Discarding a public base candidate does not remove an attempt's private verified
predecessor. Exact duplicate attempt digests reuse one registry entry.

Every admitted operation runs under held-source authentication before semantic
work and after rendering the complete bounded response. Preparation, compiler
validation, registry capacity admission, response bounding, and final source
recheck all precede insertion or removal. Overflow discards prepared mutations.
Malformed/stale/unknown repair selectors, full registries, and failed repairs do
not alter prior entries. Source drift permanently invalidates session usability,
even if the disk bytes are subsequently restored. EOF drops in-memory state.

## Bounds, chunks, and evidence limits

JSON-RPC request frames remain at most 65,536 bytes and responses at most
1,048,576 bytes. Duplicate-key validation, non-executing silent notifications,
and existing transport diagnostics remain unchanged. Attempt report/library
bounds remain 2 MiB, 256 diagnostics, and 1 MiB diagnostic text, in addition to
ordinary typed-intention limits. Attempts are not recovery capsules.

Chunk queries default to 16,384 bytes and accept 1,024–65,536 bytes. Offsets must
be within the exact report and on UTF-8 boundaries. Chunks retain offset, total
bytes, next offset, exact selected handle, and the owning `report_schema`.
Concatenation recovers canonical report bytes including the final LF. Semantic
delta and catalogue chunks use `semaprax.image-semantic-delta-chunk.v1`; the
underlying reports retain their v1 schemas. Delta reports may be up to 8 MiB and
catalogues up to 1 MiB; derivation happens before slicing, so chunk size is not a
proportional-work or total-memory bound.

Delta reports contain bounded structural semantic differences and static test
relevance, not executed coverage, runtime behavior, a compatibility guarantee,
or approval. Runtime test capability remains separately selected and visible.

[Focused authored tests](../tests/image_diagnostic_transport_v4.rs) cover legacy
profile rejection, diagnostics without tests, host-only fixed optional policies,
exact attempt chunks, retained-base discard, typed repair, semantic-delta
chunks, registry capacity, source immutability, and absorbing drift. Tests were
not run for this change.
