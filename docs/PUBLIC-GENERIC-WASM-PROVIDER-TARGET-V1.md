# Public Generic Wasm Provider Target v1

Audience: compiler contributors and Wasm provider integrators.

Status: **internal admission and compiler-owned Core Wasm artifact implemented**.

This document owns the additive compiler target selected by the Package
Manifest v1 profile `public-generic-wasm-provider.v1`. It creates a checked,
replayable compiler subject and a closed executable provider for issues #162
and #229. It does not publish a supported public generic ABI.

In plain terms: the compiler can build this narrowly defined Wasm provider, but nobody is promised a supported public interface.

## Manifest contract

The profile is available only through canonical `semaprax.manifest.v1` tables.
It lowers to `semaprax.project.v20`; frozen positional v20 manifests fail.

The manifest must contain:

- exactly one stable ID in `[exports] web`;
- no command table;
- no capability request;
- no effect, interface, permit, or publication authority; and
- the ordinary bounded source and test-module inventories.

Existing profiles are unchanged. In particular, this one does not relax
`SPX-W115` or reinterpret an existing `web_export` calling convention.

## Admission product

`src/project/admission/public_generic_wasm.rs` selects the one checked export
from the aggregate-aware linked public program. It calls the existing Public
Generic Descriptor v1 producer and immediately replays the resulting bytes
through the independent verifier. The admitted endpoint retains, as one unit:

- the explicit endpoint stable ID and presentation name;
- the exact concrete generic input and result instance facts;
- the compiler-derived owned-leaf inventory and settlement plan; and
- the verified canonical descriptor bytes and descriptor digest bound to the
  exact project revision and checked program root.

Admission follows the frozen Public Generic Boundary Profile v1. The endpoint
is synchronous and effect-free, has exactly one `own` concrete authored record
instance input, and returns a concrete authored record instance. All existing
depth, field, leaf, payload, and cleanup-plan bounds continue to apply.

`ProjectRevision::public_generic_wasm_provider_endpoint_v1` does not trust the
retained Rust object as a shortcut. It replays the canonical bytes against the
retained checked program and exact project revision before returning a new
admitted value. Project Lock v1 records the replayed descriptor digest as the
profile's interface identity.

## Artifact boundary

`ProjectRevision::public_generic_wasm_provider_artifact_v1` replays the
retained endpoint and deterministically emits the dedicated artifact. The
following unrelated publication routes still fail before staging or creating
output:

- pathless and filesystem Web builds;
- npm/package builds;
- native executable builds; and
- Agent Transport build requests.

Their refusal names `public-generic-wasm-provider.v1`; this internal artifact
does not silently reopen legacy Web/npm/native/transport contracts.

## Implemented Phase B artifact

The emitter consumes only `AdmittedPublicGenericEndpointV1`; it does not
regenerate a fixture descriptor or accept caller-selected endpoint facts. It
deterministically emits one closed Core Wasm module with no ambient imports and
the versioned provider operations for:

1. provider open and exact descriptor/binding replay;
2. bounded canonical input preparation and copy-in;
3. whole-value transfer and invocation of the selected checked function;
4. private result staging followed by exact non-consuming copy-out; and
5. explicit value, result, and provider release.

The module owns scratch growth, opaque non-recycled handles, carrier SHA-256
replay, checked source invocation, result staging/export, and explicit
release/close transitions. Its binding covers normalized exact artifact bytes,
endpoint export, compiler backend, descriptor, runtime identity, and Core Wasm
target. When its binding selects the compiled provider, the generated
TypeScript runtime calls these exports and does not use its host allocator or
handle registry. The same generated file retains an explicitly selected
reference-provider route for the predecessor fixture and settlement corpora;
there is no fallback between the bindings.

The generated TypeScript package now emits canonical descriptor-bound carrier
frames and executes the complete lifecycle against this compiler artifact.
The hand-assembled reference module remains an explicit legacy test lane and
cannot satisfy compiled-provider acceptance. This closes the former codec
mismatch. The shipped generator template is unchanged for this: a dedicated
test, `compiler_provider_artifact.rs`'s
`generated_typescript_diagnostics_prove_the_compiled_providers_own_abi_hostility`,
instead temporarily wraps `WebAssembly.instantiate` to capture the real,
digest-verified `WebAssembly.Instance` the generated `Provider.open(wasm)`
itself produces (restored in `finally`), then drives the captured
`instance.exports.spx_pg_v1_*` and writes its `memory` directly to prove the
module's OWN closed ABI refuses a mutated canonical frame, an
over-capacity/out-of-bounds declared length (including 32-bit-wraparound
arguments), and lifecycle misuse — call after close (checked at both the
real module and, as a distinct, separately labeled case, the generated
wrapper's own guard), export before call, release of a foreign/stale handle,
release of an input already consumed by `call`, and double release — each at
the module's own exact status, proven (by a real `spx_pg_v1_call` dispatch
counter obtained from the same capture) never to reach a second physical
dispatch, with the session remaining healthy afterward. This is a real-ABI
hostile sample, not the full hostile/settlement matrix #229/#287's broader
acceptance still requires; that, hosted evidence, and endpoint shapes beyond
the admitted flat owned-`Bytes` profile remain separate.

## Private lifecycle admission

Zero is an absent-slot sentinel, never admitted as a live provider, input, or
result handle. Every handle-taking compiled-provider operation requires a nonzero
handle equal to its live slot; an absent or stale identity returns status 8
before checked invocation, copy-out, or release. This includes the initial
state and state after release/close. It does not change the export inventory.

The exact-source `Pair<Bytes>` lifecycle gate compares native C11 O0/O2 and
compiled Core-Wasm short-capacity export refusal (12), untouched destination,
non-consuming exact retry, stale result refusal, and close/reopen. Literal
`requires false` returns 11 without a result on both targets. Native consumes
that failed input, while Core-Wasm retains it until explicit value release;
the gate asserts each rule rather than declaring transfer parity. Native null
provider status 13 and Core-Wasm invalid-provider status 8 remain distinct.
Native allocation balance and Core-Wasm opaque-slot release are different
observations: no shared heap peak, physical leaf-release order, injected cleanup
failure, or all-phase settlement parity is inferred from this lifecycle cell.

Its owning selector is
`authenticated_handoff::same_subject::lifecycle::checked_identity_native_and_core_wasm_refuse_absent_handles_and_retry_export`
in `public_generic_native_adapter_v1`.

## Nonclaims

The implemented Phase A product is not:

- a public or supported generic ABI;
- a replacement for the C11 reference provider or in-process Wasm model;
- proof that every physical adapter or broader body shape invokes a selected
  Semaprax function;
- permission to publish npm, native, Web, component, or transport artifacts;
  or
- completion of PG-7, PG-8, PG-9, issue #162, or issue #229.

The predecessor native settlement harness still invokes its explicitly
labelled reversal fixture. Separately, the additive
`semaprax.authenticated-native-identity.v1` profile executes one compiler-
checked identity body after authenticating its canonical frame in the physical
C entry point. That narrow local profile is not a general native public ABI.

## Focused gates

The repository pins this phase with:

```sh
cargo test --locked -p semaprax --test project -- \
  public_generic_wasm_provider --test-threads=1
cargo test --locked -p semaprax --test public_generic_wasm_adapter_v1 -- \
  compiler_provider_artifact --test-threads=1
cargo test --locked -p semaprax --test public_generic_native_adapter_v1 -- \
  consumer_settlement:: --test-threads=1
cargo test --locked -p semaprax --test public_generic_native_adapter_v1 -- \
  authenticated_handoff:: --test-threads=1
```

The first gate covers manifest selection, checked admission, deterministic
artifact derivation, Project Lock binding, source-drift identity, shape/count
refusals, and fail-closed legacy output routes. The second validates the exact
zero-import export inventory and executes open/prepare/checked-call/export/
release/close under Node. The third proves the predecessor generated-caller
fixture lane. The fourth proves the additive authenticated identity lane,
which rejects malformed metadata, binding drift, stale generation, wrong
ownership, and settlement-plan drift before allocation or dispatch across C,
C++, and Rust at `-O0` and `-O2`.
