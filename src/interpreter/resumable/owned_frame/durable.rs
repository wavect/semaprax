//! One committed root. All Drop paths dispose process backing only.
use super::settlement::{DurableRelease, UnpublishedOwnedFrameResult};
use super::*;

pub(crate) enum DurableOwner {
    PreYield {
        plan: CheckedOwnedFramePlan,
        root: Value,
    },
    Parked(OwnedFrameParked),
    Terminal(OwnedFrameStagedTerminal),
    Unpublished(UnpublishedOwnedFrameResult),
}
impl DurableOwner {
    pub(crate) fn from_argument(mut argument: OwnedFrameArgument) -> Self {
        Self::PreYield {
            plan: argument.plan.clone(),
            root: argument.root.take().expect("admitted argument"),
        }
    }
    pub(crate) fn input(&self) -> Result<OwnedFrameInput, Diagnostic> {
        let (plan, root) = match self {
            Self::PreYield { plan, root } => (plan, root),
            Self::Parked(p) => (&p.plan, &p.root),
            Self::Terminal(t) => (&t.plan, &t.root),
            Self::Unpublished(r) => (&r.plan, &r.root),
        };
        snapshot::root_input(plan, root)
    }
    pub(crate) fn request(&self) -> Option<&ArgumentValue> {
        match self {
            Self::Parked(p) => Some(&p.request),
            _ => None,
        }
    }
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        match self {
            Self::Terminal(t) => t.failure.as_ref(),
            _ => None,
        }
    }
    pub(crate) fn pending_cleanup(&self) -> Option<&[FinalizeAction]> {
        match self {
            Self::Terminal(t) if t.failure.is_some() && t.provisional => {
                Some(&t.plan.liveness().result_disposal)
            }
            Self::Terminal(t) if t.failure.is_some() => Some(&t.plan.liveness().failure_cleanup),
            Self::Terminal(t) => Some(&t.plan.liveness().completion_cleanup),
            _ => None,
        }
    }
    pub(crate) fn start(self, budget: &mut OwnedFrameBudget) -> Self {
        let Self::PreYield { plan, root } = self else {
            panic!("checked pre-yield phase")
        };
        from_step(if budget.cancelled {
            terminal(plan, root, OwnedFrameFailure::HostAbandoned, false)
        } else {
            evaluate_phase(
                plan,
                root,
                Environment::from(Vec::new()),
                0,
                None,
                true,
                budget,
            )
        })
    }
    pub(crate) fn resume(self, answer: ArgumentValue, budget: &mut OwnedFrameBudget) -> Self {
        let Self::Parked(parked) = self else {
            panic!("checked parked phase")
        };
        from_step(resume_owned_frame(parked, answer, budget))
    }
    pub(crate) fn abandon(self, selected: OwnedFrameFailure) -> Self {
        let (plan, root, provisional) = match self {
            Self::PreYield { plan, root } => (plan, root, false),
            Self::Parked(p) => (p.plan, p.root, false),
            Self::Terminal(t) => return Self::Terminal(t), // primary stays sticky
            Self::Unpublished(r) => (r.plan, r.root, true),
        };
        Self::Terminal(OwnedFrameStagedTerminal {
            plan,
            root,
            failure: Some(selected),
            provisional,
        })
    }
    pub(crate) fn settle(
        self,
        observer: &mut dyn FnMut(&FinalizeAction) -> bool,
    ) -> Result<DurableRelease, (Self, Diagnostic)> {
        let Self::Terminal(terminal) = self else {
            panic!("checked terminal phase")
        };
        settlement::settle(terminal, observer)
            .map_err(|e| (Self::Terminal(e.terminal), e.diagnostic))
    }
}
fn from_step(step: OwnedFrameFoundationStep) -> DurableOwner {
    match step {
        OwnedFrameFoundationStep::Parked(p) => DurableOwner::Parked(p),
        OwnedFrameFoundationStep::Terminal(t) => DurableOwner::Terminal(t),
    }
}
