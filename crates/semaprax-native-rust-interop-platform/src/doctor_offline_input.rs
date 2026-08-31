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

/// Create anonymous executable storage for explicit native ELF image bytes.
///
/// Limits and native minimum ELF framing are checked before OS effects. The
/// new file requires explicit executable-memfd support, owner-only mode 0500,
/// immutable content and execute-bit seals, close-on-exec and exact snapshot
/// verification. No pathname, image discovery, execution or fallback is used.
/// Failure cleanup has the same one-shot/fail-stop contract as input creation.
///
/// The returned file is executable storage, not an approved image, loader
/// closure, confinement proof or profile admission. The caller owns its lifetime
/// and subsequent metadata changes; seals do not freeze all permission bits.
/// Other hosts return Unsupported after the common byte/limit checks.
pub fn create_doctor_offline_executable(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(File, DoctorOfflineInput), DoctorOfflineInputError> {
    semaprax_native_rust_interop_platform_sys::create_doctor_offline_executable(bytes, max_bytes)
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
        type CreateInput =
            fn(&[u8], usize) -> Result<(File, DoctorOfflineInput), DoctorOfflineInputError>;
        let _: CreateInput = create_doctor_offline_input;
        let _: CreateInput = create_doctor_offline_executable;
    }
}
