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
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use semaprax::codegen::emit_native_callable_v3_descriptor;
    use semaprax::hir::DeclarationId;
    use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::callable_wire_v3::{
        action_chain_digest, arm_reusable_storage_allocation_failure, decision_digest,
        frame_digest, request_digest, response_storage_digest, ActionBoundary, CandidateOutcome,
        CellState, Decision, Disposition, DispositionCell, ExecuteOutcome, ExecuteReturn,
        FramePhase, ResourceCell,
    };
    use crate::descriptor_v3::{Action as GraphAction, Outcome as GraphOutcome, ResourceState};

    #[derive(Clone)]
    struct TestPin {
        instance: NonZeroU64,
        drops: Rc<RefCell<Vec<&'static str>>>,
        label: &'static str,
    }

    impl SettlementPin for TestPin {
        fn retain(&self) -> Self {
            Self {
                instance: self.instance,
                drops: Rc::clone(&self.drops),
                label: "retain",
            }
        }

        fn instance_nonce(&self) -> NonZeroU64 {
            self.instance
        }

        fn is_same_instance(&self, other: &Self) -> bool {
            self.instance == other.instance && Rc::ptr_eq(&self.drops, &other.drops)
        }
    }

    impl Drop for TestPin {
        fn drop(&mut self) {
            self.drops.borrow_mut().push(self.label);
        }
    }

    fn descriptor() -> Descriptor {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let artifact = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
        )
        .unwrap();
        Descriptor::parse(artifact.bytes()).unwrap()
    }

    fn authority() -> ReceiptAuthority {
        ReceiptAuthority::from_os(NonZeroU64::new(91).unwrap()).unwrap()
    }

    fn ledger() -> (SettlementLedger<TestPin>, Rc<RefCell<Vec<&'static str>>>) {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let pin = TestPin {
            instance: NonZeroU64::new(91).unwrap(),
            drops: Rc::clone(&drops),
            label: "root",
        };
        (
            SettlementLedger::try_new(pin, descriptor(), authority()).unwrap(),
            drops,
        )
    }

    fn ledger_for(function: &str) -> (SettlementLedger<TestPin>, Rc<RefCell<Vec<&'static str>>>) {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let pin = TestPin {
            instance: NonZeroU64::new(91).unwrap(),
            drops: Rc::clone(&drops),
            label: "root",
        };
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let artifact =
            emit_native_callable_v3_descriptor(&corpus.program, &DeclarationId::new(function))
                .unwrap();
        let descriptor = Descriptor::parse(artifact.bytes()).unwrap();
        (
            SettlementLedger::try_new(pin, descriptor, authority()).unwrap(),
            drops,
        )
    }

    fn request(identity: RecoveryIdentity) -> ExecuteRequest {
        ExecuteRequest {
            identity: identity.call,
            arguments: vec![
                crate::callable_wire_v3::RequestArgument::Owned {
                    index: 0,
                    owner_ordinal: 0,
                    payload: 10,
                },
                crate::callable_wire_v3::RequestArgument::Owned {
                    index: 1,
                    owner_ordinal: 1,
                    payload: 20,
                },
            ],
        }
    }

    fn request_one(identity: RecoveryIdentity, payload: u64) -> ExecuteRequest {
        ExecuteRequest {
            identity: identity.call,
            arguments: vec![crate::callable_wire_v3::RequestArgument::Owned {
                index: 0,
                owner_ordinal: 0,
                payload,
            }],
        }
    }

    fn owners(ledger: &SettlementLedger<TestPin>) -> [SettlementOwnerHandle; 2] {
        [
            ledger.register_owner(4, 7).unwrap(),
            ledger.register_owner(5, 9).unwrap(),
        ]
    }

    struct AbortEvidence {
        request: ExecuteRequest,
        response_storage: Vec<u8>,
        frame: RecoveryFrame,
        decision: SettlementDecision,
        actions: Vec<ActionRecord>,
        candidate: CandidateReceipt,
    }

    fn abort_evidence(descriptor: &Descriptor, identity: RecoveryIdentity) -> AbortEvidence {
        let request = ExecuteRequest {
            identity: identity.call,
            arguments: vec![
                crate::callable_wire_v3::RequestArgument::Owned {
                    index: 0,
                    owner_ordinal: 0,
                    payload: 10,
                },
                crate::callable_wire_v3::RequestArgument::Owned {
                    index: 1,
                    owner_ordinal: 1,
                    payload: 20,
                },
            ],
        };
        let request_hash = request_digest(&request.encode());
        let response_storage = vec![0; descriptor.capacities.execute_response as usize];
        let response_hash = response_storage_digest(9, &response_storage);
        let decision = SettlementDecision {
            identity,
            decision: Decision::AbortPhysical(9),
        };
        let decision_hash = decision_digest(&decision.encode());
        let checkpoint = descriptor.graph.checkpoints.first().unwrap();
        let payloads = [10_u64, 20_u64];
        let mut cells = checkpoint
            .resources
            .iter()
            .enumerate()
            .map(|(owner, state)| ResourceCell {
                state: match state {
                    ResourceState::Live => CellState::Live,
                    ResourceState::ProvisionalResult => CellState::ProvisionalResult,
                    ResourceState::Dead => CellState::Dead,
                    ResourceState::Finalizing | ResourceState::Published => unreachable!(),
                },
                payload: payloads[owner],
            })
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        for (semantic_index, owner) in checkpoint.abort_order.iter().copied().enumerate() {
            let before = cells[owner as usize];
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index as u32,
                boundary: ActionBoundary::Started,
                owner_ordinal: owner,
                payload: before.payload,
                before: before.state,
                after: CellState::Finalizing,
                checkpoint: checkpoint.id,
            });
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index as u32,
                boundary: ActionBoundary::Completed,
                owner_ordinal: owner,
                payload: before.payload,
                before: CellState::Finalizing,
                after: CellState::Dead,
                checkpoint: checkpoint.id,
            });
            cells[owner as usize].state = CellState::Dead;
        }
        let action_hash =
            action_chain_digest(decision_hash, checkpoint.abort_order.len(), &actions).unwrap();
        let mut frame = RecoveryFrame {
            identity,
            request_digest: request_hash,
            response_storage_digest: response_hash,
            semantic_trace_digest: [0; 32],
            execute_return: ExecuteReturn::Returned(9),
            checkpoint: checkpoint.id,
            phase: FramePhase::ProviderSettled,
            decision_digest: decision_hash,
            next_action: checkpoint.abort_order.len() as u32,
            record_count: actions.len() as u32,
            active_finalizers: 0,
            cells,
            action_chain_digest: action_hash,
            pre_candidate_digest: [0; 32],
        };
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        let candidate = CandidateReceipt {
            identity,
            request_digest: request_hash,
            response_storage_digest: response_hash,
            semantic_trace_digest: [0; 32],
            frame_digest: frame.pre_candidate_digest,
            decision_digest: decision_hash,
            action_evidence_digest: action_hash,
            outcome: CandidateOutcome::Abort,
            active_finalizers: 0,
            dispositions: frame
                .cells
                .iter()
                .map(|cell| DispositionCell {
                    disposition: Disposition::Dead,
                    payload: cell.payload,
                })
                .collect(),
        };
        AbortEvidence {
            request,
            response_storage,
            frame,
            decision,
            actions,
            candidate,
        }
    }

    struct OwnedEvidence {
        request: ExecuteRequest,
        response_storage: Vec<u8>,
        response: ExecuteResponse,
        frame: RecoveryFrame,
        decision: SettlementDecision,
        actions: Vec<ActionRecord>,
        candidate: CandidateReceipt,
    }

    fn owned_evidence(descriptor: &Descriptor, identity: RecoveryIdentity) -> OwnedEvidence {
        let request = ExecuteRequest {
            identity: identity.call,
            arguments: vec![crate::callable_wire_v3::RequestArgument::Owned {
                index: 0,
                owner_ordinal: 0,
                payload: 42,
            }],
        };
        let checkpoint = descriptor
            .graph
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.outcome == Some(GraphOutcome::OwnedSuccess(0)))
            .unwrap();
        let trace = descriptor
            .graph
            .edges
            .iter()
            .find_map(|edge| match &edge.action {
                GraphAction::CertifyOutcome(evidence) if edge.to == checkpoint.id => Some(evidence),
                _ => None,
            })
            .unwrap();
        let response = ExecuteResponse {
            identity: identity.call,
            request_digest: request_digest(&request.encode()),
            checkpoint: checkpoint.id,
            outcome: ExecuteOutcome::Owned {
                owner_ordinal: 0,
                payload: 42,
            },
            event_ordinals: trace.ordinals.clone(),
            storage_capacity: descriptor.capacities.execute_response as usize,
        };
        let decision = SettlementDecision {
            identity,
            decision: Decision::AcceptOwned(0),
        };
        let decision_hash = decision_digest(&decision.encode());
        let payloads = request
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                crate::callable_wire_v3::RequestArgument::Owned {
                    owner_ordinal,
                    payload,
                    ..
                } => Some((*owner_ordinal, *payload)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut cells = checkpoint
            .resources
            .iter()
            .enumerate()
            .map(|(owner, state)| ResourceCell {
                state: match state {
                    ResourceState::Live => CellState::Live,
                    ResourceState::ProvisionalResult => CellState::ProvisionalResult,
                    ResourceState::Dead => CellState::Dead,
                    ResourceState::Finalizing | ResourceState::Published => unreachable!(),
                },
                payload: payloads[&(owner as u32)],
            })
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        let mut semantic_index = 0_u32;
        for owner in &checkpoint.accept_order {
            let cell = cells[*owner as usize];
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index,
                boundary: ActionBoundary::Started,
                owner_ordinal: *owner,
                payload: cell.payload,
                before: cell.state,
                after: CellState::Finalizing,
                checkpoint: checkpoint.id,
            });
            actions.push(ActionRecord {
                identity,
                semantic_action_index: semantic_index,
                boundary: ActionBoundary::Completed,
                owner_ordinal: *owner,
                payload: cell.payload,
                before: CellState::Finalizing,
                after: CellState::Dead,
                checkpoint: checkpoint.id,
            });
            cells[*owner as usize].state = CellState::Dead;
            semantic_index += 1;
        }
        let cell = cells[0];
        actions.push(ActionRecord {
            identity,
            semantic_action_index: semantic_index,
            boundary: ActionBoundary::Publish,
            owner_ordinal: 0,
            payload: cell.payload,
            before: CellState::ProvisionalResult,
            after: CellState::Published,
            checkpoint: checkpoint.id,
        });
        cells[0].state = CellState::Published;
        semantic_index += 1;
        let action_hash =
            action_chain_digest(decision_hash, semantic_index as usize, &actions).unwrap();
        let response_storage = response.encode();
        let mut trace_hasher = Sha256::new();
        trace_hasher.update(b"semaprax.native-recovery-trace-evidence.v1\0");
        trace_hasher.update(descriptor.fingerprints.trace_path_certificate);
        trace_hasher.update((response.event_ordinals.len() as u64).to_le_bytes());
        for ordinal in &response.event_ordinals {
            trace_hasher.update(ordinal.to_le_bytes());
        }
        trace_hasher.update([2]);
        let semantic_trace_digest: [u8; 32] = trace_hasher.finalize().into();
        let mut frame = RecoveryFrame {
            identity,
            request_digest: request_digest(&request.encode()),
            response_storage_digest: response_storage_digest(0, &response_storage),
            semantic_trace_digest,
            execute_return: ExecuteReturn::Returned(0),
            checkpoint: checkpoint.id,
            phase: FramePhase::ProviderSettled,
            decision_digest: decision_hash,
            next_action: semantic_index,
            record_count: actions.len() as u32,
            active_finalizers: 0,
            cells,
            action_chain_digest: action_hash,
            pre_candidate_digest: [0; 32],
        };
        let provisional = frame.encode();
        frame.pre_candidate_digest = frame_digest(&provisional[..provisional.len() - 32]);
        let candidate = CandidateReceipt {
            identity,
            request_digest: frame.request_digest,
            response_storage_digest: frame.response_storage_digest,
            semantic_trace_digest,
            frame_digest: frame.pre_candidate_digest,
            decision_digest: decision_hash,
            action_evidence_digest: action_hash,
            outcome: CandidateOutcome::Owned(0),
            active_finalizers: 0,
            dispositions: vec![DispositionCell {
                disposition: Disposition::Published,
                payload: 42,
            }],
        };
        OwnedEvidence {
            request,
            response_storage,
            response,
            frame,
            decision,
            actions,
            candidate,
        }
    }

    #[test]
    fn reservation_is_monotonic_exact_instance_and_fully_preallocated() {
        let (ledger, _) = ledger();
        let first = ledger.reserve().unwrap();
        let second = ledger.reserve().unwrap();
        assert_ne!(
            first.identity().call.invocation,
            second.identity().call.invocation
        );
        assert_ne!(
            first.identity().call.frame_generation,
            second.identity().call.frame_generation
        );
        assert_ne!(
            first.identity().call.provider_challenge,
            second.identity().call.provider_challenge
        );
        assert_eq!(
            first.frame.as_ref().unwrap().storage.request.len(),
            ledger.core.borrow().descriptor.capacities.request as usize
        );
        assert_eq!(
            first.frame.as_ref().unwrap().storage.receipt.len(),
            HOST_RECEIPT_BYTES
        );
        assert_eq!(
            first
                .frame
                .as_ref()
                .unwrap()
                .storage
                .before_entries
                .capacity(),
            2
        );
    }

    #[test]
    fn authority_cannot_be_reused_for_another_exact_instance() {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let pin = TestPin {
            instance: NonZeroU64::new(91).unwrap(),
            drops,
            label: "root",
        };
        let wrong_authority = ReceiptAuthority::from_os(NonZeroU64::new(92).unwrap()).unwrap();
        assert!(matches!(
            SettlementLedger::try_new(pin, descriptor(), wrong_authority),
            Err(SettlementLedgerError::WrongInstance)
        ));
    }

    #[test]
    fn exhausted_counter_does_not_consume_preallocated_storage_or_advance_peer_counter() {
        let (ledger, _) = ledger();
        ledger.core.borrow_mut().next_generation = u64::MAX;
        let active_before = ledger.core.borrow().active_storage.len();
        let invocation_before = ledger.core.borrow().next_invocation;
        assert_eq!(
            ledger.reserve().err(),
            Some(SettlementLedgerError::CounterExhausted)
        );
        assert_eq!(ledger.core.borrow().active_storage.len(), active_before);
        assert_eq!(ledger.core.borrow().next_invocation, invocation_before);
    }

    #[test]
    fn call_commit_and_finalizer_boundaries_are_fail_closed() {
        let (ledger, _) = ledger();
        let handles = owners(&ledger);
        let mut frame = ledger.reserve().unwrap();
        assert_eq!(frame.call_commit(), Err(SettlementLedgerError::NotStaged));
        let request = request(frame.identity());
        frame.stage_call(&request, &handles).unwrap();
        frame.call_commit().unwrap();
        assert_eq!(frame.call_commit(), Err(SettlementLedgerError::WrongPhase));
        let decision = SettlementDecision {
            identity: frame.identity(),
            decision: crate::callable_wire_v3::Decision::AbortHostUnwind,
        };
        frame.decision_commit(&decision).unwrap();
        frame.finalizer_started().unwrap();
        assert_eq!(
            frame.provider_settled(),
            Err(SettlementLedgerError::WrongPhase)
        );
        drop(frame);
        assert!(ledger.is_poisoned());
        assert!(ledger.is_draining());
        assert_eq!(ledger.quarantined_count(), 1);
        assert_eq!(
            ledger.reserve().err(),
            Some(SettlementLedgerError::Poisoned)
        );
    }

    #[test]
    fn duplicate_stale_and_cross_frame_owner_use_reject_before_mutation() {
        let (ledger, _) = ledger();
        let handles = owners(&ledger);

        let mut duplicate = ledger.reserve().unwrap();
        let duplicate_request = request(duplicate.identity());
        duplicate
            .stage_call(&duplicate_request, &[handles[0], handles[0]])
            .unwrap();
        assert_eq!(
            duplicate.call_commit(),
            Err(SettlementLedgerError::DuplicateOwner)
        );
        drop(duplicate);
        assert!(!ledger.is_poisoned());

        let mut stale_handle = handles[0];
        stale_handle.generation += 1;
        let mut stale = ledger.reserve().unwrap();
        let stale_request = request(stale.identity());
        stale
            .stage_call(&stale_request, &[stale_handle, handles[1]])
            .unwrap();
        assert_eq!(stale.call_commit(), Err(SettlementLedgerError::StaleOwner));
        drop(stale);
        assert!(!ledger.is_poisoned());

        let mut first = ledger.reserve().unwrap();
        let mut second = ledger.reserve().unwrap();
        let first_request = request(first.identity());
        let second_request = request(second.identity());
        first.stage_call(&first_request, &handles).unwrap();
        second.stage_call(&second_request, &handles).unwrap();
        first.call_commit().unwrap();
        assert_eq!(second.call_commit(), Err(SettlementLedgerError::StaleOwner));
        drop(second);
        assert!(!ledger.is_poisoned());
        drop(first);
        assert!(ledger.is_poisoned());
        assert_eq!(ledger.quarantined_count(), 1);
    }

    #[test]
    fn raii_drop_and_unwind_absorb_every_postcommit_phase() {
        for phase in 0..5 {
            let (ledger, _) = ledger();
            let handles = owners(&ledger);
            let mut transaction = ledger.reserve().unwrap();
            let request = request(transaction.identity());
            transaction.stage_call(&request, &handles).unwrap();
            transaction.call_commit().unwrap();
            let decision = SettlementDecision {
                identity: transaction.identity(),
                decision: Decision::AbortHostUnwind,
            };
            if phase >= 1 {
                transaction.decision_commit(&decision).unwrap();
            }
            if phase == 2 {
                transaction.finalizer_started().unwrap();
            }
            if phase >= 3 {
                transaction.finalizer_started().unwrap();
                transaction.finalizer_completed().unwrap();
            }
            if phase == 4 {
                transaction.provider_settled().unwrap();
            }
            drop(transaction);
            assert!(ledger.is_poisoned(), "phase {phase} did not poison");
            assert_eq!(ledger.quarantined_count(), 1);
        }

        let (ledger, _) = ledger();
        let handles = owners(&ledger);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut transaction = ledger.reserve().unwrap();
            let request = request(transaction.identity());
            transaction.stage_call(&request, &handles).unwrap();
            transaction.call_commit().unwrap();
            panic!("injected postcommit unwind");
        }));
        assert!(unwind.is_err());
        assert!(ledger.is_poisoned());
        assert_eq!(ledger.quarantined_count(), 1);
    }

    #[test]
    fn precommit_drop_recycles_and_quarantine_permits_bound_all_outstanding_frames() {
        let (ledger, _) = ledger();
        {
            let mut outstanding = Vec::new();
            for _ in 0..64 {
                outstanding.push(ledger.reserve().unwrap());
            }
            assert_eq!(
                ledger.reserve().err(),
                Some(SettlementLedgerError::CapacityExhausted)
            );
        }
        assert!(!ledger.is_poisoned());
        assert_eq!(ledger.core.borrow().available_quarantine.len(), 64);
        assert!(ledger.reserve().is_ok());
    }

    #[test]
    fn forgotten_postcommit_guard_leaves_future_reservation_fail_closed() {
        let (ledger, _) = ledger();
        let handles = owners(&ledger);
        let mut transaction = ledger.reserve().unwrap();
        let request = request(transaction.identity());
        transaction.stage_call(&request, &handles).unwrap();
        transaction.call_commit().unwrap();
        std::mem::forget(transaction);
        assert_eq!(ledger.core.borrow().active_postcommit, 1);
        assert_eq!(
            ledger.reserve().err(),
            Some(SettlementLedgerError::Draining)
        );
    }

    #[test]
    fn call_commit_preserves_every_preallocated_buffer_and_capacity() {
        let (ledger, _) = ledger();
        let handles = owners(&ledger);
        let mut frame = ledger.reserve().unwrap();
        let request = request(frame.identity());
        frame.stage_call(&request, &handles).unwrap();
        let before = frame.buffer_signature();
        frame.call_commit().unwrap();
        assert_eq!(before, frame.buffer_signature());
    }

    #[test]
    fn leaf_pins_drop_after_quarantine_evidence_and_root_drops_last() {
        let (ledger, drops) = ledger();
        let handles = owners(&ledger);
        let mut frame = ledger.reserve().unwrap();
        let request = request(frame.identity());
        frame.stage_call(&request, &handles).unwrap();
        frame.call_commit().unwrap();
        let decision = SettlementDecision {
            identity: frame.identity(),
            decision: crate::callable_wire_v3::Decision::AbortHostUnwind,
        };
        frame.decision_commit(&decision).unwrap();
        frame.finalizer_started().unwrap();
        drop(frame);
        assert!(drops.borrow().is_empty());
        drop(ledger);
        assert_eq!(drops.borrow().as_slice(), ["retain", "root"]);
    }

    #[test]
    fn receipt_commit_is_atomic_replay_is_exact_and_conflict_preserves_original() {
        let (ledger, _) = ledger();
        let handles = owners(&ledger);
        let mut reserved = ledger.reserve().unwrap();
        let evidence = abort_evidence(&ledger.core.borrow().descriptor, reserved.identity());
        reserved.stage_call(&evidence.request, &handles).unwrap();
        reserved.call_commit().unwrap();
        reserved.decision_commit(&evidence.decision).unwrap();
        reserved
            .candidate_storage_mut()
            .copy_from_slice(&evidence.candidate.encode());
        reserved.provider_settled().unwrap();
        let identity = reserved.identity();
        let candidate_bytes = evidence.candidate.encode();
        let result = reserved
            .receipt_commit(ReceiptCommitEvidence {
                request: &evidence.request,
                execute_return_code: 9,
                response_storage: ResponseStorageEvidence::External(&evidence.response_storage),
                response: None,
                frame: &evidence.frame,
                decision: &evidence.decision,
                actions: &evidence.actions,
                candidate: &evidence.candidate,
            })
            .unwrap();
        assert_eq!(result.publication, Publication::NoOwned);
        assert_ne!(result.ledger_before, result.ledger_after);
        let owners_after_commit = ledger.core.borrow().authoritative_owners.clone();
        assert_eq!(
            ledger.replay_committed(identity, &candidate_bytes).unwrap(),
            result
        );
        assert_eq!(
            ledger.core.borrow().authoritative_owners,
            owners_after_commit
        );
        let original = ledger.committed_result(identity).unwrap();
        let mut conflicting = candidate_bytes;
        *conflicting.last_mut().unwrap() ^= 1;
        assert_eq!(
            ledger.replay_committed(identity, &conflicting),
            Err(SettlementLedgerError::ConflictingReplay)
        );
        assert_eq!(ledger.committed_result(identity), Some(original));
        assert_eq!(
            ledger.core.borrow().authoritative_owners,
            owners_after_commit
        );
        assert!(ledger.is_poisoned());
        assert!(ledger.is_draining());
        assert_eq!(ledger.quarantined_count(), 1);
    }

    #[test]
    fn owned_commit_returns_one_refreshed_handle_old_is_stale_and_replay_is_idempotent() {
        let (ledger, _) = ledger_for("token.identity");
        let old = ledger.register_owner(4, 7).unwrap();
        let mut transaction = ledger.reserve().unwrap();
        let evidence = owned_evidence(&ledger.core.borrow().descriptor, transaction.identity());
        transaction.stage_call(&evidence.request, &[old]).unwrap();
        transaction.call_commit().unwrap();
        transaction.decision_commit(&evidence.decision).unwrap();
        transaction
            .candidate_storage_mut()
            .copy_from_slice(&evidence.candidate.encode());
        transaction.provider_settled().unwrap();
        let identity = transaction.identity();
        let candidate_bytes = evidence.candidate.encode();
        let committed = transaction
            .receipt_commit(ReceiptCommitEvidence {
                request: &evidence.request,
                execute_return_code: 0,
                response_storage: ResponseStorageEvidence::External(&evidence.response_storage),
                response: Some(&evidence.response),
                frame: &evidence.frame,
                decision: &evidence.decision,
                actions: &evidence.actions,
                candidate: &evidence.candidate,
            })
            .unwrap();
        let refreshed = committed.published_owner.unwrap();
        assert_eq!(refreshed.slot, old.slot);
        assert_eq!(refreshed.generation, old.generation + 1);
        let owners_after_commit = ledger.core.borrow().authoritative_owners.clone();
        assert_eq!(
            ledger.replay_committed(identity, &candidate_bytes).unwrap(),
            committed
        );
        assert_eq!(
            ledger.core.borrow().authoritative_owners,
            owners_after_commit
        );

        let mut stale = ledger.reserve().unwrap();
        let stale_request = request_one(stale.identity(), 42);
        stale.stage_call(&stale_request, &[old]).unwrap();
        assert_eq!(stale.call_commit(), Err(SettlementLedgerError::StaleOwner));
        drop(stale);

        let mut reusable = ledger.reserve().unwrap();
        let reusable_request = request_one(reusable.identity(), 42);
        reusable
            .stage_call(&reusable_request, &[refreshed])
            .unwrap();
        reusable.call_commit().unwrap();
        drop(reusable);
        assert!(ledger.is_poisoned());
    }

    #[test]
    fn receipt_hmac_panic_quarantines_exact_evidence_and_retains_leaf_pin() {
        let (ledger, drops) = ledger_for("token.identity");
        let owner = ledger.register_owner(4, 7).unwrap();
        let mut transaction = ledger.reserve().unwrap();
        let evidence = owned_evidence(&ledger.core.borrow().descriptor, transaction.identity());
        transaction.stage_call(&evidence.request, &[owner]).unwrap();
        transaction.call_commit().unwrap();
        transaction.decision_commit(&evidence.decision).unwrap();
        transaction
            .execute_response_storage_mut()
            .copy_from_slice(&evidence.response_storage);
        let frame_bytes = evidence.frame.encode();
        transaction
            .frame_storage_mut()
            .copy_from_slice(&frame_bytes);
        let action_bytes = evidence.actions[0].encode();
        transaction
            .action_storage_mut()
            .copy_from_slice(&action_bytes);
        let candidate_bytes = evidence.candidate.encode();
        transaction
            .candidate_storage_mut()
            .copy_from_slice(&candidate_bytes);
        transaction.provider_settled().unwrap();
        let identity = transaction.identity();
        let request_bytes = evidence.request.encode();

        arm_receipt_prepare_panic();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = transaction.receipt_commit(ReceiptCommitEvidence {
                request: &evidence.request,
                execute_return_code: 0,
                response_storage: ResponseStorageEvidence::Reserved,
                response: Some(&evidence.response),
                frame: &evidence.frame,
                decision: &evidence.decision,
                actions: &evidence.actions,
                candidate: &evidence.candidate,
            });
        }));
        assert!(panic.is_err());

        {
            let core = ledger.core.borrow();
            assert_eq!(core.active_postcommit, 0);
            assert!(core.poisoned);
            assert!(core.draining);
            assert_eq!(core.quarantined_count(), 1);
            let quarantined = core.quarantined.iter().flatten().next().unwrap();
            assert_eq!(quarantined.frame.identity, identity);
            assert_eq!(quarantined.frame.storage.request, request_bytes);
            assert_eq!(
                quarantined.frame.storage.execute_response,
                evidence.response_storage
            );
            assert_eq!(quarantined.frame.storage.frame, frame_bytes);
            assert_eq!(quarantined.frame.storage.action, action_bytes);
            assert_eq!(quarantined.frame.storage.candidate, candidate_bytes);
            assert!(matches!(
                core.authoritative_owners[0].state,
                AuthoritativeOwnerState::Quarantined
            ));
        }
        assert!(
            drops.borrow().is_empty(),
            "quarantine must retain the leaf pin"
        );
        drop(ledger);
        assert_eq!(drops.borrow().as_slice(), ["retain", "root"]);
    }

    #[test]
    fn postcommit_decode_allocation_failure_quarantines_exact_reserved_evidence() {
        let (ledger, drops) = ledger_for("token.identity");
        let owner = ledger.register_owner(4, 7).unwrap();
        let mut transaction = ledger.reserve().unwrap();
        let evidence = owned_evidence(&ledger.core.borrow().descriptor, transaction.identity());
        transaction.stage_call(&evidence.request, &[owner]).unwrap();
        transaction.call_commit().unwrap();
        transaction
            .execute_response_storage_mut()
            .copy_from_slice(&evidence.response_storage);
        let frame_bytes = evidence.frame.encode();
        transaction
            .frame_storage_mut()
            .copy_from_slice(&frame_bytes);
        let candidate_bytes = evidence.candidate.encode();
        transaction
            .candidate_storage_mut()
            .copy_from_slice(&candidate_bytes);
        let identity = transaction.identity();
        let request_bytes = transaction.request_bytes().to_vec();

        arm_reusable_storage_allocation_failure();
        assert_eq!(
            ExecuteResponse::parse_reusing(
                transaction.execute_response_bytes(),
                &ledger.core.borrow().descriptor,
                Vec::new(),
            ),
            Err(WireError::CapacityMismatch)
        );
        drop(transaction);

        assert!(ledger.is_poisoned());
        assert!(ledger.is_draining());
        assert_eq!(ledger.quarantined_count(), 1);
        let core = ledger.core.borrow();
        assert_eq!(core.active_postcommit, 0);
        let quarantined = core.quarantined.iter().flatten().next().unwrap();
        assert_eq!(quarantined.frame.identity, identity);
        assert_eq!(quarantined.frame.storage.request, request_bytes);
        assert_eq!(
            quarantined.frame.storage.execute_response,
            evidence.response_storage
        );
        assert_eq!(quarantined.frame.storage.frame, frame_bytes);
        assert_eq!(quarantined.frame.storage.candidate, candidate_bytes);
        assert_eq!(
            core.authoritative_owners[0].state,
            AuthoritativeOwnerState::Quarantined
        );
        assert!(
            drops.borrow().is_empty(),
            "leaf pin must remain quarantined"
        );
    }

    #[test]
    fn failed_postcommit_evidence_is_absorbed_and_never_retried() {
        let (ledger, _) = ledger();
        let handles = owners(&ledger);
        let mut reserved = ledger.reserve().unwrap();
        let mut evidence = abort_evidence(&ledger.core.borrow().descriptor, reserved.identity());
        reserved.stage_call(&evidence.request, &handles).unwrap();
        reserved.call_commit().unwrap();
        reserved.decision_commit(&evidence.decision).unwrap();
        reserved.provider_settled().unwrap();
        evidence.candidate.frame_digest[0] ^= 1;
        assert!(matches!(
            reserved.receipt_commit(ReceiptCommitEvidence {
                request: &evidence.request,
                execute_return_code: 9,
                response_storage: ResponseStorageEvidence::External(&evidence.response_storage),
                response: None,
                frame: &evidence.frame,
                decision: &evidence.decision,
                actions: &evidence.actions,
                candidate: &evidence.candidate,
            }),
            Err(SettlementLedgerError::Wire(_))
        ));
        assert!(ledger.is_poisoned());
        assert!(ledger.is_draining());
        assert_eq!(ledger.quarantined_count(), 1);
        assert_eq!(
            ledger.reserve().err(),
            Some(SettlementLedgerError::Poisoned)
        );
    }
}
