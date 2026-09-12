# agent_proposal/runtime_v1.rs

- ACTION_SCHEMA · constant · L14-L14 — const ACTION_SCHEMA: &str = "semaprax.agent-runtime-action.v1";
- AgentRuntimeV1ActionKind · enum · L18-L21 — pub enum AgentRuntimeV1ActionKind
- AgentRuntimeV1ActionBytes · struct · L28-L30 — pub struct AgentRuntimeV1ActionBytes
- canonical_json · function · L35-L37 — pub fn canonical_json(&self) -> &str
- kind · function · L41-L43 — pub fn kind(&self) -> AgentRuntimeV1ActionKind
- AgentProposalRuntimeV1Compatibility · struct · L51-L58 — pub struct AgentProposalRuntimeV1Compatibility<'a>
- definition_digest · function · L63-L65 — pub fn definition_digest(&self) -> &str
- proposal_schema_digest · function · L69-L71 — pub fn proposal_schema_digest(&self) -> &str
- runtime_profile_digest · function · L75-L77 — pub fn runtime_profile_digest(&self) -> &str
- decode_and_render · function · L85-L104 — pub fn decode_and_render(
- compile_agent_proposal_runtime_v1_compatibility · function · L112-L138 — pub fn compile_agent_proposal_runtime_v1_compatibility<'a>(
- incompatible · function · L140-L145 — fn incompatible(field: &str) -> Diagnostic
