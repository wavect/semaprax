//! Deliberately separate unsafe entry; the compiler and report policy stay safe.
//! Only the trusted DOCTOR-OFFLINE-COLLECTOR-V1 provisioner may launch this binary.
#![deny(unsafe_op_in_unsafe_fn)]

fn main() {
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    {
        use semaprax_native_rust_interop_platform_sys as platform;
        // SAFETY: dedicated single-threaded entry under the provisioned fixed
        // descriptor, parenthood, executable and endpoint ownership contract.
        // Failed or uncertain collection terminates; only settled data returns.
        let observation = unsafe { platform::collect_provisioned_doctor_worker() };
        let (report, status) = semaprax_toolchain::render_settled_doctor(&observation, true);
        // SAFETY: collection consumed 3..7; standard anonymous pipes 0..2 still
        // belong exclusively to this invocation. No embedding caller resumes.
        unsafe { platform::finish_provisioned_doctor_report(report.as_bytes(), status) }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    std::process::exit(125);
}
