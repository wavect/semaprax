# HTTP Application Routing v1

Audience: language users, tool authors, and compiler contributors.

Status: first bounded, offline slice of the `semaprax-app-http.v1` profile
tracked by issue #189. Route/status typing, refusal behaviour, and a
deterministic fixture-transport exercise are implemented and locally green.
Multi-file Project export, real listen/accept wiring, middleware chains, TLS,
and hosted evidence are explicitly out of scope for this slice; see
[Non-claims](#non-claims-and-remaining-work).

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

## Deterministic fixture exercise

[`tests/http_app_routing.rs`](../tests/http_app_routing.rs) is this profile's
owning harness. It parses, verifies, and `hir::resolve`s
[examples/http_app_routing.spx](../examples/http_app_routing.spx) (also
covered automatically by `tests/examples.rs`'s top-level example walk), then
calls `app.http_router.route_status` and `app.http_router.route_body_len`
directly through `semaprax::interpreter::interpret` with five literal
HTTP/1.1 request byte arrays standing in for what a deterministic transport
would have delivered:

- `GET /health HTTP/1.1 …` → `200`
- `GET /echo HTTP/1.1 …` → `200`, body length `2`
- `GET /missing HTTP/1.1 …` → `404`
- `DELETE /health HTTP/1.1 …` → `405`
- a five-byte truncated line with no terminator → `400`

No test opens a socket, binds a port, or performs any network access; the
fixture bytes are literal Rust byte slices serialized as the interpreter's
ordinary JSON argument encoding, run twice each and compared, so the harness
also asserts the exact byte-for-byte determinism invariant this repository
requires of every checked artifact. `examples/http_app_routing.spx`'s own
`main` repeats the same five cases and returns `0`, so `semaprax run
examples/http_app_routing.spx` is a second, CLI-level observation of the same
fixture exercise.

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
- **Real sockets, TLS, listen/accept wiring, or a server lifecycle.** This
  slice's transport is a literal fixture byte array. Wiring
  `net_listen`/`net_accept`/`net_tls_accept` (all already implemented by
  [Bounded Network Services v1](BOUNDED-NETWORK-SERVICES-V1.md)) to this
  routing/dispatch convention, including graceful shutdown and connection
  deadlines, is separate work this slice does not perform or claim.
- **Middleware, JSON bodies, query-string parsing, or header value
  extraction beyond `Content-Length`.** Only method/path routing and a single
  header's presence and value are implemented.
- **Chunked transfer encoding or request-smuggling defenses.** Only
  `Content-Length` is read; a request declaring `Transfer-Encoding` is not
  specially recognized or rejected by this slice, so smuggling ambiguities
  between the two are not yet a closed case here.
- **Hosted evidence.** All evidence for this slice is local, offline, and
  fixture-driven; it establishes no hosted, production, or public-network
  claim.

Database access (#190), authentication (#191), background jobs (#192), and
observability adapters (#193) are separate issues with separate acceptance
surfaces and are untouched by this slice.
