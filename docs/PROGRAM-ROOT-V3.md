# ProgramRoot v3

Status: additive SEG-02 successor contract; focused local evidence passes.
ProgramRoot v1/v2 and Canonical Semantic Workspace Revision v1 remain
byte-for-byte unchanged.

Audience: compiler contributors, semantic-service implementers, and reviewers
of exact source-owned ProgramRoot identity.

ProgramRoot v3 binds one independently replayable
[Contracts and Tests Facts v1](CONTRACTS-AND-TESTS-FACTS-V1.md) object to the
complete existing ProgramRoot v2 association. It adds no contract proof, test
coverage, test execution, runtime root, or authority.

## Identity and inputs

`ProgramRootV3::derive` accepts the same already-admitted workspace, default
Project root, interface/artifact facts, and Project Lock association used to
freshly derive ProgramRoot v2, plus one `ContractsAndTestsFacts` object for the
same exact Project subject. A cross-Project fact bundle fails closed.

The schema is `semaprax.program-root.v3`; compatibility is
`extends-semaprax.program-root.v2`; and the complete canonical manifest is
capped at 768 KiB. The v3 identity is SHA-256 over the canonical manifest
without `program_root_v3_digest`, framed by:

```text
"semaprax.program-root.digest.v3\0" || u64le(byte_length)
```

The manifest records the exact derived ProgramRoot-v2 digest and enriched
semantic-workspace revision. It does not deserialize a root, fact bundle, HIR,
Graph, or lock from caller JSON.

## Fixed segments and relationships

The segment array contains exactly twelve descriptors in this order:

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
12. `contracts_and_tests_facts`

Entries 1–11 are exact `ProgramRootSegment` values from the freshly derived
ProgramRoot v2 in the same order. Entry 12 uses the existing ProgramRoot
segment-v1 descriptor schema and digest domain to bind the fact schema, fact
digest, and exact fact byte count. It does not contain the fact payload.

The three deployment, instance, and evidence relationships are exact clones of
the ProgramRoot-v2 unbound, acyclic placeholders. V3 neither binds nor creates
any runtime root.

## Replay and diagnostics

`replay` validates the expected v3 digest, byte limit, UTF-8, canonical JSON,
closed fixed fields, twelve-segment order, inherited relationships, and
self-authenticated v3 identity. It then freshly derives ProgramRoot v2 and v3
from the typed inputs and exact-compares the digest and complete submitted
bytes.

| Code | Meaning |
| --- | --- |
| `SPX-G580` | Malformed, noncanonical, internally inconsistent, reordered, or over-bound ProgramRoot-v3 material. |
| `SPX-G581` | Stale v3 identity, self-consistently reminted material, Project/fact association, or exact replay mismatch. |

ProgramRoot v1/v2, segment, relationship, interface/artifact, Project Lock,
fact, and canonical-workspace diagnostics propagate when the respective owning
layer rejects first.

## Compatibility and nonclaims

The exact ordered nonclaims are:

```text
additive_successor_of_program_root_v2
first_eleven_segments_are_exact_program_root_v2_descriptors
contracts_and_tests_facts_segment_is_a_descriptor_not_node_payload
program_root_v1_v2_and_canonical_workspace_identities_are_unchanged
runtime_root_relationships_are_unbound_acyclic_placeholders
no_filesystem_network_execution_deployment_publication_or_commit_authority
```

V3 does not replace the frozen Canonical Semantic Workspace v1
`ContractsAndTests` node. It associates the richer typed facts as an appended
descriptor so old workspace and root identities remain valid and byte-stable.
It adds no source mutation, test/build/run route, contract discharge, coverage
analysis, external principal authentication, lock acquisition, deployment,
evidence, commit, or publication capability.

## Focused local evidence

The current two-case Workspace selector passes locally. It exercises exact
derivation and replay; independent ProgramRoot-v2 derivation; byte-identical
preservation of the first eleven descriptors and all relationships; the exact
appended fact descriptor; cross-Project fact rejection; stale selection; a
self-consistently reminted segment reorder; a self-consistently reminted change
to the appended descriptor; the complete-root byte ceiling; and unchanged
Canonical Semantic Workspace Revision v1 and ProgramRoot v1/v2 bytes:

```sh
cargo test --locked -p semaprax --test workspace program_root_v3::
```

This is bounded association evidence only. The additive [Exact Program Context
v2](EXACT-PROGRAM-CONTEXT-V2.md) now supplies a separate typed selection layer
for this exact root without changing ProgramRoot-v3 bytes. Candidate-v3
identity, exact service refresh, execution, and publication remain closed.
