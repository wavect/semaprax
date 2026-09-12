# parser/agents.rs

- MAX_RUNTIME_V1_JSON_BYTES · constant · L12-L12 — const MAX_RUNTIME_V1_JSON_BYTES: usize = 1_310_720;
- TYPE_ROLES · constant · L13-L20 — const TYPE_ROLES: [(&str, AgentTypeRole); 6] = [
- OPERATIONS · constant · L21-L52 — const OPERATIONS: [(&str, AgentOperationRole, AgentOperationKind); 6] = [
- agent · function · L55-L171 — pub(super) fn agent(
- required_role_id · function · L173-L180 — fn required_role_id(&mut self, subject: &'static str) -> Result<String, Diagnostic>
