# Installed Proof Task Cache v1 (LAW-11)

Status: implemented bounded installed Z3 and pinned Lean profile. This is logical
query reuse for the admitted LAW-06 straight-line, direct, monomorphic, pure
scalar call profile, LAW-07 finite immutable aggregate scalarization, native
LAW-04 scalar relational laws for installed Z3 and pinned Lean, and direct
scalar Project postconditions.
It is not a general law cache or a replacement for source-bound certificates,
strict LAW-04 policy, or Project/Workspace publication checks.

## Subject and key

The compiler rederives `modular_law::summary::prepare` from authenticated
current Project HIR before consulting cache state. The prepared inventory
orders every transitive callee postcondition, staged argument and callee
precondition, and caller postcondition. The same acceptance loop handles cold
and warm work. The key is domain-separated SHA-256 over length-framed fields:

- exact translated SMT-LIB query digest, including typed checked-arithmetic
  range and lazy-order obligations, domain axioms, and SMT timeout;
- stable query role and owner; a bottom-up semantic summary digest over each
  declaration's translated queries and ordered callee summary digests;
- compiler version, checked scalar numeric model, bounded LAW-06 profile,
  explicit empty axiom set, installed Z3 version pin, exact selected
  executable-byte digest, and all selected process limits.

The key excludes source span, formatting bytes, Project revision, and
revision-scoped expression/obligation IDs. Those IDs are freshly carried into
current evidence. Source-to-logical equivalence is established only by the
same compiler translator rendering identical complete query scripts and
transitive summary keys; no heuristic source equivalence is guessed. A change
to a leaf's translated body or contract changes its semantic summary key and
all dependent tasks. Independent call closures retain their keys. If two
queries in one run have identical complete keys, the installed work is done
once and its checked result is reused for the second exact task.

The LAW-07 route rederives scalarization from the current retained Project
source before lookup. Its logical subject binds the scalarized definition,
field/case declaration identities and paths, source clause, complete Z3 query
and satisfiable-domain query or complete generated Lean module, theorem names,
backend coverage, and proof boundary. The task key adds the exact structured
backend profile and, for Lean, the full pinned export assumption inventory.
A cached success is accepted only under the same pinned installed tool bytes
and process options. The current source-bound certificate and Project proof
are newly constructed; a previous certificate is never retargeted.

The native relational Z3 route replays the current typed `LawSet` and builds a
deterministic law dependency index. Each logical digest binds its normalized
law semantics, proof profile, named assumption IDs and owning module, and the
logical digests of every prerequisite law in topological order. It omits only
the current Project association, which is required when a new opaque proof is
built. The exact generated checked-scalar query, translator profile, tool
binary/version and process options also enter the task key. A prerequisite
statement or assumption change invalidates its dependent closure while an
independent law may reuse its success. Pinned Lean native relational laws use
the same complete checked theorem/axiom-report mechanism as direct Project
Lean exports. Their cached key additionally binds the law dependency index.
`prove_scalar_law_batch_cached` expands selected native relational laws to
their exact transitive prerequisites, rejects missing/cyclic or nonrelational
dependencies before any installed process starts, and checks the closure in
stable prerequisite order. Every returned proof binds the current LawSet and
Project revision. A changed leaf reproves that leaf and its dependents while
an independent law may validate and reuse its prior checked task. An unrelated
Project artifact change still mints current evidence; old opaque proofs are
not accepted as current.

For a direct scalar Project postcondition, the compiler rebuilds current HIR,
the complete proof script and the satisfiable-domain script. Each invocation
runs and checked-replays a fresh domain model because that witness is included
in the exact current source-bound receipt. The cached task skips only the
postcondition proof query. Its key binds both current scripts, the selected
declaration and clause, the scalar translator profile, installed Z3 identity,
and process limits. The direct scalar subset admits no calls; unsupported call
graphs are refused by translation before lookup.

The direct Project Lean route rebuilds the complete export, bounded
precondition-domain witness, and bound Wasm artifact before cache lookup. A
pinned kernel acceptance stores only its ordered, standard-axiom report for
the exact complete Lean module, theorem inventory, and current compiled Wasm
artifact digest. A warm run validates
that report against the current theorem names and reconstructs a fresh
source-bound certificate and ProgramRoot attachment. The private snapshot
stores no arbitrary kernel output or prior certificate.

## Storage and replay

`ProofTaskCache::for_project` binds a cache to one canonical local Project
root. The caller must hold the matching authenticated Project revision; the
cache cannot construct one. The cache contains only successful installed-tool
task keys, translated script digests, and pinned tool versions. It stores no
source, solver transcript, counterexample, secret, or application input.

`semantic_cache_store::{persist_modular_proofs,load_modular_proofs}` reuses the
existing host-selected dedicated private root, key, complete-envelope HMAC,
compiler-file binding, inventory, create-new publication, 32-entry retention
ceiling, and digest-selected eviction. A distinct canonical
`semaprax.modular-proof-task-cache.v1` payload caps at 1 MiB and 1,024 tasks.
The Unix store authenticates the entire selected envelope before invoking its
private decoder. Loaded entries are still structurally replayed against live
translated scripts, the transitive key, Project root scope, tool pin, and
process limits at each lookup. A mismatch is stale and invokes fresh installed
checking. A selected modular law stages successful queries in private memory
and commits them only after every dependency and caller query is proved and the final
cancellation check passes. Failed, refuted, unknown, timed-out, or cancelled
work inserts no partial task set. Cancellation is checked before, during, and
after warm reuse.

The installed cached API rederives a complete current `ModularProof` with the
ordinary current obligation IDs and Project revision. The existing installed
Project attachment then mints an opaque `VerifiedProjectProof` against the
current source digest, Project revision and ProgramRoot. No prior byte-bound
certificate is rewritten or treated as current. Cold and warm verdicts and
law inventory must match; `fresh`, `reused`, and `stale` are work metrics only.

## Per-law work inventory

`law_set::work_inventory::derive` renders the bounded canonical
`semaprax.law-proof-work-inventory.v1` report for an authenticated current
Project revision and an independently retained strict LAW-04 policy. It
rederives the law inventory, strict verdicts, and topological logical dependency
index from the current LawSet and host-held opaque proofs. Each law is ordered
by stable identity and retains its semantic digest, logical dependency digest,
current obligation ID, exact reason, and separate `proved`, `missing`,
`unsupported`, or `inconclusive` outcome. The strict satisfaction bit stays
separate from work counters; a cache hit alone cannot change the verdict.

The cache holds at most 4,096 process-local recent task events for one exact
Project revision. An event is recorded only after the relevant installed proof
route completes successfully; failed, cancelled, and refuted work cannot
masquerade as checked success. The report associates each event with its law
and lists tasks in deterministic role and owner order. `fresh`,
`validated_reuse`, and `stale` count work for the latest completed invocation
of each task owner in this process, not a global performance history. Revision
change clears the observations, and event overflow refuses the report. Events
are never serialized in the authenticated cache snapshot and cannot grant
proof, source, execution, or publication authority. The report is a read-only
diagnostic projection; strict LAW-04 replay remains the decision boundary.

## Boundaries

Only the installed LAW-06 modular scalar, LAW-07 structured aggregate,
native LAW-04 scalar relational, and direct scalar Project postcondition
profiles (Z3 and pinned Lean) are cached here. Separately
authored library lemmas, foreign or dynamic calls, effects, generic instances, unsupported
branches/lazy calls, and target-artifact claims have no cache admission under
these profiles. LAW-07 Lean reuse binds its explicit export assumptions;
LAW-06 accepts no added axioms or named assumptions. Their
existing proof/refusal routes remain authoritative and no cache entry can
promote one. The existing trusted-local installed tool and compiler
translation are trusted; a protected cache key and immutable static compiler
installation are host preconditions. The proof grants no process, filesystem,
network, execution, source-mutation, publication, or runtime-guard-removal
authority. No timing or memory improvement is claimed without measurement.
