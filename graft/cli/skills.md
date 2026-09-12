# cli/skills.rs

- SkillsGet · struct · L6-L8 — pub(crate) struct SkillsGet
- parse · function · L10-L24 — pub(crate) fn parse(args: &[String]) -> Result<SkillsGet, u8>
- run · function · L26-L30 — pub(crate) fn run(request: SkillsGet, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8>
- usage · function · L32-L35 — fn usage<T>() -> Result<T, u8>
- tests · module · L38-L60 — mod tests
- strings · function · L41-L43 — fn strings(values: &[&str]) -> Vec<String>
- installed_skill_grammar_is_closed · function · L46-L59 — fn installed_skill_grammar_is_closed()
