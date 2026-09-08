//! Explicit filesystem Project authority; no public nominal ABI or ambient provider.
use super::{ProjectManifest, ProjectProfile, ProjectRevision, ProjectSnapshot};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram};

pub(super) fn admit(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
) -> Result<(), Diagnostic> {
    if manifest.project_profile() != ProjectProfile::FilesystemIoV1
        || !manifest.web_exports().is_empty()
    {
        return Err(Diagnostic::io(
            "SPX-J113",
            "filesystem profile has no public web exports",
        ));
    }
    let entry = manifest
        .command()
        .ok_or_else(|| Diagnostic::io("SPX-J113", "filesystem command is absent"))?;
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == entry)
        .ok_or_else(|| Diagnostic::io("SPX-J113", "filesystem command is not retained"))?;
    if !function.params.is_empty()
        || function.return_type != hir::ResolvedType::Bool
        || !program.interfaces.is_empty()
        || program
            .permits
            .iter()
            .any(|effect| !crate::filesystem_ops::FILESYSTEM_EFFECTS.contains(&effect.as_str()))
    {
        return Err(Diagnostic::io(
            "SPX-J113",
            "filesystem profile requires fn()->bool and only explicit filesystem effects",
        ));
    }
    hir::validate(program)?;
    crate::command_io_ops::validate_operation_profile(
        program,
        &function.id,
        crate::command_io_ops::CommandOperationProfile::FilesystemV1,
    )?;
    crate::wasm::emit_resolved_filesystem_ops_v1(program, entry).map(drop)
}
impl ProjectRevision {
    pub fn execute_filesystem_command(
        &self,
        provider: &mut dyn crate::filesystem_provider::FileProvider,
        max_steps: usize,
    ) -> Result<crate::interpreter::CommandEvaluation, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::hosted_interpreter::execute_filesystem_command(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
            provider,
            max_steps,
        )
        .map_err(|error| vec![error])
    }
    pub fn filesystem_c_source(&self) -> Result<String, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::codegen::emit_hir_c_with_filesystem_io(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
        )
        .map_err(|error| vec![error])
    }
    pub fn filesystem_wasm_module(&self) -> Result<Vec<u8>, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::wasm::emit_resolved_filesystem_ops_v1(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
        )
        .map_err(|error| vec![error])
    }
}
impl ProjectSnapshot {
    pub fn execute_filesystem_command(
        &self,
        provider: &mut dyn crate::filesystem_provider::FileProvider,
        max_steps: usize,
    ) -> Result<crate::interpreter::CommandEvaluation, Vec<Diagnostic>> {
        self.revision
            .execute_filesystem_command(provider, max_steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filesystem_manifest_retains_explicit_authority_without_web_exports() {
        let text = include_str!("../../std/fs/semaprax.toml");
        let manifest = ProjectManifest::parse(text).expect("filesystem package manifest");
        assert_eq!(manifest.project_profile(), ProjectProfile::FilesystemIoV1);
        assert_eq!(manifest.schema(), super::super::PROJECT_SCHEMA_V14);
        assert!(manifest.web_exports().is_empty());
        assert_eq!(manifest.command(), Some("std.fs.examples.roundtrip"));
        assert_eq!(manifest.to_canonical_toml(), text);
        assert!(
            ProjectManifest::parse(&text.replace("\"fs.read\", \"fs.write\"", "\"fs.read\""))
                .is_err()
        );
        assert!(ProjectManifest::parse(
            &text.replace("web = []", "web = [\"std.fs.examples.roundtrip\"]")
        )
        .is_err());
    }
}
