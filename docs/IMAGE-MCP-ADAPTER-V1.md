# Semantic workspace MCP adapter v1

Status: implementation and regression evidence authored, unrun.

Audience: MCP clients, embedding hosts and compiler contributors.

The optional stdio adapter exposes the existing v5 semantic workspace through
Model Context Protocol tools. It pins MCP `2025-11-25`; it does not claim support
for later revisions or every optional MCP facility. Canonical `.spx` source,
typed intentions, immutable candidates and separate publication authority remain
owned by the [v5 workspace protocol](IMAGE-WORKSPACE-PROTOCOL-V5.md).

The external protocol references are the pinned
[lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle),
[tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) and
[stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
specifications. This document owns Semaprax's adapter bounds and authority model,
not the upstream protocol.

The optional [saved-source VS Code adapter](VSCODE-SAVED-SOURCE-ADAPTER-V1.md)
uses this transport for an explicit candidate and read-only diff workflow. It
does not expose server build, test or publication grants as editor commands.

## Host startup

```text
semaprax serve-workspace-mcp <manifest> <host-policy.json>
```

The CLI uses exactly the same startup loader as `serve-workspace`. The existing
[closed host-policy versions v1–v6](WORKSPACE-SESSION-CLI-V1.md) select the fixed
manifest, candidate and diagnostic permissions, optional test/build grants,
cache strategy, historical archives and separately approved Git host. Archives
and cache entries load through their existing authenticated owners before Git
provider startup. No new policy field or inferred permission is introduced.
Standard output contains only newline-delimited MCP responses, with no banner.

Embedding hosts first configure an unused `VNextSession`, then consume it through
`McpSession::new(session)`. `handle_frame` accepts one frame body without LF;
`serve_mcp` accepts host-supplied buffered input, output and an `McpSession`.
`finish` retains the inner session's final source authentication. The wrapper
exposes no mutable inner session, approval setter, replacement manifest or
storage-root selector. Hosts finish archive restoration and any exact Git
approval before handing the session to the adapter.

## Lifecycle

The client sends `initialize` with `protocolVersion`, `capabilities` and
`clientInfo`, receives the pinned supported version and tools capability, then
sends `notifications/initialized`. An unsupported requested version receives
the server's supported version; a client that cannot use it must disconnect.
Normal tool calls require the initialized state. `ping` returns an empty result.
Repeated or out-of-order initialization does not create another semantic session.

Client capability metadata is not authority. The adapter requests no filesystem
roots, model sampling, elicitation, credentials, tools from the client or remote
resources. It advertises no resources, prompts, logging, the optional MCP Tasks
facility, or server-initiated requests. When the host selected candidate tests,
four ordinary Semaprax task-lifecycle tools can schedule, poll, cancel, and page
one candidate test; they do not change the advertised MCP capabilities. Notifications
cannot apply intentions or invoke a semantic tool; the initialized notification
only advances the adapter lifecycle.

Frames use a strict closed JSON-RPC request envelope with duplicate-key checks.
The adapter accepts bounded signed/unsigned integer and string request IDs and
preserves the original ID in the outer response. Its ID decoder is separate
from the existing v1–v5 unsigned/nonempty-string decoder; those older wire
contracts are unchanged. Metadata is bounded by the request envelope and cannot
become a session policy or semantic argument.

## Tool catalogue and calls

`tools/list` returns only methods already granted by the selected v5 host. A
tool name replaces `/` in the v5 method name with `__`; hyphens remain unchanged.
For example, `candidate/apply-intent` becomes `candidate__apply-intent` and
`hole/query` becomes `hole__query`. The original name remains the tool title.
Names are case-sensitive and collision-checked against the complete selected
catalogue; tool invocation resolves an exact retained mapping, never an arbitrary
string rewrite into a dispatcher.

Each `inputSchema` derives from the actual method's closed request parameters.
Reachable constructor and recovery definitions are bundled under local schema
references, with no external URN fetch or weakened fallback. Schema validation
describes structure; stable identity, typing, ownership, source replay and
authority remain the ordinary compiler's responsibility. No `outputSchema` is
claimed for heterogeneous compiler reports. Discovery tools remain available
for their versioned payload details and agent instructions.

The catalogue is deterministic and bounded. Pages contain at most eight tools
and fit their fixed byte limit. Continuation cursors bind the complete canonical
catalogue and offset; changing host-selected tools invalidates an old cursor.
The client cannot choose a larger page or a different policy. Cursor values are
descriptive selectors, not secret capabilities or durable session state.

`tools/call` accepts the exact tool name and an argument object. Those arguments
become the ordinary v5 request parameters without an injected image revision,
candidate, draft, approval or default source subject. Use `workspace__open` to
obtain the current image, then include the exact revision fields required by
each tool's schema. Source refresh and historical rebase remain explicit.

The adapter forwards a single bounded v5 request with the fixed internal
correlation ID `0`. It returns the exact complete inner JSON-RPC response as one
MCP text-content item. The outer response preserves the client's original ID;
the inner response retains all revision bindings, diagnostic codes, source-diff
handles and publication classifications. `isError` distinguishes an inner
error from a successful semantic result. Unknown tools and malformed MCP
envelopes are outer protocol errors and never reach semantic dispatch.

The zero-authority `@semaprax/agent-workflow` package includes a pinned client
composition for this exact envelope. It performs initialization, sends the
response-free initialized notification, calls the selected tool, checks the
one-text-item/`isError` binding and inner ID zero, then restores the generated
v5 codec's original correlation ID. Its installed-package review and separately
approved publication gate now targets real `serve-workspace-mcp`; that additive
gate is authored but unrun and does not enlarge this adapter's grants.

## Authority and result publication

Every forwarded method keeps its normal held-source authentication, independent
candidate replay, registry admission and bounded-response preparation. Neither
MCP initialization nor tool discovery grants candidates, runtime tests, builds
or publication. Mutations are available only if the corresponding ordinary host
grant exists. Test execution still uses the fixed host test policy; builds still
produce only their admitted pathless artifacts.

Source commit is exposed only when the fully configured inner session already
has its fixed Git host. Its exact candidate approval must still precede the
first request. MCP arguments cannot approve, change the Git target or regain a
consumed approval. Review/export and separately approved restore/commit remain
distinct host sessions. A completed tool result is evidence, never an approval.

Before forwarding any call, the adapter checks the complete inner request bound
and reserves enough outer response storage for the worst-case escaped inner
response. It does not perform an ordinary fallible wrapper-size check after a
candidate has been retained or a Git ref pivot has occurred. JSON escaping
preserves the entire inner response rather than truncating a diagnostic or
turning a known publication into a generic overflow error.

Output-stream failure can still prevent delivery after an operation. EOF and
stream failure retain the inner final authentication and its publication-aware
failure classification through `VNextSessionFailure`; known publication or
uncertainty is not reported as rollback. Clients must inspect the inner outcome
and use the existing status/receipt routes instead of blindly retrying a commit.
There is no request-ID deduplication, generic request cancellation,
`notifications/cancelled`, exactly-once delivery, background retry, or
transactional rollback of a completed operation. The selected
`candidate__test-task-cancel` tool is narrower: it forwards the explicit
Semaprax cooperative task lifecycle defined by
[Candidate Test Tasks v1](IMAGE-CANDIDATE-TEST-TASKS-V1.md). Build and commit are
never task-wrapped.

## Bounds and remaining evidence

The MCP frame bound is 128 KiB; the reconstructed v5 request must still fit
64 KiB before dispatch. Bounded JSON decoding retains serde's ordinary recursion
limit, followed by a 128-depth and 32,768-node check on the decoded value before
dispatch. The node check is not a pre-allocation bound. String IDs remain
limited to 128 UTF-8 bytes. The outer response
capacity is 8 MiB, while the inner v5 response remains at most 1 MiB. Six bytes
per inner byte is a conservative JSON-escaping bound; the remaining capacity
covers the bounded outer ID and fixed envelope. Reservation precedes invocation.
These are wire/allocation bounds, not an OOM, total-heap, CPU or latency guarantee.

The tool catalogue admits at most 256 tools and 16 MiB of canonical tool data;
individual pages contain at most eight tools and 900 KiB. Oversized or unresolved
schema graphs fail catalogue construction instead of silently omitting granted
tools. `SPX-G349` identifies adapter lifecycle failures, `SPX-G350`
catalogue/cursor failures and `SPX-G351` capacity or reservation failures.
Malformed framing and request shapes use ordinary JSON-RPC error codes. Inner
semantic diagnostics and existing publication-aware final errors retain their
ordinary owners.

`tests/image_mcp_transport_v1.rs` and `tests/workspace_mcp_cli_v1.rs` author
lifecycle, catalogue, exact v5 forwarding, semantic workflow, grant separation,
malformed input, output and final-authentication cases. The existing full
workspace loader and old `serve-workspace` route remain covered by their own
unrun regression suites. Tests, compiler checks, MCP clients and long local
quality gates were not run in this change.

Independent MCP-client conformance, HTTP transport and authorization, real
Extension Host task evidence, the optional MCP Tasks facility, general
concurrent scheduling, complete typed report payloads, and measured workflow
improvements remain open. This adapter does not complete
the full graph-operational programme or promote a completion-matrix row.
