//! Create only fresh anonymous input storage; never seal a caller's descriptor.
use super::{validate_max, DoctorOfflineInput, DoctorOfflineInputError as Error};
use std::fs::File;

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux;

/// Create non-executable immutable memory-file storage and its verified snapshot.
///
/// Limits precede all effects. Only native64 Linux with MFD_NOEXEC_SEAL support
/// is admitted; there is no pathname, executable fallback or caller-fd mutation.
/// The snapshot comes exclusively from the existing sealed-input acquisition
/// validator. The returned file has position zero and FD_CLOEXEC, but transfers
/// no worker/provisioning authority. Kernel/LSM/VM behavior remains trusted.
///
/// On failure the newly owned unpublished descriptor receives one checked close.
/// A negative close result terminates this process with status 126, without retry:
/// syscall filtering can reject close before the kernel releases the descriptor.
/// This narrow fail-stop contract does not change borrowed-input acquisition.
pub fn create_doctor_offline_input(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(File, DoctorOfflineInput), Error> {
    create(
        bytes,
        max_bytes,
        #[cfg(all(
            test,
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        None,
    )
}

fn create(
    bytes: &[u8],
    max_bytes: usize,
    #[cfg(all(
        test,
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    control: Option<&mut linux::TestControl>,
) -> Result<(File, DoctorOfflineInput), Error> {
    validate_max(max_bytes)?;
    if bytes.is_empty() {
        return Err(Error::Invalid);
    }
    if bytes.len() > max_bytes {
        return Err(Error::Limit);
    }
    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        linux::create(
            bytes,
            max_bytes,
            #[cfg(test)]
            control,
        )
    }
    #[cfg(not(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        Err(Error::Unsupported)
    }
}

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use linux::{TestControl, TestFault, TestStage, TestWriteFault};

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn create_with_test(
    bytes: &[u8],
    max_bytes: usize,
    control: &mut TestControl,
) -> Result<(File, DoctorOfflineInput), Error> {
    create(bytes, max_bytes, Some(control))
}

#[cfg(test)]
mod tests;
