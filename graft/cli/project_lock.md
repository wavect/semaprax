# cli/project_lock.rs

- ProjectLockCliError · enum · L12-L18 — pub(crate) enum ProjectLockCliError
- Mode · enum · L20-L27 — enum Mode
- CODE_WRITE · constant · L29-L29 — const CODE_WRITE: &str = "SPX-J125";
- CODE_MISSING · constant · L30-L30 — const CODE_MISSING: &str = "SPX-J124";
- run · function · L33-L55 — pub(crate) fn run(arguments: &[String]) -> Result<String, ProjectLockCliError>
- Outcome · enum · L57-L60 — enum Outcome
- compare_mode · function · L62-L71 — fn compare_mode(snapshot: &ProjectSnapshot, baseline: &Path) -> Result<Outcome, Vec<Diagnostic>>
- emit_interface_mode · function · L76-L84 — fn emit_interface_mode(snapshot: &ProjectSnapshot) -> Result<String, Vec<Diagnostic>>
- compare_interface_mode · function · L86-L98 — fn compare_interface_mode(
- write_mode · function · L100-L110 — fn write_mode(snapshot: &ProjectSnapshot) -> Result<String, Vec<Diagnostic>>
- verify_mode · function · L112-L121 — fn verify_mode(snapshot: &ProjectSnapshot) -> Result<String, Vec<Diagnostic>>
- parse · function · L123-L168 — fn parse(arguments: &[String]) -> Result<(PathBuf, Mode), ProjectLockCliError>
- read_lock · function · L170-L193 — fn read_lock(path: &Path) -> Result<String, Vec<Diagnostic>>
- read_descriptor · function · L195-L223 — fn read_descriptor(path: &Path) -> Result<String, Vec<Diagnostic>>
- write_lock · function · L225-L239 — fn write_lock(path: &Path, lock: &str) -> Result<(), Vec<Diagnostic>>
- usage · function · L241-L243 — fn usage(message: impl Into<String>) -> ProjectLockCliError
