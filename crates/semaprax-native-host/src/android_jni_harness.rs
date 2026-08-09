//! Private Android JNI ownership adapter core.
//!
//! JNI itself remains in a generated C shim. This module exposes only a fixed
//! primitive C ABI and keeps the exact callable-v3 host in thread-local
//! storage, so neither the loader lease nor any owner becomes `Send`/`Sync`.

#![allow(
    dead_code,
    reason = "the private Android JNI entry points are linked only by the APK gate"
)]

use std::cell::RefCell;
#[cfg(target_os = "android")]
use std::ffi::c_void;
use std::mem;
use std::path::{Path, PathBuf};
#[cfg(target_os = "android")]
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use semaprax_native_loader::open_admitted_settlement_exact;
#[cfg(target_os = "android")]
use semaprax_native_loader::MAX_DESCRIPTOR_BYTES;

use crate::callable_wire_v3::{ExecuteOutcome, Publication};
use crate::descriptor_v3::Descriptor;
use crate::settlement_host_v3::PrivateSettlementHostV3;

const HANDLE_SLOT_BITS: u32 = 24;
const HANDLE_GENERATION_BITS: u32 = 24;
const HANDLE_TAG_BITS: u32 = 15;
const HANDLE_FIELD_MASK: u64 = (1_u64 << HANDLE_SLOT_BITS) - 1;
const HANDLE_TAG_MASK: u64 = (1_u64 << HANDLE_TAG_BITS) - 1;
const HANDLE_GENERATION_SHIFT: u32 = HANDLE_SLOT_BITS;
const HANDLE_TAG_SHIFT: u32 = HANDLE_SLOT_BITS + HANDLE_GENERATION_BITS;
const HANDLE_KNOWN_ANSWER: u64 = 0x0001_0000_0100_0001;
const SESSION_CAPACITY: usize = 8;
const MAX_PATH_BYTES: usize = 4_096;
const EVIDENCE_VERSION: u32 = 1;
const COMPLETE_PROOF_FLAGS: u64 = 0x0f;

const DOMAIN_ANDROID: u16 = 1;
const CLASS_ADAPTER: u8 = 5;
const RETRY_FALSE: u8 = 1;

const CODE_INVALID_ARGUMENT: u32 = 1;
const CODE_WRONG_THREAD: u32 = 2;
const CODE_ALREADY_OPEN: u32 = 3;
const CODE_NOT_OPEN: u32 = 4;
const CODE_PROVIDER_ADMISSION: u32 = 5;
const CODE_CAPACITY: u32 = 6;
const CODE_INVALID_HANDLE: u32 = 7;
const CODE_STALE_HANDLE: u32 = 8;
const CODE_CROSS_RUNTIME: u32 = 9;
const CODE_DRAINING: u32 = 10;
const CODE_REENTRANT: u32 = 11;
const CODE_WRONG_PAYLOAD: u32 = 12;
const CODE_LIVE_SESSIONS: u32 = 13;
const CODE_EXECUTION_UNCERTAIN: u32 = 0x8000_0001;
const CODE_NATIVE_PANIC: u32 = 0x8000_0002;
const CODE_HOST_UNHEALTHY: u32 = 0x8000_0003;
const CODE_EVIDENCE_MISMATCH: u32 = 0x8000_0004;

const fn status_word(domain: u16, class: u8, retryability: u8, code: u32) -> u64 {
    (code as u64) | ((class as u64) << 32) | ((retryability as u64) << 35) | ((domain as u64) << 37)
}

const fn android_status(code: u32) -> u64 {
    status_word(DOMAIN_ANDROID, CLASS_ADAPTER, RETRY_FALSE, code)
}

const _: () = {
    assert!(HANDLE_KNOWN_ANSWER == ((1_u64 << 48) | (1_u64 << 24) | 1));
    assert!(android_status(1) == 0x0000_002d_0000_0001);
};

static NEXT_RUNTIME_TAG: AtomicU32 = AtomicU32::new(1);

thread_local! {
    static RUNTIME: RefCell<Option<AndroidJniRuntimeV1>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Session {
    payloads: [u64; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SlotState<T> {
    Vacant,
    Live(T),
    Claimed(T),
    Consumed,
    Quarantined,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Slot<T> {
    generation: u32,
    state: SlotState<T>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TableError {
    Capacity,
    Invalid,
    Stale,
    CrossRuntime,
    Draining,
}

struct SessionTable<T> {
    runtime_tag: u16,
    draining: bool,
    slots: Vec<Slot<T>>,
}

impl<T: Copy> SessionTable<T> {
    fn new(runtime_tag: u16, capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        slots.resize(
            capacity,
            Slot {
                generation: 1,
                state: SlotState::Vacant,
            },
        );
        Self {
            runtime_tag,
            draining: false,
            slots,
        }
    }

    fn insert(&mut self, value: T) -> Result<u64, TableError> {
        if self.draining {
            return Err(TableError::Draining);
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            match slot.state {
                SlotState::Vacant => {
                    slot.state = SlotState::Live(value);
                    return Ok(encode_handle(
                        self.runtime_tag,
                        slot.generation,
                        u32::try_from(index + 1).map_err(|_| TableError::Capacity)?,
                    ));
                }
                SlotState::Consumed => {
                    if slot.generation == HANDLE_FIELD_MASK as u32 {
                        slot.state = SlotState::Retired;
                        continue;
                    }
                    slot.generation += 1;
                    slot.state = SlotState::Live(value);
                    return Ok(encode_handle(
                        self.runtime_tag,
                        slot.generation,
                        u32::try_from(index + 1).map_err(|_| TableError::Capacity)?,
                    ));
                }
                SlotState::Live(_)
                | SlotState::Claimed(_)
                | SlotState::Quarantined
                | SlotState::Retired => {}
            }
        }
        Err(TableError::Capacity)
    }

    fn claim(&mut self, handle: u64) -> Result<T, TableError> {
        let (tag, generation, slot_number) = decode_handle(handle)?;
        if tag != self.runtime_tag {
            return Err(TableError::CrossRuntime);
        }
        if self.draining {
            return Err(TableError::Draining);
        }
        let slot = self
            .slots
            .get_mut(slot_number as usize - 1)
            .ok_or(TableError::Invalid)?;
        if slot.generation != generation {
            return Err(TableError::Stale);
        }
        match slot.state {
            SlotState::Live(value) => {
                slot.state = SlotState::Claimed(value);
                Ok(value)
            }
            SlotState::Consumed | SlotState::Retired => Err(TableError::Stale),
            SlotState::Vacant | SlotState::Claimed(_) | SlotState::Quarantined => {
                Err(TableError::Invalid)
            }
        }
    }

    fn restore(&mut self, handle: u64) {
        if let Ok(slot) = self.exact_slot_mut(handle) {
            if let SlotState::Claimed(value) = slot.state {
                slot.state = SlotState::Live(value);
            }
        }
    }

    fn consume(&mut self, handle: u64) {
        if let Ok(slot) = self.exact_slot_mut(handle) {
            if matches!(slot.state, SlotState::Claimed(_)) {
                slot.state = SlotState::Consumed;
            }
        }
    }

    fn quarantine(&mut self, handle: u64) {
        if let Ok(slot) = self.exact_slot_mut(handle) {
            slot.state = SlotState::Quarantined;
            self.draining = true;
        }
    }

    fn quarantine_all_claimed(&mut self) {
        for slot in &mut self.slots {
            if matches!(slot.state, SlotState::Claimed(_)) {
                slot.state = SlotState::Quarantined;
            }
        }
        self.draining = true;
    }

    fn quarantine_all_active(&mut self) {
        for slot in &mut self.slots {
            if matches!(slot.state, SlotState::Live(_) | SlotState::Claimed(_)) {
                slot.state = SlotState::Quarantined;
            }
        }
        self.draining = true;
    }

    fn begin_draining(&mut self) {
        self.draining = true;
    }

    fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| {
            matches!(
                slot.state,
                SlotState::Vacant | SlotState::Consumed | SlotState::Retired
            )
        })
    }

    fn has_quarantine(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| matches!(slot.state, SlotState::Quarantined))
    }

    fn exact_slot_mut(&mut self, handle: u64) -> Result<&mut Slot<T>, TableError> {
        let (tag, generation, slot_number) = decode_handle(handle)?;
        if tag != self.runtime_tag {
            return Err(TableError::CrossRuntime);
        }
        let slot = self
            .slots
            .get_mut(slot_number as usize - 1)
            .ok_or(TableError::Invalid)?;
        if slot.generation != generation {
            return Err(TableError::Stale);
        }
        Ok(slot)
    }
}

const fn encode_handle(runtime_tag: u16, generation: u32, slot: u32) -> u64 {
    ((runtime_tag as u64) << HANDLE_TAG_SHIFT)
        | ((generation as u64) << HANDLE_GENERATION_SHIFT)
        | slot as u64
}

fn decode_handle(handle: u64) -> Result<(u16, u32, u32), TableError> {
    if handle == 0 || handle >> 63 != 0 {
        return Err(TableError::Invalid);
    }
    let slot = (handle & HANDLE_FIELD_MASK) as u32;
    let generation = ((handle >> HANDLE_GENERATION_SHIFT) & HANDLE_FIELD_MASK) as u32;
    let tag = ((handle >> HANDLE_TAG_SHIFT) & HANDLE_TAG_MASK) as u16;
    if tag == 0 || generation == 0 || slot == 0 {
        return Err(TableError::Invalid);
    }
    Ok((tag, generation, slot))
}

struct AndroidJniRuntimeV1 {
    host: PrivateSettlementHostV3,
    sessions: SessionTable<Session>,
    next_owner_slot: u64,
    poisoned: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PrivateAndroidJniEvidenceV1 {
    size: u32,
    version: u32,
    module_instance_id: u64,
    proof_flags: u64,
    postcommit_allocations: u64,
    host_state_flags: u64,
}

const _: () = {
    assert!(mem::size_of::<PrivateAndroidJniEvidenceV1>() == 40);
    assert!(mem::offset_of!(PrivateAndroidJniEvidenceV1, module_instance_id) == 8);
    assert!(mem::offset_of!(PrivateAndroidJniEvidenceV1, proof_flags) == 16);
    assert!(mem::offset_of!(PrivateAndroidJniEvidenceV1, postcommit_allocations) == 24);
    assert!(mem::offset_of!(PrivateAndroidJniEvidenceV1, host_state_flags) == 32);
};

impl AndroidJniRuntimeV1 {
    fn open(provider_path: PathBuf, descriptor_bytes: &[u8]) -> Result<Self, u64> {
        if Descriptor::parse(descriptor_bytes).is_err() {
            return Err(android_status(CODE_PROVIDER_ADMISSION));
        }
        // SAFETY: This is an unpublished fixture. The installed APK packages
        // the exact generated provider and passes its canonical extracted path.
        let lease = unsafe { open_admitted_settlement_exact(&provider_path, descriptor_bytes) }
            .map_err(|_| android_status(CODE_PROVIDER_ADMISSION))?;
        let host = PrivateSettlementHostV3::from_admitted(lease, descriptor_bytes)
            .map_err(|_| android_status(CODE_PROVIDER_ADMISSION))?;
        let tag = NEXT_RUNTIME_TAG
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current <= HANDLE_TAG_MASK as u32).then_some(current + 1)
            })
            .map_err(|_| android_status(CODE_CAPACITY))? as u16;
        Ok(Self {
            host,
            sessions: SessionTable::new(tag, SESSION_CAPACITY),
            next_owner_slot: 1,
            poisoned: false,
        })
    }

    fn adopt_pair(&mut self, payloads: [u64; 2]) -> Result<u64, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(android_status(CODE_HOST_UNHEALTHY));
        }
        self.sessions
            .insert(Session { payloads })
            .map_err(map_table_error)
    }

    fn consume_pair(&mut self, handle: u64) -> Result<PrivateAndroidJniEvidenceV1, u64> {
        if self.poisoned || self.host.is_poisoned() {
            return Err(android_status(CODE_HOST_UNHEALTHY));
        }
        let session = self.sessions.claim(handle).map_err(map_table_error)?;
        let [first_slot, second_slot] = match take_owner_pair(&mut self.next_owner_slot) {
            Ok(slots) => slots,
            Err(status) => {
                self.sessions.restore(handle);
                return Err(status);
            }
        };
        if first_slot == 0 || second_slot == 0 {
            self.sessions.restore(handle);
            return Err(android_status(CODE_CAPACITY));
        }
        let owners = match self
            .host
            .register_owner_pair([(first_slot, 1), (second_slot, 1)])
        {
            Ok(owners) => owners,
            Err(_) => {
                self.sessions.restore(handle);
                return Err(android_status(CODE_CAPACITY));
            }
        };
        let committed = match self.host.execute_owned_success(&owners, &session.payloads) {
            Ok(committed) => committed,
            Err(_) => {
                self.sessions.quarantine(handle);
                self.poisoned = true;
                return Err(android_status(CODE_EXECUTION_UNCERTAIN));
            }
        };
        let proof_flags = u64::from(committed.committed.receipt.iter().any(|byte| *byte != 0))
            | (u64::from(committed.candidate_bytes.iter().any(|byte| *byte != 0)) << 1)
            | (u64::from(
                committed
                    .identity
                    .call
                    .call_contract
                    .iter()
                    .any(|byte| *byte != 0)
                    && committed
                        .identity
                        .call
                        .provider_challenge
                        .iter()
                        .any(|byte| *byte != 0)
                    && committed
                        .identity
                        .recovery_contract
                        .iter()
                        .any(|byte| *byte != 0)
                    && committed
                        .identity
                        .settlement_graph
                        .iter()
                        .any(|byte| *byte != 0),
            ) << 2)
            | (u64::from(
                committed
                    .committed
                    .ledger_before
                    .iter()
                    .any(|byte| *byte != 0)
                    && committed
                        .committed
                        .ledger_after
                        .iter()
                        .any(|byte| *byte != 0)
                    && committed.committed.ledger_before != committed.committed.ledger_after,
            ) << 3);
        let allocations = crate::postcommit_allocation_probe::take_last();
        if committed.outcome != (ExecuteOutcome::Scalar { value: 0 })
            || committed.committed.publication != Publication::NoOwned
            || committed.committed.published_owner.is_some()
            || proof_flags != COMPLETE_PROOF_FLAGS
            || allocations != Some(0)
            || self.host.is_poisoned()
            || self.host.is_draining()
            || self.host.quarantined_count() != 0
        {
            self.sessions.quarantine(handle);
            self.poisoned = true;
            return Err(android_status(CODE_EVIDENCE_MISMATCH));
        }
        self.sessions.consume(handle);
        Ok(PrivateAndroidJniEvidenceV1 {
            size: mem::size_of::<PrivateAndroidJniEvidenceV1>() as u32,
            version: EVIDENCE_VERSION,
            module_instance_id: self.host.module_instance_id().get(),
            proof_flags,
            postcommit_allocations: allocations.unwrap_or(usize::MAX) as u64,
            host_state_flags: 0,
        })
    }

    fn can_close(&mut self) -> Result<(), u64> {
        if self.sessions.has_quarantine() || self.poisoned || self.host.is_poisoned() {
            return Err(android_status(CODE_HOST_UNHEALTHY));
        }
        if !self.sessions.is_empty() {
            return Err(android_status(CODE_LIVE_SESSIONS));
        }
        self.sessions.begin_draining();
        Ok(())
    }
}

fn take_owner_pair(next_owner_slot: &mut u64) -> Result<[u64; 2], u64> {
    let first = *next_owner_slot;
    let second = first
        .checked_add(1)
        .ok_or_else(|| android_status(CODE_CAPACITY))?;
    let next = second
        .checked_add(1)
        .ok_or_else(|| android_status(CODE_CAPACITY))?;
    *next_owner_slot = next;
    Ok([first, second])
}

fn map_table_error(error: TableError) -> u64 {
    android_status(match error {
        TableError::Capacity => CODE_CAPACITY,
        TableError::Invalid => CODE_INVALID_HANDLE,
        TableError::Stale => CODE_STALE_HANDLE,
        TableError::CrossRuntime => CODE_CROSS_RUNTIME,
        TableError::Draining => CODE_DRAINING,
    })
}

fn with_runtime_mut<T>(
    f: impl FnOnce(&mut AndroidJniRuntimeV1) -> Result<T, u64>,
) -> Result<T, u64> {
    RUNTIME.with(|runtime| {
        let mut runtime = runtime
            .try_borrow_mut()
            .map_err(|_| android_status(CODE_REENTRANT))?;
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| android_status(CODE_WRONG_THREAD))?;
        f(runtime)
    })
}

fn poison_after_panic() {
    RUNTIME.with(|runtime| {
        if let Ok(mut runtime) = runtime.try_borrow_mut() {
            if let Some(runtime) = runtime.as_mut() {
                runtime.poisoned = true;
                runtime.sessions.quarantine_all_claimed();
            }
        }
    });
}

fn ffi_guard(f: impl FnOnce() -> u64) -> u64 {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => {
            poison_after_panic();
            android_status(CODE_NATIVE_PANIC)
        }
    }
}

unsafe fn read_bytes<'a>(pointer: *const u8, length: u32, maximum: usize) -> Result<&'a [u8], u64> {
    let length = length as usize;
    if pointer.is_null() || length == 0 || length > maximum {
        return Err(android_status(CODE_INVALID_ARGUMENT));
    }
    // SAFETY: The generated JNI shim establishes the exact readable range for
    // the duration of this synchronous call.
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}

fn exact_canonical_provider_path(provider_path: &[u8]) -> Result<PathBuf, u64> {
    if provider_path.contains(&0) {
        return Err(android_status(CODE_INVALID_ARGUMENT));
    }
    let provider_path_text =
        std::str::from_utf8(provider_path).map_err(|_| android_status(CODE_INVALID_ARGUMENT))?;
    let supplied = Path::new(provider_path_text);
    if !supplied.is_absolute() {
        return Err(android_status(CODE_INVALID_ARGUMENT));
    }
    let canonical =
        std::fs::canonicalize(supplied).map_err(|_| android_status(CODE_PROVIDER_ADMISSION))?;
    if canonical
        .to_str()
        .is_none_or(|path| path.as_bytes() != provider_path)
    {
        return Err(android_status(CODE_PROVIDER_ADMISSION));
    }
    Ok(canonical)
}

#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_android_jni_v1_open(
    provider_path: *const u8,
    provider_path_len: u32,
    descriptor: *const u8,
    descriptor_len: u32,
) -> u64 {
    ffi_guard(|| {
        let provider_path =
            match unsafe { read_bytes(provider_path, provider_path_len, MAX_PATH_BYTES) } {
                Ok(path) => path,
                Err(status) => return status,
            };
        let descriptor =
            match unsafe { read_bytes(descriptor, descriptor_len, MAX_DESCRIPTOR_BYTES) } {
                Ok(descriptor) => descriptor,
                Err(status) => return status,
            };
        if Descriptor::parse(descriptor).is_err() {
            return android_status(CODE_PROVIDER_ADMISSION);
        }
        let provider_path = match exact_canonical_provider_path(provider_path) {
            Ok(provider_path) => provider_path,
            Err(status) => return status,
        };
        RUNTIME.with(|runtime| {
            let Ok(mut runtime) = runtime.try_borrow_mut() else {
                return android_status(CODE_REENTRANT);
            };
            if runtime.is_some() {
                return android_status(CODE_ALREADY_OPEN);
            }
            match AndroidJniRuntimeV1::open(provider_path, descriptor) {
                Ok(opened) => {
                    *runtime = Some(opened);
                    0
                }
                Err(status) => status,
            }
        })
    })
}

#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_android_jni_v1_adopt_pair(
    first_payload: u64,
    second_payload: u64,
    out_handle: *mut u64,
) -> u64 {
    ffi_guard(|| {
        if out_handle.is_null() || (out_handle as usize) % mem::align_of::<u64>() != 0 {
            return android_status(CODE_INVALID_ARGUMENT);
        }
        if first_payload != 11 || second_payload != 13 {
            return android_status(CODE_WRONG_PAYLOAD);
        }
        match with_runtime_mut(|runtime| runtime.adopt_pair([first_payload, second_payload])) {
            Ok(handle) => {
                // SAFETY: The generated shim supplies aligned writable storage.
                unsafe { out_handle.write(handle) };
                0
            }
            Err(status) => status,
        }
    })
}

#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn spx_private_android_jni_v1_consume_pair(
    handle: u64,
    out_evidence: *mut PrivateAndroidJniEvidenceV1,
    out_evidence_len: u32,
) -> u64 {
    ffi_guard(|| {
        if out_evidence.is_null()
            || out_evidence_len as usize != mem::size_of::<PrivateAndroidJniEvidenceV1>()
            || (out_evidence as usize) % mem::align_of::<PrivateAndroidJniEvidenceV1>() != 0
        {
            return android_status(CODE_INVALID_ARGUMENT);
        }
        match with_runtime_mut(|runtime| runtime.consume_pair(handle)) {
            Ok(evidence) => {
                // SAFETY: Exact aligned output storage was checked above and is
                // written only after authenticated receipt commit.
                unsafe { ptr::write(out_evidence, evidence) };
                0
            }
            Err(status) => status,
        }
    })
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn spx_private_android_jni_v1_poison_runtime() -> u64 {
    ffi_guard(|| {
        match with_runtime_mut(|runtime| {
            runtime.poisoned = true;
            runtime.sessions.quarantine_all_active();
            Ok(())
        }) {
            Ok(()) => 0,
            Err(status) => status,
        }
    })
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn spx_private_android_jni_v1_validate_hooks(
    first: *const c_void,
    second: *const c_void,
) -> u64 {
    ffi_guard(|| {
        match with_runtime_mut(|runtime| {
            if runtime
                .host
                .private_addresses_share_admitted_root(first, second)
            {
                Ok(())
            } else {
                Err(android_status(CODE_EVIDENCE_MISMATCH))
            }
        }) {
            Ok(()) => 0,
            Err(status) => status,
        }
    })
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn spx_private_android_jni_v1_close_runtime() -> u64 {
    ffi_guard(|| {
        RUNTIME.with(|runtime| {
            let Ok(mut runtime) = runtime.try_borrow_mut() else {
                return android_status(CODE_REENTRANT);
            };
            let Some(opened) = runtime.as_mut() else {
                return android_status(CODE_NOT_OPEN);
            };
            if let Err(status) = opened.can_close() {
                return status;
            }
            let opened = runtime.take().expect("checked runtime remains present");
            drop(opened);
            0
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(tag: u16) -> SessionTable<u64> {
        SessionTable::new(tag, 2)
    }

    #[test]
    fn handle_and_status_known_answers_are_exact() {
        assert_eq!(encode_handle(1, 1, 1), HANDLE_KNOWN_ANSWER);
        assert_eq!(decode_handle(HANDLE_KNOWN_ANSWER), Ok((1, 1, 1)));
        assert_eq!(android_status(1), 0x0000_002d_0000_0001);
    }

    #[test]
    fn provider_path_must_arrive_absolute_and_byte_exact_canonical() {
        assert_eq!(
            exact_canonical_provider_path(b"relative/provider.so"),
            Err(android_status(CODE_INVALID_ARGUMENT))
        );
        let canonical = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
        let canonical_text = canonical.to_str().unwrap();
        assert_eq!(
            exact_canonical_provider_path(canonical_text.as_bytes()),
            Ok(canonical.clone())
        );
        let redundant = format!("{canonical_text}/.");
        assert_eq!(
            exact_canonical_provider_path(redundant.as_bytes()),
            Err(android_status(CODE_PROVIDER_ADMISSION))
        );
    }

    #[test]
    fn reserved_handle_shapes_fail_closed() {
        for invalid in [
            0,
            1,
            1_u64 << HANDLE_GENERATION_SHIFT,
            1_u64 << HANDLE_TAG_SHIFT,
            1_u64 << 63,
        ] {
            assert_eq!(decode_handle(invalid), Err(TableError::Invalid));
        }
    }

    #[test]
    fn claim_restore_consume_stale_and_cross_runtime_are_exact() {
        let mut first = table(1);
        let handle = first.insert(7).unwrap();
        assert_eq!(first.claim(handle), Ok(7));
        assert_eq!(first.claim(handle), Err(TableError::Invalid));
        first.restore(handle);
        assert_eq!(first.claim(handle), Ok(7));
        first.consume(handle);
        assert_eq!(first.claim(handle), Err(TableError::Stale));
        let refreshed = first.insert(7).unwrap();
        assert_ne!(refreshed, handle);
        assert_eq!(first.claim(handle), Err(TableError::Stale));
        first.restore(refreshed);

        let mut second = table(2);
        let _ = second.insert(7).unwrap();
        assert_eq!(second.claim(refreshed), Err(TableError::CrossRuntime));
    }

    #[test]
    fn equal_payloads_have_distinct_identity_and_capacity_is_bounded() {
        let mut table = table(1);
        let first = table.insert(9).unwrap();
        let second = table.insert(9).unwrap();
        assert_ne!(first, second);
        assert_eq!(table.insert(9), Err(TableError::Capacity));
    }

    #[test]
    fn generation_exhaustion_retires_without_wraparound() {
        let mut table = table(1);
        table.slots[0].generation = HANDLE_FIELD_MASK as u32;
        table.slots[0].state = SlotState::Consumed;
        let handle = table.insert(5).unwrap();
        assert_eq!(decode_handle(handle).unwrap().2, 2);
        assert!(matches!(table.slots[0].state, SlotState::Retired));
    }

    #[test]
    fn quarantine_and_drain_are_absorbing() {
        let mut table = table(1);
        let handle = table.insert(5).unwrap();
        table.claim(handle).unwrap();
        table.quarantine(handle);
        assert!(table.has_quarantine());
        assert_eq!(table.insert(6), Err(TableError::Draining));
        assert_eq!(table.claim(handle), Err(TableError::Draining));
        assert!(!table.is_empty());
    }

    #[test]
    fn panic_poisoning_quarantines_every_claimed_session() {
        let mut table = SessionTable::new(1, 3);
        let first = table.insert(11_u64).unwrap();
        let second = table.insert(13_u64).unwrap();
        let live = table.insert(17_u64).unwrap();
        assert_eq!(table.claim(first), Ok(11));
        assert_eq!(table.claim(second), Ok(13));

        table.quarantine_all_claimed();

        assert!(table.has_quarantine());
        assert_eq!(table.claim(first), Err(TableError::Draining));
        assert_eq!(table.claim(second), Err(TableError::Draining));
        assert_eq!(table.claim(live), Err(TableError::Draining));
        assert_eq!(table.insert(19), Err(TableError::Draining));
        assert!(matches!(table.slots[0].state, SlotState::Quarantined));
        assert!(matches!(table.slots[1].state, SlotState::Quarantined));
        assert!(matches!(table.slots[2].state, SlotState::Live(17)));
    }

    #[test]
    fn boundary_poisoning_quarantines_live_and_claimed_sessions() {
        let mut table = SessionTable::new(1, 3);
        let first = table.insert(11_u64).unwrap();
        let _live = table.insert(13_u64).unwrap();
        assert_eq!(table.claim(first), Ok(11));

        table.quarantine_all_active();

        assert!(matches!(table.slots[0].state, SlotState::Quarantined));
        assert!(matches!(table.slots[1].state, SlotState::Quarantined));
        assert!(table.has_quarantine());
        assert!(table.draining);
    }

    #[test]
    fn exact_output_abi_remains_frozen() {
        assert_eq!(mem::size_of::<PrivateAndroidJniEvidenceV1>(), 40);
        assert_eq!(
            mem::offset_of!(PrivateAndroidJniEvidenceV1, module_instance_id),
            8
        );
        assert_eq!(
            mem::offset_of!(PrivateAndroidJniEvidenceV1, host_state_flags),
            32
        );
    }

    #[test]
    fn owner_pair_counter_exhaustion_is_nonmutating() {
        let mut next = u64::MAX - 1;
        assert_eq!(
            take_owner_pair(&mut next),
            Err(android_status(CODE_CAPACITY))
        );
        assert_eq!(next, u64::MAX - 1);
        let mut next = 7;
        assert_eq!(take_owner_pair(&mut next), Ok([7, 8]));
        assert_eq!(next, 9);
    }

    #[test]
    fn rejected_close_precheck_does_not_drain_live_table() {
        let mut table = table(1);
        let handle = table.insert(7).unwrap();
        assert!(!table.is_empty());
        assert!(!table.draining);
        assert_eq!(table.claim(handle), Ok(7));
        table.restore(handle);
        assert!(!table.draining);
    }
}
