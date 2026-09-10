# Exact Program Context v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, physical/provisioned, benchmark, or exact-subject
observations below retain their narrower scope. Public promotion, registry
publication and broader product completion remain separately gated.

Audience: compiler contributors, semantic-service implementers, and reviewers
of exact cross-surface ProgramRoot selection.

`ExactProgramContext` retains one `Arc<ProjectRevision>`, a non-empty
compiler-associated `SemanticWorkspaceRevision`, its separately selected v1
ProgramRoot, the default Project-derived canonical workspace and v1 ProgramRoot,
exact `InterfaceArtifactFacts`, an exact `ProgramRootDependencyLockAssociation`,
and `ProgramRootV2`.

The two v1-root slots are explicit. They are distinct for the legacy external
association bridge and may be identical when the default Project-derived
workspace is already populated from source-owned `.spx` Agents. The dependency-lock association
and interface facts are admitted against the default Project subject. The
semantic workspace has a non-empty compiler-admitted AgentDefinitions node.
ProgramRoot v2 binds both
roots and both extensions without changing either legacy artifact.

## Construction and selection

`derive` takes exact expected Project, enriched-workspace, and ProgramRoot-v2
digests. It freshly derives the default workspace/root, validates the enriched
workspace's legacy Project/workspace/graph association, freshly replays the
interface/artifact bundle from its retained selectors, checks the lock
association and its privately retained exact lock-byte digest, derives the
enriched v1 root, and invokes exact ProgramRoot-v2 replay.

`assemble` performs the same checks after deriving ProgramRoot v2 internally.
`select` always requires both the exact enriched workspace revision and exact
ProgramRoot-v2 digest. Neither selector alone identifies a context generation.
The same dual selection is reused by the additive exact query replay,
transaction replay, service-query replay, and service-history selectors. These
routes retain the selected `ProgramRootV2` only on their typed in-memory result;
they do not widen a frozen v1 query, transaction, evidence, history-query, or
history-result document.

The compact canonical context document binds the Project and legacy workspace
revisions, both v1 root digests, interface/artifact fact digest, dependency-lock
association digest, ProgramRoot-v2 digest, fixed limits, and nonclaims. Its
domain-separated identity is `context_digest`. It never embeds the privately
retained Project Lock bytes.

## Failure and authority

`SPX-G554` rejects malformed, noncanonical, empty-Agent, or internally invalid
context material. `SPX-G555` rejects stale Project, workspace, v1/v2 root,
extension, or dual-selector associations. Owning Project, interface, artifact,
lock, canonical-workspace, and ProgramRoot diagnostics propagate when their
fresh replay fails.

The context is immutable evidence and selection state. It grants no filesystem,
network, process, source mutation, execution, deployment, effect, approval,
commit, signing, or publication authority. Submitted serialized values never
become trusted AST, HIR, graph, lock, artifact, or runtime state.

`tests/workspace/exact_program_context.rs` owns a focused success/assembly case,
the distinct default/enriched v1-root assertion, extension and v2 binding,
dual-selector hostile cases, empty AgentDefinitions rejection, private lock-byte
absence, and byte preservation for the default canonical workspace and
ProgramRoot v1. The case passes locally and additionally exercises identical
ProgramRoot-v2 selection through service generation, exact snapshot, universal
query execution and replay, universal transaction validation and replay,
service-history snapshot/query/replay, and structural-diff base while
preserving the existing v1 query, transaction, evidence, history, and diff
bytes. Every exact selector fails closed when either the enriched workspace or
ProgramRoot-v2 identity is stale, reminted, or cross-paired.

The additive [Exact Program Context v2](EXACT-PROGRAM-CONTEXT-V2.md) retains
and freshly replays this complete context before associating ProgramRoot v3 and
its contract/test facts. It changes neither this schema and digest nor the
existing v1 exact selectors.
