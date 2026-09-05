# HTTPS Client Runtime v1

Status: locally evidenced native-host Rust runtime and source operation;
generated-target adapters remain open.

Audience: compiler embedders, runtime contributors, and reviewers.

`semaprax::https_client::HttpsClient` is an explicitly constructed reusable
client for bounded HTTPS GET requests. It accepts only `https` URLs, disables
ambient system-proxy discovery, authenticates TLS through Reqwest's Rustls
backend, negotiates HTTP/1.1 or HTTP/2, follows at most ten redirects, retains
at most eight idle connections per origin, and publishes a response only when
the complete body fits the caller's positive bound of at most 1 MiB.

The response carries the status, negotiated protocol, final URL, stable-sorted
lowercase headers, and body bytes. Construction and execution use one closed
error vocabulary; transport error text and platform errors do not cross the
API. Reusing the client reuses the underlying keep-alive pool. There is no
global client and no compiler path constructs one implicitly.

The native socket provider separately accepts server-side TLS when an explicit
host constructs `TcpNetworkProvider::with_tls_configs` with both client and
server Rustls policies, then calls `accept_tls`. Accepted TLS streams use the
same send, receive, close, and settlement paths as client streams. A provider
without server policy fails before TLS acceptance with `AuthorityDenied`.

Focused local evidence covers redirect resolution, keep-alive reuse, declared
and streamed body overflow, insecure URL rejection, authenticated client and
server TLS over loopback, and settlement:

```sh
cargo test --locked -p semaprax --lib https_client::tests::
cargo test --locked -p semaprax --lib network_provider::tcp::tests::
```

The ignored `public_https_endpoint_negotiates_and_returns_a_bounded_response`
case is an opt-in live public-PKI smoke. It is intentionally outside the
deterministic default gate because DNS, routing, and the remote service are not
repository-owned inputs.

The additive [HTTPS Client I/O v1](HTTPS-CLIENT-IO-V1.md) profile exposes this
runtime as the source-level `https_get` operation and returns a canonical byte
projection accepted by the existing `std.http` parsers. HTTP/3, server request parsing, native-C11 binding, Core-Wasm imports,
npm/browser Fetch binding, Project admission, structured async integration,
observability, and broad target conformance remain open.
