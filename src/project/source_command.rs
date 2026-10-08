//! Project v26 links ordinary source libraries into the existing native
//! SourceCommand adapter. Representation linking grants no host authority.
use super::{ProjectManifest, ProjectProfile};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedProgram};

pub(super) fn admit(
    program: &ResolvedProgram,
    manifest: &ProjectManifest,
) -> Result<(), Diagnostic> {
    let entry = program
        .functions
        .iter()
        .find(|function| function.id == program.entrypoint);
    if !manifest.project_profile().is_source_command()
        || !manifest.web_exports().is_empty()
        || manifest.command() != Some(program.entrypoint.as_str())
        || !entry.is_some_and(|function| {
            function.name == "main"
                && function.params.is_empty()
                && function.return_type == hir::ResolvedType::I64
        })
    {
        return Err(Diagnostic::io("SPX-J130", "source-command profiles require command.function to select the explicit entry fn main() -> i64 and empty web exports"));
    }
    if program.permits != manifest.capabilities() {
        return Err(Diagnostic::io(
            "SPX-J131",
            "source-command linked effects must equal the exact manifest capabilities",
        ));
    }
    // Replay HIR, complete reachable authority and native feature admission in
    // Phase A; no destination or filesystem provider is acquired here.
    if manifest.project_profile() == ProjectProfile::SourceCommandResourceOutputV1 {
        crate::source_command::validate_resource_output_authority(program)?;
        crate::command_io_ops::validate_operation_profile(
            program,
            &program.entrypoint,
            crate::command_io_ops::CommandOperationProfile::SourceResourceV1,
        )?;
        crate::codegen::emit_hir_c_with_source_resource_command(program).map(drop)
    } else {
        crate::codegen::emit_hir_c_with_source_command(program).map(drop)
    }
}

pub(super) fn require_portable(profile: ProjectProfile) -> Result<(), Diagnostic> {
    if profile.is_source_command() {
        return Err(Diagnostic::io(
            "SPX-W120",
            "source-command profiles admit only native64; no WebAssembly, Web or npm emitter",
        ));
    }
    Ok(())
}

pub(super) fn require_interpreter(profile: ProjectProfile) -> Result<(), Diagnostic> {
    if profile.is_source_command() {
        return Err(Diagnostic::io("SPX-F102", "source-command profiles admit only a native invocation; Project interpreter execution has no argv/file authority provider"));
    }
    Ok(())
}
