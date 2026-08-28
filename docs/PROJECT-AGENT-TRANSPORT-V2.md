# Project Agent Transport v2

Audience: agent and tool authors, plus compiler contributors.

Status: locally implemented and bounded. Hosted promotion is not claimed.

`semapraxd --stdio` is a persistent, sequential agent session over one exact
Project Manifest v1 input. The host selects the manifest when the process
starts; requests cannot select or redirect a path:

```sh
semapraxd --stdio [--manifest-path semaprax.toml] \
  [--max-request-bytes N] [--max-response-bytes N]
```

The daemon authenticates the manifest, every declared source, and their held
directory/file identities once. The same Phase-A build produces the linked
entry/test HIR, one complete declared-project semantic graph, and one typed
analysis index. Repeated graph, context, and test requests use that retained
state without parsing, resolving, linking, or reading another source.

This is `semaprax.agent-transport.v2`, separate from the byte-frozen
single-file `semaprax.agent-transport.v1` served by `semaprax serve`.

The optional `--allow-project-rename` profile is a separate additive protocol,
`semaprax.agent-transport.v3`. Default v2 remains read-only and does not report
or route its methods. See [Project Rename Transaction
v1](PROJECT-RENAME-TRANSACTION-V1.md).

## Framing and lifecycle

The wire is one JSON-RPC 2.0 object per LF-delimited UTF-8 frame. The raw frame
is bounded before decoding. Duplicate keys at any nesting depth, unknown
top-level keys, CR bytes, invalid UTF-8, batches, signed/floating/null IDs, and
non-object params fail closed. Oversized input is drained without unbounded
allocation, emits at most one bounded error, and terminates. Every response is
one complete JSON object plus exactly one LF and an immediate flush; the
configured response budget includes that LF. An oversized response is replaced
as a whole by the fixed `-32001` error and the session stops.

The logical states are `configured`, `open`, `invalidated`, and `shutdown`.
`workspace/open` takes no path and moves a healthy configured session to open.
Every semantic request supplies the exact `project_revision` and
`workspace_revision` returned by open. Before computing and again after the
complete response payload has been rendered, the daemon reauthenticates every
held project input. Drift prevents the cached payload from being written and
permanently invalidates semantic access. There is no automatic refresh or
reopen. `protocol`, `ping`, `workspace/status`, and `shutdown` remain available
for lifecycle inspection or termination.

On Windows, retained handles are intentional authority. The operating system
may deny editor-style replacement while the session is open; no replace-friendly
or ReFS 128-bit identity claim is made.

## Closed method set

| Method | Parameters | Result |
| --- | --- | --- |
| `protocol` | none | Schema/version, state, sorted methods, byte limits, bound manifest, and nonclaims |
| `ping` | none | Liveness and current state |
| `workspace/status` | none | State and last successfully authenticated revisions |
| `workspace/open` | none | Exact project and Workspace revisions; a healthy repeated open rechecks and returns the same facts |
| `workspace/snapshot` | exact revisions | Canonical manifest subject and declared source revision/digest facts |
| `check` | exact revisions | Admission success for the retained validated project |
| `graph` | exact revisions | Complete `semaprax.project-semantic-graph.v1` declared-project graph |
| `context` | exact revisions, `target_kind`, `target`; optional `direction`, `depth`, `max_bytes`, `max_nodes` | Bounded `semaprax.project-semantic-context.v1` from the retained typed index |
| `test` | exact revisions; optional `max_steps`, `max_bytes` | Exact manifest test closure as an embedded independently verifiable `semaprax.project-execution.v1` envelope |
| `shutdown` | none | One response then termination; notification shutdown is silent |

Notifications do not execute semantic or expensive methods. A test returning
nonzero, a language failure, fuel exhaustion, or call-depth exhaustion is a
completed JSON-RPC result with `command_succeeded:false`, not a transport
failure.

## Evidence and nonclaims

```sh
cargo test --locked -p semaprax --all-features --test project_agent_transport_v2 -- --test-threads=1
cargo test --locked -p semaprax --test agent_transport_v1 -- --test-threads=1
```

Evidence covers a real daemon process, retained snapshot/graph/context/test
queries, revision rejection, zero-write inventory, absorbing drift
invalidation, duplicate/CR/notification grammar, response overflow, clean
shutdown, and v1 wire preservation.

V2 provides no network/socket/TLS/peer authentication; request-selected root
or arbitrary filesystem read; source write, patch, rename, change, impact, or
review authority; Web or native build publication; target execution or test
discovery; persistent disk cache, incremental refresh, or repository index;
batch, concurrent, or out-of-order execution; request deduplication,
exactly-once effects, durability, recovery, provenance, signature, approval,
or reusable authorization. `build` and mutation methods remain open work in
v2. The separately opt-in v3 profile admits only the single exported-function
display rename documented in [Project Rename Transaction
v1](PROJECT-RENAME-TRANSACTION-V1.md); it does not widen v2.
