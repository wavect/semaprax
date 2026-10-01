//! Same existing target body; actual live consumer supplies the sealed guard.
use super::*;
use std::cell::Cell;

pub(in crate::interpreter::resumable::owned_frame::registered_stage::effect) fn dispatch_activated_owned_effect_v8<
    'j,
>(
    activated: ActivatedOwnedEffectV8<'j>,
    accounting: &mut TargetAccounting,
    mut check: impl FnMut(OwnedEffectPhaseV8) -> bool,
    handler: &mut dyn TargetHostHandler,
) -> StagedOwnedEffectV8<'j> {
    let mut staged = activated.staged;
    let phase = OwnedEffectPhaseV8::Intent(staged.intent);
    let entry_guard = guard_status(
        &staged.prepared.inputs,
        staged.prepared.creator,
        phase,
        &mut check,
        staged.prepared.plan.operation().effect_id(),
        false,
    );
    if entry_guard != LiveGuardV8::Current {
        staged.failure = Some(if staged.prepared.inputs.cancellation.is_cancelled() {
            OwnedEffectFailureV8::Cancelled
        } else {
            OwnedEffectFailureV8::AuthorityLost
        });
        staged.authority_lost = entry_guard == LiveGuardV8::AuthorityLost;
        return staged;
    }
    let request = OwnedEffectTargetRequestV8 {
        grant: staged.prepared.request.grant.clone(),
        authorization: staged.prepared.request.authorization.clone(),
        operation: staged.prepared.request.operation.clone(),
        argument: staged.prepared.request.argument.clone(),
        turn: staged.prepared.request.turn,
        limits: staged.prepared.request.limits,
    };
    let permit = OwnedEffectDispatchPermitV8 {
        request,
        argument_digest: staged.prepared.basis.argument.clone(),
        budget: staged.prepared.budget,
    };
    let dispatch = target_protocol::owned_wait_v8::physical::dispatch(
        permit,
        accounting,
        staged.prepared.inputs.cancellation,
        handler,
    );
    // The target's selected failure is sticky even if a subsequent callback
    // loses authority. Retirement is separate from the selected status.
    if dispatch.evidence().settlement() != Settlement::Returned {
        staged.failure = Some(OwnedEffectFailureV8::Target(
            dispatch.evidence().settlement(),
        ));
    }
    // This guard is outside the target host panic catch and precedes accepted
    // result projection. Lost authority permanently retires physical release.
    let exit_guard = guard_status(
        &staged.prepared.inputs,
        staged.prepared.creator,
        phase,
        &mut check,
        staged.prepared.plan.operation().effect_id(),
        true,
    );
    if exit_guard != LiveGuardV8::Current {
        let cancelled = staged.prepared.inputs.cancellation.is_cancelled();
        staged.authority_lost = exit_guard == LiveGuardV8::AuthorityLost;
        staged.failure.get_or_insert(if cancelled {
            OwnedEffectFailureV8::Cancelled
        } else {
            OwnedEffectFailureV8::AuthorityLost
        });
    } else if dispatch.evidence().settlement() == Settlement::Returned {
        staged.accepted = dispatch
            .result()
            .and_then(|carrier| staged.prepared.plan.accepted_result(carrier.payload()));
        if staged.accepted.is_none() {
            staged.failure = Some(OwnedEffectFailureV8::ResultShape);
        }
    }
    staged.dispatch = Some(dispatch);
    staged
}
/// The source actor cannot replace the live guard with an arbitrary callback.
/// The existing dispatcher body retains target/accounting/failure ordering.
pub(crate) fn dispatch_live_owned_effect_v8<'j>(
    activated: ActivatedOwnedEffectV8<'j>,
    accounting: &mut TargetAccounting,
    permit: &LiveEffectIntentPermitV8<'_, 'j>,
    handler: &mut dyn TargetHostHandler,
) -> (StagedOwnedEffectV8<'j>, Option<SourceJournalError>) {
    dispatch_with_guard(
        activated,
        accounting,
        LiveIntentGuardV8::Initial(permit),
        handler,
    )
}
pub(crate) fn dispatch_continued_owned_effect_v8<'j>(
    activated: ActivatedOwnedEffectV8<'j>,
    accounting: &mut TargetAccounting,
    permit: &crate::live_invocation::source_journal::LiveContinuedIntentPermitV8<'_, 'j>,
    handler: &mut dyn TargetHostHandler,
) -> (StagedOwnedEffectV8<'j>, Option<SourceJournalError>) {
    dispatch_with_guard(
        activated,
        accounting,
        LiveIntentGuardV8::Continued(permit),
        handler,
    )
}
fn dispatch_with_guard<'j>(
    activated: ActivatedOwnedEffectV8<'j>,
    accounting: &mut TargetAccounting,
    permit: LiveIntentGuardV8<'_, '_, 'j>,
    handler: &mut dyn TargetHostHandler,
) -> (StagedOwnedEffectV8<'j>, Option<SourceJournalError>) {
    let selected = Cell::new(None);
    let staged = dispatch_activated_owned_effect_v8(
        activated,
        accounting,
        |phase| {
            let result = if matches!(phase, OwnedEffectPhaseV8::Intent(_)) {
                permit.validate_current()
            } else {
                Err(SourceJournalError::Binding)
            };
            match result {
                Ok(()) => true,
                Err(error) => {
                    if selected.get().is_none() {
                        selected.set(Some(error));
                    }
                    false
                }
            }
        },
        handler,
    );
    (staged, selected.get())
}

impl StagedOwnedEffectV8<'_> {
    pub(crate) fn quarantine_live_dispatch(&self) {
        self.prepared.quarantine_live_authorization();
    }
    pub(crate) fn validate_live_dispatch(
        &self,
        permit: &LiveEffectIntentPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        self.prepared.validate_live_intent(permit)
    }
    #[cfg(test)]
    pub(crate) fn live_test_retired(&self) -> bool {
        self.authority_lost
    }
}

impl StagedOwnedEffectV8<'_> {
    pub(crate) fn validate_continued_dispatch(
        &self,
        permit: &crate::live_invocation::source_journal::LiveContinuedIntentPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        self.prepared
            .validate_intent_guard(&LiveIntentGuardV8::Continued(permit))
    }
}
