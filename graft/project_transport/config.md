# project_transport/config.rs

- DEFAULT_MANIFEST · constant · L6-L6 — const DEFAULT_MANIFEST: &str = "semaprax.toml";
- ServerConfig · struct · L9-L13 — pub(crate) struct ServerConfig
- ServerProfile · enum · L16-L22 — pub(crate) enum ServerProfile
- accepts_project_profile · function · L25-L42 — pub(crate) const fn accepts_project_profile(
- parse · function · L46-L153 — pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, String>
- manifest_path · function · L155-L157 — pub(crate) fn manifest_path(&self) -> &std::path::Path
- limits · function · L159-L161 — pub(crate) const fn limits(&self) -> StdioLimits
- profile · function · L163-L165 — pub(crate) const fn profile(&self) -> ServerProfile
- required_path · function · L168-L179 — fn required_path(
- required_number · function · L181-L200 — fn required_number(
- tests · module · L203-L338 — mod tests
- parse · function · L207-L209 — fn parse(arguments: &[&str]) -> Result<ServerConfig, String>
- stdio_defaults_bind_one_fixed_manifest · function · L212-L216 — fn stdio_defaults_bind_one_fixed_manifest()
- mutation_authority_is_explicit_and_nonrepeating · function · L219-L288 — fn mutation_authority_is_explicit_and_nonrepeating()
- startup_authority_is_closed_and_nonrepeating · function · L291-L306 — fn startup_authority_is_closed_and_nonrepeating()
- public_api_profile_accepts_exactly_project_v8_through_v11 · function · L309-L337 — fn public_api_profile_accepts_exactly_project_v8_through_v11()
