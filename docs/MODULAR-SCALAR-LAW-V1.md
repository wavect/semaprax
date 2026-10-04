# Modular Scalar Law v1: checked pure-call summaries

Audience: Project authors and maintainers evaluating bounded law and proof support.

Status: bounded LAW-06 Project proof profile. The completion matrix records the
executable gate and any remaining gaps. Neither a proof record nor a certificate
grants execution, source mutation, publication, or runtime guard removal.

`modular_law::plan` accepts a stable declaration ID in one retained authenticated
Project revision. It walks linked HIR for direct monomorphic pure scalar calls,
orders acyclic transitive callees before callers, and records resolved call
occurrences. A summary digest binds the exact callee source slice, contracts,
compiler profile, and ordered transitive summary digests. Project revision is
recorded separately so an unrelated edit can be identified without silently
reusing a proof token from an older revision. Self or mutual recursion, generic
instantiation, dynamic or foreign calls, effects, and unsupported scalar forms
refuse under named reasons.

`summary::prove_straight_line` checks a callee's own postconditions before its
postconditions may be used at a caller. Direct call results become fresh typed
output variables. It proves each actual argument is well defined and each
callee precondition in evaluation order using only earlier checked summaries.
The caller postcondition is then proved with the exact instantiated callee
summaries. Branches and lazy boolean operands refuse this straight-line
profile explicitly. Abstract SAT does not claim a concrete caller witness.
The bounded `prove::prove_postconditions` path remains an explicit capture-free
inlining fallback, with checked-model replay for a concrete witness where
supported. Source bodies and runtime checks are not rewritten.

`certificate` exports canonical `semaprax.modular-scalar-summary-proof.v2` JSON
with the linked dependency graph, ordered query digests, exact Project revision,
solver identity/version, and trust boundary. Each checked callee clause, staged
argument and callee precondition, and caller postcondition has an explicit
stable obligation ID. Per-call IDs include the retained resolved call expression
identity and source-order clause/argument index, so repeated calls cannot alias.
Version 1 transcripts lack those bindings and refuse replay under version 2.
A digest authenticates the envelope
shape only. `replay` rederives the live plan and reruns every query with the
explicit solver; the installed variant uses the registered held Z3 process
provider. Stale, missing, unknown, SAT, malformed, oversized, or mismatched
transcripts refuse. `classify_drift` is advisory: it distinguishes a relevant
summary change from a Project-only unrelated revision but never attaches a
proof to the latter.

`proof_export::installed_project::prove_modular_postcondition` returns an opaque
`VerifiedProjectProof` for the exact retained source, postcondition index,
ProgramRoot, and Project revision after all modular queries pass through the
registered installed Z3 provider. LAW-04 strict policy has a distinct
`pinned_modular_smt_source` requirement, which checks the installed version and
modular translation bounds. The host selects `open_modular_scalar` explicitly;
its held process ledger caps the whole tool session at 128 invocations and
8 MiB reserved input/output, while ordinary installed proof tools retain their
smaller budget. A native contract law can select a postcondition
`result` binder only at the subject function's exact return type. Existing
pinned SMT policy does not accept a modular method under its older profile.
Selected Project execution and strict managed-Workspace publication rederive
their ordinary host-selected inventory and acquire their ordinary authority;
the proof token is only evidence for the corresponding law. Publication still
performs the same final `ACTIVE` pivot and does not rewrite original files.

The direct SMT and Lean source profiles remain call-free. No branch or lazy-call
summary theorem is claimed. The explicit installed Z3 gate exercises a
three-function, two-module accounting law, a replayed caller precondition
witness, capture-free repeated calls, weakened-summary refusal, certificate
replay and drift, selected Project attachment, and selected physical Workspace
publication. An admitted `environment-io.v1` Project supplies the named
effectful-function refusal gate without altering its capability policy. Run
`cargo test --offline --locked --test workspace modular_law::`
for bounded non-solver tests and explicitly provision the pinned installed Z3
for ignored `modular_law::real_z3_` and `modular_law::installed_modular_` tests.
