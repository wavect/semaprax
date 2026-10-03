//! Windows-only authenticated request/bundle carriers.
//!
//! This module authenticates one signed-capsule artifact into an unnamed
//! paging-file mapping, then drops every writable mapping handle and view
//! before retaining one inheritable `SECTION_MAP_READ` duplicate. The
//! confinement primitive binds the two resulting handles into its fixed child
//! inventory together with the signed selector and image role. This module
//! itself neither starts a child nor selects an image.
use semaprax_doctor_capsule::{Artifact, MAX_ARTIFACT_BYTES};
use sha2::{Digest as _, Sha256};
use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, GetHandleInformation, LocalFree, HANDLE, HANDLE_FLAG_INHERIT,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, FILE_MAP_WRITE,
    PAGE_READWRITE, SECTION_MAP_READ,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

struct Mapping(HANDLE);

struct MappingSecurity(PSECURITY_DESCRIPTOR);

impl MappingSecurity {
    fn create() -> Result<Self, ()> {
        // No allow ACE grants a new handle any access. OWNER RIGHTS also
        // suppresses the owner's implicit WRITE_DAC: an empty DACL alone
        // would still let the same-user child regain permission to rewrite it.
        // The original creation handle receives full access to the new section;
        // its later read-only duplicate only reduces those existing rights.
        let sddl: Vec<u16> = "D:P(D;;GA;;;OW)\0".encode_utf16().collect();
        let mut descriptor = std::ptr::null_mut();
        // SAFETY: SDDL is a fixed, terminated string; the API allocates the
        // complete descriptor, which this owner releases with LocalFree.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for MappingSecurity {
    fn drop(&mut self) {
        // SAFETY: conversion allocated this descriptor and this owner is sole.
        unsafe { LocalFree(self.0) };
    }
}

impl Mapping {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: this wrapper is the sole owner of its mapping handle.
        unsafe { CloseHandle(self.0) };
    }
}

/// A read-only mapping handle prepared for explicit child inheritance.
///
/// The handle's name is `NULL`; it is reachable only through a retained or
/// inherited handle. The caller must still bind it to a fixed child inventory
/// before it can become transport.
pub(super) struct AuthenticatedCarrier {
    mapping: Mapping,
    length: usize,
    digest: [u8; 32],
}

/// The two authenticated inputs whose order is fixed by the signed capsule.
///
/// Keeping them in one owner prevents a caller from accidentally supplying a
/// request carrier from one capsule alongside a bundle carrier from another.
pub(super) struct AuthenticatedRequestBundle {
    request: AuthenticatedCarrier,
    bundle: AuthenticatedCarrier,
}

impl AuthenticatedRequestBundle {
    /// Copy the exact signed request and bundle bytes into separate anonymous,
    /// read-only child carriers. Both artifacts are authenticated before any
    /// token, job, scratch-root, or process effect is attempted.
    pub(super) fn create(
        request: &[u8],
        bundle: &[u8],
        capsule: &super::capsule::VerifiedCapsule,
    ) -> Result<Self, ()> {
        Ok(Self {
            request: AuthenticatedCarrier::create(request, capsule.request())?,
            bundle: AuthenticatedCarrier::create(bundle, capsule.bundle())?,
        })
    }

    /// The fixed request-then-bundle child inventory. These are the only
    /// carrier handles the confinement primitive may inherit into a child.
    pub(super) fn child_handles(&self) -> [HANDLE; 2] {
        [self.request.child_handle(), self.bundle.child_handle()]
    }
}

impl AuthenticatedCarrier {
    /// Copy and authenticate exact signed-artifact bytes, retaining only an
    /// inheritable read mapping. This consumes no pathname and starts no child.
    pub(super) fn create(bytes: &[u8], artifact: Artifact) -> Result<Self, ()> {
        if bytes.is_empty()
            || bytes.len() as u64 != artifact.length
            || artifact.length > MAX_ARTIFACT_BYTES
            || <[u8; 32]>::from(Sha256::digest(bytes)) != artifact.digest
        {
            return Err(());
        }
        let length = bytes.len();
        let length_u64 = u64::try_from(length).map_err(|_| ())?;
        let descriptor = MappingSecurity::create()?;
        let security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        // SAFETY: an unnamed paging-file mapping has an exact nonzero bounded
        // size. The live protected descriptor denies access escalation through
        // DuplicateHandle, including owner-mediated DACL changes. The null name
        // creates no ambient lookup path; transport passes the retained handle.
        let writable_raw = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                &security,
                PAGE_READWRITE,
                (length_u64 >> 32) as u32,
                length_u64 as u32,
                std::ptr::null(),
            )
        };
        if writable_raw.is_null() {
            return Err(());
        }
        let writable = Mapping(writable_raw);
        // SAFETY: `writable` owns a live mapping of exactly `length` bytes.
        // The returned view is uniquely used for the bounded copy and unmapped
        // before any read-only duplicate is retained.
        let view = unsafe { MapViewOfFile(writable.raw(), FILE_MAP_WRITE, 0, 0, length) };
        if view.Value.is_null() {
            return Err(());
        }
        // SAFETY: the writable view has exactly `length` addressable bytes and
        // `bytes` supplies exactly that many initialized source bytes.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), view.Value.cast(), length) };
        // SAFETY: this exact successful mapping is unmapped once before the
        // writable handle can be dropped.
        if unsafe { UnmapViewOfFile(view) } == 0 {
            return Err(());
        }

        let mut read_raw = std::ptr::null_mut();
        // SAFETY: duplicate the live mapping into this process with only the
        // mapping object's read right. `bInheritHandle` marks this exact
        // downscoped duplicate for a future explicit handle list; it does not
        // itself start or configure a child.
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                writable.raw(),
                GetCurrentProcess(),
                &mut read_raw,
                SECTION_MAP_READ,
                1,
                0,
            )
        } == 0
            || read_raw.is_null()
        {
            return Err(());
        }
        let carrier = Self {
            mapping: Mapping(read_raw),
            length,
            digest: artifact.digest,
        };
        drop(writable);

        let mut flags = 0u32;
        // SAFETY: `carrier.mapping` is live. Handle inheritance is kernel
        // metadata, distinct from mapping-object access rights.
        if unsafe { GetHandleInformation(carrier.mapping.raw(), &mut flags) } == 0
            || flags & HANDLE_FLAG_INHERIT == 0
            || !carrier.matches_artifact()
        {
            return Err(());
        }
        Ok(carrier)
    }

    pub(super) fn child_handle(&self) -> HANDLE {
        self.mapping.raw()
    }

    fn matches_artifact(&self) -> bool {
        // SAFETY: the retained handle has only read mapping access and the
        // mapping has the exact nonzero `length` retained by this object.
        let view = unsafe { MapViewOfFile(self.mapping.raw(), FILE_MAP_READ, 0, 0, self.length) };
        if view.Value.is_null() {
            return false;
        }
        // SAFETY: this read view has exactly `length` initialized bytes copied
        // before the only writable handle and view were released.
        let digest = unsafe {
            Sha256::digest(std::slice::from_raw_parts(
                view.Value.cast::<u8>(),
                self.length,
            ))
        };
        // SAFETY: unmap the exact read view before returning the comparison.
        (unsafe { UnmapViewOfFile(view) } != 0) && <[u8; 32]>::from(digest) == self.digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Memory::{MapViewOfFile, FILE_MAP_WRITE};
    use windows_sys::Win32::System::Threading::GetProcessHandleCount;

    fn artifact(bytes: &[u8]) -> Artifact {
        Artifact {
            length: bytes.len() as u64,
            digest: Sha256::digest(bytes).into(),
        }
    }

    fn assert_read_only(carrier: &AuthenticatedCarrier) {
        // SAFETY: the carrier retains an inheritable mapping handle that was
        // duplicated with SECTION_MAP_READ only. A writable view is the
        // hostile operation this test requires the kernel to refuse.
        let writable =
            unsafe { MapViewOfFile(carrier.child_handle(), FILE_MAP_WRITE, 0, 0, carrier.length) };
        assert!(
            writable.Value.is_null(),
            "downscoped inheritable carrier mapped writable"
        );
        assert!(carrier.matches_artifact());
    }

    fn handle_count() -> u32 {
        let mut count = 0;
        // SAFETY: the pseudo handle names this process and `count` is writable.
        assert_ne!(
            unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
            0
        );
        count
    }

    #[test]
    #[ignore = "requires the explicitly provisioned Windows runtime gate"]
    fn windows_runtime_authenticated_request_bundle_carriers_are_read_only() {
        let request = b"SPXDWK1\0request-carrier";
        let bundle: Vec<u8> = (0..257).map(|index| (index & 0xff) as u8).collect();
        let request_artifact = artifact(request);
        let bundle_artifact = artifact(&bundle);
        let mut forged = request_artifact;
        forged.digest[0] ^= 1;
        assert!(AuthenticatedCarrier::create(request, forged).is_err());

        let baseline = handle_count();
        let request = AuthenticatedCarrier::create(request, request_artifact).unwrap();
        let bundle = AuthenticatedCarrier::create(&bundle, bundle_artifact).unwrap();
        for carrier in [&request, &bundle] {
            assert_read_only(carrier);
        }
        drop(bundle);
        drop(request);
        assert_eq!(handle_count(), baseline);
    }

    #[test]
    #[ignore = "requires the explicitly provisioned Windows runtime gate"]
    fn windows_runtime_authenticated_carrier_rejects_invalid_artifacts_without_handles() {
        let bytes = b"authenticated-carrier";
        let baseline = handle_count();
        let cases = [
            (&b""[..], artifact(b"")),
            (
                &bytes[..],
                Artifact {
                    length: bytes.len() as u64 + 1,
                    digest: Sha256::digest(bytes).into(),
                },
            ),
            (
                &bytes[..],
                Artifact {
                    length: bytes.len() as u64,
                    digest: [0x55; 32],
                },
            ),
            (
                &bytes[..],
                Artifact {
                    length: MAX_ARTIFACT_BYTES + 1,
                    digest: Sha256::digest(bytes).into(),
                },
            ),
        ];
        for (input, expected) in cases {
            assert!(AuthenticatedCarrier::create(input, expected).is_err());
            assert_eq!(
                handle_count(),
                baseline,
                "invalid carrier artifact created or leaked a mapping handle"
            );
        }
    }

    #[test]
    #[ignore = "requires the explicitly provisioned Windows runtime gate"]
    fn windows_runtime_authenticated_carrier_repeated_create_drop_settles_one_handle() {
        // Initialize the Windows SDDL conversion path before measuring the
        // carrier's handle delta; its first use may load process-wide state.
        drop(MappingSecurity::create().unwrap());
        let baseline = handle_count();
        for length in [1usize, 257, 4096] {
            let bytes: Vec<u8> = (0..length).map(|index| (index & 0xff) as u8).collect();
            for _ in 0..4 {
                let carrier = AuthenticatedCarrier::create(&bytes, artifact(&bytes)).unwrap();
                assert_eq!(
                    handle_count(),
                    baseline + 1,
                    "carrier retains exactly its downscoped inheritable mapping handle"
                );
                assert_read_only(&carrier);
                drop(carrier);
                assert_eq!(
                    handle_count(),
                    baseline,
                    "carrier drop settles its retained mapping handle"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires the explicitly provisioned Windows runtime gate"]
    fn windows_runtime_authenticated_carriers_settle_independent_live_handles() {
        let baseline = handle_count();
        let payloads = [&b"one"[..], &b"two-carrier"[..], &b"three-carrier"[..]];
        let carriers: Vec<_> = payloads
            .iter()
            .map(|bytes| AuthenticatedCarrier::create(bytes, artifact(bytes)).unwrap())
            .collect();
        assert_eq!(
            handle_count(),
            baseline + carriers.len() as u32,
            "each independently authenticated carrier owns one retained mapping handle"
        );
        for carrier in &carriers {
            assert_read_only(carrier);
        }
        drop(carriers);
        assert_eq!(
            handle_count(),
            baseline,
            "independent carrier drops settle every retained mapping handle"
        );
    }
}
