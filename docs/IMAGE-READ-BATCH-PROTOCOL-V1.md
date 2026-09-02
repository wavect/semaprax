# Host-selected parallel read protocol v1

Status: implementation and regression cases authored, unrun.

Audience: embedding hosts, agent clients and protocol contributors.

`workspace/read-batch` exposes the existing [immutable read engine](IMAGE-PARALLEL-CANDIDATE-READS-V1.md)
to NDJSON and MCP clients only when the host explicitly selects it at startup.
The surrounding stream still processes one outer request at a time. This is
bounded parallel work within a request, not concurrent independent streams,
background scheduling or JSON-RPC array batching.

## Host selection

An embedding host calls `VNextSession::with_read_batch_workers(workers)` before
any request, choosing one through four workers. The selection cannot be changed
by RPC, recovered archives or a later startup configuration call. Default
sessions have no new method or grant. The existing `VNextPolicy` fields and
embedding-only `handle_read_batch` API remain unchanged.

The CLI accepts the additive closed `semaprax.workspace-host-policy.v7` schema.
It requires all v6 fields plus `read_batch_workers`, which is either null (no
RPC grant) or an integer from one through four. V1 through v6 reject the field,
including null. This selection works independently of candidate preparation,
diagnostics, caches, archives, builds, tests and Git approval. The same startup
loader applies to `serve-workspace` and `serve-workspace-mcp`.

Selected discovery adds the `parallel_read` capability and the method. The
generated instructions list the intersection of the host's ordinary methods
and the fixed immutable-read allowlist. A method's `query` flag alone never
makes it eligible. The MCP catalogue describes exactly the selected method;
MCP negotiation cannot enable it.

## Request and result

The required parameters are exactly `image_revision` and `batch`. The latter
is a closed object with only `frames`, an array of one through sixteen strings.
Each string is an ordinary JSON-RPC request frame; existing generated request
builders can provide their exact output, including a trailing LF. Inner calls
retain their own required image, candidate, draft, attempt and other selectors.
The outer image expectation does not replace them. No worker-count, method-grant
or authority override is accepted.

The request object is described by
`urn:semaprax.image-read-batch-request.v1`. The ordinary v5 success envelope
contains a closed payload with exactly these fields:

| Field | Meaning |
| --- | --- |
| `schema` | `semaprax.image-read-batch.v1` |
| `responses` | One raw JSON response string or null per input position, in input order |
| `source_authority` | Always false |

Response strings preserve the exact ordinary response bytes, including IDs and
any trailing LF. Parse each non-null string with the original method's response
decoder. Null represents an empty frame or a silent notification. Malformed
frames and ordinary per-request failures remain error response strings; one
failed row does not silently erase other rows. The outer success says the
batch was processed, not that every inner request succeeded.

TypeScript, Python and Rust clients derive the typed outer request/container
from selected schemas. They do not statically prove the semantic validity of a
string containing another request or response. Existing per-method decoders
remain necessary; generated clients perform no I/O or capability selection.

## Authority and authentication

The method reuses the existing explicit read allowlist and selected immutable
subjects. Nested `workspace/read-batch`, workspace refresh/refresh-preview, registry mutations,
candidate testing, builds, source commit and commit status/receipt methods are
excluded. Their ordinary host grants cannot make them available inside a
batch. Excluded inner calls receive `-32601` without executing the operation.

Workers receive no mutable session, registry, live source handle, Git provider,
approval, filesystem store root or test execution policy. Policy-bearing
discovery is prepared on the coordinator. Selected reads can still perform
their ordinary bounded source replay and compiler admission; there is no new
interpreter, native execution or physical cleanup authority.

Every structurally accepted outer batch authenticates the held source before
work and after all workers join and the complete bounded outer response is
prepared. This includes a batch consisting entirely of malformed, unavailable
or silent inner frames, unlike the unchanged embedding-only API's early-error
path. Observed source drift discards all rows and retains ordinary absorbing
invalidation. Stale outer expectations and invalid outer parameters can still
reject early. Authentication is point-in-time checking, not a filesystem lock
against outside writers.

## Bounds and failures

The complete outer request retains the 64 KiB v5 frame cap. Each decoded inner
frame also retains its 64 KiB cap. JSON quoting and the outer envelope make the
RPC's total input budget stricter than sixteen independent host API frames.
The complete outer response, including quoted row strings and its envelope,
retains the 1 MiB v5 cap. An individually admissible response does not imply
that several such responses fit together. Aggregate overflow returns one
bounded protocol error and no partial row array. Request fewer rows or smaller
existing query chunks; no response cursor, implicit cache or cap increase is
introduced. MCP retains its own framing bounds and cannot bypass the inner
v5 limits.

Malformed batch objects and batch bounds use `SPX-G294`; worker spawn/panic
failure retains `SPX-G295` and joins all already spawned workers before
releasing no rows. Closed top-level parameter failures remain parameter errors.
Source authentication and selected
query diagnostics retain their existing owners. No batch error retains a
candidate, grants approval, writes source or publishes an artifact.

At most four query computations run together. The existing per-query report
and replay bounds remain in force; sixteen individual bounded responses may
exist before aggregate serialization. This is not a total heap, stack, CPU,
latency, throughput or model-token guarantee. There is no cancellation,
persistent worker pool or cross-request concurrency claim.

`tests/image_protocol/read_batch_protocol_v1.rs` authors direct protocol parity,
least-authority, source-drift and bound cases.
`tests/workspace_session_read_batch_cli_v1.rs` authors actual NDJSON CLI and
closed v1-v7 startup-policy cases. Generated Rust discovery regressions retain
the 900 KiB serialized payload bound across ordinary and batch-selected
policies. These cases have not been executed in this change; hosted evidence,
actual consumers and representative task measurements remain outstanding.
