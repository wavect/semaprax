# Projected Owned-Bytes Synchronous Borrowed Call v1

Status: Partial; additive implementation and focused evidence are authored but
unpromoted.

Audience: compiler, ownership-verifier, backend, and evidence maintainers.

This specification repairs the execution boundary for the already admitted
internal `borrow Bytes` parameter and adds one direct projected origin. Source
HIR and Shared Loan Plan v1 already distinguish a synchronous shared borrow
from ownership transfer; the interpreter, native, and Core-Wasm lanes must
preserve that distinction exactly.

## Closed source profile

The callee is a synchronous, monomorphic, source-defined function with one or
more parameters. An affected parameter has exactly the type and mode
`borrow Bytes`. Its corresponding argument is exactly one of:

- an unprojected named `Bytes` place in `own` or `borrow` mode; or
- one direct stable-ID `Bytes` field of an unprojected named `own` local whose
  type is the flat monomorphic [Owned Byte Record Algebra v1](OWNED-BYTE-RECORD-ALGEBRA-V1.md)
  profile.

The projected form contains exactly one field projection. The source verifier
resolves the display name, while HIR, LoanPlan replay, and every backend retain
the root identity and stable field declaration ID.

Temporaries, constructors, call results, borrowed-record roots, additional or
variant projections, generic calls, escaping borrows, callbacks, task/async
boundaries, FFI, public Project parameters, and Component/public ABI boundaries
remain rejected.

## Loan and call semantics

The borrow starts before evaluation crosses the complete call boundary and
ends after that call returns or fails. Its canonical origin is the complete
`Place`, including the direct stable field projection when present. A later
argument may move an independent sibling field, but it may not move or mutate
the same field, an ancestor owner, or a descendant while the loan is live.

The callee receives a read-only alias to existing storage. No owner is minted,
no cleanup epoch is created, no liveness flag transfers, and no caller slot is
tombstoned. Returning or failing the call ends the alias without settling the
owner. The original owner remains available subject to ordinary LoanPlan and
CleanupPlan rules. CleanupPlan remains the sole runtime cleanup authority and
continues to create call transitions only for `own` parameters.

For an owned root, Shared Loan Plan v1 independently replays the exact
`BorrowedCall` cause, origin, start, endpoint, and path edges. A forwarded
borrowed parameter root already carries its enclosing borrowed provenance and
intentionally creates no redundant call-loan draft. HIR validation does not
trust the source verifier or serialized plan: it reauthenticates the root
type/mode, projection count, field owner/type/identity, callee parameter mode,
and monomorphic synchronous call shape, and requires the canonical
`BorrowedCall` identity whenever the origin is owned.

## Backend contract

- The reference interpreter stages an alias through authenticated place lookup
  and never executes the owned-place transfer path.
- Native C11 represents an internal borrowed-Bytes parameter as
  `const spx_bytes_v1 *`. Only `own Bytes` parameters receive owned cleanup
  slots or `spx_bytes_move` staging.
- Core Wasm forwards the existing logical carrier token. It does not create a
  call epoch, mint an owner, change a live flag, or schedule a drop.

Existing owned-argument left-to-right staging and atomic commit semantics are
unchanged. Native public ABI, Wasm public exports, Graph schemas, Project
schemas, and cleanup schemas are unchanged.

## Diagnostics

- `SPX-T266` rejects a temporary, borrowed-record projection, deeper/foreign
  projection, or otherwise unauthenticated borrowed-Bytes argument.
- `SPX-T265` rejects an overlapping move or mutation during the call-scoped
  loan.
- `SPX-H006` rejects hostile HIR place/type/field identity or LoanPlan replay
  disagreement.
- Existing `SPX-B103` and `SPX-W110` backend boundaries reject a carrier shape
  that bypasses validated HIR.

## Evidence contract

Focused evidence must cover named owned roots, forwarded borrowed roots, the
direct owned-record field, display-only field rename, sibling movement in a
later argument, and owner/field reuse after success and admitted failure. It
must reject same-field/ancestor overlap, temporaries, constructor or call
results, borrowed-record roots, deeper projections, generic calls, and forged
field identities.

The proof corpus must compare exact projected `BorrowedCall` LoanPlan facts and
hostile replay mutations, then execute identical behavior through interpreter,
native `-O0`/`-O2`, and Node/Core-Wasm with repeated entry. Native generated C
must contain no borrowed-boundary `spx_bytes_move`; tight Wasm owner capacity
must prove the borrow mints and drops nothing. Existing owned-call cleanup
traces, Graph v23/v24 selection, and artifact KATs remain preservation gates.

Authored or local focused evidence does not promote hosted, public ABI,
cross-task, mutable, escaping, nested aggregate, resource, or Component support.
