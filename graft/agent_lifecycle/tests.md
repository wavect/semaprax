# agent_lifecycle/tests.rs

- RUNTIME_V1 · constant · L14-L36 — pub(in crate::agent_lifecycle) const RUNTIME_V1: &str = concat!(
- DEFINITION · constant · L38-L55 — pub(in crate::agent_lifecycle) const DEFINITION: &str = concat!(
- MODULE · constant · L57-L147 — pub(in crate::agent_lifecycle) const MODULE: &str = r#"module fixture.agent.lifecycle;
- CountingRead · struct · L149-L149 — struct CountingRead(usize);
- read · function · L152-L155 — fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>>
- lifecycle · function · L158-L165 — fn lifecycle() -> CompiledAgentLifecycle
- proposal · function · L167-L184 — pub(in crate::agent_lifecycle) fn proposal(
- authorize · function · L188-L224 — fn authorize(
- the_derived_stage_graph_is_acyclic_and_its_order_is_unique · function · L227-L247 — fn the_derived_stage_graph_is_acyclic_and_its_order_is_unique()
- an_authorization_is_not_spendable_into_a_substituted_state_or_proposal · function · L250-L287 — fn an_authorization_is_not_spendable_into_a_substituted_state_or_proposal()
- the_authorization_binding_separates_policy_state_proposal_case_and_seal · function · L290-L319 — fn the_authorization_binding_separates_policy_state_proposal_case_and_seal()
- the_authorization_value_has_exactly_one_mint_site_in_the_crate · function · L322-L392 — fn the_authorization_value_has_exactly_one_mint_site_in_the_crate()
