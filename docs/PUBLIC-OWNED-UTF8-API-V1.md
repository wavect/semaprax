# Public Owned UTF-8 API v1

Status: authored but unrun; unpublished and unpromoted additive Project-v10
implementation tranche, gated on promoted Project v9.

Audience: compiler contributors, generated-package integrators, and promotion
reviewers.

## Closed identity

- Project schema: `semaprax.project.v10`
- Project profile: `owned-utf8-api.v1`
- Descriptor schema: `semaprax.public-owned-utf8-api.v1`
- npm metadata schema: `semaprax.owned-utf8-api.v1`
- Native Rust manifest schema: `semaprax.native-rust-owned-utf8-sdk.v1`
- Native Rust manifest file: `semaprax.native-rust-owned-utf8-sdk.json`
- Descriptor digest domain: `semaprax.public-owned-utf8-api.digest.v1\0`

All v8 descriptor, carrier, metadata, generated JavaScript, Wasm, native-provider,
and Rust-package bytes remain selected by their v8 identities. A v10 descriptor
cannot replay as v8, and a v8 descriptor cannot contain `owned-utf8`.

## Descriptor result

`owned-utf8` is a distinct result type. It is not an alias for `owned-bytes`.
The v10 profile admits all v8 result types and adds a SEMAPRAX `string` result.
Borrowed input mappings are unchanged.

The authored v10 input correction explicitly reuses the existing v8
captured-intrinsic whole-tuple preflight. It checks the cumulative 65,536-byte
borrowed-input bound, including exact UTF-8 lengths, before payload snapshots;
module bytes have a separate 16 MiB pre-copy/hash bound. V10 JavaScript and
dependent artifact bindings intentionally change, not descriptor, Wasm,
TypeScript, or v8 JavaScript bytes. Raw `Bytes` are still never decoded as text.

## Physical boundary

The physical value is `(opaque provider handle, exact byte length)`. Neither
the provider nor a target adapter searches for a NUL terminator. Embedded NUL
bytes are data. The fixed maximum result length is 65,536 bytes.

The native provider uses an allocation representation with an explicit length
header, also reused by the later ordinary [String contents correction](NATIVE-STRING-CONTENTS-V1.md)
without changing v10 output. Before it attaches an opaque handle or publishes result fields,
it validates the exact bytes as Unicode scalar-value UTF-8. An invalid value
returns the adapter-failure status and publishes no result.

The Wasm adapter transports the same length-delimited owned carrier. The npm
facade consumes the carrier once and decodes only an `owned-utf8` result with a
fatal UTF-8 decoder. An `owned-bytes` result remains `Uint8Array`; it is never
silently decoded. The generated Rust facade copies and settles the opaque
handle before `String::from_utf8`, so valid and hostile invalid byte sequences
both settle provider ownership exactly once.

The root compiler alone authenticates provider semantics from replayed HIR.
The unpublished lower package authenticates the closed descriptor, provider
byte integrity, compiler-declared textual binding, held tools, and filesystem
publication facts; it verifies the renamed stage through retained stage
authority and receives no HIR or independent semantic-proof authority.

## Settlement

The v10 Rust package shares the [Cargo build-script path boundary](PUBLIC-OWNED-DATA-API-V1.md#generated-cargo-build-script-path-boundary).
Missing, non-Unicode, and CR/LF package paths reject before Cargo instructions;
the target guard retains precedence. Only generated `build.rs` and its manifest
integrity bindings change, not UTF-8 descriptors, provider archives, safe/FFI
Rust, or package schemas. These regressions are authored but unrun.

The existing closure admission remains narrow: a String-returning function
must have a literal or direct retained-function call as its body (optionally
inside an empty block). A non-String-returning body may not stage String
expressions, and compiler-owned String intrinsics remain rejected. A direct
call's argument expressions can nevertheless contain String temporaries,
bindings, nested calls, and blocks. Unused owned String parameters also require
settlement; the narrow outer-body rule does not remove those obligations.

The v10 Wasm correction tracks those physical String owners in deterministic
local cells derived from validated HIR and the emitter's exact function plan.
This is the existing inline String-cleanup convention, not a reinterpretation
of resource CleanupPlan slots or a new cleanup schema. Place reads clone their
value, preserving the source binding; temporary handoffs move and clear their
source cell. Expression/scope exit settles nonescaping cells before reuse or
loop backedges. Calls evaluate all arguments left to right before atomically
transferring staged String arguments to the callee. Call-out memory is only
transport, not a second cleanup owner.

Success settles non-result owners before result publication. A provisional
String result is cleared only after the caller store succeeds; recoverable
failure sweeps remaining cells without changing the primary status. Host
exceptions, allocation failures, or failing finalizers do not acquire a new
recoverable cleanup guarantee: the instance remains poisoned/fail-stop.

For v10 packages selecting an owned UTF-8 result, the npm arena bound is
derived from the selected acyclic call closure:
one transient handoff slot plus the maximum call-path sum of authenticated
Bytes cleanup leaves and String owner cells. Checked arithmetic rejects token
space overflow. This is conservative simultaneous-owner accounting, not a heap
byte or exact-liveness claim. V8/v9 retain their fixed 16-owner runtime bytes;
their separate Bytes-copy admission is unchanged. V10 Wasm/JavaScript and
dependent integrity bindings intentionally change; descriptors and public
signatures do not. Scalar/Bytes-only v10 selections retain their existing
memory layout and fixed arena bound.

Copy does not transfer ownership. Consume/drop settles exactly one live handle.
Every successful string result is copied and settled before JavaScript or safe
Rust publication. Invalid UTF-8 cannot be returned as a host string, and a
conversion failure cannot leave a live provider handle. Stale, foreign,
wrong-length, repeated, or exhausted handles retain the v8 fail-closed rules.

The [owned npm invocation correction](OWNED-NPM-INVOCATION-V1.md) makes a
post-consumption JavaScript UTF-8 decoding failure poison the instance even
when its arena is empty. Imported UTF-8 validation returns zero only for
authenticated malformed text; memory/carrier faults and unexpected host
exceptions cannot masquerade as malformed user bytes. The shared v8/v9/v10
failure-state correction changes runtime JavaScript and dependent integrity
bindings, not Wasm, descriptors or public signatures. Its regressions are
authored but unrun.

The authored shared Rust invocation guard additionally proves the complete
provider context settled before any outward value or recoverable error,
including a UTF-8 conversion failure. An inner owner guard precedes context
closure on Rust unwind; uncertain settlement remains fail-stop. Only a
proven-closed context may be reinitialized on a later call. Its private
invocation counter resets while the linked provider's handle issuer remains
nonreused. Generated safe/private Rust and integrity bindings intentionally change,
not provider C/ABI, public signatures, descriptor or manifest schemas. These
regressions are authored but unrun.

The native correction is confined to the existing v10 owned-UTF8 provider
projection. Its per-function physical String owner cells are declared and
initialized before any recoverable failure branch. Emission records the exact
cells in bounded staged output; no heap-allocated runtime registry or resource
CleanupPlan change is introduced. Temporary-to-binding, branch, call, and
provisional-result handoffs transfer ownership rather than creating a second
cleanup owner. Every argument is evaluated before the complete String argument
group transfers to the callee. Normal scope exit settles nonescaping live
owners before loop reuse; the common failure exit settles every remaining
owner and preserves the primary status and caller result storage. Success settles non-result
owners before publication and relinquishes the result only after its store.

Native generation still emits the supplied program, not only the selected
public closure. The same physical bookkeeping must therefore cover emitted
String intrinsics and contracts in unselected functions; this does not admit
them in the public v10 closure. Allocation failure remains fail-stop. These
rules add no unwind, signal, or `longjmp` recovery guarantee.

The v10 correction changes only v10 native provider C and its dependent
integrity bindings. The subsequent [ordinary native String correction](NATIVE-INLINE-STRING-SETTLEMENT-V1.md)
reuses the ledger under a separate selection and leaves v10 provider bytes
unchanged. The separately amended [owned-data provider correction](NATIVE-OWNED-DATA-STRING-SETTLEMENT-V1.md)
also reuses that ledger and length-header representation for v8/v9 emitted
Strings, without changing v10 output or widening selected closure admission.
Frozen command/callable projections retain their separate String limitations.
Context-handle closure alone is not proof that
pre-handle String allocations were freed. Cross-backend failure-settlement
equivalence and native sanitizer evidence remain unrun gates before promotion.

## Authored evidence

The shared [descriptor cross-replay cases](PUBLIC-OWNED-DATA-API-V1.md#canonical-public-api-descriptor)
include authentic v10 `Bytes` and owned-UTF8 counterparts under the same profile
and synthetic subject facts. Self-replay succeeds; each correctly digested
descriptor must reject against the other's retained HIR. This is signature
binding evidence authored but unrun, not cross-schema rejection, source
provenance, behavioral equivalence or target execution.

`tests/project_v10_recipe_consumer_v1.rs` uses a real four-source Project,
with two imported owned-String helpers sharing a display name but retaining
different stable identities, including source-escaped control characters. It
replays the actual inline npm carrier and reopens all six published artifacts
for exact equality. Its Node consumer uses the published bindings and Wasm,
covering empty text, leading BOM, embedded NUL, multibyte text, raw malformed
UTF-8 remaining `Bytes`, late-argument arithmetic failure and subsequent
reuse. A helper display-only rename must preserve every descriptor fact except
the three Project revision/graph bindings. Native coverage in this fixture
stops at the compiler-replayed package passed to an intentionally rejecting
publisher; it does not compile, publish, or consume a native SDK. No test was
executed while authoring this fixture.

```sh
cargo test --locked -p semaprax --test project_v10_recipe_consumer_v1
```

The separate provisioned gate
`crates/semaprax-toolchain/tests/project_owned_utf8_sdk_v1.rs` consumes the same
four-source fixture through the real private native publisher. It regenerates
the provider from retained checked Project HIR, reopens the exact seven-file
SDK, and compares its canonical manifest against that descriptor/provider
binding and the reopened file hashes. This test-specific consistency check is
not an independent proof of archive provenance or provider semantics.
An unchanged external Rust consumer then builds in an isolated workspace with
`--locked --offline` and the existing short Cargo target-directory guard. Its
safe API forbids unsafe code and covers the same primary Node corpus: exact
17-byte BOM/NUL/Unicode text, empty text, raw malformed UTF-8, 65,536-byte
`Bytes`, capacity-plus-one input rejection, and repeated checked failure and
recovery through two SDK objects. Retained `String` and `Vec<u8>` values remain
independent after later calls, mutation and SDK destruction. These observations
do not replace physical allocation accounting or prove reuse of one initialized
native context; the maximum-byte case here is not a maximum `String` result.
The gate also checks exact revision-only descriptor changes after helper rename,
`SPX-I234` no-clobber rejection, unchanged consumer inputs, SDK artifacts and
Project source bytes.

This gate is explicitly ignored until selected on a provisioned host with
absolute `CLANG` and `SEMAPRAX_ARCHIVER` paths and native Cargo. Windows also
requires the existing `SEMAPRAX_VCTOOLS`, `INCLUDE`, `LIB` and
`SEMAPRAX_LINKER` configuration. Authoring or skipping it does not count as a
successful SDK gate, and it was not executed in this batch:

```sh
cargo test --locked -p semaprax-toolchain --test project_owned_utf8_sdk_v1 -- --ignored
```

The separate `tests/support/owned_utf8_capacity.rs` subject isolates the String
result boundary in a two-source Project. A single selected literal contains
65,535 or 65,536 UTF-8 bytes, including repeated BOM, NUL, multibyte and astral
characters. No other String literal consumes the shared 65,536-byte Wasm
literal pool. The real npm and native Rust publication gates compare returned
strings against independently spelled byte oracles, exercise repeated calls
through two instances, and retain earlier host values. Native host values are
also checked after SDK destruction. Package and source inputs remain exact.
The native gate reuses the same test-only manifest consistency oracle as the
multi-module SDK fixture; neither oracle establishes archive provenance.

A separate 65,537-byte source must fail ordinary Project admission with
`SPX-W110` (`owned UTF-8 literal table exceeds 65536 bytes`) before the Project
callback or publication. This is a compile-time literal-pool boundary, not
evidence of a native runtime over-limit rejection. These cases do not replace
physical allocation accounting, failure-path settlement, or maximum input
coverage. Both gates are authored but unrun; the native one is explicitly
ignored and requires the same provisioned tools described above:

```sh
cargo test --locked -p semaprax --test project_owned_utf8_capacity_v1
cargo test --locked -p semaprax-toolchain --test project_owned_utf8_capacity_v1 -- --ignored
```

The shared lower v8/v10 descriptor reader also rejects repeated parameter
identities within one export, even under a freshly computed descriptor digest;
see the [owned-data descriptor contract](PUBLIC-OWNED-DATA-API-V1.md#canonical-public-api-descriptor).
This tightens malformed-input rejection only, without changing emitted package
bytes, public signatures, or schema identities.

Focused authored lifetime evidence is in
`tests/project_owned_utf8_lifetimes_v1.rs` and its raw-arena/real-facade Node
consumer. It derives and replays the real descriptor before target generation;
locals and Copy-only loops occur inside admitted direct-call arguments. The
loop case retains one String before the loop and settles it on exit; it does
not allocate Strings per iteration. Direct String loop storage remains
`SPX-T252`, and an otherwise legal scalar helper staging Strings remains
outside the selected v10 closure (`SPX-J113`). Negative cases
preserve the rejected outer-body String forms. The native O0/O2 fixture is
success-value evidence only, not native failure-path allocation evidence.

```sh
cargo test --locked -p semaprax --test project_owned_utf8_lifetimes_v1
```

`tests/native_owned_utf8_settlement_v1.rs` independently derives/replays the
descriptor and emits the actual v10 length-header native provider. Its fixed
test-only allocation table observes provider allocations and frees at O0/O2:
local, late-argument, nested-call, callee, and loop failures must leave no
allocation live and must preserve poisoned result fields. Subsequent calls
reuse the same context. Success covers clones, branches, mixed Bytes/String,
more than sixteen owners, empty text, embedded NUL, and multibyte UTF-8.
Separately labeled raw semantic calls exercise emitted-but-unselected String
intrinsics, equality, guarded matching, and failed postconditions without
widening admission.
The generated Rust package consumer exercises safe API reuse after failure;
it is not a substitute for the physical allocation counter.

```sh
cargo test --locked -p semaprax --test native_owned_utf8_settlement_v1
cargo test --locked -p semaprax --test project_native_rust_owned_utf8_v1
```

The separate sanitizer gate requires an installed absolute
`SEMAPRAX_STRING_SANITIZER_CLANG` path with ASan/UBSan support:

```sh
cargo test --locked -p semaprax --test native_owned_utf8_settlement_v1 provisioned_native_string_provider_asan_ubsan -- --ignored --exact
```

Allocation counters establish the leak assertion even on hosts without
LeakSanitizer; ASan/UBSan are additional memory/undefined-behavior checks, not a
claim of LeakSanitizer coverage. None of these new gates was run in this batch.

## Non-claims

The implementation and its executable evidence are authored but were not run.
No generated package is published, and neither local nor hosted promotion is
claimed. Project v10 remains blocked on Project v9 promotion; the upstream
baseline at `4cc03820c86e70527cb65c4b10ee3841c7af167d` predates both profiles.

This profile adds no command, filesystem, process, network, daemon, recovery,
arbitrary publication, or general text-streaming authority. It does not decode
raw `Bytes`, expose a public aggregate ABI, or weaken Project v1-v9 behavior.
