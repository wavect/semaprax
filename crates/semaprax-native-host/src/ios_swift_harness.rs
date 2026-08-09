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
use crate::settlement_host_v3::PrivateSettlementHostV3;

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

#[derive(Clone, Copy)]
struct Session([u64; 2]);

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
            (v <= TAG_MASK as u32).then_some(v + 1)
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
                .insert(Session([first, second]))
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
            let committed = match runtime.host.execute_owned_success(&owners, &session.0) {
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
mod tests {
    use super::*;
    #[test]
    fn kats_are_frozen() {
        assert_eq!(encode(1, 1, 1), HANDLE_KAT);
        assert_eq!(decode(HANDLE_KAT), Ok((1, 1, 1)));
        assert_eq!(status(1), 0x0000_002d_0000_0001);
        assert_eq!(mem::size_of::<PrivateAppleSwiftEvidenceV1>(), 64);
    }
    #[test]
    fn reserved_handles_reject() {
        for handle in [0, 1, 1 << 24, 1 << 48, 1 << 63] {
            assert_eq!(decode(handle), Err(TableError::Invalid));
        }
    }
    #[test]
    fn stale_cross_capacity_and_generation_are_closed() {
        let mut first = Table::new(1);
        let h = first.insert(7).unwrap();
        assert_eq!(first.claim(h), Ok(7));
        first.consume(h);
        assert_eq!(first.claim(h), Err(TableError::Stale));
        let fresh = first.insert(7).unwrap();
        assert_ne!(h, fresh);
        let mut second: Table<u64> = Table::new(2);
        assert_eq!(second.claim(fresh), Err(TableError::Cross));
        for _ in 1..CAPACITY {
            first.insert(9).unwrap();
        }
        assert_eq!(first.insert(9), Err(TableError::Capacity));
    }
    #[test]
    fn quarantine_and_close_inventory_are_absorbing() {
        let mut table = Table::new(1);
        let h = table.insert(1).unwrap();
        table.claim(h).unwrap();
        table.quarantine(h);
        assert!(table.quarantined());
        assert!(!table.empty());
        assert_eq!(table.insert(2), Err(TableError::Draining));
    }

    #[test]
    fn equal_payloads_keep_distinct_identity_and_restore_is_exact() {
        let mut table = Table::new(1);
        let first = table.insert(7).unwrap();
        let second = table.insert(7).unwrap();
        assert_ne!(first, second);
        assert_eq!(table.claim(first), Ok(7));
        table.restore(first);
        assert_eq!(table.claim(first), Ok(7));
    }

    #[test]
    fn generation_exhaustion_retires_without_wraparound() {
        let mut table: Table<u64> = Table::new(1);
        table.slots[0].generation = MASK as u32;
        table.slots[0].state = State::Consumed;
        let handle = table.insert(5).unwrap();
        assert_eq!(decode(handle).unwrap().2, 2);
        assert!(matches!(table.slots[0].state, State::Retired));
    }

    #[test]
    fn evidence_and_terminal_status_kats_are_exact() {
        let evidence = PrivateAppleSwiftEvidenceV1 {
            words: [1, 7, 15, 0, 2, (1_u64 << 32) | 13, 11, 0],
        };
        assert_eq!(
            evidence.words,
            [1, 7, 15, 0, 2, 0x0000_0001_0000_000d, 11, 0]
        );
        assert_eq!(status(CODE_EVIDENCE), 0x0000_002d_8000_0004);
        assert_eq!(status(CODE_PANIC), 0x0000_002d_8000_0002);
    }
}
