//! Dedicated provisioner-owned entry; not an ordinary CLI subprocess.
fn main() {
    // SAFETY: this private executable is launched only under the external
    // DOCTOR-OFFLINE-WORKER-V1 provisioning contract. Its main is single-threaded
    // and transfers the entire process, including the fixed descriptor set.
    // No safe embedding API can invoke this operation with hidden prerequisites.
    unsafe { semaprax_native_rust_interop_platform_sys::provisioned_doctor_worker_entry() }
}
