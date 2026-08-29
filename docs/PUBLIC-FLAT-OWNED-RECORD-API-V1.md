# Public Flat Owned Record API v1

Status: additive implementation tranche; hosted promotion is not claimed.

Project v9 widens the public owned-data result vocabulary by exactly one
authored aggregate shape. It preserves Project v1-v8 and all v8 descriptor,
npm, Wasm, and Rust SDK bytes.

## Fixed identifiers

| Layer | Identifier |
| --- | --- |
| Project schema | `semaprax.project.v9` |
| Project profile | `flat-owned-record-api.v1` |
| API descriptor | `semaprax.public-flat-owned-record-api.v1` |
| npm metadata | `semaprax.flat-owned-record-api.v1` |
| npm carrier | `semaprax.project-npm-build.v8` |
| Rust SDK manifest | `semaprax.native-rust-flat-owned-record-sdk.v1` |

The canonical manifest has the same eight assignments and bounds as Project
v8, with the v9 schema and profile selected together. An earlier schema cannot
select this profile and v9 cannot select an earlier profile.

## Closed semantic admission

Parameters are the Project-v8 vocabulary: by-value `i64` and `bool`, or
invocation-borrowed `str` and `Slice<u8>`. Every selected result is one direct,
monomorphic, source-authored record. Its declaration and every field have an
explicit persistent identity. Its field inventory contains:

- zero or more direct `i64`, `bool`, or `usize` fields; and
- exactly one direct `Bytes` field.

Fields remain in authenticated declaration order. Nested records, variants,
resources, arrays, strings, borrowed fields, generic arguments, multiple byte
fields, owned parameters, and every other Project-v8 exclusion reject before
descriptor or target generation. Selected closures remain monomorphic,
effect-free, import-free, contract-free, acyclic, and bounded exactly as in
Public Owned Data API v1.

## Descriptor and host mapping

`semaprax.public-flat-owned-record-api.v1` is distinct from the v8 descriptor.
It binds the retained Project subject, selected exports and parameters, record
and field identities, presentation names, exact ordinals and closed field
types. Host identifiers are the role prefix followed by the lowercase hex of
every persistent-ID byte, an injective mapping independent of source display
names. Display-only renames therefore do not change callable or member
identity.

| SEMAPRAX field | TypeScript | Rust |
| --- | --- | --- |
| `i64` | `bigint` | `i64` |
| `bool` | `boolean` | `bool` |
| `usize` | `bigint` | `usize` |
| `Bytes` | `Uint8Array` | `Vec<u8>` |

The generated TypeScript result is a readonly interface. The generated safe
Rust result is a public struct and the safe API source uses
`#![forbid(unsafe_code)]`.

## Carrier and settlement

No C, Wasm, or Rust aggregate layout is public. A target adapter receives a
private profile-specific carrier consisting of copied scalar values and one
opaque provider/arena handle identified by the authenticated byte-field
ordinal. It must:

1. authenticate the record identity, field inventory, handle and liveness;
2. copy the exact bounded bytes into fresh host-owned storage;
3. settle the SEMAPRAX owner exactly once;
4. prove no provisional owner or provider obligation remains; and
5. only then construct and publish the host object or safe Rust struct.

Failure leaves the caller result untouched. Scalar values are not observable
before settlement. Invalid field order/type, stale or foreign handles,
copy/drop failure, and settlement uncertainty fail closed. No allocator
pointer, arena token, provider handle, struct offset, padding, alignment, or
aggregate ABI reaches application code.

## Evidence boundary

The physical npm/Core-Wasm and native-provider/safe-Rust routes are wired to
the exact descriptor. The npm facade authenticates every scalar before copying
and settling its sole opaque handle, then constructs the frozen object. The
Rust route independently regenerates the provider from replayed HIR before the
unpublished lower crate compiles and publishes a safe struct package. That
lower crate proves descriptor, byte-integrity, tool, and filesystem facts; it
does not independently prove provider semantics. Its published seven-file
manifest is the sole producer of
`semaprax.native-rust-flat-owned-record-sdk.v1`; the authority-free root
descriptor layer defines no second document under that schema.

Local implementation evidence must cover canonical and hostile manifests,
descriptor derivation/replay and every-byte mutation, exact one-byte-field
admission, every excluded field shape, persistent-ID rename behavior,
TypeScript and safe Rust projections, opaque carrier planning, copy-before-
settle and publish-after-settle traces, capacity boundaries, and v1-v8 known
answers. This implementation tranche has not run those target consumers or
equivalence gates. Hosted promotion requires one exact blocking
Linux/macOS/Windows head.

This tranche does not claim nested aggregates, variants, resources, owned
strings, zero-copy transfer, a public native aggregate ABI, general records,
or completion-matrix promotion.
