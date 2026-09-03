//! Release-signed Linux offline-doctor provisioning; never ambient CLI authority.
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod admission;
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod capsule;
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod cgroup;
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux;

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Consume the dedicated production provisioner process.
///
/// Unsupported hosts exit 125. Missing trust, invalid inputs and any uncertain
/// setup or settlement exit 126 without publishing an ordinary report.
///
/// # Safety
/// The caller must supply one dedicated single-threaded process with no
/// asynchronous handlers or foreign reapers and exclusively transferred fixed
/// descriptors: anonymous pipes 0..=2; sealed capsule/request/bundle files 3..=5;
/// approved immutable executable launcher/worker/collector images 6..=8; one
/// empty delegated cgroup-v2 directory at 9; and the caller's trusted procfs root
/// at 10. The release build must supply only the public Ed25519 trust anchor.
/// This function consumes the process and never returns to an embedding host.
#[doc(hidden)]
pub unsafe fn provisioned_doctor_provisioner_entry() -> ! {
    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    linux::entry();
    #[cfg(not(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    std::process::exit(125)
}
