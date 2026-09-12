# agent_deployment/migrate.rs

- migrate_agent_definition_v1 · function · L30-L39 — pub fn migrate_agent_definition_v1(
- split · function · L41-L111 — fn split(v1_source: &str, deployment_id: &str) -> Result<(String, String), Diagnostic>
- role_ids · function · L113-L121 — fn role_ids(value: Option<&Value>, expected: usize) -> Result<Vec<String>, Diagnostic>
- text · function · L123-L128 — fn text(value: Option<&Value>) -> Result<String, Diagnostic>
- list · function · L130-L137 — fn list(value: Option<&Value>) -> Result<Vec<String>, Diagnostic>
