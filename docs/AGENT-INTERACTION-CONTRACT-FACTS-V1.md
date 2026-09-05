# Agent Interaction Contract Facts v1

Status: source-owned, additive, authority-free compiler fact bundle.

Audience: compiler, Project, ProgramRoot, query, and semantic-service maintainers.

`semaprax.agent-interaction-contract-facts.v1` binds each compiler-admitted
source Agent to the exact Proposal Schema v1 and Observation Schema v1 derived
from its checked module. `ProjectRevision` retains the bundle in stable Agent-ID
byte order together with its Project revision, source-workspace revision, and
Project graph digest.

Both role IDs must resolve to actual supported record or variant declarations
in the same checked module. Construction independently replays both canonical
schema documents and rejects a missing Agent, stale definition digest,
duplicate or unordered identity, unsupported shape, or a bundle larger than 16
MiB. The Project-owned caller supplies freshly derived Project anchors;
canonical workspace publication validates those retained anchors against the
authoritative revision. Embedded schemas are exact bounded bytes, not
interpreted authority.

Only the `source_owned_spx_agent_declarations` population path adds an
`interaction_contract` object to each existing AgentDefinitions row. The
explicit AgentDefinition association path and Projects without source Agents
remain byte-identical. ProgramRoot v1 and v2 need no new segment: they already
content-address the populated AgentDefinitions node.

The typed `AgentDefinitionsQuery` result, service generation, and immutable
service snapshot expose the retained bundle in memory. No Universal Semantic
Query v1, service transport, Project Agent Transport v5, or legacy JSON grammar
changes. The bundle grants no provider, tool, filesystem, network, execution,
authorization, commit, deployment, or publication capability.

Focused evidence lives in the existing
`agent_runtime_v1::source_agent_lowering` harness. It compares the embedded
Proposal and Observation bytes and digests to direct compiler derivation, then
selects the same AgentDefinitions node and facts through Project, ProgramRoot,
typed query, and semantic service while preserving the frozen AgentDefinition,
AgentGraph, and Runtime Profile outputs.
