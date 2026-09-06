# Generic Explicit Forwarding v1

Status: specified next implementation tranche; not admitted or verified by this
specification alone. No hosted support claim.

## Semantic scope

An explicit call from an admitted generic function may map each callee type
parameter to a caller parameter or an already admitted concrete type. The
ordered argument list may permute, repeat, omit caller parameters, or combine
caller parameters with concrete arguments. Callee arity must match exactly;
caller and callee arities need not match. Existing declaration parameter-count
limits remain unchanged.

For a caller with parameters `A, B`, mappings such as `<B, A>`, `<A, A>`,
`<A, i64>`, and `<Bytes, B>` are structurally meaningful. They are executable
only when every substitution admitted for that caller produces arguments
admitted by the callee and compatible value parameters, ownership modes,
contracts, and return type. These examples do not independently admit a new
carrier, a new generic body grammar, or an ownership conversion.

Each symbolic argument references the caller's persistent declaration identity
and parameter index. A foreign owner, out-of-range index, unknown concrete
type, or wrong callee arity rejects. The initial mapping operands are direct
caller parameters and concrete arguments already supported by the callee;
composite symbolic type expressions remain outside this extension.

Substitution is simultaneous and positional. Repeated type arguments duplicate
only type information. They do not duplicate an owned value. Permuting type
arguments does not reorder runtime arguments: evaluation and staging remain
left to right and ownership transfers together at the existing commit boundary.
Return compatibility is exact after substitution, including the complete
nominal carrier and its ordered arguments. Equal size or ownership layout is
insufficient. Ownership-equivalent reconstruction requires its separate gate.

## Source and independent HIR validation

Source checks the symbolic mapping before specializing a template. It then
checks the entire function over its existing admitted substitution domain,
including unused templates. Every resulting call is checked against the
callee's concrete admission profile. Checking only combinations reached from
one entry point is insufficient.

HIR independently authenticates mapping operands, target identity, call
instance identity, substituted signatures and expression ownership. Existing
monomorphization substitutes the ordered symbolic arguments and derives each
concrete callee instance identity from the resulting ordered vector. A
repeated argument is retained at each position. Closure deduplicates identical
concrete instance identities while preserving all call edges.

The existing maximum of 256 concrete function instances remains enforced.
Direct and transitive template call cycles reject before materialization,
including cycles whose type arguments change at every edge. Mapping does not
permit recursion or bypass whole-program cleanup validation. Proof-only
specializations retain their enclosing return type during validation.

## Additive graph contract

Graph v35 is selected only when a retained template contains an explicit
mapping outside the formerly admitted in-order identity form. Identity-only
programs retain Graph v34 and its exact bytes. An unused nonidentity template
still selects v35 because its symbolic semantics require the new contract.
Legacy graph routes reject a new mapping they cannot represent.

Each v35 generic call edge records `forwarded_argument_mapping` in callee
argument order. Each entry contains `callee_owner`, `callee_index`, and a
`source` object. A parameter source contains `kind: "caller_parameter"`,
`owner`, and `index`; a concrete source contains `kind: "concrete_type"` and
`type_identity`. The edge also retains the ordered concrete argument vector,
semantic and execution callee instance identities, and transfer facts.

Mappings are projected from the checked symbolic template call, associated
with its materialized call by authenticated structural expression path across
requires, body, and ensures. They must never be inferred by searching for
matching concrete types: `<A,B>` and `<B,A>` remain distinct when both are
instantiated as `i64`. A missing or ambiguous symbolic association rejects;
there is no guessed identity mapping fallback. The corresponding template
facts expose mappings even when no concrete instance is materialized.

The top-level `generic_template_forwarding` array contains `template`,
`callee_template`, `structural_path`, and `forwarded_argument_mapping` for each
symbolic generic call. Paths start at `requires/N`, `body`, or `ensures/N` and
append zero-based child indices in authored evaluation order. Entries are
ordered by template identity and structural path. Agent Context v2 attaches
the same array to selected template facts under Types or Ownership filters,
including templates without materialized instances. Concrete `call_edges`
use the mapping at the corresponding authenticated path. Existing body and
contract references resolve in the full exact-source graph, not necessarily
inside the bounded context document.

Graph replay rederives the exact document from retained checked input. Agent
Context v2 includes v35 facts under its existing byte budget; v1 remains frozen.
SemanticProgram and ProgramRoot use their existing versioned graph binding and
normalized semantic revision seed. Comment-only edits preserve semantic
instance identities while exact source/root association still detects drift.

## Implementation ownership and focused evidence

Source/HIR work replaces identity-only predicates in
`source_verify/declared_type.rs`, `source_verify/declaration/functions.rs`,
`hir/resolve_program.rs`, and `hir/validation/generic_template.rs`. Extract a
shared HIR mapping helper into a new small module; source retains its independent
check. Keep existing substitution visitors and monomorphizer unless focused
cases expose a gap. Review call signature validation in `hir/validation.rs`
and the 256-entry closure in `hir/resolve_program.rs`; neither may be weakened.

Graph work belongs in a new mapping projection submodule plus
`graph/generic_instances.rs`, schema selection in `graph/nested_owned.rs`, and
Agent Context projection in `graph/agent_instances.rs`. Audit workspace schema
selection and normalized SemanticProgram binding for v35. Avoid growing
recorded root modules beyond their budgets.

The language harness covers permutation, repetition, differing arity, mixed
concrete arguments, transitive composition, unused templates, and full owned
return compatibility. Negative cases cover foreign parameters, wrong arity,
callee-disallowed substitutions, moved-value duplication, return mismatch,
direct/transitive cycles and closure overflow with stable diagnostics.

The runtime harness checks representative Copy and owned carrier mappings on
interpreter, C11 O0/O2 and Core Wasm, including failures and repeated calls.
IR tests distinguish mappings whose concrete vectors coincide, check retained
unused template mappings, and reject edited/reminted graph facts. Workspace
and Agent Context tests bind v35 facts without changing historical v34 known
answers or budget behavior. All tests remain modules of existing harnesses.

This is internal function semantics. Public C/C++/Rust signatures, Project and
package callable profiles, WIT/Component adapters, registry contracts and public
generic ABI remain governed by their existing specifications.
