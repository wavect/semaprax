//! Target-neutral model for callable-v3 recovery settlement.
//!
//! This module deliberately contains no loader, FFI, host-ledger, or codegen
//! integration. It fixes the bounded state machine and independently
//! authenticatable evidence that those later layers must preserve. Ordinary
//! native resource emission remains blocked by `SPX-B104`.
//!
//! `NativeSettlementFrame` is non-cloneable, but this pure model is not an
//! invocation-reservation authority: repeated calls to `prepare_frame` with
//! equal inputs deliberately produce equal model states. A future host ledger
//! must bind one frame generation to one exact module instance and reject a
//! duplicate invocation before ownership commit.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;

use sha2::{Digest, Sha256};

use crate::diagnostic::quote_json;
use crate::hir::DeclarationId;

pub const NATIVE_SETTLEMENT_CERTIFICATE_V1: &str = "semaprax.native-settlement-certificate.v1";
pub const NATIVE_SETTLEMENT_RECEIPT_V1: &str = "semaprax.native-settlement-receipt.v1";

pub const MAX_SETTLEMENT_RESOURCES: usize = 4_096;
pub const MAX_SETTLEMENT_CHECKPOINTS: usize = 65_536;
const MAX_SETTLEMENT_WORK_UNITS: usize = 1_000_000;
const CERTIFICATE_FINGERPRINT_DOMAIN: &[u8] =
    b"semaprax.native-settlement-certificate-fingerprint.v1\0";
const RECEIPT_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-settlement-receipt-fingerprint.v1\0";

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSettlementCertificate {
    schema: &'static str,
    function: DeclarationId,
    call_contract: [u8; 32],
    resource_count: usize,
    checkpoints: Vec<SettlementCheckpointSpec>,
}

impl NativeSettlementCertificate {
    pub fn try_new(
        function: DeclarationId,
        call_contract: [u8; 32],
        resource_count: usize,
        checkpoints: Vec<SettlementCheckpointSpec>,
    ) -> Result<Self, SettlementError> {
        if function.as_str().is_empty() || function.as_str().as_bytes().contains(&0) {
            return Err(SettlementError::InvalidFunctionIdentity);
        }
        if call_contract.iter().all(|byte| *byte == 0) {
            return Err(SettlementError::ZeroCallContract);
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
        Ok(Self {
            schema: NATIVE_SETTLEMENT_CERTIFICATE_V1,
            function,
            call_contract,
            resource_count,
            checkpoints,
        })
    }

    #[must_use]
    pub fn function(&self) -> &DeclarationId {
        &self.function
    }

    #[must_use]
    pub const fn call_contract(&self) -> [u8; 32] {
        self.call_contract
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
    pub fn canonical_json(&self) -> String {
        let checkpoints = self
            .checkpoints
            .iter()
            .map(checkpoint_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"function\":{},\"call_contract\":\"{}\",\"resource_count\":{},\"checkpoints\":[{}]}}",
            quote_json(self.schema),
            quote_json(self.function.as_str()),
            hex(&self.call_contract),
            self.resource_count,
            checkpoints,
        )
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(
            CERTIFICATE_FINGERPRINT_DOMAIN,
            self.canonical_json().as_bytes(),
        )
    }

    /// Construct a deterministic model frame for one checkpoint.
    ///
    /// This method does not reserve the invocation or establish process-local
    /// uniqueness. Runtime wiring must perform that linear reservation before
    /// exposing a frame to physical settlement.
    pub fn prepare_frame(
        &self,
        invocation: NonZeroU64,
        checkpoint: u32,
    ) -> Result<NativeSettlementFrame, SettlementError> {
        let checkpoint = self.checkpoint(checkpoint)?;
        Ok(NativeSettlementFrame {
            function: self.function.clone(),
            call_contract: self.call_contract,
            certificate_fingerprint: self.fingerprint(),
            invocation,
            checkpoint: checkpoint.checkpoint,
            resources: checkpoint.resources.clone(),
            terminal: None,
        })
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
            schema: NATIVE_SETTLEMENT_RECEIPT_V1,
            function: self.function.clone(),
            call_contract: self.call_contract,
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
        if receipt.schema != NATIVE_SETTLEMENT_RECEIPT_V1 {
            return Err(SettlementError::ReceiptSchemaMismatch);
        }
        if receipt.function != self.function
            || receipt.call_contract != self.call_contract
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
            || frame.call_contract != self.call_contract
            || frame.certificate_fingerprint != self.fingerprint()
        {
            return Err(SettlementError::FrameBindingMismatch);
        }
        if frame.resources.len() != self.resource_count {
            return Err(SettlementError::FrameStateMismatch);
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
/// `prepare_frame` constructor a uniqueness authority. A runtime integration
/// must additionally bind one frame generation to one exact module instance
/// and committed ledger invocation.
///
/// ```compile_fail
/// use semaprax::native_settlement::NativeSettlementFrame;
/// fn duplicate(frame: NativeSettlementFrame) { let _ = frame.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax::native_settlement::NativeSettlementFrame;
/// fn expose(frame: NativeSettlementFrame) { let _ = format!("{frame:?}"); }
/// ```
#[derive(Eq, PartialEq)]
pub struct NativeSettlementFrame {
    function: DeclarationId,
    call_contract: [u8; 32],
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
    call_contract: [u8; 32],
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
            "{{\"schema\":{},\"function\":{},\"call_contract\":\"{}\",\"certificate_fingerprint\":\"{}\",\"invocation\":{},\"checkpoint\":{},\"decision\":{},\"actions\":[{}],\"dispositions\":[{}],\"active_finalizers\":{}}}",
            quote_json(self.schema),
            quote_json(self.function.as_str()),
            hex(&self.call_contract),
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
    ZeroCallContract,
    ResourceCountOutOfBounds,
    CheckpointCountOutOfBounds,
    WorkBudgetExceeded,
    NonCanonicalCheckpoint,
    CheckpointResourceCountMismatch,
    InvalidCheckpointState,
    MultipleProvisionalResults,
    InvalidCleanupOrder,
    InvalidNormalOutcome,
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
}

impl fmt::Display for SettlementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFunctionIdentity => {
                "settlement function identity is empty or contains NUL"
            }
            Self::ZeroCallContract => "settlement call contract is zero",
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
        })
    }
}

impl Error for SettlementError {}

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

    const CONTRACT: [u8; 32] = [0x5a; 32];

    #[derive(Debug, Eq, PartialEq)]
    struct FrameSnapshot {
        function: DeclarationId,
        call_contract: [u8; 32],
        certificate_fingerprint: [u8; 32],
        invocation: NonZeroU64,
        checkpoint: u32,
        resources: Vec<SettlementResourceState>,
        terminal: Option<(SettlementDecision, String)>,
    }

    fn snapshot(frame: &NativeSettlementFrame) -> FrameSnapshot {
        FrameSnapshot {
            function: frame.function.clone(),
            call_contract: frame.call_contract,
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
                    states.push(if encoded % 2 == 0 {
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
                            states.push(if encoded % 2 == 0 {
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
            Err(SettlementError::ZeroCallContract)
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
        receipt.call_contract[0] ^= 1;
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
}
