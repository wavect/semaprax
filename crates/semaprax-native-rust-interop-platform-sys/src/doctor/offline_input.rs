//! Immutable input bytes only: no profile, filesystem path or execution authority.
use std::fs::File;

mod create;
pub use create::{create_doctor_offline_executable, create_doctor_offline_input};

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux;

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use linux::{TestControl, TestFault, TestReadFault, TestStage};

#[cfg(test)]
mod tests;

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[path = "offline_input/bundle_handoff.rs"]
mod bundle_handoff;

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[path = "offline_input/request_handoff.rs"]
mod request_handoff;

pub const DOCTOR_OFFLINE_INPUT_MAX_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorOfflineInputError {
    Invalid,
    Unsupported,
    Limit,
    Io,
}

/// An owned, read-only snapshot. It contains no OS descriptor or authority to
/// execute, publish, or attest the meaning/provenance of these bytes.
#[derive(Debug)]
pub struct DoctorOfflineInput(Vec<u8>);

impl DoctorOfflineInput {
    /// Borrow an already-open input. This operation never duplicates, closes,
    /// seeks, or changes the caller's descriptor. Opening/provisioning it is
    /// outside this API. Unsafe concurrent descriptor replacement is excluded.
    pub fn acquire(file: &File, max_bytes: usize) -> Result<Self, DoctorOfflineInputError> {
        validate_max(max_bytes)?;
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            linux::snapshot(
                file,
                max_bytes,
                #[cfg(test)]
                None,
            )
            .map(Self)
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = file;
            Err(DoctorOfflineInputError::Unsupported)
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    #[cfg(all(
        test,
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn acquire_with_test(
        file: &File,
        max_bytes: usize,
        control: &mut TestControl,
    ) -> Result<Self, DoctorOfflineInputError> {
        validate_max(max_bytes)?;
        linux::snapshot(file, max_bytes, Some(control)).map(Self)
    }
}

fn validate_max(max_bytes: usize) -> Result<(), DoctorOfflineInputError> {
    if max_bytes == 0 {
        Err(DoctorOfflineInputError::Invalid)
    } else if max_bytes > DOCTOR_OFFLINE_INPUT_MAX_BYTES {
        Err(DoctorOfflineInputError::Limit)
    } else {
        Ok(())
    }
}

#[cfg(all(
    test,
    not(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))
))]
#[test]
fn unsupported_platform_retains_limit_precedence() {
    let file = File::open(std::env::current_exe().expect("test executable"))
        .expect("open test executable");
    assert_eq!(
        DoctorOfflineInput::acquire(&file, 1).unwrap_err(),
        DoctorOfflineInputError::Unsupported
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&file, 0).unwrap_err(),
        DoctorOfflineInputError::Invalid
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&file, DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1).unwrap_err(),
        DoctorOfflineInputError::Limit
    );
}
