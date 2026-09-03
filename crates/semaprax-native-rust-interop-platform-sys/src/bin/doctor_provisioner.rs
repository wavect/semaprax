//! Dedicated signed offline-doctor provisioner entry; no argument surface.
fn main() {
    // SAFETY: the release provisioner owns the complete fixed-descriptor,
    // namespace, cgroup, trust-anchor and single-process contract documented by
    // this private consuming entry. It never resumes this process.
    unsafe { semaprax_native_rust_interop_platform_sys::provisioned_doctor_provisioner_entry() }
}
