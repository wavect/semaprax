//! Actual continued State enters Observe only after its live original ACKs.
//! No raw sequence/budget tuple constructs a committed holder.
use super::*;
use crate::live_invocation::source_journal::LiveContinueObservePermitV8;
use crate::live_invocation::source_journal::SourceJournalError;

pub(crate) enum LiveContinuedObserveFailureV8<'j> {
    Before {
        held: HeldExecutedOwnedStepV2<'j>,
        error: SourceJournalError,
    },
    Committed {
        owner: ContinuedObserveRejectionV2<'j>,
        consumed: usize,
        error: SourceJournalError,
    },
    After {
        owner: ContinuedOwnedObserveV2<'j>,
        consumed: usize,
        error: SourceJournalError,
    },
}
/// The actual source permit borrows for this call; returned owners retain only
/// their original held store lifetime, never the short permit or source borrow.
pub(crate) fn observe_live_continued_state_v8<'j>(
    held: HeldExecutedOwnedStepV2<'j>,
    permit: &LiveContinueObservePermitV8<'_, 'j>,
) -> Result<(ContinuedOwnedObserveV2<'j>, usize), LiveContinuedObserveFailureV8<'j>> {
    let checked = (|| {
        let inputs = held.inputs.as_ref().ok_or(SourceJournalError::Binding)?;
        permit.matches_inputs(inputs)?;
        if held.kind() != "continue" {
            return Err(SourceJournalError::Binding);
        }
        let fuel = permit.fuel()?;
        if fuel != inputs.execution.evaluation_fuel() {
            return Err(SourceJournalError::Binding);
        }
        let (transition, reservation) = permit.causal_refs()?;
        permit.validate_guard()?;
        Ok((fuel, transition, reservation))
    })();
    let (fuel, transition, reservation) = match checked {
        Ok(x) => x,
        Err(error) => return Err(LiveContinuedObserveFailureV8::Before { held, error }),
    };
    // Sole production constructor: physical moved State + exact live ACKs.
    let committed = CommittedContinueObserveV2 {
        held,
        transition,
        reservation,
        turn: permit.turn(),
        fuel,
    };
    let mut budget = OwnedFrameBudget::new(fuel).expect("checked full positive E allowance");
    let mut selected = None;
    #[cfg(test)]
    CONTINUE_OBSERVE_ENTRIES.with(|entries| entries.set(entries.get() + 1));
    let outcome = observe_continued_owned_state_v2(committed, &mut budget, || {
        match permit.validate_guard() {
            Ok(()) => true,
            Err(error) => {
                selected = selected.or(Some(error));
                false
            }
        }
    });
    let consumed = budget.consumed();
    let outcome = match outcome {
        Ok(x) => x,
        Err(owner) => {
            return Err(LiveContinuedObserveFailureV8::Committed {
                owner,
                consumed,
                error: selected.unwrap_or(SourceJournalError::Binding),
            })
        }
    };
    if let Some(error) = selected {
        return Err(LiveContinuedObserveFailureV8::After {
            owner: outcome,
            consumed,
            error,
        });
    }
    match permit.validate_guard() {
        Ok(()) => Ok((outcome, consumed)),
        Err(error) => Err(LiveContinuedObserveFailureV8::After {
            owner: outcome,
            consumed,
            error,
        }),
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
use tests::CONTINUE_OBSERVE_ENTRIES;
#[cfg(test)]
pub(crate) use tests::{test_continue_observe_entries_v8, test_continue_observe_oracle_v8};
