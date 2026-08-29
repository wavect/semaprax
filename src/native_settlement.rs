//! Target-neutral model for callable-v3 recovery settlement.
//!
//! This module deliberately contains no loader, FFI, host-ledger, or codegen
//! integration. It fixes the bounded state machine and independently
//! authenticatable evidence that those later layers must preserve. Ordinary
//! native resource emission remains blocked by `SPX-B104`.
//!
//! `NativeSettlementFrame` is non-cloneable, but this pure model is not an
//! invocation-reservation authority: test-only snapshot preparation can create
//! equal model states. Production proof consumers can prepare only the sole
//! post-commit start and walk authenticated progress edges. A future host
//! ledger must still bind one frame generation to one exact module instance
//! and reject a duplicate invocation before ownership commit.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "callable-v3 settlement remains private proof scaffolding"
)]

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;

use sha2::{Digest, Sha256};

use crate::diagnostic::quote_json;
use crate::hir::DeclarationId;

pub const NATIVE_SETTLEMENT_CERTIFICATE_V2: &str = "semaprax.native-settlement-certificate.v2";
pub const NATIVE_SETTLEMENT_RECEIPT_V2: &str = "semaprax.native-settlement-receipt.v2";

pub const MAX_SETTLEMENT_RESOURCES: usize = 4_096;
pub const MAX_SETTLEMENT_CHECKPOINTS: usize = 65_536;
const MAX_SETTLEMENT_WORK_UNITS: usize = 1_000_000;
const CERTIFICATE_FINGERPRINT_DOMAIN: &[u8] =
    b"semaprax.native-settlement-certificate-fingerprint.v2\0";
const RECEIPT_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-settlement-receipt-fingerprint.v2\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettlementResourceState {
    Live,
    ProvisionalResult,
    Finalizing,
    Dead,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettlementOutcome {
    ScalarSuccess,
    SemanticFailure,
    OwnedSuccess { owner_ordinal: u32 },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdapterAbortReason {
    PhysicalResult(u32),
    MalformedResponse,
    TraceRejected,
    HostUnwind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettlementDecision {
    Accept(SettlementOutcome),
    Abort(AdapterAbortReason),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettlementAction {
    Finalize { owner_ordinal: u32 },
    Publish { owner_ordinal: u32 },
}

/// One authenticated transition between dense recovery checkpoints.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettlementProgressAction {
    Finalize { owner_ordinal: u32 },
    StageOwnedResult { owner_ordinal: u32 },
    CertifyOutcome { trace_evidence: [u8; 32] },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SettlementProgressEdge {
    from: u32,
    to: u32,
    action: SettlementProgressAction,
}

impl SettlementProgressEdge {
    #[must_use]
    pub const fn new(from: u32, to: u32, action: SettlementProgressAction) -> Self {
        Self { from, to, action }
    }

    #[must_use]
    pub const fn from(&self) -> u32 {
        self.from
    }

    #[must_use]
    pub const fn to(&self) -> u32 {
        self.to
    }

    #[must_use]
    pub const fn action(&self) -> SettlementProgressAction {
        self.action
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SettlementDisposition {
    Dead,
    Published,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementCheckpointSpec {
    checkpoint: u32,
    resources: Vec<SettlementResourceState>,
    normal_outcome: Option<SettlementOutcome>,
    abort_cleanup_order: Vec<u32>,
    accept_cleanup_order: Vec<u32>,
}

impl SettlementCheckpointSpec {
    #[must_use]
    pub fn new(
        checkpoint: u32,
        resources: Vec<SettlementResourceState>,
        normal_outcome: Option<SettlementOutcome>,
        abort_cleanup_order: Vec<u32>,
        accept_cleanup_order: Vec<u32>,
    ) -> Self {
        Self {
            checkpoint,
            resources,
            normal_outcome,
            abort_cleanup_order,
            accept_cleanup_order,
        }
    }

    #[must_use]
    pub const fn checkpoint(&self) -> u32 {
        self.checkpoint
    }

    #[must_use]
    pub fn resources(&self) -> &[SettlementResourceState] {
        &self.resources
    }

    #[must_use]
    pub const fn normal_outcome(&self) -> Option<SettlementOutcome> {
        self.normal_outcome
    }

    #[must_use]
    pub fn abort_cleanup_order(&self) -> &[u32] {
        &self.abort_cleanup_order
    }

    #[must_use]
    pub fn accept_cleanup_order(&self) -> &[u32] {
        &self.accept_cleanup_order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSettlementCertificate {
    schema: &'static str,
    function: DeclarationId,
    recovery_contract: [u8; 32],
    resource_count: usize,
    checkpoints: Vec<SettlementCheckpointSpec>,
    start_checkpoints: Vec<u32>,
    progress_edges: Vec<SettlementProgressEdge>,
}

impl NativeSettlementCertificate {
    /// Test-only independent checkpoint snapshots for exhaustive state-model
    /// enumeration. Compiler derivation must use `try_new_with_progress`.
    #[cfg(test)]
    pub fn try_new(
        function: DeclarationId,
        recovery_contract: [u8; 32],
        resource_count: usize,
        checkpoints: Vec<SettlementCheckpointSpec>,
    ) -> Result<Self, SettlementError> {
        let start_checkpoints = (1..=checkpoints.len())
            .map(u32::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| SettlementError::CheckpointCountOutOfBounds)?;
        Self::try_new_with_progress(
            function,
            recovery_contract,
            resource_count,
            checkpoints,
            start_checkpoints,
            Vec::new(),
        )
    }

    pub fn try_new_with_progress(
        function: DeclarationId,
        recovery_contract: [u8; 32],
        resource_count: usize,
        checkpoints: Vec<SettlementCheckpointSpec>,
        start_checkpoints: Vec<u32>,
        progress_edges: Vec<SettlementProgressEdge>,
    ) -> Result<Self, SettlementError> {
        if function.as_str().is_empty() || function.as_str().as_bytes().contains(&0) {
            return Err(SettlementError::InvalidFunctionIdentity);
        }
        if recovery_contract.iter().all(|byte| *byte == 0) {
            return Err(SettlementError::ZeroRecoveryContract);
        }
        if resource_count == 0 || resource_count > MAX_SETTLEMENT_RESOURCES {
            return Err(SettlementError::ResourceCountOutOfBounds);
        }
        if checkpoints.is_empty() || checkpoints.len() > MAX_SETTLEMENT_CHECKPOINTS {
            return Err(SettlementError::CheckpointCountOutOfBounds);
        }
        let work = resource_count
            .checked_mul(checkpoints.len())
            .ok_or(SettlementError::WorkBudgetExceeded)?;
        if work > MAX_SETTLEMENT_WORK_UNITS {
            return Err(SettlementError::WorkBudgetExceeded);
        }
        let independent_snapshots =
            progress_edges.is_empty() && start_checkpoints.len() == checkpoints.len();
        if !independent_snapshots {
            let progress_work = work
                .checked_add(start_checkpoints.len())
                .and_then(|value| value.checked_add(progress_edges.len()))
                .ok_or(SettlementError::WorkBudgetExceeded)?;
            if progress_work > MAX_SETTLEMENT_WORK_UNITS {
                return Err(SettlementError::WorkBudgetExceeded);
            }
        }
        for (index, checkpoint) in checkpoints.iter().enumerate() {
            let expected = u32::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or(SettlementError::CheckpointCountOutOfBounds)?;
            if checkpoint.checkpoint != expected {
                return Err(SettlementError::NonCanonicalCheckpoint);
            }
            validate_checkpoint(checkpoint, resource_count)?;
        }
        validate_progress(&checkpoints, &start_checkpoints, &progress_edges)?;
        Ok(Self {
            schema: NATIVE_SETTLEMENT_CERTIFICATE_V2,
            function,
            recovery_contract,
            resource_count,
            checkpoints,
            start_checkpoints,
            progress_edges,
        })
    }

    #[must_use]
    pub fn function(&self) -> &DeclarationId {
        &self.function
    }

    #[must_use]
    pub const fn recovery_contract(&self) -> [u8; 32] {
        self.recovery_contract
    }

    #[must_use]
    pub const fn resource_count(&self) -> usize {
        self.resource_count
    }

    #[must_use]
    pub fn checkpoints(&self) -> &[SettlementCheckpointSpec] {
        &self.checkpoints
    }

    #[must_use]
    pub fn start_checkpoints(&self) -> &[u32] {
        &self.start_checkpoints
    }

    #[must_use]
    pub fn progress_edges(&self) -> &[SettlementProgressEdge] {
        &self.progress_edges
    }

    #[must_use]
    pub fn canonical_json(&self) -> String {
        let checkpoints = self
            .checkpoints
            .iter()
            .map(checkpoint_json)
            .collect::<Vec<_>>()
            .join(",");
        let starts = self
            .start_checkpoints
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let progress = self
            .progress_edges
            .iter()
            .map(progress_edge_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"function\":{},\"recovery_contract\":\"{}\",\"resource_count\":{},\"checkpoints\":[{}],\"start_checkpoints\":[{}],\"progress_edges\":[{}]}}",
            quote_json(self.schema),
            quote_json(self.function.as_str()),
            hex(&self.recovery_contract),
            self.resource_count,
            checkpoints,
            starts,
            progress,
        )
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(
            CERTIFICATE_FINGERPRINT_DOMAIN,
            self.canonical_json().as_bytes(),
        )
    }

    /// Construct a deterministic snapshot frame for one checkpoint.
    ///
    /// This method does not reserve the invocation or establish process-local
    /// uniqueness. Runtime wiring must perform that linear reservation before
    /// exposing a frame to physical settlement.
    #[cfg(test)]
    pub fn prepare_frame(
        &self,
        invocation: NonZeroU64,
        checkpoint: u32,
    ) -> Result<NativeSettlementFrame, SettlementError> {
        let checkpoint = self.checkpoint(checkpoint)?;
        Ok(NativeSettlementFrame {
            function: self.function.clone(),
            recovery_contract: self.recovery_contract,
            certificate_fingerprint: self.fingerprint(),
            invocation,
            checkpoint: checkpoint.checkpoint,
            resources: checkpoint.resources.clone(),
            terminal: None,
        })
    }

    /// Prepare the sole authenticated post-commit start checkpoint.
    pub fn prepare_start_frame(
        &self,
        invocation: NonZeroU64,
    ) -> Result<NativeSettlementFrame, SettlementError> {
        if self.start_checkpoints.as_slice() != [1] {
            return Err(SettlementError::InvalidProgressStart);
        }
        let checkpoint = self.checkpoint(1)?;
        Ok(NativeSettlementFrame {
            function: self.function.clone(),
            recovery_contract: self.recovery_contract,
            certificate_fingerprint: self.fingerprint(),
            invocation,
            checkpoint: 1,
            resources: checkpoint.resources.clone(),
            terminal: None,
        })
    }

    /// Prepare the private, linear phase model at the sole authenticated start.
    ///
    /// This proof model allocates `Vec` storage and therefore does not establish
    /// the future provider's post-`CallCommit` allocation-free obligation.
    pub fn prepare_start_transaction(
        &self,
        invocation: NonZeroU64,
    ) -> Result<NativeSettlementTransaction, SettlementError> {
        let frame = self.prepare_start_frame(invocation)?;
        Ok(NativeSettlementTransaction {
            function: frame.function,
            recovery_contract: frame.recovery_contract,
            certificate_fingerprint: frame.certificate_fingerprint,
            invocation: frame.invocation,
            checkpoint: frame.checkpoint,
            resources: frame.resources,
            phase: SettlementTransactionState::Executing,
            actions: Vec::new(),
            next_action: 0,
            candidate_receipt: None,
            committed_receipt: None,
        })
    }

    /// Advance execution before a decision has been irreversibly selected.
    pub fn advance_transaction(
        &self,
        transaction: &mut NativeSettlementTransaction,
        action: SettlementProgressAction,
    ) -> Result<(), SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        if matches!(
            transaction.phase,
            SettlementTransactionState::Quarantined { .. }
        ) {
            return Err(SettlementError::TransactionQuarantined);
        }
        if !matches!(transaction.phase, SettlementTransactionState::Executing) {
            let locked = transaction.locked_decision();
            transaction.quarantine(locked);
            return Err(SettlementError::InvalidSettlementPhase);
        }
        let current = match self.checkpoint(transaction.checkpoint) {
            Ok(current) => current,
            Err(error) => {
                transaction.quarantine(None);
                return Err(error);
            }
        };
        if transaction.resources != current.resources {
            transaction.quarantine(None);
            return Err(SettlementError::FrameStateMismatch);
        }
        let mut matches = self
            .progress_edges
            .iter()
            .filter(|edge| edge.from == transaction.checkpoint && edge.action == action);
        let Some(edge) = matches.next().copied() else {
            transaction.quarantine(None);
            return Err(SettlementError::ProgressActionNotAdmitted);
        };
        if matches.next().is_some() {
            transaction.quarantine(None);
            return Err(SettlementError::NonCanonicalProgressEdge);
        }
        let destination = match self.checkpoint(edge.to) {
            Ok(destination) => destination,
            Err(error) => {
                transaction.quarantine(None);
                return Err(error);
            }
        };
        transaction.checkpoint = edge.to;
        transaction.resources.clone_from(&destination.resources);
        Ok(())
    }

    /// Lock exactly one decision. A repeated identical lock is effect-free;
    /// any conflict monotonically quarantines the transaction.
    pub fn lock_transaction_decision(
        &self,
        transaction: &mut NativeSettlementTransaction,
        decision: SettlementDecision,
    ) -> Result<(), SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        if matches!(
            transaction.phase,
            SettlementTransactionState::Quarantined { .. }
        ) {
            return Err(SettlementError::TransactionQuarantined);
        }
        validate_decision(decision)?;
        match transaction.phase {
            SettlementTransactionState::Executing => {}
            SettlementTransactionState::DecisionLocked { decision: locked } => {
                if locked == decision {
                    return Ok(());
                }
                transaction.quarantine(Some(locked));
                return Err(SettlementError::ConflictingLockedDecision);
            }
            SettlementTransactionState::ActionInProgress {
                decision: locked, ..
            } => {
                transaction.quarantine(Some(locked));
                if locked == decision {
                    return Err(SettlementError::InvalidSettlementPhase);
                }
                return Err(SettlementError::ConflictingLockedDecision);
            }
            SettlementTransactionState::ProviderSettled { decision: locked }
            | SettlementTransactionState::ReceiptCommitted { decision: locked } => {
                if locked == decision {
                    return Err(SettlementError::InvalidSettlementPhase);
                }
                transaction.quarantine(Some(locked));
                return Err(SettlementError::ConflictingLockedDecision);
            }
            SettlementTransactionState::Quarantined { .. } => unreachable!("checked above"),
        }
        let checkpoint = self.checkpoint(transaction.checkpoint)?;
        if transaction.resources != checkpoint.resources {
            transaction.quarantine(None);
            return Err(SettlementError::FrameStateMismatch);
        }
        let actions = actions_for(checkpoint, decision)?;
        transaction.actions = actions;
        transaction.next_action = 0;
        transaction.phase = SettlementTransactionState::DecisionLocked { decision };
        Ok(())
    }

    /// Convert a host unwind into a sticky decision, or quarantine when an
    /// in-progress physical finalizer makes completion uncertain.
    pub fn observe_transaction_unwind(
        &self,
        transaction: &mut NativeSettlementTransaction,
    ) -> Result<SettlementDecision, SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        if matches!(
            transaction.phase,
            SettlementTransactionState::Quarantined { .. }
        ) {
            return Err(SettlementError::TransactionQuarantined);
        }
        if matches!(
            transaction.phase,
            SettlementTransactionState::ActionInProgress { .. }
        ) {
            let locked = transaction.locked_decision();
            transaction.quarantine(locked);
            return Err(SettlementError::FinalizerCompletionUncertain);
        }
        if let Some(decision) = transaction.locked_decision() {
            return Ok(decision);
        }
        let decision = SettlementDecision::Abort(AdapterAbortReason::HostUnwind);
        self.lock_transaction_decision(transaction, decision)?;
        Ok(decision)
    }

    /// Mark the next finalizer as in progress before any physical effect.
    pub fn begin_next_finalizer(
        &self,
        transaction: &mut NativeSettlementTransaction,
    ) -> Result<SettlementFinalizerTicket, SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        let decision = match transaction.phase {
            SettlementTransactionState::DecisionLocked { decision } => decision,
            SettlementTransactionState::ActionInProgress { decision, .. } => {
                transaction.quarantine(Some(decision));
                return Err(SettlementError::FinalizerCompletionUncertain);
            }
            SettlementTransactionState::Quarantined { .. } => {
                return Err(SettlementError::TransactionQuarantined);
            }
            _ => return Err(SettlementError::InvalidSettlementPhase),
        };
        let action_index = transaction.next_action;
        let Some(SettlementAction::Finalize { owner_ordinal }) =
            transaction.actions.get(action_index).copied()
        else {
            return Err(SettlementError::FinalizerActionNotPending);
        };
        let owner = match usize::try_from(owner_ordinal) {
            Ok(owner) => owner,
            Err(_) => {
                transaction.quarantine(Some(decision));
                return Err(SettlementError::InvalidStateTransition);
            }
        };
        if owner >= transaction.resources.len() {
            transaction.quarantine(Some(decision));
            return Err(SettlementError::InvalidStateTransition);
        }
        let state = transaction
            .resources
            .get_mut(owner)
            .expect("owner bound was checked");
        if !matches!(
            state,
            SettlementResourceState::Live | SettlementResourceState::ProvisionalResult
        ) {
            transaction.quarantine(Some(decision));
            return Err(SettlementError::InvalidStateTransition);
        }
        *state = SettlementResourceState::Finalizing;
        transaction.phase = SettlementTransactionState::ActionInProgress {
            decision,
            action_index,
            owner_ordinal,
        };
        Ok(SettlementFinalizerTicket {
            certificate_fingerprint: transaction.certificate_fingerprint,
            invocation: transaction.invocation,
            checkpoint: transaction.checkpoint,
            decision,
            action_index,
            owner_ordinal,
        })
    }

    /// Record certain completion of the exact in-progress finalizer.
    pub fn complete_finalizer(
        &self,
        transaction: &mut NativeSettlementTransaction,
        ticket: SettlementFinalizerTicket,
    ) -> Result<(), SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        let expected = match transaction.phase {
            SettlementTransactionState::ActionInProgress {
                decision,
                action_index,
                owner_ordinal,
            } => (decision, action_index, owner_ordinal),
            SettlementTransactionState::Quarantined { .. } => {
                return Err(SettlementError::TransactionQuarantined);
            }
            _ => {
                let locked = transaction.locked_decision();
                transaction.quarantine(locked);
                return Err(SettlementError::InvalidSettlementPhase);
            }
        };
        if ticket.certificate_fingerprint != transaction.certificate_fingerprint
            || ticket.invocation != transaction.invocation
            || ticket.checkpoint != transaction.checkpoint
            || (ticket.decision, ticket.action_index, ticket.owner_ordinal) != expected
        {
            transaction.quarantine(Some(expected.0));
            return Err(SettlementError::FinalizerTicketMismatch);
        }
        let owner = match usize::try_from(expected.2) {
            Ok(owner) => owner,
            Err(_) => {
                transaction.quarantine(Some(expected.0));
                return Err(SettlementError::InvalidStateTransition);
            }
        };
        if owner >= transaction.resources.len() {
            transaction.quarantine(Some(expected.0));
            return Err(SettlementError::InvalidStateTransition);
        }
        let state = transaction
            .resources
            .get_mut(owner)
            .expect("owner bound was checked");
        if *state != SettlementResourceState::Finalizing {
            transaction.quarantine(Some(expected.0));
            return Err(SettlementError::InvalidStateTransition);
        }
        *state = SettlementResourceState::Dead;
        transaction.next_action += 1;
        transaction.phase = SettlementTransactionState::DecisionLocked {
            decision: expected.0,
        };
        Ok(())
    }

    /// Quarantine an in-progress finalizer whose physical completion is not
    /// known. No retry transition exists from quarantine.
    pub fn mark_finalizer_uncertain(
        &self,
        transaction: &mut NativeSettlementTransaction,
    ) -> Result<(), SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        if matches!(
            transaction.phase,
            SettlementTransactionState::Quarantined { .. }
        ) {
            return Err(SettlementError::TransactionQuarantined);
        }
        let SettlementTransactionState::ActionInProgress { decision, .. } = transaction.phase
        else {
            return Err(SettlementError::InvalidSettlementPhase);
        };
        transaction.quarantine(Some(decision));
        Ok(())
    }

    /// Turn the final provisional result into provider-side candidate evidence.
    /// This is not publication in a host ownership ledger.
    pub fn publish_owned_candidate(
        &self,
        transaction: &mut NativeSettlementTransaction,
    ) -> Result<(), SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        let decision = match transaction.phase {
            SettlementTransactionState::DecisionLocked { decision } => decision,
            SettlementTransactionState::ActionInProgress { decision, .. } => {
                transaction.quarantine(Some(decision));
                return Err(SettlementError::FinalizerCompletionUncertain);
            }
            SettlementTransactionState::Quarantined { .. } => {
                return Err(SettlementError::TransactionQuarantined);
            }
            _ => return Err(SettlementError::InvalidSettlementPhase),
        };
        let Some(SettlementAction::Publish { owner_ordinal }) =
            transaction.actions.get(transaction.next_action).copied()
        else {
            transaction.quarantine(Some(decision));
            return Err(SettlementError::PublishActionNotPending);
        };
        if transaction.next_action + 1 != transaction.actions.len()
            || transaction.resources.iter().any(|state| {
                matches!(
                    state,
                    SettlementResourceState::Live | SettlementResourceState::Finalizing
                )
            })
        {
            transaction.quarantine(Some(decision));
            return Err(SettlementError::InvalidStateTransition);
        }
        let owner = match usize::try_from(owner_ordinal) {
            Ok(owner) => owner,
            Err(_) => {
                transaction.quarantine(Some(decision));
                return Err(SettlementError::InvalidStateTransition);
            }
        };
        if owner >= transaction.resources.len() {
            transaction.quarantine(Some(decision));
            return Err(SettlementError::InvalidStateTransition);
        }
        let state = transaction
            .resources
            .get_mut(owner)
            .expect("owner bound was checked");
        if *state != SettlementResourceState::ProvisionalResult {
            transaction.quarantine(Some(decision));
            return Err(SettlementError::InvalidStateTransition);
        }
        *state = SettlementResourceState::Published;
        transaction.next_action += 1;
        Ok(())
    }

    /// Produce provider candidate evidence only after every action is complete.
    pub fn finish_provider_settlement(
        &self,
        transaction: &mut NativeSettlementTransaction,
    ) -> Result<SettlementApplication, SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        let decision = match transaction.phase {
            SettlementTransactionState::DecisionLocked { decision } => decision,
            SettlementTransactionState::ActionInProgress { decision, .. } => {
                transaction.quarantine(Some(decision));
                return Err(SettlementError::FinalizerCompletionUncertain);
            }
            SettlementTransactionState::Quarantined { .. } => {
                return Err(SettlementError::TransactionQuarantined);
            }
            _ => return Err(SettlementError::InvalidSettlementPhase),
        };
        if transaction.next_action != transaction.actions.len()
            || transaction
                .resources
                .contains(&SettlementResourceState::Finalizing)
        {
            transaction.quarantine(Some(decision));
            return Err(SettlementError::SettlementActionsIncomplete);
        }
        let receipt = NativeSettlementReceipt {
            schema: NATIVE_SETTLEMENT_RECEIPT_V2,
            function: self.function.clone(),
            recovery_contract: self.recovery_contract,
            certificate_fingerprint: self.fingerprint(),
            invocation: transaction.invocation,
            checkpoint: transaction.checkpoint,
            decision,
            actions: transaction.actions.clone(),
            dispositions: match terminal_dispositions(&transaction.resources) {
                Ok(dispositions) => dispositions,
                Err(error) => {
                    transaction.quarantine(Some(decision));
                    return Err(error);
                }
            },
            active_finalizers: 0,
        };
        if let Err(error) = self.validate_receipt(transaction.invocation, &receipt) {
            transaction.quarantine(Some(decision));
            return Err(error);
        }
        transaction.candidate_receipt = Some(receipt.clone());
        transaction.phase = SettlementTransactionState::ProviderSettled { decision };
        Ok(SettlementApplication {
            receipt,
            performed_actions: Vec::new(),
        })
    }

    /// Validate the exact provider candidate as terminal model evidence and
    /// receipt-commit eligibility. This model has no host authentication or
    /// ownership-ledger authority.
    pub fn commit_provider_receipt(
        &self,
        transaction: &mut NativeSettlementTransaction,
        candidate: &NativeSettlementReceipt,
    ) -> Result<SettlementApplication, SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        match transaction.phase {
            SettlementTransactionState::ReceiptCommitted { decision } => {
                let Some(committed) = transaction.committed_receipt.as_ref() else {
                    transaction.quarantine(Some(decision));
                    return Err(SettlementError::FrameStateMismatch);
                };
                if candidate == committed && candidate.decision == decision {
                    return Ok(SettlementApplication {
                        receipt: committed.clone(),
                        performed_actions: Vec::new(),
                    });
                }
                transaction.quarantine(Some(decision));
                Err(SettlementError::ReceiptCommitMismatch)
            }
            SettlementTransactionState::ProviderSettled { decision } => {
                let exact = transaction
                    .candidate_receipt
                    .as_ref()
                    .is_some_and(|receipt| receipt == candidate);
                if !exact
                    || self
                        .validate_receipt(transaction.invocation, candidate)
                        .is_err()
                {
                    transaction.quarantine(Some(decision));
                    return Err(SettlementError::ReceiptCommitMismatch);
                }
                transaction.committed_receipt = Some(candidate.clone());
                transaction.phase = SettlementTransactionState::ReceiptCommitted { decision };
                Ok(SettlementApplication {
                    receipt: candidate.clone(),
                    performed_actions: Vec::new(),
                })
            }
            SettlementTransactionState::Quarantined { .. } => {
                Err(SettlementError::TransactionQuarantined)
            }
            _ => {
                let locked = transaction.locked_decision();
                transaction.quarantine(locked);
                Err(SettlementError::InvalidSettlementPhase)
            }
        }
    }

    /// Return byte-identical provider candidate evidence for the same decision.
    /// This does not commit a receipt or represent any action as newly performed.
    pub fn replay_provider_candidate(
        &self,
        transaction: &mut NativeSettlementTransaction,
        decision: SettlementDecision,
    ) -> Result<SettlementApplication, SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        if matches!(
            transaction.phase,
            SettlementTransactionState::Quarantined { .. }
        ) {
            return Err(SettlementError::TransactionQuarantined);
        }
        validate_decision(decision)?;
        match transaction.phase {
            SettlementTransactionState::ProviderSettled { decision: locked } => {
                if locked != decision {
                    transaction.quarantine(Some(locked));
                    return Err(SettlementError::ConflictingLockedDecision);
                }
                let receipt = transaction.candidate_receipt.as_ref();
                let Some(receipt) = receipt else {
                    transaction.quarantine(Some(locked));
                    return Err(SettlementError::FrameStateMismatch);
                };
                Ok(SettlementApplication {
                    receipt: receipt.clone(),
                    performed_actions: Vec::new(),
                })
            }
            SettlementTransactionState::Quarantined { .. } => unreachable!("checked above"),
            _ => Err(SettlementError::InvalidSettlementPhase),
        }
    }

    /// Return byte-identical model receipt-commit evidence for the same decision;
    /// this is not a host-authenticated ledger receipt.
    /// No provider action is represented as newly performed by this replay.
    pub fn replay_committed_receipt(
        &self,
        transaction: &mut NativeSettlementTransaction,
        decision: SettlementDecision,
    ) -> Result<SettlementApplication, SettlementError> {
        self.authenticate_transaction_or_quarantine(transaction)?;
        if matches!(
            transaction.phase,
            SettlementTransactionState::Quarantined { .. }
        ) {
            return Err(SettlementError::TransactionQuarantined);
        }
        validate_decision(decision)?;
        match transaction.phase {
            SettlementTransactionState::ReceiptCommitted { decision: locked } => {
                if locked != decision {
                    transaction.quarantine(Some(locked));
                    return Err(SettlementError::ConflictingLockedDecision);
                }
                let Some(receipt) = transaction.committed_receipt.as_ref() else {
                    transaction.quarantine(Some(locked));
                    return Err(SettlementError::FrameStateMismatch);
                };
                Ok(SettlementApplication {
                    receipt: receipt.clone(),
                    performed_actions: Vec::new(),
                })
            }
            SettlementTransactionState::Quarantined { .. } => unreachable!("checked above"),
            _ => Err(SettlementError::InvalidSettlementPhase),
        }
    }

    /// Advance one exact certified progress edge without mutating on failure.
    pub fn advance_frame(
        &self,
        frame: &mut NativeSettlementFrame,
        action: SettlementProgressAction,
    ) -> Result<(), SettlementError> {
        self.authenticate_frame(frame)?;
        if frame.terminal.is_some() {
            return Err(SettlementError::ProgressActionNotAdmitted);
        }
        let mut matches = self
            .progress_edges
            .iter()
            .filter(|edge| edge.from == frame.checkpoint && edge.action == action);
        let edge = matches
            .next()
            .ok_or(SettlementError::ProgressActionNotAdmitted)?;
        if matches.next().is_some() {
            return Err(SettlementError::NonCanonicalProgressEdge);
        }
        let destination = self.checkpoint(edge.to)?;
        frame.checkpoint = edge.to;
        frame.resources.clone_from(&destination.resources);
        Ok(())
    }

    pub fn settle(
        &self,
        frame: &mut NativeSettlementFrame,
        decision: SettlementDecision,
    ) -> Result<SettlementApplication, SettlementError> {
        validate_decision(decision)?;
        self.authenticate_frame(frame)?;
        if let Some(terminal) = &frame.terminal {
            if terminal.decision != decision {
                return Err(SettlementError::ConflictingTerminalDecision);
            }
            self.validate_receipt(frame.invocation, &terminal.receipt)?;
            if terminal_dispositions(&frame.resources)? != terminal.receipt.dispositions {
                return Err(SettlementError::FrameStateMismatch);
            }
            return Ok(SettlementApplication {
                receipt: terminal.receipt.clone(),
                performed_actions: Vec::new(),
            });
        }

        let checkpoint = self.checkpoint(frame.checkpoint)?;
        if frame.resources != checkpoint.resources {
            return Err(SettlementError::FrameStateMismatch);
        }
        let actions = actions_for(checkpoint, decision)?;
        let mut resources = frame.resources.clone();
        apply_actions(&mut resources, &actions)?;
        let dispositions = terminal_dispositions(&resources)?;
        let receipt = NativeSettlementReceipt {
            schema: NATIVE_SETTLEMENT_RECEIPT_V2,
            function: self.function.clone(),
            recovery_contract: self.recovery_contract,
            certificate_fingerprint: self.fingerprint(),
            invocation: frame.invocation,
            checkpoint: frame.checkpoint,
            decision,
            actions: actions.clone(),
            dispositions,
            active_finalizers: 0,
        };
        self.validate_receipt(frame.invocation, &receipt)?;

        frame.resources = resources;
        frame.terminal = Some(TerminalSettlement {
            decision,
            receipt: receipt.clone(),
        });
        Ok(SettlementApplication {
            receipt,
            performed_actions: actions,
        })
    }

    pub fn validate_receipt(
        &self,
        expected_invocation: NonZeroU64,
        receipt: &NativeSettlementReceipt,
    ) -> Result<(), SettlementError> {
        if receipt.schema != NATIVE_SETTLEMENT_RECEIPT_V2 {
            return Err(SettlementError::ReceiptSchemaMismatch);
        }
        if receipt.function != self.function
            || receipt.recovery_contract != self.recovery_contract
            || receipt.certificate_fingerprint != self.fingerprint()
            || receipt.invocation != expected_invocation
        {
            return Err(SettlementError::ReceiptBindingMismatch);
        }
        validate_decision(receipt.decision)?;
        if receipt.active_finalizers != 0 {
            return Err(SettlementError::NotQuiescent);
        }
        let checkpoint = self.checkpoint(receipt.checkpoint)?;
        let expected_actions = actions_for(checkpoint, receipt.decision)?;
        if receipt.actions != expected_actions {
            return Err(SettlementError::ReceiptActionMismatch);
        }
        let mut resources = checkpoint.resources.clone();
        apply_actions(&mut resources, &expected_actions)?;
        if receipt.dispositions != terminal_dispositions(&resources)? {
            return Err(SettlementError::ReceiptDispositionMismatch);
        }
        Ok(())
    }

    fn authenticate_frame(&self, frame: &NativeSettlementFrame) -> Result<(), SettlementError> {
        if frame.function != self.function
            || frame.recovery_contract != self.recovery_contract
            || frame.certificate_fingerprint != self.fingerprint()
        {
            return Err(SettlementError::FrameBindingMismatch);
        }
        if frame.resources.len() != self.resource_count {
            return Err(SettlementError::FrameStateMismatch);
        }
        Ok(())
    }

    fn authenticate_transaction(
        &self,
        transaction: &NativeSettlementTransaction,
    ) -> Result<(), SettlementError> {
        if transaction.function != self.function
            || transaction.recovery_contract != self.recovery_contract
            || transaction.certificate_fingerprint != self.fingerprint()
            || transaction.resources.len() != self.resource_count
        {
            return Err(SettlementError::FrameBindingMismatch);
        }
        self.validate_transaction_state(transaction)
    }

    fn validate_transaction_state(
        &self,
        transaction: &NativeSettlementTransaction,
    ) -> Result<(), SettlementError> {
        if matches!(
            transaction.phase,
            SettlementTransactionState::Quarantined { .. }
        ) {
            return Ok(());
        }
        let checkpoint = self.checkpoint(transaction.checkpoint)?;
        match transaction.phase {
            SettlementTransactionState::Executing => {
                if transaction.resources != checkpoint.resources
                    || !transaction.actions.is_empty()
                    || transaction.next_action != 0
                    || transaction.candidate_receipt.is_some()
                    || transaction.committed_receipt.is_some()
                {
                    return Err(SettlementError::FrameStateMismatch);
                }
            }
            SettlementTransactionState::DecisionLocked { decision } => {
                self.validate_locked_transaction_state(transaction, checkpoint, decision, None)?;
            }
            SettlementTransactionState::ActionInProgress {
                decision,
                action_index,
                owner_ordinal,
            } => {
                self.validate_locked_transaction_state(
                    transaction,
                    checkpoint,
                    decision,
                    Some((action_index, owner_ordinal)),
                )?;
            }
            SettlementTransactionState::ProviderSettled { decision } => {
                self.validate_settled_transaction_state(transaction, checkpoint, decision, false)?;
            }
            SettlementTransactionState::ReceiptCommitted { decision } => {
                self.validate_settled_transaction_state(transaction, checkpoint, decision, true)?;
            }
            SettlementTransactionState::Quarantined { .. } => unreachable!("checked above"),
        }
        Ok(())
    }

    fn validate_locked_transaction_state(
        &self,
        transaction: &NativeSettlementTransaction,
        checkpoint: &SettlementCheckpointSpec,
        decision: SettlementDecision,
        in_progress: Option<(usize, u32)>,
    ) -> Result<(), SettlementError> {
        let expected_actions = actions_for(checkpoint, decision)?;
        if transaction.actions != expected_actions
            || transaction.next_action > transaction.actions.len()
            || transaction.candidate_receipt.is_some()
            || transaction.committed_receipt.is_some()
        {
            return Err(SettlementError::FrameStateMismatch);
        }
        let mut expected_resources = checkpoint.resources.clone();
        apply_actions(
            &mut expected_resources,
            &transaction.actions[..transaction.next_action],
        )?;
        if let Some((action_index, owner_ordinal)) = in_progress {
            if action_index != transaction.next_action
                || transaction.actions.get(action_index)
                    != Some(&SettlementAction::Finalize { owner_ordinal })
            {
                return Err(SettlementError::FrameStateMismatch);
            }
            let owner =
                usize::try_from(owner_ordinal).map_err(|_| SettlementError::FrameStateMismatch)?;
            let state = expected_resources
                .get_mut(owner)
                .ok_or(SettlementError::FrameStateMismatch)?;
            if !matches!(
                state,
                SettlementResourceState::Live | SettlementResourceState::ProvisionalResult
            ) {
                return Err(SettlementError::FrameStateMismatch);
            }
            *state = SettlementResourceState::Finalizing;
        }
        if transaction.resources != expected_resources {
            return Err(SettlementError::FrameStateMismatch);
        }
        Ok(())
    }

    fn validate_settled_transaction_state(
        &self,
        transaction: &NativeSettlementTransaction,
        checkpoint: &SettlementCheckpointSpec,
        decision: SettlementDecision,
        committed: bool,
    ) -> Result<(), SettlementError> {
        let expected_actions = actions_for(checkpoint, decision)?;
        let mut expected_resources = checkpoint.resources.clone();
        apply_actions(&mut expected_resources, &expected_actions)?;
        let Some(candidate) = transaction.candidate_receipt.as_ref() else {
            return Err(SettlementError::FrameStateMismatch);
        };
        if transaction.actions != expected_actions
            || transaction.next_action != expected_actions.len()
            || transaction.resources != expected_resources
            || candidate.decision != decision
            || self
                .validate_receipt(transaction.invocation, candidate)
                .is_err()
        {
            return Err(SettlementError::FrameStateMismatch);
        }
        match (committed, transaction.committed_receipt.as_ref()) {
            (false, None) => Ok(()),
            (true, Some(receipt)) if receipt == candidate => Ok(()),
            _ => Err(SettlementError::FrameStateMismatch),
        }
    }

    fn authenticate_transaction_or_quarantine(
        &self,
        transaction: &mut NativeSettlementTransaction,
    ) -> Result<(), SettlementError> {
        if let Err(error) = self.authenticate_transaction(transaction) {
            let locked = transaction.locked_decision();
            transaction.quarantine(locked);
            return Err(error);
        }
        Ok(())
    }

    fn checkpoint(&self, checkpoint: u32) -> Result<&SettlementCheckpointSpec, SettlementError> {
        let index = usize::try_from(checkpoint)
            .ok()
            .and_then(|value| value.checked_sub(1))
            .ok_or(SettlementError::UnknownCheckpoint)?;
        self.checkpoints
            .get(index)
            .filter(|candidate| candidate.checkpoint == checkpoint)
            .ok_or(SettlementError::UnknownCheckpoint)
    }
}

/// Linear state for one future callable-v3 settlement.
///
/// The frame intentionally is not cloneable. This prevents accidental local
/// duplication, but does not make the certificate's deterministic
/// test-only snapshot constructor a uniqueness authority. A runtime integration
/// must additionally bind one frame generation to one exact module instance
/// and committed ledger invocation.
#[derive(Eq, PartialEq)]
pub struct NativeSettlementFrame {
    function: DeclarationId,
    recovery_contract: [u8; 32],
    certificate_fingerprint: [u8; 32],
    invocation: NonZeroU64,
    checkpoint: u32,
    resources: Vec<SettlementResourceState>,
    terminal: Option<TerminalSettlement>,
}

impl NativeSettlementFrame {
    #[must_use]
    pub const fn invocation(&self) -> NonZeroU64 {
        self.invocation
    }

    #[must_use]
    pub const fn checkpoint(&self) -> u32 {
        self.checkpoint
    }

    #[must_use]
    pub fn resources(&self) -> &[SettlementResourceState] {
        &self.resources
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.terminal.is_some()
    }
}

/// Observable phase of the private transaction model. The linear transaction
/// and finalizer ticket remain opaque and non-formatting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementTransactionPhase {
    Executing,
    DecisionLocked,
    ActionInProgress {
        action_index: usize,
        owner_ordinal: u32,
    },
    ProviderSettled,
    ReceiptCommitted,
    Quarantined,
}

#[derive(Eq, PartialEq)]
enum SettlementTransactionState {
    Executing,
    DecisionLocked {
        decision: SettlementDecision,
    },
    ActionInProgress {
        decision: SettlementDecision,
        action_index: usize,
        owner_ordinal: u32,
    },
    ProviderSettled {
        decision: SettlementDecision,
    },
    ReceiptCommitted {
        decision: SettlementDecision,
    },
    Quarantined {
        decision: Option<SettlementDecision>,
    },
}

/// Linear proof state for one phase-aware settlement transaction.
///
/// `Published` in this type means only that provider-side candidate evidence
/// is eligible; a future host ledger remains solely responsible for external
/// ownership publication at receipt commit.
#[derive(Eq, PartialEq)]
pub struct NativeSettlementTransaction {
    function: DeclarationId,
    recovery_contract: [u8; 32],
    certificate_fingerprint: [u8; 32],
    invocation: NonZeroU64,
    checkpoint: u32,
    resources: Vec<SettlementResourceState>,
    phase: SettlementTransactionState,
    actions: Vec<SettlementAction>,
    next_action: usize,
    candidate_receipt: Option<NativeSettlementReceipt>,
    committed_receipt: Option<NativeSettlementReceipt>,
}

impl NativeSettlementTransaction {
    #[must_use]
    pub const fn invocation(&self) -> NonZeroU64 {
        self.invocation
    }

    #[must_use]
    pub const fn checkpoint(&self) -> u32 {
        self.checkpoint
    }

    #[must_use]
    pub fn resources(&self) -> &[SettlementResourceState] {
        &self.resources
    }

    #[must_use]
    pub const fn phase(&self) -> SettlementTransactionPhase {
        match self.phase {
            SettlementTransactionState::Executing => SettlementTransactionPhase::Executing,
            SettlementTransactionState::DecisionLocked { .. } => {
                SettlementTransactionPhase::DecisionLocked
            }
            SettlementTransactionState::ActionInProgress {
                action_index,
                owner_ordinal,
                ..
            } => SettlementTransactionPhase::ActionInProgress {
                action_index,
                owner_ordinal,
            },
            SettlementTransactionState::ProviderSettled { .. } => {
                SettlementTransactionPhase::ProviderSettled
            }
            SettlementTransactionState::ReceiptCommitted { .. } => {
                SettlementTransactionPhase::ReceiptCommitted
            }
            SettlementTransactionState::Quarantined { .. } => {
                SettlementTransactionPhase::Quarantined
            }
        }
    }

    #[must_use]
    pub fn candidate_receipt(&self) -> Option<&NativeSettlementReceipt> {
        self.candidate_receipt.as_ref()
    }

    #[must_use]
    pub fn committed_receipt(&self) -> Option<&NativeSettlementReceipt> {
        self.committed_receipt.as_ref()
    }

    fn locked_decision(&self) -> Option<SettlementDecision> {
        match self.phase {
            SettlementTransactionState::Executing => None,
            SettlementTransactionState::DecisionLocked { decision }
            | SettlementTransactionState::ActionInProgress { decision, .. }
            | SettlementTransactionState::ProviderSettled { decision }
            | SettlementTransactionState::ReceiptCommitted { decision } => Some(decision),
            SettlementTransactionState::Quarantined { decision } => decision,
        }
    }

    fn quarantine(&mut self, decision: Option<SettlementDecision>) {
        self.phase = SettlementTransactionState::Quarantined { decision };
    }
}

/// Opaque evidence that one exact finalizer was marked `Finalizing` before its
/// physical effect. It is intentionally non-cloneable and non-formatting.
#[derive(Eq, PartialEq)]
pub struct SettlementFinalizerTicket {
    certificate_fingerprint: [u8; 32],
    invocation: NonZeroU64,
    checkpoint: u32,
    decision: SettlementDecision,
    action_index: usize,
    owner_ordinal: u32,
}

impl SettlementFinalizerTicket {
    #[must_use]
    pub const fn action(&self) -> SettlementAction {
        SettlementAction::Finalize {
            owner_ordinal: self.owner_ordinal,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalSettlement {
    decision: SettlementDecision,
    receipt: NativeSettlementReceipt,
}

/// Immutable terminal evidence. Cloning a receipt cannot execute settlement
/// actions or recreate a linear frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSettlementReceipt {
    schema: &'static str,
    function: DeclarationId,
    recovery_contract: [u8; 32],
    certificate_fingerprint: [u8; 32],
    invocation: NonZeroU64,
    checkpoint: u32,
    decision: SettlementDecision,
    actions: Vec<SettlementAction>,
    dispositions: Vec<SettlementDisposition>,
    active_finalizers: u32,
}

impl NativeSettlementReceipt {
    #[must_use]
    pub const fn invocation(&self) -> NonZeroU64 {
        self.invocation
    }

    #[must_use]
    pub const fn checkpoint(&self) -> u32 {
        self.checkpoint
    }

    #[must_use]
    pub const fn decision(&self) -> SettlementDecision {
        self.decision
    }

    #[must_use]
    pub fn actions(&self) -> &[SettlementAction] {
        &self.actions
    }

    #[must_use]
    pub fn dispositions(&self) -> &[SettlementDisposition] {
        &self.dispositions
    }

    #[must_use]
    pub const fn active_finalizers(&self) -> u32 {
        self.active_finalizers
    }

    #[must_use]
    pub fn canonical_json(&self) -> String {
        let actions = self
            .actions
            .iter()
            .map(action_json)
            .collect::<Vec<_>>()
            .join(",");
        let dispositions = self
            .dispositions
            .iter()
            .map(|disposition| quote_json(disposition_name(*disposition)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"function\":{},\"recovery_contract\":\"{}\",\"certificate_fingerprint\":\"{}\",\"invocation\":{},\"checkpoint\":{},\"decision\":{},\"actions\":[{}],\"dispositions\":[{}],\"active_finalizers\":{}}}",
            quote_json(self.schema),
            quote_json(self.function.as_str()),
            hex(&self.recovery_contract),
            hex(&self.certificate_fingerprint),
            self.invocation,
            self.checkpoint,
            decision_json(self.decision),
            actions,
            dispositions,
            self.active_finalizers,
        )
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(RECEIPT_FINGERPRINT_DOMAIN, self.canonical_json().as_bytes())
    }
}

/// Immutable model output. `performed_actions` is evidence, not an execution
/// capability; cloning it has no physical effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementApplication {
    receipt: NativeSettlementReceipt,
    performed_actions: Vec<SettlementAction>,
}

impl SettlementApplication {
    #[must_use]
    pub fn receipt(&self) -> &NativeSettlementReceipt {
        &self.receipt
    }

    #[must_use]
    pub fn performed_actions(&self) -> &[SettlementAction] {
        &self.performed_actions
    }

    #[must_use]
    pub fn into_parts(self) -> (NativeSettlementReceipt, Vec<SettlementAction>) {
        (self.receipt, self.performed_actions)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementError {
    InvalidFunctionIdentity,
    ZeroRecoveryContract,
    ResourceCountOutOfBounds,
    CheckpointCountOutOfBounds,
    WorkBudgetExceeded,
    NonCanonicalCheckpoint,
    CheckpointResourceCountMismatch,
    InvalidCheckpointState,
    MultipleProvisionalResults,
    InvalidCleanupOrder,
    InvalidNormalOutcome,
    InvalidProgressStart,
    NonCanonicalProgressEdge,
    InvalidProgressTransition,
    UnreachableCheckpoint,
    ProgressActionNotAdmitted,
    InvalidAbortReason,
    UnknownCheckpoint,
    FrameBindingMismatch,
    FrameStateMismatch,
    DecisionNotAdmitted,
    ConflictingTerminalDecision,
    InvalidStateTransition,
    ReceiptSchemaMismatch,
    ReceiptBindingMismatch,
    ReceiptActionMismatch,
    ReceiptDispositionMismatch,
    NotQuiescent,
    InvalidSettlementPhase,
    ConflictingLockedDecision,
    TransactionQuarantined,
    FinalizerActionNotPending,
    FinalizerTicketMismatch,
    FinalizerCompletionUncertain,
    PublishActionNotPending,
    SettlementActionsIncomplete,
    ReceiptCommitMismatch,
}

impl fmt::Display for SettlementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFunctionIdentity => {
                "settlement function identity is empty or contains NUL"
            }
            Self::ZeroRecoveryContract => "settlement recovery contract is zero",
            Self::ResourceCountOutOfBounds => "settlement resource count is outside bounds",
            Self::CheckpointCountOutOfBounds => "settlement checkpoint count is outside bounds",
            Self::WorkBudgetExceeded => "settlement certificate work budget is exceeded",
            Self::NonCanonicalCheckpoint => "settlement checkpoint identities are not canonical",
            Self::CheckpointResourceCountMismatch => {
                "settlement checkpoint resource count does not match"
            }
            Self::InvalidCheckpointState => "settlement checkpoint contains a terminal state",
            Self::MultipleProvisionalResults => {
                "settlement checkpoint contains multiple provisional results"
            }
            Self::InvalidCleanupOrder => "settlement cleanup order is not an exact permutation",
            Self::InvalidNormalOutcome => "settlement normal outcome disagrees with liveness",
            Self::InvalidProgressStart => "settlement progress start is invalid",
            Self::NonCanonicalProgressEdge => "settlement progress edge is not canonical",
            Self::InvalidProgressTransition => "settlement progress transition is invalid",
            Self::UnreachableCheckpoint => "settlement checkpoint is unreachable",
            Self::ProgressActionNotAdmitted => "settlement progress action is not admitted",
            Self::InvalidAbortReason => "settlement physical abort result must be nonzero",
            Self::UnknownCheckpoint => "settlement checkpoint is unknown",
            Self::FrameBindingMismatch => "settlement frame binding does not match certificate",
            Self::FrameStateMismatch => "settlement frame state does not match checkpoint",
            Self::DecisionNotAdmitted => "settlement decision is not admitted at checkpoint",
            Self::ConflictingTerminalDecision => {
                "settlement frame was already completed with another decision"
            }
            Self::InvalidStateTransition => "settlement action is invalid for resource state",
            Self::ReceiptSchemaMismatch => "settlement receipt schema does not match",
            Self::ReceiptBindingMismatch => "settlement receipt binding does not match",
            Self::ReceiptActionMismatch => "settlement receipt actions do not match certificate",
            Self::ReceiptDispositionMismatch => {
                "settlement receipt dispositions do not match actions"
            }
            Self::NotQuiescent => "settlement receipt is not quiescent",
            Self::InvalidSettlementPhase => "settlement transaction phase does not admit action",
            Self::ConflictingLockedDecision => {
                "settlement transaction decision conflicts with locked decision"
            }
            Self::TransactionQuarantined => "settlement transaction is quarantined",
            Self::FinalizerActionNotPending => "settlement finalizer action is not next",
            Self::FinalizerTicketMismatch => "settlement finalizer ticket does not match",
            Self::FinalizerCompletionUncertain => "settlement finalizer completion is uncertain",
            Self::PublishActionNotPending => "settlement publication action is not next",
            Self::SettlementActionsIncomplete => "settlement actions are incomplete",
            Self::ReceiptCommitMismatch => "settlement receipt commit does not match candidate",
        })
    }
}

impl Error for SettlementError {}

fn validate_progress(
    checkpoints: &[SettlementCheckpointSpec],
    starts: &[u32],
    edges: &[SettlementProgressEdge],
) -> Result<(), SettlementError> {
    let all = (1..=checkpoints.len())
        .map(u32::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SettlementError::CheckpointCountOutOfBounds)?;
    // `try_new` preserves the original proof-model meaning: each supplied row
    // can independently prepare a frame. Compiler-derived certificates use
    // `try_new_with_progress` and therefore take the strict branch below.
    if edges.is_empty() && starts == all {
        return Ok(());
    }
    if starts != [1]
        || checkpoints[0].normal_outcome.is_some()
        || checkpoints[0]
            .resources
            .iter()
            .any(|state| *state != SettlementResourceState::Live)
    {
        return Err(SettlementError::InvalidProgressStart);
    }
    let mut seen_edges = BTreeSet::new();
    let mut seen_actions = BTreeSet::new();
    let mut reachable = BTreeSet::from([1_u32]);
    for edge in edges {
        if edge.from == 0
            || edge.to == 0
            || edge.from >= edge.to
            || edge.to as usize > checkpoints.len()
            || !seen_edges.insert(*edge)
            || !seen_actions.insert((edge.from, edge.action))
            || !reachable.contains(&edge.from)
        {
            return Err(SettlementError::NonCanonicalProgressEdge);
        }
        let from = &checkpoints[(edge.from - 1) as usize];
        let to = &checkpoints[(edge.to - 1) as usize];
        if from.normal_outcome.is_some() || !valid_progress_transition(from, to, edge.action) {
            return Err(SettlementError::InvalidProgressTransition);
        }
        reachable.insert(edge.to);
    }
    if reachable.len() != checkpoints.len() {
        return Err(SettlementError::UnreachableCheckpoint);
    }
    Ok(())
}

fn valid_progress_transition(
    from: &SettlementCheckpointSpec,
    to: &SettlementCheckpointSpec,
    action: SettlementProgressAction,
) -> bool {
    if from.resources.len() != to.resources.len() {
        return false;
    }
    match action {
        SettlementProgressAction::Finalize { owner_ordinal } => {
            if to.normal_outcome.is_some() {
                return false;
            }
            let state_transition = changed_owner_state(
                &from.resources,
                &to.resources,
                owner_ordinal,
                SettlementResourceState::Live,
                SettlementResourceState::Dead,
            ) || changed_owner_state(
                &from.resources,
                &to.resources,
                owner_ordinal,
                SettlementResourceState::ProvisionalResult,
                SettlementResourceState::Dead,
            );
            let Some(finalized_position) = from
                .abort_cleanup_order
                .iter()
                .position(|candidate| *candidate == owner_ordinal)
            else {
                return false;
            };
            let prefix_is_only_provisional = from.abort_cleanup_order[..finalized_position]
                .iter()
                .all(|ordinal| {
                    from.resources[*ordinal as usize] == SettlementResourceState::ProvisionalResult
                });
            let mut expected_abort = from.abort_cleanup_order.clone();
            expected_abort.remove(finalized_position);
            state_transition
                && from.accept_cleanup_order.is_empty()
                && to.accept_cleanup_order.is_empty()
                && prefix_is_only_provisional
                && to.abort_cleanup_order == expected_abort
        }
        SettlementProgressAction::StageOwnedResult { owner_ordinal } => {
            to.normal_outcome.is_none()
                && changed_owner_state(
                    &from.resources,
                    &to.resources,
                    owner_ordinal,
                    SettlementResourceState::Live,
                    SettlementResourceState::ProvisionalResult,
                )
                && from.accept_cleanup_order.is_empty()
                && to.accept_cleanup_order.is_empty()
                && from.abort_cleanup_order == to.abort_cleanup_order
        }
        SettlementProgressAction::CertifyOutcome { trace_evidence } => {
            let expected_accept = to
                .abort_cleanup_order
                .iter()
                .copied()
                .filter(|ordinal| to.resources[*ordinal as usize] == SettlementResourceState::Live)
                .collect::<Vec<_>>();
            from.normal_outcome.is_none()
                && to.normal_outcome.is_some()
                && from.resources == to.resources
                && from.accept_cleanup_order.is_empty()
                && from.abort_cleanup_order == to.abort_cleanup_order
                && to.accept_cleanup_order == expected_accept
                && trace_evidence.iter().any(|byte| *byte != 0)
        }
    }
}

fn changed_owner_state(
    from: &[SettlementResourceState],
    to: &[SettlementResourceState],
    owner_ordinal: u32,
    expected_from: SettlementResourceState,
    expected_to: SettlementResourceState,
) -> bool {
    let Ok(owner) = usize::try_from(owner_ordinal) else {
        return false;
    };
    from.iter()
        .zip(to)
        .enumerate()
        .all(|(index, (left, right))| {
            if index == owner {
                *left == expected_from && *right == expected_to
            } else {
                left == right
            }
        })
        && owner < from.len()
}

fn validate_checkpoint(
    checkpoint: &SettlementCheckpointSpec,
    resource_count: usize,
) -> Result<(), SettlementError> {
    if checkpoint.resources.len() != resource_count {
        return Err(SettlementError::CheckpointResourceCountMismatch);
    }
    if checkpoint.resources.iter().any(|state| {
        matches!(
            state,
            SettlementResourceState::Finalizing | SettlementResourceState::Published
        )
    }) {
        return Err(SettlementError::InvalidCheckpointState);
    }
    let provisional = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter(|(_, state)| **state == SettlementResourceState::ProvisionalResult)
        .map(|(ordinal, _)| ordinal as u32)
        .collect::<Vec<_>>();
    if provisional.len() > 1 {
        return Err(SettlementError::MultipleProvisionalResults);
    }
    let abort_required = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter(|(_, state)| **state != SettlementResourceState::Dead)
        .map(|(ordinal, _)| ordinal as u32)
        .collect::<BTreeSet<_>>();
    validate_exact_order(&checkpoint.abort_cleanup_order, &abort_required)?;

    let accept_required = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter(|(_, state)| **state == SettlementResourceState::Live)
        .map(|(ordinal, _)| ordinal as u32)
        .collect::<BTreeSet<_>>();
    match checkpoint.normal_outcome {
        None => {
            if !checkpoint.accept_cleanup_order.is_empty() {
                return Err(SettlementError::InvalidNormalOutcome);
            }
        }
        Some(SettlementOutcome::ScalarSuccess | SettlementOutcome::SemanticFailure) => {
            if !provisional.is_empty() {
                return Err(SettlementError::InvalidNormalOutcome);
            }
            validate_exact_order(&checkpoint.accept_cleanup_order, &accept_required)?;
        }
        Some(SettlementOutcome::OwnedSuccess { owner_ordinal }) => {
            if provisional.as_slice() != [owner_ordinal] {
                return Err(SettlementError::InvalidNormalOutcome);
            }
            validate_exact_order(&checkpoint.accept_cleanup_order, &accept_required)?;
        }
    }
    Ok(())
}

fn validate_exact_order(order: &[u32], required: &BTreeSet<u32>) -> Result<(), SettlementError> {
    let actual = order.iter().copied().collect::<BTreeSet<_>>();
    if actual.len() != order.len() || actual != *required {
        return Err(SettlementError::InvalidCleanupOrder);
    }
    Ok(())
}

fn validate_decision(decision: SettlementDecision) -> Result<(), SettlementError> {
    if matches!(
        decision,
        SettlementDecision::Abort(AdapterAbortReason::PhysicalResult(0))
    ) {
        return Err(SettlementError::InvalidAbortReason);
    }
    Ok(())
}

fn actions_for(
    checkpoint: &SettlementCheckpointSpec,
    decision: SettlementDecision,
) -> Result<Vec<SettlementAction>, SettlementError> {
    let mut actions = Vec::new();
    match decision {
        SettlementDecision::Abort(_) => {
            actions.extend(
                checkpoint
                    .abort_cleanup_order
                    .iter()
                    .copied()
                    .map(|owner_ordinal| SettlementAction::Finalize { owner_ordinal }),
            );
        }
        SettlementDecision::Accept(outcome) => {
            if checkpoint.normal_outcome != Some(outcome) {
                return Err(SettlementError::DecisionNotAdmitted);
            }
            actions.extend(
                checkpoint
                    .accept_cleanup_order
                    .iter()
                    .copied()
                    .map(|owner_ordinal| SettlementAction::Finalize { owner_ordinal }),
            );
            if let SettlementOutcome::OwnedSuccess { owner_ordinal } = outcome {
                actions.push(SettlementAction::Publish { owner_ordinal });
            }
        }
    }
    Ok(actions)
}

fn apply_actions(
    resources: &mut [SettlementResourceState],
    actions: &[SettlementAction],
) -> Result<(), SettlementError> {
    for action in actions {
        let (owner_ordinal, expected) = match action {
            SettlementAction::Finalize { owner_ordinal } => (
                *owner_ordinal,
                [
                    SettlementResourceState::Live,
                    SettlementResourceState::ProvisionalResult,
                ]
                .as_slice(),
            ),
            SettlementAction::Publish { owner_ordinal } => (
                *owner_ordinal,
                [SettlementResourceState::ProvisionalResult].as_slice(),
            ),
        };
        let index =
            usize::try_from(owner_ordinal).map_err(|_| SettlementError::InvalidStateTransition)?;
        let state = resources
            .get_mut(index)
            .ok_or(SettlementError::InvalidStateTransition)?;
        if !expected.contains(state) {
            return Err(SettlementError::InvalidStateTransition);
        }
        *state = match action {
            SettlementAction::Finalize { .. } => SettlementResourceState::Dead,
            SettlementAction::Publish { .. } => SettlementResourceState::Published,
        };
    }
    Ok(())
}

fn terminal_dispositions(
    resources: &[SettlementResourceState],
) -> Result<Vec<SettlementDisposition>, SettlementError> {
    resources
        .iter()
        .map(|state| match state {
            SettlementResourceState::Dead => Ok(SettlementDisposition::Dead),
            SettlementResourceState::Published => Ok(SettlementDisposition::Published),
            SettlementResourceState::Live
            | SettlementResourceState::ProvisionalResult
            | SettlementResourceState::Finalizing => Err(SettlementError::NotQuiescent),
        })
        .collect()
}

fn checkpoint_json(checkpoint: &SettlementCheckpointSpec) -> String {
    let resources = checkpoint
        .resources
        .iter()
        .map(|state| quote_json(resource_state_name(*state)))
        .collect::<Vec<_>>()
        .join(",");
    let abort = checkpoint
        .abort_cleanup_order
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let accept = checkpoint
        .accept_cleanup_order
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let outcome = checkpoint
        .normal_outcome
        .map_or_else(|| "null".to_owned(), outcome_json);
    format!(
        "{{\"checkpoint\":{},\"resources\":[{}],\"normal_outcome\":{},\"abort_cleanup_order\":[{}],\"accept_cleanup_order\":[{}]}}",
        checkpoint.checkpoint, resources, outcome, abort, accept
    )
}

fn progress_edge_json(edge: &SettlementProgressEdge) -> String {
    let action = match edge.action {
        SettlementProgressAction::Finalize { owner_ordinal } => {
            format!("{{\"kind\":\"finalize\",\"owner_ordinal\":{owner_ordinal}}}")
        }
        SettlementProgressAction::StageOwnedResult { owner_ordinal } => {
            format!("{{\"kind\":\"stage_owned_result\",\"owner_ordinal\":{owner_ordinal}}}")
        }
        SettlementProgressAction::CertifyOutcome { trace_evidence } => format!(
            "{{\"kind\":\"certify_outcome\",\"trace_evidence\":\"{}\"}}",
            hex(&trace_evidence)
        ),
    };
    format!(
        "{{\"from\":{},\"to\":{},\"action\":{action}}}",
        edge.from, edge.to
    )
}

fn decision_json(decision: SettlementDecision) -> String {
    match decision {
        SettlementDecision::Accept(outcome) => {
            format!(
                "{{\"kind\":\"accept\",\"outcome\":{}}}",
                outcome_json(outcome)
            )
        }
        SettlementDecision::Abort(reason) => {
            format!(
                "{{\"kind\":\"abort\",\"reason\":{}}}",
                abort_reason_json(reason)
            )
        }
    }
}

fn outcome_json(outcome: SettlementOutcome) -> String {
    match outcome {
        SettlementOutcome::ScalarSuccess => "{\"kind\":\"scalar_success\"}".to_owned(),
        SettlementOutcome::SemanticFailure => "{\"kind\":\"semantic_failure\"}".to_owned(),
        SettlementOutcome::OwnedSuccess { owner_ordinal } => {
            format!("{{\"kind\":\"owned_success\",\"owner_ordinal\":{owner_ordinal}}}")
        }
    }
}

fn abort_reason_json(reason: AdapterAbortReason) -> String {
    match reason {
        AdapterAbortReason::PhysicalResult(code) => {
            format!("{{\"kind\":\"physical_result\",\"code\":{code}}}")
        }
        AdapterAbortReason::MalformedResponse => "{\"kind\":\"malformed_response\"}".to_owned(),
        AdapterAbortReason::TraceRejected => "{\"kind\":\"trace_rejected\"}".to_owned(),
        AdapterAbortReason::HostUnwind => "{\"kind\":\"host_unwind\"}".to_owned(),
    }
}

fn action_json(action: &SettlementAction) -> String {
    match action {
        SettlementAction::Finalize { owner_ordinal } => {
            format!("{{\"kind\":\"finalize\",\"owner_ordinal\":{owner_ordinal}}}")
        }
        SettlementAction::Publish { owner_ordinal } => {
            format!("{{\"kind\":\"publish\",\"owner_ordinal\":{owner_ordinal}}}")
        }
    }
}

const fn resource_state_name(state: SettlementResourceState) -> &'static str {
    match state {
        SettlementResourceState::Live => "live",
        SettlementResourceState::ProvisionalResult => "provisional_result",
        SettlementResourceState::Finalizing => "finalizing",
        SettlementResourceState::Dead => "dead",
        SettlementResourceState::Published => "published",
    }
}

const fn disposition_name(disposition: SettlementDisposition) -> &'static str {
    match disposition {
        SettlementDisposition::Dead => "dead",
        SettlementDisposition::Published => "published",
    }
}

fn fingerprint(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
#[path = "native_settlement/tests.rs"]
mod tests;
