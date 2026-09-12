# public_generic_settlement/tests.rs

- SOURCE · constant · L11-L52 — const SOURCE: &str = r#"
- program · function · L54-L57 — fn program() -> ResolvedProgram
- function · function · L59-L68 — fn function(program: &ResolvedProgram, id: &str) -> &'static ResolvedFunction
- obligations_are_the_checked_owned_leaves_in_agreed_order · function · L74-L116 — fn obligations_are_the_checked_owned_leaves_in_agreed_order()
- the_transfer_unit_is_the_whole_owned_parameter · function · L123-L141 — fn the_transfer_unit_is_the_whole_owned_parameter()
- derivation_is_deterministic · function · L145-L153 — fn derivation_is_deterministic()
- unsupported_parameters_fail_closed · function · L158-L176 — fn unsupported_parameters_fail_closed()
- a_disagreement_with_the_cleanup_facts_is_refused · function · L182-L215 — fn a_disagreement_with_the_cleanup_facts_is_refused()
- a_relabelled_liveness_flag_is_refused · function · L220-L253 — fn a_relabelled_liveness_flag_is_refused()
- a_retyped_storage_slot_is_refused · function · L258-L286 — fn a_retyped_storage_slot_is_refused()
