# HTTPS Client I/O v1

Status: locally evidenced source and hosted-provider tranche.

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
cargo test --locked --test useful_data hosted_http_profile_executes_a_turnkey_https_get
```

The native-C11, Core-Wasm, npm, and browser adapters currently reject this
new profile rather than weakening it. HTTP/3 and structured asynchronous
execution remain open.
