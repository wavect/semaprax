//! Live, provisioner-owned collection; serialized observations confer no authority.
use super::{DoctorOfflineArchitecture, DoctorOfflineTool, ProbeError};

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorOfflineTarget {
    Contributor,
    Native,
    Web,
    All,
}

/// Immutable results from one successfully settled provisioned invocation.
/// No constructor accepts reply bytes or caller-supplied tool results.
pub struct SettledDoctorObservation {
    selector: String,
    architecture: DoctorOfflineArchitecture,
    target: DoctorOfflineTarget,
    tools: Vec<SettledDoctorTool>,
}
impl SettledDoctorObservation {
    pub fn selector(&self) -> &str {
        &self.selector
    }
    pub fn architecture(&self) -> DoctorOfflineArchitecture {
        self.architecture
    }
    pub fn target(&self) -> DoctorOfflineTarget {
        self.target
    }
    pub fn tools(&self) -> &[SettledDoctorTool] {
        &self.tools
    }
}

pub struct SettledDoctorTool {
    tool: DoctorOfflineTool,
    path: String,
    output: Result<Vec<u8>, ProbeError>,
}
impl SettledDoctorTool {
    pub fn tool(&self) -> DoctorOfflineTool {
        self.tool
    }
    /// Absolute pathname inside the worker's private root, not a host pathname.
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn output(&self) -> Result<&[u8], ProbeError> {
        self.output.as_deref().map_err(|error| *error)
    }
}

/// Collect exactly one live worker; every failure terminates the process.
///
/// # Safety
/// The caller must be the dedicated single-threaded collector described by
/// DOCTOR-OFFLINE-COLLECTOR-V1. Descriptors 0..=7 are exclusively transferred,
/// live and exactly bound to the approved worker/request/bundle; no others may
/// exist. Parenthood, default SIGCHLD and exclusive reaping ownership must hold
/// from worker creation through collection. Provisioning authenticates the
/// immutable executable/loader, namespace, endpoint and aggregate cleanup
/// contract. Call once, then render without spawning work or acquiring handles,
/// and consume this process with finish_provisioned_doctor_report.
#[doc(hidden)]
pub unsafe fn collect_provisioned_doctor_worker() -> SettledDoctorObservation {
    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        linux::collect()
    }
    #[cfg(not(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        std::process::exit(125)
    }
}

/// Deliver one bounded report and consume the dedicated collector process.
///
/// # Safety
/// Call only after successful collect_provisioned_doctor_worker in that same
/// dedicated process. Its exclusively owned standard anonymous pipes 0..=2
/// must still be live and unchanged, with no other handles or threads created.
/// The complete report and exit status must come from the settled observation's
/// doctor policy, not an untrusted handoff. No embedding host may resume.
#[doc(hidden)]
pub unsafe fn finish_provisioned_doctor_report(report: &[u8], exit_code: u8) -> ! {
    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    linux::finish(report, exit_code);
    #[cfg(not(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        let _ = (report, exit_code);
        std::process::exit(125)
    }
}
