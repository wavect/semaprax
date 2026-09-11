# Public Generic Carrier v1

Audience: backend provider authors on native and Core Wasm, and reviewers of the ownership and settlement contract.

Status: frozen logical specification with a reference codec and local
evidence (`src/public_generic_abi/carrier.rs`). This is the carrier half of
gate #150-#153 of the [Public Generic Ownership
milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md) and answers issue #171.
It defines the ownership state machine, the phase ledger, the trace
vocabulary, and a `CarrierBindingV1` wire binding, with a reference codec and
a pure state-machine implementation. It defines **no physical target
mapping** — no C struct layout, no Wasm handle table implementation, no Rust
FFI boundary — and executes nothing: there is no provider, no allocator, and
no real target to allocate, transfer, or release against. That is PG-7's
remaining work (issues #154-#159), which this round does not touch. Public
generic ownership remains unsupported and unpublished.

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
Generic Descriptor v1](PUBLIC-GENERIC-DESCRIPTOR-V1.md).

## Required tests and evidence

Local evidence only, in `src/public_generic_abi/carrier.rs` and its `tests`
submodule:

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

No hosted run is recorded for this document; no provider, allocator, or real
target executes anything here. See the accompanying worktree report for the
exact local commands run.

## Nonclaims

This document defines no physical layout, no allocator, no memory
representation, and no generated code. It does not execute, allocate,
transfer, or release anything: every test above exercises the pure state
machine and the pure codec, never a real interpreter, native binary, or Wasm
module. It is not evidence that any backend settles a public generic
boundary. It reuses no v8-v11 carrier bytes and widens none of them. The
target-mapping table above is naming guidance for a future physical
specification, not that specification itself.
