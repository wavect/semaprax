# Scalar Snapshot Closures v2: generic construction and loops

Status: locally exercised across source/HIR/graph/cache replay, ProgramRoot,
interpreter, native C11 O0/O2, and Core Wasm. Hosted promotion remains pending.

This additive profile extends [Closures v1](CLOSURES-V1.md) to construction
inside private generic collection functions and bounded loop bodies. It keeps
the same scalar snapshot semantics and private callable representation.

## Generic construction

The admitted single-parameter generic compiler-collection profile may construct
a closure whose explicit parameter, result, local, and capture types are Copy
scalars or the enclosing declaration's own type parameter. Source verification
checks the existing eight scalar substitutions; no owning substitution or
unconstrained callable value is admitted by spelling a type parameter.

For example, a generic collection function may construct
`fn(item: T) -> T { replacement }`, capturing its scalar `replacement: T`
parameter, and invoke that callback for each element. The capture is evaluated
at construction, independently of later changes to the surrounding bindings.

The retained template describes scoped symbolic types. Materialization
substitutes the complete ordered concrete argument vector and rebuilds the
creation expression identity under that exact generic function instance.
The private body identity, parameters, result, local values, and expression
identities derive from the resulting concrete creation site. Two substitutions
therefore cannot share a closure body just because their authored source span
or template path is equal. Repeated calls selecting the same concrete instance
retain one callable identity and create independent snapshot values.

Concrete HIR validation independently checks the materialized closure using
the ordinary v1 rules. Altered symbolic owners, concrete capture types, private
parameter identities, or a body copied from another instance must reject before
interpretation or target lowering. Cached products and external graph bytes
cannot supply substitution or cleanup authority.

## Construction in loops

A bounded `while` body may construct and invoke scalar closures. Each executed
construction reads the current values once. Updating an outer scalar after
construction does not change that iteration's captured value; the next
iteration constructs a fresh snapshot. An empty loop constructs nothing.

The loop still obeys its ordinary ownership, fuel, and failure rules. Closure
construction has no owning environment and does not execute its body. Ordinary
called helpers may fail; invocation retains their selected status and the
enclosing function settles live collection owners through its canonical plan.

## Projections and compatibility

Source formatting preserves closure syntax inside the template and loop.
Graph and ProgramRoot retain checked symbolic source meaning separately from
the concrete executable closure inventory. Unused templates create no runtime
table entry or executable body. Concrete capture and body facts remain bound
to the same instance observed by interpreter, native C11, and Core Wasm.

An otherwise unchanged Graph v37 gains `template_closure_definitions` only
when an admitted generic template contains a closure. Each source-only entry
binds the template identity, symbolic creation identity, derived private
target, signature, parameters, captures, and checked body. Existing v37 bytes
without such a template remain unchanged. These entries carry no cleanup plan,
runtime table position, or executable authority; concrete closure definitions
remain derived only after materialization.

Existing AST/HIR closure carrier tags, scalar environment layouts, prelude,
host imports, cleanup schemas, and public descriptors remain unchanged. This
profile does not admit owning captures, capturing another callable, nested
anonymous closures, public callable ABI, or a consuming iterator protocol.

## Required evidence

The focused corpus must cover all eight concrete scalar substitutions,
distinct substitutions in one program, repeated loop construction with later
outer mutation, empty traversal, helper failure with live input/output owners,
exact source/HIR cache and graph replay, unused-template ProgramRoot retention,
and hostile cross-instance identities and scoped capture types. Runtime checks
must execute interpreter, C11 O0/O2, and Core Wasm with repeated owner settlement.
Local passage and exact-commit hosted promotion are separate evidence.

Focused local evidence uses `generic_closure` in the library, language,
workspace, and owned-data harnesses. It includes independent symbolic and
concrete HIR hostility, mutable annotated scalar locals, capture snapshots
checked separately on consecutive iterations, all eight scalar substitutions
in one module, source-only template body changes, and repeated native/Wasm
allocation settlement on success and checked helper failure.
