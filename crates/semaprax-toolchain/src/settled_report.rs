//! Pure version policy for an already settled, provisioner-owned invocation.
//! Parsing reply bytes is deliberately not an entry point to this adapter.
use semaprax_native_rust_interop_platform::{
    DoctorOfflineArchitecture, DoctorOfflineTarget, DoctorOfflineTool, DoctorProbeError,
    SettledDoctorObservation,
};

use semaprax::doctor::{
    clang_version, node_version, platform_checks, report, rust_version_check, Check, DoctorError,
    DoctorOutcome, DoctorTarget,
};

type ObservedTool<'a> = (
    DoctorOfflineTool,
    &'a str,
    Result<&'a [u8], DoctorProbeError>,
);

pub(crate) fn render_settled(observation: &SettledDoctorObservation, json: bool) -> DoctorOutcome {
    render_rows(
        observation.selector(),
        observation.architecture(),
        observation.target(),
        observation
            .tools()
            .iter()
            .map(|tool| (tool.tool(), tool.path(), tool.output())),
        json,
        !cfg!(debug_assertions),
    )
}

// Private policy seam, not a constructor for admission or settled observations.
// Production callers supply the opaque collector's exact ordered requested rows.
fn render_rows<'a>(
    selector: &str,
    architecture: DoctorOfflineArchitecture,
    target: DoctorOfflineTarget,
    tools: impl IntoIterator<Item = ObservedTool<'a>>,
    json: bool,
    release_build: bool,
) -> DoctorOutcome {
    let arch = match architecture {
        DoctorOfflineArchitecture::LinuxX86_64 => "x86_64",
        DoctorOfflineArchitecture::LinuxAarch64 => "aarch64",
    };
    let target = match target {
        DoctorOfflineTarget::Contributor => DoctorTarget::Contributor,
        DoctorOfflineTarget::Native => DoctorTarget::Native,
        DoctorOfflineTarget::Web => DoctorTarget::Web,
        DoctorOfflineTarget::All => DoctorTarget::All,
    };
    let mut checks = platform_checks("linux", arch, release_build);
    checks.push(Check::ok(
        "profile",
        format!("offline profile `{selector}`; checks describe this profile only"),
    ));
    for (tool, path, output) in tools {
        // Roles can share an executable pathname. Never key observations by path:
        // each independently executed role retains its own output or failure.
        let output = version_text(output);
        checks.push(match tool {
            DoctorOfflineTool::Clang => clang_version(path, output),
            DoctorOfflineTool::Node => node_version(output),
            DoctorOfflineTool::Rustc => rust_version_check(output),
        });
    }
    report(target, json, &checks)
}

fn version_text(output: Result<&[u8], DoctorProbeError>) -> Result<String, DoctorError> {
    match output {
        Ok(bytes) => std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| DoctorError::new("tool returned non-UTF-8 version output")),
        Err(error) => Err(DoctorError::new(match error {
            DoctorProbeError::Invalid => "offline tool invocation was invalid",
            DoctorProbeError::Unsupported => "offline tool invocation is unsupported",
            DoctorProbeError::Spawn => "offline tool supervisor launch failed",
            DoctorProbeError::Exit => "offline tool terminated unsuccessfully",
            DoctorProbeError::OutputLimit => "offline tool output exceeded its limit",
            DoctorProbeError::Timeout => "offline tool execution timed out",
            DoctorProbeError::Io => "offline tool capture failed",
        })),
    }
}

#[cfg(test)]
#[path = "settled_report/tests.rs"]
mod tests;
