# RFC 0005: Compute Kernel Profile v1

- Status: Design-stage; the admission classifier is implemented and tested
  offline (`src/compute_profile/classifier.rs`), no parser/HIR/backend
  integration exists, and no accelerator hardware or driver evidence backs
  any claim in this document
- Version: 0.1
- Audience: compiler contributors evaluating an eventual data-parallel
  kernel/GPU target, and reviewers of the admitted grammar and refusal
  vocabulary this profile freezes

## Summary

This RFC specifies a small, deterministic, pure data-parallel kernel profile:
a closed admitted grammar (kernel-safe scalar/vector/buffer types, an
explicit device-effect vocabulary, affine host/device ownership transfer,
bounded workgroups and grids) and a closed refusal vocabulary
(`SPX-GC001`-`SPX-GC013`) that keeps the profile's determinism promise by
naming exactly which operations, reduction orderings, and numeric behaviors
it refuses, and why. It is a design deliverable, not an implementation: no
GPU is available on any host this repository builds on, and none is assumed.
The one thing this RFC makes executable today is the admission predicate
itself — [`src/compute_profile/classifier.rs`](../src/compute_profile/classifier.rs)
classifies constructed fixture kernels exactly the way
[`src/public_generic_abi/classifier.rs`](../src/public_generic_abi/classifier.rs)
classifies constructed fixture exports for [Public Generic Boundary Profile
v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) — so every refusal reason below is
independently observed by a real, offline test, without pretending any
kernel has ever compiled, dispatched, or run.

## Why "deterministic" is the load-bearing word

[Issue #210](https://github.com) asks for a "deterministic compute/GPU
profile," and GPU execution is exactly where determinism conventionally goes
to die: floating-point reassociation across a driver-chosen reduction tree,
scheduling-dependent atomic contention order, workgroup-count-dependent
result variance, and driver/toolchain behavior this repository's authors do
not control. [AGENTS.md](../AGENTS.md)'s own non-negotiable invariant is that
"graph JSON, Wasm bytes, diagnostics, semantic patches, and contracted
generated artifacts are deterministic." A compute profile that admitted
driver-dependent scheduling or reassociation and still called itself
"deterministic" would contradict that invariant on its first shipped kernel.
So this document is organized around one question first, before any grammar
or ABI detail: **what must this profile refuse in order to keep the
determinism promise, and what does a program get instead?** See [Determinism
policy](#determinism-policy).

## Scope and non-goals

In scope for v1, per the admission predicate below:

- kernel-safe scalar and fixed-width vector integer/boolean types;
- fixed-length, bounds-checked device buffers in one of three address spaces
  (global, workgroup-shared, per-invocation-private);
- pure, effect-free (beyond the closed device-effect vocabulary), synchronous
  kernel bodies;
- a bounded three-dimensional workgroup and dispatch grid;
- one fixed, sequential reduction order and one fixed, total-order atomic
  discipline — see [Determinism policy](#determinism-policy);
- explicit, affine device buffer/queue ownership, extending [RFC 0003's
  cleanup and resource
  ABI](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md#canonical-source) rather than
  opening a parallel resource model.

Out of scope for v1, matching the owning issue's own non-goals exactly:

- general arbitrary SEMAPRAX execution on a device — only the small admitted
  kernel subset below ever reaches a device;
- implicit device selection or implicit host/device memory copies — every
  device effect names an explicit capability, see [Ownership and
  effects](#ownership-and-effects);
- any claim of bit-exact floating-point equivalence where a real platform
  does not provide one — see [Numeric policy](#numeric-policy): v1 admits no
  floating-point kernel at all, precisely to avoid ever needing this claim
  before a dedicated tolerance profile exists;
- accelerator syntax landing before host ownership, arrays/collections,
  public ABI, and assurance foundations are ready — this RFC changes no
  parser, resolver, HIR, verifier, or backend; see [What is executable
  offline vs. what needs
  hardware](#what-is-executable-offline-vs-what-needs-hardware);
- provider-specific core-language keywords.

## Sequencing

The owning issue lists two prerequisites: completing the public-generic
hosted matrix (PG-9) and specifying a minimal SEMAPRAX semantic kernel. Its
own audit baseline calls this "a future differentiator, not an immediate
blocker" and asks explicitly for a design deliverable now. This RFC is that
design: it commits no code to the parser, resolver, HIR, verifier, native
backend, or Wasm backend, and nothing here widens what a `.spx` program can
express today. It exists so that when those two prerequisites land, kernel
work has one frozen admission predicate to implement against rather than
inventing one under implementation pressure — the same reason [Public
Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) was
written and frozen before its own classifier existed.

## Backend evaluation

The owning issue's implementation sequence asks for "an RFC comparing backend
choices against portability, driver/toolchain availability, safety,
deterministic testing, and ecosystem access" before any accelerator syntax.
No hardware is available to validate any of the below against a real driver;
this table is a desk evaluation to sequence future work, not a benchmarked
decision, and no backend is selected or implemented by this RFC.

| Backend | Portability | Driver/toolchain availability | Safety surface | Deterministic testing | Ecosystem access |
| --- | --- | --- | --- | --- | --- |
| WebGPU (via `wgpu` or a native WebGPU implementation) | Widest: one shader language (WGSL) targets Vulkan, Metal, D3D12, and browsers | Available on Linux/macOS/Windows without a vendor-specific SDK; a software (CPU) Vulkan implementation exists for CI without a GPU | Bounds-checked by the specification itself (out-of-bounds buffer access is defined, not undefined behavior) — closest match to this repository's existing safety posture | A software adapter (e.g., a CPU Vulkan implementation) can run this profile's admitted integer kernels in CI without real hardware, closest to what [Quality gates](QUALITY-GATES.md) already expects of every other backend | Newest, smallest ecosystem of the four; API and WGSL both still evolving |
| Vulkan compute (SPIR-V) | Wide across desktop/mobile; no first-class Apple-native path without an additional translation layer | Requires the Vulkan SDK and a conformant driver; a CPU/software implementation exists (used above for WebGPU) but the raw API is far larger than WebGPU's compute subset | No bounds-checking guarantee from the specification alone; would need `VK_EXT_robustness2` or equivalent explicitly enabled and verified, which is itself a portability constraint | Same CPU/software driver path as WebGPU, but through a much larger API surface this profile does not need | Mature, large ecosystem, but general-purpose graphics-first; SPIR-V tooling is reusable but heavier than this profile needs |
| CUDA | Single-vendor only | Requires the vendor's proprietary toolchain and driver; no vendor-neutral CI path and no software fallback | Bounds checking is the kernel author's responsibility unless explicitly added; historically the least safe of the four by default | No hardware-free conformance path at all | Largest existing ecosystem for data-parallel numerical kernels, entirely vendor-locked |
| OpenCL | Broad on paper, inconsistent in practice (patchy or deprecated vendor support, especially on macOS) | Available on more vendors than CUDA, but driver quality and continued support vary widely by platform | Comparable to Vulkan compute: no bounds-checking guarantee without vendor extensions | No universally available software/CPU conformance path as reliable as WebGPU's | Older, broad but unevenly maintained ecosystem |

**Reading, not a decision.** WebGPU's specification-level bounds-checking
guarantee and the availability of a software adapter for driver-free CI are
the two properties this RFC's own determinism and evidence requirements care
about most, and no other row matches both. That makes WebGPU the candidate
this document would recommend prototyping first once the sequencing
prerequisites above are met — but this RFC selects no backend, writes no
adapter, and links no library. Any future issue that begins real backend work
must record its own current toolchain and driver evidence rather than
inheriting this table's evaluation.

## Determinism policy

This is the profile's core refusal: the operations, orderings, and numeric
behaviors below are inadmissible in every kernel this profile accepts, stated
independently of any backend.

### Numeric policy: exact integers only in v1

v1 admits **only** the nine kernel-safe scalars in [Admitted kernel-safe
types](#admitted-kernel-safe-types) — eight fixed-width integers and `bool`
— and their fixed-width vectors. **No floating-point type or operation is
admitted in v1**, not as a parameter type and not as a body operation,
regardless of position. This is a deliberate simplification, not an
oversight: floating-point reassociation, rounding-mode variance, NaN
payload/propagation differences, and denormal-handling differences are
precisely the class of driver-dependent behavior this profile cannot yet
honestly bound. Per the owning issue's own instruction ("exact integer
profile first; tolerance/NaN/rounding facts explicit for floats"), a future
**Compute Kernel Profile v2** would define floating-point admission as an
explicit, separately versioned extension carrying its own tolerance, NaN,
and rounding facts; it does not exist yet, and this document does not widen
v1 to cover it. A kernel naming a floating-point parameter type is refused
with [`SPX-GC004`](#refusal-vocabulary); one performing a floating-point body
operation on an otherwise-integer signature is refused with
[`SPX-GC010`](#refusal-vocabulary) — kept as two distinct reasons because a
real front end may reach them at genuinely different compilation stages (type
checking vs. body lowering), the same way [Public Generic Boundary Profile
v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#refusal-vocabulary) keeps
"unsupported result shape" and "type outside the admitted grammar" distinct
reasons rather than collapsing them.

Integer arithmetic itself stays deterministic under this profile's existing
rules: wrapping and checked integer operations already have one fixed,
platform-independent result for a given input on every backend this
repository already supports, so admitting them here changes no existing
integer semantics — it only extends where they may execute.

### Reduction order

A reduction (sum, min, max, or similar fold over per-invocation partial
results) is admitted only when it names
[`ReductionOrder::SequentialLeftToRight`](../src/compute_profile/classifier.rs):
one fixed combination order, independent of workgroup count, invocation
scheduling, or driver-chosen tree shape, so the same logical input always
combines in the same order on every device and every run. **Refused
unconditionally:**

- a tree/pairwise reduction whose shape depends on how many invocations or
  workgroups the driver schedules (`ReductionOrder::TreeAssociative`) — the
  same logical reduction can combine operands in a different order, and
  under a hypothetical future floating-point extension would produce a
  different result, on a different device, workgroup count, or driver
  version, silently;
- a reduction with no named order at all, for example "whichever invocation
  finishes first wins" (`ReductionOrder::Unspecified`).

Refused by [`SPX-GC008`](#refusal-vocabulary).

### Atomics

An atomic read-modify-write is admitted only when it operates on an admitted
integer scalar **and** names
[`AtomicOrder::SequentiallyConsistentFixedOrder`](../src/compute_profile/classifier.rs):
a fixed, total order over every contending invocation for that exact memory
location, defined independently of scheduling. **Refused unconditionally:**

- any atomic naming `AtomicOrder::RelaxedDriverDefined` — "whatever order the
  driver's own relaxed/undefined scheduling produces" is not a total order
  this profile can name in advance, so it cannot certify the result is the
  same on every device;
- any atomic on a non-integer operand, since (once a floating-point
  extension exists) a floating-point atomic add's result additionally
  depends on the very reassociation this profile refuses elsewhere.

Refused by [`SPX-GC009`](#refusal-vocabulary).

### Bounds checking

Every indexed buffer access must be either statically proven in-bounds at
compile time, or guarded by an admitted runtime bounds check. Per the owning
issue's own failure case ("Bounds checks optimized away incorrectly become
memory-safety bugs"), a bounds check is never a target-lowering optimization
decision: an index this profile cannot prove in-bounds and that carries no
runtime check is refused before dispatch, not silently elided by a backend
under target-specific optimization. Refused by
[`SPX-GC007`](#refusal-vocabulary).

### Aliasing

No two buffer parameters reachable from one dispatch may claim overlapping
memory without an explicit disjointness proof. Per the owning issue's own
failure case ("Implicit buffer copies obscure ownership and cost"), this
profile draws the line at the same place SEMAPRAX's existing ownership model
already draws it for ordinary borrows: an alias the compiler cannot rule out
statically is refused, never silently permitted and never silently copied to
paper over it. Refused by [`SPX-GC011`](#refusal-vocabulary).

### Purity and the closed device-effect vocabulary

A kernel body is refused if it performs any operation outside plain integer
arithmetic, an explicit barrier, and the admitted indexed-access/reduction/
atomic constructs above — refused by [`SPX-GC012`](#refusal-vocabulary),
independent of whatever the kernel's *declared* effect list claims, so a
kernel cannot under-declare its effects and rely on the compiler not
checking the body. A kernel's *declared* effects, separately, must stay
inside the closed six-token [`DeviceEffect`](#device-effect-vocabulary)
vocabulary — refused by [`SPX-GC001`](#refusal-vocabulary) otherwise. Neither
check reuses or widens [Build Capability Manifest
v1](CAPABILITY-MANIFEST-V1.md)'s five-domain host capability vocabulary
(`filesystem`, `home`, `network`, `process`, `secrets`); a kernel body has no
access to any of those five, ever, and this profile does not add a sixth
domain to that document — it defines a disjoint, narrower vocabulary that
exists only inside a kernel dispatch.

### What honest device variance looks like instead

This profile does not claim a kernel produces bit-identical output on every
device merely because it is admitted: an admitted integer kernel's numeric
result is the same on every conformant backend precisely because integer
arithmetic has no reassociation or rounding freedom to begin with, not
because this profile claims parity it cannot back. Once dispatch reports
exist (see [Ownership and effects](#ownership-and-effects)), a report must
name its exact target and toolchain and never describe one target's result
as representative of another's — the same discipline
[AGENTS.md](../AGENTS.md#prohibited-shortcuts) already requires ("Do not
describe private, local, proof-only, simulator, or prior-head evidence as
public, hosted, physical-device, current-head, or production support").

## Ownership and effects

### Device-effect vocabulary

A kernel dispatch names zero or more of exactly six tokens — never an
open-ended or host-shared vocabulary:

| Token | Meaning |
| --- | --- |
| `DeviceAlloc` | Allocate a device-resident buffer. |
| `DeviceCopyIn` | Copy host-owned bytes into a device buffer. |
| `DeviceCopyOut` | Copy device buffer bytes back to host-owned storage. |
| `DeviceDispatch` | Launch the kernel over one grid. |
| `DeviceSynchronize` | Block until a prior dispatch completes. |
| `DeviceRelease` | Release a device-resident resource. |

A `DeviceDispatch` effect additionally requires an **explicit** device
capability naming which device/queue it targets; declaring the effect
without one is refused ([`SPX-GC002`](#refusal-vocabulary)) rather than
falling back to an ambient "the first available device," per the owning
issue's own explicit non-goal ("Implicit device selection"). Any effect
token outside this six-token vocabulary anywhere in a kernel's declared
effects is refused ([`SPX-GC001`](#refusal-vocabulary)).

### Affine device resources

A device buffer and a device queue are affine resources, in the same sense
[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md#canonical-source) already
gives every owned SEMAPRAX value: allocated once, transferred by move, and
released exactly once through the compiler's existing cleanup-plan
machinery — never a parallel resource model with its own reference-counting
or garbage-collection discipline. Concretely, extending RFC 0003 rather than
inventing a second one:

- `DeviceAlloc` produces a newly owned device-buffer handle, exactly the way
  an owned call's result is produced only after "postconditions and
  non-result cleanup" validation, per
  [AGENTS.md](../AGENTS.md#non-negotiable-invariants)'s existing invariant;
- a kernel dispatch **borrows** device buffers for the duration of one
  dispatch — see [`ParamMode`](../src/compute_profile/classifier.rs), which
  admits only `ReadOnlyView`/`ReadWriteView` inside a kernel signature
  ([`SPX-GC005`](#refusal-vocabulary) otherwise) — it never consumes an
  owned buffer into the kernel body itself, so no per-invocation cleanup
  obligation is needed at the granularity of one workgroup invocation;
- `DeviceRelease` is the one cleanup obligation that actually consumes the
  handle, exactly like any other owned resource's declared destructor;
- a released handle used again is a double-release, refused the same way
  RFC 0003 already refuses a double-release of any other owned resource —
  this profile adds no new double-release check, it reuses the existing one
  once a real device-buffer type exists in the resolver's cleanup inventory.

### What this profile does not implement yet

The classifier this RFC ships (`src/compute_profile/classifier.rs`) decides
only whether one candidate *kernel body/signature* is admitted — the
[Determinism policy](#determinism-policy) rules above. It does **not**
implement, and this RFC does not claim exists:

- a real device-buffer/queue resource type wired into the resolver's
  cleanup inventory, so double-release, device-absence, and stale-artifact
  rejection have no code behind them yet — they are specified here as the
  shape a future resource-tracker integration must take, reusing RFC 0003's
  existing machinery, not a new one;
- a dispatch/evidence report binding kernel source identity, target,
  device/profile, limits, and outputs — the shape [Ownership and
  effects](#ownership-and-effects) describes above is a target for that
  future report, not a schema frozen by this document;
- cancellation, device loss, or timeout handling;
- a CPU reference interpreter or any accelerator backend.

Building any of the above requires real front-end integration (parser,
resolver, HIR, cleanup-plan) that does not exist for a language feature not
yet admitted into the grammar, and — for the accelerator half — hardware
this host does not have. See [What is executable offline vs. what needs
hardware](#what-is-executable-offline-vs-what-needs-hardware).

## Admission predicate

An export is admitted under this profile only if every rule below holds,
checked in the precedence order
[`classify`](../src/compute_profile/classifier.rs) implements:

### Admitted kernel-safe types

- **Scalar:** `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `bool` —
  no floating-point scalar, see [Numeric policy](#numeric-policy-exact-integers-only-in-v1).
- **Vector:** a fixed-width vector of 2, 3, or 4 lanes of one admitted
  scalar.
- **Buffer:** a fixed-length buffer of one admitted scalar, in exactly one of
  three address spaces — `Global` (host-visible device memory, moved across
  the host/device boundary only through an explicit `DeviceCopyIn`/
  `DeviceCopyOut` effect), `Shared` (workgroup-local scratch, visible only
  within one workgroup for the duration of one dispatch), or `Private`
  (per-invocation private scratch).

Any other reachable type — a dynamically-sized array, a raw pointer, a
borrowed non-buffer aggregate, a resource, class, interface, closure, or
function value — is refused ([`SPX-GC004`](#refusal-vocabulary)).

### Kernel shape

- Between 1 and [`MAX_KERNEL_PARAMS`](#bounds) parameters
  ([`SPX-GC003`](#refusal-vocabulary) otherwise);
- every parameter's access mode is `ReadOnlyView` or `ReadWriteView`
  ([`SPX-GC005`](#refusal-vocabulary) otherwise) — an owned transfer or a
  shared alias into the kernel body itself is never admitted, see
  [Affine device resources](#affine-device-resources);
- a bounded three-dimensional workgroup size and grid size, each dimension
  at least 1 and at most its bound, and the workgroup's total invocation
  count (the product of its three dimensions) at most its own bound
  ([`SPX-GC006`](#refusal-vocabulary) otherwise);
- no buffer parameter may claim overlapping memory with another without an
  explicit disjointness proof ([`SPX-GC011`](#refusal-vocabulary)
  otherwise).

### Kernel body

Every operation in the body, checked in declaration order — the first
inadmissible operation refuses the whole kernel, later operations are never
reached:

- plain integer/boolean arithmetic and an explicit workgroup barrier are
  always admitted;
- an indexed load or store is admitted only when statically proven
  in-bounds or guarded by an admitted runtime check
  ([`SPX-GC007`](#refusal-vocabulary) otherwise);
- a floating-point operation is never admitted in v1
  ([`SPX-GC010`](#refusal-vocabulary));
- a reduction is admitted only with a fixed sequential order
  ([`SPX-GC008`](#refusal-vocabulary) otherwise);
- an atomic is admitted only on an integer operand with a fixed total order
  ([`SPX-GC009`](#refusal-vocabulary) otherwise);
- any operation reaching outside this closed set (a direct host effect) is
  never admitted ([`SPX-GC012`](#refusal-vocabulary)).

## Bounds

Every bound below is a new, first-profile choice: no existing hosted-green
specification in this repository governs a device buffer, workgroup, or
dispatch grid, so nothing here reuses a citation the way [Public Generic
Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#bounds) reuses
its own numbers from the type grammar. Every value is deliberately
conservative and is not a claim about any real device's actual limit — see
[Backend evaluation](#backend-evaluation), whose driver-quoted limits (where
this desk evaluation could find them) sit above every bound here.

| Bound | Value | Basis |
| --- | --- | --- |
| Max kernel-signature parameters | 8 | new: chosen small for a first profile; a wide parameter list can be reshaped into one aggregate, as [Public Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) already requires at its own owned-aggregate boundary |
| Max admitted workgroup dimension (per axis) | 1,024 | new: conservative relative to every evaluated backend's own quoted minimum guaranteed limit |
| Max admitted workgroup invocations (product of all three dimensions) | 1,024 | new: equal to the per-dimension bound, so a single-dimension workgroup may reach the full per-dimension bound while a three-dimensional one stays bounded overall |
| Max admitted grid dimension (per axis, in workgroups) | 65,535 | new: conservative relative to every evaluated backend's own quoted minimum guaranteed limit |
| Max admitted buffer elements | 16,777,216 (16 Mi) | new: a first, conservative bound, unrelated to any existing owned-payload bound in this repository since no earlier profile has ever admitted a device-resident buffer |

## IN / DEFERRED / EXCLUDED shape table

| Shape | Status | Why |
| --- | --- | --- |
| Fixed-width integer/boolean scalar or vector (2-4 lanes) | **IN v1** | admitted grammar; core kernel-safe vocabulary |
| Fixed-length, bounds-checked buffer in an admitted address space | **IN v1** | admitted grammar |
| Sequential, fixed-order reduction over an admitted scalar | **IN v1** | the only reduction order this profile can certify is deterministic across devices |
| Integer atomic with a fixed total order | **IN v1** | the only atomic discipline this profile can certify is deterministic across devices |
| Explicit workgroup barrier | **IN v1** | the deterministic synchronization primitive this profile requires in place of implicit cross-invocation ordering |
| Bounded three-dimensional workgroup/grid dispatch | **IN v1** | admitted shape, subject to [Bounds](#bounds) |
| Affine device buffer/queue allocation, copy, dispatch, release | **IN v1 (specified; not yet implemented)** | extends RFC 0003 rather than opening a new resource model; no resolver/cleanup-inventory integration exists yet — see [What this profile does not implement yet](#what-this-profile-does-not-implement-yet) |
| Floating-point scalar, vector, or operation, any position | **EXCLUDED from v1** | see [Numeric policy](#numeric-policy-exact-integers-only-in-v1); reserved for a separate, explicitly versioned tolerance/NaN/rounding extension |
| Tree/associative or unspecified-order reduction | **EXCLUDED** | see [Reduction order](#reduction-order) — the profile's core determinism refusal |
| Relaxed/driver-defined-order atomic, or a non-integer atomic | **EXCLUDED** | see [Atomics](#atomics) |
| An indexed access neither statically proven nor runtime-checked | **EXCLUDED** | see [Bounds checking](#bounds-checking) |
| Overlapping buffer aliasing without a disjointness proof | **EXCLUDED** | see [Aliasing](#aliasing) |
| A kernel body effect outside plain arithmetic/barrier/indexed-access/reduction/atomic | **EXCLUDED** | see [Purity and the closed device-effect vocabulary](#purity-and-the-closed-device-effect-vocabulary) |
| Implicit device selection (a dispatch effect with no explicit device capability) | **EXCLUDED** | explicit epic-wide non-goal; see [Device-effect vocabulary](#device-effect-vocabulary) |
| An owned (consuming) or shared-alias kernel parameter mode | **EXCLUDED** | see [Affine device resources](#affine-device-resources) |
| General arbitrary SEMAPRAX execution on a device | **EXCLUDED** | explicit epic-wide non-goal; only the admitted kernel subset above ever reaches a device |
| A CPU reference interpreter or accelerator backend adapter | **DEFERRED** | requires real front-end integration and, for the accelerator half, hardware this host does not have; see [What is executable offline vs. what needs hardware](#what-is-executable-offline-vs-what-needs-hardware) |
| Bit-exact floating-point parity claim across devices | **EXCLUDED, permanently, not merely for v1** | no floating-point kernel is admitted at all in v1, and no future version of this profile may claim bit-exact parity where a real platform does not provide one, per the owning issue's own explicit non-goal |

## Refusal vocabulary

[`classify`](../src/compute_profile/classifier.rs) returns one of the closed
reasons below, never a partial admission. Namespace `SPX-GC` ("GPU/data-parallel
Compute") is unused elsewhere in this repository as of commit `d45db653`
(2026-09-11; checked with `rg -n 'SPX-GC[0-9]' src docs`).

| Code | Reason | Independently observed by a classifier test? |
| --- | --- | --- |
| `SPX-GC001` | declared effect outside the closed device-effect vocabulary | yes |
| `SPX-GC002` | dispatch effect declared without an explicit device capability (implicit device selection) | yes |
| `SPX-GC003` | parameter count is zero or exceeds the bound | yes (both zero and over-bound; also the exact-bound positive case) |
| `SPX-GC004` | parameter type outside the kernel-safe scalar/vector/buffer vocabulary (including any floating-point scalar, and an unadmitted vector lane count) | yes |
| `SPX-GC005` | parameter access mode is not an admitted read-only/read-write buffer view | yes (both an owned-transfer mode and a shared-alias mode) |
| `SPX-GC006` | a workgroup or grid dimension is zero or exceeds its bound, including the invocation-product bound | yes (zero and over-bound for each of workgroup dimension, workgroup invocation product, and grid dimension; also the exact-bound positive case) |
| `SPX-GC007` | an indexed buffer access is neither statically proven in-bounds nor runtime-checked | yes (load and store) |
| `SPX-GC008` | a reduction combines its operands in a non-deterministic order | yes (tree-associative and unspecified) |
| `SPX-GC009` | an atomic has no admitted deterministic total order, or operates on a non-integer operand | yes (both cases) |
| `SPX-GC010` | the kernel body performs a floating-point operation, not admitted by the v1 exact-integer profile | yes |
| `SPX-GC011` | two or more buffer parameters may alias beyond the profile's checked disjointness rule | yes |
| `SPX-GC012` | the kernel body performs a host effect directly, independent of its declared effect list | yes |
| `SPX-GC013` | a buffer parameter's element count exceeds the capacity bound | yes (also the exact-bound positive case) |

Diagnostic precedence (which reason wins when several apply): effect-
vocabulary closure, explicit device-capability presence, parameter-count
bound, parameter-type admission, buffer-capacity bound, parameter-ownership-
mode admission, grid/workgroup-shape bounds, buffer aliasing, then each
kernel-body operation in declaration order. This exact order is pinned by
construction in `classify`'s control flow, not independently re-derived; see
[`src/compute_profile/classifier.rs`](../src/compute_profile/classifier.rs).

## What is executable offline vs. what needs hardware

**Executable offline, today, and covered by real tests** (`cargo test
--locked -p semaprax --lib compute_profile`, 31 tests):

- the admission predicate above, over constructed fixture kernels — every
  refusal code has at least one dedicated negative test asserting the exact
  code, and the bound-exactness cases (`SPX-GC003`, `SPX-GC006`, `SPX-GC013`)
  additionally have a positive at-the-bound test;
- each negative test mutates exactly one field of a fixture
  ([`admitted_baseline`](../src/compute_profile/classifier/tests.rs)) a
  dedicated test ([`baseline_is_fully_admitted`](../src/compute_profile/classifier/tests.rs))
  proves is fully admitted on its own, so every earlier-precedence check a
  given test does not target is independently known to already pass —
  see the test module's own doc comment for the isolation argument in full;
- the diagnostic-code shape check (every code is `SPX-GC` plus exactly three
  ASCII digits, matching [Installed Diagnostics
  v1](INSTALLED-DIAGNOSTICS-V1.md)'s static-scan rule) and one rendered-
  diagnostic check.

**Needs hardware, a real front end, or both — not built, not claimed, and not
simulated as if it were evidence:**

- parsing, resolving, or checking any real kernel source — no kernel syntax
  exists in this compiler;
- a device-buffer/queue resource type integrated into the resolver's cleanup
  inventory, so double-release/device-absence/stale-artifact rejection have
  no executable check yet;
- a CPU reference interpreter executing an admitted kernel's semantics;
- any accelerator backend (WebGPU or otherwise) actually compiling or
  dispatching a kernel;
- any driver, toolchain, or device-capability probing;
- any conformance, parity, cancellation, or device-loss evidence — none of
  this RFC's claims describe a simulated result as execution evidence, per
  the assignment's own instruction, and none will until real hardware and a
  real front end exist.

## Freeze and change procedure

This is version 0.1, not yet frozen: it precedes the front-end integration
[What this profile does not implement yet](#what-this-profile-does-not-implement-yet)
names, and the two dependency issues in [Sequencing](#sequencing) remain
open. A later revision that narrows or widens the admitted grammar, the
refusal vocabulary, or any bound must:

- cite the exact rule or bound it changes and why, the same way this
  document cites [Public Generic Boundary Profile
  v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s own freeze procedure;
- never describe a bound as reused from real hardware evidence unless a
  dated, attributed measurement backs it — every bound in this version is
  explicitly a first-profile choice, not a measurement;
- add or update the corresponding [`src/compute_profile/classifier.rs`](../src/compute_profile/classifier.rs)
  test before or with the change, per this repository's own change
  protocol.
