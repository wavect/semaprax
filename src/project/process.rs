//! Additive Project process authority with host-injected providers.

use super::{ProjectManifest, ProjectProfile, ProjectRevision, ProjectSnapshot};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram};

pub(super) fn admit(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
) -> Result<(), Diagnostic> {
    if manifest.project_profile() != ProjectProfile::ProcessIoV1
        || !manifest.web_exports().is_empty()
    {
        return Err(Diagnostic::io(
            "SPX-J113",
            "process profile has no public web exports",
        ));
    }
    let entry = manifest
        .command()
        .ok_or_else(|| Diagnostic::io("SPX-J113", "process command is absent"))?;
    if !super::profile::valid_process_capabilities(&program.permits)
        || !program.interfaces.is_empty()
    {
        return Err(Diagnostic::io(
            "SPX-J113",
            "process profile requires explicit command effects without callback interfaces",
        ));
    }
    hir::validate(program)?;
    crate::wasm::process_io::emit_resolved_process_io_v1(program, entry).map(drop)
}

impl ProjectRevision {
    pub fn execute_process_command(
        &self,
        input: &crate::hosted_interpreter::HostedEnvironmentCommandInput,
        provider: &mut dyn crate::process_provider::ProcessProvider,
        max_steps: usize,
    ) -> Result<crate::hosted_interpreter::HostedCommandResult, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::hosted_interpreter::execute_process_command(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
            input,
            provider,
            max_steps,
        )
    }

    pub fn process_c_source(&self) -> Result<String, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::codegen::emit_hir_c_with_process_io(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
        )
        .map_err(|error| vec![error])
    }

    pub fn process_wasm_module(&self) -> Result<Vec<u8>, Vec<Diagnostic>> {
        admit(&self.public_api_program, &self.manifest).map_err(|error| vec![error])?;
        crate::wasm::process_io::emit_resolved_process_io_v1(
            &self.public_api_program,
            self.manifest.command().unwrap_or(""),
        )
        .map_err(|error| vec![error])
    }
}

impl ProjectSnapshot {
    pub fn execute_process_command(
        &self,
        input: &crate::hosted_interpreter::HostedEnvironmentCommandInput,
        provider: &mut dyn crate::process_provider::ProcessProvider,
        max_steps: usize,
    ) -> Result<crate::hosted_interpreter::HostedCommandResult, Vec<Diagnostic>> {
        self.revision
            .execute_process_command(input, provider, max_steps)
    }
}
