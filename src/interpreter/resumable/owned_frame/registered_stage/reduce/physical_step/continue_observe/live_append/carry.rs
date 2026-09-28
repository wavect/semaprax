//! Pure consuming preparation of the actual continued State. Preparation grants
//! no evaluator, model, append, restoration or target-entry authority.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::observe::prepare_observed_owned_copy_wait_v2;
use crate::interpreter::resumable::owned_frame::registered_stage::PreparedOwnedCopyWaitV2;

pub(crate) struct PreparedHeldContinuedWaitV2<'j> {
    prepared: PreparedOwnedCopyWaitV2,
    context: HeldOwnedTurnContextV2<'j>,
    turn: u32,
    transition: u32,
    reservation: u32,
    consumed: usize,
}
impl Drop for PreparedHeldContinuedWaitV2<'_> {
    fn drop(&mut self) {
        // Durable abandonment is backing-only. The retained context/store
        // outlives the physical root; no foundation semantic receipt is minted.
        drop(self.prepared.argument.root.take());
    }
}
pub(crate) enum ContinuedWaitPreparationFailureV8<'j> {
    Before {
        owner: ContinuedOwnedObserveV2<'j>,
        error: SourceJournalError,
    },
    After {
        owner: PreparedHeldContinuedWaitV2<'j>,
        error: SourceJournalError,
    },
}
/// Consumes only an actual live Observe holder. Raw State/JSON/field facts
/// cannot construct it, and no authority is gained by preparing the helper.
pub(crate) fn prepare_continued_copy_wait_v8<'j>(
    outcome: ContinuedOwnedObserveV2<'j>,
) -> Result<PreparedHeldContinuedWaitV2<'j>, ContinuedWaitPreparationFailureV8<'j>> {
    let valid = matches!(&outcome, ContinuedOwnedObserveV2::Observed(o)
        if o.validate_store() && o.observed.live_state_facts_v8().is_ok());
    if !valid {
        return Err(ContinuedWaitPreparationFailureV8::Before {
            owner: outcome,
            error: SourceJournalError::Binding,
        });
    }
    let ContinuedOwnedObserveV2::Observed(owner) = outcome else {
        unreachable!("checked actual Observe success")
    };
    let ObservedHeldOwnedStateV2 {
        observed,
        context,
        turn,
        transition,
        reservation,
        consumed,
    } = owner;
    let prepared = match prepare_observed_owned_copy_wait_v2(observed) {
        Ok(prepared) => prepared,
        Err(observed) => {
            return Err(ContinuedWaitPreparationFailureV8::Before {
                owner: ContinuedOwnedObserveV2::Observed(ObservedHeldOwnedStateV2 {
                    observed,
                    context,
                    turn,
                    transition,
                    reservation,
                    consumed,
                }),
                error: SourceJournalError::Binding,
            })
        }
    };
    let actual = PreparedHeldContinuedWaitV2 {
        prepared,
        context,
        turn,
        transition,
        reservation,
        consumed,
    };
    if let Err(error) = actual.validate_store() {
        actual.context.store.quarantine();
        return Err(ContinuedWaitPreparationFailureV8::After {
            owner: actual,
            error,
        });
    }
    Ok(actual)
}
impl PreparedHeldContinuedWaitV2<'_> {
    pub(crate) fn validate_store(&self) -> Result<(), SourceJournalError> {
        let argument = &self.prepared.argument;
        let valid = self.context.validate_guard()
            && argument.creator == std::process::id()
            && argument.root.as_ref().is_some_and(|root| {
                crate::interpreter::resumable::owned_frame::registered_stage::root_valid(
                    &argument.plan,
                    root,
                ) && argument
                    .allocations
                    .as_ref()
                    .is_some_and(|proof| proof.validate(&[root]))
            });
        if !valid {
            self.context.store.quarantine();
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
