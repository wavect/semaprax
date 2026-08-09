//! Feature-gated C bridge for the private Android Emulator callable-v3 gate.
//!
//! This module is absent from ordinary builds and has no Rust re-export. Its
//! one unmangled symbol exists only on Android so a standalone NDK executable
//! can drive an exact generated dynamic provider through the unchanged private
//! loader and host receipt ledger while `SPX-B104` remains closed. It is not a
//! JNI, Kotlin, AAR, Android application, or lifecycle integration boundary.

use std::mem;
use std::path::PathBuf;
use std::ptr;

use semaprax_native_loader::{open_admitted_settlement_exact, MAX_DESCRIPTOR_BYTES};

use crate::callable_wire_v3::{ExecuteOutcome, Publication};
use crate::descriptor_v3::Descriptor;
use crate::settlement_host_v3::PrivateSettlementHostV3;

const MAX_PATH_BYTES: usize = 4_096;
const EVIDENCE_VERSION: u32 = 1;
const TARGET_ANDROID_X86_64: u32 = 1;
const OUTCOME_SCALAR_I64: u32 = 1;
const PUBLICATION_NONE: u32 = 1;

const STEP_NULL_EVIDENCE: u32 = 1;
const STEP_EVIDENCE_SIZE: u32 = 2;
const STEP_PATH_LENGTH: u32 = 3;
const STEP_NULL_PATH: u32 = 4;
const STEP_PATH_UTF8: u32 = 5;
const STEP_DESCRIPTOR_LENGTH: u32 = 6;
const STEP_NULL_DESCRIPTOR: u32 = 7;
const STEP_DESCRIPTOR_PREFLIGHT: u32 = 8;
const STEP_IMAGE_ADMISSION: u32 = 9;
const STEP_INSTANCE_RETAIN: u32 = 10;
const STEP_HOST_ADMISSION: u32 = 11;
const STEP_FIRST_OWNER: u32 = 12;
const STEP_SECOND_OWNER: u32 = 13;
const STEP_EXECUTION: u32 = 14;
const STEP_OUTCOME: u32 = 15;
const STEP_PUBLICATION: u32 = 16;
const STEP_RECEIPT: u32 = 17;
const STEP_IDENTITY_DIGESTS: u32 = 18;
const STEP_LEDGER_TRANSITION: u32 = 19;
const STEP_HOST_STATE: u32 = 20;
const STEP_POSTCOMMIT_ALLOCATION: u32 = 21;

/// Fixed, versioned evidence published only after the complete private Android
/// call has passed. Zero remains the fail-closed value for every proof flag.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PrivateAndroidEmulatorEvidenceV1 {
    size: u32,
    version: u32,
    target: u32,
    retained_instance: u32,
    module_instance_id: u64,
    outcome: u32,
    publication: u32,
    receipt_nonzero: u32,
    candidate_nonzero: u32,
    identity_digests_nonzero: u32,
    ledger_before_nonzero: u32,
    ledger_after_nonzero: u32,
    ledger_changed: u32,
    poisoned: u32,
    draining: u32,
    quarantined_count: u32,
    postcommit_allocations: u64,
}

const _: () = {
    assert!(mem::size_of::<PrivateAndroidEmulatorEvidenceV1>() == 80);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, size) == 0);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, version) == 4);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, target) == 8);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, retained_instance) == 12);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, module_instance_id) == 16);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, outcome) == 24);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, publication) == 28);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, receipt_nonzero) == 32);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, identity_digests_nonzero) == 40);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, ledger_changed) == 52);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, quarantined_count) == 64);
    assert!(mem::offset_of!(PrivateAndroidEmulatorEvidenceV1, postcommit_allocations) == 72);
};

impl PrivateAndroidEmulatorEvidenceV1 {
    fn success(module_instance_id: u64, postcommit_allocations: u64) -> Self {
        Self {
            size: mem::size_of::<Self>() as u32,
            version: EVIDENCE_VERSION,
            target: TARGET_ANDROID_X86_64,
            retained_instance: 1,
            module_instance_id,
            outcome: OUTCOME_SCALAR_I64,
            publication: PUBLICATION_NONE,
            receipt_nonzero: 1,
            candidate_nonzero: 1,
            identity_digests_nonzero: 1,
            ledger_before_nonzero: 1,
            ledger_after_nonzero: 1,
            ledger_changed: 1,
            poisoned: 0,
            draining: 0,
            quarantined_count: 0,
            postcommit_allocations,
        }
    }
}

fn nonzero(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}

#[cfg_attr(
    not(target_os = "android"),
    allow(
        dead_code,
        reason = "the bridge logic is type-checked before Android linking"
    )
)]
unsafe fn run(
    provider_path: *const u8,
    provider_path_len: u32,
    descriptor: *const u8,
    descriptor_len: u32,
    evidence: *mut PrivateAndroidEmulatorEvidenceV1,
    evidence_len: u32,
) -> u32 {
    if evidence.is_null() {
        return STEP_NULL_EVIDENCE;
    }
    if evidence_len as usize != mem::size_of::<PrivateAndroidEmulatorEvidenceV1>() {
        return STEP_EVIDENCE_SIZE;
    }
    // SAFETY: The caller supplies an exact writable evidence object. Clear it
    // before parsing or opening anything so every failure remains incomplete.
    unsafe { ptr::write_bytes(evidence.cast::<u8>(), 0, evidence_len as usize) };

    let provider_path_len = provider_path_len as usize;
    if provider_path_len == 0 || provider_path_len > MAX_PATH_BYTES {
        return STEP_PATH_LENGTH;
    }
    if provider_path.is_null() {
        return STEP_NULL_PATH;
    }
    // SAFETY: The caller establishes this exact readable path range.
    let provider_path = unsafe { std::slice::from_raw_parts(provider_path, provider_path_len) };
    if provider_path.contains(&0) {
        return STEP_PATH_UTF8;
    }
    let Ok(provider_path) = std::str::from_utf8(provider_path) else {
        return STEP_PATH_UTF8;
    };
    let provider_path = PathBuf::from(provider_path);

    let descriptor_len = descriptor_len as usize;
    if descriptor_len == 0 || descriptor_len > MAX_DESCRIPTOR_BYTES {
        return STEP_DESCRIPTOR_LENGTH;
    }
    if descriptor.is_null() {
        return STEP_NULL_DESCRIPTOR;
    }
    // SAFETY: The caller establishes this exact immutable descriptor range for
    // at least the duration of this synchronous call.
    let descriptor = unsafe { std::slice::from_raw_parts(descriptor, descriptor_len) };

    // Complete independent host decoding precedes native image open. This is
    // deliberately redundant with the loader's bounded projection and the
    // host constructor's byte-for-byte parse after admission.
    if Descriptor::parse(descriptor).is_err() {
        return STEP_DESCRIPTOR_PREFLIGHT;
    }
    // SAFETY: The gate admits only its just-generated exact provider in a
    // private emulator directory. The generated entries satisfy the complete
    // synchronous no-unwind/no-retention contract documented by the loader.
    let lease = match unsafe { open_admitted_settlement_exact(&provider_path, descriptor) } {
        Ok(lease) => lease,
        Err(_) => return STEP_IMAGE_ADMISSION,
    };
    let retained = lease.retain();
    if !lease.is_same_instance(&retained) || lease.instance_id() != retained.instance_id() {
        return STEP_INSTANCE_RETAIN;
    }
    let instance_id = lease.instance_id();
    let host = match PrivateSettlementHostV3::from_admitted(lease, descriptor) {
        Ok(host) => host,
        Err(_) => return STEP_HOST_ADMISSION,
    };
    if host.module_instance_id() != instance_id {
        return STEP_INSTANCE_RETAIN;
    }
    let first_owner = match host.register_owner(401, 7) {
        Ok(owner) => owner,
        Err(_) => return STEP_FIRST_OWNER,
    };
    let second_owner = match host.register_owner(402, 7) {
        Ok(owner) => owner,
        Err(_) => return STEP_SECOND_OWNER,
    };
    let committed = match host.execute_owned_success(&[first_owner, second_owner], &[11, 13]) {
        Ok(committed) => committed,
        Err(_) => return STEP_EXECUTION,
    };
    if committed.outcome != (ExecuteOutcome::Scalar { value: 0 }) {
        return STEP_OUTCOME;
    }
    if committed.committed.publication != Publication::NoOwned
        || committed.committed.published_owner.is_some()
    {
        return STEP_PUBLICATION;
    }
    if !nonzero(&committed.committed.receipt) || !nonzero(&committed.candidate_bytes) {
        return STEP_RECEIPT;
    }
    if !nonzero(&committed.identity.call.call_contract)
        || !nonzero(&committed.identity.call.provider_challenge)
        || !nonzero(&committed.identity.recovery_contract)
        || !nonzero(&committed.identity.settlement_graph)
    {
        return STEP_IDENTITY_DIGESTS;
    }
    if !nonzero(&committed.committed.ledger_before)
        || !nonzero(&committed.committed.ledger_after)
        || committed.committed.ledger_before == committed.committed.ledger_after
    {
        return STEP_LEDGER_TRANSITION;
    }
    if host.is_poisoned() || host.is_draining() || host.quarantined_count() != 0 {
        return STEP_HOST_STATE;
    }
    let Some(postcommit_allocations) = crate::postcommit_allocation_probe::take_last() else {
        return STEP_POSTCOMMIT_ALLOCATION;
    };
    if postcommit_allocations != 0 {
        return STEP_POSTCOMMIT_ALLOCATION;
    }

    // Keep the independent exact-instance retain live through every assertion.
    drop(retained);
    let completed =
        PrivateAndroidEmulatorEvidenceV1::success(instance_id.get(), postcommit_allocations as u64);
    // SAFETY: The exact writable output was validated and cleared above; it
    // does not alias the immutable inputs under the caller contract.
    unsafe { evidence.write(completed) };
    0
}

/// Run one exact generated callable-v3 provider through dynamic admission and
/// the private host ledger in an x86_64 Android process.
///
/// Returns zero only after complete evidence has been written. Null or
/// wrong-sized output storage is rejected untouched. Once an exact non-null
/// output object is accepted it is cleared before every later validation or
/// image-open step, remains zero on failure, and is populated only on success.
///
/// # Safety
///
/// `provider_path` must address exactly `provider_path_len` readable UTF-8
/// bytes naming an absolute, canonical, stable provider path. `descriptor`
/// must address exactly `descriptor_len` immutable readable bytes.
/// `evidence` must address writable storage of exactly `evidence_len` bytes
/// and must not alias either input. The admitted provider and all reachable
/// initializer, entry, finalizer, and terminator code must satisfy the loader's
/// complete trusted-image and synchronous ABI contract.
#[cfg(all(target_os = "android", target_arch = "x86_64"))]
#[no_mangle]
pub unsafe extern "C" fn spx_private_android_emulator_v3_run(
    provider_path: *const u8,
    provider_path_len: u32,
    descriptor: *const u8,
    descriptor_len: u32,
    evidence: *mut PrivateAndroidEmulatorEvidenceV1,
    evidence_len: u32,
) -> u32 {
    // SAFETY: This wrapper preserves the complete pointer/length contract.
    unsafe {
        run(
            provider_path,
            provider_path_len,
            descriptor,
            descriptor_len,
            evidence,
            evidence_len,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonzero_evidence() -> PrivateAndroidEmulatorEvidenceV1 {
        PrivateAndroidEmulatorEvidenceV1 {
            size: 1,
            version: 1,
            target: 1,
            retained_instance: 1,
            module_instance_id: 1,
            outcome: 1,
            publication: 1,
            receipt_nonzero: 1,
            candidate_nonzero: 1,
            identity_digests_nonzero: 1,
            ledger_before_nonzero: 1,
            ledger_after_nonzero: 1,
            ledger_changed: 1,
            poisoned: 1,
            draining: 1,
            quarantined_count: 1,
            postcommit_allocations: 1,
        }
    }

    fn evidence_is_zero(evidence: &PrivateAndroidEmulatorEvidenceV1) -> bool {
        evidence.size == 0
            && evidence.version == 0
            && evidence.target == 0
            && evidence.retained_instance == 0
            && evidence.module_instance_id == 0
            && evidence.outcome == 0
            && evidence.publication == 0
            && evidence.receipt_nonzero == 0
            && evidence.candidate_nonzero == 0
            && evidence.identity_digests_nonzero == 0
            && evidence.ledger_before_nonzero == 0
            && evidence.ledger_after_nonzero == 0
            && evidence.ledger_changed == 0
            && evidence.poisoned == 0
            && evidence.draining == 0
            && evidence.quarantined_count == 0
            && evidence.postcommit_allocations == 0
    }

    #[test]
    fn bridge_rejects_output_shape_before_touching_inputs() {
        // SAFETY: Null is the intended first validation case and no pointer is
        // dereferenced before it returns.
        let null = unsafe { run(ptr::null(), 0, ptr::null(), 0, ptr::null_mut(), 0) };
        assert_eq!(null, STEP_NULL_EVIDENCE);

        let mut evidence = nonzero_evidence();
        // SAFETY: The evidence object is valid; the deliberately wrong length
        // is rejected before any input pointer is touched.
        let wrong_size = unsafe { run(ptr::null(), 0, ptr::null(), 0, &mut evidence, 79) };
        assert_eq!(wrong_size, STEP_EVIDENCE_SIZE);
        assert_eq!(evidence.size, 1);
    }

    #[test]
    fn malformed_path_and_descriptor_fail_closed_before_image_open() {
        let mut evidence = nonzero_evidence();
        // SAFETY: The exact evidence object is valid. Empty-path validation
        // precedes both null input pointers.
        let empty_path = unsafe {
            run(
                ptr::null(),
                0,
                ptr::null(),
                0,
                &mut evidence,
                mem::size_of::<PrivateAndroidEmulatorEvidenceV1>() as u32,
            )
        };
        assert_eq!(empty_path, STEP_PATH_LENGTH);
        assert!(evidence_is_zero(&evidence));

        let path_with_nul = b"/private/tmp/provider\0.so";
        evidence = nonzero_evidence();
        // SAFETY: Both supplied byte ranges and the output are exact and live.
        let nul_path = unsafe {
            run(
                path_with_nul.as_ptr(),
                path_with_nul.len() as u32,
                b"bad".as_ptr(),
                3,
                &mut evidence,
                mem::size_of::<PrivateAndroidEmulatorEvidenceV1>() as u32,
            )
        };
        assert_eq!(nul_path, STEP_PATH_UTF8);
        assert!(evidence_is_zero(&evidence));

        let path = b"/private/tmp/provider-does-not-exist.so";
        evidence = nonzero_evidence();
        // SAFETY: Both supplied byte ranges and the output are exact and live.
        // The malformed descriptor must reject before filesystem access.
        let malformed = unsafe {
            run(
                path.as_ptr(),
                path.len() as u32,
                b"bad".as_ptr(),
                3,
                &mut evidence,
                mem::size_of::<PrivateAndroidEmulatorEvidenceV1>() as u32,
            )
        };
        assert_eq!(malformed, STEP_DESCRIPTOR_PREFLIGHT);
        assert!(evidence_is_zero(&evidence));
    }
}
