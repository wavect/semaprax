# Language-native Agent lowering v1

Status: locally exercised bounded AGENT-03 semantic-lowering and Project
integration tranche. Agent role execution and installed-product promotion
remain separate gates.

Audience: compiler contributors, semantic-workspace integrators, and reviewers.

The lowering consumes the closed `.spx` `AgentDeclaration` AST produced by the
frontend. It does not define another Agent artifact. Instead it renders the
exact existing `semaprax.agent-definition.v1` canonical JSON and immediately
re-admits those bytes through `compile_agent_definition`. The resulting
AgentDefinition v1, AgentGraph v1, and Runtime Profile v1 are therefore the
existing compiler products, not similar replacements.

Each Agent must contain exactly the six type roles in order:
`task`, `state`, `observation`, `proposal`, `outcome`, `result`. It must contain
exactly the six operation roles and fixed kinds in order:
`initialize` deterministic, `observe` deterministic, `propose` model,
`authorize` deterministic, `execute` effect, and `reduce` deterministic. Missing,
duplicate, reordered, or misclassified roles fail before compatibility
compilation. Agent, type, and operation stable identities must be unique within
the declaration and across the complete supplied Project Program set.

The display name does not enter AgentDefinition bytes, so a display-only rename
preserves all three compatibility outputs. The `runtime_v1_json` AST field is
the exact canonical object admitted by the source grammar. Lowering neither
normalizes nor repairs it; malformed or noncanonical content is rejected by the
lowering and existing AgentDefinition compiler.

`compile_source_agent_declaration` lowers one declaration.
`compile_source_program_agents` lowers one Program.
`compile_source_project_agents` checks the Project-wide identity inventory and
returns `CompiledSourceAgents` in stable Agent-ID byte order. Its
`hir::ResolvedAgentDeclaration` is a real field of every `ResolvedProgram` and
is retained through resolver, workspace linking, and the private HIR cache. It
carries the display/stable Agent identity, all typed role/kind identities, and
the exact Runtime-v1 carrier. Compatibility definition/graph digests remain in
the Project-owned compiled products. The HIR node deliberately carries no
executable role body until that separately specified lowering exists.
Project-wide validation also rejects an Agent,
Agent-type, or Agent-operation identity that collides with a declaration
identity already retained by the exact Project graph. Its
Project construction retains those products on the immutable `ProjectRevision`,
and default canonical derivation populates the existing AgentDefinitions node.
An empty Program set retains the legacy empty/default canonical workspace bytes.

`SPX-G558` reports malformed source-lowering carrier material. `SPX-G559`
reports role, kind, capacity, identity, or Project-association invariants.
Existing `SPX-G501` through `SPX-G504` remain authoritative for the final
AgentDefinition/Profile/Graph compatibility admission and replay.

Lowering is pure. It invokes no provider or tool, reads no path or environment,
executes no Agent role, mints no `Authorized<T>`, and grants no filesystem,
network, process, deployment, approval, signing, commit, or publication
authority. Runtime execution and durable Agent instances remain outside this
tranche.

The focused `agent_runtime_v1::source_agent_lowering` tests bind the existing
fixture's exact definition and graph known-answer digests and byte-identical
Runtime Profile, prove display-name independence and canonical Project ordering,
and reject missing/duplicate roles, wrong operation kinds, local identity
collisions, and cross-module collisions. The three focused cases pass locally.
