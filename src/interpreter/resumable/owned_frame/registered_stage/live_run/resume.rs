//! Actual Resume is charged only behind its own same-history reservation ACK.
use super::*;
use crate::live_invocation::source_journal::LiveWaitResumePermitV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8;
pub(crate) struct LiveResumedStateV8 {
    pub(super) terminal: OwnedCopyWaitTerminalV2,
    pub(super) consumed: u64,
}
impl LiveResumedStateV8 {
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        let t = &self.terminal;
        if t.failure.is_some()
            || t.creator != std::process::id()
            || !t.provisional
            || !t.plan.same_helper(binding.helper())
        {
            return None;
        }
        let root = t.root.as_ref()?;
        if !t.allocations.validate(&[root]) {
            return None;
        }
        root_facts(&t.plan, root)
    }
    pub(crate) fn transfer_ready(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
        proposal: &crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
    ) -> bool {
        self.checked_facts(binding).is_some()
            && self.terminal.plan.liveness().completion_cleanup.is_empty()
            && self.terminal.proposal.as_ref() == Some(proposal.carrier())
    }
    #[cfg(test)]
    pub(crate) fn test_substitute_answer(&mut self) {
        let Some(ResumableChannelValue::Record { fields, .. }) = &mut self.terminal.proposal else {
            panic!("actual record answer");
        };
        fields[0] = crate::interpreter::ArgumentValue::Int(99);
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        super::super::super::snapshot::weak_leaves(self.terminal.root.as_ref().unwrap())
    }
}
pub(crate) enum LiveWaitResumeOutcomeV8 {
    Resumed(LiveResumedStateV8),
    Refused(LiveParkedStateV8),
    Terminal(LiveResumedStateV8),
    GuardLost(LiveResumedStateV8),
}
pub(crate) fn resume_live_owned_wait_v8(
    permit: LiveWaitResumePermitV8<'_>,
    parked: LiveParkedStateV8,
    answer: ResumableChannelValue,
) -> LiveWaitResumeOutcomeV8 {
    if permit.validate_guard().is_err() {
        return LiveWaitResumeOutcomeV8::Refused(parked);
    }
    let mut budget = OwnedFrameBudget::new(permit.fuel()).expect("checked Resume F");
    let step = match resume_owned_copy_wait_v2(parked.parked, answer, &mut budget) {
        Ok(step) => step,
        Err((parked, _)) => {
            return LiveWaitResumeOutcomeV8::Refused(LiveParkedStateV8 {
                parked,
                consumed: 0,
            })
        }
    };
    let OwnedCopyWaitStepV2::Terminal(terminal) = step else {
        unreachable!("exact one yield helper")
    };
    let failed = terminal.failure.is_some();
    let owner = LiveResumedStateV8 {
        terminal,
        consumed: budget.consumed() as u64,
    };
    if permit.validate_guard().is_err() {
        LiveWaitResumeOutcomeV8::GuardLost(owner)
    } else if failed {
        LiveWaitResumeOutcomeV8::Terminal(owner)
    } else {
        LiveWaitResumeOutcomeV8::Resumed(owner)
    }
}
