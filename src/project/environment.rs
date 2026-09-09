//! Additive Project environment authority with explicitly injected snapshots.
use super::{ProjectManifest, ProjectProfile, ProjectRevision, ProjectSnapshot};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram};

pub(super) fn admit(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
) -> Result<(), Diagnostic> {
    if manifest.project_profile() != ProjectProfile::EnvironmentIoV1
        || !manifest.web_exports().is_empty()
    {
        return Err(Diagnostic::io(
            "SPX-J113",
            "environment profile has no public web exports",
        ));
    }
    let entry = manifest
        .command()
        .ok_or_else(|| Diagnostic::io("SPX-J113", "environment command is absent"))?;
    if !super::profile::valid_environment_capabilities(&program.permits)
        || !program.interfaces.is_empty()
    {
        return Err(Diagnostic::io(
            "SPX-J113",
            "environment profile requires explicit command effects without callback interfaces",
        ));
    }
    hir::validate(program)?;
    crate::wasm::environment_io::emit_resolved_environment_io_v1(program, entry).map(drop)
}
impl ProjectRevision {
    /// Validating provider factory for this private command profile. Its input
    /// must be supplied explicitly; the adapter reads no ambient process data.
    pub fn environment_provider_source(&self) -> Result<&'static str, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        Ok(include_str!("../wasm/environment_provider.mjs"))
    }
    pub fn execute_environment_command(
        &self,
        input: &crate::hosted_interpreter::HostedEnvironmentCommandInput,
        max_steps: usize,
    ) -> Result<crate::hosted_interpreter::HostedCommandResult, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::hosted_interpreter::execute_environment_command(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
            input,
            max_steps,
        )
    }
    pub fn environment_c_source(&self) -> Result<String, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::codegen::emit_hir_c_with_environment_io(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
        )
        .map_err(|error| vec![error])
    }
    pub fn environment_wasm_module(&self) -> Result<Vec<u8>, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::wasm::environment_io::emit_resolved_environment_io_v1(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
        )
        .map_err(|error| vec![error])
    }
}
impl ProjectSnapshot {
    pub fn execute_environment_command(
        &self,
        input: &crate::hosted_interpreter::HostedEnvironmentCommandInput,
        max_steps: usize,
    ) -> Result<crate::hosted_interpreter::HostedCommandResult, Vec<Diagnostic>> {
        self.revision.execute_environment_command(input, max_steps)
    }
}
