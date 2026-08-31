//! Fixed-descriptor launch plumbing, not bootstrap or image provenance authority.
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
mod lifetime;
#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux;

/// Consume a dedicated launcher, becoming the collector of its own worker.
/// Unsupported hosts exit 125; rejection/uncertainty exits 126 without a report.
///
/// # Safety
/// The caller must satisfy DOCTOR-OFFLINE-LAUNCHER-V1: single-threaded dedicated
/// process, exclusively transferred live descriptors exactly 0..6 (anonymous
/// standard pipes, sealed request/bundle and approved executable memory files),
/// no foreign reapers, descriptor/signal/image-metadata mutators, and the full
/// trusted worker/collector namespace, immutable loader and cgroup context.
/// Correct image roles/provenance and absence of privilege-changing or unapproved
/// binfmt/loader execution are external preconditions, not inferred from bytes.
/// This never returns and must not be called from an embedding application.
#[doc(hidden)]
pub unsafe fn provisioned_doctor_launcher_entry() -> ! {
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
