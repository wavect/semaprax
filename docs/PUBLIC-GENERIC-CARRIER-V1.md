# Public Generic Carrier v1

Audience: backend provider authors on native and Core Wasm, and reviewers of the ownership and settlement contract.

Status: frozen logical specification with a reference codec and local
evidence (`src/public_generic_abi/carrier.rs`, and its `frame`, `machine`,
and `trace` submodules). This is the carrier half of gate #150-#153 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)
and answers issue #171 and issue #153. It defines the ownership state
machine, the phase ledger, a `CarrierBindingV1` wire binding, [Canonical
carrier bytes](#canonical-carrier-bytes) (`LogicalCarrierFrame`, the
payload-bearing wire frame, and `CarrierFrameBinding`, which validates one
against a real `VerifiedPublicGenericDescriptor`-derived plan), the
[call-machine orchestration](#the-call-machine) that drives the state
machine and phase ledger together atomically, and the [normalized trace
vocabulary](#the-normalized-trace) — all as pure, locally-tested logic. It
defines **no physical target mapping** — no C struct layout, no Wasm handle
table implementation, no Rust FFI boundary — and executes nothing itself:
there is no provider, no allocator, and no real target to allocate,
transfer, or release against, in this LOGICAL section.
[Native C11 physical adapter (issue #154)](#native-c11-physical-adapter-issue-154)
and [Core Wasm physical adapter (issue #155)](#core-wasm-physical-adapter-issue-155)
below are the first two PHYSICAL adapters built on top of it, each with real
allocation, release, and normalized-trace emission — locally evidenced only,
against a fixture endpoint, per each section's own scope note. The remaining
per-target adapters and generated consumers (issues #156-#159, #162) are
still outstanding. Public generic ownership remains unsupported and
unpublished.

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

## Canonical carrier bytes

[`carrier::frame::LogicalCarrierFrame`](../src/public_generic_abi/carrier/frame.rs)
is the bounded, target-neutral wire encoding of one prepared input or staged
result: the exact semantic binding facts plus every owned leaf's exact
payload bytes, in canonical order. Like [`CarrierBindingV1`](#compatibility-and-lifecycle)
below, it is a pure codec — nothing here allocates, transfers, or releases a
real value. Canonical byte order:

```text
schema/version
direction
descriptor_digest
endpoint_identity_digest
instance_identity_digest
leaf_inventory_digest
leaf_count                      (8-byte little-endian, not length-framed)
total_payload_length            (8-byte little-endian, not length-framed)
for each leaf in canonical order:
    leaf_path_identity          (length-framed UTF-8)
    canonical leaf kind         (one closed tag byte)
    payload length + payload    (one length-framed field)
carrier_facts_digest            (length-framed; always recomputed, never trusted)
```

Every identity/path field uses the same 8-byte-little-endian-length-then-bytes
framing [`CarrierBindingV1`](#compatibility-and-lifecycle) already uses
(`public_generic_abi::frame`/`read_frame`); `leaf_count` and
`total_payload_length` are raw 8-byte fields, matching the requirement that
"all lengths have one canonical fixed or explicitly framed encoding."
`carrier_facts_digest` is a domain-separated digest
(`semaprax.public-generic-carrier.v1.frame\0`) over every preceding byte;
[`parse_bounded`](../src/public_generic_abi/carrier/frame.rs) always
independently recomputes and compares it before returning a value, so a
bit-flipped or hand-tampered frame that otherwise decodes structurally is
still rejected — the same "never transmitted as a trust input, always
recomputed" discipline `CarrierBindingV1::binding_digest` already uses.

**Scope: flat owned-`Bytes` leaves only.** [`LeafKind`](../src/public_generic_abi/carrier/frame.rs)
is deliberately closed to one variant (`Bytes`) this round, matching the rest
of this milestone's generated consumers and physical adapters: nested
records and Copy scalars remain blocked on #119, so every leaf a frame
carries today is a direct owned `Bytes` leaf. Widening `LeafKind` when #119
lands is additive, not a breaking change to this framing.

**Bounds** (reused from [Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#bounds),
enforced by `parse_bounded` before the corresponding allocation, `SPX-PG802`):
declared `leaf_count` over 256; declared `total_payload_length` over 16 MiB;
any one leaf's declared payload length over 64 KiB. Because
`MAX_TOTAL_PAYLOAD_BYTES` equals exactly `MAX_OWNED_LEAVES_PER_INSTANCE *
MAX_BYTES_PER_LEAF`, the running-total check inside the per-leaf loop is
defense in depth: given the other two bounds already enforced, no combination
of real leaf bytes can independently trip it — it exists so a future change
that breaks that exact relationship fails closed immediately rather than
silently admitting a larger total.

**Hostile-input handling** (`SPX-PG801` malformed/framing, `SPX-PG802`
capacity, `SPX-PG803` replay/self-digest mismatch — the same three codes
[`CarrierBindingV1`](#compatibility-and-lifecycle) already uses, restated for
a payload-bearing frame rather than allocated a fourth time): truncation at
every byte boundary; trailing bytes; an unknown schema or direction; an
oversized length claim; a duplicate leaf path (rejected structurally, by
`parse_bounded` itself, independent of any trusted plan); and a declared
`total_payload_length` that disagrees with the actual sum of leaf payload
lengths (a noncanonical-length refusal, distinct from the capacity and
self-digest checks).

**Binding a parsed frame to a trusted context** is
[`carrier::frame::CarrierFrameBinding`](../src/public_generic_abi/carrier/frame.rs)'s
job, kept deliberately separate from `parse_bounded` — mirroring
`CarrierBindingV1::decode_binding` versus `replay_binding`'s own split — so a
frame that merely decodes is never confused with one that is semantically
bound to the right value:

- `CarrierFrameBinding::from_verified_descriptor(descriptor, direction)`
  derives the trusted plan **directly from a real
  [`VerifiedPublicGenericDescriptor`](PUBLIC-GENERIC-DESCRIPTOR-V1.md)** —
  never from raw or merely parsed descriptor bytes, per this issue's own
  "Required APIs" requirement. It reuses that descriptor's own
  `descriptor_digest()`, `export_id()`, and per-direction
  [`InstanceFacts`](../src/public_generic_type.rs) (`instance_digest` and the
  canonical `owned_leaves` path list) rather than re-deriving or trusting any
  of those facts a second time.
- `validate_frame` checks direction, descriptor/endpoint/instance/leaf-
  inventory digests, and the leaf-path sequence, in that order. A missing
  leaf, an extra leaf, and a reordered leaf are all caught by the same one
  `Vec` equality check against the canonical inventory, since each changes
  the sequence relative to it. A frame that decodes cleanly (self-digest
  intact) but is bound to a different descriptor, endpoint, instance, or
  direction — a "reminted carrier digest with the wrong semantic binding" —
  is rejected here as `SPX-PG803`, never at parse time.

This closes this issue's own "Required APIs" list items
`LogicalCarrierFrame::parse_bounded` and the `LogicalCarrierPlan`
responsibilities (`from_verified_descriptor`, `validate_frame`). It does not
change what [`CarrierCallMachine`](#the-call-machine) requires: that
orchestration still drives opaque `Handle`s only and does not itself bind to
a descriptor or parse carrier bytes — see [LOGICAL versus
PHYSICAL](#logical-versus-physical) and the call machine's own scope note.
Wiring `CarrierCallMachine` to a `LogicalCarrierFrame`/`CarrierFrameBinding`
pair end to end (so a real call is driven from parsed, descriptor-bound bytes
rather than hand-constructed `Handle` values) remains a real physical/wiring
gap, tracked separately (#154/#155 already drive the state machine and trace
against fixture endpoints; wiring the frame codec into that same path is
follow-on work, not performed this round).

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
are separate acceptance surfaces this issue does not build. Issue #156's
generated Rust *calling* consumer links against this exact adapter (the same
`render_reference_provider` output, compiled once and reused, never a second
implementation) without modifying it; see [Rust calling consumer (issue
#156)](PUBLIC-GENERIC-CONSUMERS-V1.md#rust-calling-consumer-issue-156) for
that consumer's own scope, safety argument, and execution evidence.

## Core Wasm physical adapter (issue #155)

Audience: Wasm provider/adapter implementers and reviewers of the physical
allocation and release path.

Status: local, proof-only reference implementation
(`src/public_generic_abi/wasm/`), unsupported and unpublished. This is the
second PHYSICAL adapter built on the LOGICAL layer above, a sibling to
[Native C11 physical adapter (issue #154)](#native-c11-physical-adapter-issue-154):
it decides no legality the [state machine](#the-logical-value-state-machine),
[phase ledger](#the-call-phase-ledger), or [`CarrierCallMachine`](#the-call-machine)
do not already fix, and it emits exactly [the normalized trace
vocabulary](#the-normalized-trace) above, adding no second vocabulary.
Answers issue #155.

**What is different from native, and why.** Wasm linear memory is a
growable byte array addressed by `u32` offsets, grown only in 64 KiB pages
and never shrunk — there is no native pointer a foreign caller could forge
into a dereferenceable address, and no process-wide heap to route every
byte through. `src/public_generic_abi/wasm/memory.rs`'s `WasmLinearMemory`
models exactly that arena, and its `StackAllocator` is a bounded,
exact-last-in-first-out bump allocator over it: every carrier release is
already required to be the exact reverse of allocation order (see [Failure
settlement and release order](#failure-settlement-and-release-order)), so a
strict LIFO allocator is not a simplification of the physical adapter's
job, it is the direct physical form of that logical rule. Freed spans are
zeroed in place for real, observable release, and reused by the next
allocation that fits.

**Rust-hosted, not C-hosted.** Unlike the native adapter (pure C, which
restates the LOGICAL state machine and trace vocabulary independently
because C cannot call this repository's Rust types), this Wasm adapter
(`src/public_generic_abi/wasm/provider.rs`'s `WasmProvider`) drives
[`carrier::machine::CarrierCallMachine`](../src/public_generic_abi/carrier/machine.rs)
directly for every phase, commit, sticky-settlement, and release-order
decision — the literal, not restated, LOGICAL layer. This is a stronger
reuse guarantee than native's own C restatement can offer, with one stated
gap: `CarrierCallMachine` exposes no "consume a successfully transferred
input on the call's success path" transition, only the failure-shaped
`release_input_after_transfer`. This adapter reuses that same method
unconditionally after execution finishes, success or failure, exactly
mirroring what native's own `spx_pg_release_leaves` helper does — input
bytes are always physically freed once the endpoint has read them,
regardless of outcome. This is the shared machine's own documented scope
gap, not something this physical adapter re-decides, and it is why the two
release ordinals (`LeafRelease`, `CarrierRelease`) fire on every call, not
only a failing one — matching native's behavior exactly.

**Deferred scope**, identical to native's own: deriving a provider from a
real checked *generic* export requires #119's still-blocked owned-record
ownership evidence. Until that lands, the bound endpoint is the same
fixture (`spx_pg_wasm_endpoint_reverse_bytes_v1`, byte-reversal per owned
leaf) operating on the same flat owned-`Bytes` shape, and the trusted
descriptor bytes an `open` caller replays against are a hand-constructed
fixture compared byte-for-byte, not [`descriptor::verify`](PUBLIC-GENERIC-DESCRIPTOR-V1.md)
output, since no admitted public generic export exists yet.

### Provider binding

[`wasm::binding::WasmProviderBindingV1`](../src/public_generic_abi/wasm/binding.rs)
is a new, physical-layer-only artifact layered on top of — never
modifying — [`CarrierBindingV1`](#compatibility-and-lifecycle) above,
mirroring [`NativeProviderBindingV1`](#provider-binding)'s own convention
exactly (framed fields, a domain-separated digest never transmitted,
byte-exact `replay`) for `TargetProfile::CoreWasm` instead of
`TargetProfile::NativeC11`. It wraps a `CarrierBindingV1` naming
`TargetProfile::CoreWasm` unchanged, and adds exactly the facts a physical
Wasm provider needs and the logical carrier never should:
`wasm_adapter_abi_version` (closed to `"v1"` this round),
`provider_artifact_digest`, `exported_endpoint_export_name` (a Wasm export
name, since Wasm has no linker-visible "symbol" the way a native shared
object does), a `compiler_backend_version` fact, and its own closed
`SupportPublicationState` (`unsupported-unpublished` is the only admitted
value) — independent of native's own `SupportPublicationState` so the two
physical adapters never share mutable state through a common type.

### Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-PG910` | malformed Wasm provider binding bytes (framing, unknown ABI version, unrecognized support/publication claim, or an embedded carrier binding not naming `TargetProfile::CoreWasm`) |
| `SPX-PG911` | independent replay found the recomputed Wasm binding preimage does not equal the submitted one |
| `SPX-PG912` | a physical memory access (`offset`, `length`) does not fit inside the current linear-memory arena, independent of any carrier or handle-level legality question |
| `SPX-PG913` | the bounded allocator could not satisfy an allocation: it would exceed `MAX_BYTES_PER_LEAF`, `MAX_TOTAL_PAYLOAD_BYTES`, or growing memory failed |
| `SPX-PG914` | a release was asked to free a span that is not exactly the most recently allocated, still-live span — the physical proof that a submitted release order violates the exact-reverse-of-allocation-order rule |
| `SPX-PG915` | a handle is not live in this provider's registry: foreign, stale, already consumed/released, forged, or presented against a provider that never minted it |
| `SPX-PG916` | a handle was presented where a different structural position or role was required (root vs. leaf, input value vs. result), independent of the handle's own lifecycle state |

`rg -n "SPX-PG9" docs src tests` at the time this section was written found
`SPX-PG901`-`SPX-PG905` already in use by [Native C11 physical adapter
(issue #154)](#native-c11-physical-adapter-issue-154); this section
allocates `SPX-PG910`-`SPX-PG916`, leaving `SPX-PG906`-`SPX-PG909` free for
that section's own future growth. The caller-facing `WasmPgStatus` return
codes this adapter actually returns (`Ok` = 0 through `NullOrWrongKind` =
13) are a deliberate, non-required convergence with native's own
`spx_pg_status_v1` integer vocabulary — see
`src/public_generic_abi/wasm/provider.rs`'s own doc comment — chosen only
because a caller-facing status vocabulary the two physical adapters already
happen to agree on is one less translation issue #162's cross-engine
comparison has to solve; the two adapters' internal `SPX-PG9xx` diagnostic
codes remain independent, restating logical `SPX-PG7xx`/`SPX-PG8xx` codes
identically but minting disjoint physical-only ranges.

### Handle safety

Every handle this adapter mints is recorded in one per-provider registry
(`src/public_generic_abi/wasm/registry.rs`'s `HandleRegistry`) before any
use, and every use looks the handle up there before touching linear
memory — reusing, not reinventing, native's own "scan the registry for the
exact value first, dereference only once a live, correctly-kinded entry is
found" pattern, restated for Wasm's numeric-handle world. Two independent
facts make cross-pairing deterministic rather than accidental: first, two
providers hold two independent registries, so a handle minted by one is
simply absent from the other's table; second, because two freshly opened
providers can (and in the required test do) independently mint the exact
same `(id, generation)` pair, every handle this adapter hands back also
carries an opaque per-provider tag checked before any registry lookup runs,
so cross-provider misuse is caught by construction rather than by
coincidentally not colliding. A result handle's `id` range is disjoint from
that same call's input `id` range even though both share one `generation`
(the call is the one "carrier instance" this document describes,
encompassing both directions) — without that split, a call's result root
would be byte-identical to that same call's already-released input root,
and the registry could not tell a stale input handle from the fresh result
handle now occupying the identical key. This is a physical-layer choice the
LOGICAL layer's `id` numbering (root `0`, leaves `1..=256`) leaves open per
direction; native's adapter never needs an equivalent split because its
value and result objects already have distinct pointer identities.

### Allocation, release, and sticky failure

Every byte this adapter ever allocates — the always-zero-length root
marker and every leaf's payload alike — routes through
`WasmLinearMemory`/`StackAllocator`, so `WasmProvider::live_allocations`
and `WasmProvider::live_bytes` are exact counts, not samples, and
`WasmProvider::live_handles` is the registry's own exact live-entry count.
A test-only `WasmProvider::test_inject_failure` arms deterministic failure
at any of the 14 non-terminal [normalized trace](#the-normalized-trace)
ordinals for the next `input_prepare`/`call` sequence, rolling back every
allocation already made — physical evidence that "failure at every logical
injection point" leaves zero live allocations and handles. Sticky failure
([above](#the-call-phase-ledger)) is not reimplemented: every settlement
attempt goes through `CarrierCallMachine::settle` itself, and
`WasmProvider::test_settlement_overwrite_attempts` counts (without
applying) any later, different attempt — exercised directly by a
cleanup-failure-with-no-earlier-failure case (legally becoming the terminal
status) and a cleanup-failure-after-an-earlier-failure case (discarded,
sticky), both driven through the two release ordinals exactly as native's
own two release-ordinal injection sites are.

### Wasm-hosted fixture and evidence

Two separate, deliberately non-overlapping pieces of evidence answer "does
this really touch Wasm," matching this section's own LOGICAL/PHYSICAL split
at a finer grain:

- **The protocol adapter** (`src/public_generic_abi/wasm/provider/tests.rs`,
  a Rust unit-test module) exercises the full carrier protocol — the
  success round trip with two-pass, byte-identical repeated export; the
  exact and first-over-bound leaf-count and leaf-byte-size cases; a legal
  pre-call abandon; handle hostility (stale generation, wrong kind both
  directions, cross-provider even on a colliding `(id, generation)` pair,
  double release); buffer-too-small nonconsuming export; sticky failure and
  the cleanup-becomes-terminal case; and the full 0-13 failure-injection
  matrix (every non-terminal `TraceLabel`, one fresh provider and call
  each), asserting zero live allocations, zero live bytes, and zero live
  handles after every terminal case — against `WasmProvider`, entirely in
  Rust, no external process.
- **The real-Wasm-host proof**
  (`src/public_generic_abi/wasm/reverse_probe.mjs`, run by
  `tests/public_generic_wasm_adapter_v1`) is a small, real, hand-authored
  Node script proving the byte-reversal fixture endpoint executes for real
  against a genuine `WebAssembly.Memory` instance — real linear memory,
  grown in real 64 KiB pages by a real WebAssembly host (Node/V8), not a
  Rust-hosted stand-in — with the freed span zeroed and asserted zero
  afterward. This is deliberately narrower than the full protocol: it
  proves the physical primitive is real under a real Wasm host, answering
  this issue's "Direct Wasm host fixture executes the real endpoint"
  criterion at the primitive level; it does not re-run the handle/registry/
  sticky-settlement protocol, which has no access to `CarrierCallMachine`
  from JavaScript and is exercised in Rust instead, per the split above.

### Nonclaims (Core Wasm adapter)

This adapter is local, proof-only evidence, not hosted, supported, or
published evidence. It does not derive a provider from a real checked
public generic export (blocked on #119); its bound endpoint and trusted
descriptor bytes are fixtures. It does not compile or execute a full
`.wasm` module produced by this repository's own Wasm backend or any
external toolchain — the real-Wasm-host proof above exercises the
byte-reversal primitive and genuine `WebAssembly.Memory` allocation
directly, not a compiled module, and the full carrier protocol is exercised
in Rust against `WasmProvider`, not replayed a second time in JavaScript.
It has not been exercised under a hosted CI sanitizer or fuzzing gate. It
is not the generated TypeScript/Wasm consumer (#157) — that is a separate
acceptance surface this issue does not build, and #157 is a foreign-caller
concern with its own separate trust boundary, not a re-scoping of this
provider-side adapter. It is not a cross-engine equivalence test against
the native adapter: real compiled-and-executed native (at `-O0`/`-O2`) has
no in-process Rust adapter comparable to this one, so it is not a party to
`carrier::settlement_corpus` below; see that section's own nonclaims. It
*is* now a party to a cross-engine equivalence test against the reference
interpreter adapter — see [Reference interpreter physical adapter (issue
#162)](#reference-interpreter-physical-adapter-issue-162) below.

## Reference interpreter physical adapter (issue #162)

Audience: implementers and reviewers of cross-engine settlement
equivalence, and of the reference interpreter's own boundary adapter.

Status: local, proof-only reference implementation
(`src/public_generic_abi/interpreter.rs`), unsupported and unpublished.
This is the third PHYSICAL adapter built on the LOGICAL layer above, a
sibling to [Native C11 physical adapter (issue
#154)](#native-c11-physical-adapter-issue-154) and [Core Wasm physical
adapter (issue #155)](#core-wasm-physical-adapter-issue-155): it decides no
legality the [state machine](#the-logical-value-state-machine), [phase
ledger](#the-call-phase-ledger), or [`CarrierCallMachine`](#the-call-machine)
do not already fix, and it emits exactly [the normalized trace
vocabulary](#the-normalized-trace) above, adding no second vocabulary.
Answers part of issue #162 (the reference-interpreter-adapter and
cross-engine-corpus portions; native O0/O2 execution, generated-consumer
execution, and evidence-replay tooling remain outstanding — see this
section's own nonclaims).

**Why a third adapter, and why now.** Before this section, no adapter drove
`CarrierCallMachine` as "the reference interpreter route" at all —
`carrier.rs`'s own scope note ("no provider, no native or Wasm adapter, and
no execution") and #153-#155's own deferred-scope notes left
`TargetProfile::Interpreter` defined but never bound to a concrete adapter.
Issue #162's crux is proving equal observable behavior across engines, and
that requires a second real, executing adapter to compare the Wasm adapter
against — a direct interpreter function call would skip the boundary being
proven, per this issue's own instruction.

**Physical model, deliberately different from both other adapters.**
Native's C11 adapter allocates from the process heap through pointer
identities; the Wasm adapter allocates from one bounded, page-grown
linear-memory arena through a strict LIFO stack allocator, because a
release out of allocation order is otherwise unobservable in a reused byte
array. `InterpreterProvider`'s `Heap` (`src/public_generic_abi/interpreter.rs`)
is a third, genuinely different physical model: a plain, arena-free slot
table (`HashMap<u32, Vec<u8>>`) with no address space and no forced LIFO
discipline — release order is enforced once, upstream, by
`CarrierCallMachine`'s own reverse-obligation-order rule, not restated by
this physical layer. Exercising three distinct physical representations
against the same case shapes is the actual point: agreement is not an
artifact of one shared allocator.

**No new wire-binding artifact.** Unlike native and Wasm, this adapter
introduces no `InterpreterProviderBindingV1`: the reference interpreter is
in-process Rust with no cross-language wire boundary to cross, so
`InterpreterProvider::open` replays directly against
[`CarrierBindingV1`](#compatibility-and-lifecycle) naming
`TargetProfile::Interpreter`. No new diagnostic code range is allocated;
`InterpreterProvider` reuses `carrier.rs`'s own `SPX-PG8xx` codes and
`wasm::registry`'s `SPX-PG915`/`SPX-PG916` handle-safety codes rather than
minting a fourth, parallel range for facts those codes already name.

**Deferred scope**, identical to native's and Wasm's own: deriving a
provider from a real checked *generic* export needs #119's still-blocked
owned-record ownership evidence, so the bound endpoint here is the same
fixture (`spx_pg_interpreter_endpoint_reverse_bytes_v1`, byte-reversal per
owned leaf) operating on the same flat owned-`Bytes` shape, and the
trusted descriptor bytes an `open` caller replays against are a
hand-constructed fixture compared byte-for-byte, not
[`descriptor::verify`](PUBLIC-GENERIC-DESCRIPTOR-V1.md) output.

### Interpreter-hosted evidence

`src/public_generic_abi/interpreter/tests.rs` exercises `InterpreterProvider`
on its own: a success round trip with two-pass, byte-identical repeated
export; zero-length and embedded-zero-byte leaves; the exact and
first-over-bound leaf-count and leaf-byte-size cases; a stale handle from a
prior provider generation; the full 0-13 failure-injection matrix (every
non-terminal `TraceLabel`, one fresh provider and call each), asserting
zero live allocations, zero live bytes, and zero live handles after every
terminal case; repeated invocation with no state leak between independent
calls; and provider recreation rejecting a stale child handle from a prior
generation.

### Cross-engine settlement corpus (issue #162)

`src/public_generic_abi/carrier/settlement_corpus.rs` is
`semaprax.public-generic-settlement-corpus.v1`: one shared case table (the
base success/rejection shapes above, plus one case per non-terminal
`TraceLabel` failure-injection ordinal) run against both
`InterpreterProvider` and `WasmProvider`, with one `compare` checker that
diffs accept/reject, the normalized status, the result carrier bytes, the
normalized trace label sequence (the literal `CarrierCallMachine` trace
each adapter's `settle` helper now snapshots via a test-only
`test_last_trace` accessor, not a second restatement of it), the canonical
leaf release order extracted from that trace, final live
allocation/handle/byte counts, the sticky-settlement overwrite-attempt
count, and the provider close status — each engine against an
independently pinned expectation, and engine against engine. Four
`should_panic` negative controls prove `compare` can actually fail (a
perturbed result, a status that does not match the pinned expectation, a
truncated trace, and a leaked resource count), matching issue #160's own
consumer-corpus pattern of proving the checker itself, not merely
asserting a pass.

### Nonclaims (reference interpreter adapter and cross-engine corpus)

This adapter and corpus are local, proof-only evidence, not hosted,
supported, or published evidence. The adapter does not derive a provider
from a real checked public generic export (blocked on #119); its bound
endpoint and trusted descriptor bytes are fixtures. The corpus compares
only `InterpreterProvider` and `WasmProvider`: native C11 has no
in-process Rust adapter analogous to either (only a C-source renderer,
`native::template::render_reference_provider`), so it is not a party to
this corpus, and real compiled-and-executed native (at `-O0`/`-O2`) equality
against this same case shape, plus generated Rust/TypeScript/C/C++ consumer
execution, and independent evidence-replay tooling, remain
`tests/public_generic_native_adapter_v1/**`'s and
`tests/public_generic_wasm_adapter_v1/**`'s own leased, outstanding work.
Peak allocation/handle counters are not tracked by either adapter (only
live/current counts are); the corpus compares final (post-terminal) counts
only. Nested multi-level owned records are not exercised (#119's
flat-owned-`Bytes`-leaves limitation applies to both adapters equally); the
corpus's two-leaf case stands in for "at least two owned leaves with
visible structural order," not a nested record. Native O0/O2 sanitizer
equivalence, hosted CI execution, and the evidence-artifact canonical
summary/independent-replay format issue #162 also describes are not built
by this section.

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
- a wrong-generation handle is rejected with `SPX-PG805`
  (`a_wrong_generation_handle_is_rejected`). Corrected in place: `Handle`
  itself carries only `{id, generation}`, no binding reference, so
  "generation" is the only mechanism this logical layer has for
  distinguishing one carrier instance (and therefore one binding) from
  another — there is no separate "handle bound to a different carrier
  binding" check or test independent of generation at this layer. Verifying
  that a real generation counter is actually minted per real provider
  instance (so two instances never share one) is a physical-adapter fact,
  proven by `native`'s and `wasm`'s own handle-registry tests, not by this
  logical module;
- golden byte-determinism for `CarrierBindingV1::encode`, a hostile decode
  corpus (truncation, trailing bytes, unknown target profile, oversized
  length claim), and a cross-paired `replay` failure for each bound field.

The `frame` submodule covers [Canonical carrier bytes](#canonical-carrier-bytes),
issue #153's Section B, at the wire-byte level:

- golden byte-determinism for `LogicalCarrierFrame::encode` and
  `carrier_facts_digest`;
- a minimal (zero-leaf) frame, a zero-length `Bytes` leaf, and a leaf with
  embedded zero bytes, all round-tripping exactly;
- each first-over-bound case: one leaf's payload one byte over
  `MAX_BYTES_PER_LEAF`; one leaf over `MAX_OWNED_LEAVES_PER_INSTANCE`; a
  declared `total_payload_length` one byte over `MAX_TOTAL_PAYLOAD_BYTES`
  (probed directly at the header level, since the per-leaf and leaf-count
  bounds already make that total unreachable through real leaf bytes); and a
  positive case reaching the total-payload bound exactly through
  `MAX_OWNED_LEAVES_PER_INSTANCE` leaves at `MAX_BYTES_PER_LEAF` each;
- a duplicate leaf path, rejected by `parse_bounded` itself as malformed;
- a missing leaf, an extra leaf, and a reordered leaf, each rejected by
  `CarrierFrameBinding::validate_frame` specifically on the leaf-sequence
  check (asserted on the diagnostic message, not only its code, so a
  regression that made an earlier field check swallow these cases would fail
  the test);
- a self-consistent (correctly self-digested) frame bound to the wrong
  direction, descriptor, endpoint, or instance, each rejected by
  `validate_frame` as `SPX-PG803` — the "reminted carrier digest with the
  wrong semantic binding" case — while the correctly bound plan still
  accepts the same frame;
- a tampered frame (one payload byte flipped, stale digest kept) rejected by
  `parse_bounded`'s own self-digest check, before any binding plan is ever
  considered;
- truncation at every single byte boundary of a well-formed frame, trailing
  bytes, an unknown schema, an unknown direction, an oversized length claim,
  and a declared total-payload length that disagrees with the actual sum of
  leaf payload lengths (noncanonical length, distinct from the capacity and
  self-digest checks);
- `CarrierFrameBinding::from_verified_descriptor`, driven through the real
  parser, resolver, descriptor producer, and independent descriptor verifier
  (never a hand-built fixture) against a `Pair<Leaf, i64>`-in/`Leaf`-out
  export chosen specifically so input and result facts genuinely differ:
  each direction's binding is checked against that direction's own
  `owned_leaves`, not the other one's, which a "reads `input_facts()`
  regardless of the requested direction" bug would fail.

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
bytes, or perform any physical allocation — only [`CarrierFrameBinding`](#canonical-carrier-bytes)
does, and only at the byte-validation layer, not wired into
`CarrierCallMachine`'s own orchestration this round. Failure injection by
semantic ordinal (issue #153's Section H) exists as physical, real evidence
in the native and Wasm adapters below (`spx_pg_test_inject_failure_v1`,
`WasmProvider::test_inject_failure`, each driving or restating this exact
LOGICAL layer), not as a separate ordinal-indexed API on this LOGICAL layer
itself; this module's own tests exercise the identical failure shapes by
direct, pre-commit state manipulation instead (see `machine`'s
`failure_arriving_mid_transfer_leaves_no_handle_transferred` and its result-
staging mirror), which is equivalent for a pure state machine with no
allocation of its own to roll back. [Native C11 physical adapter
(issue #154)](#native-c11-physical-adapter-issue-154), [Core Wasm
physical adapter (issue #155)](#core-wasm-physical-adapter-issue-155), and
[Reference interpreter physical adapter (issue
#162)](#reference-interpreter-physical-adapter-issue-162) below are the
three PHYSICAL adapters to emit the normalized trace and perform real
allocation and release, each against a fixture endpoint only — see each
section's own nonclaims for its exact, narrower scope. The interpreter and
Core Wasm adapters are now cross-engine compared directly by
`carrier::settlement_corpus` (issue #162); native C11 has no in-process
Rust adapter to include in that same corpus (only a C-source renderer), so
#156-#159's generated consumers, real compiled-and-executed native
`-O0`/`-O2` equality, and independent evidence-replay tooling remain
outstanding.
It reuses no v8-v11 carrier bytes and widens none of them. The
target-mapping
table above is naming guidance for a future physical specification, not
that specification itself.
