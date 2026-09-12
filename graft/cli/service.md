# cli/service.rs

- ServiceOptions · struct · L12-L15 — pub(crate) struct ServiceOptions
- parse · function · L17-L32 — pub(crate) fn parse(args: &[String]) -> Result<ServiceOptions, u8>
- run · function · L34-L54 — pub(crate) fn run(options: ServiceOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8>
- usage · function · L56-L59 — fn usage<T>() -> Result<T, u8>
- tests · module · L62-L109 — mod tests
- strings · function · L65-L67 — fn strings(values: &[&str]) -> Vec<String>
- grammar_is_closed · function · L70-L108 — fn grammar_is_closed()
