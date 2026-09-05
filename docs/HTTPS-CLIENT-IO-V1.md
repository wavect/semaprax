# HTTPS Client I/O v1

Status: locally evidenced source, hosted-provider, Core-Wasm, and generated
npm fixture tranche under Node and Chromium.

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

Focused evidence:

```sh
cargo test --locked --lib https_client::tests
cargo test --locked --lib network_provider::tcp::tests::tls_client_authenticates_name_and_transfers_over_loopback
cargo test --locked --lib network_provider::fixture::tests
cargo test --locked --lib wasm::http_io::tests
cargo test --locked --test useful_data hosted_http_profile_executes_a_turnkey_https_get
cargo test --locked --test project manifest_v13::
SEMAPRAX_HTTPS_PACKAGE_ROOT=/absolute/generated npm --prefix platform-tests/https-browser-v1 test
```

The native-C11 adapter, live browser-fetch adapter, multi-engine browser gate,
HTTP/3, structured asynchronous execution, and language/backend task lowering
remain open.
