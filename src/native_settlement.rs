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
mod tests {
    use super::*;

    macro_rules! assert_not_impl {
        ($type:ty, $trait:path) => {{
            trait AmbiguousIfImplemented<Marker> {
                fn probe() {}
            }
            impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
            struct Implemented;
            impl<T: ?Sized + $trait> AmbiguousIfImplemented<Implemented> for T {}
            let _ = <$type as AmbiguousIfImplemented<_>>::probe;
        }};
    }

    const CONTRACT: [u8; 32] = [0x5a; 32];

    #[derive(Debug, Eq, PartialEq)]
    struct FrameSnapshot {
        function: DeclarationId,
        recovery_contract: [u8; 32],
        certificate_fingerprint: [u8; 32],
        invocation: NonZeroU64,
        checkpoint: u32,
        resources: Vec<SettlementResourceState>,
        terminal: Option<(SettlementDecision, String)>,
    }

    fn snapshot(frame: &NativeSettlementFrame) -> FrameSnapshot {
        FrameSnapshot {
            function: frame.function.clone(),
            recovery_contract: frame.recovery_contract,
            certificate_fingerprint: frame.certificate_fingerprint,
            invocation: frame.invocation,
            checkpoint: frame.checkpoint,
            resources: frame.resources.clone(),
            terminal: frame
                .terminal
                .as_ref()
                .map(|terminal| (terminal.decision, terminal.receipt.canonical_json())),
        }
    }

    fn certificate(checkpoints: Vec<SettlementCheckpointSpec>) -> NativeSettlementCertificate {
        let resource_count = checkpoints[0].resources.len();
        NativeSettlementCertificate::try_new(
            DeclarationId::new("token.settlement"),
            CONTRACT,
            resource_count,
            checkpoints,
        )
        .unwrap()
    }

    fn reverse_non_dead(states: &[SettlementResourceState]) -> Vec<u32> {
        states
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, state)| **state != SettlementResourceState::Dead)
            .map(|(ordinal, _)| ordinal as u32)
            .collect()
    }

    fn reverse_live(states: &[SettlementResourceState]) -> Vec<u32> {
        states
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, state)| **state == SettlementResourceState::Live)
            .map(|(ordinal, _)| ordinal as u32)
            .collect()
    }

    #[test]
    fn abort_exhaustively_finalizes_every_non_dead_resource_once() {
        for resource_count in 1..=6_usize {
            let mut checkpoint_id = 1_u32;
            let mut specs = Vec::new();
            let combinations = 3_usize.pow(resource_count as u32);
            for mut encoded in 0..combinations {
                let mut states = Vec::new();
                let mut provisional_count = 0;
                for _ in 0..resource_count {
                    let state = match encoded % 3 {
                        0 => SettlementResourceState::Live,
                        1 => SettlementResourceState::Dead,
                        _ => {
                            provisional_count += 1;
                            SettlementResourceState::ProvisionalResult
                        }
                    };
                    encoded /= 3;
                    states.push(state);
                }
                if provisional_count > 1 {
                    continue;
                }
                specs.push(SettlementCheckpointSpec::new(
                    checkpoint_id,
                    states.clone(),
                    None,
                    reverse_non_dead(&states),
                    Vec::new(),
                ));
                checkpoint_id += 1;
            }
            let certificate = certificate(specs);
            for checkpoint in 1..checkpoint_id {
                for reason in [
                    AdapterAbortReason::PhysicalResult(1),
                    AdapterAbortReason::PhysicalResult(u32::MAX),
                    AdapterAbortReason::MalformedResponse,
                    AdapterAbortReason::TraceRejected,
                    AdapterAbortReason::HostUnwind,
                ] {
                    let mut frame = certificate
                        .prepare_frame(NonZeroU64::new(checkpoint as u64).unwrap(), checkpoint)
                        .unwrap();
                    let initial = frame.resources.clone();
                    let decision = SettlementDecision::Abort(reason);
                    let first = certificate.settle(&mut frame, decision).unwrap();
                    assert!(frame.is_terminal());
                    assert!(frame
                        .resources()
                        .iter()
                        .all(|state| *state == SettlementResourceState::Dead));
                    let finalized = first
                        .performed_actions()
                        .iter()
                        .map(|action| match action {
                            SettlementAction::Finalize { owner_ordinal } => *owner_ordinal,
                            SettlementAction::Publish { .. } => panic!("abort cannot publish"),
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(finalized, reverse_non_dead(&initial));
                    assert_eq!(
                        finalized.iter().copied().collect::<BTreeSet<_>>().len(),
                        finalized.len()
                    );
                    certificate
                        .validate_receipt(frame.invocation(), first.receipt())
                        .unwrap();

                    let replay = certificate.settle(&mut frame, decision).unwrap();
                    assert_eq!(replay.receipt(), first.receipt());
                    assert!(replay.performed_actions().is_empty());
                }
            }
        }
    }

    #[test]
    fn accepted_outcomes_are_exact_and_owned_publication_is_unique() {
        let scalar_states = vec![
            SettlementResourceState::Live,
            SettlementResourceState::Dead,
            SettlementResourceState::Live,
        ];
        let failure_states = vec![
            SettlementResourceState::Dead,
            SettlementResourceState::Live,
            SettlementResourceState::Live,
        ];
        let owned_states = vec![
            SettlementResourceState::Live,
            SettlementResourceState::ProvisionalResult,
            SettlementResourceState::Dead,
        ];
        let certificate = certificate(vec![
            SettlementCheckpointSpec::new(
                1,
                scalar_states.clone(),
                Some(SettlementOutcome::ScalarSuccess),
                reverse_non_dead(&scalar_states),
                reverse_live(&scalar_states),
            ),
            SettlementCheckpointSpec::new(
                2,
                failure_states.clone(),
                Some(SettlementOutcome::SemanticFailure),
                reverse_non_dead(&failure_states),
                reverse_live(&failure_states),
            ),
            SettlementCheckpointSpec::new(
                3,
                owned_states.clone(),
                Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
                reverse_non_dead(&owned_states),
                reverse_live(&owned_states),
            ),
        ]);

        for (checkpoint, outcome) in [
            (1, SettlementOutcome::ScalarSuccess),
            (2, SettlementOutcome::SemanticFailure),
            (3, SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
        ] {
            let mut frame = certificate
                .prepare_frame(NonZeroU64::new(checkpoint as u64).unwrap(), checkpoint)
                .unwrap();
            let application = certificate
                .settle(&mut frame, SettlementDecision::Accept(outcome))
                .unwrap();
            let published = application
                .receipt()
                .dispositions()
                .iter()
                .filter(|disposition| **disposition == SettlementDisposition::Published)
                .count();
            assert_eq!(published, usize::from(checkpoint == 3));
            assert_eq!(application.receipt().active_finalizers(), 0);
        }

        let mut wrong = certificate
            .prepare_frame(NonZeroU64::new(9).unwrap(), 3)
            .unwrap();
        let before = snapshot(&wrong);
        assert_eq!(
            certificate.settle(
                &mut wrong,
                SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 0 })
            ),
            Err(SettlementError::DecisionNotAdmitted)
        );
        assert_eq!(snapshot(&wrong), before);
    }

    #[test]
    fn accepted_outcomes_exhaust_every_owner_liveness_combination() {
        for resource_count in 1..=6_usize {
            let mut specs = Vec::new();
            let mut outcomes = Vec::new();
            let combinations = 2_usize.pow(resource_count as u32);
            for mut encoded in 0..combinations {
                let mut states = Vec::new();
                for _ in 0..resource_count {
                    states.push(if encoded.is_multiple_of(2) {
                        SettlementResourceState::Live
                    } else {
                        SettlementResourceState::Dead
                    });
                    encoded /= 2;
                }
                for outcome in [
                    SettlementOutcome::ScalarSuccess,
                    SettlementOutcome::SemanticFailure,
                ] {
                    let checkpoint = u32::try_from(specs.len() + 1).unwrap();
                    specs.push(SettlementCheckpointSpec::new(
                        checkpoint,
                        states.clone(),
                        Some(outcome),
                        reverse_non_dead(&states),
                        reverse_live(&states),
                    ));
                    outcomes.push(outcome);
                }
            }
            for result_ordinal in 0..resource_count {
                let other_combinations = 2_usize.pow((resource_count - 1) as u32);
                for mut encoded in 0..other_combinations {
                    let mut states = Vec::new();
                    for ordinal in 0..resource_count {
                        if ordinal == result_ordinal {
                            states.push(SettlementResourceState::ProvisionalResult);
                        } else {
                            states.push(if encoded.is_multiple_of(2) {
                                SettlementResourceState::Live
                            } else {
                                SettlementResourceState::Dead
                            });
                            encoded /= 2;
                        }
                    }
                    let outcome = SettlementOutcome::OwnedSuccess {
                        owner_ordinal: result_ordinal as u32,
                    };
                    let checkpoint = u32::try_from(specs.len() + 1).unwrap();
                    specs.push(SettlementCheckpointSpec::new(
                        checkpoint,
                        states.clone(),
                        Some(outcome),
                        reverse_non_dead(&states),
                        reverse_live(&states),
                    ));
                    outcomes.push(outcome);
                }
            }

            let certificate = certificate(specs);
            for (index, outcome) in outcomes.iter().copied().enumerate() {
                let checkpoint = u32::try_from(index + 1).unwrap();
                let mut frame = certificate
                    .prepare_frame(NonZeroU64::new((index + 1) as u64).unwrap(), checkpoint)
                    .unwrap();
                let application = certificate
                    .settle(&mut frame, SettlementDecision::Accept(outcome))
                    .unwrap();
                let expected_published = match outcome {
                    SettlementOutcome::OwnedSuccess { owner_ordinal } => Some(owner_ordinal),
                    SettlementOutcome::ScalarSuccess | SettlementOutcome::SemanticFailure => None,
                };
                let actual_published = application
                    .receipt()
                    .dispositions()
                    .iter()
                    .enumerate()
                    .filter(|(_, disposition)| **disposition == SettlementDisposition::Published)
                    .map(|(ordinal, _)| ordinal as u32)
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual_published,
                    expected_published.into_iter().collect::<Vec<_>>()
                );
                assert!(frame.resources().iter().all(|state| matches!(
                    state,
                    SettlementResourceState::Dead | SettlementResourceState::Published
                )));
                assert_eq!(application.receipt().active_finalizers(), 0);
            }
        }
    }

    #[test]
    fn conflicting_terminal_decision_is_nonmutating() {
        let states = vec![SettlementResourceState::Live];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            Some(SettlementOutcome::ScalarSuccess),
            reverse_non_dead(&states),
            reverse_live(&states),
        )]);
        let mut frame = certificate
            .prepare_frame(NonZeroU64::new(1).unwrap(), 1)
            .unwrap();
        certificate
            .settle(
                &mut frame,
                SettlementDecision::Abort(AdapterAbortReason::MalformedResponse),
            )
            .unwrap();
        let terminal = snapshot(&frame);
        assert_eq!(
            certificate.settle(
                &mut frame,
                SettlementDecision::Accept(SettlementOutcome::ScalarSuccess)
            ),
            Err(SettlementError::ConflictingTerminalDecision)
        );
        assert_eq!(snapshot(&frame), terminal);
    }

    #[test]
    fn certificate_builder_rejects_every_structural_ambiguity() {
        let valid = SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Live],
            Some(SettlementOutcome::ScalarSuccess),
            vec![0],
            vec![0],
        );
        for function in ["", "token\0settlement"] {
            assert_eq!(
                NativeSettlementCertificate::try_new(
                    DeclarationId::new(function),
                    CONTRACT,
                    1,
                    vec![valid.clone()]
                ),
                Err(SettlementError::InvalidFunctionIdentity)
            );
        }
        assert_eq!(
            NativeSettlementCertificate::try_new(
                DeclarationId::new("token.settlement"),
                [0; 32],
                1,
                vec![valid.clone()]
            ),
            Err(SettlementError::ZeroRecoveryContract)
        );
        assert_eq!(
            NativeSettlementCertificate::try_new(
                DeclarationId::new("token.settlement"),
                CONTRACT,
                0,
                vec![valid.clone()]
            ),
            Err(SettlementError::ResourceCountOutOfBounds)
        );
        let mut noncanonical = valid.clone();
        noncanonical.checkpoint = 2;
        assert_eq!(
            NativeSettlementCertificate::try_new(
                DeclarationId::new("token.settlement"),
                CONTRACT,
                1,
                vec![noncanonical]
            ),
            Err(SettlementError::NonCanonicalCheckpoint)
        );
        let duplicate = SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Live],
            None,
            vec![0, 0],
            Vec::new(),
        );
        assert_eq!(
            NativeSettlementCertificate::try_new(
                DeclarationId::new("token.settlement"),
                CONTRACT,
                1,
                vec![duplicate]
            ),
            Err(SettlementError::InvalidCleanupOrder)
        );
        let terminal = SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Published],
            None,
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            NativeSettlementCertificate::try_new(
                DeclarationId::new("token.settlement"),
                CONTRACT,
                1,
                vec![terminal]
            ),
            Err(SettlementError::InvalidCheckpointState)
        );
        let multiple = SettlementCheckpointSpec::new(
            1,
            vec![
                SettlementResourceState::ProvisionalResult,
                SettlementResourceState::ProvisionalResult,
            ],
            None,
            vec![1, 0],
            Vec::new(),
        );
        assert_eq!(
            NativeSettlementCertificate::try_new(
                DeclarationId::new("token.settlement"),
                CONTRACT,
                2,
                vec![multiple]
            ),
            Err(SettlementError::MultipleProvisionalResults)
        );
    }

    #[test]
    fn malformed_receipts_fail_independent_validation() {
        let states = vec![
            SettlementResourceState::Live,
            SettlementResourceState::ProvisionalResult,
        ];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
            reverse_non_dead(&states),
            reverse_live(&states),
        )]);
        let invocation = NonZeroU64::new(7).unwrap();
        let mut frame = certificate.prepare_frame(invocation, 1).unwrap();
        let valid = certificate
            .settle(
                &mut frame,
                SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
            )
            .unwrap()
            .receipt;

        let mut mutations = Vec::new();
        let mut receipt = valid.clone();
        receipt.schema = "wrong";
        mutations.push((receipt, SettlementError::ReceiptSchemaMismatch));
        let mut receipt = valid.clone();
        receipt.recovery_contract[0] ^= 1;
        mutations.push((receipt, SettlementError::ReceiptBindingMismatch));
        let mut receipt = valid.clone();
        receipt.certificate_fingerprint[0] ^= 1;
        mutations.push((receipt, SettlementError::ReceiptBindingMismatch));
        let mut receipt = valid.clone();
        receipt.invocation = NonZeroU64::new(8).unwrap();
        mutations.push((receipt, SettlementError::ReceiptBindingMismatch));
        let mut receipt = valid.clone();
        receipt.actions.swap(0, 1);
        mutations.push((receipt, SettlementError::ReceiptActionMismatch));
        let mut receipt = valid.clone();
        receipt.dispositions[1] = SettlementDisposition::Dead;
        mutations.push((receipt, SettlementError::ReceiptDispositionMismatch));
        let mut receipt = valid.clone();
        receipt.active_finalizers = 1;
        mutations.push((receipt, SettlementError::NotQuiescent));

        for (receipt, expected) in mutations {
            assert_eq!(
                certificate.validate_receipt(invocation, &receipt),
                Err(expected)
            );
        }
    }

    #[test]
    fn canonical_certificate_and_receipt_are_deterministic_and_domain_separated() {
        let states = vec![SettlementResourceState::Live];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            Some(SettlementOutcome::ScalarSuccess),
            reverse_non_dead(&states),
            reverse_live(&states),
        )]);
        assert_eq!(certificate.canonical_json(), certificate.canonical_json());
        assert_eq!(certificate.fingerprint(), certificate.fingerprint());

        let mut frame = certificate
            .prepare_frame(NonZeroU64::new(1).unwrap(), 1)
            .unwrap();
        let receipt = certificate
            .settle(
                &mut frame,
                SettlementDecision::Accept(SettlementOutcome::ScalarSuccess),
            )
            .unwrap()
            .receipt;
        assert_eq!(receipt.canonical_json(), receipt.canonical_json());
        assert_eq!(receipt.fingerprint(), receipt.fingerprint());
        assert_ne!(certificate.fingerprint(), receipt.fingerprint());
    }

    #[test]
    fn physical_result_zero_is_never_an_abort_reason() {
        let states = vec![SettlementResourceState::Live];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            None,
            reverse_non_dead(&states),
            Vec::new(),
        )]);
        let mut frame = certificate
            .prepare_frame(NonZeroU64::new(1).unwrap(), 1)
            .unwrap();
        let before = snapshot(&frame);
        assert_eq!(
            certificate.settle(
                &mut frame,
                SettlementDecision::Abort(AdapterAbortReason::PhysicalResult(0))
            ),
            Err(SettlementError::InvalidAbortReason)
        );
        assert_eq!(snapshot(&frame), before);
    }

    #[test]
    fn deterministic_frame_preparation_is_not_a_uniqueness_reservation() {
        let states = vec![SettlementResourceState::Live];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            None,
            reverse_non_dead(&states),
            Vec::new(),
        )]);
        let invocation = NonZeroU64::new(77).unwrap();
        let first = certificate.prepare_frame(invocation, 1).unwrap();
        let second = certificate.prepare_frame(invocation, 1).unwrap();
        assert_eq!(snapshot(&first), snapshot(&second));
        assert!(!first.is_terminal());
        assert!(!second.is_terminal());
    }

    #[test]
    fn strict_progress_graph_rejects_bad_starts_edges_transitions_and_orphans() {
        let live = SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Live],
            None,
            vec![0],
            Vec::new(),
        );
        let dead = SettlementCheckpointSpec::new(
            2,
            vec![SettlementResourceState::Dead],
            None,
            Vec::new(),
            Vec::new(),
        );
        let terminal = SettlementCheckpointSpec::new(
            3,
            vec![SettlementResourceState::Dead],
            Some(SettlementOutcome::ScalarSuccess),
            Vec::new(),
            Vec::new(),
        );
        let finalize = SettlementProgressEdge::new(
            1,
            2,
            SettlementProgressAction::Finalize { owner_ordinal: 0 },
        );
        let certify = SettlementProgressEdge::new(
            2,
            3,
            SettlementProgressAction::CertifyOutcome {
                trace_evidence: [7; 32],
            },
        );
        let build = |starts, edges| {
            NativeSettlementCertificate::try_new_with_progress(
                DeclarationId::new("token.progress"),
                CONTRACT,
                1,
                vec![live.clone(), dead.clone(), terminal.clone()],
                starts,
                edges,
            )
        };
        assert!(build(vec![1], vec![finalize, certify]).is_ok());
        assert_eq!(
            build(vec![1, 2], vec![finalize, certify]),
            Err(SettlementError::InvalidProgressStart)
        );
        assert_eq!(
            build(vec![1], vec![certify]),
            Err(SettlementError::NonCanonicalProgressEdge)
        );
        assert_eq!(
            build(vec![1], vec![finalize]),
            Err(SettlementError::UnreachableCheckpoint)
        );
        assert_eq!(
            build(vec![1], vec![finalize, finalize, certify]),
            Err(SettlementError::NonCanonicalProgressEdge)
        );
        assert_eq!(
            build(
                vec![1],
                vec![
                    SettlementProgressEdge::new(
                        1,
                        2,
                        SettlementProgressAction::StageOwnedResult { owner_ordinal: 0 },
                    ),
                    certify,
                ],
            ),
            Err(SettlementError::InvalidProgressTransition)
        );
        assert_eq!(
            build(
                vec![1],
                vec![
                    finalize,
                    SettlementProgressEdge::new(
                        2,
                        3,
                        SettlementProgressAction::Finalize { owner_ordinal: 0 },
                    ),
                ],
            ),
            Err(SettlementError::InvalidProgressTransition)
        );

        let finalize_order_counterexample = NativeSettlementCertificate::try_new_with_progress(
            DeclarationId::new("token.progress.finalize-order"),
            CONTRACT,
            3,
            vec![
                SettlementCheckpointSpec::new(
                    1,
                    vec![SettlementResourceState::Live; 3],
                    None,
                    vec![2, 1, 0],
                    Vec::new(),
                ),
                SettlementCheckpointSpec::new(
                    2,
                    vec![
                        SettlementResourceState::Live,
                        SettlementResourceState::Live,
                        SettlementResourceState::Dead,
                    ],
                    None,
                    vec![0, 1],
                    Vec::new(),
                ),
            ],
            vec![1],
            vec![SettlementProgressEdge::new(
                1,
                2,
                SettlementProgressAction::Finalize { owner_ordinal: 2 },
            )],
        );
        assert_eq!(
            finalize_order_counterexample,
            Err(SettlementError::InvalidProgressTransition)
        );

        let skipped_live_counterexample = NativeSettlementCertificate::try_new_with_progress(
            DeclarationId::new("token.progress.finalize-skips-live"),
            CONTRACT,
            2,
            vec![
                SettlementCheckpointSpec::new(
                    1,
                    vec![SettlementResourceState::Live; 2],
                    None,
                    vec![0, 1],
                    Vec::new(),
                ),
                SettlementCheckpointSpec::new(
                    2,
                    vec![SettlementResourceState::Live, SettlementResourceState::Dead],
                    None,
                    vec![0],
                    Vec::new(),
                ),
            ],
            vec![1],
            vec![SettlementProgressEdge::new(
                1,
                2,
                SettlementProgressAction::Finalize { owner_ordinal: 1 },
            )],
        );
        assert_eq!(
            skipped_live_counterexample,
            Err(SettlementError::InvalidProgressTransition)
        );

        let stage_order_counterexample = NativeSettlementCertificate::try_new_with_progress(
            DeclarationId::new("token.progress.stage-order"),
            CONTRACT,
            2,
            vec![
                SettlementCheckpointSpec::new(
                    1,
                    vec![SettlementResourceState::Live; 2],
                    None,
                    vec![1, 0],
                    Vec::new(),
                ),
                SettlementCheckpointSpec::new(
                    2,
                    vec![
                        SettlementResourceState::Live,
                        SettlementResourceState::ProvisionalResult,
                    ],
                    None,
                    vec![0, 1],
                    Vec::new(),
                ),
            ],
            vec![1],
            vec![SettlementProgressEdge::new(
                1,
                2,
                SettlementProgressAction::StageOwnedResult { owner_ordinal: 1 },
            )],
        );
        assert_eq!(
            stage_order_counterexample,
            Err(SettlementError::InvalidProgressTransition)
        );

        let certify_accept_counterexample = NativeSettlementCertificate::try_new_with_progress(
            DeclarationId::new("token.progress.certify-order"),
            CONTRACT,
            2,
            vec![
                SettlementCheckpointSpec::new(
                    1,
                    vec![SettlementResourceState::Live; 2],
                    None,
                    vec![1, 0],
                    Vec::new(),
                ),
                SettlementCheckpointSpec::new(
                    2,
                    vec![SettlementResourceState::Live; 2],
                    Some(SettlementOutcome::ScalarSuccess),
                    vec![1, 0],
                    vec![0, 1],
                ),
            ],
            vec![1],
            vec![SettlementProgressEdge::new(
                1,
                2,
                SettlementProgressAction::CertifyOutcome {
                    trace_evidence: [9; 32],
                },
            )],
        );
        assert_eq!(
            certify_accept_counterexample,
            Err(SettlementError::InvalidProgressTransition)
        );
    }

    fn complete_phase_transaction(
        certificate: &NativeSettlementCertificate,
        transaction: &mut NativeSettlementTransaction,
        decision: SettlementDecision,
    ) -> NativeSettlementReceipt {
        certificate
            .lock_transaction_decision(transaction, decision)
            .unwrap();
        certificate
            .lock_transaction_decision(transaction, decision)
            .unwrap();
        loop {
            match transaction.actions.get(transaction.next_action).copied() {
                Some(SettlementAction::Finalize { owner_ordinal }) => {
                    let ticket = certificate.begin_next_finalizer(transaction).unwrap();
                    assert_eq!(
                        ticket.action(),
                        SettlementAction::Finalize { owner_ordinal }
                    );
                    assert_eq!(
                        transaction.resources[owner_ordinal as usize],
                        SettlementResourceState::Finalizing
                    );
                    certificate.complete_finalizer(transaction, ticket).unwrap();
                    assert_eq!(
                        transaction.resources[owner_ordinal as usize],
                        SettlementResourceState::Dead
                    );
                }
                Some(SettlementAction::Publish { owner_ordinal }) => {
                    assert!(transaction.resources.iter().all(|state| {
                        !matches!(
                            state,
                            SettlementResourceState::Live
                                | SettlementResourceState::Finalizing
                                | SettlementResourceState::Published
                        )
                    }));
                    certificate.publish_owned_candidate(transaction).unwrap();
                    assert_eq!(
                        transaction.resources[owner_ordinal as usize],
                        SettlementResourceState::Published
                    );
                }
                None => break,
            }
        }
        let candidate = certificate.finish_provider_settlement(transaction).unwrap();
        assert!(candidate.performed_actions().is_empty());
        assert_eq!(
            transaction.phase(),
            SettlementTransactionPhase::ProviderSettled
        );
        let provider_replay = certificate
            .replay_provider_candidate(transaction, decision)
            .unwrap();
        assert_eq!(provider_replay.receipt(), candidate.receipt());
        assert!(provider_replay.performed_actions().is_empty());
        let receipt = candidate.receipt().clone();
        let committed = certificate
            .commit_provider_receipt(transaction, &receipt)
            .unwrap();
        assert_eq!(committed.receipt(), &receipt);
        assert!(committed.performed_actions().is_empty());
        assert_eq!(
            transaction.phase(),
            SettlementTransactionPhase::ReceiptCommitted
        );
        let duplicate = certificate
            .commit_provider_receipt(transaction, &receipt)
            .unwrap();
        assert_eq!(
            duplicate.receipt().canonical_json(),
            receipt.canonical_json()
        );
        assert!(duplicate.performed_actions().is_empty());
        let replay = certificate
            .replay_committed_receipt(transaction, decision)
            .unwrap();
        assert_eq!(replay.receipt().canonical_json(), receipt.canonical_json());
        assert!(replay.performed_actions().is_empty());
        receipt
    }

    #[test]
    fn phase_machine_exhausts_decisions_and_preserves_receipt_kats() {
        let states = vec![
            SettlementResourceState::Live,
            SettlementResourceState::ProvisionalResult,
            SettlementResourceState::Dead,
        ];
        let owned_certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
            reverse_non_dead(&states),
            reverse_live(&states),
        )]);
        let decisions = [
            SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
            SettlementDecision::Abort(AdapterAbortReason::PhysicalResult(1)),
            SettlementDecision::Abort(AdapterAbortReason::PhysicalResult(u32::MAX)),
            SettlementDecision::Abort(AdapterAbortReason::MalformedResponse),
            SettlementDecision::Abort(AdapterAbortReason::TraceRejected),
            SettlementDecision::Abort(AdapterAbortReason::HostUnwind),
        ];
        for (index, decision) in decisions.into_iter().enumerate() {
            let invocation = NonZeroU64::new((index + 1) as u64).unwrap();
            let mut phased = owned_certificate
                .prepare_start_transaction(invocation)
                .unwrap();
            let phased_receipt =
                complete_phase_transaction(&owned_certificate, &mut phased, decision);

            let mut legacy = owned_certificate.prepare_start_frame(invocation).unwrap();
            let legacy_receipt = owned_certificate
                .settle(&mut legacy, decision)
                .unwrap()
                .receipt;
            assert_eq!(
                phased_receipt.canonical_json(),
                legacy_receipt.canonical_json()
            );
            assert_eq!(phased_receipt.fingerprint(), legacy_receipt.fingerprint());
        }

        for outcome in [
            SettlementOutcome::ScalarSuccess,
            SettlementOutcome::SemanticFailure,
        ] {
            let states = vec![SettlementResourceState::Live, SettlementResourceState::Dead];
            let certificate = certificate(vec![SettlementCheckpointSpec::new(
                1,
                states.clone(),
                Some(outcome),
                reverse_non_dead(&states),
                reverse_live(&states),
            )]);
            let decision = SettlementDecision::Accept(outcome);
            let mut transaction = certificate
                .prepare_start_transaction(NonZeroU64::new(99).unwrap())
                .unwrap();
            complete_phase_transaction(&certificate, &mut transaction, decision);
        }
    }

    #[test]
    fn phase_machine_covers_every_certified_checkpoint_and_abort_reason() {
        let checkpoints = vec![
            SettlementCheckpointSpec::new(
                1,
                vec![SettlementResourceState::Live; 3],
                None,
                vec![2, 1, 0],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                2,
                vec![
                    SettlementResourceState::Live,
                    SettlementResourceState::Live,
                    SettlementResourceState::Dead,
                ],
                None,
                vec![1, 0],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                3,
                vec![
                    SettlementResourceState::Live,
                    SettlementResourceState::ProvisionalResult,
                    SettlementResourceState::Dead,
                ],
                None,
                vec![1, 0],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                4,
                vec![
                    SettlementResourceState::Live,
                    SettlementResourceState::ProvisionalResult,
                    SettlementResourceState::Dead,
                ],
                Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
                vec![1, 0],
                vec![0],
            ),
        ];
        let progress = [
            SettlementProgressAction::Finalize { owner_ordinal: 2 },
            SettlementProgressAction::StageOwnedResult { owner_ordinal: 1 },
            SettlementProgressAction::CertifyOutcome {
                trace_evidence: [0x39; 32],
            },
        ];
        let certificate = NativeSettlementCertificate::try_new_with_progress(
            DeclarationId::new("token.phase-corpus"),
            CONTRACT,
            3,
            checkpoints,
            vec![1],
            vec![
                SettlementProgressEdge::new(1, 2, progress[0]),
                SettlementProgressEdge::new(2, 3, progress[1]),
                SettlementProgressEdge::new(3, 4, progress[2]),
            ],
        )
        .unwrap();
        let aborts = [
            AdapterAbortReason::PhysicalResult(1),
            AdapterAbortReason::PhysicalResult(u32::MAX),
            AdapterAbortReason::MalformedResponse,
            AdapterAbortReason::TraceRejected,
            AdapterAbortReason::HostUnwind,
        ];
        let mut invocation = 1_u64;
        for checkpoint in 1..=4_u32 {
            let accepts = (checkpoint == 4).then_some(SettlementDecision::Accept(
                SettlementOutcome::OwnedSuccess { owner_ordinal: 1 },
            ));
            for decision in aborts
                .into_iter()
                .map(SettlementDecision::Abort)
                .chain(accepts)
            {
                let mut transaction = certificate
                    .prepare_start_transaction(NonZeroU64::new(invocation).unwrap())
                    .unwrap();
                invocation += 1;
                for action in progress.iter().take((checkpoint - 1) as usize) {
                    certificate
                        .advance_transaction(&mut transaction, *action)
                        .unwrap();
                }
                assert_eq!(transaction.checkpoint(), checkpoint);
                let receipt = complete_phase_transaction(&certificate, &mut transaction, decision);
                assert_eq!(receipt.checkpoint(), checkpoint);
                assert_eq!(receipt.decision(), decision);
            }
        }
    }

    #[test]
    fn unwind_is_phase_aware_and_finalizer_uncertainty_is_absorbing() {
        let states = vec![SettlementResourceState::Live];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            Some(SettlementOutcome::ScalarSuccess),
            reverse_non_dead(&states),
            reverse_live(&states),
        )]);
        let mut before_lock = certificate
            .prepare_start_transaction(NonZeroU64::new(1).unwrap())
            .unwrap();
        let host_unwind = SettlementDecision::Abort(AdapterAbortReason::HostUnwind);
        assert_eq!(
            certificate.observe_transaction_unwind(&mut before_lock),
            Ok(host_unwind)
        );
        assert_eq!(
            before_lock.phase(),
            SettlementTransactionPhase::DecisionLocked
        );

        let decision = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
        let mut after_lock = certificate
            .prepare_start_transaction(NonZeroU64::new(2).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut after_lock, decision)
            .unwrap();
        assert_eq!(
            certificate.observe_transaction_unwind(&mut after_lock),
            Ok(decision)
        );

        let ticket = certificate.begin_next_finalizer(&mut after_lock).unwrap();
        assert_eq!(
            certificate.observe_transaction_unwind(&mut after_lock),
            Err(SettlementError::FinalizerCompletionUncertain)
        );
        assert_eq!(after_lock.phase(), SettlementTransactionPhase::Quarantined);
        assert_eq!(
            certificate.complete_finalizer(&mut after_lock, ticket),
            Err(SettlementError::TransactionQuarantined)
        );
        assert_eq!(
            certificate.observe_transaction_unwind(&mut after_lock),
            Err(SettlementError::TransactionQuarantined)
        );

        let mut settled = certificate
            .prepare_start_transaction(NonZeroU64::new(3).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut settled, decision)
            .unwrap();
        let ticket = certificate.begin_next_finalizer(&mut settled).unwrap();
        certificate
            .complete_finalizer(&mut settled, ticket)
            .unwrap();
        let candidate = certificate
            .finish_provider_settlement(&mut settled)
            .unwrap();
        assert_eq!(
            certificate.observe_transaction_unwind(&mut settled),
            Ok(decision)
        );
        let receipt = candidate.receipt().clone();
        certificate
            .commit_provider_receipt(&mut settled, &receipt)
            .unwrap();
        assert_eq!(
            certificate.observe_transaction_unwind(&mut settled),
            Ok(decision)
        );
    }

    #[test]
    fn conflicts_and_skips_monotonically_quarantine_without_publication() {
        let states = vec![
            SettlementResourceState::Live,
            SettlementResourceState::ProvisionalResult,
        ];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            Some(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }),
            reverse_non_dead(&states),
            reverse_live(&states),
        )]);
        let accept =
            SettlementDecision::Accept(SettlementOutcome::OwnedSuccess { owner_ordinal: 1 });
        let abort = SettlementDecision::Abort(AdapterAbortReason::MalformedResponse);

        let mut conflict = certificate
            .prepare_start_transaction(NonZeroU64::new(1).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut conflict, accept)
            .unwrap();
        assert_eq!(
            certificate.lock_transaction_decision(&mut conflict, abort),
            Err(SettlementError::ConflictingLockedDecision)
        );
        assert_eq!(conflict.phase(), SettlementTransactionPhase::Quarantined);

        let mut skipped = certificate
            .prepare_start_transaction(NonZeroU64::new(2).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut skipped, accept)
            .unwrap();
        assert_eq!(
            certificate.publish_owned_candidate(&mut skipped),
            Err(SettlementError::PublishActionNotPending)
        );
        assert_eq!(skipped.phase(), SettlementTransactionPhase::Quarantined);
        assert!(!skipped
            .resources()
            .contains(&SettlementResourceState::Published));

        let mut unfinished = certificate
            .prepare_start_transaction(NonZeroU64::new(3).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut unfinished, abort)
            .unwrap();
        assert_eq!(
            certificate.finish_provider_settlement(&mut unfinished),
            Err(SettlementError::SettlementActionsIncomplete)
        );
        assert_eq!(unfinished.phase(), SettlementTransactionPhase::Quarantined);

        let mut in_progress = certificate
            .prepare_start_transaction(NonZeroU64::new(4).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut in_progress, abort)
            .unwrap();
        let _ticket = certificate.begin_next_finalizer(&mut in_progress).unwrap();
        assert_eq!(
            certificate.lock_transaction_decision(&mut in_progress, abort),
            Err(SettlementError::InvalidSettlementPhase)
        );
        assert_eq!(in_progress.phase(), SettlementTransactionPhase::Quarantined);

        let terminal_states = vec![SettlementResourceState::Dead];
        let terminal_certificate = self::certificate(vec![SettlementCheckpointSpec::new(
            1,
            terminal_states,
            Some(SettlementOutcome::ScalarSuccess),
            Vec::new(),
            Vec::new(),
        )]);
        let terminal_decision = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
        let mut provider_settled = terminal_certificate
            .prepare_start_transaction(NonZeroU64::new(5).unwrap())
            .unwrap();
        terminal_certificate
            .lock_transaction_decision(&mut provider_settled, terminal_decision)
            .unwrap();
        terminal_certificate
            .finish_provider_settlement(&mut provider_settled)
            .unwrap();
        assert_eq!(
            terminal_certificate.lock_transaction_decision(&mut provider_settled, abort),
            Err(SettlementError::ConflictingLockedDecision)
        );
        assert_eq!(
            provider_settled.phase(),
            SettlementTransactionPhase::Quarantined
        );
    }

    #[test]
    fn forged_progress_and_cross_certificate_calls_quarantine_exact_transaction() {
        let start = SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Live],
            None,
            vec![0],
            Vec::new(),
        );
        let end = SettlementCheckpointSpec::new(
            2,
            vec![SettlementResourceState::Dead],
            None,
            Vec::new(),
            Vec::new(),
        );
        let action = SettlementProgressAction::Finalize { owner_ordinal: 0 };
        let make_certificate = |function| {
            NativeSettlementCertificate::try_new_with_progress(
                DeclarationId::new(function),
                CONTRACT,
                1,
                vec![start.clone(), end.clone()],
                vec![1],
                vec![SettlementProgressEdge::new(1, 2, action)],
            )
            .unwrap()
        };
        let certificate = make_certificate("token.progress-a");
        let other = make_certificate("token.progress-b");

        let mut forged_action = certificate
            .prepare_start_transaction(NonZeroU64::new(1).unwrap())
            .unwrap();
        assert_eq!(
            certificate.advance_transaction(
                &mut forged_action,
                SettlementProgressAction::StageOwnedResult { owner_ordinal: 0 },
            ),
            Err(SettlementError::ProgressActionNotAdmitted)
        );
        assert_eq!(
            forged_action.phase(),
            SettlementTransactionPhase::Quarantined
        );

        let mut forged_state = certificate
            .prepare_start_transaction(NonZeroU64::new(2).unwrap())
            .unwrap();
        forged_state.resources[0] = SettlementResourceState::Dead;
        assert_eq!(
            certificate.advance_transaction(&mut forged_state, action),
            Err(SettlementError::FrameStateMismatch)
        );
        assert_eq!(
            forged_state.phase(),
            SettlementTransactionPhase::Quarantined
        );

        let mut cross_bound = certificate
            .prepare_start_transaction(NonZeroU64::new(3).unwrap())
            .unwrap();
        assert_eq!(
            other.lock_transaction_decision(
                &mut cross_bound,
                SettlementDecision::Abort(AdapterAbortReason::HostUnwind),
            ),
            Err(SettlementError::FrameBindingMismatch)
        );
        assert_eq!(cross_bound.phase(), SettlementTransactionPhase::Quarantined);
        assert_eq!(
            certificate.observe_transaction_unwind(&mut cross_bound),
            Err(SettlementError::TransactionQuarantined)
        );
    }

    #[test]
    fn stale_cross_binding_and_duplicate_finalizer_completion_never_retry() {
        let states = vec![SettlementResourceState::Live];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            None,
            reverse_non_dead(&states),
            Vec::new(),
        )]);
        let decision = SettlementDecision::Abort(AdapterAbortReason::TraceRejected);
        let mut first = certificate
            .prepare_start_transaction(NonZeroU64::new(1).unwrap())
            .unwrap();
        let mut second = certificate
            .prepare_start_transaction(NonZeroU64::new(2).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut first, decision)
            .unwrap();
        certificate
            .lock_transaction_decision(&mut second, decision)
            .unwrap();
        let first_ticket = certificate.begin_next_finalizer(&mut first).unwrap();
        let second_ticket = certificate.begin_next_finalizer(&mut second).unwrap();
        assert_eq!(
            certificate.complete_finalizer(&mut first, second_ticket),
            Err(SettlementError::FinalizerTicketMismatch)
        );
        assert_eq!(first.phase(), SettlementTransactionPhase::Quarantined);
        certificate.mark_finalizer_uncertain(&mut second).unwrap();
        assert_eq!(second.phase(), SettlementTransactionPhase::Quarantined);
        assert_eq!(
            certificate.complete_finalizer(&mut first, first_ticket),
            Err(SettlementError::TransactionQuarantined)
        );

        let mut duplicate = certificate
            .prepare_start_transaction(NonZeroU64::new(3).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut duplicate, decision)
            .unwrap();
        let ticket = certificate.begin_next_finalizer(&mut duplicate).unwrap();
        let forged_duplicate = SettlementFinalizerTicket {
            certificate_fingerprint: ticket.certificate_fingerprint,
            invocation: ticket.invocation,
            checkpoint: ticket.checkpoint,
            decision: ticket.decision,
            action_index: ticket.action_index,
            owner_ordinal: ticket.owner_ordinal,
        };
        certificate
            .complete_finalizer(&mut duplicate, ticket)
            .unwrap();
        assert_eq!(
            certificate.complete_finalizer(&mut duplicate, forged_duplicate),
            Err(SettlementError::InvalidSettlementPhase)
        );
        assert_eq!(duplicate.phase(), SettlementTransactionPhase::Quarantined);
    }

    #[test]
    fn unwind_at_every_finalizer_index_preserves_prefix_current_and_suffix() {
        let states = vec![SettlementResourceState::Live; 4];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            None,
            reverse_non_dead(&states),
            Vec::new(),
        )]);
        let decision = SettlementDecision::Abort(AdapterAbortReason::MalformedResponse);
        for interruption in 0..4_usize {
            let invocation = NonZeroU64::new((interruption + 1) as u64).unwrap();
            let mut transaction = certificate.prepare_start_transaction(invocation).unwrap();
            certificate
                .lock_transaction_decision(&mut transaction, decision)
                .unwrap();
            for _ in 0..interruption {
                let ticket = certificate.begin_next_finalizer(&mut transaction).unwrap();
                certificate
                    .complete_finalizer(&mut transaction, ticket)
                    .unwrap();
            }
            let ticket = certificate.begin_next_finalizer(&mut transaction).unwrap();
            let current_owner = 3 - interruption;
            for owner in 0..4_usize {
                let expected = match owner.cmp(&current_owner) {
                    std::cmp::Ordering::Greater => SettlementResourceState::Dead,
                    std::cmp::Ordering::Equal => SettlementResourceState::Finalizing,
                    std::cmp::Ordering::Less => SettlementResourceState::Live,
                };
                assert_eq!(transaction.resources[owner], expected);
            }
            let resources_at_uncertainty = transaction.resources.clone();
            assert_eq!(
                certificate.observe_transaction_unwind(&mut transaction),
                Err(SettlementError::FinalizerCompletionUncertain)
            );
            assert_eq!(transaction.resources, resources_at_uncertainty);
            assert_eq!(transaction.phase(), SettlementTransactionPhase::Quarantined);

            let mut legacy = certificate.prepare_start_frame(invocation).unwrap();
            let receipt = certificate.settle(&mut legacy, decision).unwrap().receipt;
            assert_eq!(
                certificate.advance_transaction(
                    &mut transaction,
                    SettlementProgressAction::Finalize { owner_ordinal: 0 },
                ),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(
                certificate.lock_transaction_decision(&mut transaction, decision),
                Err(SettlementError::TransactionQuarantined)
            );
            assert!(matches!(
                certificate.begin_next_finalizer(&mut transaction),
                Err(SettlementError::TransactionQuarantined)
            ));
            assert_eq!(
                certificate.complete_finalizer(&mut transaction, ticket),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(
                certificate.mark_finalizer_uncertain(&mut transaction),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(
                certificate.publish_owned_candidate(&mut transaction),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(
                certificate.finish_provider_settlement(&mut transaction),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(
                certificate.commit_provider_receipt(&mut transaction, &receipt),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(
                certificate.replay_provider_candidate(&mut transaction, decision),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(
                certificate.replay_committed_receipt(&mut transaction, decision),
                Err(SettlementError::TransactionQuarantined)
            );
            assert_eq!(transaction.resources, resources_at_uncertainty);
        }
    }

    #[test]
    fn hostile_internal_mutations_are_detected_in_every_irreversible_phase() {
        let states = vec![SettlementResourceState::Live];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states.clone(),
            None,
            reverse_non_dead(&states),
            Vec::new(),
        )]);
        let decision = SettlementDecision::Abort(AdapterAbortReason::TraceRejected);

        let mut locked = certificate
            .prepare_start_transaction(NonZeroU64::new(1).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut locked, decision)
            .unwrap();
        locked.next_action = 1;
        assert_eq!(
            certificate.lock_transaction_decision(&mut locked, decision),
            Err(SettlementError::FrameStateMismatch)
        );
        assert_eq!(locked.phase(), SettlementTransactionPhase::Quarantined);

        let mut in_progress = certificate
            .prepare_start_transaction(NonZeroU64::new(2).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut in_progress, decision)
            .unwrap();
        let _ticket = certificate.begin_next_finalizer(&mut in_progress).unwrap();
        in_progress.next_action = 1;
        assert_eq!(
            certificate.observe_transaction_unwind(&mut in_progress),
            Err(SettlementError::FrameStateMismatch)
        );
        assert_eq!(in_progress.phase(), SettlementTransactionPhase::Quarantined);

        let terminal_certificate = self::certificate(vec![SettlementCheckpointSpec::new(
            1,
            vec![SettlementResourceState::Dead],
            Some(SettlementOutcome::ScalarSuccess),
            Vec::new(),
            Vec::new(),
        )]);
        let accepted = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
        let mut provider = terminal_certificate
            .prepare_start_transaction(NonZeroU64::new(3).unwrap())
            .unwrap();
        terminal_certificate
            .lock_transaction_decision(&mut provider, accepted)
            .unwrap();
        terminal_certificate
            .finish_provider_settlement(&mut provider)
            .unwrap();
        provider
            .candidate_receipt
            .as_mut()
            .unwrap()
            .active_finalizers = 1;
        assert_eq!(
            terminal_certificate.replay_provider_candidate(&mut provider, accepted),
            Err(SettlementError::FrameStateMismatch)
        );
        assert_eq!(provider.phase(), SettlementTransactionPhase::Quarantined);

        let mut committed = terminal_certificate
            .prepare_start_transaction(NonZeroU64::new(4).unwrap())
            .unwrap();
        complete_phase_transaction(&terminal_certificate, &mut committed, accepted);
        committed
            .committed_receipt
            .as_mut()
            .unwrap()
            .recovery_contract[0] ^= 1;
        assert_eq!(
            terminal_certificate.replay_committed_receipt(&mut committed, accepted),
            Err(SettlementError::FrameStateMismatch)
        );
        assert_eq!(committed.phase(), SettlementTransactionPhase::Quarantined);
    }

    #[test]
    fn receipt_commit_is_exact_and_quarantine_preserves_terminal_evidence() {
        let states = vec![SettlementResourceState::Dead];
        let certificate = certificate(vec![SettlementCheckpointSpec::new(
            1,
            states,
            Some(SettlementOutcome::ScalarSuccess),
            Vec::new(),
            Vec::new(),
        )]);
        let decision = SettlementDecision::Accept(SettlementOutcome::ScalarSuccess);
        let mut transaction = certificate
            .prepare_start_transaction(NonZeroU64::new(7).unwrap())
            .unwrap();
        certificate
            .lock_transaction_decision(&mut transaction, decision)
            .unwrap();
        let candidate = certificate
            .finish_provider_settlement(&mut transaction)
            .unwrap()
            .receipt;
        let mut forged = candidate.clone();
        forged.recovery_contract[0] ^= 1;
        assert_eq!(
            certificate.commit_provider_receipt(&mut transaction, &forged),
            Err(SettlementError::ReceiptCommitMismatch)
        );
        assert_eq!(transaction.phase(), SettlementTransactionPhase::Quarantined);
        assert_eq!(transaction.candidate_receipt(), Some(&candidate));

        let mut committed = certificate
            .prepare_start_transaction(NonZeroU64::new(8).unwrap())
            .unwrap();
        complete_phase_transaction(&certificate, &mut committed, decision);
        let preserved = committed.committed_receipt().unwrap().canonical_json();
        assert_eq!(
            certificate.lock_transaction_decision(
                &mut committed,
                SettlementDecision::Abort(AdapterAbortReason::MalformedResponse),
            ),
            Err(SettlementError::ConflictingLockedDecision)
        );
        assert_eq!(committed.phase(), SettlementTransactionPhase::Quarantined);
        assert_eq!(
            committed.committed_receipt().unwrap().canonical_json(),
            preserved
        );
    }

    #[test]
    fn phase_transactions_are_start_only_and_linear() {
        let checkpoints = vec![
            SettlementCheckpointSpec::new(
                1,
                vec![SettlementResourceState::Live],
                None,
                vec![0],
                Vec::new(),
            ),
            SettlementCheckpointSpec::new(
                2,
                vec![SettlementResourceState::Dead],
                None,
                Vec::new(),
                Vec::new(),
            ),
        ];
        let snapshots = certificate(checkpoints);
        assert!(matches!(
            snapshots.prepare_start_transaction(NonZeroU64::new(1).unwrap()),
            Err(SettlementError::InvalidProgressStart)
        ));
        assert_not_impl!(NativeSettlementTransaction, Clone);
        assert_not_impl!(NativeSettlementTransaction, fmt::Debug);
        assert_not_impl!(NativeSettlementTransaction, fmt::Display);
        assert_not_impl!(SettlementFinalizerTicket, Clone);
        assert_not_impl!(SettlementFinalizerTicket, fmt::Debug);
        assert_not_impl!(SettlementFinalizerTicket, fmt::Display);
    }

    #[test]
    fn frame_traits_are_deliberately_linear_and_nonformatting() {
        assert_not_impl!(NativeSettlementFrame, Clone);
        assert_not_impl!(NativeSettlementFrame, fmt::Debug);
        assert_not_impl!(NativeSettlementFrame, fmt::Display);
    }

    #[test]
    fn certificate_bounds_accept_exact_limits_and_reject_zero_over_and_excess_work() {
        fn specs(resources: usize, checkpoints: usize) -> Vec<SettlementCheckpointSpec> {
            (1..=checkpoints)
                .map(|checkpoint| {
                    SettlementCheckpointSpec::new(
                        u32::try_from(checkpoint).unwrap(),
                        vec![SettlementResourceState::Dead; resources],
                        None,
                        Vec::new(),
                        Vec::new(),
                    )
                })
                .collect()
        }
        let build = |resources, checkpoints| {
            NativeSettlementCertificate::try_new(
                DeclarationId::new("token.bounds"),
                CONTRACT,
                resources,
                specs(resources.max(1), checkpoints),
            )
        };
        assert!(build(1, 1).is_ok());
        assert!(build(MAX_SETTLEMENT_RESOURCES, 1).is_ok());
        assert_eq!(build(0, 1), Err(SettlementError::ResourceCountOutOfBounds));
        assert_eq!(
            build(MAX_SETTLEMENT_RESOURCES + 1, 1),
            Err(SettlementError::ResourceCountOutOfBounds)
        );
        assert!(build(1, MAX_SETTLEMENT_CHECKPOINTS).is_ok());
        assert_eq!(
            build(1, 0),
            Err(SettlementError::CheckpointCountOutOfBounds)
        );
        assert_eq!(
            build(1, MAX_SETTLEMENT_CHECKPOINTS + 1),
            Err(SettlementError::CheckpointCountOutOfBounds)
        );
        assert!(build(1_000, 1_000).is_ok());
        assert_eq!(
            build(1_001, 1_000),
            Err(SettlementError::WorkBudgetExceeded)
        );
    }
}
