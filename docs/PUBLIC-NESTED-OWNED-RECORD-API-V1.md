# Public Nested Owned-Record API v1

Status: internal additive Project-v11 implementation tranche; unpublished and
unpromoted.

Audience: compiler contributors, generated-package integrators, and promotion
reviewers.

## Purpose and fixed identifiers

Project v11 exposes the already admitted bounded nested owned-record result
semantics through one target-neutral public description and two generated host
adapters. It does not change the source language or reinterpret Project v1-v10.

| Layer | Identifier |
| --- | --- |
| Project schema | `semaprax.project.v11` |
| Project profile | `nested-owned-record-api.v1` |
| API descriptor | `semaprax.public-nested-owned-record-api.v1` |
| npm metadata | `semaprax.nested-owned-record-api.v1` |
| npm carrier | `semaprax.project-npm-build.v10` |
| Rust SDK manifest | `semaprax.native-rust-nested-owned-record-sdk.v1` |

The canonical manifest has the same eight ordered assignments and bounds as
Project v9. Schema and profile select each other exactly. No earlier schema may
select this profile, and v11 may not select an earlier profile.

## Closed admission

Each selected export has the existing Project-v8 parameter vocabulary: by-value
`i64` and `bool`, plus invocation-borrowed `str` and `Slice<u8>`. Its result is
one source-authored monomorphic acyclic record tree containing only:

- `i64`, `bool`, or `usize` leaves;
- one or more owned `Bytes` leaves; and
- other admitted source-authored monomorphic records.

Every selected function, parameter, record, and field has the identity required
by the existing public owned-data profiles. Selected closures remain bounded,
effect-free, import-free, contract-free, and monomorphic. Generic records,
variants, `String`, resources, classes, arrays, stored borrows, owned parameters,
borrowed results, nested input records, and every other result shape reject
before target generation.

Classification is iterative and uses checked accounting. A result tree admits
at most 64 record levels, 256 owned leaves, and 4,096 examined fields. The
complete linked function inventory remains at most 256, exports at most 32,
parameters per export at most 8, canonical descriptor bytes at most 1 MiB, and
the cumulative owned output of one call at most 65,536 bytes. Exact limits
admit; the first excess rejects before any target or publication effect.

## Canonical descriptor

The descriptor binds the exact Project revision, Workspace revision, Project
graph digest, selected exports, parameters, nominal result records, and carrier
leaf occurrences. Nominal record declarations appear once in a deterministic
stable-ID table. Each record retains its explicit declaration identity,
presentation name, injective host name, and declaration-ordered fields. A field
is either a closed scalar/owned-Bytes kind or an exact reference to another
record declaration.

Every carrier occurrence is identified by the root result record plus its full,
nonempty declaration-ID field path. Reusing one nominal child record at two
siblings therefore creates two distinct ownership occurrences. Display names,
source offsets, target offsets, padding, generated symbols, pointers, and arena
tokens never identify ownership.

Derivation and replay independently walk validated retained HIR with bounded
worklists. Replay checks closed JSON objects and tags, exact declaration and
field inventories, canonical ordering, reachability, acyclicity, path/type
agreement, all limits, terminal LF, absence of NUL, the domain-separated digest,
and exact canonical byte reconstruction. Submitted record tables or paths never
override retained HIR facts. A descriptor grants no source, target, allocator,
filesystem, process, publication, or execution authority.

## Private carriers and atomic settlement

There is no public C, Rust, or Wasm aggregate layout. Each target independently
validates its compiler-private aggregate layout against retained HIR, then
flattens leaf occurrences into a private fixed-width carrier in descriptor
order. Scalar slots are copied values. Owned slots contain invocation-local
opaque handles only.

Before transferring any owner, a provider authenticates the complete result
shape, every scalar, every owned leaf, cumulative byte length, all required free
slots, and all serial identities. The batch commit then transfers all owners in
one non-failing transition and publishes the complete carrier last. A capacity,
serial, shape, or arithmetic failure before that transition leaves all semantic
owners live. No adapter may implement the batch as repeated fallible single-owner
attachment.

A host validates every slot and every handle before the first settlement. It
copies all owned payloads into fresh host-owned buffers while one guard retains
the complete provider-handle set, settles every provider handle exactly once,
proves the provider context closed, and only then constructs and publishes the
nested host value. Host allocation or copy failure settles the complete set;
settlement uncertainty is fail-stop. Scalar values and partially constructed
objects are never outwardly observable. An ordinary language failure leaves the
caller's output untouched and may be followed by another proven-clean call.

The generated safe Rust module forbids unsafe code and exposes recursively typed
public structs containing `i64`, `bool`, `usize`, nested structs, and `Vec<u8>`.
Its context, raw carrier, handles, and unsafe FFI remain private and not
`Send`/`Sync`. The npm facade exposes recursively readonly TypeScript interfaces,
copies every `Uint8Array`, builds and freezes nested JavaScript objects only
after complete settlement, and permanently poisons an instance after unexpected
engine behavior, malformed success, reentry, or uncertain cleanup.

## Cross-target replay

The reference interpreter, native C provider/safe Rust package, and Core-Wasm
provider/npm package consume the same canonical descriptor and exact retained
program. Native validates Native64 layouts; Wasm validates Wasm32 layouts. These
target-private facts may differ without changing the public descriptor.

All three lanes must agree on selected function, parameter order, nested result
shape, scalar values, byte contents, language failure, left-to-right evaluation,
owner settlement, and absence of partial publication. Native evidence executes
the emitted C at `-O0` and `-O2`; portable evidence validates the module and
executes the generated package under Node. Neither is evidence for a Component,
browser matrix, registry publication, or stable aggregate ABI.

## Compatibility

Project v11 uses new manifest, descriptor, metadata, carrier, and SDK-manifest
schemas and new digest domains. Project v8-v10 descriptor bytes, Wasm bytes,
JavaScript templates, npm artifacts, native provider sources, generated Rust
packages, diagnostics, and known answers remain byte-identical. In particular,
v11 does not widen the v9 single-owner carrier or the shared v8-v10 JavaScript
state machine.

## Evidence gate

Promotion requires all of the following at one exact revision:

- canonical manifest, descriptor, metadata, npm carrier, and Rust SDK replay;
- canonical source round trip and exact retained-HIR identity/path agreement;
- exact and plus-one depth, owned-leaf, examined-field, descriptor, cumulative
  output, handle, and generated-package bounds;
- duplicate, reordered, missing, unreachable, cyclic, foreign, stale, swapped,
  truncated, trailing, one-bit-mutated, and re-digested descriptor rejection;
- shared nominal child types at multiple occurrence paths without owner
  collapse;
- rejection of generic, variant, String, resource, class, array, stored-borrow,
  owned-parameter, nested-input, and borrowed-result shapes;
- failure before and after every initializer, batch-attachment boundary, host
  copy, settlement, context close, and outward publication boundary;
- duplicate, zero, stale, foreign, reincarnated, wrong-context, wrong-order, and
  capacity-exhausted handle cases with no partial transfer;
- exact cumulative output accounting across all owned leaves and repeat entry
  after every recoverable semantic failure;
- normalized interpreter, native `-O0`/`-O2`, and Node/Core-Wasm result and
  cleanup parity;
- locked/offline external Rust and Node consumers with no repository source or
  workspace dependency; and
- frozen Project v1-v10 and public v8-v10 artifact/diagnostic known answers.

Authored tests, one backend, structural compilation, descriptor equality, or a
private consumer alone do not satisfy promotion. The completion matrix remains
Partial until the full selected gate passes and an explicit publication decision
is recorded.

## Nonclaims

Project v11 does not provide a public C/Wasm aggregate layout, WIT or Component
ABI, zero-copy output, allocator transfer, foreign ownership, nested parameters,
mutable host views, escaping borrows, variants, generic aggregates, resources,
classes, `String` fields, concurrency, registry publication, browser support, or
production readiness by itself.
