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

### Native C11 `-O0`/`-O2` fourth column

`platform-tests/component-runtime/src/public_generic_component_tests/parity/native.rs`
adds native C11 execution, compiled and run at both `-O0` and `-O2`, as a
fourth compared engine for the same two behavior families above (the
non-identity swap and the `requires false` contract failure), over the same
left/right byte vectors and the same expected `Outcome`. It compiles the
compiler-derived `semaprax.authenticated-native-moves-nested.v1` provider
(`render_authenticated_nested_moves_provider`, real checked HIR, never a
hand-authored reimplementation of a checked body) and drives it through the
generated C11 calling consumer
(`generate_authenticated_nested_moves_calling_consumer_v1`), the same
calling convention and `clang -std=c11 -Wall -Wextra -Werror` invocation the
read-only reference
`tests/public_generic_native_adapter_v1/authenticated_handoff/checked_moves.rs`
established, at each optimization level.

**Same endpoint as every other column (issue #292).** The interpreter/
Core-provider/Component columns above and the native column now all bind the
SAME checked-in parity fixture's actual `provider.transform` endpoint, whose
parameter is the nested `Envelope<LeafPair>` (a record wrapping a record) --
`acquire()` derives one retained `endpoint`/`descriptor` and passes it to
`native::build` directly, rather than the native column deriving a separate
fixture. `admit_component`/`core_call`'s stale-descriptor refusals above and
`acquire`'s own `native.descriptor_bytes() != endpoint.descriptor_bytes()`
check together prove all four engines execute one identical checked body
over byte-identical descriptor bytes, not four independently checked
lookalikes.
The flat-only `authenticated-native-moves.v1` profile
(`src/codegen/native_emit/public_generic_bridge.rs::admit`) is unchanged and
still refuses `provider.transform`'s own descriptor with `SPX-B103`
("requires a flat Bytes movement body"), since its `input_facts().fields` is
one record-typed `payload` field, not two `Bytes` leaves; the separate,
additively versioned `authenticated-native-moves-nested.v1` profile
(`admit_nested_moves`, `owned_bytes_leaf_field_paths`) is what admits and
lowers this nested shape -- see `docs/PUBLIC-GENERIC-CARRIER-V1.md`'s own
versioned section on it.

## Cancellation and mid-call interruption (v1)

This profile defines synchronous, effect-free calls and no retry or
cancellation protocol of its own (stated above). This section defines and
compares what each engine does when a call is interrupted before it would
otherwise return -- a Wasmtime fuel budget exhausted partway through
execution, the only interruption primitive this profile's harness has
access to (no epoch deadline is configured anywhere in this repository; fuel
is already enabled on every `Engine` this document's tests build).

| Engine | What an interruption looks like | Result |
| --- | --- | --- |
| Component | `Store::set_fuel` exhausted inside `invoke` | Wasmtime trap (`Err`, not a typed `failure`); the `Store` -- and every resource, including both still-owned inputs, it held -- is discarded; a fresh instance of the identical Component bytes is usable |
| Core provider | `Store::set_fuel` exhausted inside `spx_pg_v1_call` | Wasmtime trap; `spx_pg_v1_provider_close` is never reached; the `Store` (the one Wasm linear memory everything the provider allocated lived in) is discarded; a fresh module instance is usable |
| Interpreter | the retained-call step budget (`INTERPRETER_MAX_STEPS`, library default 1,000,000; see `docs/INTERPRETER-V1.md`) is exhausted before evaluation finishes | `RetainedCallOutcome::FuelExhausted`, a fail-closed interpreter capacity fact distinct from any language status; zero cleanup/settlement events are ever produced for an exhausted evaluation, so no partial result is published |
| Native (`authenticated-native-moves-nested.v1`) | not applicable | an in-process synchronous C call has no interruption primitive in this profile: no async work, thread, signal handler, or timeout is admitted (see `docs/PUBLIC-GENERIC-CARRIER-V1.md`'s native-adapter thread/signal restrictions); only killing the whole host process could stop a call short, which is not a documented or tested API guarantee here |

In every case that actually admits interruption (Component, Core provider,
interpreter), no partial result is ever published and every resource the
interrupted call held is released with it -- for Wasmtime, because the whole
`Store` (and the one linear memory or resource table it owns) is discarded
rather than reused; for the interpreter, because cleanup/settlement events
are only ever emitted once evaluation actually returns. This is the same
"trap is not a typed failure; its Store is discarded" rule already stated
above for resource-slot exhaustion and the oversized-list bound, generalized
to an interruption that can land at any point in a call rather than only at
its start.

`platform-tests/component-runtime/src/public_generic_component_tests/parity/cancellation.rs`
exercises the Component and Core-provider rows: each test measures one full
successful call's own fuel cost on a disposable `Store`, then repeats the
call on a fresh instance with the fuel budget reduced to roughly half of
that measured cost so the exhaustion point falls inside the call rather
than merely refusing to start it, requires the call to fail (a Wasmtime
`Err`, never a typed result), discards that `Store` without ever reaching a
provider-close or reading a result, and then proves a fresh instance of the
identical bytes still completes the checked call and (for the Component)
that its full fixed resource arena is available again. A negative control
(inflating the reduced budget so the call would not be interrupted) was run
once to confirm both assertions have teeth, then reverted. The interpreter's
own step-budget analogue is exercised by its own existing suite
(`src/interpreter/retained_call/owned_handoff/tests.rs`), not duplicated
here. The native row has nothing to execute: it is a scope statement, not an
unexercised test.

Epoch-based interruption, async cancellation, host-initiated abort of a
native in-process call, and any interruption behavior beyond Wasmtime fuel
exhaustion and the interpreter's own step budget remain unclaimed.

## Evidence and nonclaims

The current local evidence includes deterministic retained-revision
derivation/replay, tampered component/provider-digest metadata refusal, and one
successful invocation under the repository-pinned Wasmtime 48.0.5 harness. The
runtime selector checks no ambient imports, ordered two-leaf byte identity at
the exact 64 KiB per-leaf bound, preservation of an unrelated live resource,
resource read/drop, 200 maximum-size constructor/read/drop reuse cycles, and a
second successful invocation after replay rejected tampered bytes. It also
uses copied handles to require read and double-drop refusal after an explicit
close or transfer, then constructs, reads and drops a fresh resource in the
same instance. This is focused private evidence, not a support claim.

A separate Wasmtime 48.0.5 selector instantiates the exact retained Component
twice in one Store, requires the second instance's `read` to refuse the first
instance's resource with a resource-type mismatch, and proves the first
resource remains readable/droppable before the second instance successfully
constructs, reads and drops a fresh resource. This is foreign-instance
resource-owner refusal evidence only.

The separate contract-failure selector retains a second checked Project with
the same two-leaf endpoint and a `requires false` guard. It replays the exact
derived Component against that revision, invokes it in Wasmtime 48.0.5, and
requires the typed `contract-violation` result rather than a trap or success.
It then keeps all 64 fixed-arena resources live at once, proving the two
consumed input slots were settled before their replacements were constructed.
After dropping all 64, a new resource can be constructed, read and dropped.
In a separate disposable Wasmtime Store, the 65th live constructor traps.
The trapped instance is not claimed to support destructor re-entry; its Store
is discarded. This is local failure-path and saturation evidence, not
cross-target parity.

Native C11 `-O0`/`-O2` execution evidence now exists (see above) against
`provider.transform`'s own literal nested-record descriptor bytes, over the
same two behavior families as the other three columns, through the separate
`authenticated-native-moves-nested.v1` profile; the previously stated gap
(widening the native profile to admit a nested record body) is closed by that
profile (issue #292). Core-Wasm parity above 2048 combined input
bytes and broader resource/payload hostile cases also remain unclaimed. The
interpreter/Core/Component/native differential and host-side
stale-descriptor refusal described above are the only cross-engine claims.
The retained artifact replay test rejects mutated Component bytes and
mismatched provider-digest metadata. It also refuses old Component bytes
after an authenticated source-body change with stable declaration
identities, then accepts the newly derived artifact for that changed
revision. The runtime test itself does not execute a tampered candidate.
Component bytes remain immutable during the successful runtime test and the
Component requests no ambient imports.

Fuel-exhaustion mid-call interruption for the Component, Core provider and
interpreter is now defined and (for the first two) tested (see "Cancellation
and mid-call interruption" above); native has no interruption primitive to
test. Epoch-based interruption, async cancellation, hosted/provider
acceptance, publication, PG-9 support, arbitrary Component Model inputs,
effects, asynchronous work, and every source shape beyond the two-leaf
owned-`Bytes` provider slice are explicitly unclaimed.
