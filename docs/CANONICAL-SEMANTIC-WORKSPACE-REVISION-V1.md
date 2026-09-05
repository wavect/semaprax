# Canonical Semantic Workspace Revision v1

Status: local, partial; the completion matrix owns product status.

Audience: compiler contributors, workspace-service authors, agent-tool authors,
and reviewers of semantic subjects.

Canonical Semantic Workspace Revision v1 is an authority-free, immutable,
Project-derived semantic object. It gives one already admitted
`ProjectRevision` a single canonical object family with typed projections for
source, checked meaning, identity, dependencies, contracts and tests, agents,
authority, targets, and projection metadata. It does not replace canonical
`.spx` source as the Git representation and does not make a serialized object
trusted compiler state.

This version is a compatibility-preserving foundation. Existing Project,
managed Workspace, Workspace Semantic Graph, Semantic Workspace Image v1,
candidate, transport, and publication schemas retain their exact bytes and
revision algorithms.

## Public API

The implementation is owned by
`src/project/canonical_workspace_revision.rs` and exported through
`semaprax::project`:

```rust
pub const SEMANTIC_WORKSPACE_REVISION_SCHEMA: &str =
    "semaprax.semantic-workspace-revision.v1";
pub const SEMANTIC_WORKSPACE_REVISION_COMPATIBILITY: &str =
    "semaprax.semantic-workspace-revision-compatibility.v1";
pub const MAX_SEMANTIC_WORKSPACE_REVISION_BYTES: usize = 32 * 1024 * 1024;

pub struct SemanticWorkspaceRevision { /* opaque */ }

impl SemanticWorkspaceRevision {
    pub fn derive(revision: &ProjectRevision)
        -> Result<Self, Vec<Diagnostic>>;
    pub fn replay(
        revision: &ProjectRevision,
        expected_workspace_revision: &str,
        bytes: &[u8],
    ) -> Result<Self, Vec<Diagnostic>>;

    pub fn to_json(&self) -> &str;
    pub fn workspace_revision(&self) -> &str;
    pub fn semantic_digest(&self) -> &str;
    pub fn source_projection_digest(&self) -> &str;
    pub fn manifest_digest(&self) -> &str;
    pub fn dependency_lock_digest(&self) -> &str;

    pub fn source_projection(&self) -> &SourceProjection;
    pub fn semantic_program(&self) -> &SemanticProgram;
    pub fn stable_identity_index(&self) -> &StableIdentityIndex;
    pub fn dependency_closure(&self) -> &DependencyClosure;
    pub fn contracts_and_tests(&self) -> &ContractsAndTests;
    pub fn agent_definitions(&self) -> &AgentDefinitions;
    pub fn authority_policies(&self) -> &AuthorityPolicies;
    pub fn target_profiles(&self) -> &TargetProfiles;
    pub fn projection_metadata(&self) -> &ProjectionMetadata;
}

impl ProjectRevision {
    pub fn canonical_workspace_revision(&self)
        -> Result<SemanticWorkspaceRevision, Vec<Diagnostic>>;
}
```

The nine node wrapper types are distinct, public, immutable values:

```text
SourceProjection
SemanticProgram
StableIdentityIndex
DependencyClosure
ContractsAndTests
AgentDefinitions
AuthorityPolicies
TargetProfiles
ProjectionMetadata
```

They expose only their canonical read-only projections and digests. None
contains a filesystem root, held handle, mutable cache, process, network
client, secret, signing key, approval token, or publication method.

## Object model

The top-level schema is `semaprax.semantic-workspace-revision.v1`. Objects are
recursively sorted by key, arrays retain their declared order, and the compact
document has exactly one terminal LF. The exact top-level key order is:

```text
compatibility,digests,limits,nodes,nonclaims,schema,workspace_revision
```

`digests` has exact order
`dependency_lock,manifest,semantic,source_projection`. `limits` contains only
`max_revision_bytes`, whose value is 33,554,432. `nodes` has exact order:

```text
agent_definitions,authority_policies,contracts_and_tests,dependency_closure,
projection_metadata,semantic_program,source_projection,stable_identity_index,
target_profiles
```

Every node entry has exact keys `digest,value`. `value` is the canonical node
object with exact keys `payload,schema`. Node schemas use
`semaprax.semantic-workspace-revision.<kebab-node-name>.v1`; their digest
domains use the corresponding
`semaprax.semantic-workspace-revision.<kebab-node-name>.digest.v1\0` string.
Projection metadata carries the compiler package/version, fixed compatibility
identifier, legacy Project/workspace revision identities, and Project graph
digest needed for exact association.

The object is derived only after ordinary complete Project admission. The
`SemanticProgram` node is derived from retained checked Project meaning; the
`SourceProjection` node retains exact canonical source facts; the stable-index,
dependency, contract/test, agent, authority, target, and metadata nodes are
bounded projections of facts already owned by that same admitted Project.
They are not independently mutable stores.

The v1 node payloads are closed:

| Node | Payload |
| --- | --- |
| `SourceProjection` | ordered `files` with `bytes`, `path`, `source_digest`, `source_graph_schema`, and `source_revision`; exact `workspace_manifest` |
| `SemanticProgram` | `entry_module`; ordered comment-free `normalized_sources` with `path` and `semantic_source_digest`; compiler prelude digest |
| `StableIdentityIndex` | retained Project image `stable_ids` index |
| `DependencyClosure` | declared source `name`/`path` bindings, dependency `name`/`range` requirements, and resolved dependency-source path/revision/digest facts |
| `ContractsAndTests` | `contract_fingerprints` set to the semantic-program digest and selected `test_module` |
| `AgentDefinitions` | empty `definitions` inventory plus `integration: no_project_agent_definition_declarations` |
| `AuthorityPolicies` | manifest `required_capabilities` inventory |
| `TargetProfiles` | manifest `contract` and `profile`, `targets` matrix or the existing native64/wasm32 default, and `web_exports` identities |
| `ProjectionMetadata` | `compiler_package`, `compiler_version`, `compatibility`, `legacy_project_revision`, `legacy_workspace_revision`, and `project_graph_digest` |

The empty `AgentDefinitions` inventory is an honest compatibility marker, not
evidence that Project declarations already derive language-native agents.

`DependencyClosure` records the noncyclic dependency requirements, declared
subject bindings, and resolved dependency-source facts available in the
admitted Project. Its `dependency_lock_digest` is the digest of that canonical
node projection. It is not a `Project Lock v1` document, does not read a lock
path, and does not claim registry resolution, acquisition, signature admission,
revocation state, vulnerability state, or publication provenance.

## Component and composite identity

The four component identities remain deliberately distinct:

```text
semantic_digest
source_projection_digest
manifest_digest
dependency_lock_digest
```

The source-projection component reuses the `SourceProjection` node digest. The
manifest component is SHA-256 over the exact canonical Project manifest under
domain `semaprax.semantic-workspace-revision.manifest.digest.v1\0`. The
dependency-lock component is SHA-256 over the complete canonical
`DependencyClosure` node under domain
`semaprax.semantic-workspace-revision.dependency-lock.digest.v1\0`. Node and
byte-payload digests use
`domain || u64le(byte_length) || exact_bytes`.

The semantic component uses domain
`semaprax.semantic-workspace-revision.semantic.digest.v1\0` and length-frames
these six node digest strings in order:

```text
SemanticProgram
StableIdentityIndex
ContractsAndTests
AgentDefinitions
AuthorityPolicies
TargetProfiles
```

Normalized comment-free semantic-source and prelude payloads use the additional
domains `semaprax.semantic-workspace-revision.normalized-source.digest.v1\0`
and `semaprax.semantic-workspace-revision.prelude.digest.v1\0` respectively.

The composite `workspace_revision` uses domain
`semaprax.semantic-workspace-revision.digest.v1\0` and length-frames
the four canonical digest strings in this order:

```text
semantic_digest
source_projection_digest
manifest_digest
dependency_lock_digest
```

This separation is normative. A source-only projection change can change the
source projection component without pretending that it is a behavioral delta;
a semantic change changes the semantic component; manifest or dependency
closure changes remain visible in their own components. The composite binds
all four into one exact subject.

This `workspace_revision` is the revision of the new canonical object. It does
not replace, alias, or silently redefine the existing managed Workspace v1
revision or `ProjectRevision::workspace_revision()` value. Callers must not
compare the two merely because both are encoded as `sha256:` tokens.

## Exact derivation and replay

`derive` consumes only the authority-neutral facts of one immutable,
fully-admitted `ProjectRevision`. It renders every node and the outer object
canonically, with compact UTF-8 JSON and exactly one terminal LF. Arrays retain
their specified semantic order; set-like inventories use their specified
canonical order. Derivation is deterministic for the same exact Project
subject.

`replay` first validates the independently supplied expected canonical
Workspace revision. It freshly derives the complete object from the supplied
`ProjectRevision`, then requires both the expected revision and the submitted
bytes to equal the fresh result exactly. Unknown or duplicate fields,
noncanonical JSON, alternate whitespace or escaping, missing or extra LF,
cross-Project substitution, stale component digests, reordered inventories,
self-consistently reminted outer JSON, and trailing data fail closed because the
supplied byte string cannot equal the fresh canonical object.

Submitted bytes are never deserialized into trusted AST, HIR, semantic graph,
cleanup plan, loan plan, dependency closure, policy, target, or compiler cache
state. Successful replay returns the freshly derived object, not state retained
from the submitted bytes.

## Compatibility boundary

This version adds one new authority-neutral projection. It does not change the
bytes, schemas, digest domains, revision identities, limits, or behavior of:

- canonical `.spx` formatting, source Graph v10-v14, HIR, cleanup plans, or
  loan plans;
- Project manifests, Project revisions, their existing workspace revisions,
  Project graphs, or target-admission descriptors;
- managed Workspace v1 manifests, snapshots, transactions, generations, or
  `ACTIVE` publication;
- Semantic Workspace Image v1, image transport v1-v6, candidate handles,
  candidate reports, archives, stores, or recovery;
- Semantic Patch, Workspace Change, Operations, Structural Change, evidence,
  review, build, execution, Git, package, or deployment artifacts.

The fixed compatibility identifier describes only this derivation/replay
contract. It is not a compiler binary fingerprint and does not promise forward
acceptance of later schemas.

## Bounds, diagnostics, and failure

The complete canonical object is bounded by
`MAX_SEMANTIC_WORKSPACE_REVISION_BYTES` (33,554,432 bytes), including its
terminal LF. Existing Project, manifest, source, graph, dependency, contract,
test, agent, authority, and target bounds remain in force before this additional
output bound is applied. Bounded rendering fails before an oversized object is
returned.

`SPX-G222` owns malformed, noncanonical, or over-bound canonical Workspace
Revision input/projection failures. `SPX-G223` owns stale expectation,
association, digest, or exact-replay disagreement. Existing Project admission
diagnostics propagate unchanged when derivation cannot obtain the required
already-admitted facts.

Failure returns no partial object and changes no source, managed generation,
candidate, image, cache, Project, or external state.

## Nonclaims

The exact ordered outer `nonclaims` array is:

```text
no_filesystem_or_publication_authority
no_trusted_hir_deserialization
no_project_agent_definition_integration
dependency_lock_is_a_local_admitted_closure_projection_not_project_lock_v1
```

In addition, Canonical Semantic Workspace Revision v1 does not establish:

- the universal `SemanticTransaction` operation algebra or a mutation route;
- full semantic completeness for every language, package, agent, authority,
  deployment, target, evidence, or generated-artifact fact;
- a persistent or incremental workspace service, index database, daemon, LSP,
  MCP server, editor protocol, invalidation algorithm, or warm-cache guarantee;
- `.spx` agent syntax, compiler-derived `AgentDefinition`, compiled agent
  transitions, provider bindings, mutable agent instances, or evidence ledgers;
- source, filesystem, process, network, dependency-acquisition, package,
  signing, payment, Git, build, deployment, or publication authority;
- approval, authentication of an external principal, signature verification,
  provenance, durability, rollback, garbage collection, or atomic visibility;
- a migration from the existing Project, Workspace, Image, candidate, patch,
  evidence, or publication protocols.

## Focused evidence

The owning regressions must prove deterministic derivation, exact schema and
component identities, all nine distinct typed node projections, exact replay,
one-byte-over-limit rejection, stale expected revision, malformed and
self-consistently reminted hostile inputs, source/semantic/manifest/dependency
component separation, authority absence, no writes, and byte preservation for
legacy Project/workspace/Image v1 artifacts.

The three focused canonical-revision regressions passed locally on 2026-09-05.
They establish only this bounded authority-free projection, not any nonclaim
above or completion of a broader product row. After later upstream baseline
repairs, the repository-wide full gate reached 1,536 passing library tests but
still stopped on 11 unrelated Project, Wasm, and WIT failures before its later
stages.
