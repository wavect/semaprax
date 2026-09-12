# cli_driver/source_execution.rs

- checked · function · L5-L7 — pub(super) fn checked(path: &Path) -> Result<semaprax::ast::Program, u8>
- checked_for_output · function · L9-L17 — pub(super) fn checked_for_output(path: &Path, json: bool) -> Result<semaprax::ast::Program, u8>
- load · function · L19-L27 — pub(super) fn load(path: &Path) -> Result<semaprax::ast::Program, Vec<Diagnostic>>
- required_path · function · L29-L34 — pub(super) fn required_path(args: &[String], index: usize) -> Result<PathBuf, u8>
- build_source · function · L36-L117 — pub(super) fn build_source(options: &cli::build::BuildOptions, input: &Path) -> Result<(), u8>
- report_source_build_success · function · L119-L139 — pub(super) fn report_source_build_success(
- run_native_source · function · L141-L177 — pub(super) fn run_native_source(path: &Path) -> Result<(), u8>
- run_interpreted_source · function · L179-L213 — pub(super) fn run_interpreted_source(
- run_network_project · function · L215-L312 — pub(super) fn run_network_project(options: &cli::execution::NetworkRunOptions) -> Result<(), u8>
- MAX_COMMAND_INPUT_BYTES · constant · L218-L218 — const MAX_COMMAND_INPUT_BYTES: usize = 65_536;
- read_bounded_file · function · L314-L346 — fn read_bounded_file(path: &Path, max_bytes: usize, label: &str) -> Result<Vec<u8>, u8>
- publish_interpretation · function · L348-L393 — pub(super) fn publish_interpretation(envelope: &str) -> Result<(), u8>
- publish_interpreted_stdout · function · L395-L480 — pub(super) fn publish_interpreted_stdout(
- report · function · L482-L485 — pub(super) fn report(errors: &[Diagnostic], json: bool) -> u8
- report_all · function · L487-L495 — pub(super) fn report_all(errors: &[Diagnostic], json: bool)
