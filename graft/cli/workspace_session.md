# cli/workspace_session.rs

- run · function · L14-L17 — pub(crate) fn run(manifest: &Path, policy_path: &Path) -> Result<(), Vec<Diagnostic>>
- run_mcp · function · L19-L22 — pub(crate) fn run_mcp(manifest: &Path, policy_path: &Path) -> Result<(), Vec<Diagnostic>>
- open_session · function · L24-L165 — fn open_session(manifest: &Path, policy_path: &Path) -> Result<VNextSession, Vec<Diagnostic>>
- session_failure · function · L167-L179 — fn session_failure(error: std::io::Error) -> Vec<Diagnostic>
- cache_policy · function · L182-L335 — fn cache_policy(value: &Value) -> Result<(bool, bool), Vec<Diagnostic>>
- COMMON · constant · L183-L190 — const COMMON: &[&str] = &[
- exact · function · L336-L344 — fn exact(value: &Value, keys: &[&str]) -> Result<(), Vec<Diagnostic>>
- string · function · L345-L349 — fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Vec<Diagnostic>>
- integer · function · L350-L354 — fn integer(value: &Value, key: &str) -> Result<u64, Vec<Diagnostic>>
- size · function · L355-L357 — fn size(value: &Value, key: &str) -> Result<usize, Vec<Diagnostic>>
- boolean · function · L358-L362 — fn boolean(value: &Value, key: &str) -> Result<bool, Vec<Diagnostic>>
- invalid · function · L363-L365 — fn invalid(message: &'static str) -> Vec<Diagnostic>
