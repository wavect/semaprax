# Owned Byte Variant Algebra v1

Audience: language users, tool authors, and compiler contributors.

Status: local implementation tranche; hosted promotion is not claimed.

## Purpose

Owned Byte Variant Algebra v1 admits the first non-Copy sum execution path.
It is deliberately limited to flat monomorphic authored variants with at least
one direct `Bytes` field and to the exact compiler-owned `Option<Bytes>`,
`Result<Bytes, i64|bool>`, and `Result<i64|bool, Bytes>` instances. It does not
create a public aggregate ABI or widen generic authored variants, nesting,
postfix `?`, components, Project exports, callable interfaces, or native Rust
interoperability.

## Closed admission

An admitted authored variant:

- is monomorphic;
- contains at least one direct `Bytes` field;
- contains only direct `Bytes` or already admitted Copy-scalar fields; and
- contains no nested record or variant, resource, array, slice, string,
  generic field, or `Bytes` field on both sides of a generic `Result`.

Explicit owned and borrowed matching is exhaustive, guard-free, and lists
every case with its exact declared field inventory:

```semaprax
match own value {
    Choice::None {} => 0,
    Choice::Data { payload, marker } => marker,
}

match borrow value {
    Choice::None {} => 0,
    Choice::Data { payload, marker } => byte_len(bytes_as_slice(payload)),
}
```

`match own` consumes the active variant case and transfers every direct owned
field to its exact arm binding. An owned wildcard cannot conceal a payload.
`match borrow` accepts one unprojected named owned or borrowed place, creates
arm-scoped aliases, transfers no cleanup epoch, and leaves the owner available
after the arm. Explicit owned/borrowed arms return Copy scalars in v1.

## Conditional ownership representation

Only the authenticated active union case is live. Cleanup Inventory v2 stores
each leaf under a stable case-qualified path:

```text
StorageId(variant epoch) / CaseId / FieldId
```

CleanupPlan v6 represents a dynamic variant as a closed conditional case
group rather than marking every case live. `InitializeVariant` authenticates a
callee result tag and activates one case. `TransferVariant` maps one dynamic
case inventory to another without widening inactive authority.
`AuthenticateVariantCase` converts a conditional group to the selected case
before `match own` transfers its fields. Ordinary projected `Transfer` remains
the exact field move after selection. Payload-free and Copy-only cases remain
explicit members of the conditional case domain with an empty owned-leaf list;
their authenticated selection is carried by the tag and is never inferred
from the presence of a live cleanup flag.

Independent replay reconstructs the case domain, paths, conditional groups,
transitions, call commit, arm settlement, and finalizer order from HIR. It
rejects foreign tags or fields, forged modes or ownership, inactive-case
liveness, whole-union Copy operations, missing transitions, and cleanup drift.
Legacy CleanupPlan schemas remain byte-stable.

## Executable backends

The interpreter keys carriers by concrete type and stable variant, case, and
field identities. Owned matching uniquely consumes the active payload;
borrowed matching creates an arm-scoped alias.

Native C11 and Core Wasm validate the tag before any union payload access.
Construction evaluates fields left-to-right, moves only selected owned fields,
and publishes the tag last. Dynamic parameters, calls, and results move the
selected case field-by-field. Inactive fields must remain dead. Borrowed calls
carry authenticated byte-leaf aliases without transferring ownership. A
variant containing `Bytes` is never moved with a shallow aggregate assignment,
`memcpy`, or `memory.copy`; every exact cleanup remains flag-guarded on success
and admitted failure paths. Invalid tags and any tag/liveness disagreement are
backend invariant failures: native terminates and Core Wasm traps out of band
before reading or finalizing a payload or publishing a result. They are never
translated into an ordinary language failure status.

## Versioned projections

- Cleanup Inventory v2 introduces stable case-qualified conditional leaves.
- CleanupPlan v6 introduces conditional initialization, dynamic transfer, and
  selected-case authentication while preserving all earlier schemas.
- Graph v22 serializes the exact conditional inventory and transitions while
  preserving legacy graph bytes for programs outside this tranche.
- Consumers that have not explicitly admitted these versions remain
  fail-closed.

## Evidence boundary

Completion requires source round-trip and stable diagnostics, hostile HIR and
replay mutation tests, interpreter execution, native C11 execution at `-O0`
and `-O2`, and Node/Core-Wasm execution under tight owned-token capacity.
Evidence covers authored and compiler-owned cases, borrow followed by own,
dynamic parameters/results/calls, repeated entry, inactive cases, invalid
carriers, payload-free conditional cases, exact-once cleanup, and failure
settlement. Evidence in this tranche is local only; it does not claim hosted
promotion or a public ABI widening.
