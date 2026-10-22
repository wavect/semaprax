# Immutable nominal payload ownership v1

Status: draft specification; qualification remains as recorded below.

Audience: compiler contributors, tool authors, and reviewers.

This source implementation is pending executable qualification. Public SEMAPRAX source, semantic/graph projections, diagnostic order, ordered case/field/parameter values and cache wire bytes remain the contract. The internal Rust sequence carrier changes from Vec to NominalPayload; no pointer identity grants semantic authority.

An explicit existing caller byte reservation acquires a whole immutable cases, fields or parameter owner. Already-funded Vec backing moves only after all owner/receiver/disposal reservations succeed. Case-field entries retain a validated ordinal in the same whole cases owner; no field Vec or per-case Arc is cloned. Equality/debug/codec inspect the complete ordered values. Decoding retains its actual legacy facts and never substitutes a trusted payload from an ID match.

Construction acquires one source-shaped iterative rollback frontier before any fallible identity acquisition. Local case vectors, field vectors, and an already-resolved field type awaiting identity all retain that prepaid frontier. Published shared owners each acquire their own last-owner disposal scratch. Construction scratch is released before publication; later host mutation cannot reuse a stale source frontier. Shared mutation requires explicit paid materialization; ordinary mutable dereference cannot perform copy-on-write. Legacy Vec producers and decoded/hostile inputs retain their explicit compatibility representation. All actual enlarged nominal-map nodes are charged in addition to unchanged caller prebounds, with no credit for removed copies.

A fully funded index constructor must acquire the legacy success producer's allocations as well as the new shared owners: source-name backing, every receiving map node, parameter sequences, source-type frames/results/arguments/function boxes, inheritance snapshots/effective fields, and complete type-fact keys/memo/frontiers. Declaration identity payment is not payment for those other allocations. Each actual acquisition precedes allocation and preserves the same caller error without a later callback or allocation. Completed recursive roots enter the prepaid rollback guard before the next metadata acquisition. The pthread mutex's lazy native backing is acquired and initialized before any published rollback guard can lock it; inline futex targets acquire no nonexistent native box. Full constructor exact/short/zero controls derive the entire same-owner debit, including unchanged ambient formatting, and independently require actual requested backing <= paid debit through Drop. Current086c runtime exposed incomplete legacy producer payment; the in-progress paid-site corrective is not executable closure until those unchanged controls pass.

The actual workspace core owns one invocation base bundle across non-hit module resolutions. The original core ledger must still be current; foreign/restored scopes permanently select a typed detached producer failure without acquiring or reusing base storage. Full source verification precedes producer acquisition. The bundle has the separately specified finite fixed-schema producer counter; no authored/replay work allowance is created or reset. Bytes, work and structural producer failures stay distinct until publication through the existing prepaid report text/slot. The index retains complete logical maps; only the exact current compiler-owned AST source pair may select schema payloads. Authored or decoded ID matches substitute nothing.

Standalone public/no-owner resolution, final linker activation, decoded caches and selected extension-prelude ownership remain explicit residuals. No cap increase, forecast/P discount, total-deficit inference, runtime-fit claim or full827/828 completion is authorized by this source change. Qualification must include complete old/current full maps/HIR/wire/diagnostics, actual same-owner receiving paths, exact/short/zero and sticky refusal, actual allocation/backing and iterative Drop, current source custody, generated artifact pins, and the existing unchanged runtime fixtures.

## Explicit unfunded Legacy sharing successor (source only)

Successful ordinary Legacy construction now performs a separate post-validation
storage pass while retaining its complete old checking/error order. Immutable
payloads may share a private LegacyUnfunded owner; is_shared still denotes only
explicitly acquired Funded owners. Legacy direct/mutable-iteration writes detach
complete Owned values (unique move or iterative copy), preserving index/sibling
isolation and the old iterative disposal fallback. Funded implicit mutation stays
forbidden. Cache decode and public Vec conversion remain Owned. No new public enum
variant, payer discovery, controlled Legacy budget refusal, forecast discount or
application recovery is claimed. Complete old/current owners and allocation/Drop
observations are pending execution; source custody and scope are recorded under
`docs/evidence/legacy-nominal-sharing-827/`.

## Legacy final-link coordinate successor (source only)

After the existing complete Phase-A case and field checks, final linking now
retains an actual Legacy outer owner for its case-field coordinate too. This
physical storage decision grants no funded provenance, byte payer or source
authority. Funded coordinates remain unchanged; decoded/hostile Owned payloads
retain their complete independent field copies. Legacy index/HIR mutation still
detaches complete Owned values. Removing an eager field copy may move copying
to a later host mutation; no universal mutation/allocation or application-fit
claim follows. The immediate full prior index and finalizer are independently
compiled controls, alongside the older complete original linker.

## Legacy detached-field disposal successor (source only)

The final linked Legacy coordinate still detaches a complete independent Owned
field vector on host mutation. A no-workspace declaration index now disposes
each recursive field root to completion by reusing its already-owned nominal
argument Vec as the iterative continuation. Empty-parameter function spines
move their result directly. No cloned value, new owner, source authority, byte
allowance or refund is introduced. Funded/immutable shared last owners keep
their original acquired disposal scratch. Every adopted or pending vector is
fully drained before normal destruction; no recursive type subtree is left to
ordinary Drop glue. Arbitrary wider Legacy frontiers may still grow ordinary
unfunded scratch, just as this compatibility route did before. There is no
universal zero-allocation/funding claim.

This responds to actual828f's unchanged 512-depth detached-case owner observing
192 bytes of new Drop allocation. The old empty Vec's minimum four-type backing
is source-consistent with that delta, not a new runtime attribution receipt.
The unchanged original owner retains both disposal orders, all case coordinates,
detached fields, depth512 and its zero-allocation assertion. A complete previous
Drop body compiles independently through only a consuming-receiver/signature
adapter. Corrective tests additionally cover multiple roots, function spines,
branching settlement and unchanged sticky ledger state. All await execution.

## Independent legacy constructor compatibility

The active Legacy identity-payer route retains the complete original nominal
constructor, including checking and allocation order. The funded source-cell
route uses a separately named constructor with its existing debit and rollback
behavior. Neither route lends funded provenance to the other. The immutable
legacy source audit still checks the entire original constructor tail; the
funded implementation and its helper remain in the same audited source file.
