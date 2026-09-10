# Owning Iterator Renewal v1

Status: implemented private renewal profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).

Audience: compiler contributors, reviewers, and agent authors.

This profile defines conditional same-owner renewal for a mutable `Vec<T>`
binding inside an `If` branch of an authenticated consuming `for own` loop. It
covers the compiler-owned intrinsic assignment
`output = vec_push<T>(output, item)` when the loop's checked ownership facts
prove that `output` is the same owner on both sides of the assignment.

## Renewal boundary

Before evaluating the right-hand side, the loop reserves `output`'s existing
cleanup position. The RHS then stages its arguments left to right and may
return a replacement owner. On failure, the selected status is sticky and the
actual staged owner remains governed by cleanup; the old output is never
revived or published as a fallback. On success, the replacement resource is
returned to the reserved cleanup position, preserving one live owner.

The rule applies to the admitted compiler-owned Vec intrinsics in that branch;
arbitrary operation calls and unconditional assignments do not select renewal.
A complete ordinary binding transfer still creates a new ownership epoch under
[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md); renewal is the
reserved-position case and does not erase that distinction.

## Proof facts and projections

`CleanupPlan v12` adds `ReserveRenewal(at, binding)` before the RHS and
`Renew(at, source, destination)` at the successful replacement boundary.
These facts bind the exact loop location, source owner, destination binding,
and reserved cleanup position. Independent replay authenticates both facts
and rejects missing, reordered, aliased, or forged renewal edges.

`Graph v40` binds the selected v12 plan and the exact renewal facts. Existing
CleanupPlan v11, Graph v39, unconditional loop paths, and earlier source,
cache, and graph bytes remain frozen. Downstream consumers do not sort or
repair renewal facts, and the proof grants no extra runtime permission,
allocation authority, or hidden operation.

## Scope and evidence boundary

This profile applies only to compiler-owned scalar `Vec<T>` values, admitted
compiler Vec intrinsics, and an `If` branch inside an authenticated consuming
`for own` loop. It does not add public ABI, lazy adapters, owned payloads,
arbitrary operation-call renewal, unconditional renewal, or general
conditional ownership. Source and HIR authenticate the admitted ownership
shape. Cleanup construction and independent replay agree on the reserved
position and renewal boundary before backends consume the checked plan.

The `iterator` library selector exercises independent renewal replay, omitted
reservations, ordinary-transfer substitutions, and v12/v40 downgrades. The
owned-data `iterator_operations` selector exercises interpreter, C11 O0/O2,
and Core Wasm for all 64 scalar pairs, conditional capacity failure, and
callback failure after staging the output owner. Repeated runs verify exact
allocation settlement. [Generic Iterator Operations v1](GENERIC-ITERATOR-OPERATIONS-V1.md)
records the shared corpus and Linux selector. The implemented release corpus
is hosted green; historical local results retain their original scope.
