//! Sole consuming Resume of the actual continued park. Original held context
//! and Start consumption survive all source and authority outcomes.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::resume_owned_copy_wait_v2;
use crate::live_invocation::source_journal::LiveContinuedWaitResumePermitV8;
pub(crate) struct LiveContinuedResumedStateV8<'j> {
    resumed: super::super::super::resume::LiveResumedStateV8,
    predecessor: PreparedHeldContinuedWaitV2<'j>,
    start_consumed: u64,
}
pub(crate) enum LiveContinuedWaitResumeOutcomeV8<'j> {
    Resumed(LiveContinuedResumedStateV8<'j>),
    Refused(LiveContinuedParkedStateV8<'j>),
    Terminal(LiveContinuedResumedStateV8<'j>),
    GuardLost {
        owner: LiveContinuedResumedStateV8<'j>,
        error: SourceJournalError,
    },
}
pub(crate) fn resume_live_continued_wait_v8<'j>(
    permit: &LiveContinuedWaitResumePermitV8<'_, 'j>,
    owner: LiveContinuedParkedStateV8<'j>,
    answer: ResumableChannelValue,
) -> LiveContinuedWaitResumeOutcomeV8<'j> {
    if !owner.predecessor.matches_resume_permit(permit)
        || permit.validate_guard().is_err()
        || owner.predecessor.validate_retained_context().is_err()
    {
        return LiveContinuedWaitResumeOutcomeV8::Refused(owner);
    }
    let fuel = permit.fuel();
    if fuel != owner.predecessor.evaluation_fuel() {
        return LiveContinuedWaitResumeOutcomeV8::Refused(owner);
    }
    let LiveContinuedParkedStateV8 {
        parked,
        predecessor,
    } = owner;
    let start_consumed = parked.consumed;
    let mut budget = OwnedFrameBudget::new(fuel).expect("actual full positive Resume F");
    #[cfg(test)]
    RESUME_ENTRIES.with(|n| n.set(n.get() + 1));
    let step = match resume_owned_copy_wait_v2(parked.parked, answer, &mut budget) {
        Ok(step) => step,
        Err((parked, _)) => {
            return LiveContinuedWaitResumeOutcomeV8::Refused(LiveContinuedParkedStateV8 {
                parked: LiveParkedStateV8 {
                    parked,
                    consumed: start_consumed,
                },
                predecessor,
            })
        }
    };
    let OwnedCopyWaitStepV2::Terminal(terminal) = step else {
        unreachable!("exact one-site helper")
    };
    let failed = terminal.failure.is_some();
    let owner = LiveContinuedResumedStateV8 {
        resumed: super::super::super::resume::LiveResumedStateV8 {
            terminal,
            consumed: budget.consumed() as u64,
        },
        predecessor,
        start_consumed,
    };
    if let Err(error) = permit
        .validate_guard()
        .and_then(|_| owner.predecessor.validate_retained_context())
    {
        return LiveContinuedWaitResumeOutcomeV8::GuardLost { owner, error };
    }
    if failed {
        LiveContinuedWaitResumeOutcomeV8::Terminal(owner)
    } else {
        LiveContinuedWaitResumeOutcomeV8::Resumed(owner)
    }
}
impl LiveContinuedResumedStateV8<'_> {
    pub(crate) fn consumed(&self) -> u64 {
        self.resumed.consumed()
    }
    pub(crate) fn start_consumed(&self) -> u64 {
        self.start_consumed
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        self.predecessor.validate_retained_context().ok()?;
        self.resumed.checked_facts(binding)
    }
    #[cfg(test)]
    pub(crate) fn test_resume_entries() -> usize {
        RESUME_ENTRIES.with(std::cell::Cell::get)
    }
}
#[cfg(test)]
thread_local! {static RESUME_ENTRIES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}

#[cfg(test)]
pub(crate) fn test_continued_resume_entries_v8() -> usize {
    RESUME_ENTRIES.with(std::cell::Cell::get)
}

pub(in crate::interpreter::resumable::owned_frame::registered_stage) mod authorize;
