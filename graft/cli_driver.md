---
covers: []
---
# cli_driver.rs

- cli · module · L22-L22 — mod cli;
- native_scratch · module · L24-L24 — mod native_scratch;
- options · module · L26-L26 — mod options;
- project_scaffold_options · module · L28-L28 — mod project_scaffold_options;
- report_options · module · L30-L30 — mod report_options;
- source_execution · module · L32-L32 — mod source_execution;
- supply_chain · module · L34-L34 — mod supply_chain;
- native_output_tests · module · L38-L38 — mod native_output_tests;
- native_scratch_tests · module · L46-L46 — mod native_scratch_tests;
- NewProjectHook · type · L51-L51 — pub type NewProjectHook = fn(&[String]) -> Result<(PathBuf, &'static str), (String, u8)>;
- PrivateHost · struct · L53-L58 — pub struct PrivateHost
- CLI_STACK_BYTES · constant · L63-L63 — const CLI_STACK_BYTES: usize = 16 * 1024 * 1024;
- main_with_host · function · L65-L84 — pub fn main_with_host(host: Option<&'static PrivateHost>) -> ExitCode
- require_private_host · function · L86-L94 — fn require_private_host<'a>(
- run · function · L96-L1481 — fn run(args: Vec<String>, host: Option<&PrivateHost>) -> Result<(), u8>
- print_help · function · L1483-L1485 — fn print_help(has_private_host: bool)
- print_scoped_help · function · L1487-L1500 — fn print_scoped_help(command: &str, has_private_host: bool) -> Result<(), u8>
