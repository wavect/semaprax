//! Dedicated trusted-provisioner entry, never ambient CLI discovery.
fn main() {
    // SAFETY: this private binary is launched only under the complete fixed-FD,
    // image, loader and process-context contract in DOCTOR-OFFLINE-LAUNCHER-V1.
    // It consumes the dedicated process; no embedding application resumes.
    unsafe { semaprax_native_rust_interop_platform_sys::provisioned_doctor_launcher_entry() }
}
