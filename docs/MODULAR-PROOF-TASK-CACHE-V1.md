# Modular Proof Task Cache v1 (LAW-11 partial)

Status: bounded installed-Z3 implementation. This is logical-query reuse for
the admitted LAW-06 straight-line, direct, monomorphic, pure scalar call
profile. It is not a general law cache or a replacement for source-bound
certificates, strict LAW-04 policy, or Project/Workspace publication checks.

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

## Storage and replay

`ProofTaskCache::for_project` binds a cache to one canonical local Project
root. The caller must hold the matching authenticated Project revision; the
cache cannot construct one. The cache contains only successful installed-Z3
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
checking. A selected law stages successful queries in private memory and commits them
only after every dependency and caller query is proved and the final
cancellation check passes. Failed, refuted, unknown, timed-out, or cancelled
work inserts no partial task set. Cancellation is checked before, during, and
after warm reuse.

The installed cached API rederives a complete current `ModularProof` with the
ordinary current obligation IDs and Project revision. The existing installed
Project attachment then mints an opaque `VerifiedProjectProof` against the
current source digest, Project revision and ProgramRoot. No prior byte-bound
certificate is rewritten or treated as current. Cold and warm verdicts and
law inventory must match; `fresh`, `reused`, and `stale` are work metrics only.

## Boundaries

Only the installed LAW-06 modular scalar profile is cached here. Native
relational laws, Lean, LAW-07 aggregate queries, assumptions, library lemmas,
foreign or dynamic calls, effects, generic instances, branches and lazy calls,
and target-artifact claims have no cache admission under this profile. Their
existing proof/refusal routes remain authoritative and no cache entry can
promote one. The existing trusted-local installed tool and compiler
translation are trusted; a protected cache key and immutable static compiler
installation are host preconditions. The proof grants no process, filesystem,
network, execution, source-mutation, publication, or runtime-guard-removal
authority. No timing or memory improvement is claimed without measurement.
