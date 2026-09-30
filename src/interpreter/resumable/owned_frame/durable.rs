//! One committed root. All Drop paths dispose process backing only.
use super::settlement::{DurableRelease, UnpublishedOwnedFrameResult};
use super::*;

#[cfg(test)]
pub(crate) fn evaluation_count() -> usize {
    replay::evaluation_count()
}
#[cfg(test)]
pub(crate) fn reset_evaluations() {
    replay::reset_evaluations();
}

/// Opaque across the interpreter boundary; no enum arm exposes private Value.
pub(crate) struct DurableOwner {
    state: OwnerState,
}
enum OwnerState {
    PreYield {
        plan: CheckedOwnedFramePlan,
        root: Value,
    },
    // An authenticated root awaiting charged historical start reconstruction.
    RestoringParked {
        plan: CheckedOwnedFramePlan,
        root: Value,
    },
    Parked(OwnedFrameParked),
    Terminal(OwnedFrameStagedTerminal),
    Unpublished(UnpublishedOwnedFrameResult),
}
// Contains only Copy evaluation state, never a root or leaf Arc. The driver
// compares its facts to authenticated history before installing it after ACK.
pub(crate) struct OwnedFrameReplay {
    environment: Environment,
    request: Option<ArgumentValue>,
    next: usize,
    failure: Option<OwnedFrameFailure>,
    provisional: bool,
}
impl OwnedFrameReplay {
    pub(crate) fn request(&self) -> Option<&ArgumentValue> {
        self.request.as_ref()
    }
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.failure.as_ref()
    }
    pub(crate) fn provisional(&self) -> bool {
        self.provisional
    }
}
impl DurableOwner {
    #[cfg(test)]
    pub(crate) fn weak_leaves(&self) -> Vec<std::sync::Weak<[u8]>> {
        snapshot::weak_leaves(self.root_and_plan().1)
    }
    pub(crate) fn from_unpublished(root: UnpublishedOwnedFrameResult) -> Self {
        Self {
            state: OwnerState::Unpublished(root),
        }
    }
    pub(crate) fn claim(
        self,
        permit: crate::resumable_effects::owned_frame::journal::OwnedFrameClaimPermit<'_>,
    ) -> Result<OwnedFrameResult, Self> {
        if permit.consume(self.root_and_plan().0).is_err() {
            return Err(self);
        }
        let OwnerState::Unpublished(root) = self.state else {
            return Err(self);
        };
        Ok(OwnedFrameResult {
            plan: root.plan,
            root: Some(root.root),
        })
    }
    // Caller must ACK its phase reservation before entering either evaluator.
    pub(crate) fn replay_start(&self, budget: &mut OwnedFrameBudget) -> OwnedFrameReplay {
        let (plan, root) = self.root_and_plan();
        let (outcome, environment, provisional) = replay::evaluate(
            plan,
            root,
            Environment::from(Vec::new()),
            0,
            None,
            true,
            budget,
        );
        replay_result(outcome, environment, provisional)
    }
    pub(crate) fn replay_resume(
        &self,
        start: &OwnedFrameReplay,
        answer: &ArgumentValue,
        budget: &mut OwnedFrameBudget,
    ) -> OwnedFrameReplay {
        let (plan, root) = self.root_and_plan();
        let Some(scalar) = super::super::scalar_of(
            &plan
                .function()
                .yields
                .as_ref()
                .expect("checked yield")
                .response_type,
            answer,
        ) else {
            return OwnedFrameReplay {
                environment: Environment::from(Vec::new()),
                request: None,
                next: 0,
                failure: Some(OwnedFrameFailure::AnswerTypeMismatch),
                provisional: false,
            };
        };
        let environment = Environment::from(
            start
                .environment
                .bindings
                .iter()
                .map(|(id, value)| {
                    (
                        id.clone(),
                        super::super::clone_scalar(value).expect("Copy-only replay environment"),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let (outcome, environment, provisional) = replay::evaluate(
            plan,
            root,
            environment,
            start.next,
            Some(scalar),
            false,
            budget,
        );
        replay_result(outcome, environment, provisional)
    }
    pub(crate) fn install_replayed_park(self, replayed: OwnedFrameReplay) -> Result<Self, Self> {
        if replayed.failure.is_some() || replayed.request.is_none() {
            return Err(self);
        }
        let (plan, root) = match self.state {
            OwnerState::RestoringParked { plan, root } => (plan, root),
            _ => return Err(self),
        };
        Ok(Self {
            state: OwnerState::Parked(OwnedFrameParked {
                plan,
                root,
                environment: replayed.environment,
                request: replayed.request.expect("checked request"),
                next: replayed.next,
            }),
        })
    }
    pub(crate) fn restore(
        plan: &CheckedOwnedFramePlan,
        permit: crate::resumable_effects::owned_frame::journal::OwnedFrameRestorePermit<'_>,
    ) -> Result<Self, crate::resumable_effects::owned_frame::OwnedFrameError> {
        use crate::resumable_effects::owned_frame::journal::RestorationKind;
        let (input, kind) = permit.consume(plan)?;
        let root = stage_root(plan, input);
        let plan = plan.clone();
        Ok(Self {
            state: match kind {
                RestorationKind::PreYield => OwnerState::PreYield { plan, root },
                RestorationKind::Parked => OwnerState::RestoringParked { plan, root },
                RestorationKind::Terminal {
                    failure,
                    provisional,
                } => OwnerState::Terminal(OwnedFrameStagedTerminal {
                    plan,
                    root,
                    failure,
                    provisional,
                }),
            },
        })
    }
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
            OwnerState::RestoringParked { plan, root } => (plan, root),
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
            OwnerState::RestoringParked { plan, root } => (plan, root, false),
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
        self.settle_guarded(observer, &mut || true)
    }
    pub(crate) fn settle_guarded(
        self,
        observer: &mut dyn FnMut(&FinalizeAction) -> bool,
        current_authority: &mut dyn FnMut() -> bool,
    ) -> Result<DurableRelease, (Self, Diagnostic)> {
        let OwnerState::Terminal(terminal) = self.state else {
            panic!("checked terminal phase")
        };
        settlement::settle_guarded(terminal, observer, current_authority).map_err(|e| {
            (
                Self {
                    state: OwnerState::Terminal(e.terminal),
                },
                e.diagnostic,
            )
        })
    }
}
fn replay_result(
    outcome: replay::PhaseResult,
    environment: Environment,
    provisional: bool,
) -> OwnedFrameReplay {
    match outcome {
        Ok(Some((request, next))) => OwnedFrameReplay {
            environment,
            request: Some(request),
            next,
            failure: None,
            provisional,
        },
        Ok(None) => OwnedFrameReplay {
            environment,
            request: None,
            next: 0,
            failure: None,
            provisional,
        },
        Err(flow) => OwnedFrameReplay {
            environment,
            request: None,
            next: 0,
            failure: Some(failure(flow)),
            provisional,
        },
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
