# Projected Owned-Byte Field Shared Borrow v1

Status: Partial, authored and intentionally unrun in this implementation tranche.

Audience: compiler, verifier, backend, and evidence maintainers.

This specification admits one additive ownership profile on top of
[Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md): `bytes_as_slice` may borrow one
direct authenticated `Bytes` field from an exact named `own` local whose type
is the flat, monomorphic Owned Byte Record v1 profile.

## Closed admission

The admitted source has exactly this shape:

```semaprax
let view = bytes_as_slice(packet.payload);
```

Admission requires all of the following:

- `packet` is an exact named place with `own` mode;
- its resolved type is one flat, monomorphic Owned Byte Record v1 record;
- `payload` resolves to one direct field of type `Bytes` with owned field
  ownership;
- the operation is exactly `bytes_as_slice`; and
- the place has exactly one projection.

The source verifier resolves the display field name, while HIR, Shared Loan
Plan v1, replay, additive Graph v24, and every backend retain the stable field
declaration ID. Graph v23 retains its unprojected schema selection and fields.
Constructors, call results, temporaries, borrowed roots,
additional projections, variants, generics, resources, and other byte-view
operations remain rejected.

## Loan and move rules

The canonical loan origin is the complete `Place`: owner `ValueId` plus the
one stable field-ID projection. Alias and range reborrows preserve that place
rather than collapsing it to the record root. A move conflicts exactly when
its place is an ancestor or descendant of a live loan origin. Therefore:

- moving the borrowed field while the view remains live is rejected;
- moving the parent record while the view remains live is rejected;
- moving a sibling field is independent; and
- either the field or parent may move after the view's path-exact last use.

Independent Shared Loan Plan and byte-slice provenance replay authenticate the
exact projected place and resolved `Bytes` field type. Direct aliases and
`byte_range` reborrows retain the same root and stable projection vector.
Changing the root, deleting or replacing the field ID, or adding another
projection invalidates HIR with `SPX-H006`.

## Runtime and cleanup

The interpreter, native C11 emitter, and Core-Wasm emitter lower only the
admitted direct field view. They borrow the field's existing byte storage and
do not create a second owner. The ordinary cleanup inventory and CleanupPlan
remain authoritative; this feature adds no cleanup action, schema, or
destructor. The live-loan gate prevents the field or containing owner from
being destroyed while the view can still be used. Backends fail closed on a
projected borrow outside this profile.

This is an internal language feature only. Additive Graph v24 does not
reinterpret Graph v23. It adds no public borrowed ABI, Component Model claim,
syntax, Cleanup schema, ambient authority, dependency, or unsafe code.

## Diagnostics

- `SPX-T266` rejects a byte view outside the closed source profile.
- `SPX-T265` rejects a move that overlaps a live projected loan.
- `SPX-H006` rejects inconsistent resolved-HIR or replayed loan provenance.
- existing backend unsupported-profile diagnostics remain the fail-closed
  boundary for a carrier that evades ordinary validated-HIR entry.

## Evidence inventory

The authored evidence covers direct admission; constructor, temporary,
borrowed-root, deeper, and substituted-projection rejection; same-field and
parent move, assignment, record update, and `match own` rejection; sibling
field movement and assignment independence; post-last-use movement; stable-ID
retention through direct aliases/ranges and across a display rename; hostile
Shared Loan Plan and byte-slice provenance mutations; legacy borrowed `Bytes`,
string and slice call preservation; unchanged unprojected Graph v23 schema
selection and serialized fields; and equivalent interpreter, native
`-O0`/`-O2`, and Core-Wasm execution while retaining the
existing multiple-view and reborrow corpus.

Those checks were authored but deliberately not executed in this tranche.
Consequently the completion-matrix rows remain `Partial`, and this document
does not promote any hosted-CI, portability, sanitizer, or release claim.

## Non-claims

This specification does not admit general nested aggregate borrowing, mutable
borrows, escaping borrows, public borrowed calls, fields reached through
variants or multiple projections, generic aggregates, resources, constructors
or temporaries, or a widening of Shared Loan Plan v1 beyond this exact profile.
Deeper byte-field projections are additionally impossible in the current
language because Owned Byte Record v1 rejects nested owned-byte fields before
loan inference.
