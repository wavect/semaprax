# HTTPS Client I/O v1

Status: locally evidenced source, hosted-provider, native C11, Core-Wasm, and
generated npm fixture tranche under Node and Chromium.

Audience: language users, host integrators, compiler contributors, and reviewers.

HTTPS Client I/O v1 adds one compiler-owned source operation:

```semaprax
https_get(url: borrow Slice<u8>, max: usize) -> own Bytes
```

The calling module and function must declare `network.http`. The URL must be
non-empty UTF-8 without NUL, at most 2,048 bytes, and use the `https` scheme.
`max` is positive and at most 65,536 bytes. The operation publishes no partial
result: redirects, protocol negotiation, headers, and the complete body must
fit before the owned result becomes visible.

The returned bytes are a deterministic HTTP/1.1-shaped response accepted by
the existing `std.http` parsers. `x-semaprax-http-version` records the actually
negotiated `0.9`, `1.0`, `1.1`, `2`, or `3` version; hop-by-hop framing is
removed and `content-length` is regenerated from the collected body. Response
header names are lowercase and sorted by `(name, value)`.

The injected provider owns authority and transport. `TcpNetworkProvider`
retains one reusable client, uses Mozilla roots and hostname validation,
disables ambient proxy discovery, negotiates HTTP/1.1 or HTTP/2, follows at
most ten redirects, and keeps at most eight idle connections per origin. The
ordinary interpreter has no provider and cannot perform network I/O.

`semaprax.network-fixture.v3` is the deterministic carrier. It preserves v1
TCP and v2 TLS/listener fields and adds an ordered `https` array whose entries
carry exactly `url` and canonical string `response`. At most eight entries are
accepted; URLs are exact-match HTTPS values and responses obey the 65,536-byte
source result bound. A mismatched URL or undersized invocation bound fails
without consuming the queued entry.

Project Manifest v13 exposes this boundary as
`profile = "https-command-io.v1"` with an exact `network.http` capability.
The `network-run --fixture` CLI authenticates the selected project command and
replays fixture v3 without opening a socket.

The Project-v13 native target emits a real C11 HTTPS client and links it with
libcurl 7.85 or newer. Each command invocation owns one reusable easy handle,
disables proxy discovery, accepts only HTTPS through redirects, bounds the URL,
headers, body, redirects, connection cache and timeouts, and settles before
publishing output. It requires certificate and hostname verification, permits
only TLS 1.2 or TLS 1.3, requests HTTP/2 with HTTP/1.1 fallback, and embeds the
compiler-owned 146-certificate Mozilla root projection documented in
[`src/codegen/MOZILLA-ROOTS.md`](../src/codegen/MOZILLA-ROOTS.md). It does not
read a host trust-store path. The deterministic gate performs an actual
encrypted localhost handshake using an explicit fixture CA; a separate ignored
public-endpoint smoke proves the production embedded-root path when public DNS
and network authority are deliberately granted.

Project v13 Core-Wasm appends one synchronous `spx_https_get_v1` import and an
independent `__spx_http_status_v1` marker. The generated npm package supplies
that import from a branded, one-invocation fixture-v3 provider, authenticates
the Wasm before instantiation, validates the returned owned-byte carrier, and
publishes output only after cleanup succeeds. It imports neither browser
`fetch`, Node sockets, nor WASI sockets. The provisioned HTTPS Browser v1 gate
executes the real generated package under Chromium, checks exact output,
one-shot invocation and tampered-Wasm rejection, and observes that every
browser request remains on the loopback fixture origin. Node executes the same
generated carrier in the Project-v13 integration suite. This is not
multi-engine or live-browser-TLS evidence.

The bounded Rust structured-task runtime exposes
`TaskScope::spawn_https_get`. It moves an explicit provider into the lexical
task, settles it exactly once on every exit path, and publishes the typed HTTP
result only after settlement. Cancellation before transport prevents the
request; started blocking I/O drains, with responses completing after the
bounded deadline discarded. This is scoped host execution, not an async
executor or SEMAPRAX native/Wasm task lowering.

Failures use the closed `semaprax.http.v1` status domain:

| Code | Meaning |
| ---: | --- |
| 1 | invalid URL |
| 2 | insecure scheme |
| 3 | transport or TLS failure |
| 4 | response or configured bound exceeded |
| 5 | unsupported negotiated HTTP version |
| 6 | authority denied |

These codes do not alter `semaprax.network.v1` or
`semaprax.network-service.v1`. The operation is appended as HIR cache tag 17;
all earlier tags retain their bytes.

## Deadlines

`https_get` reuses the reusable client's own configured request timeout. That
timeout is *not* the raw TCP provider's aggregate operation deadline, and the
two must not be described as one bound. [Bounded Language Network I/O
v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md#three-different-bounds) owns the
distinction between a per-syscall timeout, an operation deadline, and the
evaluator's invocation budget; `net_connect` and its family answer to the
provider's aggregate deadline, while `https_get` answers to the client
configuration the host installed. A host that wants one number for both sets
both.

## Redaction

A URL is program input and may carry userinfo, a token in a query string, or a
private hostname. The operation's failure surface is therefore code-only:

- `semaprax.http.v1` failures carry a domain identifier and one of six closed
  codes, and never a message, URL, host, header, or transport error text;
- `HttpsError` is a fieldless enum, so no transport detail crosses the
  provider boundary into a diagnostic;
- the canonical response projection omits the final URL, and a failed
  invocation discards both the stdout and stderr transcripts;
- the interpreter seam captures no trace events for this operation.

The URL is still a source literal: it appears verbatim in `.spx` text, in
graph JSON, and in documentation projections, exactly like any other literal.
Nothing here redacts source. What is guaranteed is that performing the request
adds no new place for those bytes to surface.
`a_credential_bearing_url_never_reaches_a_status_or_transcript` is the
sentinel regression.

## Target admission

Each target states its own behaviour; none inherits another's.

| Target | Behaviour |
| --- | --- |
| Reference interpreter (`semaprax run`) | refuses: no provider, `SPX-B103` |
| Hosted interpreter seam | executes, with the host's injected provider |
| Core Wasm | executes, through one synchronous `spx_https_get_v1` `env` import |
| Project v13 npm/Web | executes, from a branded one-invocation fixture-v3 provider |
| Native C11 | rejects before emission: `network.http` is outside the native permit inventory (`SPX-B103`), and the emitter's own HTTPS arm is unreachable defence in depth |
| Browser Fetch adapter | not implemented; the npm/Web lane imports no `fetch` |

The native rejection is checked at the permit gate of both native entry
points, so no translation unit is produced and no socket or HTTPS text is
emitted.

Focused evidence:

```sh
cargo test --locked --lib https_client::tests
cargo test --locked --lib network_provider::tcp::tests::tls_client_authenticates_name_and_transfers_over_loopback
cargo test --locked --lib network_provider::fixture::tests
cargo test --locked --lib wasm::http_io::tests
cargo test --locked --lib codegen::native_emit::http_io::tests
cargo test --locked --test useful_data hosted_http_profile_executes_a_turnkey_https_get
cargo test --locked --test useful_data network_io_interpreter::
cargo test --locked --test useful_data network_io_native::native_lane_rejects_https_get_precisely_and_emits_no_https_text
cargo test --locked --lib network_provider::tcp::tls_rejection_tests::
cargo test --locked --test project manifest_v13::
SEMAPRAX_HTTPS_PACKAGE_ROOT=/absolute/generated npm --prefix platform-tests/https-browser-v1 test
```

`examples/https-project` is the committed multi-module example: `src/app.spx`
performs the scripted HTTPS GET and imports `src/response.spx`, which reads a
typed status code and body length out of a borrowed view of the canonical
response. The parsers mirror `std.http`; they are written as nested `if`
expressions rather than literal `match` arms because the npm/Web semantic
recipe admits only the `Option` and `Result` variant patterns. A
`[dependencies]` import of the `std.http` package itself awaits the Package
Manifest v1 table layout, which the frozen Project v13 key-value manifest does
not carry.

The interpreter regressions cover no-provider, insecure scheme, malformed and
oversized URLs, declared and streamed response-bound overflow, an unavailable
target, and the credential sentinel. The loopback TLS rejections cover an
untrusted root and a certificate that does not name the requested host; both
are provider-level Rust evidence and neither has been driven from `.spx`
source through `net_tls_connect` against a real socket.

The native-C11 adapter, live browser-fetch adapter, multi-engine browser gate,
HTTP/3, structured asynchronous execution, and language/backend task lowering
remain open. So do a typed owned `Response` record, a redirect-limit
regression, and a request-timeout regression for the reusable client.

Live browser-fetch authority, multi-engine browser evidence, HTTP/3,
structured asynchronous execution, cross-platform libcurl provisioning, and
language/backend task lowering remain open.
