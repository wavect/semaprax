# Function Values v1

Status: implementation in progress; no hosted or public ABI support claim.

Audience: language users, compiler contributors, backend implementers, and
workspace-service authors.

This additive LANG-07 foundation supplies noncapturing function values for
subsequent closures and iterator adapters. It does not complete those milestones.

## Source and eligibility

The structural type is `fn(T0, T1) -> R`, including `fn() -> R`.
A signature has zero through eight value parameters and one result. Each leaf
is one of `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, or `bool`.
Function values are Copy and carry no captured environment or cleanup owner.
Pointer equality is not a source operation; callable equality rejects with
`SPX-T207`. Unsupported callable signatures reject with `SPX-T287`.

A bare name first resolves a lexical binding, then an eligible local function.
An eligible reference identifies an ordinary monomorphic, effect-free function
with the scalar signature above. Import, generic, method and capturing targets
are excluded. Private monomorphic helpers may receive or return function values;
such higher-order helpers are not themselves eligible scalar function targets.
Function types in public descriptors, stored record fields and generic
substitutions remain outside this version.

```semaprax
module example.function_values;
@id("example.increment") fn increment(value:i64)->i64 { value + 1 }
@id("example.decrement") fn decrement(value:i64)->i64 { value - 1 }
@id("example.apply") fn apply(callback:fn(i64)->i64, value:i64)->i64 {
    callback(value)
}
@id("example.main") fn main()->i64 {
    let callback = if true { increment } else { decrement };
    apply(callback, 41)
}
```

Invoking a callable binding evaluates its value before arguments, then evaluates
arguments once in left-to-right order. Argument types and result type exactly
match the callable signature. Invocation uses the selected target's ordinary
contracts, checked arithmetic and failure status. Failure remains sticky and
result publication follows postconditions and cleanup.

## Checked meaning and graph

HIR retains `FunctionReference { target }` and `Invoke { callable, args }`.
The target is a declaration identity, not a display name, source offset or
backend address. Independent HIR validation recomputes eligibility and exact
signature compatibility before execution or graph projection.

The target universe consists of declarations actually retained by reference
expressions, ordered by declaration identity, with at most 256 distinct targets.
An invocation's graph candidate edges describe every signature-compatible target
in that universe. These are conservative possible edges, not an exact flow
analysis. Reference and invocation dependencies participate in checked closure
and cycle analysis; a callback name must never be mistaken for an unrelated
same-named global direct call.

Graph v36 adds callable types, reference identities, indirect operands and
candidate edges. Earlier graph versions and programs without function values
retain their previous bytes. Submitted projections must replay against retained
checked source/HIR; reminting a digest does not authorize a forged target or
signature. ProgramRoot associations use those same checked graph facts.

## Retained workspace roots

A retained workspace containing function values uses semantic-program v3 and its
v3 digest domain. Its `checked_callable_closures` contains the independently
validated graph for each callable or generic-bearing entry, public API, or tests
role. A role combining generics and callables has one Graph v36 closure containing
both sets of facts. ProgramRoot binds these graphs through the semantic-program
digest; exact-source replay also binds SourceProjection. A changed target cannot
be paired with the prior root. Workspaces without function values preserve the
semantic-program v1/v2 schemas and bytes. This adds internal body support without
admitting function-valued public workspace signatures.

## Runtime representation

The interpreter retains the checked target identity and resolves it through its
admitted function closure. Native lowering uses signature-specific C function
pointer types with the existing context, status and output-result ABI. Wasm
lowering uses a deterministic funcref table and typed `call_indirect`.
Backend addresses and table indexes are projections, never semantic identities.
Legacy Wasm modules without callable features do not gain a table. A module
with an unused callback helper may require an empty table for validation even
when it contains no reference targets.

No engine may replace indirect selection with a source-name rewrite, weaken
contract checks, reinterpret a signature, or add ambient host authority.

## Required evidence

The focused corpus must cover all eight scalar leaves, zero/eight parameter
bounds, dynamic target choice, copying/passing/returning callable values,
shadowing, nested invocation, contract and argument failures, exact deterministic
source/graph/Wasm round trips, and repeated interpreter/C11 O0/O2/Wasm execution.
Hostile coverage must reject unknown or ineligible references, altered signatures,
forged graph targets/candidates, excess target count, public callable boundaries,
and unsupported generic or owned signatures. Frozen graph and backend fixtures
remain preservation evidence. Completion status changes only after these
executable checks pass; hosted claims require the exact published commit.

## Batch verification status

The combined v1/v2 language selector passes 15 local cases, including required
C11 O0/O2 and Node execution, the signed table-index boundary, lexical binding
precedence, generic template hostility, and owned-export argument snapshots.
Two additional adapter runtime corpora pass on interpreter, native O0/O2 and
Core Wasm, checking all eight scalar types, empty input, selection/fold order,
callback failure and balanced owner settlement. Eight library hostile cases and four workspace checks pass, including v3
ordinary/mixed callable bodies and v4 retained unused generic callback templates.
Comment-only edits preserve semantic identity while changing the exact-source
ProgramRoot.

The flat 65-target source corpus passes without stack overflow; the supplied
deep source reproductions reject with the documented SPX-P207 nesting bound.
Full gates and hosted validation have not been run for this batch. Captures,
iterator interfaces and public callable ABI remain unfinished. See
[Function Values v2](FUNCTION-VALUES-V2.md) for the private generic callback
profile and its remaining boundaries.
