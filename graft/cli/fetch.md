# cli/fetch.rs

- USAGE · constant · L15-L15 — const USAGE: &str = "fetch requires exactly <cache-dir> <subject.json>...";
- CODE · constant · L16-L16 — const CODE: &str = "SPX-J128";
- FetchOptions · struct · L18-L21 — pub(crate) struct FetchOptions
- parse · function · L23-L37 — pub(crate) fn parse(args: &[String]) -> Result<FetchOptions, u8>
- Filed · struct · L39-L44 — struct Filed
- cache_error · function · L46-L48 — fn cache_error(message: String) -> Vec<Diagnostic>
- read_subject · function · L50-L61 — fn read_subject(path: &Path) -> Result<String, Vec<Diagnostic>>
- run · function · L65-L191 — pub(crate) fn run(options: &FetchOptions) -> Result<String, Vec<Diagnostic>>
- tests · module · L194-L218 — mod tests
- strings · function · L197-L199 — fn strings(values: &[&str]) -> Vec<String>
- fetch_grammar_is_closed · function · L202-L217 — fn fetch_grammar_is_closed()
