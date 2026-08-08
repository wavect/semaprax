# WebAssembly owned-resource ABI v1

Status: implemented narrow Core Wasm slice; the full cross-target vertical
contract remains open

`semaprax.wasm-owned.v1` is the first public WebAssembly execution path for
SEMAPRAX uniquely owned resources. It is intentionally smaller than the final
RFC 0003 ABI. Unsupported resource shapes still fail with `SPX-W111`.

## Admission

The compiler admits a function only when all of the following are proven from
validated HIR and its independently replayed `semaprax.cleanup-plan.v1`:

- the module declares exactly one direct resource identity and its lifecycle is
  `drop trivial`;
- every resource parameter is direct, non-generic, and `own`;
- scalar parameters are `i64` or `bool` values;
- the result is `i64` or one direct owned input resource;
- the root body has no statements or calls and is a literal, scalar parameter,
  checked addition, or owned-parameter identity;
- contracts are in the narrow Boolean/lowerable comparison corpus; and
- every terminal cleanup action maps exactly once to the lifecycle and
  parameter proven by the attached plan.

Records, imported lifecycles, borrows, shared resources, general calls,
resource construction, resource-containing aggregates, generic resources,
and broader control flow remain excluded. Record diagnostics retain `SPX-W110`
precedence.

The emitter consumes `ExitTarget.finalize_in_order` verbatim. It neither sorts
cleanup actions nor reconstructs destruction order from source syntax. Owned
publication is admitted only when the plan commits an owned provisional result
and the HIR result is the same owned parameter.

## Core Wasm ABI

Each admitted function is listed in `semaprax.manifest.json` and exported as a
deterministic `semaprax_owned_N` function in declaration order:

```text
(context: i32, source parameters..., result_out: i32) -> status_token: i32
```

Zero status means success. A nonzero token resolves in the instance's status
arena. `result_out` is written only after requirements, checked operations,
postconditions, and plan-driven cleanup succeed. An `i64` result occupies eight
little-endian bytes; a resource result is one opaque `i32` handle. Before any
owner is staged, the emitted adapter checks unsigned pointer-plus-width
arithmetic, complete linear-memory bounds, and natural 4- or 8-byte alignment.
An invalid low-level address returns adapter status code 6, preserves the out
slot, and consumes nothing. The JavaScript facade always uses aligned address
zero and independently checks poison preservation on failure.

Compiler failures normalize to `semaprax.status.v1` contract or arithmetic
records with canonical fields `schema`, `domain_id`, `code`, `class`, and
`retryable`. Adapter rejection uses `semaprax.wasm-adapter.v1`. `begin` reserves
the invocation's only mutable status cell before ownership can move; later
compiler or adapter failure fills that cell rather than allocating after
commit. Token `0x7fffffff` is an immutable, always-resolvable arena-exhaustion
status, so exhaustion is a stable non-mutating rejection rather than a host
exception. Status tokens are instance-local and never cross the semantic trace
boundary.

## Trusted JavaScript host

`semaprax.js` creates one host runtime for each owned Wasm instance. The trusted
v1 ingress is deliberately two-step:

```js
const ticket = owned.prepareTrustedAdoption(value);
const handle = owned.adopt(ticket);
```

The unforgeable runtime-branded ticket is one-shot. Reuse is rejected; if slot
allocation fails, the ticket remains unconsumed and may be retried. Distinct
tickets may intentionally carry equal payloads. Creating a ticket is the
trusted ownership assertion: the application adapter must already have proved
that the payload identity is exclusively owned and correctly typed. General
untrusted raw-object adoption is not a language operation.

The 31-bit nonzero handle representation contains:

- an 11-bit runtime tag;
- a 10-bit generation; and
- a 10-bit slot.

One non-configurable `Symbol.for` allocator coordinates runtime tags across
separately evaluated copies of the generated host in the same JavaScript realm.
A v1 application must run that host in a trusted realm where earlier code has
not preinstalled or replaced this reserved global binding. Arbitrary hostile
co-resident JavaScript can already replace WebAssembly and other host
primitives; defending such a realm is outside this ABI. Returned tags are still
range-checked and repeated tags within each generated host evaluation fail
closed.
A freed slot increments its generation before reuse; exhausted generations
retire the slot. Owned-runtime creation fails closed after 2,047 tags in that
realm, 1,023 simultaneous/reusable slots per instance, or 1,023 generations per
slot. Scalar-only modules create no owned runtime and consume no tag. Thus a
stale copy, consumed handle, duplicate owner argument, or handle from another
tested same-realm instance is rejected before commit. Cross-realm or worker
identity coordination is not part of v1.

An invocation performs one indivisible commit:

1. `begin` reserves a status cell before ownership can move;
2. every owner is staged in parameter order without mutation;
3. an owned-result slot/generation is reserved when needed;
4. `commit` revalidates the complete set, then changes every entry to
   in-flight as one synchronous host operation; and
5. emitted Wasm clears each liveness bit before calling `drop`, or publishes
   through the already-reserved result entry.

Any precommit rejection aborts reservations and leaves all owners usable.
After commit, cleanup and publication use only reserved table/status capacity;
trusted host imports are required to be synchronous, non-throwing, and
non-reentrant. The emitted Wasm, not a JavaScript shadow interpreter, drives
contracts, checked addition, cleanup order, liveness clearing, failure
selection, and out-slot publication.

The imports and instance-binding hooks are private to `instantiateBytes`; the
returned `owned` facade exposes neither. `owned.invoke` accepts only an exact
generated export, argument count, canonical signed-i64/scalar and positive-i32
handle representation, parameter-kind vector, and result kind.
`instantiateBytes` copies and authenticates its input against the SHA-256 digest
embedded in the generated host before creating a runtime or linking ownership
imports; arbitrary or mutated Wasm bytes are rejected.
The same deterministic metadata appears in `semaprax.web.v3`, including the
resource and lifecycle identities, so an arbitrary Wasm export or caller-chosen
result reinterpretation cannot enter the ownership transaction.

## Evidence and nonclaims

`tests/wasm_owned.rs` executes real generated core Wasm under Node and proves:

- deterministic bytes and manifest mappings;
- exact export/argument/result metadata rejection and private host imports;
- exact Wasm artifact authentication before ownership-host construction;
- one-shot adoption, capacity-retry behavior, and equal-payload distinct owners;
- atomic multi-owner rejection and replay safety;
- reverse exact-once cleanup from the validated plan;
- owned-result handle rotation and stale, same-module, and duplicated-module
  same-realm cross-instance rejection;
- requires, checked-overflow, and ensures failures;
- exact normalized status objects plus stable status/slot/owned-result-capacity
  exhaustion;
- poison-preserving result publication and precommit rejection of unaligned,
  negative-as-unsigned, and out-of-bounds result pointers;
- scalar packages surviving more than the complete owned-tag namespace without
  constructing an owned runtime; and
- hostile attached-plan rejection as `SPX-H006` before backend admission.

This slice does not yet claim WIT/component resources, callable imports,
imported finalizers, async or worker transfer, shared memory, reentrancy,
resource acquisition from SEMAPRAX source, cross-realm identity isolation,
an adversarially pre-poisoned same-realm global environment, aggregate shadow
stacks, semantic conformance-trace emission, or
native/reference/Wasm full-corpus conformance. Those remain required before the
broad WebAssembly resource row can be marked complete.
