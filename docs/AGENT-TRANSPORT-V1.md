# Graph Agent Transport v1

Audience: agent and tool authors, plus compiler contributors.

Status: locally implemented and bounded. This is the first executable slice of
the roadmap 0.2 item "a persistent graph daemon and JSON-RPC agent
transport". It adds the transport and one warm in-memory session per process;
persistent indexed revisions remain open.

## Purpose

Agents consume program meaning through the semantic graph, not through source
text. Until now every query paid a full process spawn plus parse plus verify:
`semaprax graph file.spx`, `semaprax context file.spx symbol`. Graph Agent
Transport v1 binds one checked program to a session once and then answers many
requests over a single deterministic byte stream.

## Wire contract

`semaprax.agent-transport.v1` is newline-delimited JSON-RPC 2.0 over stdin and
stdout:

```sh
semaprax serve examples/meaning.spx [--max-request-bytes N]
```

- One request object per input line; one response object per output line.
  Responses end with exactly one LF and are flushed immediately so a
  cooperating agent can pipeline reads.
- Blank lines are silently skipped.
- Requests without an `id` are notifications; they never produce a response,
  including for errors detected after the id position is known.
- `id` must be an unsigned JSON integer or a nonempty string of at most 128
  bytes without control characters. Everything else rejects the whole frame as
  invalid request with a `null` id.
- The top-level member set is closed: `jsonrpc`, `id`, `method`, `params`.
  Unknown members reject as invalid requests. Batch arrays are rejected.
- `params`, when present, must be an object. Methods that take no parameters
  (`protocol`, `ping`, `graph`, `shutdown`) accept only absent or empty
  params.

## Closed method set

| Method | Params | Result |
| --- | --- | --- |
| `protocol` | none | Protocol name and version, cached source revision (the exact [`crate::graph`] revision), sorted method list, effective limits, bound source path and byte length |
| `graph` | none | `{"graph":<payload>}` where `<payload>` is byte-identical to `semaprax graph <file>` output |
| `context` | `symbol` (required), `depth`, `max_bytes`, `max_nodes`, `filters` | `{"context":<payload>}` byte-identical to Agent Context v1 output |
| `context_v2` | as `context` plus required `direction` | `{"context":<payload>}` byte-identical to Agent Context v2 output |
| `ping` | none | `{"pong":true}` |
| `shutdown` | none | `{"ok":true}` (requests only) and stops the loop after the response; notifications stop silently |

Unknown methods answer `-32601`. Context option validation reuses the exact
Agent Context v1/v2 option grammar and limits; failures surface as `-32602`
with the unchanged diagnostic message. A symbol that matches no function
answers `-32000` with `symbol \`X\` was not found`.

## Error model

| Code | Meaning |
| --- | --- |
| `-32700` | Line is not valid JSON, or the frame exceeds `max_request_bytes` (fail-closed: the session stops after this response) |
| `-32600` | Framing or envelope violations (batch array, wrong `jsonrpc`, unknown member, malformed `id`) with id `null` |
| `-32601` | Unknown method (echoed id) |
| `-32602` | Closed parameter violations, including Agent Context option-bound messages (echoed id) |
| `-32000` | Application errors: semantic resolution failure (with a `diagnostics` data array of exact diagnostic JSON) or symbol not found |

## Authority boundary

The session owns no ambient authority. It reads exactly one source path —
named by the host at construction, never by a request — parses it, verifies
it, caches its revision, and serves projections from memory. Requests cannot
redirect the session to another file, trigger writes, spawn processes, or open
network connections. An oversized frame terminates the session instead of
being parsed.

## Determinism

Every response byte is deterministic for one source and one request sequence:
envelopes are hand-rolled canonical JSON with `quote_json` escaping, ids are
rendered canonically (decimal integers, quoted strings), method lists are
sorted, and payload bytes are embedded verbatim from the unchanged Graph and
Agent Context serializers. Repeated sessions over identical inputs produce
identical transcripts.

## Executable evidence

`cargo test --locked -p semaprax --test agent_transport_v1 -- --test-threads=1`

The suite freezes canonical envelopes for `protocol`/`ping`/`shutdown`,
byte-equality of `graph`/`context`/`context_v2` payloads against direct
library calls, cross-run replay identity, notification silence, blank-line
handling, the closed grammar matrix (batches, scalars, wrong versions,
malformed and floating ids, unknown members, parse errors), the closed params
matrix, oversized-frame fail-closed termination, missing/unverifiable source
startup rejection, EOF behavior, shutdown pivots, and exact limit bounds.

## Nonclaims

Transport v1 does not provide: persistent indexed HIR across processes or an
incremental build cache (each session still resolves per query through the
unchanged public functions); multi-source workspaces, patches, mutations, or
any write authority; concurrent request interleaving within one session;
authentication, TLS, sockets, or any network transport; target execution,
project tests, provenance, approval, or evidence authority; new Graph,
CleanupPlan, patch, or backend semantics; any completion-matrix status change.
