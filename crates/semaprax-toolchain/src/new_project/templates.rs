#[cfg(test)]
pub(super) struct TemplateFile {
    pub(super) path: &'static str,
    pub(super) bytes: Vec<u8>,
}

// Compatibility for existing private binding tests. Production derives the
// same bytes directly from the public, authority-free scaffold artifact.
#[cfg(test)]
pub(super) fn render(name: &str) -> Vec<TemplateFile> {
    semaprax::project::derive_project_scaffold(
        name,
        semaprax::project::PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR,
    )
    .expect("built-in test scaffold must derive")
    .files()
    .iter()
    .map(|file| TemplateFile {
        path: file.path(),
        bytes: file.bytes().to_vec(),
    })
    .collect()
}
