//! Borrowed sealed input for a future offline doctor backend, not execution authority.
use std::fs::File;

pub use semaprax_native_rust_interop_platform_sys::DoctorOfflineInputError;

pub const DOCTOR_OFFLINE_INPUT_MAX_BYTES: usize =
    semaprax_native_rust_interop_platform_sys::DOCTOR_OFFLINE_INPUT_MAX_BYTES;

/// Immutable owned bytes copied from a caller-provisioned sealed memory file.
///
/// This unpublished carrier authenticates an input storage boundary only. Its
/// contents remain untrusted: it proves neither profile identity nor executable
/// provenance, and grants no file publication or process-launch authority.
pub struct DoctorOfflineInput(semaprax_native_rust_interop_platform_sys::DoctorOfflineInput);

/// Create an anonymous, non-executable sealed memory file from explicit bytes,
/// returning that owned file and its independently acquired snapshot.
///
/// This uses no host pathname or environment discovery. Native Linux support
/// for mandatory non-executable sealing is required; there is no fallback.
/// Limits and empty input are rejected before any OS operation. An unpublished
/// file is closed exactly once on failure; uncertain closure terminates the
/// process. The caller owns the successfully returned file and its lifetime.
/// Storage immutability does not establish content provenance, confinement or
/// authority to execute a worker. Ordinary CLI admission remains unchanged.
pub fn create_doctor_offline_input(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(File, DoctorOfflineInput), DoctorOfflineInputError> {
    semaprax_native_rust_interop_platform_sys::create_doctor_offline_input(bytes, max_bytes)
        .map(|(file, input)| (file, DoctorOfflineInput(input)))
}

impl DoctorOfflineInput {
    /// Borrow `file` without duplicating, closing, reopening or seeking it.
    ///
    /// Supported native64 Linux hosts authenticate immutable shmem seals before
    /// filesystem metadata or content access. All other hosts fail closed.
    /// The caller may lower the hard byte ceiling, never widen it. Positional
    /// reads leave the caller's offset unchanged; any incomplete read fails
    /// without publishing a partial carrier. The caller retains its file.
    ///
    /// Trusted kernel/LSM/VM activity and provisioning before this call are not
    /// confined. This API is not connected to the real doctor CLI yet.
    pub fn acquire(file: &File, max_bytes: usize) -> Result<Self, DoctorOfflineInputError> {
        semaprax_native_rust_interop_platform_sys::DoctorOfflineInput::acquire(file, max_bytes)
            .map(Self)
    }

    /// Return untrusted content without exposing a raw descriptor or mutator.
    pub fn bytes(&self) -> &[u8] {
        self.0.bytes()
    }

    pub(super) fn into_inner(
        self,
    ) -> semaprax_native_rust_interop_platform_sys::DoctorOfflineInput {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_input_surface_keeps_the_quarantine_limit_and_borrowed_file_contract() {
        assert_eq!(DOCTOR_OFFLINE_INPUT_MAX_BYTES, 536_870_912);
        let _: fn(&File, usize) -> Result<DoctorOfflineInput, DoctorOfflineInputError> =
            DoctorOfflineInput::acquire;
        let _: fn(&DoctorOfflineInput) -> &[u8] = DoctorOfflineInput::bytes;
        let _: fn(&[u8], usize) -> Result<(File, DoctorOfflineInput), DoctorOfflineInputError> =
            create_doctor_offline_input;
    }
}
