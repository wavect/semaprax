# Public Generic Settlement Obligations v1

Status: implemented bounded projection with local evidence; the specification
half of gate PG-7 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
PG-7 stays open: there is no public generic boundary to execute, so nothing
here allocates, transfers, releases, or observes a runtime, and no engine
evidence is claimed. No hosted run is recorded, and public generic ownership
remains unsupported and unpublished.

Audience: ownership, cleanup, backend, and ABI reviewers.

## What this settles

A consumer that receives an owned generic instance has to know three things,
and none of them may be a boundary's invention:

1. which owned leaves it is accountable for, and in which order;
2. how each one is discharged; and
3. what happens to the ones already taken when a transfer fails part way.

The compiler already proves all three for the internal path. This module
derives those facts as a target-neutral plan and *binds* them to the checked
cleanup facts, so a future boundary cannot quietly diverge from the ownership
the compiler verified.

| Layer | Identifier |
| --- | --- |
| Settlement plan | `semaprax.public-generic-settlement-plan.v1` |
| Type spelling | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) |

## The three bindings

A plan is derived for one `own` parameter whose type is an admitted instance
with at least one owned leaf. Everything else is refused rather than described
with an empty plan (`SPX-PG501`).

**Structural order.** The grammar's transitive owned-leaf paths must equal the
cleanup inventory's leaf paths, in the same order. The inventory is structural
metadata — the leaf tree in declaration order — so this is where a
target-neutral projection and the compiler can be compared directly.

**Discharge.** The inventory carries one liveness flag per owned leaf, with
that leaf's exact projection chain and its drop lifecycle. Each obligation is
paired with its flag in flag order, and the flag's place must name the same
leaf. An obligation whose flag does not match it would have no stated way to
be discharged, so a relabelled flag is a refusal.

**The transfer unit.** The cleanup plan's entry state names the owned
parameter *whole* — exactly one live owned place, with no projections. That is
the unit a boundary receives or refuses. Reading the leaves out of the plan
instead would be inventing a per-leaf transfer the compiler does not perform,
so the plan requires the whole place and reports the parameter's own value
identity as the unit.

Every disagreement — a missing storage slot, a retyped slot, a differing leaf
count, a leaf path that differs from the grammar's, a relabelled flag, a
missing or projected live owned place — is `SPX-PG502`. Nothing is sorted,
padded, or repaired: an order that silently differed from the checked cleanup
order would be worse than no order at all.

## Failure settlement

Release order after a failed transfer is the exact reverse of the canonical
obligation order. This states, in the milestone's terms, what the repository's
invariants already require elsewhere: failure selection is sticky, so cleanup
cannot replace the selected status, and result publication follows
postconditions and non-result cleanup. A boundary may not publish a result on
the failure path, and may not renumber, sort, or repair the release order.

An authored variant shape is refused rather than flattened: a variant leaf is
live only for its authenticated case, so it has no unconditional owned-leaf
order. The grammar admits no variant, so reaching one is a disagreement.

## Nonclaims

This defines no calling convention, descriptor, carrier, allocator, or memory
representation; it allocates nothing, frees nothing, executes nothing, and
observes no engine. It is not evidence that any backend settles a public
generic boundary, because there is no such boundary: PG-7 requires bounded
allocation, exact copy-out, sticky failure selection, and canonical cleanup
order to be *exercised* on the interpreter, native C11, and Core Wasm across a
real boundary, and that remains open. It admits no public generic signature,
and the milestone's separation gate continues to prove the public projections
reject one.
