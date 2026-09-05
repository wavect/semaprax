# Bounded Network Services v1

Status: locally evidenced hosted-provider and language tranche.

This protocol extends [Bounded Language Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md)
without changing its six operations, fixture-v1 bytes, Project-v12 profile, or
native/Wasm admission. Four new compiler-owned operations are available only
through the explicitly injected hosted provider:

| Operation | Capability | Result |
| --- | --- | --- |
| `net_tls_connect(host, port)` | `network.tls` | authenticated TLS connection handle |
| `net_listen(host, port)` | `network.listen` | listener handle |
| `net_accept(listener)` | `network.accept` | accepted connection handle |
| `net_close_listener(listener)` | `network.listen` | zero |

Connection and listener handles occupy one invocation-wide dense 1–8 space in
the evaluator and are type-checked dynamically. All open streams and listeners
are released during settlement on success and failure.

`TcpNetworkProvider` implements outbound TLS 1.2/1.3 with Rustls and Mozilla
roots from `webpki-roots`. The checked DNS name is the authenticated server
name. `with_tls_config` lets an explicit host install private roots. The same
provider implements bounded blocking TCP bind/accept. Raw OS and TLS errors
are normalized to the closed `TLS_FAILED`, `LISTEN_FAILED`, or `ACCEPT_FAILED`
statuses.

Fixture v2 preserves v1 and adds `tls: true` to outbound connections plus a
bounded `listeners` array with ordered `accept` queues. npm/Web remain on
fixture v1 through Project v12; no browser receives raw sockets.

Existing native and Wasm network profiles reject the new operations before
emission because their ABI remains v1. There is no cleartext TLS fallback,
implicit bind address, server-side TLS, UDP, or production-hosting claim.

Focused evidence:

```sh
cargo test --locked --lib network_provider::
cargo test --locked -p semaprax --test useful_data -- network_io_interpreter::hosted_service_profile_executes_tls_and_listen_fixtures --exact
```
