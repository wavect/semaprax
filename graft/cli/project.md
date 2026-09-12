# cli/project.rs

- DEFAULT_MANIFEST · constant · L3-L3 — pub(crate) const DEFAULT_MANIFEST: &str = "semaprax.toml";
- CheckInput · enum · L6-L9 — pub(crate) enum CheckInput
- CheckOptions · struct · L12-L15 — pub(crate) struct CheckOptions
- parse_check_options · function · L17-L75 — pub(crate) fn parse_check_options(args: &[String]) -> Result<CheckOptions, u8>
- is_project_manifest · function · L77-L79 — pub(crate) fn is_project_manifest(path: &Path) -> bool
- resolve_positional · function · L87-L98 — pub(crate) fn resolve_positional(path: PathBuf) -> PathBuf
- normalize_project_path · function · L100-L119 — pub(crate) fn normalize_project_path(path: PathBuf) -> PathBuf
- tests · module · L122-L233 — mod tests
- strings · function · L125-L127 — fn strings(values: &[&str]) -> Vec<String>
- project_check_selectors_preserve_legacy_source_detection · function · L130-L199 — fn project_check_selectors_preserve_legacy_source_detection()
- directory_operand_selects_the_manifest_inside_it · function · L202-L232 — fn directory_operand_selects_the_manifest_inside_it()
