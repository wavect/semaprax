//! Windows-only request/bundle carrier experiment.
//!
//! This module authenticates one signed-capsule artifact into an unnamed
//! paging-file mapping, then drops every writable mapping handle and view
//! before retaining one inheritable `SECTION_MAP_READ` duplicate. It is not
//! connected to process creation, request/bundle transport, or image launch.
//! In particular, an inheritable handle alone does not select a child input or
//! bind any launched image.
use semaprax_doctor_capsule::{Artifact, MAX_ARTIFACT_BYTES};
use sha2::{Digest as _, Sha256};
use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, GetCurrentProcess, GetHandleInformation, HANDLE,
    HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, FILE_MAP_WRITE,
    PAGE_READWRITE, SECTION_MAP_READ,
};

struct Mapping(HANDLE);

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
        // SAFETY: an unnamed paging-file mapping has an exact nonzero bounded
        // size. The null security descriptor and name create no ambient named
        // lookup path; later transport must pass the retained handle directly.
        let writable_raw = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
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
        let request_artifact = Artifact {
            length: request.len() as u64,
            digest: Sha256::digest(request).into(),
        };
        let bundle_artifact = Artifact {
            length: bundle.len() as u64,
            digest: Sha256::digest(&bundle).into(),
        };
        let mut forged = request_artifact;
        forged.digest[0] ^= 1;
        assert!(AuthenticatedCarrier::create(request, forged).is_err());

        let baseline = handle_count();
        let request = AuthenticatedCarrier::create(request, request_artifact).unwrap();
        let bundle = AuthenticatedCarrier::create(&bundle, bundle_artifact).unwrap();
        for carrier in [&request, &bundle] {
            // SAFETY: the carrier retains an inheritable mapping handle that
            // was duplicated with SECTION_MAP_READ only. A writable view is
            // the hostile operation this test requires the kernel to refuse.
            let writable = unsafe {
                MapViewOfFile(carrier.child_handle(), FILE_MAP_WRITE, 0, 0, carrier.length)
            };
            assert!(
                writable.Value.is_null(),
                "downscoped inheritable carrier mapped writable"
            );
            assert!(carrier.matches_artifact());
        }
        drop(bundle);
        drop(request);
        assert_eq!(handle_count(), baseline);
    }
}
