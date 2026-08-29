# Public Owned UTF-8 API v1

Status: additive Project-v10 contract, gated on promoted Project v9.

## Closed identity

- Project schema: `semaprax.project.v10`
- Project profile: `owned-utf8-api.v1`
- Descriptor schema: `semaprax.public-owned-utf8-api.v1`
- npm metadata schema: `semaprax.owned-utf8-api.v1`
- Descriptor digest domain: `semaprax.public-owned-utf8-api.digest.v1\0`

All v8 descriptor, carrier, metadata, generated JavaScript, Wasm, native-provider,
and Rust-package bytes remain selected by their v8 identities. A v10 descriptor
cannot replay as v8, and a v8 descriptor cannot contain `owned-utf8`.

## Descriptor result

`owned-utf8` is a distinct result type. It is not an alias for `owned-bytes`.
The v10 profile admits all v8 result types and adds a SEMAPRAX `string` result.
Borrowed input mappings are unchanged.

## Physical boundary

The physical value is `(opaque provider handle, exact byte length)`. Neither
the provider nor a target adapter searches for a NUL terminator. Embedded NUL
bytes are data. The fixed maximum result length is 65,536 bytes.

The native provider uses a v10-only allocation representation with an explicit
length header. Before it attaches an opaque handle or publishes result fields,
it validates the exact bytes as Unicode scalar-value UTF-8. An invalid value
returns the adapter-failure status and publishes no result.

The Wasm adapter transports the same length-delimited owned carrier. The npm
facade consumes the carrier once and decodes only an `owned-utf8` result with a
fatal UTF-8 decoder. An `owned-bytes` result remains `Uint8Array`; it is never
silently decoded. The generated Rust facade copies and settles the opaque
handle before `String::from_utf8`, so valid and hostile invalid byte sequences
both settle provider ownership exactly once.

## Settlement

Copy does not transfer ownership. Consume/drop settles exactly one live handle.
Every successful string result is copied and settled before JavaScript or safe
Rust publication. Invalid UTF-8 cannot be returned as a host string, and a
conversion failure cannot leave a live provider handle. Stale, foreign,
wrong-length, repeated, or exhausted handles retain the v8 fail-closed rules.

## Non-claims

This profile adds no command, filesystem, process, network, daemon, recovery,
arbitrary publication, or general text-streaming authority. It does not decode
raw `Bytes`, expose a public aggregate ABI, or weaken Project v1-v9 behavior.
