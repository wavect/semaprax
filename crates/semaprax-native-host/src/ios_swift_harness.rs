//! Private Swift ownership adapter over the exact iOS static callable-v3 host.
//!
//! This module is feature-gated, not re-exported, and keeps the loader lease,
//! host, handles, and finalizer hooks in same-thread TLS. It does not open the
//! public resource backend or weaken `SPX-B104`.

#![allow(dead_code, reason = "linked only by the private Apple evidence gate")]

use std::cell::RefCell;
use std::mem;
#[cfg(target_os = "ios")]
use std::ptr;
use std::sync::atomic::AtomicU32;
#[cfg(target_os = "ios")]
use std::sync::atomic::Ordering;

use semaprax_native_loader::IosStaticTarget;
#[cfg(target_os = "ios")]
use semaprax_native_loader::{
    register_admitted_ios_static_settlement_exact, StaticDescriptorGetter, StaticExecuteEntry,
    StaticSettleEntry,
};

#[cfg(target_os = "ios")]
use crate::callable_wire_v3::{ExecuteOutcome, Publication};
#[cfg(target_os = "ios")]
use crate::descriptor_v3::Descriptor;
#[cfg(not(target_os = "ios"))]
use crate::settlement_host_v3::PrivateSettlementHostV3;
#[cfg(target_os = "ios")]
use crate::settlement_host_v3::{
    PrivateSettlementArgumentV3, PrivateSettlementExecutionError, PrivateSettlementHostV3,
};
#[cfg(target_os = "ios")]
use crate::settlement_ledger::SettlementLedgerError;

type ResetHook = unsafe extern "C" fn() -> u32;
type SnapshotHook = unsafe extern "C" fn(*mut u32, *mut u32, *mut u64, u32) -> u32;

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn spx_private_apple_swift_fixture_reset_v1() -> u32;
    fn spx_private_apple_swift_fixture_snapshot_v1(
        count: *mut u32,
        ordinals: *mut u32,
        payloads: *mut u64,
        capacity: u32,
    ) -> u32;
}

const CAPACITY: usize = 8;
const MASK: u64 = (1 << 24) - 1;
const TAG_SHIFT: u32 = 48;
const GENERATION_SHIFT: u32 = 24;
const TAG_MASK: u64 = (1 << 15) - 1;
const HANDLE_KAT: u64 = 0x0001_0000_0100_0001;
const PROOF_FLAGS: u64 = 0x0f;
const REQUIRES_FALSE_PAYLOAD: u64 = u64::MAX;
const REQUIRES_FALSE_SELECTED_ORDINAL: u32 = 1;
const IDENTITY_MAX_PAYLOAD: u64 = u64::MAX;
const IDENTITY_MAX_OWNER_ORDINAL: u32 = 0;
const IDENTITY_MAX_PUBLICATIONS: u64 = 2;
const CHECKED_ADD_OVERFLOW_PAYLOAD: u64 = u64::MAX;
const CHECKED_ADD_OVERFLOW_I64: i64 = i64::MAX;
const CHECKED_ADD_OVERFLOW_SELECTED_ORDINAL: u32 = 2;
const ENSURES_FALSE_PAYLOAD: u64 = u64::MAX;
const ENSURES_FALSE_SELECTED_ORDINAL: u32 = 3;
const OWNER_GENERATION: u64 = 1;

const CODE_INVALID_ARGUMENT: u32 = 1;
const CODE_WRONG_THREAD: u32 = 2;
const CODE_CAPACITY: u32 = 3;
const CODE_DRAINING: u32 = 4;
const CODE_LIVE: u32 = 5;
const CODE_ADMISSION: u32 = 6;
const CODE_INVALID_HANDLE: u32 = 7;
const CODE_STALE: u32 = 8;
const CODE_CROSS_RUNTIME: u32 = 9;
const CODE_ALREADY_OPEN: u32 = 10;
const CODE_REENTRANT: u32 = 11;
const CODE_WRONG_PAYLOAD: u32 = 12;
const CODE_UNCERTAIN: u32 = 0x8000_0001;
const CODE_PANIC: u32 = 0x8000_0002;
const CODE_UNHEALTHY: u32 = 0x8000_0003;
const CODE_EVIDENCE: u32 = 0x8000_0004;

const fn status(code: u32) -> u64 {
    (1_u64 << 37) | (5_u64 << 32) | (1_u64 << 35) | code as u64
}

const _: () = {
    assert!(HANDLE_KAT == ((1 << 48) | (1 << 24) | 1));
    assert!(status(1) == 0x0000_002d_0000_0001);
};

static NEXT_TAG: AtomicU32 = AtomicU32::new(1);
thread_local! { static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) }; }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionShape {
    Pair,
    SingleWitness,
    SingleOwnedResult,
    CheckedAddOverflow,
    EnsuresFalse,
}

#[derive(Clone, Copy)]
struct Session {
    shape: SessionShape,
    payloads: [u64; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State<T> {
    Vacant,
    Live(T),
    Claimed(T),
    Consumed,
    Quarantined,
    Retired,
}

#[derive(Clone, Copy)]
struct Slot<T> {
    generation: u32,
    state: State<T>,
}

#[derive(Debug, Eq, PartialEq)]
enum TableError {
    Capacity,
    Invalid,
    Stale,
    Cross,
    Draining,
}

struct Table<T> {
    tag: u16,
    draining: bool,
    slots: [Slot<T>; CAPACITY],
}

impl<T: Copy> Table<T> {
    fn new(tag: u16) -> Self {
        Self {
            tag,
            draining: false,
            slots: [Slot {
                generation: 1,
                state: State::Vacant,
            }; CAPACITY],
        }
    }

    fn insert(&mut self, value: T) -> Result<u64, TableError> {
        if self.draining {
            return Err(TableError::Draining);
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if matches!(slot.state, State::Consumed) {
                if slot.generation == MASK as u32 {
                    slot.state = State::Retired;
                    continue;
                }
                slot.generation += 1;
                slot.state = State::Vacant;
            }
            if matches!(slot.state, State::Vacant) {
                slot.state = State::Live(value);
                return Ok(encode(self.tag, slot.generation, index as u32 + 1));
            }
        }
        Err(TableError::Capacity)
    }

    fn exact(&mut self, handle: u64) -> Result<&mut Slot<T>, TableError> {
        let (tag, generation, slot) = decode(handle)?;
        if tag != self.tag {
            return Err(TableError::Cross);
        }
        let entry = self
            .slots
            .get_mut(slot as usize - 1)
            .ok_or(TableError::Invalid)?;
        if entry.generation != generation {
            return Err(TableError::Stale);
        }
        Ok(entry)
    }

    fn claim(&mut self, handle: u64) -> Result<T, TableError> {
        if self.draining {
            return Err(TableError::Draining);
        }
        let slot = self.exact(handle)?;
        match slot.state {
            State::Live(value) => {
                slot.state = State::Claimed(value);
                Ok(value)
            }
            State::Consumed | State::Retired => Err(TableError::Stale),
            _ => Err(TableError::Invalid),
        }
    }

    fn restore(&mut self, handle: u64) {
        if let Ok(slot) = self.exact(handle) {
            if let State::Claimed(v) = slot.state {
                slot.state = State::Live(v);
            }
        }
    }
    fn consume(&mut self, handle: u64) {
        if let Ok(slot) = self.exact(handle) {
            if matches!(slot.state, State::Claimed(_)) {
                slot.state = State::Consumed;
            }
        }
    }
    fn quarantine(&mut self, handle: u64) {
        if let Ok(slot) = self.exact(handle) {
            slot.state = State::Quarantined;
        }
        self.draining = true;
    }
    fn quarantine_active(&mut self) {
        for slot in &mut self.slots {
            if matches!(slot.state, State::Live(_) | State::Claimed(_)) {
                slot.state = State::Quarantined;
            }
        }
        self.draining = true;
    }
    fn empty(&self) -> bool {
        self.slots
            .iter()
            .all(|s| matches!(s.state, State::Vacant | State::Consumed | State::Retired))
    }
    fn quarantined(&self) -> bool {
        self.slots
            .iter()
            .any(|s| matches!(s.state, State::Quarantined))
    }
}

const fn encode(tag: u16, generation: u32, slot: u32) -> u64 {
    ((tag as u64) << TAG_SHIFT) | ((generation as u64) << GENERATION_SHIFT) | slot as u64
}

fn decode(handle: u64) -> Result<(u16, u32, u32), TableError> {
    if handle == 0 || handle >> 63 != 0 {
        return Err(TableError::Invalid);
    }
    let tag = ((handle >> TAG_SHIFT) & TAG_MASK) as u16;
    let generation = ((handle >> GENERATION_SHIFT) & MASK) as u32;
    let slot = (handle & MASK) as u32;
    if tag == 0 || generation == 0 || slot == 0 {
        return Err(TableError::Invalid);
    }
    Ok((tag, generation, slot))
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PrivateAppleSwiftEvidenceV1 {
    words: [u64; 8],
}
const _: () = assert!(mem::size_of::<PrivateAppleSwiftEvidenceV1>() == 64);

struct Runtime {
    host: PrivateSettlementHostV3,
    table: Table<Session>,
    next_owner: u64,
    reset: ResetHook,
    snapshot: SnapshotHook,
    poisoned: bool,
}

#[cfg(target_os = "ios")]
impl Runtime {
    fn adopt_single_witness(&mut self, payload: u64) -> Result<u64, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        if payload != REQUIRES_FALSE_PAYLOAD {
            return Err(status(CODE_WRONG_PAYLOAD));
        }
        self.table
            .insert(Session {
                shape: SessionShape::SingleWitness,
                payloads: [payload, 0],
            })
            .map_err(map_table)
    }

    fn adopt_owned_result(&mut self, payload: u64) -> Result<u64, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        if payload != IDENTITY_MAX_PAYLOAD {
            return Err(status(CODE_WRONG_PAYLOAD));
        }
        self.table
            .insert(Session {
                shape: SessionShape::SingleOwnedResult,
                payloads: [payload, 0],
            })
            .map_err(map_table)
    }

    fn adopt_checked_add_overflow(&mut self, payload: u64) -> Result<u64, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        if payload != CHECKED_ADD_OVERFLOW_PAYLOAD {
            return Err(status(CODE_WRONG_PAYLOAD));
        }
        self.table
            .insert(Session {
                shape: SessionShape::CheckedAddOverflow,
                payloads: [payload, 0],
            })
            .map_err(map_table)
    }

    fn adopt_ensures_false(&mut self, payload: u64) -> Result<u64, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        if payload != ENSURES_FALSE_PAYLOAD {
            return Err(status(CODE_WRONG_PAYLOAD));
        }
        self.table
            .insert(Session {
                shape: SessionShape::EnsuresFalse,
                payloads: [payload, 0],
            })
            .map_err(map_table)
    }

    fn requires_false_witness(&mut self, handle: u64) -> Result<PrivateAppleSwiftEvidenceV1, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        let session = self.table.claim(handle).map_err(map_table)?;
        if session.shape != SessionShape::SingleWitness {
            self.table.restore(handle);
            return Err(status(CODE_INVALID_HANDLE));
        }
        let payload = session.payloads[0];
        if unsafe { (self.reset)() } != 0 {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        let slot = self.next_owner;
        let Some(next) = slot.checked_add(1) else {
            self.table.restore(handle);
            return Err(status(CODE_CAPACITY));
        };
        let owner = match self.host.register_owner(slot, OWNER_GENERATION) {
            Ok(owner) => owner,
            Err(_) => {
                self.table.restore(handle);
                return Err(status(CODE_CAPACITY));
            }
        };
        self.next_owner = next;
        // The canonical `requires-false` corpus witness: one owned argument at
        // the corpus-maximum payload plus one false boolean scalar.
        let arguments = [
            PrivateSettlementArgumentV3::Owned {
                handle: owner,
                payload,
            },
            PrivateSettlementArgumentV3::Bool(false),
        ];
        let committed = match self.host.execute_canonical(&arguments) {
            Ok(committed) => committed,
            Err(_) => {
                self.poisoned = true;
                self.table.quarantine(handle);
                return Err(status(CODE_UNCERTAIN));
            }
        };
        let allocations = crate::postcommit_allocation_probe::take_last();
        let mut count = 0;
        let mut ordinals = [0; 2];
        let mut payloads = [0; 2];
        let trace =
            unsafe { (self.snapshot)(&mut count, ordinals.as_mut_ptr(), payloads.as_mut_ptr(), 2) };
        let healthy = !self.host.is_poisoned()
            && !self.host.is_draining()
            && self.host.quarantined_count() == 0;
        if committed.outcome
            != (ExecuteOutcome::SemanticFailure {
                selected_ordinal: REQUIRES_FALSE_SELECTED_ORDINAL,
            })
            || committed.committed.publication != Publication::NoOwned
            || committed.committed.published_owner.is_some()
            || allocations != Some(0)
            || trace != 0
            || count != 1
            || ordinals != [0, 0]
            || payloads != [REQUIRES_FALSE_PAYLOAD, 0]
            || !healthy
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        if self
            .host
            .replay_committed(committed.identity, &committed.candidate_bytes)
            != Ok(committed.committed)
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        // Failure selection is sticky: the consumed owner must make a second
        // canonical execution fail closed without poisoning the host.
        if self.host.execute_canonical(&arguments)
            != Err(PrivateSettlementExecutionError::Ledger(
                SettlementLedgerError::StaleOwner,
            ))
            || self.host.is_poisoned()
            || self.host.is_draining()
            || self.host.quarantined_count() != 0
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        self.table.consume(handle);
        Ok(PrivateAppleSwiftEvidenceV1 {
            words: [
                1,
                self.host.module_instance_id().get(),
                u64::from(REQUIRES_FALSE_SELECTED_ORDINAL),
                0,
                1,
                REQUIRES_FALSE_PAYLOAD,
                0,
                0,
            ],
        })
    }

    fn identity_max_witness(&mut self, handle: u64) -> Result<PrivateAppleSwiftEvidenceV1, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        let session = self.table.claim(handle).map_err(map_table)?;
        if session.shape != SessionShape::SingleOwnedResult {
            self.table.restore(handle);
            return Err(status(CODE_INVALID_HANDLE));
        }
        let payload = session.payloads[0];
        if unsafe { (self.reset)() } != 0 {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        let slot = self.next_owner;
        let Some(next) = slot.checked_add(1) else {
            self.table.restore(handle);
            return Err(status(CODE_CAPACITY));
        };
        let owner = match self.host.register_owner(slot, OWNER_GENERATION) {
            Ok(owner) => owner,
            Err(_) => {
                self.table.restore(handle);
                return Err(status(CODE_CAPACITY));
            }
        };
        self.next_owner = next;
        // The canonical `identity-max` corpus witness: one owned argument at
        // the corpus-maximum payload is published outward as the owned result.
        let arguments = [PrivateSettlementArgumentV3::Owned {
            handle: owner,
            payload,
        }];
        let committed = match self.host.execute_canonical(&arguments) {
            Ok(committed) => committed,
            Err(_) => {
                self.poisoned = true;
                self.table.quarantine(handle);
                return Err(status(CODE_UNCERTAIN));
            }
        };
        let allocations = crate::postcommit_allocation_probe::take_last();
        let mut count = 0;
        let mut ordinals = [0; 2];
        let mut payloads = [0; 2];
        let trace =
            unsafe { (self.snapshot)(&mut count, ordinals.as_mut_ptr(), payloads.as_mut_ptr(), 2) };
        let healthy = !self.host.is_poisoned()
            && !self.host.is_draining()
            && self.host.quarantined_count() == 0;
        if committed.outcome
            != (ExecuteOutcome::Owned {
                owner_ordinal: IDENTITY_MAX_OWNER_ORDINAL,
                payload,
            })
            || committed.committed.publication != Publication::Owned(IDENTITY_MAX_OWNER_ORDINAL)
            || committed.committed.published_owner.is_none()
            || allocations != Some(0)
            || trace != 0
            || count != 0
            || ordinals != [0, 0]
            || payloads != [0, 0]
            || !healthy
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        if self
            .host
            .replay_committed(committed.identity, &committed.candidate_bytes)
            != Ok(committed.committed)
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        // Outward publication rotates the consumed generation in place: a stale
        // replay of the exact pre-publication argument must fail closed without
        // poisoning the host.
        if self.host.execute_canonical(&arguments)
            != Err(PrivateSettlementExecutionError::Ledger(
                SettlementLedgerError::StaleOwner,
            ))
            || self.host.is_poisoned()
            || self.host.is_draining()
            || self.host.quarantined_count() != 0
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        let refreshed = match committed.committed.published_owner {
            Some(owner) => owner,
            None => {
                self.poisoned = true;
                self.table.quarantine(handle);
                return Err(status(CODE_EVIDENCE));
            }
        };
        // The refreshed published owner is a live caller-owned capability: it
        // must re-adopt and publish exactly one more owned result.
        let readopted = [PrivateSettlementArgumentV3::Owned {
            handle: refreshed,
            payload,
        }];
        let second = match self.host.execute_canonical(&readopted) {
            Ok(committed) => committed,
            Err(_) => {
                self.poisoned = true;
                self.table.quarantine(handle);
                return Err(status(CODE_UNCERTAIN));
            }
        };
        let reallocated = crate::postcommit_allocation_probe::take_last();
        let still_healthy = !self.host.is_poisoned()
            && !self.host.is_draining()
            && self.host.quarantined_count() == 0;
        if second.outcome
            != (ExecuteOutcome::Owned {
                owner_ordinal: IDENTITY_MAX_OWNER_ORDINAL,
                payload,
            })
            || second.committed.publication != Publication::Owned(IDENTITY_MAX_OWNER_ORDINAL)
            || second.committed.published_owner.is_none()
            || reallocated != Some(0)
            || !still_healthy
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        if self
            .host
            .replay_committed(second.identity, &second.candidate_bytes)
            != Ok(second.committed)
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        self.table.consume(handle);
        Ok(PrivateAppleSwiftEvidenceV1 {
            words: [
                1,
                self.host.module_instance_id().get(),
                IDENTITY_MAX_PUBLICATIONS,
                0,
                0,
                0,
                0,
                0,
            ],
        })
    }

    fn checked_add_overflow_witness(
        &mut self,
        handle: u64,
    ) -> Result<PrivateAppleSwiftEvidenceV1, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        let session = self.table.claim(handle).map_err(map_table)?;
        if session.shape != SessionShape::CheckedAddOverflow {
            self.table.restore(handle);
            return Err(status(CODE_INVALID_HANDLE));
        }
        let payload = session.payloads[0];
        if unsafe { (self.reset)() } != 0 {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        let slot = self.next_owner;
        let Some(next) = slot.checked_add(1) else {
            self.table.restore(handle);
            return Err(status(CODE_CAPACITY));
        };
        let owner = match self.host.register_owner(slot, OWNER_GENERATION) {
            Ok(owner) => owner,
            Err(_) => {
                self.table.restore(handle);
                return Err(status(CODE_CAPACITY));
            }
        };
        self.next_owner = next;
        // The canonical `checked-add-overflow` corpus witness: one owned
        // argument at the corpus-maximum payload plus one i64::MAX scalar that
        // overflows the checked addition, publishing no owned result and
        // finalizing exactly that one owner after selection.
        let arguments = [
            PrivateSettlementArgumentV3::Owned {
                handle: owner,
                payload,
            },
            PrivateSettlementArgumentV3::I64(CHECKED_ADD_OVERFLOW_I64),
        ];
        let committed = match self.host.execute_canonical(&arguments) {
            Ok(committed) => committed,
            Err(_) => {
                self.poisoned = true;
                self.table.quarantine(handle);
                return Err(status(CODE_UNCERTAIN));
            }
        };
        let allocations = crate::postcommit_allocation_probe::take_last();
        let mut count = 0;
        let mut ordinals = [0; 2];
        let mut payloads = [0; 2];
        let trace =
            unsafe { (self.snapshot)(&mut count, ordinals.as_mut_ptr(), payloads.as_mut_ptr(), 2) };
        let healthy = !self.host.is_poisoned()
            && !self.host.is_draining()
            && self.host.quarantined_count() == 0;
        if committed.outcome
            != (ExecuteOutcome::SemanticFailure {
                selected_ordinal: CHECKED_ADD_OVERFLOW_SELECTED_ORDINAL,
            })
            || committed.committed.publication != Publication::NoOwned
            || committed.committed.published_owner.is_some()
            || allocations != Some(0)
            || trace != 0
            || count != 1
            || ordinals != [0, 0]
            || payloads != [CHECKED_ADD_OVERFLOW_PAYLOAD, 0]
            || !healthy
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        if self
            .host
            .replay_committed(committed.identity, &committed.candidate_bytes)
            != Ok(committed.committed)
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        // Failure selection is sticky: the consumed owner must make a second
        // canonical execution fail closed without poisoning the host.
        if self.host.execute_canonical(&arguments)
            != Err(PrivateSettlementExecutionError::Ledger(
                SettlementLedgerError::StaleOwner,
            ))
            || self.host.is_poisoned()
            || self.host.is_draining()
            || self.host.quarantined_count() != 0
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        self.table.consume(handle);
        Ok(PrivateAppleSwiftEvidenceV1 {
            words: [
                1,
                self.host.module_instance_id().get(),
                u64::from(CHECKED_ADD_OVERFLOW_SELECTED_ORDINAL),
                0,
                1,
                CHECKED_ADD_OVERFLOW_PAYLOAD,
                0,
                0,
            ],
        })
    }

    fn ensures_false_witness(&mut self, handle: u64) -> Result<PrivateAppleSwiftEvidenceV1, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(status(CODE_UNHEALTHY));
        }
        let session = self.table.claim(handle).map_err(map_table)?;
        if session.shape != SessionShape::EnsuresFalse {
            self.table.restore(handle);
            return Err(status(CODE_INVALID_HANDLE));
        }
        let payload = session.payloads[0];
        if unsafe { (self.reset)() } != 0 {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        let slot = self.next_owner;
        let Some(next) = slot.checked_add(1) else {
            self.table.restore(handle);
            return Err(status(CODE_CAPACITY));
        };
        let owner = match self.host.register_owner(slot, OWNER_GENERATION) {
            Ok(owner) => owner,
            Err(_) => {
                self.table.restore(handle);
                return Err(status(CODE_CAPACITY));
            }
        };
        self.next_owner = next;
        // The canonical `ensures-false` corpus witness: one owned argument at
        // the corpus-maximum payload that fails the `ensures false`
        // postcondition, publishing no owned result and finalizing exactly that
        // one owner after selection.
        let arguments = [PrivateSettlementArgumentV3::Owned {
            handle: owner,
            payload,
        }];
        let committed = match self.host.execute_canonical(&arguments) {
            Ok(committed) => committed,
            Err(_) => {
                self.poisoned = true;
                self.table.quarantine(handle);
                return Err(status(CODE_UNCERTAIN));
            }
        };
        let allocations = crate::postcommit_allocation_probe::take_last();
        let mut count = 0;
        let mut ordinals = [0; 2];
        let mut payloads = [0; 2];
        let trace =
            unsafe { (self.snapshot)(&mut count, ordinals.as_mut_ptr(), payloads.as_mut_ptr(), 2) };
        let healthy = !self.host.is_poisoned()
            && !self.host.is_draining()
            && self.host.quarantined_count() == 0;
        if committed.outcome
            != (ExecuteOutcome::SemanticFailure {
                selected_ordinal: ENSURES_FALSE_SELECTED_ORDINAL,
            })
            || committed.committed.publication != Publication::NoOwned
            || committed.committed.published_owner.is_some()
            || allocations != Some(0)
            || trace != 0
            || count != 1
            || ordinals != [0, 0]
            || payloads != [ENSURES_FALSE_PAYLOAD, 0]
            || !healthy
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        if self
            .host
            .replay_committed(committed.identity, &committed.candidate_bytes)
            != Ok(committed.committed)
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        // Failure selection is sticky: the consumed owner must make a second
        // canonical execution fail closed without poisoning the host.
        if self.host.execute_canonical(&arguments)
            != Err(PrivateSettlementExecutionError::Ledger(
                SettlementLedgerError::StaleOwner,
            ))
            || self.host.is_poisoned()
            || self.host.is_draining()
            || self.host.quarantined_count() != 0
        {
            self.poisoned = true;
            self.table.quarantine(handle);
            return Err(status(CODE_EVIDENCE));
        }
        self.table.consume(handle);
        Ok(PrivateAppleSwiftEvidenceV1 {
            words: [
                1,
                self.host.module_instance_id().get(),
                u64::from(ENSURES_FALSE_SELECTED_ORDINAL),
                0,
                1,
                ENSURES_FALSE_PAYLOAD,
                0,
                0,
            ],
        })
    }
}

fn map_table(error: TableError) -> u64 {
    status(match error {
        TableError::Capacity => CODE_CAPACITY,
        TableError::Invalid => CODE_INVALID_HANDLE,
        TableError::Stale => CODE_STALE,
        TableError::Cross => CODE_CROSS_RUNTIME,
        TableError::Draining => CODE_DRAINING,
    })
}

fn target(tag: u32) -> Option<IosStaticTarget> {
    match tag {
        1 => Some(IosStaticTarget::DeviceArm64),
        2 => Some(IosStaticTarget::SimulatorArm64),
        3 => Some(IosStaticTarget::SimulatorX86_64),
        _ => None,
    }
}

fn with_runtime<T>(f: impl FnOnce(&mut Runtime) -> Result<T, u64>) -> Result<T, u64> {
    RUNTIME.with(|cell| {
        let mut cell = cell.try_borrow_mut().map_err(|_| status(CODE_REENTRANT))?;
        let runtime = cell.as_mut().ok_or_else(|| status(CODE_WRONG_THREAD))?;
        f(runtime)
    })
}

fn guard(f: impl FnOnce() -> u64) -> u64 {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            RUNTIME.with(|cell| {
                if let Ok(mut cell) = cell.try_borrow_mut() {
                    if let Some(runtime) = cell.as_mut() {
                        runtime.poisoned = true;
                        runtime.table.quarantine_active();
                    }
                }
            });
            status(CODE_PANIC)
        }
    }
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_fixture_register_v1(
    target_tag: u32,
    descriptor: *const u8,
    descriptor_len: u32,
    getter: Option<StaticDescriptorGetter>,
    execute: Option<StaticExecuteEntry>,
    settle: Option<StaticSettleEntry>,
) -> u64 {
    guard(|| {
        let availability = RUNTIME.with(|cell| match cell.try_borrow() {
            Ok(cell) if cell.is_none() => 0,
            Ok(_) => status(CODE_ALREADY_OPEN),
            Err(_) => status(CODE_REENTRANT),
        });
        if availability != 0 {
            return availability;
        }
        let Some(expected_target) = target(target_tag) else {
            return status(CODE_INVALID_ARGUMENT);
        };
        if IosStaticTarget::current() != Some(expected_target) {
            return status(CODE_ADMISSION);
        }
        if descriptor.is_null()
            || descriptor_len == 0
            || descriptor_len as usize > semaprax_native_loader::MAX_DESCRIPTOR_BYTES
        {
            return status(CODE_INVALID_ARGUMENT);
        }
        // SAFETY: the generated fixture guarantees process-lifetime descriptor storage.
        let descriptor = unsafe { std::slice::from_raw_parts(descriptor, descriptor_len as usize) };
        if Descriptor::parse(descriptor).is_err() {
            return status(CODE_ADMISSION);
        }
        let (Some(getter), Some(execute), Some(settle)) = (getter, execute, settle) else {
            return status(CODE_INVALID_ARGUMENT);
        };
        let lease = match unsafe {
            register_admitted_ios_static_settlement_exact(
                expected_target,
                descriptor,
                getter,
                execute,
                settle,
            )
        } {
            Ok(lease) => lease,
            Err(_) => return status(CODE_ADMISSION),
        };
        let host = match PrivateSettlementHostV3::from_static_admitted(lease, descriptor) {
            Ok(host) => host,
            Err(_) => return status(CODE_ADMISSION),
        };
        let tag = match NEXT_TAG.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            // The issued tag itself must stay within the tag field;
            // accepting `v == TAG_MASK` would mint tag MASK+1, whose shifted
            // encoding sets the invalid-handle sign bit and permanently
            // poisons this runtime's handles.
            (v < TAG_MASK as u32).then_some(v + 1)
        }) {
            Ok(tag) => tag as u16,
            Err(_) => return status(CODE_CAPACITY),
        };
        RUNTIME.with(|cell| {
            let Ok(mut cell) = cell.try_borrow_mut() else {
                return status(CODE_REENTRANT);
            };
            if cell.is_some() {
                return status(CODE_ALREADY_OPEN);
            }
            *cell = Some(Runtime {
                host,
                table: Table::new(tag),
                next_owner: 1,
                reset: spx_private_apple_swift_fixture_reset_v1,
                snapshot: spx_private_apple_swift_fixture_snapshot_v1,
                poisoned: false,
            });
            0
        })
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_adopt_pair(
    first: u64,
    second: u64,
    output: *mut u64,
) -> u64 {
    guard(|| {
        if output.is_null()
            || (output as usize) % mem::align_of::<u64>() != 0
            || first != 11
            || second != 13
        {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| {
            if runtime.poisoned {
                return Err(status(CODE_UNHEALTHY));
            }
            runtime
                .table
                .insert(Session {
                    shape: SessionShape::Pair,
                    payloads: [first, second],
                })
                .map_err(map_table)
        }) {
            Ok(handle) => {
                unsafe { output.write(handle) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_consume(
    handle: u64,
    output: *mut PrivateAppleSwiftEvidenceV1,
    output_len: u32,
) -> u64 {
    guard(|| {
        if output.is_null()
            || output_len as usize != mem::size_of::<PrivateAppleSwiftEvidenceV1>()
            || (output as usize) % mem::align_of::<PrivateAppleSwiftEvidenceV1>() != 0
        {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| {
            if runtime.poisoned {
                return Err(status(CODE_UNHEALTHY));
            }
            let session = runtime.table.claim(handle).map_err(map_table)?;
            if session.shape != SessionShape::Pair {
                runtime.table.restore(handle);
                return Err(status(CODE_INVALID_HANDLE));
            }
            if unsafe { (runtime.reset)() } != 0 {
                runtime.poisoned = true;
                runtime.table.quarantine_active();
                return Err(status(CODE_EVIDENCE));
            }
            let first = runtime.next_owner;
            let Some(second) = first.checked_add(1) else {
                runtime.table.restore(handle);
                return Err(status(CODE_CAPACITY));
            };
            let Some(next) = second.checked_add(1) else {
                runtime.table.restore(handle);
                return Err(status(CODE_CAPACITY));
            };
            let owners = match runtime.host.register_owner_pair([(first, 1), (second, 1)]) {
                Ok(owners) => owners,
                Err(_) => {
                    runtime.table.restore(handle);
                    return Err(status(CODE_CAPACITY));
                }
            };
            runtime.next_owner = next;
            let committed = match runtime
                .host
                .execute_owned_success(&owners, &session.payloads)
            {
                Ok(value) => value,
                Err(_) => {
                    runtime.poisoned = true;
                    runtime.table.quarantine(handle);
                    return Err(status(CODE_UNCERTAIN));
                }
            };
            let allocations = crate::postcommit_allocation_probe::take_last();
            let proof = u64::from(committed.committed.receipt.iter().any(|b| *b != 0))
                | (u64::from(committed.candidate_bytes.iter().any(|b| *b != 0)) << 1)
                | (u64::from(
                    committed
                        .identity
                        .call
                        .call_contract
                        .iter()
                        .any(|b| *b != 0)
                        && committed
                            .identity
                            .call
                            .provider_challenge
                            .iter()
                            .any(|b| *b != 0)
                        && committed.identity.recovery_contract.iter().any(|b| *b != 0)
                        && committed.identity.settlement_graph.iter().any(|b| *b != 0),
                ) << 2)
                | (u64::from(
                    committed.committed.ledger_before.iter().any(|b| *b != 0)
                        && committed.committed.ledger_after.iter().any(|b| *b != 0)
                        && committed.committed.ledger_before != committed.committed.ledger_after,
                ) << 3);
            let mut count = 0;
            let mut ordinals = [0; 2];
            let mut payloads = [0; 2];
            let trace = unsafe {
                (runtime.snapshot)(&mut count, ordinals.as_mut_ptr(), payloads.as_mut_ptr(), 2)
            };
            if committed.outcome != (ExecuteOutcome::Scalar { value: 0 })
                || committed.committed.publication != Publication::NoOwned
                || committed.committed.published_owner.is_some()
                || proof != PROOF_FLAGS
                || allocations != Some(0)
                || trace != 0
                || count != 2
                || ordinals != [1, 0]
                || payloads != [13, 11]
                || runtime.host.is_poisoned()
                || runtime.host.is_draining()
                || runtime.host.quarantined_count() != 0
            {
                runtime.poisoned = true;
                runtime.table.quarantine(handle);
                return Err(status(CODE_EVIDENCE));
            }
            runtime.table.consume(handle);
            Ok(PrivateAppleSwiftEvidenceV1 {
                words: [
                    1,
                    runtime.host.module_instance_id().get(),
                    proof,
                    0,
                    2,
                    (1_u64 << 32) | 13,
                    11,
                    0,
                ],
            })
        }) {
            Ok(evidence) => {
                unsafe { ptr::write(output, evidence) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_adopt_single(
    payload: u64,
    output: *mut u64,
) -> u64 {
    guard(|| {
        if output.is_null() || (output as usize) % mem::align_of::<u64>() != 0 {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.adopt_single_witness(payload)) {
            Ok(handle) => {
                unsafe { output.write(handle) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_execute_requires_false(
    handle: u64,
    output: *mut PrivateAppleSwiftEvidenceV1,
    output_len: u32,
) -> u64 {
    guard(|| {
        if output.is_null()
            || output_len as usize != mem::size_of::<PrivateAppleSwiftEvidenceV1>()
            || (output as usize) % mem::align_of::<PrivateAppleSwiftEvidenceV1>() != 0
        {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.requires_false_witness(handle)) {
            Ok(evidence) => {
                // SAFETY: Exact aligned output storage was checked above and is
                // written only after authenticated receipt commit.
                unsafe { ptr::write(output, evidence) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_adopt_owned(
    payload: u64,
    output: *mut u64,
) -> u64 {
    guard(|| {
        if output.is_null() || (output as usize) % mem::align_of::<u64>() != 0 {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.adopt_owned_result(payload)) {
            Ok(handle) => {
                unsafe { output.write(handle) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_execute_identity_max(
    handle: u64,
    output: *mut PrivateAppleSwiftEvidenceV1,
    output_len: u32,
) -> u64 {
    guard(|| {
        if output.is_null()
            || output_len as usize != mem::size_of::<PrivateAppleSwiftEvidenceV1>()
            || (output as usize) % mem::align_of::<PrivateAppleSwiftEvidenceV1>() != 0
        {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.identity_max_witness(handle)) {
            Ok(evidence) => {
                // SAFETY: Exact aligned output storage was checked above and is
                // written only after authenticated receipt commit.
                unsafe { ptr::write(output, evidence) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_adopt_checked_add_overflow(
    payload: u64,
    output: *mut u64,
) -> u64 {
    guard(|| {
        if output.is_null() || (output as usize) % mem::align_of::<u64>() != 0 {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.adopt_checked_add_overflow(payload)) {
            Ok(handle) => {
                unsafe { output.write(handle) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_adopt_ensures_false(
    payload: u64,
    output: *mut u64,
) -> u64 {
    guard(|| {
        if output.is_null() || (output as usize) % mem::align_of::<u64>() != 0 {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.adopt_ensures_false(payload)) {
            Ok(handle) => {
                unsafe { output.write(handle) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_execute_checked_add_overflow(
    handle: u64,
    output: *mut PrivateAppleSwiftEvidenceV1,
    output_len: u32,
) -> u64 {
    guard(|| {
        if output.is_null()
            || output_len as usize != mem::size_of::<PrivateAppleSwiftEvidenceV1>()
            || (output as usize) % mem::align_of::<PrivateAppleSwiftEvidenceV1>() != 0
        {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.checked_add_overflow_witness(handle)) {
            Ok(evidence) => {
                // SAFETY: Exact aligned output storage was checked above and is
                // written only after authenticated receipt commit.
                unsafe { ptr::write(output, evidence) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_apple_swift_v1_execute_ensures_false(
    handle: u64,
    output: *mut PrivateAppleSwiftEvidenceV1,
    output_len: u32,
) -> u64 {
    guard(|| {
        if output.is_null()
            || output_len as usize != mem::size_of::<PrivateAppleSwiftEvidenceV1>()
            || (output as usize) % mem::align_of::<PrivateAppleSwiftEvidenceV1>() != 0
        {
            return status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime(|runtime| runtime.ensures_false_witness(handle)) {
            Ok(evidence) => {
                // SAFETY: Exact aligned output storage was checked above and is
                // written only after authenticated receipt commit.
                unsafe { ptr::write(output, evidence) };
                0
            }
            Err(error) => error,
        }
    })
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn spx_private_apple_swift_v1_close_runtime() -> u64 {
    guard(|| {
        RUNTIME.with(|cell| {
            let Ok(mut cell) = cell.try_borrow_mut() else {
                return status(CODE_REENTRANT);
            };
            let Some(runtime) = cell.as_mut() else {
                return status(CODE_WRONG_THREAD);
            };
            if runtime.poisoned || runtime.table.quarantined() || runtime.host.is_poisoned() {
                return status(CODE_UNHEALTHY);
            }
            if !runtime.table.empty() {
                return status(CODE_LIVE);
            }
            runtime.table.draining = true;
            drop(cell.take());
            0
        })
    })
}

#[cfg(test)]
#[path = "ios_swift_harness/tests.rs"]
mod tests;
