//! Fixed-capacity host settlement ledger for private callable-v3 execution.
//!
//! Provider frames and receipts are evidence. The phase below is authoritative
//! host state and the only component allowed to perform `ReceiptCommit`.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "callable-v3 physical settlement remains private and unwired"
)]

use std::cell::RefCell;
use std::num::NonZeroU64;

use crate::callable_wire_v3::{
    candidate_digest, ledger_transition_digests, validate_candidate_replay_preencoded,
    ActionRecord, CallIdentity, CandidateOutcome, CandidateReceipt, DispositionCell,
    ExecuteRequest, ExecuteResponse, LedgerEntry, LedgerState, Publication, RecoveryFrame,
    RecoveryIdentity, SettlementDecision, WireError, HOST_RECEIPT_BYTES,
};
use crate::descriptor_v3::{Capacities, Descriptor};
use crate::receipt_authority::{ReceiptAuthority, ReceiptAuthorityError};

#[cfg(test)]
std::thread_local! {
    static RECEIPT_PREPARE_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn arm_receipt_prepare_panic() {
    RECEIPT_PREPARE_PANIC.with(|armed| armed.set(true));
}

#[cfg(test)]
fn receipt_prepare_panic_failpoint() {
    RECEIPT_PREPARE_PANIC.with(|armed| {
        if armed.replace(false) {
            panic!("injected receipt HMAC preparation panic");
        }
    });
}

/// Narrow leaf-pin contract implemented by the loader's exact v3 lease.
///
/// Retention is explicit and the trait intentionally requires ownership of a
/// concrete pin; no raw handle, function pointer, or close operation crosses
/// this boundary.
pub(crate) trait SettlementPin: Sized {
    fn retain(&self) -> Self;
    fn instance_nonce(&self) -> NonZeroU64;
    fn is_same_instance(&self, other: &Self) -> bool;
}

#[cfg(not(target_os = "ios"))]
impl SettlementPin for semaprax_native_loader::NativeSettlementModuleLease {
    fn retain(&self) -> Self {
        self.retain()
    }

    fn instance_nonce(&self) -> NonZeroU64 {
        NonZeroU64::new(self.instance_id().get())
            .expect("loader instance identities are structurally nonzero")
    }

    fn is_same_instance(&self, other: &Self) -> bool {
        self.is_same_instance(other)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettlementLedgerError {
    Draining,
    Poisoned,
    CapacityExhausted,
    CounterExhausted,
    WrongInstance,
    WrongIdentity,
    WrongPhase,
    NotStaged,
    Wire(WireError),
    Authority(ReceiptAuthorityError),
    ConflictingReplay,
    UncertainFinalizer,
    DuplicateOwner,
    StaleOwner,
    OwnerTableFull,
    DescriptorMismatch,
}

impl From<WireError> for SettlementLedgerError {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}

impl From<ReceiptAuthorityError> for SettlementLedgerError {
    fn from(value: ReceiptAuthorityError) -> Self {
        Self::Authority(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoritativePhase {
    Reserved,
    CallCommitted,
    DecisionCommitted,
    Finalizing,
    ProviderSettled,
    ReceiptCommitted,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoritativeOwnerState {
    Live,
    InInvocation(RecoveryIdentity),
    Retired,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthoritativeOwner {
    slot: u64,
    generation: u64,
    state: AuthoritativeOwnerState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettlementOwnerHandle {
    instance_nonce: NonZeroU64,
    slot: u64,
    generation: u64,
}

struct FrameStorage {
    request: Vec<u8>,
    execute_response: Vec<u8>,
    frame: Vec<u8>,
    decision: Vec<u8>,
    action: Vec<u8>,
    candidate: Vec<u8>,
    receipt: Vec<u8>,
    before_entries: Vec<LedgerEntry>,
    after_entries: Vec<LedgerEntry>,
    candidate_dispositions: Vec<DispositionCell>,
    resource_count: usize,
}

impl FrameStorage {
    fn try_new(capacities: Capacities) -> Result<Self, SettlementLedgerError> {
        let resource_count = usize::try_from(capacities.resource_count)
            .map_err(|_| SettlementLedgerError::CapacityExhausted)?;
        Ok(Self {
            request: fixed_zeroed(capacities.request)?,
            execute_response: fixed_zeroed(capacities.execute_response)?,
            frame: fixed_zeroed(capacities.frame)?,
            decision: fixed_zeroed(capacities.decision)?,
            action: fixed_zeroed(capacities.action_evidence)?,
            candidate: fixed_zeroed(capacities.candidate_receipt)?,
            receipt: vec![0; HOST_RECEIPT_BYTES],
            before_entries: Vec::with_capacity(resource_count),
            after_entries: Vec::with_capacity(resource_count),
            candidate_dispositions: Vec::with_capacity(resource_count),
            resource_count,
        })
    }

    fn clear_sensitive(&mut self) {
        self.request.fill(0);
        self.execute_response.fill(0);
        self.frame.fill(0);
        self.decision.fill(0);
        self.action.fill(0);
        self.candidate.fill(0);
        self.receipt.fill(0);
        self.before_entries.clear();
        self.after_entries.clear();
        self.candidate_dispositions.clear();
    }
}

fn fixed_zeroed(length: u32) -> Result<Vec<u8>, SettlementLedgerError> {
    let length = usize::try_from(length).map_err(|_| SettlementLedgerError::CapacityExhausted)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| SettlementLedgerError::CapacityExhausted)?;
    bytes.resize(length, 0);
    Ok(bytes)
}

/// Linear frame capability. The exact-image pin is the last field so all host
/// evidence and buffers are destroyed before native module terminators may run.
struct ReservedFrame<P: SettlementPin> {
    identity: RecoveryIdentity,
    phase: AuthoritativePhase,
    staged: bool,
    storage: FrameStorage,
    quarantine_index: usize,
    quarantine_replacement: Option<FrameStorage>,
    committed_index: usize,
    _pin: P,
}

impl<P: SettlementPin> ReservedFrame<P> {
    const fn identity(&self) -> RecoveryIdentity {
        self.identity
    }

    fn stage_call(
        &mut self,
        request: &ExecuteRequest,
        entries: &[LedgerEntry],
    ) -> Result<(), SettlementLedgerError> {
        if self.phase != AuthoritativePhase::Reserved || self.staged {
            return Err(SettlementLedgerError::WrongPhase);
        }
        if request.identity != self.identity.call || entries.len() != self.storage.resource_count {
            return Err(SettlementLedgerError::WrongIdentity);
        }
        for (index, entry) in entries.iter().enumerate() {
            if entry.owner_ordinal as usize != index
                || entry.slot == 0
                || entry.generation == 0
                || entry.state != LedgerState::InInvocation
            {
                return Err(SettlementLedgerError::WrongIdentity);
            }
        }
        let request_bytes = request.encode();
        if request_bytes.len() != self.storage.request.len() {
            return Err(SettlementLedgerError::CapacityExhausted);
        }
        self.storage.request.copy_from_slice(&request_bytes);
        self.storage.before_entries.extend_from_slice(entries);
        self.storage.after_entries.extend_from_slice(entries);
        self.staged = true;
        Ok(())
    }

    /// The first irreversible boundary. This method performs only scalar
    /// validation and state writes: every byte/vector/pin was prepared first.
    /// The second irreversible boundary, owned by the host rather than the
    /// provider's mutable frame bytes.
    fn decision_commit(
        &mut self,
        decision: &SettlementDecision,
    ) -> Result<(), SettlementLedgerError> {
        if self.phase != AuthoritativePhase::CallCommitted {
            return Err(SettlementLedgerError::WrongPhase);
        }
        if decision.identity != self.identity {
            return Err(SettlementLedgerError::WrongIdentity);
        }
        let bytes = decision.encode_fixed()?;
        if bytes.len() != self.storage.decision.len() {
            return Err(SettlementLedgerError::CapacityExhausted);
        }
        self.storage.decision.copy_from_slice(&bytes);
        self.phase = AuthoritativePhase::DecisionCommitted;
        Ok(())
    }

    /// Must be called before invoking a physical finalizer.
    fn finalizer_started(&mut self) -> Result<(), SettlementLedgerError> {
        if self.phase != AuthoritativePhase::DecisionCommitted {
            return Err(SettlementLedgerError::WrongPhase);
        }
        self.phase = AuthoritativePhase::Finalizing;
        Ok(())
    }

    /// A completed physical effect may become `Dead` only after normal return.
    fn finalizer_completed(&mut self) -> Result<(), SettlementLedgerError> {
        if self.phase != AuthoritativePhase::Finalizing {
            return Err(SettlementLedgerError::WrongPhase);
        }
        self.phase = AuthoritativePhase::DecisionCommitted;
        Ok(())
    }

    fn provider_settled(&mut self) -> Result<(), SettlementLedgerError> {
        if self.phase != AuthoritativePhase::DecisionCommitted {
            return Err(SettlementLedgerError::WrongPhase);
        }
        self.phase = AuthoritativePhase::ProviderSettled;
        Ok(())
    }

    fn request_bytes(&self) -> &[u8] {
        &self.storage.request
    }

    fn execute_response_storage_mut(&mut self) -> &mut [u8] {
        &mut self.storage.execute_response
    }

    fn frame_storage_mut(&mut self) -> &mut [u8] {
        &mut self.storage.frame
    }

    fn action_storage_mut(&mut self) -> &mut [u8] {
        &mut self.storage.action
    }

    fn candidate_storage_mut(&mut self) -> &mut [u8] {
        &mut self.storage.candidate
    }
}

pub(crate) struct ReceiptCommitEvidence<'a> {
    pub(crate) request: &'a ExecuteRequest,
    pub(crate) execute_return_code: u32,
    pub(crate) response_storage: ResponseStorageEvidence<'a>,
    pub(crate) response: Option<&'a ExecuteResponse>,
    pub(crate) frame: &'a RecoveryFrame,
    pub(crate) decision: &'a SettlementDecision,
    pub(crate) actions: &'a [ActionRecord],
    pub(crate) candidate: &'a CandidateReceipt,
}

pub(crate) enum ResponseStorageEvidence<'a> {
    External(&'a [u8]),
    Reserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommittedResult {
    pub(crate) receipt: [u8; HOST_RECEIPT_BYTES],
    pub(crate) publication: Publication,
    pub(crate) ledger_before: [u8; 32],
    pub(crate) ledger_after: [u8; 32],
    pub(crate) published_owner: Option<SettlementOwnerHandle>,
}

struct PreparedReceiptCommit {
    candidate_digest: [u8; 32],
    ledger_before: [u8; 32],
    ledger_after: [u8; 32],
    receipt: [u8; HOST_RECEIPT_BYTES],
    publication: Publication,
    cache_index: usize,
    published_owner: Option<u32>,
    refreshed_owner: Option<SettlementOwnerHandle>,
}

/// Cached authenticated result. The exact pin is deliberately last.
struct CommittedRecord<P: SettlementPin> {
    identity: RecoveryIdentity,
    candidate_digest: [u8; 32],
    result: CommittedResult,
    conflicted: bool,
    _pin: P,
}

/// Absorbing uncertain frame. There is no API that returns this pin or retries
/// its finalizer. The pin is last in `ReservedFrame`, preserving drop order.
struct QuarantinedFrame<P: SettlementPin> {
    frame: ReservedFrame<P>,
}

/// One exact-instance, fixed-capacity settlement ledger.
///
/// Field order encodes the one-way lifetime DAG: records/quarantine are dropped
/// before the receipt authority and root image pin.
struct LedgerCore<P: SettlementPin> {
    committed: Vec<Option<CommittedRecord<P>>>,
    quarantined: Vec<Option<QuarantinedFrame<P>>>,
    descriptor: Descriptor,
    next_invocation: u64,
    next_generation: u64,
    active_storage: Vec<FrameStorage>,
    quarantine_storage: Vec<FrameStorage>,
    available_quarantine: Vec<usize>,
    authoritative_owners: Vec<AuthoritativeOwner>,
    owner_capacity: usize,
    active_postcommit: usize,
    committed_reserved: Vec<bool>,
    poisoned: bool,
    draining: bool,
    authority: ReceiptAuthority,
    root_pin: P,
}

pub(crate) struct SettlementLedger<P: SettlementPin> {
    core: RefCell<LedgerCore<P>>,
}

impl<P: SettlementPin> SettlementLedger<P> {
    pub(crate) fn try_new(
        root_pin: P,
        descriptor: Descriptor,
        authority: ReceiptAuthority,
    ) -> Result<Self, SettlementLedgerError> {
        if authority.instance_nonce() != root_pin.instance_nonce() {
            return Err(SettlementLedgerError::WrongInstance);
        }
        let active = usize::try_from(descriptor.capacities.active_frames)
            .map_err(|_| SettlementLedgerError::CapacityExhausted)?;
        let quarantine = usize::try_from(descriptor.capacities.quarantined_frames)
            .map_err(|_| SettlementLedgerError::CapacityExhausted)?;
        // The descriptor reserves more active work buffers than quarantine
        // permits. This host intentionally caps simultaneously outstanding
        // transactions to the smaller quarantine bound, so every possible
        // postcommit frame owns an infallible absorption permit.
        let usable_active = active.min(quarantine);
        let owner_capacity = usable_active
            .checked_mul(descriptor.capacities.resource_count as usize)
            .ok_or(SettlementLedgerError::CapacityExhausted)?;
        let mut active_storage = Vec::new();
        active_storage
            .try_reserve_exact(active)
            .map_err(|_| SettlementLedgerError::CapacityExhausted)?;
        for _ in 0..active {
            active_storage.push(FrameStorage::try_new(descriptor.capacities)?);
        }
        let mut quarantine_storage = Vec::new();
        quarantine_storage
            .try_reserve_exact(quarantine)
            .map_err(|_| SettlementLedgerError::CapacityExhausted)?;
        for _ in 0..quarantine {
            quarantine_storage.push(FrameStorage::try_new(descriptor.capacities)?);
        }
        let mut available_quarantine = Vec::with_capacity(quarantine);
        available_quarantine.extend(0..quarantine);
        let authoritative_owners = Vec::with_capacity(owner_capacity);
        let committed = std::iter::repeat_with(|| None).take(active).collect();
        let committed_reserved = vec![false; active];
        let quarantined = std::iter::repeat_with(|| None).take(quarantine).collect();
        Ok(Self {
            core: RefCell::new(LedgerCore {
                descriptor,
                authority,
                next_invocation: 1,
                next_generation: 1,
                active_storage,
                quarantine_storage,
                available_quarantine,
                authoritative_owners,
                owner_capacity,
                active_postcommit: 0,
                committed,
                committed_reserved,
                quarantined,
                poisoned: false,
                draining: false,
                root_pin,
            }),
        })
    }
}

impl<P: SettlementPin> LedgerCore<P> {
    fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn is_draining(&self) -> bool {
        self.draining
    }

    pub(crate) fn quarantined_count(&self) -> usize {
        self.quarantined.iter().flatten().count()
            + self
                .committed
                .iter()
                .flatten()
                .filter(|record| record.conflicted)
                .count()
    }

    fn register_owner(
        &mut self,
        slot: u64,
        generation: u64,
    ) -> Result<SettlementOwnerHandle, SettlementLedgerError> {
        self.require_live()?;
        if slot == 0 || generation == 0 {
            return Err(SettlementLedgerError::StaleOwner);
        }
        if self
            .authoritative_owners
            .iter()
            .any(|owner| owner.slot == slot)
        {
            return Err(SettlementLedgerError::DuplicateOwner);
        }
        if self.authoritative_owners.len() == self.owner_capacity {
            return Err(SettlementLedgerError::OwnerTableFull);
        }
        self.authoritative_owners.push(AuthoritativeOwner {
            slot,
            generation,
            state: AuthoritativeOwnerState::Live,
        });
        Ok(SettlementOwnerHandle {
            instance_nonce: self.root_pin.instance_nonce(),
            slot,
            generation,
        })
    }

    fn register_owner_pair(
        &mut self,
        owners: [(u64, u64); 2],
    ) -> Result<[SettlementOwnerHandle; 2], SettlementLedgerError> {
        self.require_live()?;
        if owners
            .iter()
            .any(|(slot, generation)| *slot == 0 || *generation == 0)
        {
            return Err(SettlementLedgerError::StaleOwner);
        }
        if owners[0].0 == owners[1].0
            || self
                .authoritative_owners
                .iter()
                .any(|owner| owners.iter().any(|(slot, _)| owner.slot == *slot))
        {
            return Err(SettlementLedgerError::DuplicateOwner);
        }
        if self
            .authoritative_owners
            .len()
            .checked_add(owners.len())
            .is_none_or(|required| required > self.owner_capacity)
        {
            return Err(SettlementLedgerError::OwnerTableFull);
        }
        let instance_nonce = self.root_pin.instance_nonce();
        let handles = owners.map(|(slot, generation)| SettlementOwnerHandle {
            instance_nonce,
            slot,
            generation,
        });
        self.authoritative_owners
            .extend(owners.map(|(slot, generation)| AuthoritativeOwner {
                slot,
                generation,
                state: AuthoritativeOwnerState::Live,
            }));
        Ok(handles)
    }

    fn stage_call(
        &self,
        frame: &mut ReservedFrame<P>,
        request: &ExecuteRequest,
        handles: &[SettlementOwnerHandle],
    ) -> Result<(), SettlementLedgerError> {
        if handles.len() != frame.storage.resource_count {
            return Err(SettlementLedgerError::WrongIdentity);
        }
        let mut entries = Vec::with_capacity(handles.len());
        for (owner_ordinal, handle) in handles.iter().enumerate() {
            if handle.instance_nonce != self.root_pin.instance_nonce() {
                return Err(SettlementLedgerError::WrongInstance);
            }
            entries.push(LedgerEntry {
                owner_ordinal: owner_ordinal as u32,
                slot: handle.slot,
                generation: handle.generation,
                state: LedgerState::InInvocation,
            });
        }
        frame.stage_call(request, &entries)
    }

    fn call_commit(&mut self, frame: &mut ReservedFrame<P>) -> Result<(), SettlementLedgerError> {
        self.require_live()?;
        if frame.phase != AuthoritativePhase::Reserved {
            return Err(SettlementLedgerError::WrongPhase);
        }
        if !frame.staged {
            return Err(SettlementLedgerError::NotStaged);
        }
        for (index, entry) in frame.storage.before_entries.iter().enumerate() {
            if frame.storage.before_entries[..index]
                .iter()
                .any(|prior| prior.slot == entry.slot)
            {
                return Err(SettlementLedgerError::DuplicateOwner);
            }
            let owner = self
                .authoritative_owners
                .iter()
                .find(|owner| owner.slot == entry.slot)
                .ok_or(SettlementLedgerError::StaleOwner)?;
            if owner.generation != entry.generation || owner.state != AuthoritativeOwnerState::Live
            {
                return Err(SettlementLedgerError::StaleOwner);
            }
        }
        for entry in &frame.storage.before_entries {
            let owner = self
                .authoritative_owners
                .iter_mut()
                .find(|owner| owner.slot == entry.slot)
                .expect("validated owner remains in fixed authoritative table");
            owner.state = AuthoritativeOwnerState::InInvocation(frame.identity);
        }
        frame.phase = AuthoritativePhase::CallCommitted;
        self.active_postcommit += 1;
        Ok(())
    }

    pub(crate) fn reserve(&mut self) -> Result<ReservedFrame<P>, SettlementLedgerError> {
        self.require_live()?;
        if self.active_postcommit != 0 {
            return Err(SettlementLedgerError::Draining);
        }
        let invocation =
            NonZeroU64::new(self.next_invocation).ok_or(SettlementLedgerError::CounterExhausted)?;
        let generation =
            NonZeroU64::new(self.next_generation).ok_or(SettlementLedgerError::CounterExhausted)?;
        let next_invocation = self
            .next_invocation
            .checked_add(1)
            .ok_or(SettlementLedgerError::CounterExhausted)?;
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(SettlementLedgerError::CounterExhausted)?;
        let challenge = self.authority.provider_challenge(
            self.descriptor.fingerprints.call_contract,
            invocation,
            generation,
        )?;
        let pin = self.root_pin.retain();
        if !self.root_pin.is_same_instance(&pin)
            || self.root_pin.instance_nonce() != pin.instance_nonce()
        {
            self.poisoned = true;
            self.draining = true;
            return Err(SettlementLedgerError::WrongInstance);
        }
        let quarantine_index = *self
            .available_quarantine
            .last()
            .ok_or(SettlementLedgerError::CapacityExhausted)?;
        let committed_index = self
            .committed
            .iter()
            .zip(&self.committed_reserved)
            .position(|(record, reserved)| record.is_none() && !reserved)
            .ok_or(SettlementLedgerError::CapacityExhausted)?;
        let mut storage = self
            .active_storage
            .pop()
            .ok_or(SettlementLedgerError::CapacityExhausted)?;
        storage.clear_sensitive();
        let quarantine_replacement = self
            .quarantine_storage
            .pop()
            .ok_or(SettlementLedgerError::CapacityExhausted)?;
        let removed_quarantine = self
            .available_quarantine
            .pop()
            .ok_or(SettlementLedgerError::CapacityExhausted)?;
        debug_assert_eq!(removed_quarantine, quarantine_index);
        self.committed_reserved[committed_index] = true;
        self.next_invocation = next_invocation;
        self.next_generation = next_generation;
        Ok(ReservedFrame {
            identity: RecoveryIdentity {
                call: CallIdentity {
                    call_contract: self.descriptor.fingerprints.call_contract,
                    invocation,
                    frame_generation: generation,
                    provider_challenge: challenge,
                },
                recovery_contract: self.descriptor.fingerprints.recovery_contract,
                settlement_graph: self.descriptor.fingerprints.settlement_graph,
            },
            phase: AuthoritativePhase::Reserved,
            staged: false,
            storage,
            quarantine_index,
            quarantine_replacement: Some(quarantine_replacement),
            committed_index,
            _pin: pin,
        })
    }

    /// Perform every fallible, allocating, parsing and authentication step
    /// while the RAII transaction still owns `reserved`. A returned error or
    /// panic therefore unwinds through `SettlementTransaction::drop`, which
    /// absorbs this exact frame and pin into quarantine.
    fn prepare_receipt_commit(
        &mut self,
        reserved: &mut ReservedFrame<P>,
        evidence: ReceiptCommitEvidence<'_>,
    ) -> Result<PreparedReceiptCommit, SettlementLedgerError> {
        self.require_live()?;
        if reserved.phase != AuthoritativePhase::ProviderSettled || self.active_postcommit != 1 {
            return Err(SettlementLedgerError::WrongPhase);
        }
        if !self.root_pin.is_same_instance(&reserved._pin)
            || evidence.request.identity != reserved.identity.call
            || evidence.frame.identity != reserved.identity
            || evidence.decision.identity != reserved.identity
            || evidence.candidate.identity != reserved.identity
        {
            return Err(SettlementLedgerError::WrongIdentity);
        }
        for entry in &reserved.storage.before_entries {
            let owner = self
                .authoritative_owners
                .iter()
                .find(|owner| owner.slot == entry.slot)
                .ok_or(SettlementLedgerError::StaleOwner)?;
            if owner.generation != entry.generation
                || owner.state != AuthoritativeOwnerState::InInvocation(reserved.identity)
            {
                return Err(SettlementLedgerError::StaleOwner);
            }
        }
        let response_storage = match evidence.response_storage {
            ResponseStorageEvidence::External(storage) => storage,
            ResponseStorageEvidence::Reserved => &reserved.storage.execute_response,
        };
        let candidate_scratch = std::mem::take(&mut reserved.storage.candidate_dispositions);
        let decoded_candidate = CandidateReceipt::parse_reusing(
            &reserved.storage.candidate,
            &self.descriptor,
            candidate_scratch,
        )?;
        if decoded_candidate != *evidence.candidate {
            return Err(SettlementLedgerError::Wire(WireError::CrossBinding));
        }
        reserved.storage.candidate_dispositions = decoded_candidate.dispositions;
        validate_candidate_replay_preencoded(
            &self.descriptor,
            evidence.request,
            &reserved.storage.request,
            evidence.execute_return_code,
            response_storage,
            evidence.response,
            evidence.frame,
            evidence.decision,
            &reserved.storage.decision,
            evidence.actions,
            evidence.candidate,
        )?;
        let candidate_hash = candidate_digest(&reserved.storage.candidate);
        let published_owner = match evidence.candidate.outcome {
            CandidateOutcome::Owned(owner) => Some(owner),
            CandidateOutcome::Scalar | CandidateOutcome::Failure | CandidateOutcome::Abort => None,
        };
        for entry in &mut reserved.storage.after_entries {
            if published_owner == Some(entry.owner_ordinal) {
                entry.generation = entry
                    .generation
                    .checked_add(1)
                    .ok_or(SettlementLedgerError::CounterExhausted)?;
                entry.state = LedgerState::Published;
            } else {
                entry.generation = 0;
                entry.state = LedgerState::Retired;
            }
        }
        let (ledger_before, ledger_after) = ledger_transition_digests(
            self.authority.instance_binding(),
            reserved.identity.call.call_contract,
            reserved.identity.call.invocation,
            reserved.identity.call.frame_generation,
            candidate_hash,
            &reserved.storage.before_entries,
            &reserved.storage.after_entries,
            published_owner,
        )?;
        #[cfg(test)]
        receipt_prepare_panic_failpoint();
        let receipt = self.authority.authenticate_receipt(
            evidence.candidate,
            candidate_hash,
            ledger_before,
            ledger_after,
        )?;
        self.authority.verify_receipt(
            &receipt,
            &self.descriptor,
            evidence.candidate,
            candidate_hash,
            ledger_before,
            ledger_after,
        )?;
        let cache_index = reserved.committed_index;
        if self.committed.get(cache_index).is_none()
            || self.committed[cache_index].is_some()
            || self.committed_reserved.get(cache_index) != Some(&true)
            || reserved.quarantine_replacement.is_none()
        {
            return Err(SettlementLedgerError::CapacityExhausted);
        }
        let publication = match published_owner {
            Some(owner) => Publication::Owned(owner),
            None => Publication::NoOwned,
        };
        let refreshed_owner = match published_owner {
            Some(owner_ordinal) => {
                let entry = reserved
                    .storage
                    .before_entries
                    .get(owner_ordinal as usize)
                    .ok_or(SettlementLedgerError::StaleOwner)?;
                Some(SettlementOwnerHandle {
                    instance_nonce: self.root_pin.instance_nonce(),
                    slot: entry.slot,
                    generation: entry
                        .generation
                        .checked_add(1)
                        .ok_or(SettlementLedgerError::CounterExhausted)?,
                })
            }
            None => None,
        };
        Ok(PreparedReceiptCommit {
            candidate_digest: candidate_hash,
            ledger_before,
            ledger_after,
            receipt,
            publication,
            cache_index,
            published_owner,
            refreshed_owner,
        })
    }

    /// Third irreversible boundary. `prepare_receipt_commit` established every
    /// index, generation, capacity, receipt and owner transition. This method
    /// only performs fixed-capacity state writes and transfers the exact pin.
    fn linearize_receipt_commit(
        &mut self,
        mut reserved: ReservedFrame<P>,
        prepared: PreparedReceiptCommit,
    ) -> CommittedResult {
        let result = CommittedResult {
            receipt: prepared.receipt,
            publication: prepared.publication,
            ledger_before: prepared.ledger_before,
            ledger_after: prepared.ledger_after,
            published_owner: prepared.refreshed_owner,
        };

        // ReceiptCommit linearization point: both the authoritative terminal
        // phase and exact cached authenticated result become visible within
        // this one `&mut self` operation, with no fallible work afterward.
        reserved.phase = AuthoritativePhase::ReceiptCommitted;
        self.active_postcommit = self
            .active_postcommit
            .checked_sub(1)
            .expect("receipt commit owns one active postcommit frame");
        for entry in &reserved.storage.before_entries {
            let owner = self
                .authoritative_owners
                .iter_mut()
                .find(|owner| owner.slot == entry.slot)
                .expect("validated owner remains in fixed authoritative table");
            if prepared.published_owner == Some(entry.owner_ordinal) {
                owner.generation = owner
                    .generation
                    .checked_add(1)
                    .expect("publication generation was preflighted");
                owner.state = AuthoritativeOwnerState::Live;
            } else {
                owner.state = AuthoritativeOwnerState::Retired;
            }
        }
        let pin = reserved._pin;
        reserved.storage.clear_sensitive();
        self.active_storage.push(reserved.storage);
        self.quarantine_storage.push(
            reserved
                .quarantine_replacement
                .take()
                .expect("every active frame owns one quarantine permit"),
        );
        self.available_quarantine.push(reserved.quarantine_index);
        self.committed_reserved[prepared.cache_index] = false;
        self.committed[prepared.cache_index] = Some(CommittedRecord {
            identity: reserved.identity,
            candidate_digest: prepared.candidate_digest,
            result,
            conflicted: false,
            _pin: pin,
        });
        result
    }

    /// Idempotent replay returns the byte-identical receipt. A conflicting
    /// postcommit replay cannot alter the original ledger result; it only
    /// monotonically poisons, drains and quarantines this exact instance.
    pub(crate) fn replay_committed(
        &mut self,
        identity: RecoveryIdentity,
        candidate_bytes: &[u8],
    ) -> Result<CommittedResult, SettlementLedgerError> {
        let candidate_hash = candidate_digest(candidate_bytes);
        let record = self
            .committed
            .iter_mut()
            .flatten()
            .find(|record| record.identity == identity)
            .ok_or(SettlementLedgerError::WrongIdentity)?;
        if record.candidate_digest == candidate_hash {
            return Ok(record.result);
        }
        record.conflicted = true;
        self.poisoned = true;
        self.draining = true;
        Err(SettlementLedgerError::ConflictingReplay)
    }

    pub(crate) fn committed_result(&self, identity: RecoveryIdentity) -> Option<CommittedResult> {
        self.committed
            .iter()
            .flatten()
            .find(|record| record.identity == identity)
            .map(|record| record.result)
    }

    /// Absorbing interruption path. In particular, a frame in `Finalizing`
    /// can never transition back to a retryable phase.
    fn absorb_uncertain(&mut self, mut reserved: ReservedFrame<P>) {
        self.active_postcommit = self
            .active_postcommit
            .checked_sub(1)
            .expect("uncertain absorption owns one active postcommit frame");
        let index = reserved.quarantine_index;
        let replacement = reserved
            .quarantine_replacement
            .take()
            .expect("postcommit frame structurally owns its quarantine permit");
        self.active_storage.push(replacement);
        self.committed_reserved[reserved.committed_index] = false;
        for entry in &reserved.storage.before_entries {
            if let Some(owner) = self
                .authoritative_owners
                .iter_mut()
                .find(|owner| owner.slot == entry.slot)
            {
                if owner.state == AuthoritativeOwnerState::InInvocation(reserved.identity) {
                    owner.state = AuthoritativeOwnerState::Quarantined;
                }
            }
        }
        reserved.phase = AuthoritativePhase::Uncertain;
        self.quarantined[index] = Some(QuarantinedFrame { frame: reserved });
        self.poisoned = true;
        self.draining = true;
    }

    fn recycle_precommit(&mut self, mut reserved: ReservedFrame<P>) {
        reserved.storage.clear_sensitive();
        self.active_storage.push(reserved.storage);
        self.quarantine_storage.push(
            reserved
                .quarantine_replacement
                .take()
                .expect("precommit frame structurally owns its quarantine permit"),
        );
        self.available_quarantine.push(reserved.quarantine_index);
        self.committed_reserved[reserved.committed_index] = false;
    }

    fn require_live(&self) -> Result<(), SettlementLedgerError> {
        if self.poisoned {
            Err(SettlementLedgerError::Poisoned)
        } else if self.draining {
            Err(SettlementLedgerError::Draining)
        } else {
            Ok(())
        }
    }
}

impl<P: SettlementPin> SettlementLedger<P> {
    pub(crate) fn register_owner(
        &self,
        slot: u64,
        generation: u64,
    ) -> Result<SettlementOwnerHandle, SettlementLedgerError> {
        self.core.borrow_mut().register_owner(slot, generation)
    }

    pub(crate) fn register_owner_pair(
        &self,
        owners: [(u64, u64); 2],
    ) -> Result<[SettlementOwnerHandle; 2], SettlementLedgerError> {
        self.core.borrow_mut().register_owner_pair(owners)
    }

    pub(crate) fn reserve(&self) -> Result<SettlementTransaction<'_, P>, SettlementLedgerError> {
        let frame = self.core.borrow_mut().reserve()?;
        Ok(SettlementTransaction {
            core: &self.core,
            frame: Some(frame),
        })
    }

    pub(crate) fn is_poisoned(&self) -> bool {
        self.core.borrow().is_poisoned()
    }

    pub(crate) fn is_draining(&self) -> bool {
        self.core.borrow().is_draining()
    }

    pub(crate) fn quarantined_count(&self) -> usize {
        self.core.borrow().quarantined_count()
    }

    pub(crate) fn replay_committed(
        &self,
        identity: RecoveryIdentity,
        candidate_bytes: &[u8],
    ) -> Result<CommittedResult, SettlementLedgerError> {
        self.core
            .borrow_mut()
            .replay_committed(identity, candidate_bytes)
    }

    pub(crate) fn committed_result(&self, identity: RecoveryIdentity) -> Option<CommittedResult> {
        self.core.borrow().committed_result(identity)
    }
}

/// Host-owned RAII settlement transaction. Safe code cannot detach a
/// post-CallCommit frame from the ledger. Every ordinary or unwinding drop
/// infallibly quarantines its exact pin, owner set and evidence.
pub(crate) struct SettlementTransaction<'a, P: SettlementPin> {
    core: &'a RefCell<LedgerCore<P>>,
    frame: Option<ReservedFrame<P>>,
}

impl<P: SettlementPin> SettlementTransaction<'_, P> {
    pub(crate) fn identity(&self) -> RecoveryIdentity {
        self.frame
            .as_ref()
            .expect("live transaction owns one frame")
            .identity()
    }

    pub(crate) fn stage_call(
        &mut self,
        request: &ExecuteRequest,
        owners: &[SettlementOwnerHandle],
    ) -> Result<(), SettlementLedgerError> {
        let frame = self
            .frame
            .as_mut()
            .ok_or(SettlementLedgerError::WrongPhase)?;
        self.core.borrow().stage_call(frame, request, owners)
    }

    pub(crate) fn call_commit(&mut self) -> Result<(), SettlementLedgerError> {
        let frame = self
            .frame
            .as_mut()
            .ok_or(SettlementLedgerError::WrongPhase)?;
        self.core.borrow_mut().call_commit(frame)
    }

    pub(crate) fn decision_commit(
        &mut self,
        decision: &SettlementDecision,
    ) -> Result<(), SettlementLedgerError> {
        self.frame
            .as_mut()
            .ok_or(SettlementLedgerError::WrongPhase)?
            .decision_commit(decision)
    }

    pub(crate) fn finalizer_started(&mut self) -> Result<(), SettlementLedgerError> {
        self.frame
            .as_mut()
            .ok_or(SettlementLedgerError::WrongPhase)?
            .finalizer_started()
    }

    pub(crate) fn finalizer_completed(&mut self) -> Result<(), SettlementLedgerError> {
        self.frame
            .as_mut()
            .ok_or(SettlementLedgerError::WrongPhase)?
            .finalizer_completed()
    }

    pub(crate) fn provider_settled(&mut self) -> Result<(), SettlementLedgerError> {
        self.frame
            .as_mut()
            .ok_or(SettlementLedgerError::WrongPhase)?
            .provider_settled()
    }

    pub(crate) fn receipt_commit(
        mut self,
        evidence: ReceiptCommitEvidence<'_>,
    ) -> Result<CommittedResult, SettlementLedgerError> {
        let prepared = {
            let frame = self
                .frame
                .as_mut()
                .ok_or(SettlementLedgerError::WrongPhase)?;
            self.core
                .borrow_mut()
                .prepare_receipt_commit(frame, evidence)?
        };
        // Borrow the ledger before detaching the frame. Any RefCell failure or
        // preparation panic therefore still unwinds through this guard's Drop.
        let mut core = self.core.borrow_mut();
        let frame = self
            .frame
            .take()
            .expect("receipt preparation retained the guarded frame");
        Ok(core.linearize_receipt_commit(frame, prepared))
    }

    pub(crate) fn request_bytes(&self) -> &[u8] {
        self.frame
            .as_ref()
            .expect("live transaction owns one frame")
            .request_bytes()
    }

    pub(crate) fn execute_response_storage_mut(&mut self) -> &mut [u8] {
        self.frame
            .as_mut()
            .expect("live transaction owns one frame")
            .execute_response_storage_mut()
    }

    pub(crate) fn execute_response_bytes(&self) -> &[u8] {
        &self
            .frame
            .as_ref()
            .expect("live transaction owns one frame")
            .storage
            .execute_response
    }

    pub(crate) fn frame_storage_mut(&mut self) -> &mut [u8] {
        self.frame
            .as_mut()
            .expect("live transaction owns one frame")
            .frame_storage_mut()
    }

    pub(crate) fn decision_bytes(&self) -> &[u8] {
        &self
            .frame
            .as_ref()
            .expect("live transaction owns one frame")
            .storage
            .decision
    }

    pub(crate) fn frame_bytes(&self) -> &[u8] {
        &self
            .frame
            .as_ref()
            .expect("live transaction owns one frame")
            .storage
            .frame
    }

    pub(crate) fn action_storage_mut(&mut self) -> &mut [u8] {
        self.frame
            .as_mut()
            .expect("live transaction owns one frame")
            .action_storage_mut()
    }

    pub(crate) fn candidate_storage_mut(&mut self) -> &mut [u8] {
        self.frame
            .as_mut()
            .expect("live transaction owns one frame")
            .candidate_storage_mut()
    }

    pub(crate) fn candidate_bytes(&self) -> &[u8] {
        &self
            .frame
            .as_ref()
            .expect("live transaction owns one frame")
            .storage
            .candidate
    }

    #[cfg(test)]
    fn buffer_signature(&self) -> [(*const u8, usize); 7] {
        let storage = &self.frame.as_ref().unwrap().storage;
        [
            (storage.request.as_ptr(), storage.request.capacity()),
            (
                storage.execute_response.as_ptr(),
                storage.execute_response.capacity(),
            ),
            (storage.frame.as_ptr(), storage.frame.capacity()),
            (storage.decision.as_ptr(), storage.decision.capacity()),
            (storage.action.as_ptr(), storage.action.capacity()),
            (storage.candidate.as_ptr(), storage.candidate.capacity()),
            (storage.receipt.as_ptr(), storage.receipt.capacity()),
        ]
    }
}

impl<P: SettlementPin> Drop for SettlementTransaction<'_, P> {
    fn drop(&mut self) {
        let Some(frame) = self.frame.take() else {
            return;
        };
        let mut core = self.core.borrow_mut();
        if frame.phase == AuthoritativePhase::Reserved {
            core.recycle_precommit(frame);
        } else {
            core.absorb_uncertain(frame);
        }
    }
}

#[cfg(test)]
#[path = "settlement_ledger/tests.rs"]
mod tests;
