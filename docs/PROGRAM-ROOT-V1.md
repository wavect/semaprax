# ProgramRoot v1

Status: additive SEG-02 foundation; focused evidence passes locally.

Audience: compiler contributors, semantic-service authors, Agent-runtime
designers, and reviewers of canonical semantic identity.

ProgramRoot v1 is the content-addressed, source-owned root of the emerging
Semantic Execution Graph. It is an additive segmented projection of an exact
[Canonical Semantic Workspace Revision v1](CANONICAL-SEMANTIC-WORKSPACE-REVISION-V1.md),
not a competing Project, workspace, image, or graph representation. Canonical
`.spx` and the admitted `ProjectRevision` remain its source of truth.

## Public API and schemas

`src/project/program_root.rs` exports:

```rust
pub const PROGRAM_ROOT_SCHEMA: &str = "semaprax.program-root.v1";
pub const PROGRAM_ROOT_SEGMENT_SCHEMA: &str =
    "semaprax.program-root.segment.v1";
pub const PROGRAM_ROOT_RELATIONSHIP_SCHEMA: &str =
    "semaprax.program-root.relationship.v1";
pub const MAX_PROGRAM_ROOT_BYTES: usize = 256 * 1024;
pub const MAX_PROGRAM_ROOT_SEGMENT_BYTES: usize = 16 * 1024;
pub const MAX_PROGRAM_ROOT_RELATIONSHIP_BYTES: usize = 4 * 1024;

pub struct ProgramRoot { /* opaque */ }
pub struct ProgramRootSegment { /* opaque */ }
pub struct ProgramRootRelationship { /* opaque */ }
```

`ProgramRoot::derive` accepts one already-derived
`SemanticWorkspaceRevision`. `SemanticWorkspaceRevision::program_root`
provides the same operation, and `ProjectRevision::program_root` first derives
the existing canonical revision. No route reads a path or admits caller
semantic bytes.

`ProgramRoot::replay` takes the exact canonical workspace object, the expected
ProgramRoot digest, and submitted ProgramRoot bytes. It returns a freshly
derived value only after canonical shape, identity, association, and complete
byte equality succeed.

## Small manifest and identity

The ProgramRoot manifest binds:

- schema and compatibility identifiers;
- the exact Canonical Semantic Workspace Revision v1 identity;
- its separate semantic, source-projection, manifest, and dependency-lock
  component digests;
- nine ordered node-segment descriptors;
- three explicitly unbound runtime-root relationships;
- fixed limits and nonclaims; and
- `program_root`, the ProgramRoot identity.

The identity uses domain
`semaprax.program-root.digest.v1\0` over compact, recursively key-sorted JSON
with one terminal LF. The identity subject is the complete manifest except the
self-referential `program_root` field. The final manifest embeds that digest.
Replay reconstructs and verifies both the identity subject and final bytes.

## Content-addressed node segments

Each `semaprax.program-root.segment.v1` descriptor contains exactly:

```text
schema
kind
node_schema
node_digest
node_bytes
segment_digest
```

`node_digest`, `node_schema`, and `node_bytes` bind the exact existing typed
node and its terminal-LF canonical bytes. The descriptor does not duplicate or
deserialize the node payload. `segment_digest` uses domain
`semaprax.program-root.segment.digest.v1\0` over the canonical descriptor with
the self-referential digest field removed. Replay freshly derives every
descriptor from the trusted canonical workspace object and exact-compares it.

Segment array order is semantic and fixed:

1. `source_projection`
2. `semantic_program`
3. `stable_identity_index`
4. `dependency_closure`
5. `contracts_and_tests`
6. `agent_definitions`
7. `authority_policies`
8. `target_profiles`
9. `projection_metadata`

Consumers must neither sort nor repair this vector. The order preserves the
existing object family's source, program, identity, dependency, verification,
Agent, authority, target, and metadata boundaries. It does not imply execution
order or mutable storage layout.

## Runtime-root relationships

ProgramRoot v1 contains exactly three
`semaprax.program-root.relationship.v1` descriptors, in this order:

| kind | expected root schema |
| --- | --- |
| `deployment_root` | `semaprax.deployment-root.v1` |
| `instance_root` | `semaprax.instance-root.v1` |
| `evidence_root` | `semaprax.evidence-root.v1` |

Every v1 relationship is compiler-constructed with `binding: "unbound"` and
`digest: null`. Callers cannot inject a binding. These values reserve explicit
typed relationships for later AgentDeployment, durable instance, and execution
evidence work; they are not claims that those roots or their lifecycles exist.

Changing a future compatible deployment is intended to change DeploymentRoot
without changing ProgramRoot. That separation is an architectural direction,
not functionality admitted by this placeholder version.

## Compatibility and authority boundary

ProgramRoot v1 leaves the complete Canonical Semantic Workspace Revision v1
JSON and digest algorithm unchanged. It also changes no canonical source,
Project manifest/revision, managed workspace, Semantic Workspace Image, source
Graph, HIR, cleanup/loan plan, universal query, universal transaction,
semantic-service protocol, build, execution, package, or generated-consumer
bytes. Existing users opt into the new projection.

The projection owns no filesystem, network, process, model, effect, approval,
deployment, instance, evidence, commit, signing, or publication authority. It
contains no durable runtime state. Segment descriptors are indexes over already
authenticated canonical nodes, not trusted node deserialization or a new
semantic cache.

AgentDefinitions remains exactly the selected canonical workspace node. The
additive explicit-association bridge can therefore populate this ProgramRoot
segment automatically with compiler-admitted AgentDefinition bundles. It does
not synthesize `.spx` Agent declarations or relabel explicit caller association
as intrinsic Project ownership; that belongs to AGENT-03.

## Bounds and diagnostics

The complete root is capped at 256 KiB, each segment descriptor at 16 KiB, and
each relationship descriptor at 4 KiB. Existing Project and canonical
workspace bounds apply first.

| code | meaning |
| --- | --- |
| `SPX-G550` | Invalid, malformed, noncanonical, over-bound, or internally inconsistent ProgramRoot material. |
| `SPX-G551` | Stale expected identity, workspace association, or exact replay mismatch. |

## Focused evidence

`tests/workspace/canonical_revision.rs` derives ProgramRoot through all three
public entry points, independently recomputes its root and descriptor digests,
checks exact node schema/digest/byte associations and fixed ordering, verifies
the unbound typed relationships, exercises exact replay and malformed/stale
failure, and snapshots the legacy Project, semantic graph, canonical workspace
identity, and canonical workspace bytes before and after derivation.

The ProgramRoot, explicit AgentDefinition association, persistent service,
transaction, and composition filters pass together as a 33-case local focused
gate. Strict all-target clippy also passes.
