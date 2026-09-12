# cli_driver/project_scaffold_options.rs

- parse · function · L5-L67 — pub(super) fn parse(arguments: &[String]) -> Result<(&str, &str, project::ScaffoldLayout), u8>
- tests · module · L70-L184 — mod tests
- argv · function · L73-L75 — fn argv(tokens: &[&str]) -> Vec<String>
- name_is_required_and_the_other_two_options_have_defaults · function · L78-L90 — fn name_is_required_and_the_other_two_options_have_defaults()
- every_admitted_template_and_layout_value_is_stored · function · L93-L105 — fn every_admitted_template_and_layout_value_is_stored()
- option_order_does_not_change_the_parse · function · L108-L126 — fn option_order_does_not_change_the_parse()
- a_repeated_option_is_an_error_rather_than_last_or_first_wins · function · L129-L139 — fn a_repeated_option_is_an_error_rather_than_last_or_first_wins()
- a_value_taking_option_never_swallows_the_following_flag · function · L142-L157 — fn a_value_taking_option_never_swallows_the_following_flag()
- unknown_flags_and_unexpected_positionals_are_refused · function · L160-L165 — fn unknown_flags_and_unexpected_positionals_are_refused()
- template_and_layout_vocabularies_are_closed_and_case_sensitive · function · L168-L183 — fn template_and_layout_vocabularies_are_closed_and_case_sensitive()
