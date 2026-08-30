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

Record, field, and parameter identities are retained validated-HIR facts,
not exported method names. The lower native descriptor reader must preserve
their NUL-free UTF-8 bytes, including empty, uppercase, non-ASCII, and escaped
control-bearing identities admitted by the compiler. Display names are JSON
presentation strings; they neither supply Rust identifiers nor need to be
unique across distinct record identities. Repeated descriptions of the same
record identity must still agree exactly. Export identities retain their
existing portable spelling and bound.

The authored replay-alignment correction removes native-only 128-byte name
and record/field restrictions and the display-name uniqueness check. It does
not expand source/HIR admission, change hex-derived host names, or change any
previously accepted canonical descriptor bytes. The existing 1 MiB canonical
descriptor bound remains authoritative. Native canonical replay uses the
compiler's exact control-character escape spelling, not an interchangeable
JSON serializer's spelling; semantic JSON equality alone is insufficient.
Root derivation additionally charges
a conservative lower bound on repeated string/hex content before cloning;
that is not a peak-memory or exact rendering-work bound. The inherited
complete linked-function inventory limit of 256 is checked before indexing,
including functions outside the selected export closure.

### Identity-preserving semantic recipe

The shared owned-data package recipe is a compiler-private replay projection,
not a second source module or a new public descriptor. Linked declarations may
have identical display names in different source modules. When such a collision
exists, the recipe assigns every authored type a deterministic alias in sorted
stable-ID order and retains the exact original names in a canonical, bounded
header. This avoids collapsing nominal identity while flattening the source.
Collision-free recipes retain their previous spelling and have no header.

Independent replay checks the complete alias inventory, original identifier
spellings, and the existence of a genuine collision. It restores names only by
the resolved stable identities, rebuilds the declaration index, type facts,
provenance, and cleanup through the existing owned-data linker, and requires
exact canonical re-rendering. Descriptor and target replay consume that checked
result; neither substitutes aliases for descriptor presentation facts nor
ignores display-name differences. Header bytes share the existing 1 MiB recipe
limit and existing carrier digest binding. No capsule gains publication authority.

All source `@id` literals use the canonical SEMAPRAX string formatter, not JSON
quoting. JSON descriptor escaping stays unchanged. Previously replayable
collision-free recipes remain byte-identical; control-bearing identities that
previously produced invalid source and colliding names that previously failed
resolution now have replayable projections. This correction does not widen
language, Project-profile, descriptor, or target admission.

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
the existing v8 JavaScript helper and rendered bytes remain exact for that
input-admission extension.

The subsequent [owned npm invocation correction](OWNED-NPM-INVOCATION-V1.md)
changes v8/v9/v10 JavaScript failure handling. V9 preserves consume, settlement,
then frozen-record construction, with guarded scratch cleanup before outward
publication. Unexpected post-entry exceptions and caught reentry latch poison;
cleanup cannot replace an earlier thrown value. Only authenticated checked
statuses can recover after complete settlement. Wasm, descriptors and public
types remain unchanged; the real-package failure regressions are unrun.

The generated Rust invocation guard now closes the complete provider context
after its owner guard settles but before any outward value or recoverable
error. It reinitializes only a proven-closed context on a later invocation;
uncertain settlement is fail-stop. This shared v8/v9/v10 correction changes
generated safe/private Rust and integrity bindings, not provider C/ABI, public
types, descriptors, or manifest schemas. The private invocation counter resets
on reinitialization; the linked provider's handle issuer does not. These
corrections and their hostile-consumer evidence are authored but unrun.

The subsequent [owned-data internal String correction](NATIVE-OWNED-DATA-STRING-SETTLEMENT-V1.md)
applies to the shared v8/v9 native emitter. Its length-header helpers and inline
owner ledger cover the full emitted function inventory, including unselected
Strings; native artifacts and dependent bindings intentionally change for those
subjects. Direct descriptor/provider evidence is not activated Project-v9
admission, which keeps its existing Wasm restrictions. Public record/field
types, carrier layout, descriptors, and String-free output remain unchanged.
The new physical allocation evidence is authored but unrun.

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

The replay-alignment regressions are authored in
`tests/project_flat_owned_record_api_v1.rs` and the lower package's
`flat_descriptor::tests`. A shared hand-authored source/canonical-byte oracle
connects actual compiler derivation/replay with lower native replay without
adding dependencies or exposing a new public replay API. These tests remain
unrun; the private descriptor-size model is only a byte-guard check, not a
proof of semantic admission or peak allocation.

`tests/project_v9_recipe_identity_v1.rs` adds actual multi-module Project
admission and npm replay for colliding display names, retained control-bearing
identities, and display-only renames. Its native assertion reaches a deliberately
rejecting injected publisher only: it proves semantic replay reaches the package
handoff, not compilation, successful publication, or physical consumer behavior.
Private recipe tests cover exact historical source bytes and hostile restoration
headers. All of this additional evidence is authored and unrun.

The follow-on published-product fixtures share one four-source subject in
`tests/support/flat_record_product.rs`. Two `Payload` records have distinct
identities, including a control-bearing record identity and one empty field
identity. The byte field is first in one declaration and last in the other;
division by zero therefore exercises failure after and before byte creation.
`tests/project_v9_recipe_consumer_v1.rs` publishes and reopens all six npm
artifacts against the exact inline carrier before consuming the real bindings
and Wasm under Node. It also checks the complete TypeScript declaration text
against a source-derived oracle; this is not TypeScript compiler execution.

The private toolchain's `project_flat_record_sdk_v1` gate invokes the actual
Project-to-Rust host, reopens the seven-file SDK, checks its exact manifest and
source-derived provider binding, rejects repeat publication without clobbering,
and runs an external dependency-free Rust consumer with a literal lockfile and
isolated short Cargo target directory. Both targets exercise the same binary
payload/divisor corpus, both field orders, exact scalars, 65,536/+1 input bounds,
independent outputs, checked failure followed by SDK/runtime-object reuse, and
unchanged consumers after display-only renames. Context reinitialization is not
persistent-context reuse, and observed recovery is not an allocator-count or
complete destruction-trace measurement.

The native gate is explicitly selected after tool provisioning; it remains
ignored in an ordinary test run. These commands are documented, not executed:

```sh
cargo test --locked -p semaprax --test project_v9_recipe_consumer_v1
cargo test --locked -p semaprax-toolchain --test project_flat_record_sdk_v1 -- --ignored
```

Neither fixture is a new public archive verifier. All new physical-consumer
evidence remains authored and unrun; no hosted release blocker or promotion is
established by adding it.

No test, target consumer, hosted job, registry publication, or release
promotion is claimed by the authored source state. The upstream baseline at
`4cc03820c86e70527cb65c4b10ee3841c7af167d` predates Project v9.

This tranche does not claim nested aggregates, variants, resources, owned
strings, zero-copy transfer, a public native aggregate ABI, general records,
or completion-matrix promotion.
