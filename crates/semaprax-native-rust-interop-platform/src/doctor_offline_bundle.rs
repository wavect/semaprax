//! Safe ownership facade over the single quarantined offline inventory parser.
use crate::DoctorOfflineInput;
use semaprax_native_rust_interop_platform_sys as sys;

pub use sys::{
    encode_doctor_offline_bundle, DoctorOfflineArchitecture, DoctorOfflineBundleEntry,
    DoctorOfflineBundleError, DoctorOfflineBundleFile, DoctorOfflineBundleRoles, DoctorOfflineTool,
};

/// An immutable structurally admitted inventory, not executable provenance or
/// permission to publish files or launch a process.
pub struct DoctorOfflineBundle(sys::DoctorOfflineBundle);

impl std::fmt::Debug for DoctorOfflineBundle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, formatter)
    }
}

impl DoctorOfflineBundle {
    /// Consume one sealed snapshot and bind its inventory to the explicit
    /// selector and compiled native Linux architecture, without ambient lookup.
    pub fn parse(
        input: DoctorOfflineInput,
        selector: &str,
    ) -> Result<Self, DoctorOfflineBundleError> {
        sys::DoctorOfflineBundle::parse(input.into_inner(), selector).map(Self)
    }

    pub fn selector(&self) -> &str {
        self.0.selector()
    }

    pub fn architecture(&self) -> DoctorOfflineArchitecture {
        self.0.architecture()
    }

    pub fn files(&self) -> impl ExactSizeIterator<Item = DoctorOfflineBundleFile<'_>> {
        self.0.files()
    }

    pub fn tool(&self, tool: DoctorOfflineTool) -> Option<DoctorOfflineBundleFile<'_>> {
        self.0.tool(tool)
    }

    /// Prepare request bytes for this exact retained bundle. The caller owns
    /// nonce freshness, sealing and provisioning; bytes confer no authority.
    pub fn encode_worker_request(
        &self,
        target: crate::DoctorOfflineTarget,
        nonce: [u8; 32],
    ) -> Result<Vec<u8>, DoctorOfflineBundleError> {
        self.0.encode_worker_request(target, nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_surface_requires_owned_sealed_input_and_exposes_read_only_views() {
        let _: fn(
            DoctorOfflineInput,
            &str,
        ) -> Result<DoctorOfflineBundle, DoctorOfflineBundleError> = DoctorOfflineBundle::parse;
        let _: fn(&DoctorOfflineBundle) -> &str = DoctorOfflineBundle::selector;
        let _: fn(&DoctorOfflineBundle) -> DoctorOfflineArchitecture =
            DoctorOfflineBundle::architecture;
        let _: for<'a> fn(
            &'a DoctorOfflineBundle,
            DoctorOfflineTool,
        ) -> Option<DoctorOfflineBundleFile<'a>> = DoctorOfflineBundle::tool;
    }

    #[test]
    fn file_view_accessors_preserve_retained_bundle_lifetime() {
        fn check_path<'a>(_f: fn(&DoctorOfflineBundleFile<'a>) -> &'a str) {}
        check_path(DoctorOfflineBundleFile::path);
        fn check_bytes<'a>(_f: fn(&DoctorOfflineBundleFile<'a>) -> &'a [u8]) {}
        check_bytes(DoctorOfflineBundleFile::bytes);
    }

    #[test]
    fn preparation_returns_bytes_and_request_derivation_requires_the_retained_bundle() {
        type EncodeBundle = fn(
            DoctorOfflineArchitecture,
            &str,
            &[DoctorOfflineBundleEntry<'_>],
            DoctorOfflineBundleRoles,
            usize,
        ) -> Result<Vec<u8>, DoctorOfflineBundleError>;
        type EncodeRequest = fn(
            &DoctorOfflineBundle,
            crate::DoctorOfflineTarget,
            [u8; 32],
        ) -> Result<Vec<u8>, DoctorOfflineBundleError>;
        let _: EncodeBundle = encode_doctor_offline_bundle;
        let _: EncodeRequest = DoctorOfflineBundle::encode_worker_request;
    }
}
