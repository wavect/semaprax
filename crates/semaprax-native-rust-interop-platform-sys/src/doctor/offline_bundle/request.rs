//! Canonical provisioner request bytes, not an execution or admission token.
use super::{DoctorOfflineBundle, DoctorOfflineBundleError};
use crate::DoctorOfflineTarget;

impl DoctorOfflineBundle {
    /// Encode one target against this exact retained, structurally admitted
    /// bundle. The caller supplies the nonzero nonce and owns its freshness.
    /// This neither seals inputs nor provisions, starts or authenticates a worker.
    pub fn encode_worker_request(
        &self,
        target: DoctorOfflineTarget,
        nonce: [u8; 32],
    ) -> Result<Vec<u8>, DoctorOfflineBundleError> {
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            use super::{DoctorOfflineArchitecture, DoctorOfflineTool};
            use crate::doctor::offline_worker::{wire, Error};
            use sha2::{Digest, Sha256};

            if nonce == [0; 32] {
                return Err(DoctorOfflineBundleError::Invalid);
            }
            let (target, roles) = match target {
                DoctorOfflineTarget::Contributor => (0, 4),
                DoctorOfflineTarget::Native => (1, 1),
                DoctorOfflineTarget::Web => (2, 2),
                DoctorOfflineTarget::All => (3, 7),
            };
            for (role, tool) in [
                (1, DoctorOfflineTool::Clang),
                (2, DoctorOfflineTool::Node),
                (4, DoctorOfflineTool::Rustc),
            ] {
                if roles & role != 0 && self.tool(tool).is_none() {
                    return Err(DoctorOfflineBundleError::Invalid);
                }
            }
            let architecture = match self.architecture() {
                DoctorOfflineArchitecture::LinuxX86_64 => 1,
                DoctorOfflineArchitecture::LinuxAarch64 => 2,
            };
            let selector = self.selector().as_bytes();
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(85 + selector.len())
                .map_err(|_| DoctorOfflineBundleError::Allocation)?;
            bytes.extend_from_slice(b"SPXDWK1\0");
            bytes.extend_from_slice(&[1, architecture, target, roles]);
            bytes.extend_from_slice(&nonce);
            bytes.extend_from_slice(&(self.input.bytes().len() as u64).to_le_bytes());
            bytes.extend_from_slice(&Sha256::digest(self.input.bytes()));
            bytes.push(selector.len() as u8);
            bytes.extend_from_slice(selector);
            // Replay the sole decoder before returning bytes; its acceptance,
            // diagnostics and worker/collector binding semantics are unchanged.
            wire::Request::parse(&bytes).map_err(|error| match error {
                Error::Invalid | Error::Io => DoctorOfflineBundleError::Invalid,
                Error::Limit => DoctorOfflineBundleError::Limit,
                Error::Allocation => DoctorOfflineBundleError::Allocation,
            })?;
            Ok(bytes)
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = (target, nonce);
            Err(DoctorOfflineBundleError::Unsupported)
        }
    }
}
