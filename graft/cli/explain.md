# cli/explain.rs

- ExplainOptions · struct · L6-L9 — pub(crate) struct ExplainOptions
- parse · function · L11-L28 — pub(crate) fn parse(args: &[String]) -> Result<ExplainOptions, u8>
- run · function · L30-L39 — pub(crate) fn run(options: ExplainOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8>
- usage · function · L41-L44 — fn usage<T>() -> Result<T, u8>
- tests · module · L47-L74 — mod tests
- strings · function · L50-L52 — fn strings(values: &[&str]) -> Vec<String>
- grammar_is_closed · function · L55-L73 — fn grammar_is_closed()
