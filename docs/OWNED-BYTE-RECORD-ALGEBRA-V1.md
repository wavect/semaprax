# Owned Byte Record Algebra v1

Status: local implementation tranche; hosted promotion is not claimed.

## Purpose

Owned Byte Record Algebra v1 admits the first non-Copy record execution path
whose ownership is derived from ordinary record structure. It is deliberately
limited to flat monomorphic records with one or more direct `Bytes` fields and
zero or more direct Copy-scalar fields. It does not create a public aggregate
ABI or widen variants, generics, components, Project exports, callable
interfaces, or native Rust interoperability.

## Closed admission

An admitted record:

- is monomorphic;
- contains at least one direct `Bytes` field;
- contains only direct `Bytes` or already admitted Copy-scalar fields;
- has no nested owned-byte record, variant, resource, array, slice, string, or
  generic field; and
- remains outside every public parameter and result boundary in this tranche.

The exact irrefutable record pattern must list every declared field once. A
plain `match` remains the existing Copy-only operation. A non-Copy record must
select exactly one explicit mode:

```semaprax
match own packet {
    Packet { payload, marker } => marker,
}

match borrow packet {
    Packet { payload, marker } => marker,
}
```

`match own` consumes one owned record value and transfers every direct owned
field into its exact arm binding. Every droppable field must be bound; wildcard
discard is rejected. `match borrow` accepts one named owned or borrowed record
place, creates arm-scoped borrowed bindings, transfers no cleanup epoch, and
leaves the owner available after the arm. Explicit modes on Copy records and
top-level wildcard explicit matches are rejected with `SPX-O117`. A plain
match on a non-Copy record remains `SPX-O111`.

## Ownership representation

Type facts distinguish `needs_drop` from user-resource containment. `Bytes`
makes its enclosing record non-Copy and drop-aware without pretending that the
record contains an authored resource lifecycle.

Cleanup inventory and CleanupPlan v5 address direct byte fields as projected
places:

```text
StorageId(record epoch) / FieldId
```

Each projected leaf has one compiler-owned Bytes lifecycle and one liveness
flag. `match own` contains an exact projected `Transfer` for every such leaf
into the corresponding arm binding. The source flags become dead at that
transition. The arm child region settles the binding leaves in canonical
field order. `match borrow` creates the arm region but no transfer and leaves
the source flags unchanged.

Independent replay derives schema selection, record inventory, binding
ownership, projected transfers, arm-region settlement, and finalizer order
from authenticated HIR rather than trusting the emitted plan. Replay rejects
schema substitution, forged modes or bindings, wildcard-owned fields,
projection drift, moved storage, missing transitions, and missing finalizers.
Legacy programs retain their prior CleanupPlan schemas and exact output.

## Executable backends

The reference interpreter stores fields by persistent declaration identity,
not source display name. Owned matching uniquely consumes the record carrier;
borrowed matching uses an arm-scoped alias that must settle before a later
owned use.

Native C11 and Core Wasm use the existing compiler-owned Bytes carrier and
CleanupPlan liveness epochs. A record containing Bytes must never be copied by
`memcpy`, `memory.copy`, or a shallow aggregate assignment. Construction and
owned destructuring move each authenticated carrier independently and poison
the source carrier. Borrowed destructuring aliases without changing liveness.
Cleanup remains guarded and exact-once on success and every admitted failure
path.

## Versioned projections

- Graph v21 serializes explicit `own`/`borrow` match modes. A legacy Value
  match omits the field and preserves its earlier schema bytes.
- CleanupPlan v5 is selected only when an explicit ownership match occurs.
  Existing v2, v3, and v4 programs retain their prior selection and bytes.
- Consumers that have not explicitly admitted Graph v21 remain fail-closed.

## Evidence boundary

Completion requires source round-trip and diagnostics, hostile HIR and replay
mutation tests, interpreter execution, native C11 execution at `-O0` and
`-O2`, and Node/Core-Wasm execution. Runtime evidence must cover multiple
direct byte fields, borrow followed by own, repeated entry, exact-once cleanup,
and failure settlement. Local evidence does not claim hosted promotion or any
public ABI widening.
