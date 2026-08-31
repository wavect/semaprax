//! Create only fresh anonymous storage; never seal a caller's descriptor.
use super::{validate_max, DoctorOfflineInput, DoctorOfflineInputError as Error};
use std::fs::File;

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Storage {
    NonExecutable,
    Executable,
}

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
        Storage::NonExecutable,
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

/// Create a sealed native ELF memory file and its independently acquired snapshot.
///
/// Caller bytes must pass the shared minimum native ELF validator before any
/// storage effect. Creation requires MFD_EXEC without fallback, sets mode 0500,
/// writes exact bounded chunks, and adds immutable plus execution seals. The
/// returned file has position zero and FD_CLOEXEC; its bytes equal the snapshot.
///
/// This prepares storage only. It neither executes nor approves an image role,
/// authenticates provenance, resolves loaders, or provisions a launch context.
/// Execution seals lock execute bits, not all metadata or extended attributes.
/// The caller must preserve the launcher's separate image/startup prerequisites.
///
/// Limits and unsupported-host behavior match non-executable creation. Failed
/// creation closes its new unpublished descriptor once; uncertain close exits
/// the process with status 126 without retry, as for the input factory.
pub fn create_doctor_offline_executable(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(File, DoctorOfflineInput), Error> {
    create(
        bytes,
        max_bytes,
        Storage::Executable,
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
    storage: Storage,
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
        if storage == Storage::Executable {
            use crate::doctor::{offline_bundle::elf, DoctorOfflineArchitecture};
            let architecture = if cfg!(target_arch = "x86_64") {
                DoctorOfflineArchitecture::LinuxX86_64
            } else {
                DoctorOfflineArchitecture::LinuxAarch64
            };
            elf::validate(bytes, architecture).map_err(|_| Error::Invalid)?;
        }
        linux::create(
            bytes,
            max_bytes,
            storage,
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
        let _ = storage;
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
    create(bytes, max_bytes, Storage::NonExecutable, Some(control))
}

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn create_executable_with_test(
    bytes: &[u8],
    max_bytes: usize,
    control: &mut TestControl,
) -> Result<(File, DoctorOfflineInput), Error> {
    create(bytes, max_bytes, Storage::Executable, Some(control))
}

#[cfg(test)]
mod executable_tests;
#[cfg(test)]
mod tests;
