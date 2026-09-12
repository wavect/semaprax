# execution_revision/typed.rs

- MAX_ITERATIVE_PROPOSAL_BYTES · constant · L13-L13 — pub const MAX_ITERATIVE_PROPOSAL_BYTES: usize = 2 * 1024 * 1024;
- AgentRuntimeV2 · struct · L15-L26 — pub struct AgentRuntimeV2
- deployment_root · function · L28-L30 — pub fn deployment_root(&self) -> &ExecutionRoot
- instance_root · function · L31-L33 — pub fn instance_root(&self) -> &ExecutionRoot
- execution_revision · function · L34-L36 — pub fn execution_revision(&self) -> &ExecutionRoot
- project_revision · function · L37-L39 — pub fn project_revision(&self) -> &ProjectRevision
- run · function · L40-L66 — pub fn run(
- AgentRuntimeV2Evidence · struct · L69-L73 — pub struct AgentRuntimeV2Evidence
- run · function · L75-L77 — pub fn run(&self) -> &TypedEffectRun
- evidence_root · function · L78-L80 — pub fn evidence_root(&self) -> &ExecutionRoot
- execution_revision · function · L81-L83 — pub fn execution_revision(&self) -> &ExecutionRoot
- bind_agent_runtime_v2 · function · L90-L121 — pub fn bind_agent_runtime_v2(
- bind_linked_agent_runtime_v2 · function · L125-L156 — pub fn bind_linked_agent_runtime_v2(
- bind_runtime · function · L159-L327 — fn bind_runtime(
- durable · module · L330-L330 — mod durable;
- migration · module · L334-L334 — pub(crate) mod migration;
