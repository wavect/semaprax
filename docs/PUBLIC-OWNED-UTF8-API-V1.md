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

The root compiler alone authenticates provider semantics from replayed HIR.
The unpublished lower package authenticates the closed descriptor, provider
byte integrity, compiler-declared textual binding, held tools, and filesystem
publication facts; it verifies the renamed stage through retained stage
authority and receives no HIR or independent semantic-proof authority.

## Settlement

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

The authored shared Rust invocation guard additionally proves the complete
provider context settled before any outward value or recoverable error,
including a UTF-8 conversion failure. An inner owner guard precedes context
closure on Rust unwind; uncertain settlement remains fail-stop. Only a
proven-closed context may be reinitialized on a later call. Its private
invocation counter resets while the linked provider's handle issuer remains
nonreused. Generated safe/private Rust and integrity bindings intentionally change,
not provider C/ABI, public signatures, descriptor or manifest schemas. These
regressions are authored but unrun.

Known separate gap: native C11 failure exits can bypass inline cleanup of
String locals and provisional results. The Wasm correction does not fix that
native physical-allocation gap, and context-handle closure alone is not proof
that those allocations were freed. Cross-backend failure-settlement equivalence
and native sanitizer evidence remain open before promotion.

## Authored evidence

Focused authored lifetime evidence is in
`tests/project_owned_utf8_lifetimes_v1.rs` and its raw-arena/real-facade Node
consumer. It derives and replays the real descriptor before target generation;
locals and loops occur inside admitted direct-call arguments. Negative cases
preserve the rejected outer-body String forms. The native O0/O2 fixture is
success-value evidence only, not native failure-path allocation evidence.

```sh
cargo test --locked -p semaprax --test project_owned_utf8_lifetimes_v1
```

## Non-claims

The implementation and its executable evidence are authored but were not run.
No generated package is published, and neither local nor hosted promotion is
claimed. Project v10 remains blocked on Project v9 promotion; the upstream
baseline at `4cc03820c86e70527cb65c4b10ee3841c7af167d` predates both profiles.

This profile adds no command, filesystem, process, network, daemon, recovery,
arbitrary publication, or general text-streaming authority. It does not decode
raw `Bytes`, expose a public aggregate ABI, or weaken Project v1-v9 behavior.
