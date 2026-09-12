# cli/fmt.rs

- FmtInput · enum · L19-L22 — pub(crate) enum FmtInput
- FmtOptions · struct · L25-L28 — pub(crate) struct FmtOptions
- parse · function · L30-L53 — pub(crate) fn parse(args: &[String]) -> Result<FmtOptions, u8>
- Formatted · struct · L56-L60 — struct Formatted
- run · function · L64-L111 — pub(crate) fn run(options: FmtOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8>
- first_differing_line · function · L115-L126 — fn first_differing_line(source: &str, canonical: &str) -> usize
- project_sources · function · L130-L153 — fn project_sources(
- reject_symlink_components · function · L158-L187 — fn reject_symlink_components(path: &Path) -> Result<(), Diagnostic>
- metadata_is_reparse · function · L190-L193 — fn metadata_is_reparse(metadata: &std::fs::Metadata) -> bool
- metadata_is_reparse · function · L196-L198 — fn metadata_is_reparse(_: &std::fs::Metadata) -> bool
- tests · module · L201-L284 — mod tests
- strings · function · L204-L206 — fn strings(values: &[&str]) -> Vec<String>
- formatter_grammar_is_closed · function · L209-L250 — fn formatter_grammar_is_closed()
- differing_line_includes_content_and_eof_drift · function · L253-L257 — fn differing_line_includes_content_and_eof_drift()
- directory_operand_selects_the_manifest_inside_it · function · L260-L283 — fn directory_operand_selects_the_manifest_inside_it()
