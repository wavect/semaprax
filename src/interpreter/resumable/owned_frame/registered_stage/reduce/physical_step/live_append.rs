//! Actual staged/ReadyStep consumers. Only a sealed owner-bound source permit
//! supplies cleanup/transfer ACK lineage; decoded rows cannot call this route.
use super::*;
use crate::live_invocation::source_journal::{LiveOwnedStepTransferPermitV8, SourceJournalError};
use serde_json::{json, Value as Json};

pub(crate) enum LiveOwnedReduceCleanupFailureV8<'j> {
    Before {
        staged: StagedExecutedOwnedReduceV2<'j>,
        error: SourceJournalError,
    },
    Engine {
        rejected: ExecutedOwnedReduceCleanupRejectionV2<'j>,
        error: SourceJournalError,
    },
    After {
        released: ExecutedOwnedReduceSettledV2<'j>,
        error: SourceJournalError,
    },
}
/// Only source permits bound to a live owner implement the cleanup gate.
pub(crate) trait LiveOwnedReduceCleanupGuardV8 {
    fn cleanup_origin(&self) -> Result<OwnedReduceCleanupOriginV8, SourceJournalError>;
    fn validate_cleanup_current(&self) -> Result<(), SourceJournalError>;
    fn validate_staged(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
        facts: &CheckedLiveOwnedReduceStageFactsV8,
    ) -> Result<(), SourceJournalError>;
}
pub(crate) fn settle_live_owned_reduce_v8<'j>(
    staged: StagedExecutedOwnedReduceV2<'j>,
    permit: &impl LiveOwnedReduceCleanupGuardV8,
    observe: impl FnMut(&FinalizeAction),
) -> Result<ExecutedOwnedReduceSettledV2<'j>, LiveOwnedReduceCleanupFailureV8<'j>> {
    let checked = (|| {
        let facts = staged
            .live_stage_facts(staged.inputs.execution.wait())
            .map_err(|_| SourceJournalError::Binding)?;
        permit.validate_staged(&staged.inputs, &facts)?;
        let origin = permit.cleanup_origin()?;
        if matches!(origin, OwnedReduceCleanupOriginV8::CompilerEmpty { .. })
            && facts.step().is_none()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(origin)
    })();
    let origin = match checked {
        Ok(origin) => origin,
        Err(error) => return Err(LiveOwnedReduceCleanupFailureV8::Before { staged, error }),
    };
    // Sole production origin-carrier constructor: actual owner and sealed live
    // ACK/proven compiler-empty basis stay joined, never a raw cursor factory.
    let committed = CommittedExecutedOwnedReduceCleanupV2 {
        staged,
        started: origin,
        observations: Vec::new(),
    };
    let mut guard_error = None;
    let released = settle_executed_owned_reduce_v2(
        committed,
        || match permit.validate_cleanup_current() {
            Ok(()) => true,
            Err(e) => {
                guard_error = guard_error.or(Some(e));
                false
            }
        },
        observe,
    );
    match released {
        Err(rejected) => Err(LiveOwnedReduceCleanupFailureV8::Engine {
            rejected,
            error: guard_error.unwrap_or(SourceJournalError::Binding),
        }),
        Ok(released) => match permit.validate_cleanup_current() {
            Ok(()) => Ok(released),
            Err(error) => Err(LiveOwnedReduceCleanupFailureV8::After { released, error }),
        },
    }
}

pub(crate) enum LiveOwnedStepTransferFailureV8<'j> {
    Before {
        ready: ReadyExecutedOwnedStepV2<'j>,
        error: SourceJournalError,
    },
    Engine {
        rejected: ExecutedOwnedStepTransferRejectionV2<'j>,
        error: SourceJournalError,
    },
    After {
        held: HeldExecutedOwnedStepV2<'j>,
        error: SourceJournalError,
    },
}
pub(crate) fn consume_live_owned_step_v8<'j>(
    ready: ReadyExecutedOwnedStepV2<'j>,
    permit: &LiveOwnedStepTransferPermitV8<'_, 'j>,
) -> Result<HeldExecutedOwnedStepV2<'j>, LiveOwnedStepTransferFailureV8<'j>> {
    let checked = (|| {
        let inputs = ready.inputs.as_ref().ok_or(SourceJournalError::Binding)?;
        permit.validate_ready(inputs, &ready.live_receipt_v8()?, ready.cleanup_started)?;
        permit.transfer_reserved()
    })();
    let reserved = match checked {
        Ok(reserved) => reserved,
        Err(error) => return Err(LiveOwnedStepTransferFailureV8::Before { ready, error }),
    };
    // Sole production transfer-origin constructor: the actual ReadyStep is not
    // replaced by its JSON projection or reconstructed after an uncertain ACK.
    let committed = CommittedExecutedOwnedStepTransferV2 { ready, reserved };
    let mut guard_error = None;
    let held =
        consume_executed_owned_step_v2(committed, || match permit.validate_transfer_current() {
            Ok(()) => true,
            Err(e) => {
                guard_error = guard_error.or(Some(e));
                false
            }
        });
    match held {
        Err(rejected) => Err(LiveOwnedStepTransferFailureV8::Engine {
            rejected,
            error: guard_error.unwrap_or(SourceJournalError::Binding),
        }),
        Ok(held) => match permit.validate_transfer_current() {
            Ok(()) => Ok(held),
            Err(error) => Err(LiveOwnedStepTransferFailureV8::After { held, error }),
        },
    }
}
fn actual_receipt(
    observed: &[OwnedReduceObservationV2],
    valid: bool,
) -> Result<Json, SourceJournalError> {
    if !valid {
        return Err(SourceJournalError::Binding);
    }
    let entries = observed.iter().map(|o| {
        let operation = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
            std::slice::from_ref(&o.operation)).map_err(|_| SourceJournalError::Binding)?;
        let operation = operation.as_array().and_then(|v| v.first()).ok_or(SourceJournalError::Binding)?;
        Ok(json!({"operation":operation,"outcome":if o.succeeded {"completed"} else {"failed"}}))
    }).collect::<Result<Vec<_>, SourceJournalError>>()?;
    Ok(
        json!({"kind":"observed","settlement":if observed.iter().all(|o| o.succeeded) {"completed"} else {"failed"},"operations":entries}),
    )
}
impl ReadyExecutedOwnedStepV2<'_> {
    pub(crate) fn live_receipt_v8(&self) -> Result<Json, SourceJournalError> {
        actual_receipt(&self.observations, self.receipt_valid)
    }
}
impl FailedExecutedOwnedReduceV2<'_> {
    pub(crate) fn live_receipt_v8(&self) -> Result<Json, SourceJournalError> {
        // The existing failure body exposes the exact active action inventory;
        // equality proves complete capture even when an observer failed.
        let valid = self
            .operations
            .iter()
            .eq(self.observations.iter().map(|o| &o.operation));
        actual_receipt(&self.observations, valid)
    }
}

impl HeldExecutedOwnedStepV2<'_> {
    /// Borrowed identity projection from the actual moved root. It cannot
    /// construct/re-admit an owner, and is compared to the checked field map.
    pub(crate) fn live_target_v8(&self) -> Result<Json, SourceJournalError> {
        if !self.validate_store() {
            return Err(SourceJournalError::Binding);
        }
        let inputs = self.inputs.as_ref().ok_or(SourceJournalError::Binding)?;
        let (kind, key, root) = match self.owner.as_ref().ok_or(SourceJournalError::Binding)? {
            OwnedStepTransferV2::Continue(s) => ("continue", "state", s.root.as_ref()),
            OwnedStepTransferV2::Suspend(s) => ("suspend", "state", s.root.as_ref()),
            OwnedStepTransferV2::Complete(s) => ("complete", "report", s.root.as_ref()),
            OwnedStepTransferV2::Fail(code) => return Ok(json!({"kind":"fail","code":code})),
        };
        let Some(Value::Record(record)) = root else {
            return Err(SourceJournalError::Binding);
        };
        let declarations = &inputs.execution.wait().helper().program().declarations;
        let fields = declarations
            .record_fields(&record.record)
            .ok_or(SourceJournalError::Binding)?;
        if fields.len() != record.fields.len() {
            return Err(SourceJournalError::Binding);
        }
        let fields = fields.iter().map(|field| {
            let actual = record.fields.get(&field.id).ok_or(SourceJournalError::Binding)?;
            let value = match actual {
                Value::Bytes(bytes) => json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(&bytes.bytes)}),
                scalar => crate::interpreter::resumable::checkpoint::scalar_json(
                    &crate::interpreter::resumable::argument_of(scalar).ok_or(SourceJournalError::Binding)?),
            };
            Ok(json!({"identity":field.id.as_str(),"value":value}))
        }).collect::<Result<Vec<_>,SourceJournalError>>()?;
        let mut target = serde_json::Map::new();
        target.insert("kind".into(), kind.into());
        target.insert(
            key.into(),
            json!({"declaration":record.record.as_str(),"fields":fields}),
        );
        Ok(Json::Object(target))
    }
}
