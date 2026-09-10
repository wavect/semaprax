# Language-native Agent lowering v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, physical/provisioned, benchmark, or exact-subject
observations below retain their narrower scope. Public promotion, registry
publication and broader product completion remain separately gated.

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
copied executable role body. The additive `compile_source_agent_lifecycle`
bridge instead selects one checked Agent by stable identity, lowers that exact
declaration through the frozen AgentDefinition-v1 compiler, and lets Agent
Lifecycle v1 resolve its four deterministic role identities against the same
immutable source module and ordinary HIR. Callers therefore cannot pair source
roles with a separately mutable definition document.
Project-wide validation also rejects an Agent declaration identity that
collides with an ordinary declaration identity already retained by the exact
Project graph. Type-role and operation-role identities are bindings and may
name compatible persistent type or function declarations. The twelve role
bindings remain unique within one source Agent definition and distinct from
that Agent declaration identity; multiple Agents may bind the same ordinary
declarations. Project construction retains those
products on the immutable `ProjectRevision`,
and default canonical derivation populates the existing AgentDefinitions node.
An empty Program set retains the legacy empty/default canonical workspace bytes.
For source-owned Agents whose Proposal and Observation roles resolve to checked
same-module declarations, Project construction also retains the independently
replayed [Agent Interaction Contract Facts v1](AGENT-INTERACTION-CONTRACT-FACTS-V1.md)
bundle and embeds those exact bounded schema facts in the source-only
AgentDefinitions rows. Explicit association bytes remain unchanged.

`SPX-G558` reports malformed source-lowering carrier material. `SPX-G559`
reports role, kind, capacity, identity, or Project-association invariants.
Existing `SPX-G501` through `SPX-G504` remain authoritative for the final
AgentDefinition/Profile/Graph compatibility admission and replay.

Lowering and bridge compilation are pure. They invoke no provider or tool,
read no path or environment, execute no Agent role, mint no `Authorized<T>`,
and grant no filesystem, network, process, deployment, approval, signing,
commit, or publication authority. A caller may execute the resulting existing
Lifecycle v1 value only by explicitly supplying its ordinary task, proposal,
budget, cancellation, and injected read operation.

The focused `agent_runtime_v1::source_agent_lowering` tests bind the existing
fixture's exact definition and graph known-answer digests and byte-identical
Runtime Profile, prove display-name independence and canonical Project ordering,
and reject missing/duplicate roles, wrong operation kinds, local identity
collisions, and cross-module collisions. The three focused cases pass locally.

The additive `agent_runtime_v1::source_agent_lifecycle` gate proves exact
source-Agent selection, byte parity with the existing lifecycle compiler, one
successful acyclic pass, refusal and injected-effect failure, and stable
missing-Agent or incompatible-role rejection. Lifecycle replay additionally binds
the checked module's semantic revision, so a role-body change fails closed even
when the frozen Lifecycle v1 document intentionally remains byte-identical.
