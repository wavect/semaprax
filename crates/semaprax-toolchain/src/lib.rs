//! Unpublished physical hosts for the standalone compiler's checked subjects.
//! This crate has no registry distribution and introduces no compiler fork.
use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::ProjectSnapshot;

mod doctor;

/// Run ordinary doctor policy without discovering or spawning a worker.
pub fn run_doctor(arguments: &[String]) -> Result<(String, u8), String> {
    doctor::run(arguments)
        .map(|outcome| (outcome.output, outcome.exit_code))
        .map_err(|error| error.to_string())
}

/// Render only an opaque, live-collected and settled offline worker observation.
/// This applies version policy; it neither acquires authority nor runs tools.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn render_settled_doctor(
    observation: &semaprax_native_rust_interop_platform::SettledDoctorObservation,
    json: bool,
) -> (String, u8) {
    let outcome = doctor::render_settled(observation, json);
    (outcome.output, outcome.exit_code)
}

#[cfg(windows)]
mod windows_revision_store;

#[cfg(windows)]
mod owned_npm;

/// Windows-only held publication of the compiler's opaque Project-v8/v9/v10 plan.
#[cfg(windows)]
pub fn build_owned_npm(
    snapshot: &mut ProjectSnapshot,
    output: &Path,
) -> Result<(), Vec<Diagnostic>> {
    snapshot.build_owned_npm_with(output, owned_npm::publish)
}

#[cfg(windows)]
pub fn persist_windows(
    root: &Path,
    revision: &semaprax::project::ProjectRevision,
    expected: &str,
) -> Result<semaprax::project_revision_store::ProjectRevisionStoreReceipt, Vec<Diagnostic>> {
    semaprax::project_revision_store::windows_host::persist(
        root,
        revision,
        expected,
        windows_revision_store::persist,
    )
}

#[cfg(windows)]
pub fn load_windows(
    root: &Path,
    digest: &str,
    expected: &str,
) -> Result<semaprax::project::ProjectRevision, Vec<Diagnostic>> {
    semaprax::project_revision_store::windows_host::load(
        root,
        digest,
        expected,
        windows_revision_store::load,
    )
}

/// Publish only after compiler-owned descriptor/provider replay and lease checks.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn build_rust(snapshot: &mut ProjectSnapshot, output: &Path) -> Result<(), Vec<Diagnostic>> {
    use semaprax::project::ProjectNativeRustPackageMode;
    use semaprax_native_rust_owned_data_package as package;

    snapshot.build_rust_with(output, |subject, output| {
        let (mode, version) = match subject.mode() {
            ProjectNativeRustPackageMode::OwnedData => (package::PackageMode::ProjectV8, "v8"),
            ProjectNativeRustPackageMode::FlatOwnedRecord => (package::PackageMode::ProjectV9FlatRecord, "v9"),
            ProjectNativeRustPackageMode::OwnedUtf8 => (package::PackageMode::ProjectV10OwnedUtf8, "v10"),
            ProjectNativeRustPackageMode::NestedOwnedRecord => (package::PackageMode::ProjectV11NestedRecord, "v11"),
        };
        let plan = package::PackagePlan::new(
            subject.descriptor().to_vec(),
            subject.descriptor_digest().to_owned(),
            subject.selected().to_vec(),
            subject.provider().to_vec(),
            package::provider_sha256(subject.provider()),
            mode,
        );
        let result = match subject.mode() {
            ProjectNativeRustPackageMode::FlatOwnedRecord => package::build_flat_record_and_publish(plan, output),
            ProjectNativeRustPackageMode::NestedOwnedRecord => package::build_nested_record_and_publish(plan, output),
            ProjectNativeRustPackageMode::OwnedData | ProjectNativeRustPackageMode::OwnedUtf8 => package::build_and_publish(plan, output),
        };
        result.map(|_| ()).map_err(|error| {
            let (code, message) = match error.kind() {
                package::PackageErrorKind::Descriptor | package::PackageErrorKind::Provider => (
                    "SPX-B114",
                    format!("Project {version} Native Rust package replay failed"),
                ),
                package::PackageErrorKind::ToolConfiguration => (
                    "SPX-I234",
                    format!("Project {version} Native Rust package requires explicit absolute CLANG and archiver tools"),
                ),
                package::PackageErrorKind::Publication => (
                    "SPX-I234",
                    match error.detail() {
                        Some(detail) => format!(
                            "Project {version} Native Rust package publication failed: {detail}"
                        ),
                        None => {
                            format!("Project {version} Native Rust package publication failed")
                        }
                    },
                ),
            };
            vec![Diagnostic::io(code, message)]
        })
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn build_rust(snapshot: &mut ProjectSnapshot, output: &Path) -> Result<(), Vec<Diagnostic>> {
    snapshot.build_rust(output)
}
