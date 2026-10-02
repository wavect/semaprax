//! Runtime custody for a fresh two-turn owned run. A session is only a mutable
//! borrow; closing it never destroys the reached physical owner. This remains
//! private until every pending phase has a shutdown/recovery settlement.
use super::model::{model_live_actor_v8, CompletedLiveOwnedRunV8, LiveModelQuarantinedV8};
use super::observe::settlement::LiveObserveSettlementActorFailureV8;
use super::observe::{observe_live_actor_v8, LiveObserveFailureV8};
use super::wait::{start_live_actor_v8, LiveWaitFailureV8};
use super::*;
use crate::agent_runtime::AgentCancellation;
use crate::cleanup_plan::FinalizeAction;
use crate::live_invocation::SourceInvocationClock;
use crate::provider_adapter_sdk::StreamingSourceProposalAdapter;

mod continue_run;
use continue_run::RunQuarantineV8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OwnedLifecycleStatusV8 {
    Ready,
    ModelCompleted,
    Complete,
    ObserveCleanupPending,
    ObserveStopped,
    Quarantined(&'static str),
}

// Enumerating the physical phases preserves the prescribed transfer. In
// particular an ACKed failed Observe is still cleanup-capable, not erased into
// a methodless box and prematurely poisoned with unrelated model failures.
enum CustodyV8<'j> {
    Ready,
    InFlight,
    ModelCompleted(Box<CompletedLiveOwnedRunV8<'j>>),
    Complete(serde_json::Value),
    Run(Box<RunQuarantineV8<'j>>),
    ObserveCleanupPending(Box<LiveSettledObserveV8<'j>>),
    ObserveStopped(Box<LiveFailedObserveStateStoppedV8<'j>>),
    Admission(LiveRunAdmissionRefusalV8),
    Initialize(Box<LiveRunFailureV8<'j>>),
    Observe(Box<LiveObserveFailureV8<'j>>),
    Start(Box<LiveWaitFailureV8<'j>>),
    Model(Box<LiveModelQuarantinedV8<'j>>),
    ObserveCleanup(Box<LiveFailedObserveStateQuarantinedV8<'j>>),
}

/// One journal's runtime slot. The host owns this independently of ephemeral
/// session handles. There is no owner extraction, retry, or raw-store accessor.
///
/// `try_close` refuses outstanding obligations and returns the entire runtime.
/// This internal type is deliberately not a public constructor: forced runtime
/// destruction still releases backing without a semantic cleanup receipt, so
/// public shutdown remains a separate required contract.
pub(super) struct OwnedLifecycleRuntimeV8<'j> {
    custody: CustodyV8<'j>,
    journal: &'j SourceOwnedWaitJournalV8,
    cancellation: &'j AgentCancellation,
    #[cfg(test)]
    backings: Vec<std::sync::Weak<[u8]>>,
}

/// Dropping this handle releases only the mutable borrow of the runtime slot.
pub(super) struct OwnedLifecycleSessionV8<'r, 'j> {
    runtime: &'r mut OwnedLifecycleRuntimeV8<'j>,
}

impl<'j> OwnedLifecycleRuntimeV8<'j> {
    pub(super) fn open(
        journal: &'j SourceOwnedWaitJournalV8,
        cancellation: &'j AgentCancellation,
    ) -> Result<Self, SourceJournalError> {
        if cancellation.is_cancelled() {
            return Err(SourceJournalError::Binding);
        }
        let context = journal.context();
        let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        if !context.fold().cumulative_initialization || execution.ordinary().max_iterations() != 2 {
            return Err(SourceJournalError::Binding);
        }
        journal.begin_fresh_session()?;
        journal.hold()?.validate_guard()?;
        Ok(Self {
            custody: CustodyV8::Ready,
            journal,
            cancellation,
            #[cfg(test)]
            backings: Vec::new(),
        })
    }

    #[cfg(test)]
    pub(super) fn test_backings(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.backings.clone()
    }

    pub(super) fn session(&mut self) -> OwnedLifecycleSessionV8<'_, 'j> {
        OwnedLifecycleSessionV8 { runtime: self }
    }

    pub(super) fn status(&self) -> OwnedLifecycleStatusV8 {
        match &self.custody {
            CustodyV8::Ready => OwnedLifecycleStatusV8::Ready,
            CustodyV8::InFlight => OwnedLifecycleStatusV8::Quarantined("runtime-unwind"),
            CustodyV8::ModelCompleted(_) => OwnedLifecycleStatusV8::ModelCompleted,
            CustodyV8::Complete(_) => OwnedLifecycleStatusV8::Complete,
            CustodyV8::Run(failure) => OwnedLifecycleStatusV8::Quarantined(failure.phase()),
            CustodyV8::ObserveCleanupPending(_) => OwnedLifecycleStatusV8::ObserveCleanupPending,
            CustodyV8::ObserveStopped(_) => OwnedLifecycleStatusV8::ObserveStopped,
            CustodyV8::Admission(_) => OwnedLifecycleStatusV8::Quarantined("initialize-admission"),
            CustodyV8::Initialize(_) => OwnedLifecycleStatusV8::Quarantined("initialize"),
            CustodyV8::Observe(_) => OwnedLifecycleStatusV8::Quarantined("observe"),
            CustodyV8::Start(_) => OwnedLifecycleStatusV8::Quarantined("start"),
            CustodyV8::Model(_) => OwnedLifecycleStatusV8::Quarantined("model"),
            CustodyV8::ObserveCleanup(_) => OwnedLifecycleStatusV8::Quarantined("observe-cleanup"),
        }
    }

    /// Only a never-entered runtime, acknowledged Stop, or consumed Complete
    /// Report can be closed.
    /// An unresolved return retains the exact same runtime, including its
    /// physical owner. Admission failures have admitted no physical State.
    pub(super) fn try_close(self) -> Result<(), Self> {
        match self.custody {
            CustodyV8::Ready
            | CustodyV8::Admission(_)
            | CustodyV8::ObserveStopped(_)
            | CustodyV8::Complete(_) => Ok(()),
            _ => Err(self),
        }
    }

    /// A copied, terminal-ACK-bound data projection. It grants no owner,
    /// dispatch, cleanup, store, or restoration authority.
    pub(super) fn delivery_projection(&self) -> Option<&serde_json::Value> {
        match &self.custody {
            CustodyV8::Complete(projection) => Some(projection),
            _ => None,
        }
    }

    fn retain_observe_failure(&mut self, failure: LiveObserveFailureV8<'j>) {
        self.custody = match failure {
            LiveObserveFailureV8::Settlement(LiveObserveSettlementActorFailureV8::Failed(
                owner,
            )) => CustodyV8::ObserveCleanupPending(Box::new(owner)),
            failure => {
                self.journal.quarantine();
                CustodyV8::Observe(Box::new(failure))
            }
        };
    }

    /// Runs only the prescribed failed-Observe cleanup/receipt/Stop tail.
    /// Reconciliation can be requested by the runtime after a session handle
    /// has gone away. It never restarts Observe, Model or target dispatch.
    pub(super) fn settle_failed_observe(
        &mut self,
        observe: impl FnMut(&FinalizeAction),
    ) -> OwnedLifecycleStatusV8 {
        if !matches!(self.custody, CustodyV8::ObserveCleanupPending(_)) {
            return self.status();
        }
        let CustodyV8::ObserveCleanupPending(owner) =
            std::mem::replace(&mut self.custody, CustodyV8::InFlight)
        else {
            unreachable!("matched runtime phase")
        };
        self.custody = match stop_failed_observe_state_v8(*owner, observe) {
            Ok(stopped) => CustodyV8::ObserveStopped(Box::new(stopped)),
            Err(failure) => CustodyV8::ObserveCleanup(Box::new(failure)),
        };
        self.status()
    }
}

impl<'j> OwnedLifecycleSessionV8<'_, 'j> {
    pub(super) fn status(&self) -> OwnedLifecycleStatusV8 {
        self.runtime.status()
    }

    /// Continue the one retained first Model owner through both actual effects
    /// and the second Model into terminal Report consumption. Reopened handles
    /// only observe the reached status; neither success nor failure is retryable.
    pub(super) fn finish_two_turn_run(
        &mut self,
        policy: &'j crate::resumable_effects::CapabilityPolicy,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler,
        observe: impl FnMut(&FinalizeAction),
    ) -> OwnedLifecycleStatusV8 {
        let runtime = &mut self.runtime;
        if !matches!(runtime.custody, CustodyV8::ModelCompleted(_)) {
            return runtime.status();
        }
        let CustodyV8::ModelCompleted(owner) =
            std::mem::replace(&mut runtime.custody, CustodyV8::InFlight)
        else {
            unreachable!("matched runtime phase")
        };
        runtime.custody = match continue_run::finish_run(
            runtime.journal,
            *owner,
            policy,
            adapter,
            handler,
            observe,
        ) {
            Ok(projection) => CustodyV8::Complete(projection),
            Err(failure) => CustodyV8::Run(Box::new(failure)),
        };
        runtime.status()
    }

    /// Initialize through the first Model/Resume ACK using the existing
    /// consuming joins. Return only status; all reached owners stay in runtime
    /// custody even when this handle is discarded immediately.
    pub(super) fn run_first_turn_model(
        &mut self,
        input: OwnedFrameInput,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        clock: &'j dyn SourceInvocationClock,
    ) -> OwnedLifecycleStatusV8 {
        let runtime = &mut self.runtime;
        if !matches!(runtime.custody, CustodyV8::Ready) {
            return runtime.status();
        }
        runtime.custody = CustodyV8::InFlight;
        let initialized =
            match initialize_live_actor_v8(runtime.journal, input, runtime.cancellation) {
                Ok(Ok(owner)) => owner,
                Err(refusal) => {
                    runtime.custody = CustodyV8::Admission(refusal);
                    return runtime.status();
                }
                Ok(Err(failure)) => {
                    runtime.journal.quarantine();
                    runtime.custody = CustodyV8::Initialize(Box::new(failure));
                    return runtime.status();
                }
            };
        #[cfg(test)]
        {
            runtime.backings = initialized.owner.test_weak();
        }
        let observed = match observe_live_actor_v8(initialized) {
            Ok(owner) => owner,
            Err(failure) => {
                runtime.retain_observe_failure(failure);
                return runtime.status();
            }
        };
        let parked = match start_live_actor_v8(observed) {
            Ok(owner) => owner,
            Err(failure) => {
                runtime.journal.quarantine();
                runtime.custody = CustodyV8::Start(Box::new(failure));
                return runtime.status();
            }
        };
        runtime.custody = match model_live_actor_v8(parked, adapter, clock) {
            Ok(owner) => CustodyV8::ModelCompleted(Box::new(owner)),
            Err(failure) => CustodyV8::Model(Box::new(failure.quarantine())),
        };
        runtime.status()
    }
}

impl Drop for OwnedLifecycleRuntimeV8<'_> {
    fn drop(&mut self) {
        if !matches!(
            self.custody,
            CustodyV8::Ready
                | CustodyV8::Admission(_)
                | CustodyV8::ObserveStopped(_)
                | CustodyV8::Complete(_)
        ) {
            // Forced host runtime teardown is not a semantic settlement. Retire
            // authority before fields release backing, so the remaining journal
            // borrow cannot continue from an owner that no longer exists.
            self.journal.quarantine();
        }
    }
}
