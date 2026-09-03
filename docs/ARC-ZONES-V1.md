# Deterministic ARC Zone Model v1

- Status: Locally evidenced hidden proof model; no runtime reference counting,
  allocator, language syntax, compiler, backend wiring exists or is claimed
- Version: 0.1
- Audience: language, compiler, runtime, and conformance-test implementers;
  agents auditing shared-immutable ownership semantics before any
  implementation exists

## Summary

This document fixes the bounded target-neutral model that a future
shared-immutable ARC and managed-zone implementation must preserve. The
repository contains `src/arc_zones.rs`, a deterministic proof-data model of
retain/release reference counting inside explicit opt-in managed zones: a
bounded object graph per zone, a closed retain/release state machine with an
exact deterministic finalization order (reverse construction with
cycle-participation deferral), a closed cycle policy that rejects cycles at
zone exit with canonical diagnostics instead of leaking silently, escape
demotion as a deterministic rewrite rule for proven zone-local shared handles,
and closed concurrency annotations under which zones are single-threaded by
declaration and any cross-zone or cross-thread sharing requires an explicit
`Shareable` mark on the shared object.

The module deliberately contains no allocator, no reference-counted smart
pointer, no runtime integration, no language syntax, no parser/HIR/Graph/
verifier/backend change, and no real allocation behavior. Like the callable-v3
settlement model, everything it produces is evidence of what a conforming
implementation MUST do, never authority to allocate or destroy anything. An
object is a bounded proof record; "finalization" records evidence only and
performs no physical destruction. No runtime RC integration exists.

The key rule is:

> Within one managed zone, objects never outlive their zone: every live object
> is drained at zone exit in exact reverse construction order through its
> payload links in canonical target order; objects participating in a retained
> cycle defer that drain and force the zone exit to fail closed with one
> canonical smallest-member witness diagnostic — a cycle is rejected, never
> leaked silently and never auto-collected.

## Relationship to existing contracts

This model extends, but does not replace, existing contracts:

- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) defines target-neutral
  semantic cleanup order and sticky failure selection for single-function
  cleanup plans; this model lifts the same ordering discipline to a per-zone
  object graph. Cleanup-plan vectors remain canonical execution order; this
  model never repairs, sorts, or substitutes them.
- [RFC 0004](RFC-0004-NATIVE-CALL-SETTLEMENT.md) defines recovery settlement
  for native owned calls; its evidence-not-authority framing and its preference
  for bounded rejection over uncertain cleanup are reused here.
- [Deterministic Scoped Task Model v1](SCOPED-TASKS-V1.md) shares the strict
  containment-tree shape, the canonically ordered inventories, and the closed
  `Shareable` annotation vocabulary; zones and scopes are separate models and
  neither subsumes the other.
- RFC 0001 names shared immutable ARC and opt-in managed zones as part of the
  full memory model; this document proves none of that integration and changes
  no completion row beyond the bounded Partial status recorded in the
  completion matrix.
- The completion-matrix row "Shared immutable ARC and opt-in managed zones"
  remains far from complete: retain/release correctness, cycle policy, escape
  optimization, and concurrency constraints are now fixed as bounded proof data
  for one admitted model shape, while real allocation, compiler analysis,
  language syntax, weak references, cross-backend equivalence, and public
  surface remain open exactly as before.

## Normative goals

The terms **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

For an admitted model:

1. Zones **MUST** form a strict containment tree with exactly one root; each
   non-root zone **MUST** have exactly one existing parent, entered only while
   its parent is the innermost open zone.
2. Zone exits **MUST** be balanced: only the innermost open zone may exit, and
   exiting a closed or non-innermost zone **MUST** be rejected.
3. Every mutating operation **MUST** address objects homed in the innermost
   open zone; touching another zone's object **MUST** be rejected as foreign.
4. Releases **MUST** be accounted: releasing more references than the script's
   outstanding explicit handle plus unreleased base reference **MUST** be
   rejected as a double release, and an object kept alive solely by payload
   links **MUST** stay live through that rejection.
5. Cross-zone or cross-thread payload sharing **MUST** require an explicit
   `Shareable` mark on the shared target; zones declare their single executing
   thread, and there is no inside-zone parallelism by declaration.
6. Escape demotion **MUST** be exactly the rewrite of a proven zone-local
   shared handle — sole unreleased base reference, zero incoming payload
   links, no prior demotion — into unique ownership; any later shared use of a
   demoted object **MUST** fail closed.
7. Zone exit **MUST** drain all remaining live zone objects in exact reverse
   construction order, cascading through outgoing payload links in canonical
   target order, depth-first.
8. Live objects participating in a directed cycle (strongly connected component
   of size greater than one, or a self-loop) **MUST** be deferred out of that
   drain and **MUST** reject the zone exit with one canonical witness: the
   smallest stable identity among the zone's cycle participants. The run then
   terminates rejected; further operations fail closed.
9. Every structural ambiguity or bound violation **MUST** fail closed at model
   construction or at the offending operation.

## Bounded structure

`semaprax.arc-zones-model.v1` enforces:

| Quantity | Maximum |
| --- | ---: |
| Zones per model | 4,096 |
| Objects per model | 4,096 |
| Script operations per run | 65,536 |
| Validation work units (`zones * (objects + 1) + objects`) | 1,000,000 |

Zone identities form the containment tree; object identities are globally
unique and homed in exactly one zone. Declared payload links are not static
model structure: they are created and destroyed by script operations against
live state, so cycles are observed where they arise — at zone exit — rather
than pre-declared. These bounds make validation finite; they are not an
implemented allocator capacity or runtime guarantee.

## Closed state vocabulary

Object phases are exactly `NotConstructed -> Live -> Finalized`; a finalized
object never revives, and re-construction is rejected. While live, an object's
strong-reference total is the sum of its unreleased base reference (exactly
one, from construction), one per live incoming payload link, and one per
outstanding explicit retain. Payload links are a set: a duplicate live link is
rejected, and removing an unknown link is rejected.

Operations are the closed eight: `construct`, `retain`, `release`, `link`,
`unlink`, `demote`, `enter_zone`, `exit_zone`. Observable events are closed
too: `constructed`, `retained`, `released`, `linked`, `unlinked`,
`escaped_to_unique`, `finalized` (carrying cause `release`, `cascade`, or
`zone_exit`), `zone_entered`, `zone_exited`, and `zone_rejected_cycle`
(carrying zone and witness). Concurrency annotations are the closed pair
`Shareable`/`NotShareable`; they are recorded projections of declared intent
and imply no thread or aliasing analysis of any real program.

The chosen cycle policy is rejection, documented deliberately: when a zone
exit finds live cycle participants, it emits one `zone_rejected_cycle` event
naming the smallest participant as witness, marks the run rejected, and stops.
It does not collect the cycle, does not leak silently, does not finalize cycle
members, and does not guess which member "owns" the cycle. A conforming
implementation MUST preserve this fail-closed behavior or replace this model
with a new versioned one.

## Deterministic traces and digests

Every run projects a canonical JSON trace bound to the model fingerprint, the
run status (`running`, `complete`, `rejected`), and the rejection witness once
present:

```json
{"schema":"semaprax.arc-zones-trace.v1","model_fingerprint":"…","status":"…","rejected_witness":null,"events":[…]}
```

Model and trace projections use separately domain-separated SHA-256
fingerprints (`semaprax.arc-zones-model-fingerprint.v1` and
`semaprax.arc-zones-trace-fingerprint.v1`) over length-prefixed bytes, so
identical logical models built from different input orders are byte-identical
and cross-domain digest confusion fails. Declarative inventories (zones,
objects) are canonically ordered so input permutation cannot change any
projection; scripts are ordered semantics, like program statement order, and
are executed verbatim. These projections are test evidence, not a wire format.

## Required executable evidence

`tests/cleanup_backends/arc_zones_model.rs` plus seven focused module units currently cover:

- pinned known-answer trace digests for four canonical scenarios:
  - shared fan-out release with canonical-order cascade:
    `b4d9a89367c410b74b243b2e4c206e334f7a2883161431a53bcc6aee3eece956`
  - cycle rejection at zone exit with smallest-member witness:
    `c25ca301dadced10c52cdbf6593e8b428524734ff5d1e3234b6255cd5ff09e51`
  - escape demotion of a proven zone-local shared handle:
    `a9da55d283c201899b99fe5e5389da682edb86f181485bc5cb6c73e8f36169e7`
  - nested zones draining children before parents in reverse construction:
    `f04b2180c74cb364ea6734cd50ff723af21956018c8fa18749a3ec071286833e`
- exact event sequences and finalization causes for fan-out release, cascade
  ordering, children-before-parents drains, and demoted-object teardown;
- hostile rejections: release of a foreign-zone handle, double release beyond
  every outstanding reference while the linked object stays live, unbalanced
  zone exit, construct outside the home zone, already-constructed objects,
  dead/unconstructed operands, duplicate and unknown live links, cross-thread
  sharing without `Shareable`, shared use after demotion, and inapplicable
  demotion;
- structural hostility: missing/multiple roots, duplicate identities, unknown
  zones, self-parenting zone cycles, invalid identities, and bound violations;
- determinism under inventory permutation and double execution (identical
  fingerprints, canonical JSON, event vectors, and digests);
- byte-pinned canonical trace projection and JSON-validity parsing of both
  projections including partial-run status; and
- domain separation between model fingerprints and trace digests.

These cases prove the bounded deterministic model only.

## Explicit nonclaims

Deterministic ARC Zone Model v1 adds no language syntax, no parser, HIR,
Graph, verifier, formatter, CLI, compiler-backend, or Wasm change, and no
runtime RC integration: no allocator, no heap, no reference-count mutation of
real storage, no smart pointer, no weak references, no finalizer execution, no
threads, and no real allocation behavior of any kind. It performs no aliasing,
escape, liveness, or data-race analysis of real programs, no cycle collection,
no leak accounting, no performance claim about retain/release cost, and no
cross-backend or conformance claim. `Shareable` marks grant no thread-safety;
demotion grants no unique-ownership enforcement on real code; rejection
diagnostics are proof data, not compiler diagnostics with codes. It mutates no
source, exposes no CLI, and satisfies none of the remaining completion-gate
requirements for shared-immutable ARC and managed zones. No feature is
"implemented" beyond this proof data and its executable evidence.

## Current status

The model, its seven focused module units, and the eight-test integration
suite are locally evidenced under fmt, strict clippy, and the focused test
gates. There is no public surface change beyond the additive library module,
no diagnostic family, and no backend behavior change. Any future compiler or
runtime integration must preserve the containment, balance, locality,
accounting, sharing-annotation, demotion, drain-order, and cycle-rejection
rules above and must not cite this document as evidence for allocation or
finalization semantics it does not implement.
