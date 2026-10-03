# Modular Scalar Law v1: bounded call inlining

Status: implemented as an additive, read-only Project proof API. The completion
matrix records remaining LAW-06 work. This profile does not attach evidence to
LAW-04 protected publication, and does not claim reusable modular summaries.

`src/assurance_manifest/modular_law/` accepts a target stable declaration ID in
one retained authenticated `ProjectRevision`. It walks linked HIR rather than
source call names, orders transitive pure scalar callees before callers, and
refuses a cycle or a body outside the SMT scalar subset. The plan records each
call's resolved expression and callee IDs, a digest of each function's exact
source slice, and ordered transitive dependency digests. The Project revision
is recorded separately: a change to an unrelated function changes that
revision without changing the selected function's semantic digest. The digest
includes this profile and compiler package version; it is proof metadata, not
an opaque certificate or a publication authorization.

The current real Z3 path is **bounded inlining**. It creates a proof-only AST
from retained HIR value IDs. Fresh local names prevent capture by shadowed
source names; each actual argument is evaluated once, left to right. Each
callee `requires` is checked in source order before its body. A false clause
selects a deliberately checked-overflowing branch, so the existing typed VC
must prove that branch unreachable. A caller cannot assume a callee's
precondition or a later clause before it has proved the earlier check. The
ordinary checked source and runtime guards remain unchanged.

`prove_postconditions` proves each transitive function's own postconditions
before its caller through the existing explicit Z3 provider and checked-model
replay. Every `unsat` is recorded with the exact plan digest, script digest,
solver identity and version. A `sat` is returned only after checked replay;
when possible, the concrete parameter model is returned. Unknown, timeout,
missing solver, unsupported grammar, and contradictory input domain never
produce a proof. The full inlined proof subject has a 4096-node bound, and the
plan has 64-function and 256-call bounds. This fallback retranslates the
reachable call graph; it does not consume a checked callee postcondition as a
summary assumption.

Current open LAW-06 requirements: reusable separately proved summary
substitution and exact composition under assumptions; explicit SCC component
reporting beyond cycle refusal; complete effect, foreign, dynamic and generic
negative corpus; and LAW-04 report/strict publication integration. The public
SMT and Lean no-call profiles are unchanged. Neither a plan nor a `Proof`
record grants publication or removal of a runtime guard.

Focused evidence: `cargo test --offline --locked --test workspace modular_law::`
for the planner/cycle tests, then explicitly provision installed Z3 and run
`cargo test --offline --locked --test workspace modular_law::real_z3_ --
--ignored`. The latter includes the three-function/two-module proof and an
out-of-precondition caller witness.
