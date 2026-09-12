# agent_lifecycle/rich_stage/tests.rs

- FIXTURE · constant · L25-L99 — const FIXTURE: &str = r#"
- RENAMED_FIXTURE · constant · L104-L117 — const RENAMED_FIXTURE: &str = r#"
- STRUCTURAL_FIXTURE · constant · L121-L136 — const STRUCTURAL_FIXTURE: &str = r#"
- write_temp · function · L138-L149 — fn write_temp(source: &str, label: &str) -> PathBuf
- bind · function · L151-L162 — fn bind(label: &str) -> RichProposalStages
- state · function · L164-L172 — fn state(count: i64) -> RetainedValue
- proposal_document · function · L177-L182 — fn proposal_document(schema_digest: &str, urgent: bool, weight: i64) -> String
- granted_urgent_proposal_reaches_reduce_and_the_harvested_next_state_is_exact · function · L185-L215 — fn granted_urgent_proposal_reaches_reduce_and_the_harvested_next_state_is_exact()
- non_urgent_proposal_is_refused_before_reduce_ever_dispatches · function · L218-L234 — fn non_urgent_proposal_is_refused_before_reduce_ever_dispatches()
- granted_proposal_with_a_negative_weight_reaches_reduce_and_fails_with_the_exact_code · function · L237-L251 — fn granted_proposal_with_a_negative_weight_reaches_reduce_and_fails_with_the_exact_code()
- malformed_and_wrong_nominal_and_stale_schema_proposals_refuse_before_any_stage_evaluates · function · L254-L301 — fn malformed_and_wrong_nominal_and_stale_schema_proposals_refuse_before_any_stage_evaluates()
- cancellation_before_dispatch_refuses_before_any_decode · function · L304-L319 — fn cancellation_before_dispatch_refuses_before_any_decode()
- a_display_only_rename_keeps_the_same_schema_digest_but_a_structural_edit_changes_it · function · L322-L361 — fn a_display_only_rename_keeps_the_same_schema_digest_but_a_structural_edit_changes_it()
