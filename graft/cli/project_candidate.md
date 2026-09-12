# cli/project_candidate.rs

- preview · function · L9-L17 — pub(crate) fn preview(manifest: &Path, change_path: &Path) -> Result<String, Vec<Diagnostic>>
- read_change · function · L19-L57 — fn read_change(path: &Path) -> Result<Vec<u8>, Diagnostic>
- export · function · L59-L68 — pub(crate) fn export(manifest: &Path, change_path: &Path) -> Result<String, Vec<Diagnostic>>
- restore · function · L70-L80 — pub(crate) fn restore(manifest: &Path, capsule_path: &Path) -> Result<String, Vec<Diagnostic>>
- read_capsule · function · L82-L122 — pub(super) fn read_capsule(path: &Path) -> Result<Vec<u8>, Diagnostic>
