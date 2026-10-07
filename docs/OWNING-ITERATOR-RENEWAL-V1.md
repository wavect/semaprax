# Owning Iterator Renewal v1

Status: implemented private renewal profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).

Audience: compiler contributors, reviewers, and agent authors.

This profile allows conditional same-owner renewal of a mutable `Vec<T>` in
an `If` branch of an authenticated consuming `for own` loop. It covers the
compiler-owned intrinsic assignment `output = vec_push<T>(output, item)` when
checked ownership facts
prove that `output` is the same owner on both sides of the assignment.

## Renewal boundary

Before evaluating the right-hand side, the loop reserves `output`'s existing
cleanup position. The RHS then stages its arguments left to right and may
return a replacement owner. On failure, the selected status is sticky and the
actual staged owner remains governed by cleanup; the old output is never
revived or published as a fallback. On success, the replacement resource is
returned to the reserved cleanup position, preserving one live owner.

The Vec rule applies to the admitted compiler-owned intrinsics in that branch;
other operation calls and unconditional Vec assignments do not select renewal.
A complete ordinary binding transfer still creates a new ownership epoch under
[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md); renewal is the
reserved-position case and does not erase that distinction.

## Authored flat-record renewal

The same reserved-position proof also admits an unconditional assignment in a
bounded `while` body when all of these facts are checked independently in
source, retained HIR, cleanup construction, and cleanup replay:

- the mutable local is an owned, monomorphic, explicit-identity record with
  exactly one `Bytes` field, one `usize` field, and explicit field identities;
- an effect-free, monomorphic call consumes that whole record exactly once and
  returns the identical record type;
- every other owned parameter is absent, and any borrowed record input has the
  same exact cursor shape and is a whole named place; and
- ordinary ownership validation proves that no outstanding view, stale alias,
  or duplicate argument survives the owning update.

This admits the existing source-authored `std.io` Reader and Writer transitions
without trusting their names or a function allowlist. Scalar observers may
borrow a cursor within the iteration. A checked transition failure keeps the
selected status, publishes no replacement epoch, and uses the same canonical
cleanup reservation to settle all staged owners.

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

This profile applies to the closed compiler-owned scalar `Vec<T>` rule and the
exact authored flat-record rule above. It does not add public ABI, lazy
adapters, owned collection payloads, effectful renewal, general record update,
or general conditional ownership. Source and HIR authenticate the admitted
ownership shape. Cleanup construction and independent replay agree on the
reserved position and renewal boundary before backends consume the checked
plan.

The `iterator` library selector exercises independent renewal replay, omitted
reservations, ordinary-transfer substitutions, and v12/v40 downgrades. The
owned-data `iterator_operations` selector exercises interpreter, C11 O0/O2,
and Core Wasm for all 64 scalar pairs, conditional capacity failure, and
callback failure after staging the output owner. Repeated runs verify exact
allocation settlement. [Generic Iterator Operations v1](GENERIC-ITERATOR-OPERATIONS-V1.md)
records the shared corpus and Linux selector. The implemented release corpus
is hosted green; historical local results retain their original scope. The
additive Reader/Writer evidence lives in the `std.io.lines` all-backend and
contract-failure selectors recorded by [IO Lines v1](IO-LINES-V1.md).
