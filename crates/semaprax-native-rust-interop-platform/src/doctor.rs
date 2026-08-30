//! Bounded local version probes, independent of authenticated build tooling.
use std::path::Path;

pub use semaprax_native_rust_interop_platform_sys::DoctorProbeError;

/// Invokes exactly `--version` and returns bounded stdout only after process
/// settlement. Trusted installed tools only; this is not a network sandbox.
/// On Unix the host must exclude foreign child reapers and SIGCHLD policy
/// mutation for the call. Non-default SIGCHLD policy is rejected at admission.
pub fn doctor_version_probe(path: &Path) -> Result<Vec<u8>, DoctorProbeError> {
    semaprax_native_rust_interop_platform_sys::doctor_version_probe(path)
}
