//! Feature-gated C bridge for the private iOS Simulator callable-v3 gate.
//!
//! This module is deliberately absent from ordinary builds and has no Rust
//! re-export. Its one unmangled symbol exists only so a statically linked,
//! generated provider can cross the real Simulator C ABI into the unchanged
//! private host ledger while `SPX-B104` remains closed.

use std::mem;
use std::ptr;

use semaprax_native_loader::{
    register_admitted_ios_static_settlement_exact, IosStaticTarget, StaticDescriptorGetter,
    StaticExecuteEntry, StaticSettleEntry, MAX_DESCRIPTOR_BYTES,
};

use crate::callable_wire_v3::{ExecuteOutcome, Publication};
use crate::settlement_host_v3::PrivateSettlementHostV3;

const EVIDENCE_VERSION: u32 = 1;
const TARGET_SIMULATOR_ARM64: u32 = 1;
const OUTCOME_SCALAR_I64: u32 = 1;
const PUBLICATION_NONE: u32 = 1;

const STEP_NULL_EVIDENCE: u32 = 1;
const STEP_EVIDENCE_SIZE: u32 = 2;
const STEP_DESCRIPTOR_LENGTH: u32 = 3;
const STEP_NULL_DESCRIPTOR: u32 = 4;
const STEP_MISSING_GETTER: u32 = 5;
const STEP_MISSING_EXECUTE: u32 = 6;
const STEP_MISSING_SETTLE: u32 = 7;
const STEP_WRONG_TARGET: u32 = 8;
const STEP_FIRST_REGISTRATION: u32 = 9;
const STEP_SECOND_REGISTRATION: u32 = 10;
const STEP_INSTANCE_MISMATCH: u32 = 11;
const STEP_HOST_ADMISSION: u32 = 12;
const STEP_FIRST_OWNER: u32 = 13;
const STEP_SECOND_OWNER: u32 = 14;
const STEP_EXECUTION: u32 = 15;
const STEP_OUTCOME: u32 = 16;
const STEP_PUBLICATION: u32 = 17;
const STEP_RECEIPT: u32 = 18;
const STEP_IDENTITY_DIGESTS: u32 = 19;
const STEP_LEDGER_TRANSITION: u32 = 20;
const STEP_HOST_STATE: u32 = 21;
const STEP_POSTCOMMIT_ALLOCATION: u32 = 22;

/// Fixed, versioned evidence written only after the complete private call has
/// passed every host-side assertion. All fields use C-stable integer types;
/// zero remains the fail-closed/unwritten value for every proof flag.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PrivateIosSimulatorEvidenceV1 {
    size: u32,
    version: u32,
    target: u32,
    same_instance: u32,
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
    assert!(mem::size_of::<PrivateIosSimulatorEvidenceV1>() == 80);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, size) == 0);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, version) == 4);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, target) == 8);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, same_instance) == 12);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, module_instance_id) == 16);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, outcome) == 24);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, publication) == 28);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, receipt_nonzero) == 32);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, candidate_nonzero) == 36);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, identity_digests_nonzero) == 40);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, ledger_before_nonzero) == 44);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, ledger_after_nonzero) == 48);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, ledger_changed) == 52);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, poisoned) == 56);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, draining) == 60);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, quarantined_count) == 64);
    assert!(mem::offset_of!(PrivateIosSimulatorEvidenceV1, postcommit_allocations) == 72);
};

impl PrivateIosSimulatorEvidenceV1 {
    fn success(module_instance_id: u64, postcommit_allocations: u64) -> Self {
        Self {
            size: mem::size_of::<Self>() as u32,
            version: EVIDENCE_VERSION,
            target: TARGET_SIMULATOR_ARM64,
            same_instance: 1,
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
    not(all(target_os = "ios", target_abi = "sim")),
    allow(
        dead_code,
        reason = "the bridge logic is type-checked before Simulator linking"
    )
)]
unsafe fn run(
    descriptor: *const u8,
    descriptor_len: u32,
    getter: Option<StaticDescriptorGetter>,
    execute: Option<StaticExecuteEntry>,
    settle: Option<StaticSettleEntry>,
    evidence: *mut PrivateIosSimulatorEvidenceV1,
    evidence_len: u32,
) -> u32 {
    if evidence.is_null() {
        return STEP_NULL_EVIDENCE;
    }
    if evidence_len as usize != mem::size_of::<PrivateIosSimulatorEvidenceV1>() {
        return STEP_EVIDENCE_SIZE;
    }
    // SAFETY: The caller supplies an exact writable evidence object. Clear it
    // before any provider entry so every failure remains visibly incomplete.
    unsafe { ptr::write_bytes(evidence.cast::<u8>(), 0, evidence_len as usize) };

    let descriptor_len = descriptor_len as usize;
    if descriptor_len == 0 || descriptor_len > MAX_DESCRIPTOR_BYTES {
        return STEP_DESCRIPTOR_LENGTH;
    }
    if descriptor.is_null() {
        return STEP_NULL_DESCRIPTOR;
    }
    let Some(getter) = getter else {
        return STEP_MISSING_GETTER;
    };
    let Some(execute) = execute else {
        return STEP_MISSING_EXECUTE;
    };
    let Some(settle) = settle else {
        return STEP_MISSING_SETTLE;
    };
    if IosStaticTarget::current() != Some(IosStaticTarget::SimulatorArm64) {
        return STEP_WRONG_TARGET;
    }

    // SAFETY: The caller's contract establishes process-lifetime immutable
    // storage for this exact complete range.
    let descriptor: &'static [u8] =
        unsafe { std::slice::from_raw_parts(descriptor, descriptor_len) };
    // SAFETY: The caller establishes each process-lifetime ABI and the exact
    // descriptor address. Registration independently validates all of it.
    let first = match unsafe {
        register_admitted_ios_static_settlement_exact(
            IosStaticTarget::SimulatorArm64,
            descriptor,
            getter,
            execute,
            settle,
        )
    } {
        Ok(lease) => lease,
        Err(_) => return STEP_FIRST_REGISTRATION,
    };
    // SAFETY: Identical evidence and entries are intentionally re-registered
    // on the same thread to prove idempotent exact-instance retention.
    let second = match unsafe {
        register_admitted_ios_static_settlement_exact(
            IosStaticTarget::SimulatorArm64,
            descriptor,
            getter,
            execute,
            settle,
        )
    } {
        Ok(lease) => lease,
        Err(_) => return STEP_SECOND_REGISTRATION,
    };
    if !first.is_same_instance(&second) || first.instance_id() != second.instance_id() {
        return STEP_INSTANCE_MISMATCH;
    }
    let instance_id = first.instance_id();
    let host = match PrivateSettlementHostV3::from_static_admitted(first, descriptor) {
        Ok(host) => host,
        Err(_) => return STEP_HOST_ADMISSION,
    };
    if host.module_instance_id() != instance_id {
        return STEP_INSTANCE_MISMATCH;
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

    let completed =
        PrivateIosSimulatorEvidenceV1::success(instance_id.get(), postcommit_allocations as u64);
    // SAFETY: The exact writable output was validated and cleared above; it
    // does not alias any input under the caller contract.
    unsafe { evidence.write(completed) };
    0
}

/// Run one exact generated callable-v3 provider through static registration
/// and the existing private host ledger on an arm64 iOS Simulator process.
///
/// Returns zero only after complete evidence has been written. Every nonzero
/// value identifies the first failed step and leaves `evidence` all-zero.
///
/// # Safety
///
/// `descriptor` must address exactly `descriptor_len` readable, immutable
/// bytes for the process lifetime. The getter must synchronously return that
/// exact address. All three function pointers must implement the synchronous
/// SPXNABI3 contracts, must remain valid for the process lifetime, and must
/// never unwind, longjmp, retain pointers, or access outside the supplied
/// ranges. `evidence` must address writable storage of exactly `evidence_len`
/// bytes and must not alias descriptor or provider storage.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
#[no_mangle]
pub unsafe extern "C" fn spx_private_ios_simulator_v3_run(
    descriptor: *const u8,
    descriptor_len: u32,
    getter: Option<StaticDescriptorGetter>,
    execute: Option<StaticExecuteEntry>,
    settle: Option<StaticSettleEntry>,
    evidence: *mut PrivateIosSimulatorEvidenceV1,
    evidence_len: u32,
) -> u32 {
    // SAFETY: This wrapper preserves every pointer, length and function entry
    // supplied under its documented caller contract.
    unsafe {
        run(
            descriptor,
            descriptor_len,
            getter,
            execute,
            settle,
            evidence,
            evidence_len,
        )
    }
}
