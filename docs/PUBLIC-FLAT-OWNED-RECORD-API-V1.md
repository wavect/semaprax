# Public Flat Owned Record API v1

Status: authored but unrun; unpublished and unpromoted additive Project-v9
implementation tranche.

Audience: compiler contributors, generated-package integrators, and promotion
reviewers.

Project v9 widens the public owned-data result vocabulary by exactly one
authored aggregate shape. Its initial additive tranche preserves Project
v1-v8 and their artifacts. Separately reviewed shared boundary corrections
described below and in Public Owned Data API v1 intentionally change v8-v10
Wasm/native-provider or Rust artifacts; selecting v9 never reinterprets a v8
descriptor or selects a different profile's renderer.

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

The authored input correction explicitly selects the same captured-intrinsic
whole-tuple preflight as v8, before payload snapshots, scratch writes, or arena
entry. The cumulative borrowed-input bound is 65,536 bytes; module input is
bounded separately at 16 MiB before copy/hash. Record field ordering, scalar
authentication, sole-handle settlement, and frozen-object publication are
unchanged. Only v9 JavaScript and its dependent artifact bindings change;
the existing v8 JavaScript helper and rendered bytes remain exact.

The generated Rust invocation guard now closes the complete provider context
after its owner guard settles but before any outward value or recoverable
error. It reinitializes only a proven-closed context on a later invocation;
uncertain settlement is fail-stop. This shared v8/v9/v10 correction changes
generated safe/private Rust and integrity bindings, not provider C/ABI, public
types, descriptors, or manifest schemas. The private invocation counter resets
on reinitialization; the linked provider's handle issuer does not. These
corrections and their hostile-consumer evidence are authored but unrun.

## Evidence boundary

The authored physical npm/Core-Wasm and native-provider/safe-Rust routes are
wired to the exact descriptor. The npm facade authenticates every scalar
before copying and settling its sole opaque handle, then constructs the frozen
object. The root Rust route independently regenerates the provider from
replayed HIR before the unpublished lower crate compiles and stages a safe
struct package. That lower crate proves descriptor, byte-integrity, tool, and
filesystem facts; it verifies the renamed stage through its retained stage
authority, and does not independently prove provider semantics. Root
HIR/codegen replay alone owns that semantic proof. Its generated seven-file
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

No test, target consumer, hosted job, registry publication, or release
promotion is claimed by the authored source state. The upstream baseline at
`4cc03820c86e70527cb65c4b10ee3841c7af167d` predates Project v9.

This tranche does not claim nested aggregates, variants, resources, owned
strings, zero-copy transfer, a public native aggregate ABI, general records,
or completion-matrix promotion.
