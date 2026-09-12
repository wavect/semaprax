# cli/package_lock.rs

- PackageLockCliError · enum · L9-L12 — pub(crate) enum PackageLockCliError
- run · function · L14-L19 — pub(crate) fn run(arguments: &[String]) -> Result<String, PackageLockCliError>
- parse · function · L21-L59 — fn parse(arguments: &[String]) -> Result<(Vec<PathBuf>, PackageLockOptions), PackageLockCliError>
- canonical_number · function · L61-L75 — fn canonical_number(option: &str, value: &str) -> Result<usize, PackageLockCliError>
- read_subjects · function · L77-L167 — fn read_subjects(paths: &[PathBuf]) -> Result<Vec<String>, Diagnostic>
- HeldInput · struct · L78-L81 — struct HeldInput
- subject_limit · function · L169-L177 — fn subject_limit() -> Diagnostic
- held_input_identity · function · L180-L186 — fn held_input_identity(
- held_input_identity · function · L189-L200 — fn held_input_identity(
- held_input_identity · function · L203-L211 — fn held_input_identity(
- usage · function · L213-L215 — fn usage(message: impl Into<String>) -> PackageLockCliError
