# cli/add.rs

- USAGE · constant · L12-L12 — const USAGE: &str = "add requires exactly <dir>|semaprax.toml <package> <range>";
- AddOptions · struct · L14-L18 — pub(crate) struct AddOptions
- parse · function · L20-L38 — pub(crate) fn parse(args: &[String]) -> Result<AddOptions, u8>
- run · function · L42-L82 — pub(crate) fn run(options: &AddOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8>
- tests · module · L85-L110 — mod tests
- strings · function · L88-L90 — fn strings(values: &[&str]) -> Vec<String>
- add_grammar_is_closed · function · L93-L109 — fn add_grammar_is_closed()
