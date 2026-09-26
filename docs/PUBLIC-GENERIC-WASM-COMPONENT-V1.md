# Public Generic Wasm Component v1

Audience: compiler contributors and Wasm component integrators.

Status: **private local execution profile; not public support**.

This specification defines an additive callable Component Model artifact over
the existing `public-generic-wasm-provider.v1` subject. It does not change the
WIT type projection, descriptor, carrier, Project profile, legacy build routes,
or PG-9 decision.

In plain terms: this produces one private Component Model artifact from an already admitted provider; it does not make the provider public.

## Admission and derivation

Input is the checked `ProjectRevision` and its retained
`AdmittedPublicGenericEndpointV1`. The endpoint stays synchronous and
effect-free, with exactly two ordered owned-`Bytes` leaves on both sides. This
admits no new source form or leaf count.

Derivation is deterministic and takes no caller-selected descriptor, endpoint,
binding, WIT, or toolchain facts. The artifact binds the exact replayed
descriptor bytes, the component-specific provider Core Wasm bytes, the Core
bridge/module composition, and the final Component bytes. A replay API
re-derives the provider and component from the same retained revision and
rejects any component-byte, component-digest, descriptor-digest, or provider-
digest mismatch before execution. The retained revision exposes
`public_generic_wasm_component_artifact_v1` and
`replay_public_generic_wasm_component_v1`; replay does not accept replacement
source, endpoint, or binding authority.

## Private calling convention

The Component exports one versioned `adapter` interface. Its callable surface
is intentionally flattened to the descriptor's ordered leaf inventory; the
existing type-only WIT projection continues to describe the authored record
types and is not reinterpreted as a calling convention.

```wit
package semaprax:public-generic-component@0.1.0;

interface adapter {
  resource owned-bytes {
    constructor(payload: list<u8>);
    read: func() -> list<u8>;
  }

  enum failure {
    contract-violation,
    resource-or-provider-refusal,
  }

  invoke: func(left: own<owned-bytes>, right: own<owned-bytes>)
    -> result<tuple<own<owned-bytes>, own<owned-bytes>>, failure>;
}

world public-generic-component-v1 {
  export adapter;
}
```

The first and second parameters are exactly input-leaf ordinals 0 and 1; the
successful result contains exactly result-leaf ordinals 0 and 1. They are not
reordered, sorted, or inferred from source spelling. The Component-owned
`owned-bytes` resource constructor copies the input list into a bounded
component resource. `read` returns a copy and does not consume the resource.
Consuming an `own` value transfers it to `invoke`; every success and error path
settles the consumed inputs. Dropping an owning resource invokes the generated
in-component destructor, which clears its bounded storage. A stale, closed,
borrowed-as-owned, or otherwise invalid handle fails closed.

The generated Core bridge constructs and validates the existing canonical
descriptor-bound input carrier, calls the exact checked provider, validates
and decodes its result carrier, and constructs output resources. It does not
call an imported host adapter, WASI service, or ambient capability. Core
provider status maps to the closed `failure` cases above; no unknown status is
treated as success. Checked pre/postcondition failure is reported as
`contract-violation`. Provider/codec refusal maps to
`resource-or-provider-refusal`. Exhausting the fixed Component resource slots
traps in the in-component constructor; it is not represented as either enum
case. Core execution is synchronous; this profile defines no retry or
cancellation behavior.

## Bounds and compatibility

Per-resource, total-payload, carrier-wire, descriptor, semantic-graph, and
provider bounds are inherited without increase from the owning descriptor,
carrier, and provider specifications. Resource-slot exhaustion traps;
aggregate payload and carrier limit violations detected by the checked
provider remain refusals. The resource arena starts on a page boundary after
the provider's bounded scratch/private workspace. Component-owned list staging
uses a separate 64 KiB realloc range; the constructor reclaims that one-list
cursor only after copying into resource storage, and positive-size realloc
limit/growth failures trap rather than returning address zero.

The standalone `public-generic-wasm-provider.v1` emission keeps its original
bytes, memory maximum, and reserve instruction stream. The component-specific
provider variant intentionally differs only by its two private codec-helper
exports, its disjoint private layout for codec workspaces/tables, maximum-size
payloads, aggregate records, and result carrier, the already-grown reserve
guard, and the larger bounded Core memory maximum needed for that layout and
the component resource/staging arenas. The Component-specific
artifact digest is separate from the Core provider binding domain; stable IDs
and the standalone provider binding remain unchanged.

WIT projection, compatibility reports, and Wasm provider v1 remain
independently versioned. Adding this callable artifact does not change their
bytes or imply compatibility with other WIT/component producers.

## Host admission and differential conformance

This section adds no new calling convention: the v1 `adapter` interface
above stays byte-for-byte unchanged. It fixes how a host binds and checks a
Component for one checked source endpoint.

Descriptor binding is enforced before instantiation. A host adapter admits
candidate Component bytes only after
`replay_public_generic_wasm_component_v1` on the retained revision re-derives
the identical Component from the compiler-owned provider and all three
claimed identities (Component, descriptor, provider) match, and after the raw
Component SHA-256 matches an independently pinned value. A Component derived
from another revision of the same stable declarations, or presented under
another endpoint's descriptor digest, is refused before `Component::new`; it
is never instantiated and never called. Inside the Component, every `invoke`
still opens the embedded checked provider with its embedded descriptor and
binding, so a guest cannot run the provider against a different descriptor.
The standalone Core provider refuses a stale descriptor at
`spx_pg_v1_open`, before any input is staged.

Result and error mapping for the admitted two-leaf owned-`Bytes` subject:

| Endpoint outcome | Interpreter (retained call) | Core provider | Component |
| --- | --- | --- | --- |
| success | owned `LeafPair` record, two settled leaves | status `0`, result carrier | `ok((leaf 0, leaf 1))` |
| checked `requires`/`ensures` failure | contract status, zero settled leaves | status `11` (contract) | `err(contract-violation)` |
| provider or codec refusal | not applicable | nonzero refusal status | `err(resource-or-provider-refusal)` |
| resource-slot exhaustion, list above 64 KiB | not applicable | not applicable | trap (no enum case) |

Ownership settlement: both `own` inputs are consumed exactly once by
`invoke` on every success and error path; the caller owns both result
resources and must drop them. After any call returns, all 64 fixed resource
slots can be held live simultaneously, which proves that neither inputs nor
provider state leaked. A trap is not a typed failure; its Store is discarded
and a fresh instance of the same Component is usable. The Component has zero
imports, so guest-to-host re-entry is structurally impossible; sequential
calls on one instance are ordinary re-entry and are exercised.

Local differential evidence uses the checked-in
`platform-tests/component-runtime/fixtures/public-generic-parity-v1` Project,
whose endpoint swaps its two leaves (a non-identity body), and
`public-generic-parity-failure-v1`, whose endpoint has `requires false`.
Each Project also carries a monomorphic `provider.witness(left, right)`
adapter that builds the endpoint's owned `Envelope<LeafPair>`, calls the
endpoint and returns its `LeafPair`; the reference interpreter evaluates it
through its retained-call seam. For each input the interpreter, the
standalone compiled Core provider (driven through its closed `spx_pg_v1_*`
ABI in Wasmtime 47.0.4) and the Component (typed Component Model bindings,
Wasmtime 47.0.4) must return identical leaves or the identical checked
contract failure. A Component skipping descriptor-bound admission fails the
selector: the negative control was run once and reverted.

The same harness found a standalone Core provider defect: when the two input
payloads together exceed 2048 bytes, the provider's input aggregate record
overwrites payload bytes and the call still reports success. The
Component-specific provider layout does not share the overlap. Three-way
agreement is therefore gated for inputs up to exactly 2048 combined bytes.
Interpreter and Component agreement is gated through the 64 KiB per-leaf
bound. The ignored selector
`large_payload_core_provider_matches_component_and_interpreter` reproduces
the defect and becomes the Core gate once the provider is fixed.

## Evidence and nonclaims

The current local evidence includes deterministic retained-revision
derivation/replay, tampered component/provider-digest metadata refusal, and one
successful invocation under the repository-pinned Wasmtime 47.0.4 harness. The
runtime selector checks no ambient imports, ordered two-leaf byte identity at
the exact 64 KiB per-leaf bound, preservation of an unrelated live resource,
resource read/drop, 200 maximum-size constructor/read/drop reuse cycles, and a
second successful invocation after replay rejected tampered bytes. It also
uses copied handles to require read and double-drop refusal after an explicit
close or transfer, then constructs, reads and drops a fresh resource in the
same instance. This is focused private evidence, not a support claim.

A separate Wasmtime 47.0.4 selector instantiates the exact retained Component
twice in one Store, requires the second instance's `read` to refuse the first
instance's resource with a resource-type mismatch, and proves the first
resource remains readable/droppable before the second instance successfully
constructs, reads and drops a fresh resource. This is foreign-instance
resource-owner refusal evidence only.

The separate contract-failure selector retains a second checked Project with
the same two-leaf endpoint and a `requires false` guard. It replays the exact
derived Component against that revision, invokes it in Wasmtime 47.0.4, and
requires the typed `contract-violation` result rather than a trap or success.
It then keeps all 64 fixed-arena resources live at once, proving the two
consumed input slots were settled before their replacements were constructed.
After dropping all 64, a new resource can be constructed, read and dropped.
In a separate disposable Wasmtime Store, the 65th live constructor traps.
The trapped instance is not claimed to support destructor re-entry; its Store
is discarded. This is local failure-path and saturation evidence, not
cross-target parity.

Native C11 `-O0`/`-O2` parity, Core-Wasm parity above 2048 combined input
bytes, and broader resource/payload hostile cases remain unclaimed. The
interpreter/Core/Component differential and host-side stale-descriptor
refusal described above are the only cross-engine claims. The
retained artifact replay test rejects mutated Component bytes and mismatched
provider-digest metadata. It also refuses old Component bytes after an
authenticated source-body change with stable declaration identities, then
accepts the newly derived artifact for that changed revision. The runtime test
itself does not execute a tampered candidate. Component bytes remain immutable
during the successful runtime test and the Component requests no ambient
imports.

Cancellation, hosted/provider acceptance, publication, PG-9 support, arbitrary
Component Model inputs, effects, asynchronous work, and every source shape
beyond the two-leaf owned-`Bytes` provider slice are explicitly unclaimed.
