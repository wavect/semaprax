//! Sole helper entry from the actual continued prepared owner. The opaque
//! evaluation outcome has no public constructor, owner getter, or parts API.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::{
    begin_owned_copy_wait_v2, OwnedCopyWaitStepV2,
};
use crate::live_invocation::source_journal::LiveWaitStartPermitV8;

/// Fields are confined to the trusted engine stage so the fixed live-run
/// delegate can map its actual outcome. Source callers cannot detach them.
pub(in crate::interpreter::resumable::owned_frame::registered_stage) struct EvaluatedContinuedWaitV2<
    'j,
> {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) outcome:
        OwnedCopyWaitStepV2,
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) consumed: u64,
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) predecessor:
        PreparedHeldContinuedWaitV2<'j>,
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) guard_error:
        Option<SourceJournalError>,
}
pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn enter_continued_wait_v8<
    'j,
>(
    permit: LiveWaitStartPermitV8<'_>,
    mut owner: PreparedHeldContinuedWaitV2<'j>,
) -> Result<EvaluatedContinuedWaitV2<'j>, PreparedHeldContinuedWaitV2<'j>> {
    if !permit.matches_held_store(&owner.context.store)
        || permit.validate_guard().is_err()
        || owner.validate_store().is_err()
    {
        return Err(owner);
    }
    let fuel = permit.fuel();
    if fuel != owner.context.execution.evaluation_fuel() {
        owner.context.store.quarantine();
        return Err(owner);
    }
    let prepared = owner
        .prepared
        .take()
        .expect("actual continued prepared owner");
    let mut budget = OwnedFrameBudget::new(fuel).expect("checked full positive F");
    // No source entry before the actual permit's reservation guard succeeds.
    if permit.validate_guard().is_err() || !owner.context.validate_guard() {
        owner.prepared = Some(prepared);
        owner.context.store.quarantine();
        return Err(owner);
    }
    let outcome = match begin_owned_copy_wait_v2(prepared, &mut budget) {
        Ok(step) => step,
        Err(prepared) => {
            owner.prepared = Some(prepared);
            return Err(owner);
        }
    };
    let guard_error = permit
        .validate_guard()
        .err()
        .or_else(|| (!owner.context.validate_guard()).then_some(SourceJournalError::Binding));
    if guard_error.is_some() {
        owner.context.store.quarantine();
    }
    Ok(EvaluatedContinuedWaitV2 {
        outcome,
        consumed: budget.consumed() as u64,
        predecessor: owner,
        guard_error,
    })
}
impl PreparedHeldContinuedWaitV2<'_> {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn validate_retained_context(
        &self,
    ) -> Result<(), SourceJournalError> {
        if self.context.validate_guard() {
            Ok(())
        } else {
            self.context.store.quarantine();
            Err(SourceJournalError::Binding)
        }
    }
}
