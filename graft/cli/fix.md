# cli/fix.rs

- FixCommand · enum · L10-L16 — pub(crate) enum FixCommand
- parse · function · L18-L36 — pub(crate) fn parse(args: &[String]) -> Result<FixCommand, u8>
- run · function · L38-L56 — pub(crate) fn run(command: FixCommand, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8>
- usage · function · L58-L63 — fn usage<T>() -> Result<T, u8>
- tests · module · L66-L122 — mod tests
- strings · function · L69-L71 — fn strings(values: &[&str]) -> Vec<String>
- grammar_is_closed · function · L74-L121 — fn grammar_is_closed()
