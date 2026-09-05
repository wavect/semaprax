# ProgramRoot v2

Status: additive core contract. ProgramRoot v1 and Canonical Semantic Workspace
Revision v1 remain byte-for-byte unchanged.

Audience: compiler contributors, semantic-service implementers, and reviewers
of exact source-owned ProgramRoot identity.

## Identity and inputs

`semaprax.program-root.v2` is derived from four already-admitted typed values:

1. one exact `SemanticWorkspaceRevision` with a non-empty compiler-admitted
   `AgentDefinitions` v1 node, populated either from source-owned `.spx`
   declarations or the legacy explicit association bridge;
2. the default Project-derived ProgramRoot v1 (`base_project_root_digest`),
   which anchors the admitted Project Lock association;
3. one exact `InterfaceArtifactFacts` v1 bundle; and
4. one exact `ProgramRootDependencyLockAssociation` v1.

The manifest separately records `semantic_workspace_root_digest`, the
ProgramRoot v1 derived from the possibly agent-enriched semantic workspace.
The base and semantic-workspace roots are distinct anchor roles, but their
digests may coincide when default Project derivation already contains
source-owned Agent definitions. Their typed inputs must name the same legacy
Project revision. The association must
name the exact supplied base root and its canonical workspace revision.

The v2 identity is SHA-256 over the canonical manifest without
`program_root_v2_digest`, framed by
`semaprax.program-root.digest.v2\0` and the little-endian byte length.
Canonical JSON has lexicographically ordered object keys and one terminal LF.
The manifest is bounded by 512 KiB.

## Fixed segments

The segment inventory has exactly eleven entries in this order:

1. `source_projection`
2. `semantic_program`
3. `stable_identity_index`
4. `dependency_closure`
5. `contracts_and_tests`
6. `agent_definitions`
7. `authority_policies`
8. `target_profiles`
9. `projection_metadata`
10. `interface_artifact_facts`
11. `project_lock_association`

Entries 1–9 are the exact descriptors, in exact order, from the enriched
semantic workspace's ProgramRoot v1. Entry 6 is therefore the existing
canonical AgentDefinitions node, never a duplicate extension. Entries 10–11
use the ProgramRoot segment v1 descriptor schema and digest domain and bind the
exact typed bundle/association schema, digest, and canonical byte count. Node
payload bytes are not embedded in the root.

For a Project containing `.spx` Agent declarations, ordinary Project admission
retains the frozen compatibility products and default canonical derivation
selects them automatically. The `agent_definitions` segment is therefore the
same node digest observed through the Project revision, canonical workspace,
typed AgentDefinitions query, and semantic-service generation.

## Relationships and replay

The fixed `deployment_root`, `instance_root`, and `evidence_root`
relationships are inherited exactly from ProgramRoot v1. Each is `unbound`,
has a null digest, and names its expected v1 root schema. Callers cannot inject
a binding, so this root contains no root cycle.

Replay first rejects non-canonical, oversized, malformed, reordered, reminted,
or bound/cyclic submitted manifests as `SPX-G550`. It then rederives from the
four typed inputs and requires the expected v2 digest and every submitted byte
to match, otherwise returning `SPX-G551`.

## Nonclaims

ProgramRoot v2 is a content-addressed association, not dependency resolution.
It grants no filesystem, network, execution, deployment, publication, or
commit authority. Interface/artifact and lock entries are exact fact
descriptors, not embedded payloads. Unbound runtime-root placeholders are not
evidence that any DeploymentRoot, InstanceRoot, or EvidenceRoot exists.

The two focused Workspace-harness cases pass locally, covering exact replay,
segment preservation/order, cross-Project binding, empty-Agent rejection,
reminted reordering, and an attempted root self-cycle.
