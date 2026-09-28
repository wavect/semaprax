//! One committed root. All Drop paths dispose process backing only.
use super::settlement::{DurableRelease, UnpublishedOwnedFrameResult};
use super::*;

/// Opaque across the interpreter boundary; no enum arm exposes private Value.
pub(crate) struct DurableOwner {
    state: OwnerState,
}
enum OwnerState {
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
        Self {
            state: OwnerState::PreYield {
                plan: argument.plan.clone(),
                root: argument.root.take().expect("admitted argument"),
            },
        }
    }
    pub(in crate::interpreter) fn root_and_plan(&self) -> (&CheckedOwnedFramePlan, &Value) {
        match &self.state {
            OwnerState::PreYield { plan, root } => (plan, root),
            OwnerState::Parked(p) => (&p.plan, &p.root),
            OwnerState::Terminal(t) => (&t.plan, &t.root),
            OwnerState::Unpublished(r) => (&r.plan, &r.root),
        }
    }
    pub(crate) fn input(&self) -> Result<OwnedFrameInput, Diagnostic> {
        let (plan, root) = self.root_and_plan();
        snapshot::root_input(plan, root)
    }
    pub(crate) fn request(&self) -> Option<&ArgumentValue> {
        match &self.state {
            OwnerState::Parked(p) => Some(&p.request),
            _ => None,
        }
    }
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        match &self.state {
            OwnerState::Terminal(t) => t.failure.as_ref(),
            _ => None,
        }
    }
    pub(crate) fn pending_cleanup(&self) -> Option<&[FinalizeAction]> {
        match &self.state {
            OwnerState::Terminal(t) if t.failure.is_some() && t.provisional => {
                Some(&t.plan.liveness().result_disposal)
            }
            OwnerState::Terminal(t) if t.failure.is_some() => {
                Some(&t.plan.liveness().failure_cleanup)
            }
            OwnerState::Terminal(t) => Some(&t.plan.liveness().completion_cleanup),
            _ => None,
        }
    }
    pub(crate) fn start(self, budget: &mut OwnedFrameBudget) -> Self {
        let OwnerState::PreYield { plan, root } = self.state else {
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
        let OwnerState::Parked(parked) = self.state else {
            panic!("checked parked phase")
        };
        from_step(resume_owned_frame(parked, answer, budget))
    }
    pub(crate) fn abandon(self, selected: OwnedFrameFailure) -> Self {
        let (plan, root, provisional) = match self.state {
            OwnerState::PreYield { plan, root } => (plan, root, false),
            OwnerState::Parked(p) => (p.plan, p.root, false),
            OwnerState::Terminal(t) => {
                return Self {
                    state: OwnerState::Terminal(t),
                }
            }
            OwnerState::Unpublished(r) => (r.plan, r.root, true),
        };
        Self {
            state: OwnerState::Terminal(OwnedFrameStagedTerminal {
                plan,
                root,
                failure: Some(selected),
                provisional,
            }),
        }
    }
    pub(crate) fn settle(
        self,
        observer: &mut dyn FnMut(&FinalizeAction) -> bool,
    ) -> Result<DurableRelease, (Self, Diagnostic)> {
        let OwnerState::Terminal(terminal) = self.state else {
            panic!("checked terminal phase")
        };
        settlement::settle(terminal, observer).map_err(|e| {
            (
                Self {
                    state: OwnerState::Terminal(e.terminal),
                },
                e.diagnostic,
            )
        })
    }
}
fn from_step(step: OwnedFrameFoundationStep) -> DurableOwner {
    DurableOwner {
        state: match step {
            OwnedFrameFoundationStep::Parked(p) => OwnerState::Parked(p),
            OwnedFrameFoundationStep::Terminal(t) => OwnerState::Terminal(t),
        },
    }
}
