# cli/new_project.rs

- NewOptions · struct · L14-L18 — pub(crate) struct NewOptions
- parse · function · L20-L66 — pub(crate) fn parse(arguments: &[String]) -> Result<NewOptions, String>
- known_template · function · L68-L78 — fn known_template(value: &str) -> Result<&'static str, String>
- option_value · function · L80-L90 — fn option_value<'a>(
- validate_name · function · L92-L106 — fn validate_name(name: &str) -> Result<(), String>
- tests · module · L109-L185 — mod tests
- strings · function · L112-L114 — fn strings(values: &[&str]) -> Vec<String>
- grammar_is_closed_and_names_default_to_the_destination_leaf · function · L117-L184 — fn grammar_is_closed_and_names_default_to_the_destination_leaf()
