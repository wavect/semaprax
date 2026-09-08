# Generic Argument Inference v1

Status: local, partial. Seven source/HIR/graph checks, exact private ProgramRoot
replay, and all-eight-scalar interpreter/native O0/O2/Core-Wasm success and
contract-failure settlement pass. No hosted or public-support claim.

Audience: compiler contributors and reviewers.

This additive internal profile permits a monomorphic caller to omit the entire
explicit type-argument vector for an already admitted generic function with one
type parameter. The compiler derives that parameter from argument types, never
from a requested return type, target layout, or ownership equivalence.

Every argument must provide evidence through a typed scalar literal, an existing
local or parameter binding, or an explicitly typed record or variant constructor.
Calls, projections, operators, blocks, matches, and conditional expressions in
argument position retain the explicit-vector requirement in this version. Binding
such a checked expression to a local makes its exact type available. This
restriction avoids speculative evaluation and does not change ordinary local
binding inference. Constructor type arguments remain explicit.

Unification compares the entire declared parameter type with the corresponding
argument type. Nominal declarations and ordered argument positions must agree;
only the callee's sole scoped type parameter is a variable. Every occurrence must
resolve to the same one of the eight admitted Copy scalars. An absent binding,
conflicting observations, mismatched nominal identities, a second generic
parameter, or an unsupported evidence expression rejects with SPX-T225. The
existing generic function's substitution profile must independently admit the
inferred vector. This profile does not widen generic bodies or scalar domains.

Inference observes types and performs no moves, loan activation, expression
checking, or runtime evaluation. Ordinary verification then checks each argument
exactly once in left-to-right order and retains the owned call's existing commit
boundary. A moved local can supply a type fact but still fails ordinary ownership
checking; type evidence grants no right to read or transfer it.

Source checking and HIR resolution derive the vector independently. HIR unifies
resolved declaration identities and scoped parameter owners. The resulting call
contains the same explicit concrete type vector and instance identity as the
corresponding explicitly written call. Materialization, cleanup replay, graph
validation, and backend admission remain unchanged. Canonical source retains the
authored omission; graph and ProgramRoot continue binding exact checked source.

This is neither general inference nor generic constraints. Generic callers,
partial explicit vectors, return-context inference, overload selection, generic
methods, and compiler intrinsic inference remain outside this profile. Public
Project, package, native, and component signatures remain unchanged.
