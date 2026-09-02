# Image Agent Protocol v1

Audience: agent client authors, embedding-host authors, and compiler contributors.

Status: additive implementation with authored regression coverage; local tests
and quality gates were intentionally not run for this change. No hosted or
cross-platform completion claim follows from the implementation.

`semaprax serve-image <manifest>` serves one host-selected authenticated Project
through `semaprax.image-agent-protocol.v1`. This is separate from every existing
Graph/Project transport version; none of their method sets change. The public
`image_transport::serve` and `ImageSession::open` APIs require the host's
explicit `ImageHostCapability::ReadOnly` selection. No request can elevate it.

## Session and authority

Startup authenticates the exact manifest and declared source set once, retains
the checked Project revision, and derives one immutable Semantic Workspace
Image. `workspace/open` selects no path: it returns compact image, Project,
and workspace revision handles for this already bound input. It does not emit
the image or source bodies. Repeated opens return the same handle.

Every admitted call reauthenticates held Project inputs before executing and
after rendering its complete result. Output is released only after that final
check. Observed source drift permanently invalidates the held snapshot, even
if original source bytes later return. Subsequent calls fail; the host must
create a new session. EOF also performs a final held-input check. Windows can
deny edits while these authority handles remain open.

An image revision is a selection handle, never permission. The only initial
capability is `semantic_read`. There is no source write, candidate application,
build, test execution, subprocess, network, arbitrary file read, request-selected
manifest, durable cache, watcher, incremental refresh, or agent elevation route.

## Framing

The protocol reuses the existing Project transport's strict NDJSON codec and
bounded framing. Frames have at most 65,536 bytes before LF; responses have at
most 1,048,576 bytes including LF. Each response is flushed. Duplicate keys at
any depth, CR, invalid UTF-8, batches, unknown envelope members, malformed IDs,
and non-object parameters fail closed. IDs are unsigned 64-bit JSON integers
or nonempty strings of at most 128 UTF-8 bytes without control characters.
Empty frames are ignored. Notifications are silent and do no semantic work.

Oversized requests are drained without retaining their remainder, then emit
one `-32700` error and stop. Oversized responses are replaced by `-32001` and
stop; no partial payload is emitted. EOF ends the session; there is no shutdown
method or protocol state mutation.

Other errors are `-32600` for invalid envelopes, `-32601` for unavailable
methods, `-32602` for closed parameter violations, and `-32000` for semantic
or authentication failure. Application messages retain diagnostic codes.

## Catalog

| Method | Parameters | Result payload |
| --- | --- | --- |
| `protocol/capabilities` | none | Exact read-only capability, sorted method names, limits |
| `protocol/schemas` | none | Catalog-generated request and success/error envelope schemas |
| `protocol/instructions` | none | Version-matched instructions for handle selection and authority |
| `protocol/client` | `language`: `typescript`, `python`, or `rust` | Compiler-generated pure request/result helper source |
| `workspace/open` | none | Compact retained image/Project/workspace revisions |
| `workspace/status` | none | Authenticated state and the same revisions |
| `query/catalog` | none | Only available semantic query descriptors |
| `image/symbol` | `image_revision`, `stable_id` | Existing image declaration lookup |
| `image/function-summary` | `image_revision`, `target` | Compact function facts and bound facet handles |
| `image/facet` | `image_revision`, `target`, `facet`, `handle`; optional `cursor`, `page_size`, `max_bytes` | One bound facet page |
| `image/context` | `image_revision`, `target_kind`, `target`; optional `direction`, `depth`, `max_bytes`, `max_nodes` | Existing Project semantic context |
| `image/impact` | `image_revision`, `target_kind`, `target`; optional `depth`, `max_bytes`, `max_nodes` | Existing Project semantic impact |

All parameter objects are closed. `image_revision` must be the exact lowercase
`sha256:` handle returned by open. Target strings have at most 4096 UTF-8 bytes
and no control characters. `target_kind` is `declaration` or `capability`;
`direction` is `forward`, `reverse`, or `both`. Context defaults to depth 4,
impact to depth 16, both with 1024 nodes and 524,288 output bytes. Limits are
depth 0–1024, nodes 1–8208, and bytes 4096–524,288.

Facets are `signature`, `contracts`, `callers`, `ownership`, `loans`, `cleanup`,
and `relationships`. Their underlying Image facet API owns exact handle/cursor
validation. Protocol page sizes are 1–128 (default 32), output bytes
1024–524,288 (default 65,536), and cursors at most 100 UTF-8 bytes. A caller
obtains a handle from the summary before expanding a facet; handles do not
confer authority.

## Result envelope and generated material

Every successful JSON-RPC `result` is a closed object with `schema` equal to
`semaprax.image-agent-result.v1`, `protocol`, `image_revision`,
`project_revision`, and `payload`. The payload retains its owning semantic
schema. Wrapping context and impact binds the successful transport response to
the selected image without changing those existing semantic schemas.

One compiler-owned catalog drives dispatch admission, parameter validation,
request schemas, response envelopes, query discovery, and client method lists.
Schema descriptors use JSON Schema draft 2020-12. Existing semantic payloads
are referenced by their schema URNs; this is not a bundled replacement schema
for all nested HIR/graph structures. `x-max-utf8-bytes` records byte constraints
that standard JSON Schema string lengths cannot enforce. Lexical duplicate-key
and integer-token rules remain the strict codec's responsibility.

Generated TypeScript, Python, and Rust helpers encode requests and decode
results while checking the protocol version. They perform no I/O themselves;
the host supplies streams and lifecycle. They are small generated source
helpers, not installed SDK packages, authority credentials, or a promise that
untrusted responses are independently verified.

## Authored evidence

`tests/image_protocol/transport_v1.rs` covers catalog consistency, generated helper
method lists, compact workspace handles, semantic and facet queries, authority
and path rejection, stale image rejection, strict codec reuse, notification
silence, bounded deterministic framing, Unix absorbing drift, and CLI arity.
These tests were authored but not run in this change. Existing transport
preservation suites likewise remain unrun.
