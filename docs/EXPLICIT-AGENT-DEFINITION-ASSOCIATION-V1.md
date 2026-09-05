# Explicit AgentDefinition Association v1

Status: bounded SEG-02 bridge; implementation and focused evidence pass
locally.

Audience: compiler contributors, semantic-workspace integrators, Agent-runtime
authors, and reviewers of ProgramRoot identity.

## Purpose and boundary

SEMAPRAX has a canonical AgentDefinition v1 compiler, but Project source and
manifests do not yet contain Agent declarations. Inventing Agent facts from
ordinary functions would falsely claim the language integration reserved for
AGENT-03. This bridge instead lets a caller explicitly associate already
compiler-admitted AgentDefinition bundles with one exact admitted Project when
deriving its Canonical Semantic Workspace Revision.

The association is explicit input. It is not proof that the Agent definitions
originated in the Project, is not `.spx` Agent syntax, and does not make the
definitions language-native or executable. The resulting non-empty
`AgentDefinitions` node flows automatically into ProgramRoot's existing
content-addressed `agent_definitions` segment.

## API and preconditions

```rust
SemanticWorkspaceRevision::derive_with_agent_definitions(
    revision: &ProjectRevision,
    expected_project_revision: &str,
    definitions: &[&CompiledAgentDefinition],
) -> Result<SemanticWorkspaceRevision, Vec<Diagnostic>>;

SemanticWorkspaceRevision::replay_with_agent_definitions(
    revision: &ProjectRevision,
    expected_project_revision: &str,
    definitions: &[&CompiledAgentDefinition],
    expected_workspace_revision: &str,
    bytes: &[u8],
) -> Result<SemanticWorkspaceRevision, Vec<Diagnostic>>;
```

The expected Project revision is a lowercase canonical SHA-256 identity and
must equal the retained Project exactly. Input contains 1–64 definitions, is
already in strictly increasing stable agent-ID byte order, and has no duplicate
agent identity. The combined exact definition, graph, and Runtime Profile bytes
are capped at 8 MiB before the existing 32 MiB canonical-workspace cap.

Every entry is independently recompiled with
`compile_agent_definition` and checked by `verify_agent_graph_bundle`. The
compiler-owned canonical AgentDefinition bytes/digest, AgentGraph bytes/digest,
and byte-identical Runtime Profile plus its graph-bound digest must agree before
derivation. No submitted JSON is trusted as HIR or runtime state.

## Canonical payload

The `AgentDefinitions` node retains its existing schema and digest domain. The
explicit payload has exact top-level keys:

```text
definitions,expected_project_revision,integration
```

`integration` is exactly
`explicit_compiler_admitted_association_input`. Each definition row has:

```text
agent_definition,agent_definition_digest,agent_graph,agent_graph_digest,
agent_id,runtime_v1_profile,runtime_v1_profile_digest
```

The three artifacts are exact LF-terminated canonical strings, not
request-authored summaries. Their existing digests remain owned by the Agent
compiler/runtime contracts. Agent array order is the authenticated stable-ID
order and must never be sorted or repaired downstream.

Changing any associated definition changes the AgentDefinitions node digest,
semantic component digest, Canonical Workspace revision, and consequently its
ProgramRoot and AgentDefinitions segment digests. Source-projection, manifest,
and dependency-lock component identities remain separately derived.

## Compatibility and authority

The existing `SemanticWorkspaceRevision::derive`, `replay`, and
`ProjectRevision::canonical_workspace_revision` routes still produce the exact
empty payload:

```json
{"definitions":[],"integration":"no_project_agent_definition_declarations"}
```

Their existing nonclaims and bytes are unchanged. The explicit route has a
separate truthful nonclaim that compiler-admitted association input is not
`.spx` Agent syntax or intrinsic Project-ownership proof.

Neither route reads files, invokes a model, runs an Agent role, grants provider
or tool authority, mutates the Project, adopts a workspace generation, or
publishes anything. DeploymentRoot, InstanceRoot, and EvidenceRoot remain
unbound.

## Diagnostics and evidence

Existing Canonical Workspace diagnostics apply: `SPX-G222` rejects malformed,
unordered, duplicate, empty, or over-bound association input; `SPX-G223`
rejects stale Project/workspace association or exact replay mismatch. Agent
compiler diagnostics retain their existing meanings.

The focused Workspace-harness cases cover deterministic population, exact
artifact/digest association, ProgramRoot segment propagation, stale selector,
unordered/duplicate/count rejection, cross-paired replay, no writes, and exact
preservation of the default empty derivation. All three cases pass locally as
part of the 33-case combined SEG-02 focused gate.
