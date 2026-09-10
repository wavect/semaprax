# Generic Owned Record Composition v2

Status: implemented source, HIR, cleanup, graph and backend integration;
**HOSTED GREEN** under the
[accepted v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
This remains internal function semantics, not a new public ABI.

Historical local witness: the seven focused language checks and all-eight-scalar
runtime corpus passed. Their counts and timings retain their original local
scope rather than becoming a new test run.

Audience: compiler contributors and language reviewers.

## Checked expression composition

A generic owned-record function with one admitted owning record parameter and
an admitted owning record result may compose record construction, recursive
owning or borrowing record matches, Copy-field projection, record update,
blocks, branches and generic calls. Existing generic scalar parameters and
substitution bounds remain unchanged. Every materialized expression must also
satisfy the ordinary concrete ownership rules.

Whether an expression is a top-level binding, a branch, a call argument or
another record field does not grant or remove ownership authority. A source
variable's spelling and its identity as the original function parameter are
not admission rules. Checked aliases can serve as update bases wherever the
ordinary update profile admits them.

Complete nominal input and result types remain explicit. Reconstruction may
move checked owning leaves into a different declared record shape, including
nested wrappers, when the authored result has exactly its declared type. This
is source reconstruction, not a coercion between ownership-equivalent layouts.
Equal byte size, leaf count or cleanup shape cannot justify a type conversion.
Every owned leaf transfers at most once. Unused owned bindings follow ordinary
cleanup; ordinary restrictions on pattern wildcards remain in force. Borrowed
values cannot become owners or escape.

Every admitted caller substitution is checked, including unused templates.
The eight Copy substitutions, one/two type-parameter bound, depth, field,
instance and layout bounds remain in effect. Source independently checks the
specialized programs. HIR validates symbolic types and identities, every
materialized body, and independently replayed inventory, cleanup and loan plans.

Nested matches bind exact recursively declared fields. Constructors and updates
use canonical declaration field order after resolution. Missing, duplicate or
foreign fields, wrong generic argument order, repeated owned bindings, stale
loans and wrong return types reject. Source constructor move tracking uses the
substituted field type, so a declared field `A` instantiated as `Bytes` consumes
its owned value and a second use rejects with `SPX-O101`.

## Projection and execution

Graph v34 represents identity-forwarded concrete instances using complete body,
inventory, owning-leaf shape and replay-selected cleanup schema. Graph v35 is
selected by nonidentity symbolic forwarding. Nested destructure and update
select existing cleanup v8/v9 when those operations require them; nominal
nesting alone does not determine the plan version. Exact source graph replay
rejects altered fields, types, ownership and plan facts.

An owning match result is associated with its exact arm result and its own
expression type. Its scrutinee and pattern independently agree on their exact
input type. Neither association requires the internal expression type to equal
the enclosing function's return type. This permits an owning match inside a
constructor field while preserving full nominal typing.

C11, Core Wasm and the interpreter use the checked transfers and ordinary
cleanup behavior. Native reconstruction copies scalar fields recursively
inside owning subrecords as well as transferring owning leaves. Cleanup
construction and independent replay preserve arm-result transfer before scope
cleanup. Public C/C++/Rust, package, WIT and Component signatures retain their
existing admission rules.

## Implementation ownership

`source_verify/declared_type/generic_composition.rs` selects structural record
expression grammar. `declared_type.rs` authenticates the owning signature;
iterative and oracle record-match validation independently admit exact owning
and Copy expression results. Both constructor validators use substituted field
types when deciding whether a value moves.

`hir/validation/generic_record_composition.rs` authenticates symbolic record
fields, recursive patterns, binding identities, scopes and expression result
types. `generic_template.rs` and `type_profiles.rs` select the owning profile
and retain exhaustive materialized validation. `hir/monomorphize.rs` recursively
substitutes pattern types and remints each binding at its structural path.
`hir.rs` authenticates a concrete function against its complete materialized
template meaning before cleanup or backend generic-match admission.

Cleanup replay owns its independent match-result shape check in
`cleanup_plan/replay/record_destructure.rs`; construction and its recursive
reference preserve the same transfer order. Interpreter, native and Wasm
admission independently check the instantiated expression shape. No renderer
or backend repairs a submitted plan or infers ownership from layout equality.

## Focused evidence

The registered language module `generic_record_composition_next` covers all
eight Copy instantiations of recursive reconstruction, branch/call-argument
composition, changed nominal wrapper, nested borrowed Copy observation, alias
update selecting cleanup v9, and a match inside a constructor field. It also
covers canonical round trips, graph replay, forged HIR field identities and
unused-template duplicate-owner rejection (`SPX-O101`).

The owned-data runtime module `generic_owned_function_runtime::nested_composition`
compares interpreter, C11 O0/O2 and Core Wasm behavior and resource accounting
for both branches, reconstruction, observation, updates and contract failure
after reconstruction. In the historical local witness, the three runtime
profiles passed on all four engine configurations (27.23 seconds), with repeated
invocations, allocation settlement and no additional Wasm memory.copy compared
with direct construction. Nested update classification substitutes concrete
fields independently in construction and replay. Native If lowering emits the
selected canonical ownership join and its call-argument staging before invocation.
Current release evidence follows the accepted baseline above; the historical
duration is not a hosted performance claim or evidence for later code changes.

[Generic Multi-Owner Records v1](GENERIC-MULTI-OWNER-RECORDS-V1.md) extends the
owning-parameter composition separately. It does not retroactively widen this
version's stated one-owner profile or any public signature.
