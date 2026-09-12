# cli/doc.rs

- DocOptions · struct · L9-L12 — pub(crate) struct DocOptions
- USAGE · constant · L14-L14 — const USAGE: &str = "doc requires exactly <file> [--json]";
- parse · function · L16-L42 — pub(crate) fn parse(args: &[String]) -> Result<DocOptions, u8>
- run · function · L46-L66 — pub(crate) fn run(options: DocOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8>
- tests · module · L69-L95 — mod tests
- strings · function · L72-L74 — fn strings(values: &[&str]) -> Vec<String>
- doc_grammar_is_closed · function · L77-L94 — fn doc_grammar_is_closed()
