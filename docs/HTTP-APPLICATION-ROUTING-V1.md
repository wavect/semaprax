# HTTP Application Routing v1

Audience: language users, tool authors, and compiler contributors.

Status: second bounded slice of the `semaprax-app-http.v1` profile tracked by
issue #189. Route/status typing, a request-smuggling defense, a one-connection
accept/dispatch/respond/close server lifecycle over the already-hosted-green
Bounded Network Services v1 operations, refusal behaviour, and both
deterministic-fixture and real-loopback exercises are implemented and locally
green. Multi-file Project export, a persistent accept loop, graceful
shutdown, connection-limit/deadline enforcement, TLS, middleware, JSON
bodies, and hosted (non-loopback) evidence remain explicitly out of scope;
see [Non-claims](#non-claims-and-remaining-work).

This tranche composes the existing [Bounded Language Network
I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md) and [Bounded Network Services
v1](BOUNDED-NETWORK-SERVICES-V1.md) transport primitives, [Bounded Stdout
Transcript v1](BOUNDED-STDOUT-TRANSCRIPT-V1.md)'s capability model, and
`std.http`'s byte-parsing idiom into a routing/dispatch convention. It adds no
host operation, no new effect, no new ABI, and no second network stack: a
route handler receives bytes a transport already delivered (`net_recv`, a
`net_stream_stdout` accumulation, or — for this slice's tests — a literal
fixture byte array standing in for either) and returns bytes for the
transport to send; how those bytes arrived or leave remains entirely the
concern of [Bounded Language Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md)
and [Bounded Network Services v1](BOUNDED-NETWORK-SERVICES-V1.md).

## Objective

Give a SEMAPRAX program a typed way to say "this is a route, this is its
handler, this is the request it accepts, this is the response it returns"
without inventing new syntax, new host authority, or a parallel transport.
[examples/http_app_routing.spx](../examples/http_app_routing.spx) is the
committed, compiler-verified, and executed instance; every function below
names its stable identity there.

## Route and status identity

A route is a closed `i64` domain, the same idiom every existing hosted status
domain in this repository already uses (`semaprax.network.v1`,
`std.net.wait_is_readable`/`wait_is_closed`): a small integer with a fixed,
documented meaning, checked by ordinary `requires`/`ensures` contracts instead
of a bespoke enum-like runtime. This profile's example fixes:

| Route | Code | Status |
| --- | ---: | ---: |
| `Health` | 0 | 200 |
| `Echo` | 1 | 200 |
| `NotFound` | 2 | 404 |
| `MethodNotAllowed` | 3 | 405 |
| `Malformed` | 4 | 400 |

`app.http_router.route_for(view: borrow Slice<u8>) -> i64` derives the route
from the raw request bytes; its `ensures result >= 0 && result <= 4` is a
compiler-checked contract that the route table is closed. Extending the table
means adding one more `ensures`-checked code and one more match arm — the
compiler rejects (`SPX-T257`, below) a dispatcher that forgets one.

A record or variant models the same shape and type-checks in this profile
today — `semaprax check` verifies a `Route` variant and `Request`/`Response`
record pair using exactly the shapes in [the agent quick
reference](AGENT-QUICK-REFERENCE.md#records-variants-classes) — but the
bounded reference interpreter that backs plain `semaprax run` does not yet
admit record-field projection (`SPX-F102`, reason `record_projection`), so a
record/variant-shaped router does not execute end to end on that lane today.
This slice's committed example and its deterministic fixture test therefore
use the closed-`i64`-domain idiom, which already executes on every lane
(interpreter, native, Wasm) exactly like `std.http`, `std.net`, and every
other status-domain package in this repository. A record/variant-shaped
router is future work gated on interpreter admission, tracked in
[Non-claims](#non-claims-and-remaining-work), not a shape this profile
refuses to check.

## Typed extraction

`app.http_router.request_content_length(view: borrow Slice<u8>) -> i64`
extracts the request's declared body length the same way
[`std.http.content_length`](../std/http/src/http.spx) extracts it from a
response: scan for the blank-line header terminator within a hard byte
limit, scan for a `content-length:` header line (case-insensitive, ASCII),
skip leading blanks, and parse a bounded decimal run. `-1` means absent or
malformed, matching `std.http`'s convention exactly. A single-file module
cannot `use function … from std.http` (cross-module imports resolve only
inside a Project, see [the agent quick
reference](AGENT-QUICK-REFERENCE.md#projects)), so this slice's example
carries its own small copy of the header-scan primitives rather than
depending on an unavailable import; a Project-scoped version of this profile
should import `std.http`'s existing functions instead of duplicating them.

Every scan in `route_for` and `request_content_length` is bounded by an
explicit `limit` parameter derived from `byte_len(view)` capped at a fixed
256-byte request-line search window; `find_from` never reads past `limit`
regardless of what bytes are present, so an unterminated or over-long request
line is treated as route `4` (`Malformed`, status `400`) rather than scanned
without bound. This is the same "hard limits are load-bearing, not
best-effort" posture as [Bounded Language Network I/O
v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md#limits): the buffer a transport hands to
this profile is already capped (`net_recv`'s 65,536-byte `max`, or a fixture's
literal length), and every scan inside it is capped again against an explicit
limit rather than the buffer's incidental length.

## Request-smuggling defense

`app.http_router.route_for` rejects a request carrying a `Transfer-Encoding`
header before it ever inspects the method or path:
`app.http_router.request_has_smuggling_risk(view: borrow Slice<u8>) -> bool`
scans the same bounded header region `request_content_length` scans and
reports whether any header line's name matches `transfer-encoding:`
(case-insensitive, ASCII, the same idiom as `header_name_is_content_length`).
When it does, `route_for` returns `4` (`Malformed`, status `400`) regardless
of method or path.

This profile implements no chunked-body framing, so refusing the header
outright — whether it appears alone or alongside `Content-Length` — is the
closed, fail-safe answer for a profile that never parses chunked framing at
all. Answering only the narrower both-headers-present conflict would still
leave a bare `Transfer-Encoding: chunked` request silently falling through to
`Content-Length`-based (mis)interpretation of a body this profile cannot
frame; refusing the header unconditionally closes both cases named in issue
#189's failure list at once: "HTTP request smuggling and parser disagreement
between host and language" and "reject smuggling ambiguities such as
conflicting `Content-Length`/transfer encoding". A future slice that
implements chunked-body parsing replaces this blanket refusal with real
framing, not the other way around.

## Handler shape and explicit capability

A handler in this slice is an ordinary function: `route_status(view: borrow
Slice<u8>) -> i64` and `route_body_len(view: borrow Slice<u8>) -> usize` are
pure — no `permit`, no `uses` — because request parsing and response-code
selection need no authority. Nothing here grants a route or its dispatcher
implicit reach to the network, the filesystem, the clock, or stdout: the
ordinary effect system is the entire capability model, unchanged and
unwidened. A handler that legitimately needs a capability (structured logging
to the bounded stdout transcript, a clock read for a deadline decision,
`std.http`'s own parsing helpers) receives it exactly the way every other
effectful SEMAPRAX function does — by declaring `uses { the.effect }`, with
the enclosing module's `permit { the.effect }` and every caller on the path
back to `main` declaring the same effect — never by virtue of being
"a route handler". [Refusal behaviour](#refusal-behaviour) demonstrates both
failure directions: an effect used without `uses` and an effect declared
without a module `permit`.

## Server lifecycle

`app.http_router.serve_one(bind_host: borrow Slice<u8>, port: usize,
max_request: usize) -> bool` is this profile's first real server: one
accept/dispatch/respond/close lifecycle composed entirely from operations
[Bounded Network Services v1](BOUNDED-NETWORK-SERVICES-V1.md) already
implements and already runs hosted-green — `net_listen`, `net_accept`,
`net_recv`, `net_send`, `net_close`, `net_close_listener`. It binds, accepts
exactly one peer, reads at most `max_request` bytes, calls `route_for` and
one of five `send_*` helpers (one literal, fully-formed HTTP/1.1 response per
route — `send_health`, `send_echo`, `send_not_found`,
`send_method_not_allowed`, `send_malformed`, dispatched through `respond`'s
ordinary scalar `match`), then releases both the connection and the
listener. `app.http_router.serve_health_example() -> bool` is the
zero-argument, bool-returning entry point that binds this to a fixed
illustrative loopback port — the exact shape
`semaprax::hosted_interpreter::execute_network_command` requires of a
Language Network I/O v1 entry.

The host that constructs the injected `NetworkProvider` — never this
function — owns every socket, TLS, and credential decision, exactly
[Bounded Language Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md#authority)
already requires: `serve_one` only ever sees the bounded, invocation-scoped
handles that provider hands back, and every operation it calls was already
effect-gated before this profile existed. This module's `permit { …
}` widens nothing; it only lets `serve_one`, `respond`, and the `send_*`
helpers declare the five tokens (`network.listen`, `network.accept`,
`network.read`, `network.write`, `network.connect`) those already-admitted
operations require.

This is one connection, not a server process: `serve_one` returns after its
one peer is handled, so a caller that wants to keep serving calls it again
with a fresh listener. There is no persistent accept loop, no graceful
shutdown signal, no connection-limit counter, and no per-connection deadline
here — see [Non-claims](#non-claims-and-remaining-work).

## Refusal behaviour

A route or handler shape this profile cannot express is refused by the
*existing* compiler diagnostics that already govern the primitives this
profile is built from — there is no separate, weaker "routing" type checker
that could silently approximate an inadmissible shape:

| Shape | Diagnostic | Why |
| --- | --- | --- |
| A handler calls an effectful host operation (`stdout_write`) without declaring `uses` | `SPX-E102` | [Bounded Stdout Transcript v1](BOUNDED-STDOUT-TRANSCRIPT-V1.md)'s ordinary effect rule; a route handler is an ordinary function |
| A handler declares `uses { process.stdout.write }` but the module has no matching `permit` | `SPX-E101` | Effects are declared at both ends; a route table cannot open authority the module never admitted |
| A route dispatcher's `match` on the closed route-code domain omits a code (or the catch-all) | `SPX-T257` | [Refutable Match v1](REFUTABLE-MATCH-V1.md)'s exhaustiveness rule; an incomplete route table fails closed at compile time, never at the first unmatched request |
| A handler tries to build a typed `Response` record/variant directly inside a `match` arm | `SPX-T258` | Match arms cannot yield nominal aggregates; the fix (`if`, or extract scalars first) is exactly [the agent quick reference](AGENT-QUICK-REFERENCE.md#control-flow-mutation-contracts-effects)'s existing rule, reused unchanged for response construction |

Each row is exercised as a hostile fixture by
[`tests/http_app_routing.rs`](../tests/http_app_routing.rs), asserting the
exact stable code and that the source produces at least one diagnostic — a
route this profile cannot express fails closed with a named, stable reason,
never a silent best-effort acceptance.

## Deterministic fixture and real-loopback exercise

[`tests/http_app_routing.rs`](../tests/http_app_routing.rs) is this profile's
owning harness, in three parts.

`fixture_transport` parses, verifies, and `hir::resolve`s
[examples/http_app_routing.spx](../examples/http_app_routing.spx) (also
covered automatically by `tests/examples.rs`'s top-level example walk), then
calls `app.http_router.route_status` and `app.http_router.route_body_len`
directly through `semaprax::interpreter::interpret` with literal HTTP/1.1
request byte arrays standing in for what a deterministic transport would have
delivered:

- `GET /health HTTP/1.1 …` → `200`
- `GET /echo HTTP/1.1 …` → `200`, body length `2`
- `GET /missing HTTP/1.1 …` → `404`
- `DELETE /health HTTP/1.1 …` → `405`
- a five-byte truncated line with no terminator → `400`
- `GET /health HTTP/1.1 …` with a `Transfer-Encoding: chunked` header and no
  `Content-Length` → `400`
- `GET /health HTTP/1.1 …` with both `Content-Length` and
  `Transfer-Encoding: chunked` (the classic smuggling shape) → `400`

No test in `fixture_transport` opens a socket, binds a port, or performs any
network access; the fixture bytes are literal Rust byte slices serialized as
the interpreter's ordinary JSON argument encoding, run twice each and
compared, so the harness also asserts the exact byte-for-byte determinism
invariant this repository requires of every checked artifact.
`examples/http_app_routing.spx`'s own `main` repeats the first five cases and
returns `0`, so `semaprax run examples/http_app_routing.spx` is a second,
CLI-level observation of the same fixture exercise.

`hosted_server_lifecycle` runs `app.http_router.serve_health_example`
end to end, executing the *exact* committed example source (only its
illustrative `18080usize` port literal substituted) through
`semaprax::hosted_interpreter::execute_network_command`:

- against a real loopback `TcpNetworkProvider` bound to an OS-assigned
  ephemeral port — the same throwaway-reservation pattern
  `real_listener_accepts_and_settles_a_loopback_connection` already uses in
  `src/network_provider/tcp.rs` — with a client thread in the same test
  process sending a literal `GET /health` request and asserting the **exact**
  committed response bytes (`HTTP/1.1 200 OK\r\nContent-Length: 0\r\n
  Connection: close\r\n\r\n`) come back over the socket;
- against the deterministic `FixtureNetworkProvider` replaying the identical
  request through the identical lifecycle with no socket, port, or process
  I/O anywhere in the test, asserting the same documented success outcome.

Loopback only: nothing here reaches a host outside the test process, and no
build-time or ambient network access is used anywhere in this repository's
gates. The real-socket test is this profile's byte-exact evidence; the
fixture test is this profile's deterministic, I/O-free evidence for the same
lifecycle — see [Non-claims](#non-claims-and-remaining-work) for why they are
not compared byte-for-byte against each other.

`refusal` proves the four rows of the [Refusal behaviour](#refusal-behaviour)
table above still fire.

Focused evidence:

```sh
cargo test --locked -p semaprax --test http_app_routing
cargo test --locked -p semaprax --test examples -- every_committed_example_is_canonical_and_verified
```

## Non-claims and remaining work

This slice does not implement, and does not claim:

- **A Project-scoped or multi-file profile.** Project v1 restricts every
  function signature crossing a Project function boundary to Copy scalars
  (`SPX-G174`, owned by `src/hir/workspace_link.rs`); a record- or
  variant-typed `Route`/`Request`/`Response` cannot cross that boundary today,
  so a real multi-file service built on this profile is future work gated on
  a Project-boundary extension this slice does not make.
- **A record/variant-shaped router that executes end to end.** Such a shape
  type-checks (`semaprax check`) but the bounded reference interpreter does
  not yet admit record-field projection for `semaprax run`; see [Route and
  status identity](#route-and-status-identity).
- **Compile-time route-table extraction, ambiguity detection, or semantic
  graph entries for routes.** Every route above is one ordinary function a
  developer writes and the compiler type-checks like any other function;
  nothing yet inspects a module's declarations to build or validate a route
  table automatically, or projects routes into `semaprax graph`. This is
  issue #189's implementation-sequence step 4, sequenced for a later round.
- **A persistent accept loop, graceful shutdown, connection limits, or
  per-connection deadlines.** `serve_one` handles exactly one connection and
  returns; there is no supervising loop, no shutdown signal, no
  concurrency/connection-count bound, and no deadline wired to any operation
  it calls (each individual operation is still bounded by [Bounded Language
  Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md#three-different-bounds)'s
  own per-operation deadline, but nothing in this profile yet composes those
  into a server-level slow-client or connection-slot defense). This is a real
  remaining gap against issue #189's Slowloris failure case, not a closed
  one.
- **TLS.** `serve_one` calls `net_accept`, not `net_tls_accept`; a TLS
  variant is a small, separate extension this slice does not make.
- **Middleware, JSON bodies, or query-string parsing.** Only method/path
  routing, one fixed response per route, and `Content-Length`/
  `Transfer-Encoding` header presence are implemented; there is no ordered
  middleware composition, no typed body decoding, and no query-string
  extraction.
- **Chunked-body parsing.** [Request-smuggling
  defense](#request-smuggling-defense) refuses any `Transfer-Encoding`
  header outright rather than framing a chunked body; a request that legally
  needs chunked transfer is refused, not served.
- **Byte-exact fixture-provider evidence for `serve_one`.** Fixture v2 checks
  an accepted connection's `expect_send` only at its first `recv`
  (`FixtureConnection::check_expected_send` in
  `src/network_provider/fixture.rs`), which in this lifecycle happens before
  `serve_one`'s own `net_send`, so the fixture engine cannot itself assert
  the exact response bytes the way the real-socket test does; the fixture
  test asserts only that the same lifecycle reports the same documented
  success outcome.
- **Hosted (non-loopback) evidence.** All evidence for this slice is local:
  either fixture-driven with no I/O, or a real socket between two threads of
  the same test process on `127.0.0.1`. Neither establishes a hosted,
  production, or public-network claim.

Database access (#190), authentication (#191), background jobs (#192), and
observability adapters (#193) are separate issues with separate acceptance
surfaces and are untouched by this slice.
