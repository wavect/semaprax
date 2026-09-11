# Public Generic Carrier v1

Audience: backend provider authors on native and Core Wasm, and reviewers of the ownership and settlement contract.

Status: frozen logical specification with a reference codec and local
evidence (`src/public_generic_abi/carrier.rs`, and its `machine` and `trace`
submodules). This is the carrier half of gate #150-#153 of the [Public
Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md) and
answers issue #171 and issue #153. It defines the ownership state machine,
the phase ledger, a `CarrierBindingV1` wire binding, the [call-machine
orchestration](#the-call-machine) that drives both together atomically, and
the [normalized trace vocabulary](#the-normalized-trace) — all as pure,
locally-tested logic. It defines **no physical target mapping** — no C
struct layout, no Wasm handle table implementation, no Rust FFI boundary —
and executes nothing itself: there is no provider, no allocator, and no real
target to allocate, transfer, or release against, in this LOGICAL section.
[Native C11 physical adapter (issue #154)](#native-c11-physical-adapter-issue-154)
below is the first PHYSICAL adapter built on top of it, with real allocation,
release, and normalized-trace emission — locally evidenced only, against a
fixture endpoint, per that section's own scope note. The remaining per-target
adapters and generated consumers (issues #155-#159, #162) are still
outstanding. Public generic ownership remains unsupported and unpublished.

Audience: ownership, cleanup, backend, ABI, and generated-consumer
maintainers.

## Scope

A carrier is what a foreign caller and the SEMAPRAX side agree happened to
one [Public Generic Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md)-named
value as it crosses the boundary: allocated, filled, transferred, read,
consumed, released — or abandoned at any point with a sticky, exactly-once
failure outcome. This document is deliberately **logical**: it specifies
states, legal transitions, and binding identity that every target must
implement identically. It is not itself a physical layout.

| Layer | Identifier |
| --- | --- |
| Carrier schema | `semaprax.public-generic-carrier.v1` |
| Binding digest domain | `semaprax.public-generic-carrier.v1.binding\0` |
| Depends on | [Public Generic Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md) |
| Depends on | [Public Generic Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md) |
| Reserved/allocated diagnostic range | `SPX-PG8xx` |

`rg -n "public-generic-carrier" docs src tests` at freeze time (commit
`d45db653`) found no colliding schema; the existing v8-v11 carriers are
narrower, profile-specific, and unchanged by this document.

## LOGICAL versus PHYSICAL

Every fact in this document is one of exactly two kinds, and mixing them is
the single most common way a public carrier design goes wrong:

- **LOGICAL** (this document, shared across every target): the state
  machine's states and legal transitions; the phase ledger; the trace
  vocabulary; which side owns a value at which phase; the sticky-failure
  rule; the release-order rule; capacity bounds; binding identity.
- **PHYSICAL** (a later, per-target document — none exists yet): how a
  handle is represented in memory on native C11 (an opaque pointer-sized
  integer inside a generated wrapper struct); how it is represented in Core
  Wasm (a canonical numeric handle or a component-model resource); how a
  generated Rust caller represents ownership (an owning newtype around the
  same logical handle); alignment, endianness, and symbol names.

A target-neutral state machine is not a shared physical layout, and this
document does not become one by being detailed. [PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)'s
architecture diagram places "Native C11 carrier adapter" and "Core Wasm
carrier adapter" as siblings below this logical layer for exactly this
reason: both must implement the same states and transitions, and may differ
only in how a byte pattern spells "the same logical fact."

## Handles

An owned value crossing the boundary is addressed by an **opaque,
generation-scoped handle**, never a raw pointer or offset:

```text
Handle { id: u32, generation: u32 }
```

- `id` identifies a position within one carrier instance: `0` is always the
  root aggregate handle (the whole owned parameter or result); `1..=256` are
  the up-to-256 owned-leaf handles, in the same structural order the
  [Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md) plan already
  fixes — this carrier does not invent a different leaf order.
- `generation` identifies one carrier instance's lifetime. A handle presented
  against the wrong generation is `SPX-PG805`, never silently accepted —
  this is what makes a stale or reused handle from an earlier call fail
  closed on both native (where a freed struct could otherwise be replayed)
  and Wasm (where small integer handles are especially prone to reuse
  collisions).

Maximum live handles per carrier instance is **257** (256 owned leaves plus
the one root handle), reused from [Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#bounds).

## The logical value state machine

Every handle (root or leaf) is in exactly one of these states:

```text
Created -> Initialized -> Transferred -> Consumed -> Released
              |               |  ^
              |               |  | (loan ends)
              |               v  |
              |            Borrowed
              |
              +-------------------> Released   (failure before transfer)
                              \
                               `-> Released     (failure after transfer,
                                                  consumer refusal)

any state -- illegal transition --> Invalid   (terminal, absorbing)
```

| From | Event | To | Who drives it |
| --- | --- | --- | --- |
| — | allocate | `Created` | provider |
| `Created` | fill storage | `Initialized` | provider |
| `Initialized` | commit (see [phase ledger](#the-call-phase-ledger)) | `Transferred` | provider, at the call's one commit point |
| `Transferred` | begin a read-only loan | `Borrowed` | consumer |
| `Borrowed` | loan ends | `Transferred` | consumer |
| `Transferred` | final owning read / copy-out | `Consumed` | consumer |
| `Consumed` | discharge | `Released` | whichever side is accountable per the settlement plan |
| `Created` or `Initialized` | provider-side failure (allocation failure, copy-in failure) | `Released` | provider — released without ever transferring |
| `Transferred` | consumer-side failure (contract failure, consumer refusal, copy-out failure) | `Released` | consumer — released without a `Consumed` step, because refusing a value is not consuming it |
| any state | an operation not listed above for that state (double transfer, transfer before `Initialized`, consume before `Transferred`, release before `Consumed` on the success path, any use after `Released`, a wrong-generation handle, a handle naming another carrier's descriptor) | `Invalid` | the side that attempted the illegal operation; `Invalid` has no outgoing transition |

`Transferred -> Transferred` (a second transfer of the same handle) is not in
the table and is therefore illegal: ownership transfers exactly once per
handle, matching the repository's "an owned call stages arguments left to
right and transfers them together at its declared commit boundary" and "no
implicit clone" invariants. `Released -> anything` is likewise illegal: a
released handle is inert.

## The call phase ledger

The state machine above governs one handle. The **phase ledger** governs one
whole call and gates when the commit transition (`Initialized -> Transferred`)
is legal for every handle in the carrier at once:

```text
Preparing -> Validated -> Committed -> Settled(Success)
                                     -> Settled(Failure(reason))
```

- **Preparing.** Every argument handle exists and is being filled. Nothing is
  owned by the far side yet. Non-committing: the call may still be abandoned
  with no owned handle ever reaching `Transferred`.
- **Validated.** Every argument has been checked against capacity and shape
  (leaf count, byte bounds, generation) and found admissible. Still
  non-committing.
- **Committed.** The **exact ownership commit point**: every argument
  handle's `Initialized -> Transferred` transition happens here, together,
  left to right in structural (leaf) order, never individually before this
  phase and never partially — a call that reaches `Committed` has
  transferred every argument handle or none of them.
- **Settled(Success).** The result's own handles complete `Initialized ->
  Transferred` and the result is published to the consumer, only after every
  non-result obligation (cleanup of any transient provider-side state) is
  discharged — the milestone's "result publication follows postconditions
  and non-result cleanup" invariant, restated for this boundary.
- **Settled(Failure(reason))**. Reached from `Preparing`, `Validated`,
  `Committed`, or result preparation. Once set, the outcome is **sticky**:
  no later event may replace a selected `Settled` state with a different
  one, in either direction (failure cannot become success, and one failure
  reason cannot become another). An attempt to do so is `SPX-PG806`.

## Failure settlement and release order

Release order after a failed transfer is the **exact reverse of the
canonical obligation order** the settlement plan already fixes — never
renumbered, sorted, or repaired by this carrier or by any consumer. A
`ReleasePlan` derived from a settlement plan's obligations enforces this:
`verify_release_order` rejects (`SPX-PG804`) a submitted release sequence
that is not exactly the reverse of the obligation order, even when it
releases the same set of handles.

Two failure shapes are distinguished, matching [Public Generic Settlement
Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md#failure-settlement):

- **Failure before commit.** No handle reached `Transferred`; every allocated
  handle is `Released` directly from `Created` or `Initialized`, provider
  side, in the reverse of allocation order.
- **Failure after commit, before consume.** Every argument handle already
  reached `Transferred`; the consumer releases them from `Transferred`
  directly (no `Consumed` step for a refused value), in the reverse of the
  structural obligation order.

A partial transfer that exposes some handles as `Transferred` and others as
still `Initialized` after a failure is impossible by construction: the
`Committed` phase transitions every argument handle together, so there is no
observable state between "none transferred" and "all transferred" for one
call's arguments. This is the direct answer to issue #171's stated failure
case: "transferring ownership before all validation completes can leak or
double-free on later failure" — validation and capacity reservation happen
in `Preparing`/`Validated`, strictly before the one `Committed` transition.

## The call machine

[`carrier::machine::CarrierCallMachine`](../src/public_generic_abi/carrier/machine.rs)
is the executable orchestration that ties the state machine above and the
phase ledger together for one whole call, gated exactly the way [the call
phase ledger](#the-call-phase-ledger) requires:

- [`CarrierCallMachine::validate`] advances `Preparing -> Validated`.
- [`CarrierCallMachine::prepare_input`] fills every input handle (`Created ->
  Initialized`), root then leaves.
- [`CarrierCallMachine::commit_input_transfer`] is **the** atomic commit
  point: it checks — without mutating anything — that every input handle is
  `Initialized`, and only if that check passes does it advance the phase to
  `Committed` and apply `Initialized -> Transferred` to every handle. A
  failure discovered mid-transfer (one handle not yet `Initialized` while its
  siblings already are, simulating an injected failure between one leaf's
  allocation and the next) is refused before any handle is mutated: there is
  no way to observe some handles `Transferred` and others not. Calling it
  twice is refused the same way — the second call's readiness check sees
  every handle already `Transferred`, which is not a legal source state for
  `Commit`.
- [`CarrierCallMachine::begin_execution`] / `finish_execution` bracket the
  checked function's execution, legal only once, only after input commits.
- [`CarrierCallMachine::begin_result`] / `prepare_result` /
  [`commit_result`] mirror input staging and commit for the result
  direction, only after execution finishes; the consumer-visible
  [`CarrierCallMachine::result`] accessor returns handles still `Initialized`
  (never `Transferred`) until `commit_result` runs, which is the executable
  form of "the consumer sees no result leaf before whole-result commit".
- [`CarrierCallMachine::settle`] selects the terminal [`Settlement`], sticky
  exactly as [above](#the-call-phase-ledger): a later, different outcome
  (including a cleanup failure arriving after an earlier semantic failure) is
  rejected with `SPX-PG806`; when no earlier failure exists, a cleanup
  failure may legally become the terminal status.
- [`CarrierCallMachine::release_input_before_transfer`],
  `release_input_after_transfer`, and `release_result_before_commit` release
  a [`HandleSet`] in the exact reverse of its obligation order (root last),
  matching [failure settlement and release order](#failure-settlement-and-release-order)
  above; releasing an already-released set a second time is refused, because
  the second handle in reverse order is no longer in a state `Release*` can
  legally leave.

This machine binds no [descriptor](PUBLIC-GENERIC-DESCRIPTOR-V1.md), parses
no carrier bytes, and allocates nothing physical — see [LOGICAL versus
PHYSICAL](#logical-versus-physical). Requiring a
`VerifiedPublicGenericDescriptor` and wiring `LogicalCarrierFrame::parse_bounded`
/ `LogicalCarrierPlan` from a real descriptor and a real physical adapter
remains #154/#155's follow-on work; this machine is the shared LOGICAL
orchestration both of them drive identically.

## The normalized trace

[`carrier::trace::Trace`](../src/public_generic_abi/carrier/trace.rs) records
an append-only, ordinal-numbered sequence of
[`TraceEvent`](../src/public_generic_abi/carrier/trace.rs)s as
[`CarrierCallMachine`](../src/public_generic_abi/carrier/machine.rs) runs.
Every event carries only deterministic identity/lifecycle fields — an
ordinal, a [`TraceLabel`], a [`Direction`] (`Input` or `Result`), an optional
structural leaf index, an optional state-before/state-after pair, and an
optional [`Settlement`] — never a payload byte, a host pointer, a native
offset, a Wasm address, a random nonce, a wall-clock time, or a process
identifier. The vocabulary is closed and matches issue #153's recommended
event names exactly:

```text
FrameValidated
LeafAllocationStarted
LeafAllocationCommitted
LeafPayloadCopied
InputValuePrepared
InputTransferCommitted
ExecutionStarted
ExecutionFinished
ResultLeafAllocationStarted
ResultLeafAllocationCommitted
ResultValuePrepared
ResultCommit
LeafRelease
CarrierRelease
TerminalStatus
```

Allocation/copy events (`LeafAllocationStarted`, `LeafAllocationCommitted`,
`LeafPayloadCopied`, and their `Result*` counterparts) are per-handle, one
triple per root or leaf. The two commit markers
(`InputTransferCommitted`, `ResultCommit`) and the two preparation markers
(`InputValuePrepared`, `ResultValuePrepared`) are whole-value events with no
leaf index, matching the commit rule above: the call commits one marker for
every handle together, not one marker per handle. `TerminalStatus` carries
the selected [`Settlement`], recorded once per [`CarrierCallMachine::settle`]
call that actually changes the sticky outcome.

The same normalized trace vocabulary is the intended comparison point for a
later cross-engine equivalence test across the interpreter, native C11, and
Core Wasm adapters; this document and its reference implementation define the
vocabulary and a local, in-memory recorder only; no adapter emits it yet.

## Target mappings (logical only)

This document commits every target to the same states and transitions; it
does not commit to physical representation. The columns below are naming
guidance for the next tranche (issues #154-#159), not a specification of
layout:

| Target | Logical mapping |
| --- | --- |
| Reference interpreter | `Handle` maps directly onto the interpreter's existing owned-value identity; no physical adapter needed |
| Native C11 | opaque handle inside a generated wrapper struct; C consumers may not copy the wrapper (copying an owning wrapper without generation tracking is exactly the double-release risk #171 names) |
| Native C++17 | a noncopyable, move-only RAII type wrapping the same opaque handle |
| Rust | an owning newtype around the same logical handle, `Drop`-checked |
| Core Wasm | a canonical numeric handle, or a component-model resource where the runtime provides one; integer-handle reuse across generations is exactly why `generation` is part of the handle, not an afterthought |

## Native C11 physical adapter (issue #154)

Audience: native provider/adapter implementers and reviewers of the physical
allocation and release path.

Status: local, proof-only reference implementation
(`src/public_generic_abi/native/`), unsupported and unpublished. This is the
first PHYSICAL adapter built on the LOGICAL layer above: it decides no
legality the [state machine](#the-logical-value-state-machine), [phase
ledger](#the-call-phase-ledger), or [`CarrierCallMachine`](#the-call-machine)
do not already fix, and it emits exactly [the normalized trace
vocabulary](#the-normalized-trace) above, adding no second vocabulary.
Answers issue #154.

**Deferred scope.** Deriving a provider from a real checked *generic* export
requires #119's still-blocked owned-record ownership evidence. Until that
lands, the bound endpoint is a fixture (`spx_pg_endpoint_reverse_bytes_v1`,
byte-reversal per owned leaf) operating on the existing owned-Bytes shapes —
a flat sequence of independent owned `Bytes` leaves, matching this issue's
brief. `NativeProviderBindingV1`'s trusted descriptor bytes are likewise a
hand-constructed fixture rather than bytes produced by
[`descriptor::verify`](PUBLIC-GENERIC-DESCRIPTOR-V1.md) against an admitted
public generic export, since no such export exists yet. The replay behavior
under test — byte-exact rejection of a wrong, tampered, or cross-paired
descriptor/binding — is identical regardless of which trusted bytes a real
provider is generated from; only the *source* of those bytes is deferred.
Cross-platform hosted execution (Linux, macOS, Windows/MSVC) is not claimed;
local evidence exists only for the host this round ran on (see the
accompanying worktree report). Sanitizer coverage is local
(`-fsanitize=address,undefined` under Clang) and not yet a hosted CI gate;
that is issue #163's own recording step, per [Platform
requirements](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).

### Provider binding

[`native::binding::NativeProviderBindingV1`](../src/public_generic_abi/native/binding.rs)
is a new, physical-layer-only artifact layered on top of — never modifying —
[`CarrierBindingV1`](#compatibility-and-lifecycle) above. It wraps a
`CarrierBindingV1` naming `TargetProfile::NativeC11` unchanged, and adds
exactly the facts a physical native provider needs and the logical carrier
never should: `native_adapter_abi_version` (closed to `"v1"` this round),
`provider_artifact_digest`, `exported_endpoint_symbol`, a
`compiler_backend_version` fact, and a closed `SupportPublicationState`
(`unsupported-unpublished` is the only admitted value; decoding any other
claim is rejected, so a generated binding can never claim otherwise). Its
wire format, digest domain, decode/replay split, and hostile-input handling
follow `CarrierBindingV1`'s own convention exactly (framed fields, a
domain-separated digest never transmitted, and `replay` requiring byte-exact
preimage equality).

### Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-PG901` | malformed native provider binding bytes (framing, unknown ABI version, unrecognized support/publication claim, or an embedded carrier binding not naming `TargetProfile::NativeC11`) |
| `SPX-PG902` | independent replay found the recomputed native binding preimage does not equal the submitted one |

`SPX-PG9xx` was free at freeze time (`rg -n "SPX-PG9" docs src tests` found no
prior use); this document allocates exactly `SPX-PG901`-`SPX-PG902`.

### The native ABI surface

[`src/public_generic_abi/native/spx_pg_v1.h`](../src/public_generic_abi/native/spx_pg_v1.h)
is the versioned C11 header: `spx_pg_provider_open_v1`,
`spx_pg_input_prepare_v1`, `spx_pg_call_v1`, `spx_pg_result_export_v1`,
`spx_pg_value_release_v1`, `spx_pg_result_release_v1`, and
`spx_pg_provider_close_v1`, plus a closed `spx_pg_status_v1` (`int32_t`)
vocabulary. `spx_pg_provider_v1`, `spx_pg_value_v1`, and `spx_pg_result_v1`
are forward-declared only — incomplete in the header, defined only in
[`provider_body.c`](../src/public_generic_abi/native/provider_body.c) — so no
internal aggregate layout is public and no foreign caller can copy an owning
wrapper. Every byte slice is `(pointer, length)`; every status maps to
exactly one closed constant:

| `spx_pg_status_v1` | Restates | Meaning |
| --- | --- | --- |
| `SPX_PG_STATUS_OK` (0) | — | success |
| `SPX_PG_STATUS_MALFORMED_DESCRIPTOR` (1) | `SPX-PG701` | malformed descriptor bytes |
| `SPX_PG_STATUS_DESCRIPTOR_REPLAY_MISMATCH` (2) | `SPX-PG703` | descriptor bytes do not replay against the trusted value |
| `SPX_PG_STATUS_MALFORMED_BINDING` (3) | `SPX-PG901` | malformed native provider binding bytes |
| `SPX_PG_STATUS_BINDING_REPLAY_MISMATCH` (4) | `SPX-PG902` | binding bytes do not replay, or name a different descriptor/target/provider artifact/endpoint |
| `SPX_PG_STATUS_MALFORMED_CARRIER` (5) | `SPX-PG801` | malformed input/result carrier bytes |
| `SPX_PG_STATUS_CARRIER_CAPACITY` (6) | `SPX-PG802` | a carrier bound was reached (leaf count or byte total) |
| `SPX_PG_STATUS_ILLEGAL_TRANSITION` (7) | `SPX-PG804` | an illegal handle-lifecycle operation |
| `SPX_PG_STATUS_HANDLE_INVALID` (8) | `SPX-PG805` | the handle is not live in this provider's registry: foreign, stale, already consumed/released, or forged |
| `SPX_PG_STATUS_STICKY_SETTLEMENT_VIOLATION` (9) | `SPX-PG806` | reserved; the adapter itself never issues a second, different outcome |
| `SPX_PG_STATUS_ALLOCATION_FAILURE` (10) | new, physical-only | the bounded allocator could not satisfy a leaf allocation |
| `SPX_PG_STATUS_CONTRACT_FAILURE` (11) | new, physical-only | the checked endpoint itself refused, failed, or a cleanup failure became terminal |
| `SPX_PG_STATUS_BUFFER_TOO_SMALL` (12) | protocol, not a failure | `out_capacity` was smaller than `*out_required`; nothing was written or consumed |
| `SPX_PG_STATUS_NULL_OR_WRONG_KIND` (13) | new, physical-only | a required pointer was null, or a handle argument was the wrong kind, independent of lifecycle state |

The two ALLOCATION_FAILURE/CONTRACT_FAILURE codes are new because they name
facts the LOGICAL layer above has no vocabulary for (a real allocator running
out of bounded capacity; a real endpoint's own refusal) — they never
duplicate an existing SPX-PG reason.

### Handle safety

Every opaque handle is a pointer minted by this adapter and tracked in one
process-wide registry entry `{pointer, kind, owner, generation}` (native
C11's physical spelling of the logical `Handle{id, generation}`: pointer
identity is `id`, a per-provider monotonic counter is `generation`). Handle
validation always scans the registry for the exact pointer **value** first
and dereferences the pointee only once a live, correctly-kinded entry is
found — the repository's existing FFI-handle safety pattern, reused rather
than reinvented, so a forged or foreign pointer is rejected without ever
being dereferenced. A released or transferred entry is zeroed immediately, so
a later allocation reusing the same address is never mistaken for the old
handle. This is why `spx_pg_result_export_v1`/`spx_pg_value_release_v1`/
`spx_pg_result_release_v1` need no separate provider argument even though the
registry backs every provider: cross-provider misuse is caught because a
handle minted by one provider is tagged with that provider as `owner` and
`spx_pg_call_v1`/`spx_pg_provider_close_v1` check it explicitly.

### Allocation, release, and sticky failure

[`provider_body.c`](../src/public_generic_abi/native/provider_body.c) routes
every heap byte — provider/value/result structs, leaf pointer/length arrays,
and leaf payload bytes alike — through one bounded allocator
(`spx_pg_alloc`/`spx_pg_dealloc`), so
`spx_pg_test_live_allocations_v1`/`spx_pg_test_live_handles_v1` are exact
counts, not samples; the fixture harness additionally intercepts raw
`malloc`/`free` independently (mirroring the repository's existing
allocation-observation fixtures) as a second, external proof. A test-only
`spx_pg_test_inject_failure_v1(ordinal)` arms deterministic failure at any of
the 15 [normalized trace](#the-normalized-trace) ordinals for the next
call, rolling back every allocation already made — physical evidence that
"failure at every logical injection point" leaves zero live allocations and
handles. Sticky failure ([above](#the-call-phase-ledger)) is restated, not
reimplemented: a private `spx_pg_settle` keeps the first selected outcome and
counts (via `spx_pg_test_settlement_overwrite_attempts_v1`) any later,
different attempt without applying it — exercised directly by
`spx_pg_test_force_settlement_conflict_v1`, and naturally by the two release
ordinals (`SPX_PG_TRACE_LEAF_RELEASE`, `SPX_PG_TRACE_CARRIER_RELEASE`), where
injection never skips the physical free but settles a cleanup failure as a
side effect — legally becoming the terminal status when nothing failed
earlier, and safely discarded when something already did. A cleanup failure
discovered after every other step of a call already succeeded (during the
non-result input release the success path performs before result
publication) is caught before `*out_result` is ever set: `spx_pg_call_v1`
settles first and only publishes the result handle if the STICKY outcome is
actually `SPX_PG_STATUS_OK`, so a failed call never hands back a live result.

### C-hosted fixture and evidence

`tests/public_generic_native_adapter_v1` (harness; `fixture.rs` drives it,
`probe.c` is the C-hosted test body) renders the reference provider via
[`native::template::render_reference_provider`](../src/public_generic_abi/native/template.rs)
and compiles it together with `probe.c` as one pure-C translation unit —
reusing the repository's existing native-fixture pattern
(`tests/support/native_fixture_stdio.c`, a local allocation-counting shim
matching `tests/native_owned_utf8_settlement_v1/allocations.c`'s
`#define malloc/free` convention) rather than building a second FFI stack.
It covers: provider-open replay (correct, wrong descriptor, wrong binding,
null out-provider); a full success round trip with a two-pass, byte-identical
repeated export; the exact and first-over-bound leaf-count and leaf-byte-size
cases; malformed/truncated carrier bytes; handle hostility (null, foreign,
cross-provider, transferred-and-reused, wrong-kind, double release, close
with a live handle); sticky failure and the cleanup-becomes-terminal case;
the exact trace-label sequence on success; and the full 0-13 failure-injection
matrix, asserting zero live allocations and handles after every terminal
case. `header_compiles_standalone_as_c11` compiles `spx_pg_v1.h` alone.
`native_public_generic_adapter_settles_at_o0_and_o2` runs unconditionally;
`provisioned_native_public_generic_adapter_asan_ubsan` is `#[ignore]`d,
requiring `SEMAPRAX_STRING_SANITIZER_CLANG`, matching the repository's
existing sanitizer-gating convention.

### Nonclaims (native adapter)

This adapter is local, proof-only evidence, not hosted, supported, or
published evidence. It does not derive a provider from a real checked public
generic export (blocked on #119); its bound endpoint and trusted descriptor
bytes are fixtures. It has not been exercised on Linux or Windows/MSVC, or
under a hosted CI sanitizer gate (#163's remaining work). It is not the
generated Rust, C, or C++ consumer (#156, #158, #159 respectively) — those
are separate acceptance surfaces this issue does not build.

## Compatibility and lifecycle

A `CarrierBindingV1` binds one carrier instance to:

- the exact [descriptor](PUBLIC-GENERIC-DESCRIPTOR-V1.md) identity digest it
  serves;
- a `TargetProfile` (`Interpreter`, `NativeC11`, or `CoreWasm` in this
  version — closed, not open for an unlisted target to claim);
- the carrier schema version (`semaprax.public-generic-carrier.v1`);
- an opaque `runtime_identity` digest (compiler/runtime build identity,
  opaque here for the same reason the descriptor's program-root digest is
  opaque: deriving it from a real build is the next tranche's work).

Binding bytes and replay follow the identical convention [Descriptor
v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md#canonical-bytes) uses: a length-framed
identity preimage, a domain-separated digest computed from it (never
transmitted and trusted, always recomputed), and a `replay` function that
requires byte-exact preimage equality against a trusted value. A binding for
one descriptor, target, or runtime generation must never be accepted against
another; `replay` rejects any one of those fields differing, even when the
bytes otherwise decode.

## Bounds

Reused from [Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#bounds):

| Bound | Value |
| --- | --- |
| Max live handles per carrier instance | 257 |
| Max bytes per single owned `Bytes` leaf | 65,536 |
| Max total owned payload bytes per carrier | 16,777,216 (16 MiB) |
| Max record nesting depth (bounds handle-tree depth) | 64 |

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-PG801` | malformed carrier-binding bytes (framing, unknown target profile, unknown schema) |
| `SPX-PG802` | a carrier bound was reached (handle count, byte total, or a framed field's length) |
| `SPX-PG803` | independent replay found the recomputed binding preimage does not equal the submitted one |
| `SPX-PG804` | an illegal state transition, a submitted release order that is not the exact reverse of the canonical obligation order, or an operation on an `Invalid` handle |
| `SPX-PG805` | a handle presented with the wrong generation, or against a different carrier/descriptor binding than the one it was minted for |
| `SPX-PG806` | an attempt to replace an already-sticky `Settled` outcome with a different one |

`SPX-PG8xx` is the range this document allocates; `SPX-PG6xx` stays reserved
for the boundary-profile classifier and `SPX-PG7xx` belongs to [Public
Generic Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md). `SPX-PG9xx` belongs
to [this document's own native C11 physical adapter
section](#native-c11-physical-adapter-issue-154), which restates these codes
rather than reinterpreting them.

## Required tests and evidence

Local evidence only, in `src/public_generic_abi/carrier.rs` and its `tests`
submodule, and in the `machine` and `trace` submodules:

- every legal transition in the [state table](#the-logical-value-state-machine)
  succeeds, driven from every reachable starting state;
- every transition *not* in that table lands on `Invalid` and stays there
  (`Invalid` has no legal outgoing transition, tested directly);
- a full phase-ledger run from `Preparing` through `Settled(Success)`, with
  every argument and result handle's own state transitions checked at each
  phase boundary;
- a failure-before-commit run and a failure-after-commit run, each asserting
  the release order is the exact reverse of the canonical obligation order,
  and that a reordered or partial release sequence is rejected;
- an attempt to set a second, different `Settled` outcome after the first is
  rejected with `SPX-PG806` (sticky failure);
- a wrong-generation handle and a handle bound to a different carrier binding
  are both rejected with `SPX-PG805`;
- golden byte-determinism for `CarrierBindingV1::encode`, a hostile decode
  corpus (truncation, trailing bytes, unknown target profile, oversized
  length claim), and a cross-paired `replay` failure for each bound field.

The `machine` submodule additionally covers, at the whole-call orchestration
level named by issue #153:

- a full success run through every [`CarrierCallMachine`] method, asserting
  every input and result handle reaches `Transferred` and the normalized
  trace records exactly the expected label sequence with sequential
  ordinals;
- **double transfer**: a second `commit_input_transfer` after a successful
  one is rejected, and the already-committed handles are left unchanged;
- **use after transfer**: mutating (`Fill`) an already-`Transferred` handle
  is rejected and the handle latches to `Invalid`;
- **copy-out after release**: consuming (`Consume`) an already-`Released`
  handle is rejected;
- **failure arriving mid-transfer**: with one handle `Initialized` and a
  sibling still `Created`, `commit_input_transfer` is rejected and leaves
  every handle exactly as it was — no partial transfer is observable — after
  which the failure is settled and the handles release in reverse order;
- **release exactly once**: releasing an already-released handle set a
  second time is rejected;
- **cleanup cannot overwrite a sticky failure status**: a settled primary
  failure rejects a later, different cleanup outcome with `SPX-PG806`, while
  a cleanup failure with no earlier failure may legally become the terminal
  status;
- result-staging mirrors: result leaves are invisible (still `Initialized`)
  before `commit_result`, a partially staged result releases only the
  leaves actually completed, and result staging/commit is gated on
  execution having begun and finished exactly once.

No hosted run is recorded for this document; no provider, allocator, or real
target executes anything here. See the accompanying worktree report for the
exact local commands run.

## Nonclaims

This document defines no physical layout, no allocator, no memory
representation, and no generated code. It does not execute, allocate,
transfer, or release anything: every test above exercises the pure state
machine, the pure call-machine orchestration, the pure trace recorder, and
the pure codec, never a real interpreter, native binary, or Wasm module. It
is not evidence that any backend settles a public generic boundary this
LOGICAL layer's own types execute against: [`CarrierCallMachine`] itself
still does not bind to a `VerifiedPublicGenericDescriptor`, parse carrier
bytes, or perform any physical allocation. [Native C11 physical adapter
(issue #154)](#native-c11-physical-adapter-issue-154) below is the first
PHYSICAL adapter to emit the normalized trace and perform real allocation
and release, against a fixture endpoint only — see that section's own
nonclaims for its exact, narrower scope; #155's Core Wasm adapter remains
outstanding. It reuses no v8-v11 carrier bytes and widens none of them. The
target-mapping
table above is naming guidance for a future physical specification, not
that specification itself.
