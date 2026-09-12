# agent_lifecycle_typed_carrier/binding.rs

- LifecycleStageRole · enum · L34-L42 — pub enum LifecycleStageRole
- name · function · L46-L54 — pub fn name(self) -> &'static str
- StageBinding · struct · L63-L68 — pub struct StageBinding
- new · function · L74-L81 — pub fn new(role: LifecycleStageRole, schema: &CompiledInteractionSchema) -> Self
- expect_case · function · L87-L90 — pub fn expect_case(mut self, case: impl Into<String>) -> Self
- role · function · L93-L95 — pub fn role(&self) -> &str
- root_type_id · function · L98-L100 — pub fn root_type_id(&self) -> &str
- schema_digest · function · L103-L105 — pub fn schema_digest(&self) -> &str
- expected_case · function · L108-L110 — pub fn expected_case(&self) -> Option<&str>
- admit · function · L125-L142 — pub fn admit(
