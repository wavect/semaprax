# cli/agent.rs

- USAGE · constant · L24-L24 — const USAGE: &str = "agent accepts exactly `inspect <definition.json> [--profile]`, `run <definition.json> <task.json> <transcript.json> [--evidence|--trace]`, `replay <definition.json> <task.json> <transcript.json> <evidence.json>`, or `skill [--require-schema <schema>]`; `resume` and `reconcile` are not admitted";
- RunOutput · enum · L26-L30 — pub(crate) enum RunOutput
- AgentCommand · enum · L32-L52 — pub(crate) enum AgentCommand
- usage · function · L54-L57 — fn usage() -> u8
- operand · function · L59-L64 — fn operand(value: &str) -> Result<PathBuf, u8>
- parse · function · L66-L141 — pub(crate) fn parse(args: &[String]) -> Result<AgentCommand, u8>
- read · function · L143-L150 — fn read(path: &Path) -> Result<String, Vec<Diagnostic>>
- run · function · L152-L198 — pub(crate) fn run(command: &AgentCommand) -> Result<String, Vec<Diagnostic>>
- tests · module · L201-L330 — mod tests
- strings · function · L204-L206 — fn strings(values: &[&str]) -> Vec<String>
- agent_grammar_is_closed · function · L209-L294 — fn agent_grammar_is_closed()
- skill_run_prints_the_agent_skill_bundle_and_negotiates_its_schema · function · L297-L309 — fn skill_run_prints_the_agent_skill_bundle_and_negotiates_its_schema()
- public_workflow_commands_are_all_catalogued · function · L319-L329 — fn public_workflow_commands_are_all_catalogued()
