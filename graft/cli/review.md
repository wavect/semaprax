# cli/review.rs

- USAGE · constant · L11-L12 — const USAGE: &str =
- ReviewInput · enum · L14-L24 — enum ReviewInput
- ReviewError · enum · L26-L29 — pub(crate) enum ReviewError
- from · function · L32-L34 — fn from(diagnostics: Vec<Diagnostic>) -> Self
- run · function · L37-L58 — pub(crate) fn run(args: &[String]) -> Result<String, ReviewError>
- parse · function · L60-L87 — fn parse(args: &[String]) -> Result<ReviewInput, ()>
- read_transaction · function · L89-L112 — fn read_transaction(path: &Path) -> Result<Vec<u8>, Vec<Diagnostic>>
- tests · module · L115-L133 — mod tests
- strings · function · L118-L120 — fn strings(values: &[&str]) -> Vec<String>
- grammar_preserves_legacy_and_closes_evidence_mode · function · L123-L132 — fn grammar_preserves_legacy_and_closes_evidence_mode()
