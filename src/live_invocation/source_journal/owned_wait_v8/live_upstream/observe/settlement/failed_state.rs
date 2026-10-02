//! Actual failed Observe State cleanup. Descriptive facts never recreate State.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::model::{self as journal_model, OwnedBodyV8};
use crate::agent_lifecycle::authorization::target_protocol::TargetAccounting;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    InitialObserveStateCleanupFailureV8, ReleasedInitialObserveStateV8,
};
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    ContinuedObserveStateCleanupFailureV8, ReleasedContinuedObserveStateV8,
};
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::FailedHeldOwnedObserveV2;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedFailedObserveStateSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::append::LiveFailedObserveStateAppendFailureV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::settlement::failed_state::ContinuedFailedObserveContextV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::r#continue::later::settlement::failed_state::LaterFailedObserveContextV8;
use crate::live_invocation::source_journal::{SourceStopReason,SourceStopStatus};
use crate::resumable_effects::owned_frame::v2::CheckedOwnedFrameHelperV2;
use serde_json::{json,Value as Json};

pub(in crate::live_invocation::source_journal::owned_wait_v8) enum FailedObserveOwnerV8<'j> {
    Initial(
        crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveFailedObserveV8,
    ),
    Continued {
        failed: FailedHeldOwnedObserveV2<'j>,
        accounting: TargetAccounting,
    },
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct InitialFailedObserveContextV8<
    'j,
> {
    held: HeldOwnedWaitStoreV8<'j>,
    journal: &'j SourceOwnedWaitJournalV8,
    cancellation: &'j crate::agent_runtime::AgentCancellation,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum FailedObserveContextV8<'j> {
    Initial(InitialFailedObserveContextV8<'j>),
    Continued(ContinuedFailedObserveContextV8<'j>),
    Later(LaterFailedObserveContextV8<'j>),
}
impl<'j> FailedObserveContextV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        match self {
            Self::Initial(c) => c.journal,
            Self::Continued(c) => c.journal(),
            Self::Later(c) => c.journal(),
        }
    }
    fn guard(
        &self,
        sequence: usize,
        bytes: usize,
        incurred: bool,
    ) -> Result<(), SourceJournalError> {
        let result = match self {
            Self::Initial(c) => (|| {
                c.held.validate_prefix(sequence, bytes)?;
                if !incurred && c.cancellation.is_cancelled() {
                    return Err(SourceJournalError::Order);
                }
                let (r, e) = c
                    .journal
                    .context()
                    .ready_runtime()
                    .ok_or(SourceJournalError::Binding)?;
                r.owned_wait_effects_v8(e)
                    .map_err(|_| SourceJournalError::Binding)?;
                c.held.validate_prefix(sequence, bytes)
            })(),
            Self::Continued(c) => c.guard_at(sequence, bytes, incurred),
            Self::Later(c) => c.guard_at(sequence, bytes, incurred),
        };
        result.inspect_err(|_| self.journal().quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FailedObserveCacheV8<'j> {
    observed: ObserveSettlementAckV8<'j>,
    state: Json,
    failure: OwnedFrameFailure,
    terminal: Json,
    consumed: u64,
    operations: Json,
    basis: u32,
    turn: u32,
}
impl FailedObserveCacheV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn turn(&self) -> u32 {
        self.turn
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FailedObserveAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedFailedObserveStateSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FailedObserveSourceV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) owner: FailedObserveOwnerV8<'j>,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) context:
        FailedObserveContextV8<'j>,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) cache: FailedObserveCacheV8<'j>,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) acks: Vec<FailedObserveAckV8<'j>>,
}
struct FailedObserveLineageV8<'j> {
    context: FailedObserveContextV8<'j>,
    cache: FailedObserveCacheV8<'j>,
    acks: Vec<FailedObserveAckV8<'j>>,
}
impl<'j> FailedObserveLineageV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.context.journal()
    }
    fn sequence(&self) -> usize {
        self.acks
            .last()
            .map_or(self.cache.observed.session.sequence(), |a| {
                a.session.sequence()
            })
    }
    fn bytes(&self) -> usize {
        self.acks
            .last()
            .map_or(self.cache.observed.session.acknowledged_bytes(), |a| {
                a.session.acknowledged_bytes()
            })
    }
    fn guard(&self, incurred: bool) -> Result<(), SourceJournalError> {
        let result = (|| {
            if let Some(a) = self.acks.last() {
                a.witness.validate_current_session(&a.session)?;
            } else {
                self.cache
                    .observed
                    .witness
                    .validate_current_session(&self.cache.observed.session)?;
            }
            self.context.guard(self.sequence(), self.bytes(), incurred)
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
}
impl FailedObserveSourceV8<'_> {
    fn validate_state(&self) -> Result<(), SourceJournalError> {
        let (state, failure, consumed) = match &self.owner {
            FailedObserveOwnerV8::Initial(o) => (
                o.live_state_facts_v8()
                    .map_err(|_| SourceJournalError::Binding)?,
                o.failure(),
                o.consumed(),
            ),
            FailedObserveOwnerV8::Continued { failed, .. } => (
                failed
                    .live_failed_state_facts_v8()
                    .map_err(|_| SourceJournalError::Binding)?,
                failed.failure(),
                failed.consumed() as u64,
            ),
        };
        if state != self.cache.state
            || failure != &self.cache.failure
            || consumed != self.cache.consumed
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    fn started(&self) -> Result<EntryV8, SourceJournalError> {
        if !self.acks.is_empty() {
            return Err(SourceJournalError::Order);
        }
        self.context.guard(
            self.cache.observed.session.sequence(),
            self.cache.observed.session.acknowledged_bytes(),
            false,
        )?;
        self.validate_state()?;
        let data = json!({"owner":"state","basis":self.cache.basis,"terminal":self.cache.terminal,"operations":self.cache.operations});
        Ok(EntryV8::Owned(OwnedBodyV8::OwnedCleanupStarted {
            turn: self.cache.turn,
            attempt: None,
            wait: None,
            owner: journal_model::OwnerV8::State,
            basis: self.cache.basis,
            terminal: self.cache.terminal.clone(),
            operations: self.cache.operations.clone(),
            operations_digest: wire::recipe_digest(wire::RecipeV8::Operations, &data)?,
        }))
    }
}
impl<'j> LiveSettledObserveV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_failed_state_cleanup(
        self,
    ) -> Result<LiveFailedObserveStateAppendV8<'j>, LiveFailedObserveStateFailureV8<'j>> {
        let selected = (|| {
            if self.acks.len() != 1 {
                return Err(SourceJournalError::Order);
            }
            let ack = &self.acks[0];
            ack.witness.validate_current_session(&ack.session)?;
            self.owner
                .guard_at(ack.session.sequence(), ack.session.acknowledged_bytes())?;
            let data = self.owner.data()?;
            if data.observation.is_some() {
                return Err(SourceJournalError::Binding);
            }
            let failure = data.failure.ok_or(SourceJournalError::Binding)?;
            let terminal = failure_status(&failure)?;
            let journal = self.owner.journal();
            let session = journal.begin_session()?;
            if session.sequence() != ack.session.sequence()
                || session.acknowledged_bytes() != ack.session.acknowledged_bytes()
            {
                return Err(SourceJournalError::Binding);
            }
            let (_, _, turn, basis, state_digest, status, selected) =
                session.failed_observe_cleanup_facts()?;
            if selected != ack.witness.selected_row()
                || state_digest != wire::record_argument_digest(&data.state)
                || status != &terminal
                || turn != self.owner.turn()
            {
                return Err(SourceJournalError::Binding);
            }
            let operations = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
                &journal
                    .context()
                    .ready_runtime()
                    .ok_or(SourceJournalError::Binding)?
                    .1
                    .wait()
                    .observe()
                    .helper()
                    .liveness()
                    .failure_cleanup,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            Ok((
                data.state,
                failure,
                terminal,
                data.consumed,
                operations,
                basis,
                turn,
            ))
        })();
        let (state, failure, terminal, consumed, operations, basis, turn) = match selected {
            Ok(x) => x,
            Err(error) => {
                self.owner.journal().quarantine();
                return Err(LiveFailedObserveStateFailureV8::Selection { owner: self, error });
            }
        };
        let Self { owner, mut acks } = self;
        let cache = FailedObserveCacheV8 {
            observed: acks.remove(0),
            state,
            failure,
            terminal,
            consumed,
            operations,
            basis,
            turn,
        };
        let source = match owner {
            LiveObserveSettlementOwnerV8::Initial(i) => {
                let InitialObserveSettlementV8 {
                    owner,
                    held,
                    journal,
                    cancellation,
                    ..
                } = i;
                let LiveObserveOutcomeV8::Failed(failed) = owner else {
                    unreachable!("checked genuine variant")
                };
                FailedObserveSourceV8 {
                    owner: FailedObserveOwnerV8::Initial(failed),
                    context: FailedObserveContextV8::Initial(InitialFailedObserveContextV8 {
                        held,
                        journal,
                        cancellation,
                    }),
                    cache,
                    acks: Vec::new(),
                }
            }
            LiveObserveSettlementOwnerV8::Continued(c) => {
                match c.into_failed_state_cleanup(cache) {
                    Ok(x) => x,
                    Err(_) => unreachable!("checked actual continued failure"),
                }
            }
            LiveObserveSettlementOwnerV8::Later(later) => {
                match later.into_failed_state_cleanup(cache) {
                    Ok(x) => x,
                    Err((later, cache)) => {
                        let owner = LiveObserveSettlementOwnerV8::Later(later);
                        acks.insert(0, cache.observed);
                        owner.journal().quarantine();
                        return Err(LiveFailedObserveStateFailureV8::Selection {
                            owner: LiveSettledObserveV8 { owner, acks },
                            error: SourceJournalError::Binding,
                        });
                    }
                }
            }
        };
        match source.started() {
            Ok(selected) => Ok(LiveFailedObserveStateAppendV8 {
                owner: FailedObserveAppendOwnerV8::Failed(source),
                selected,
            }),
            Err(error) => {
                source.context.journal().quarantine();
                Err(LiveFailedObserveStateFailureV8::Source {
                    owner: source,
                    error,
                })
            }
        }
    }
}

pub(crate) struct LiveFailedObserveStateCleanupPermitV8<'p, 'j> {
    lineage: &'p FailedObserveLineageV8<'j>,
}
impl LiveFailedObserveStateCleanupPermitV8<'_, '_> {
    pub(crate) fn quarantine(&self) {
        self.lineage.journal().quarantine();
    }
    pub(crate) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        if self.lineage.acks.len() != 1
            || !matches!(
                self.lineage.acks[0].witness.selected_row(),
                EntryV8::Owned(OwnedBodyV8::OwnedCleanupStarted {
                    owner: journal_model::OwnerV8::State,
                    attempt: None,
                    wait: None,
                    ..
                })
            )
        {
            return Err(SourceJournalError::Order);
        }
        self.lineage.guard(true)
    }
    pub(crate) fn validate_initial(&self) -> Result<(), SourceJournalError> {
        if matches!(self.lineage.context, FailedObserveContextV8::Initial(_)) {
            Ok(())
        } else {
            Err(SourceJournalError::Binding)
        }
    }
    pub(crate) fn validate_continued(
        &self,
        r: &crate::execution_revision::typed::AgentRuntimeV2,
        e: &crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8,
        store: &HeldOwnedWaitStoreV8<'_>,
        policy: &crate::resumable_effects::capability::CapabilityPolicy,
    ) -> Result<(), SourceJournalError> {
        match &self.lineage.context {
            FailedObserveContextV8::Continued(c) => c.matches_context(r, e, store, policy),
            FailedObserveContextV8::Later(c) => c.matches_context(r, e, store, policy),
            _ => Err(SourceJournalError::Binding),
        }
    }
    pub(crate) fn validate_actual_failed(
        &self,
        helper: &CheckedOwnedFrameHelperV2,
        state: &Json,
        failure: &OwnedFrameFailure,
        consumed: u64,
    ) -> Result<(), SourceJournalError> {
        let c = &self.lineage.cache;
        if !helper.same_helper(
            self.lineage
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1
                .wait()
                .observe()
                .helper(),
        ) || state != &c.state
            || failure != &c.failure
            || consumed != c.consumed
        {
            return Err(SourceJournalError::Binding);
        }
        let operations = crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(
            &helper.liveness().failure_cleanup,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if operations != c.operations {
            return Err(SourceJournalError::Binding);
        }
        self.validate_cleanup_current()
    }
}
enum ReleasedObserveOwnerV8<'j> {
    Initial(ReleasedInitialObserveStateV8),
    Continued {
        released: ReleasedContinuedObserveStateV8<'j>,
        accounting: TargetAccounting,
    },
}
impl ReleasedObserveOwnerV8<'_> {
    fn cleanup(&self)->&crate::interpreter::resumable::owned_frame::registered_stage::observe::ObservedFailedObserveCleanupV8{
        match self {
            Self::Initial(x) => x.cleanup(),
            Self::Continued { released, .. } => released.cleanup(),
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveStartedFailedObserveStateV8<
    'j,
> {
    owner: FailedObserveOwnerV8<'j>,
    lineage: FailedObserveLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReleasedFailedObserveStateV8<
    'j,
> {
    owner: ReleasedObserveOwnerV8<'j>,
    lineage: FailedObserveLineageV8<'j>,
    receipt: Json,
}
enum ActualObserveReleaseFailureV8<'j> {
    Initial(InitialObserveStateCleanupFailureV8),
    Continued(ContinuedObserveStateCleanupFailureV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct InterruptedObserveStateV8<'j> {
    _actual: ActualObserveReleaseFailureV8<'j>,
    _accounting: Option<TargetAccounting>,
    _lineage: FailedObserveLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveFailedObserveStateFailureV8<
    'j,
> {
    Selection {
        owner: LiveSettledObserveV8<'j>,
        error: SourceJournalError,
    },
    Source {
        owner: FailedObserveSourceV8<'j>,
        error: SourceJournalError,
    },
    Started {
        owner: LiveStartedFailedObserveStateV8<'j>,
        error: SourceJournalError,
    },
    Interrupted(InterruptedObserveStateV8<'j>),
    Released {
        owner: LiveReleasedFailedObserveStateV8<'j>,
        error: SourceJournalError,
    },
    Acknowledged {
        owner: LiveFailedObserveStateAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedFailedObserveStateSuccessorV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveStartedFailedObserveStateV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release(
        self,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
    ) -> Result<LiveReleasedFailedObserveStateV8<'j>, LiveFailedObserveStateFailureV8<'j>> {
        if let Err(error) = self.lineage.guard(true) {
            return Err(LiveFailedObserveStateFailureV8::Started { owner: self, error });
        }
        let Self { owner, lineage } = self;
        let permit = LiveFailedObserveStateCleanupPermitV8 { lineage: &lineage };
        let owner = match owner {
            FailedObserveOwnerV8::Initial(x) => match x.release_failed_state_v8(&permit, observe) {
                Ok(x) => ReleasedObserveOwnerV8::Initial(x),
                Err(actual) => {
                    lineage.journal().quarantine();
                    return Err(LiveFailedObserveStateFailureV8::Interrupted(
                        InterruptedObserveStateV8 {
                            _actual: ActualObserveReleaseFailureV8::Initial(actual),
                            _accounting: None,
                            _lineage: lineage,
                        },
                    ));
                }
            },
            FailedObserveOwnerV8::Continued { failed, accounting } => {
                match failed.release_failed_state_v8(&permit, observe) {
                    Ok(released) => ReleasedObserveOwnerV8::Continued {
                        released,
                        accounting,
                    },
                    Err(actual) => {
                        lineage.journal().quarantine();
                        return Err(LiveFailedObserveStateFailureV8::Interrupted(
                            InterruptedObserveStateV8 {
                                _actual: ActualObserveReleaseFailureV8::Continued(actual),
                                _accounting: Some(accounting),
                                _lineage: lineage,
                            },
                        ));
                    }
                }
            }
        };
        let cleanup = owner.cleanup();
        let operations = lineage
            .cache
            .operations
            .as_array()
            .expect("checked compiler vector");
        let receipt = json!({"kind":"observed","operations":operations.iter().zip(cleanup.outcomes()).map(|(op,ok)|json!({"operation":op,"outcome":if *ok{"completed"}else{"failed"}})).collect::<Vec<_>>(),"settlement":if cleanup.settled.observations_succeeded{"completed"}else{"failed"}});
        let source = LiveReleasedFailedObserveStateV8 {
            owner,
            lineage,
            receipt,
        };
        let checked = (|| {
            if source.owner.cleanup().settled.failure != source.lineage.cache.failure {
                return Err(SourceJournalError::Binding);
            }
            crate::resumable_effects::owned_frame::v2::validate_owned_wait_observed_receipt_v8(
                &source.lineage.cache.operations,
                &source.receipt,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            source.lineage.guard(true)
        })();
        if let Err(error) = checked {
            source.lineage.journal().quarantine();
            return Err(LiveFailedObserveStateFailureV8::Released {
                owner: source,
                error,
            });
        }
        Ok(source)
    }
}
impl<'j> LiveReleasedFailedObserveStateV8<'j> {
    fn receipt_row(&self) -> Result<EntryV8, SourceJournalError> {
        self.lineage.guard(true)?;
        if self.lineage.acks.len() != 1 {
            return Err(SourceJournalError::Order);
        }
        let started = self.lineage.acks[0]
            .session
            .sequence()
            .checked_sub(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Capacity)?;
        crate::resumable_effects::owned_frame::v2::validate_owned_wait_observed_receipt_v8(
            &self.lineage.cache.operations,
            &self.receipt,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        let digest = wire::recipe_digest(wire::RecipeV8::Receipt, &self.receipt)?;
        Ok(EntryV8::Owned(OwnedBodyV8::OwnedCleanupSettled {
            turn: self.lineage.cache.turn,
            attempt: None,
            wait: None,
            owner: journal_model::OwnerV8::State,
            started,
            receipt: self.receipt.clone(),
            receipt_digest: digest,
        }))
    }
    fn stop_row(&self) -> Result<EntryV8, SourceJournalError> {
        self.lineage.guard(false)?;
        if self.lineage.acks.len() != 2
            || self.receipt["settlement"] != "completed"
            || !self.owner.cleanup().settled.observations_succeeded
        {
            return Err(SourceJournalError::Order);
        }
        let (status, reason) = match self.lineage.cache.failure {
            OwnedFrameFailure::FuelExhausted | OwnedFrameFailure::CallDepthExceeded => (
                SourceStopStatus::BudgetExhausted,
                SourceStopReason::BudgetExhausted,
            ),
            _ => (SourceStopStatus::Rejected, SourceStopReason::StageRefused),
        };
        Ok(EntryV8::Ordinary(SourceJournalEntry::Stop {
            turn: Some(self.lineage.cache.turn),
            attempt: None,
            status,
            reason,
        }))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_receipt(
        self,
    ) -> Result<LiveFailedObserveStateAppendV8<'j>, LiveFailedObserveStateFailureV8<'j>> {
        match self.receipt_row() {
            Ok(selected) => Ok(LiveFailedObserveStateAppendV8 {
                owner: FailedObserveAppendOwnerV8::Released(self),
                selected,
            }),
            Err(error) => {
                self.lineage.journal().quarantine();
                Err(LiveFailedObserveStateFailureV8::Released { owner: self, error })
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_stop(
        self,
    ) -> Result<LiveFailedObserveStateAppendV8<'j>, LiveFailedObserveStateFailureV8<'j>> {
        match self.stop_row() {
            Ok(selected) => Ok(LiveFailedObserveStateAppendV8 {
                owner: FailedObserveAppendOwnerV8::Released(self),
                selected,
            }),
            Err(error) => {
                self.lineage.journal().quarantine();
                Err(LiveFailedObserveStateFailureV8::Released { owner: self, error })
            }
        }
    }
}
enum FailedObserveAppendOwnerV8<'j> {
    Failed(FailedObserveSourceV8<'j>),
    Released(LiveReleasedFailedObserveStateV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveFailedObserveStateAppendV8<
    'j,
> {
    owner: FailedObserveAppendOwnerV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveFailedObserveStateAcknowledgedV8<
    'j,
> {
    Started(LiveStartedFailedObserveStateV8<'j>),
    Released(LiveReleasedFailedObserveStateV8<'j>),
    Stopped(LiveReleasedFailedObserveStateV8<'j>),
}

/// A completed failed-Observe terminal. Its released State owner stays sealed
/// inside the runtime boundary after the acknowledged Stop row.
pub(crate) struct LiveFailedObserveStateStoppedV8<'j> {
    _released: LiveReleasedFailedObserveStateV8<'j>,
}

enum QuarantinedFailedObserveOwnerV8<'j> {
    Failure {
        _owner: LiveFailedObserveStateFailureV8<'j>,
    },
    Pending {
        _owner: LiveFailedObserveStateAppendV8<'j>,
    },
    AppendFault {
        _owner: LiveFailedObserveStateAppendFailureV8<'j>,
    },
    Acknowledged {
        _owner: LiveFailedObserveStateAcknowledgedV8<'j>,
    },
}

/// Keeps the exact reached physical owner alive while the caller retains this
/// private error. It has no retry, extraction, append, or cleanup method.
pub(crate) struct LiveFailedObserveStateQuarantinedV8<'j> {
    _owner: QuarantinedFailedObserveOwnerV8<'j>,
}

impl LiveFailedObserveStateQuarantinedV8<'_> {
    pub(crate) fn status(&self) -> SourceJournalError {
        SourceJournalError::Poisoned
    }
}

fn quarantine_failed_observe_state<'j>(
    journal: &SourceOwnedWaitJournalV8,
    owner: QuarantinedFailedObserveOwnerV8<'j>,
) -> LiveFailedObserveStateQuarantinedV8<'j> {
    journal.quarantine();
    LiveFailedObserveStateQuarantinedV8 { _owner: owner }
}

/// Drive one failed initial or continued Observe through its acknowledged State
/// cleanup, receipt, and sticky Stop rows. An incomplete boundary seals the
/// journal and returns the reached physical owner in an opaque private error.
pub(crate) fn stop_failed_observe_state_v8<'j>(
    failed: LiveSettledObserveV8<'j>,
    observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
) -> Result<LiveFailedObserveStateStoppedV8<'j>, LiveFailedObserveStateQuarantinedV8<'j>> {
    let journal = failed.owner.journal();
    let cleanup = match failed.prepare_failed_state_cleanup() {
        Ok(cleanup) => cleanup,
        Err(owner) => {
            return Err(quarantine_failed_observe_state(
                journal,
                QuarantinedFailedObserveOwnerV8::Failure { _owner: owner },
            ))
        }
    };
    let started = match journal.begin_session() {
        Ok(session) => match session.append_failed_observe_state(cleanup) {
            Ok(append) => match append.advance_failed_observe_state() {
                Ok(LiveFailedObserveStateAcknowledgedV8::Started(started)) => started,
                Ok(owner) => {
                    return Err(quarantine_failed_observe_state(
                        journal,
                        QuarantinedFailedObserveOwnerV8::Acknowledged { _owner: owner },
                    ))
                }
                Err(owner) => {
                    return Err(quarantine_failed_observe_state(
                        journal,
                        QuarantinedFailedObserveOwnerV8::Failure { _owner: owner },
                    ))
                }
            },
            Err(owner) => {
                return Err(quarantine_failed_observe_state(
                    journal,
                    QuarantinedFailedObserveOwnerV8::AppendFault { _owner: owner },
                ))
            }
        },
        Err(_) => {
            return Err(quarantine_failed_observe_state(
                journal,
                QuarantinedFailedObserveOwnerV8::Pending { _owner: cleanup },
            ))
        }
    };
    let released = match started.release(observe) {
        Ok(released) => released,
        Err(owner) => {
            return Err(quarantine_failed_observe_state(
                journal,
                QuarantinedFailedObserveOwnerV8::Failure { _owner: owner },
            ))
        }
    };
    let receipt = match released.prepare_receipt() {
        Ok(receipt) => receipt,
        Err(owner) => {
            return Err(quarantine_failed_observe_state(
                journal,
                QuarantinedFailedObserveOwnerV8::Failure { _owner: owner },
            ))
        }
    };
    let released = match journal.begin_session() {
        Ok(session) => match session.append_failed_observe_state(receipt) {
            Ok(append) => match append.advance_failed_observe_state() {
                Ok(LiveFailedObserveStateAcknowledgedV8::Released(released)) => released,
                Ok(owner) => {
                    return Err(quarantine_failed_observe_state(
                        journal,
                        QuarantinedFailedObserveOwnerV8::Acknowledged { _owner: owner },
                    ))
                }
                Err(owner) => {
                    return Err(quarantine_failed_observe_state(
                        journal,
                        QuarantinedFailedObserveOwnerV8::Failure { _owner: owner },
                    ))
                }
            },
            Err(owner) => {
                return Err(quarantine_failed_observe_state(
                    journal,
                    QuarantinedFailedObserveOwnerV8::AppendFault { _owner: owner },
                ))
            }
        },
        Err(_) => {
            return Err(quarantine_failed_observe_state(
                journal,
                QuarantinedFailedObserveOwnerV8::Pending { _owner: receipt },
            ))
        }
    };
    let stop = match released.prepare_stop() {
        Ok(stop) => stop,
        Err(owner) => {
            return Err(quarantine_failed_observe_state(
                journal,
                QuarantinedFailedObserveOwnerV8::Failure { _owner: owner },
            ))
        }
    };
    match journal.begin_session() {
        Ok(session) => match session.append_failed_observe_state(stop) {
            Ok(append) => match append.advance_failed_observe_state() {
                Ok(LiveFailedObserveStateAcknowledgedV8::Stopped(released)) => {
                    Ok(LiveFailedObserveStateStoppedV8 {
                        _released: released,
                    })
                }
                Ok(owner) => Err(quarantine_failed_observe_state(
                    journal,
                    QuarantinedFailedObserveOwnerV8::Acknowledged { _owner: owner },
                )),
                Err(owner) => Err(quarantine_failed_observe_state(
                    journal,
                    QuarantinedFailedObserveOwnerV8::Failure { _owner: owner },
                )),
            },
            Err(owner) => Err(quarantine_failed_observe_state(
                journal,
                QuarantinedFailedObserveOwnerV8::AppendFault { _owner: owner },
            )),
        },
        Err(_) => Err(quarantine_failed_observe_state(
            journal,
            QuarantinedFailedObserveOwnerV8::Pending { _owner: stop },
        )),
    }
}

impl<'j> LiveFailedObserveStateAppendV8<'j> {
    fn context(&self) -> &FailedObserveContextV8<'j> {
        match &self.owner {
            FailedObserveAppendOwnerV8::Failed(s) => &s.context,
            FailedObserveAppendOwnerV8::Released(s) => &s.lineage.context,
        }
    }
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.context().journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        j: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal(), j)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        match &self.owner {
            FailedObserveAppendOwnerV8::Failed(s) => s.cache.observed.session.sequence(),
            FailedObserveAppendOwnerV8::Released(s) => s.lineage.sequence(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        match &self.owner {
            FailedObserveAppendOwnerV8::Failed(s) => s.cache.observed.session.acknowledged_bytes(),
            FailedObserveAppendOwnerV8::Released(s) => s.lineage.bytes(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let expected = match &self.owner {
            FailedObserveAppendOwnerV8::Failed(s) => s.started(),
            FailedObserveAppendOwnerV8::Released(s) => {
                if matches!(
                    self.selected,
                    EntryV8::Ordinary(SourceJournalEntry::Stop { .. })
                ) {
                    s.stop_row()
                } else {
                    s.receipt_row()
                }
            }
        };
        expected
            .and_then(|row| {
                if row == self.selected {
                    Ok(())
                } else {
                    Err(SourceJournalError::Binding)
                }
            })
            .inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedFailedObserveStateAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedFailedObserveStateAppendPermitV8 { owner: self })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        witness: &VerifiedFailedObserveStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            witness.validate_predecessor(
                self.journal(),
                self.sequence(),
                self.acknowledged_bytes(),
                &self.selected,
            )?;
            witness.validate_against_acknowledged_session(session)?;
            let incurred = !matches!(
                self.selected,
                EntryV8::Ordinary(SourceJournalEntry::Stop { .. })
            );
            self.context()
                .guard(session.sequence(), session.acknowledged_bytes(), incurred)?;
            // This is immediately post-ACK. Do not run the old failed owner/data
            // guard, cancellation or clock in the incurred Started/receipt window.
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedFailedObserveStateAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveFailedObserveStateAppendV8<'j>,
}
impl FixedFailedObserveStateAppendPermitV8<'_, '_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.owner.selected_row()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        j: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(j) {
            return Err(SourceJournalError::Binding);
        }
        self.owner.validate_live()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        j: &SourceOwnedWaitJournalV8,
        i: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<'_>,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(j) {
            return Err(SourceJournalError::Binding);
        }
        match self.owner.context() {
            FailedObserveContextV8::Initial(_) => {
                j.validate_initial_observe_registry()?;
                i.validate_failed_observe_cleanup_prefix(self.owner.selected_row())
            }
            FailedObserveContextV8::Continued(c) => c.append_prefix(i, self.owner.selected_row()),
            FailedObserveContextV8::Later(c) => c.append_prefix(i, self.owner.selected_row()),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        w: &VerifiedFailedObserveStateSuccessorV8<'_>,
        s: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        match self.owner.context() {
            FailedObserveContextV8::Initial(_) => {
                self.owner.journal().validate_initial_observe_registry()?;
                w.validate_against_acknowledged_session(s)
            }
            FailedObserveContextV8::Continued(c) => c.advance_ack(w, s),
            FailedObserveContextV8::Later(c) => c.advance_ack(w, s),
        }
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_failed_observe_state_v8<
    'j,
>(
    obligation: LiveFailedObserveStateAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedFailedObserveStateSuccessorV8<'j>,
) -> Result<LiveFailedObserveStateAcknowledgedV8<'j>, LiveFailedObserveStateFailureV8<'j>> {
    if let Err(error) = obligation.validate_successor(&witness, &session) {
        return Err(LiveFailedObserveStateFailureV8::Acknowledged {
            owner: obligation,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let ack = FailedObserveAckV8 { session, witness };
    let LiveFailedObserveStateAppendV8 { owner, selected } = obligation;
    match owner {
        FailedObserveAppendOwnerV8::Failed(source) => {
            let FailedObserveSourceV8 {
                owner,
                context,
                cache,
                mut acks,
            } = source;
            acks.push(ack);
            Ok(LiveFailedObserveStateAcknowledgedV8::Started(
                LiveStartedFailedObserveStateV8 {
                    owner,
                    lineage: FailedObserveLineageV8 {
                        context,
                        cache,
                        acks,
                    },
                },
            ))
        }
        FailedObserveAppendOwnerV8::Released(mut source) => {
            source.lineage.acks.push(ack);
            if matches!(selected, EntryV8::Ordinary(SourceJournalEntry::Stop { .. })) {
                Ok(LiveFailedObserveStateAcknowledgedV8::Stopped(source))
            } else {
                Ok(LiveFailedObserveStateAcknowledgedV8::Released(source))
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
