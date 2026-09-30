//! Existing release and Outcome primitives consume only actual fixed cleanup
//! ACK lineage. The source permit has no raw constructor or caller bool guard.
use super::*;
use crate::live_invocation::source_journal::LiveEffectDecisionCleanupPermitV8;
use std::cell::Cell;

pub(crate) enum LiveEffectDecisionReleaseFailureV8<'j> {
    Before {
        staged: StagedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Engine {
        rejected: OwnedEffectReleaseRejectionV8<'j>,
        error: SourceJournalError,
    },
    After {
        pending: PendingOwnedEffectReceiptV8<'j>,
        error: SourceJournalError,
    },
}
pub(crate) fn release_live_owned_effect_decision_v8<'j>(
    staged: StagedOwnedEffectV8<'j>,
    permit: &LiveEffectDecisionCleanupPermitV8<'_, 'j>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<PendingOwnedEffectReceiptV8<'j>, LiveEffectDecisionReleaseFailureV8<'j>> {
    release_with_guard_v8(staged, DecisionCleanupGuardV8::Initial(permit), observe)
}
pub(crate) fn release_with_guard_v8<'j>(
    staged: StagedOwnedEffectV8<'j>,
    permit: DecisionCleanupGuardV8<'_, '_, 'j>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<PendingOwnedEffectReceiptV8<'j>, LiveEffectDecisionReleaseFailureV8<'j>> {
    let valid = (|| {
        permit.validate_guard(&staged.prepared.inputs)?;
        if staged.cleanup_started || staged.authority_lost {
            return Err(SourceJournalError::Binding);
        }
        let (basis, budget) = checked_basis(&staged.prepared.inputs, &staged.prepared.owner)
            .ok_or(SourceJournalError::Binding)?;
        if basis != staged.prepared.basis || budget != staged.prepared.budget {
            return Err(SourceJournalError::Binding);
        }
        let dispatch = staged
            .dispatch
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        if !permit.matches_settlement(
            staged.intent,
            dispatch.evidence().digest(),
            staged.observation(),
            staged.reason(),
        ) {
            return Err(SourceJournalError::Binding);
        }
        Ok((permit.references()?, permit.operations()?.clone()))
    })();
    let ((staged_ref, ready, consumed, intent, settlement_ref, recorded, started), operations) =
        match valid {
            Ok(refs) => refs,
            Err(error) => return Err(LiveEffectDecisionReleaseFailureV8::Before { staged, error }),
        };
    let settlement = OwnedEffectSettlementAckV8 {
        basis: staged.prepared.basis.clone(),
        intent,
        settlement: settlement_ref,
        evidence: staged
            .dispatch
            .as_ref()
            .unwrap()
            .evidence()
            .digest()
            .to_owned(),
        operation: staged.prepared.plan.operation().operation_id().into(),
        observation: staged.observation().map(<[u8]>::to_vec),
        reason: staged.reason(),
    };
    let start = OwnedEffectCleanupStartedAckV8 {
        basis: staged.prepared.basis.clone(),
        settlement: settlement_ref,
        recorded,
        evidence: settlement.evidence.clone(),
        started,
        operations,
        staged: staged_ref,
        ready,
        consumed,
        intent,
    };
    let error = Cell::new(None);
    let result = release_owned_effect_decision_v8(
        staged,
        settlement,
        start,
        |phase| {
            let result = if phase == OwnedEffectPhaseV8::CleanupStarted(started) {
                permit.validate_cleanup_current()
            } else {
                Err(SourceJournalError::Binding)
            };
            match result {
                Ok(()) => true,
                Err(e) => {
                    error.set(error.get().or(Some(e)));
                    false
                }
            }
        },
        observe,
    );
    match result {
        Err(rejected) => Err(LiveEffectDecisionReleaseFailureV8::Engine {
            rejected,
            error: error.get().unwrap_or(SourceJournalError::Binding),
        }),
        Ok(pending) => match permit.validate_guard(&pending.inputs) {
            Ok(()) => Ok(pending),
            Err(error) => Err(LiveEffectDecisionReleaseFailureV8::After { pending, error }),
        },
    }
}
pub(crate) enum LiveEffectOutcomeFailureV8<'j> {
    Before {
        pending: PendingOwnedEffectReceiptV8<'j>,
        error: SourceJournalError,
    },
    Engine {
        rejected: OwnedEffectCompletionRejectionV8<'j>,
        error: SourceJournalError,
    },
    After {
        executed: ExecutedOwnedAgentTurnV2<'j>,
        error: SourceJournalError,
    },
}
pub(crate) fn ack_live_owned_effect_cleanup_v8<'j>(
    pending: PendingOwnedEffectReceiptV8<'j>,
    permit: &LiveEffectDecisionCleanupPermitV8<'_, 'j>,
) -> Result<ExecutedOwnedAgentTurnV2<'j>, LiveEffectOutcomeFailureV8<'j>> {
    let valid = (|| {
        permit.validate_outcome_guard(&pending.inputs)?;
        let (started, settled, receipt) = permit.settled_receipt()?;
        if pending.started != started
            || pending.receipt != *receipt
            || pending.failure.is_some()
            || pending.accepted.is_none()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(OwnedEffectCleanupSettledAckV8 {
            basis: pending.basis.clone(),
            started,
            settled,
            receipt: receipt.clone(),
        })
    })();
    let ack = match valid {
        Ok(ack) => ack,
        Err(error) => return Err(LiveEffectOutcomeFailureV8::Before { pending, error }),
    };
    let error = Cell::new(None);
    let result = ack_owned_effect_cleanup_v8(pending, ack, |phase| {
        let result = match phase {
            OwnedEffectPhaseV8::CleanupSettled(_) => permit.validate_outcome_current(),
            _ => Err(SourceJournalError::Binding),
        };
        match result {
            Ok(()) => true,
            Err(e) => {
                error.set(error.get().or(Some(e)));
                false
            }
        }
    });
    match result {
        Err(rejected) => Err(LiveEffectOutcomeFailureV8::Engine {
            rejected,
            error: error.get().unwrap_or(SourceJournalError::Binding),
        }),
        Ok(executed) => match permit.validate_outcome_guard(&executed.inputs) {
            Ok(()) => Ok(executed),
            Err(error) => Err(LiveEffectOutcomeFailureV8::After { executed, error }),
        },
    }
}
impl StagedOwnedEffectV8<'_> {
    pub(crate) fn continued_decision_cleanup_values_v8(
        &self,
        permit: &crate::live_invocation::source_journal::LiveContinuedSettlementPermitV8<'_, '_>,
    ) -> Result<(serde_json::Value, serde_json::Value), SourceJournalError> {
        permit.validate_guard(&self.prepared.inputs)?;
        let values = self.live_decision_cleanup_values_v8()?;
        permit.validate_guard(&self.prepared.inputs)?;
        Ok(values)
    }
    /// Inert compiler projection from the still-live exact Decision. No release
    /// or ACK authority is created by these canonical values.
    pub(crate) fn live_decision_cleanup_values_v8(
        &self,
    ) -> Result<(serde_json::Value, serde_json::Value), SourceJournalError> {
        if self.cleanup_started
            || self.authority_lost
            || self.prepared.creator != std::process::id()
        {
            return Err(SourceJournalError::Binding);
        }
        let (basis, budget) = checked_basis(&self.prepared.inputs, &self.prepared.owner)
            .ok_or(SourceJournalError::Binding)?;
        if basis != self.prepared.basis || budget != self.prepared.budget {
            return Err(SourceJournalError::Binding);
        }
        let operations =
            owned_wait_operations_v8(self.prepared.inputs.execution.wait().authorize().disposal())
                .map_err(|_| SourceJournalError::Binding)?;
        Ok((basis.decision, operations))
    }
}

#[cfg(test)]
impl ExecutedOwnedAgentTurnV2<'_> {
    pub(crate) fn test_live_outcome_weak_v8(&self) -> std::sync::Weak<[u8]> {
        let Value::Record(outcome) = self.roots.outcome.as_ref().expect("actual Outcome") else {
            panic!("nominal Outcome");
        };
        let metadata = self.binding.lifecycle().owned_wait_outcome_v8();
        let Value::Bytes(bytes) = &outcome.fields[&metadata.bytes_field] else {
            panic!("actual owned payload");
        };
        std::sync::Arc::downgrade(&bytes.bytes)
    }
}

/// Test data for an independent ordinary evaluator oracle. Borrowing this
/// projection never clones the physical backing or re-admits a live owner.
#[cfg(test)]
impl ExecutedOwnedAgentTurnV2<'_> {
    pub(crate) fn test_reduce_arguments_v8(
        &self,
    ) -> Vec<crate::interpreter::retained_call::RetainedValue> {
        use crate::interpreter::retained_call::{
            RetainedField, RetainedRecord, RetainedValue as R,
        };
        let project = |value: &Value| {
            let Value::Record(record) = value else {
                panic!("actual flat record")
            };
            let fields = self
                .roots
                .helper
                .program()
                .declarations
                .record_fields(&record.record)
                .unwrap();
            R::Record(RetainedRecord {
                record: record.record.clone(),
                fields: fields
                    .iter()
                    .map(|field| {
                        let value = match &record.fields[&field.id] {
                            Value::Bytes(bytes) => R::Bytes(bytes.bytes.to_vec()),
                            Value::Int(value) => R::I64(*value),
                            Value::Int32(value) => R::I32(*value),
                            Value::Bool(value) => R::Bool(*value),
                            Value::Uint8(value) => R::U8(*value),
                            Value::Usize(value) => R::Usize(*value),
                            _ => panic!("actual admitted test leaf"),
                        };
                        RetainedField {
                            field: field.id.clone(),
                            value,
                        }
                    })
                    .collect(),
            })
        };
        let mut args = vec![project(self.roots.state.as_ref().unwrap())];
        let ResumableChannelValue::Record { fields, .. } = &self.roots.proposal else {
            panic!("actual Copy Proposal")
        };
        args.extend(fields.iter().map(|field| match field {
            ArgumentValue::Int(value) => R::I64(*value),
            ArgumentValue::Int32(value) => R::I32(*value),
            ArgumentValue::Bool(value) => R::Bool(*value),
            ArgumentValue::Uint8(value) => R::U8(*value),
            ArgumentValue::Usize(value) => R::Usize(*value),
            _ => panic!("actual admitted test Proposal leaf"),
        }));
        args.push(project(self.roots.outcome.as_ref().unwrap()));
        args
    }
}

mod failed_state;
pub(crate) use failed_state::{
    release_live_failed_effect_state_v8, LiveFailedEffectStateReleaseFailureV8,
    ReleasedFailedEffectStateV8,
};

mod observer_failed_state;

pub(crate) use observer_failed_state::{
    release_live_observer_failed_state_v8, LiveObserverFailedStateReleaseFailureV8,
    ReleasedObserverFailedStateV8,
};

#[path = "cleanup/continued_guard.rs"]
mod continued_guard;
pub(crate) use continued_guard::DecisionCleanupGuardV8;
