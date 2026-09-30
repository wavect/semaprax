//! Fixed cleanup ACKs consume the actual dispatched/released owners. Incurred
//! cleanup checks PID/pins/E/B/policy, not cancellation or fresh clock callbacks.
//! Outcome handoff restores full guards. No Reduce or recovered owner producer.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{PreparedContinuedDecisionCleanupV8, StartedContinuedDecisionCleanupV8, ReleasedContinuedDecisionCleanupV8, SettledContinuedDecisionCleanupV8};
use crate::cleanup_plan::FinalizeAction;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    ack_live_owned_effect_cleanup_v8, release_live_owned_effect_decision_v8,
    ExecutedOwnedAgentTurnV2, LiveEffectDecisionReleaseFailureV8, LiveEffectOutcomeFailureV8,
    OwnedEffectFailureV8, OwnedEffectInputsV8, PendingOwnedEffectReceiptV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedEffectCleanupSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::wire;
use crate::live_invocation::source_journal::SourceEffectFailure;
use serde_json::{json, Value};

struct CleanupAckV8<'j> {
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectCleanupSuccessorV8<'j>,
}
// Original sessions are inert causal lineage after the new actual cleanup ACK.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct CleanupLineageV8<'j> {
    recorded: RecordedLineageV8<'j>,
    started: CleanupAckV8<'j>,
    settled: Option<CleanupAckV8<'j>>,
}
struct RecordedLineageV8<'j> {
    intent: super::super::super::activation::IntentLineageV8<'j>,
    facts: CheckedLiveOwnedEffectSettlementV8,
    settlement: SettlementAckV8<'j>,
    recorded: SettlementAckV8<'j>,
}
// Keep cumulative lineage off the shared first-turn adapter's stack frames.
enum CleanupOwnerV8<'j> {
    Continued(Box<PreparedContinuedDecisionCleanupV8<'j>>),
    ContinuedReleased(Box<ReleasedContinuedDecisionCleanupV8<'j>>),
    Recorded(LiveRecordedOwnedEffectV8<'j>),
    Released(LiveReleasedOwnedEffectV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedEffectCleanupAppendV8<
    'j,
> {
    owner: CleanupOwnerV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveStartedOwnedEffectV8<'j> {
    owner: LiveRecordedOwnedEffectV8<'j>,
    ack: CleanupAckV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReleasedOwnedEffectV8<'j> {
    pending: PendingOwnedEffectReceiptV8<'j>,
    accounting: TargetAccounting,
    lineage: CleanupLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveCleanedOwnedEffectV8<'j> {
    pending: PendingOwnedEffectReceiptV8<'j>,
    accounting: TargetAccounting,
    lineage: CleanupLineageV8<'j>,
    observer_seal: Option<crate::live_invocation::source_journal::owned_wait_v8::append::observer_terminal::ObserverTerminalSealV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveExecutedOwnedEffectV8<'j> {
    executed: ExecutedOwnedAgentTurnV2<'j>,
    accounting: TargetAccounting,
    lineage: CleanupLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveFailedOwnedEffectV8<'j> {
    pending: PendingOwnedEffectReceiptV8<'j>,
    accounting: TargetAccounting,
    lineage: CleanupLineageV8<'j>,
    observer_seal: Option<crate::live_invocation::source_journal::owned_wait_v8::append::observer_terminal::ObserverTerminalSealV8<'j>>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveCleanupAcknowledgedV8<'j> {
    ContinuedStarted(Box<StartedContinuedDecisionCleanupV8<'j>>),
    ContinuedSettled(Box<SettledContinuedDecisionCleanupV8<'j>>),
    Started(LiveStartedOwnedEffectV8<'j>),
    Settled(LiveCleanedOwnedEffectV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveOutcomeV8<'j> {
    Executed(LiveExecutedOwnedEffectV8<'j>),
    Failed(LiveFailedOwnedEffectV8<'j>),
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveEffectCleanupFailureV8<'j> {
    Recorded {
        _owner: LiveRecordedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Started {
        _owner: LiveStartedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    AppendBefore {
        _owner: LiveOwnedEffectCleanupAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedEffectCleanupSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Release {
        _owner: LiveEffectDecisionReleaseFailureV8<'j>,
        _accounting: TargetAccounting,
        _lineage: CleanupLineageV8<'j>,
    },
    Released {
        _owner: LiveReleasedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Cleaned {
        _owner: LiveCleanedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    Outcome {
        _owner: LiveEffectOutcomeFailureV8<'j>,
        _accounting: TargetAccounting,
        _lineage: CleanupLineageV8<'j>,
    },
    Executed {
        _owner: LiveExecutedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
}
fn true_seq(sequence: usize) -> Result<u32, SourceJournalError> {
    u32::try_from(sequence.checked_sub(1).ok_or(SourceJournalError::Order)?)
        .map_err(|_| SourceJournalError::Capacity)
}
fn started_row(owner: &LiveRecordedOwnedEffectV8<'_>) -> Result<EntryV8, SourceJournalError> {
    owner.validate_live()?;
    let l = &owner.owner.lineage;
    let journal = l.journal;
    let (decision, operations) = owner.owner.staged.live_decision_cleanup_values_v8()?;
    let held = journal.hold()?;
    let scope = &held.registration().expected_facts().scope;
    let (_, e) = journal
        .context()
        .ready_runtime()
        .ok_or(SourceJournalError::Binding)?;
    let decision_digest = wire::recipe_digest(
        wire::RecipeV8::Decision,
        &json!({"scope":{"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()},"turn":0,"attempt":0,"authorize":e.wait().authorize().function().id.as_str(),"decision":decision}),
    )?;
    let commitment = json!({"turn":0,"attempt":0,"staged":l.staged,"ready":l.ready,"consumed":l.consumed,"intent":owner.facts.intent(),"settlement":true_seq(owner.settlement.session.sequence())?,"recorded":true_seq(owner.recorded.session.sequence())?,"decision_digest":decision_digest,"operations":operations});
    let operations_digest =
        wire::recipe_digest(wire::RecipeV8::EffectDecisionOperations, &commitment)?;
    Ok(EntryV8::Owned(
        OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
            turn: 0,
            attempt: 0,
            staged: l.staged,
            ready: l.ready,
            consumed: l.consumed,
            intent: owner.facts.intent(),
            settlement: true_seq(owner.settlement.session.sequence())?,
            recorded: true_seq(owner.recorded.session.sequence())?,
            decision_digest,
            operations,
            operations_digest,
        },
    ))
}
impl<'j> LiveRecordedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_cleanup(
        self,
    ) -> Result<LiveOwnedEffectCleanupAppendV8<'j>, LiveEffectCleanupFailureV8<'j>> {
        let selected = match started_row(&self) {
            Ok(row) => row,
            Err(error) => {
                return Err(LiveEffectCleanupFailureV8::Recorded {
                    _owner: self,
                    error,
                })
            }
        };
        Ok(LiveOwnedEffectCleanupAppendV8 {
            owner: CleanupOwnerV8::Recorded(self),
            selected,
        })
    }
}
impl<'j> CleanupLineageV8<'j> {
    fn current(&self) -> &CleanupAckV8<'j> {
        self.settled.as_ref().unwrap_or(&self.started)
    }
    fn permit(&self) -> LiveEffectDecisionCleanupPermitV8<'_, 'j> {
        LiveEffectDecisionCleanupPermitV8 { lineage: self }
    }
    fn validate_cleanup(&self) -> Result<(), SourceJournalError> {
        cleanup_current(&self.recorded.intent, self.current())
    }
    fn validate_outcome(&self) -> Result<(), SourceJournalError> {
        let result = (|| {
            if self.settled.is_none() {
                return Err(SourceJournalError::Binding);
            }
            self.validate_cleanup()?;
            let l = &self.recorded.intent;
            let ack = self.current();
            let held = l.journal.hold()?;
            let ordinary = l.journal.context().ordinary();
            check_clock_v8(
                &held,
                ack.session.sequence(),
                ack.session.acknowledged_bytes(),
                l.cancellation,
                l.clock,
                ordinary.clock_domain(),
                ordinary.initial_millis(),
                ordinary.deadline_millis(),
            )?;
            self.validate_cleanup()
        })();
        result.inspect_err(|_| self.recorded.intent.journal.quarantine())
    }
}
fn cleanup_current(
    origin: &super::super::super::activation::IntentLineageV8<'_>,
    ack: &CleanupAckV8<'_>,
) -> Result<(), SourceJournalError> {
    cleanup_current_refs(origin, &ack.session, &ack.witness)
}
fn cleanup_current_refs(
    origin: &super::super::super::activation::IntentLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedEffectCleanupSuccessorV8<'_>,
) -> Result<(), SourceJournalError> {
    let result = (|| {
        let journal = origin.journal;
        witness.validate_current_session(&session)?;
        origin.hold.validate_cleanup_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let held = journal.hold()?;
        held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            &origin.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        held.validate_prefix(session.sequence(), session.acknowledged_bytes())?;
        origin.hold.validate_cleanup_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        witness.validate_current_session(&session)
    })();
    result.inspect_err(|_| origin.journal.quarantine())
}
// Private borrowed permit is minted only from the same actual owner lineage.
pub(crate) struct LiveEffectDecisionCleanupPermitV8<'p, 'j> {
    lineage: &'p CleanupLineageV8<'j>,
}
impl LiveEffectDecisionCleanupPermitV8<'_, '_> {
    pub(crate) fn validate_cleanup_current(&self) -> Result<(), SourceJournalError> {
        self.lineage.validate_cleanup()
    }
    pub(crate) fn validate_outcome_current(&self) -> Result<(), SourceJournalError> {
        self.lineage.validate_outcome()
    }
    fn matches_inputs(&self, inputs: &OwnedEffectInputsV8<'_>) -> Result<(), SourceJournalError> {
        let l = &self.lineage.recorded.intent;
        let (runtime, execution) = l
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let ack = self.lineage.current();
        if !std::ptr::eq(runtime, inputs.runtime)
            || !std::ptr::eq(execution, inputs.execution)
            || !std::ptr::eq(l.policy, inputs.policy)
            || !std::ptr::eq(l.cancellation, inputs.cancellation)
            || inputs.turn != 0
            || inputs.attempt != 0
            || inputs.proposal.carrier() != l.proposal.carrier()
            || inputs.proposal.ordinary_digest() != l.proposal.ordinary_digest()
        {
            return Err(SourceJournalError::Binding);
        }
        inputs
            .store
            .validate_prefix(ack.session.sequence(), ack.session.acknowledged_bytes())
    }
    pub(crate) fn validate_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_cleanup_current()?;
        self.matches_inputs(inputs)?;
        self.validate_cleanup_current()
    }
    pub(crate) fn validate_outcome_guard(
        &self,
        inputs: &OwnedEffectInputsV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_outcome_current()?;
        self.matches_inputs(inputs)?;
        self.validate_outcome_current()
    }
    pub(crate) fn references(
        &self,
    ) -> Result<(u32, u32, u32, u32, u32, u32, u32), SourceJournalError> {
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
            staged,
            ready,
            consumed,
            intent,
            settlement,
            recorded,
            ..
        }) = self.lineage.started.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        Ok((
            *staged,
            *ready,
            *consumed,
            *intent,
            *settlement,
            *recorded,
            true_seq(self.lineage.started.session.sequence())?,
        ))
    }
    pub(crate) fn operations(&self) -> Result<&Value, SourceJournalError> {
        match self.lineage.started.witness.selected_row() {
            EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                operations, ..
            }) => Ok(operations),
            _ => Err(SourceJournalError::Binding),
        }
    }
    pub(crate) fn matches_settlement(
        &self,
        intent: u32,
        evidence: &str,
        observation: Option<&[u8]>,
        reason: Option<SourceEffectFailure>,
    ) -> bool {
        let facts = &self.lineage.recorded.facts;
        if intent != facts.intent() || evidence != facts.evidence_digest() {
            return false;
        }
        match facts.ordinary() {
            SourceJournalEntry::EffectObserved {
                observation: actual,
                ..
            } => observation == Some(actual.as_slice()) && reason.is_none(),
            SourceJournalEntry::EffectFailed { reason: actual, .. } => {
                observation.is_none() && reason == Some(*actual)
            }
            _ => false,
        }
    }
    pub(crate) fn settled_receipt(&self) -> Result<(u32, u32, &Value), SourceJournalError> {
        let ack = self
            .lineage
            .settled
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        match ack.witness.selected_row() {
            EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                started,
                receipt,
                ..
            }) => Ok((*started, true_seq(ack.session.sequence())?, receipt)),
            _ => Err(SourceJournalError::Binding),
        }
    }
}

impl<'j> LiveStartedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        cleanup_current(&self.owner.owner.lineage, &self.ack)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release_decision(
        self,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<LiveReleasedOwnedEffectV8<'j>, LiveEffectCleanupFailureV8<'j>> {
        if let Err(error) = self.validate_live() {
            return Err(LiveEffectCleanupFailureV8::Started {
                _owner: self,
                error,
            });
        }
        let Self {
            owner,
            ack: started,
        } = self;
        let LiveRecordedOwnedEffectV8 {
            owner:
                LiveDispatchedOwnedEffectV8 {
                    staged,
                    accounting,
                    lineage: intent,
                },
            facts,
            settlement,
            recorded,
        } = owner;
        let lineage = CleanupLineageV8 {
            recorded: RecordedLineageV8 {
                intent,
                facts,
                settlement,
                recorded,
            },
            started,
            settled: None,
        };
        let result = release_live_owned_effect_decision_v8(staged, &lineage.permit(), observe);
        match result {
            Ok(pending) => Ok(LiveReleasedOwnedEffectV8 {
                pending,
                accounting,
                lineage,
            }),
            Err(error) => Err(LiveEffectCleanupFailureV8::Release {
                _owner: error,
                _accounting: accounting,
                _lineage: lineage,
            }),
        }
    }
}
impl<'j> LiveReleasedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.lineage.validate_cleanup()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_settled(
        self,
    ) -> Result<LiveOwnedEffectCleanupAppendV8<'j>, LiveEffectCleanupFailureV8<'j>> {
        let selected = (|| {
            self.validate_live()?;
            let receipt = self.pending.receipt().clone();
            let receipt_digest = wire::recipe_digest(wire::RecipeV8::Receipt, &receipt)?;
            Ok(EntryV8::Owned(
                OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                    turn: 0,
                    attempt: 0,
                    started: true_seq(self.lineage.started.session.sequence())?,
                    receipt,
                    receipt_digest,
                },
            ))
        })();
        match selected {
            Ok(selected) => Ok(LiveOwnedEffectCleanupAppendV8 {
                owner: CleanupOwnerV8::Released(self),
                selected,
            }),
            Err(error) => Err(LiveEffectCleanupFailureV8::Released {
                _owner: self,
                error,
            }),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failure(
        &self,
    ) -> Option<OwnedEffectFailureV8> {
        self.pending.failure()
    }
}
impl<'j> LiveCleanedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        if self.observer_seal.is_some() {
            self.validate_observer_seal()
        } else {
            self.lineage.validate_cleanup()
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_outcome(
        self,
    ) -> Result<LiveOutcomeV8<'j>, LiveEffectCleanupFailureV8<'j>> {
        if let Err(error) = self.validate_live() {
            return Err(LiveEffectCleanupFailureV8::Cleaned {
                _owner: self,
                error,
            });
        }
        if self.pending.failure().is_some() {
            let Self {
                pending,
                accounting,
                lineage,
                observer_seal,
            } = self;
            return Ok(LiveOutcomeV8::Failed(LiveFailedOwnedEffectV8 {
                pending,
                accounting,
                lineage,
                observer_seal,
            }));
        }
        if let Err(error) = self.lineage.validate_outcome() {
            return Err(LiveEffectCleanupFailureV8::Cleaned {
                _owner: self,
                error,
            });
        }
        let Self {
            pending,
            accounting,
            lineage,
            observer_seal: _,
        } = self;
        let result = ack_live_owned_effect_cleanup_v8(pending, &lineage.permit());
        match result {
            Err(error) => Err(LiveEffectCleanupFailureV8::Outcome {
                _owner: error,
                _accounting: accounting,
                _lineage: lineage,
            }),
            Ok(executed) => {
                let actual = LiveExecutedOwnedEffectV8 {
                    executed,
                    accounting,
                    lineage,
                };
                if let Err(error) = actual.validate_live() {
                    return Err(LiveEffectCleanupFailureV8::Executed {
                        _owner: actual,
                        error,
                    });
                }
                Ok(LiveOutcomeV8::Executed(actual))
            }
        }
    }
}
impl LiveExecutedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.lineage.validate_outcome()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
}
impl LiveFailedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failure(
        &self,
    ) -> Option<OwnedEffectFailureV8> {
        self.pending.failure()
    }
}
impl<'j> LiveOwnedEffectCleanupAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn from_continued(owner: PreparedContinuedDecisionCleanupV8<'j>) -> Self {
        let selected = owner.selected().clone();
        Self { owner: CleanupOwnerV8::Continued(Box::new(owner)), selected }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn from_continued_released(owner: ReleasedContinuedDecisionCleanupV8<'j>, selected: EntryV8) -> Self { Self { owner: CleanupOwnerV8::ContinuedReleased(Box::new(owner)), selected } }
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        match &self.owner {
            CleanupOwnerV8::Continued(o) => o.journal(),
            CleanupOwnerV8::ContinuedReleased(o) => o.journal(),
            CleanupOwnerV8::Recorded(o) => o.owner.lineage.journal,
            CleanupOwnerV8::Released(o) => o.lineage.recorded.intent.journal,
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        match &self.owner {
            CleanupOwnerV8::Continued(o) => o.session().sequence(),
            CleanupOwnerV8::ContinuedReleased(o) => o.session().sequence(),
            CleanupOwnerV8::Recorded(o) => o.recorded.session.sequence(),
            CleanupOwnerV8::Released(o) => o.lineage.current().session.sequence(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        match &self.owner {
            CleanupOwnerV8::Continued(o) => o.session().acknowledged_bytes(),
            CleanupOwnerV8::ContinuedReleased(o) => o.session().acknowledged_bytes(),
            CleanupOwnerV8::Recorded(o) => o.recorded.session.acknowledged_bytes(),
            CleanupOwnerV8::Released(o) => o.lineage.current().session.acknowledged_bytes(),
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
        let result = match &self.owner {
            CleanupOwnerV8::ContinuedReleased(o) => o.selected().and_then(|row| if row == self.selected {Ok(())}else{Err(SourceJournalError::Binding)}),
            CleanupOwnerV8::Continued(o) => o.validate_live().and_then(|_| if o.selected() == &self.selected { Ok(()) } else { Err(SourceJournalError::Binding) }),
            CleanupOwnerV8::Recorded(o) => started_row(o).and_then(|row| {
                if row == self.selected {
                    Ok(())
                } else {
                    Err(SourceJournalError::Binding)
                }
            }),
            CleanupOwnerV8::Released(o) => {
                o.validate_live()?;
                let receipt = o.pending.receipt();
                match &self.selected {
                    EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                        turn: 0,
                        attempt: 0,
                        started,
                        receipt: actual,
                        receipt_digest,
                    }) if *started == true_seq(o.lineage.started.session.sequence())?
                        && actual == receipt
                        && *receipt_digest
                            == wire::recipe_digest(wire::RecipeV8::Receipt, receipt)? =>
                    {
                        Ok(())
                    }
                    _ => Err(SourceJournalError::Binding),
                }
            }
        };
        result.inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedEffectCleanupAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedEffectCleanupAppendPermitV8 { owner: self })
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_cleanup_successor(
        &self,
        witness: &VerifiedOwnedEffectCleanupSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.journal(),
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        witness.validate_current_session(session)?;
        let origin = match &self.owner {
            CleanupOwnerV8::Continued(o) => return o.validate_successor(session, witness),
            CleanupOwnerV8::ContinuedReleased(o) => return o.validate_successor(session, witness),
            CleanupOwnerV8::Recorded(o) => &o.owner.lineage,
            CleanupOwnerV8::Released(o) => &o.lineage.recorded.intent,
        };
        cleanup_current_refs(origin, session, witness)
    }
}

/// Borrowed exclusively from the actual pre-release or released owner.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedEffectCleanupAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedEffectCleanupAppendV8<'j>,
}
impl FixedOwnedEffectCleanupAppendPermitV8<'_, '_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.owner.selected_row()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        self.owner.validate_live()
    }
    fn hold(&self) -> Result<&ProspectiveOwnedReduceHoldV8<'_>, SourceJournalError> {
        match &self.owner.owner {
            CleanupOwnerV8::Continued(o) => o.hold(),
            CleanupOwnerV8::ContinuedReleased(o) => o.hold(),
            CleanupOwnerV8::Recorded(o) => Ok(&o.owner.lineage.hold),
            CleanupOwnerV8::Released(o) => Ok(&o.lineage.recorded.intent.hold),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.hold()?
            .validate_cleanup_append_prefix(journal, inventory, &self.owner.selected)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedEffectCleanupSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.hold()?.advance_cleanup_ack(witness, session)
    }
}
/// The verified session is consumed together with the actual owner. No ACK can
/// be manufactured from recovered rows, and post-Started checks use guard B.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_cleanup_v8<'j>(
    obligation: LiveOwnedEffectCleanupAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectCleanupSuccessorV8<'j>,
) -> Result<LiveCleanupAcknowledgedV8<'j>, LiveEffectCleanupFailureV8<'j>> {
    if let Err(error) = obligation.validate_cleanup_successor(&witness, &session) {
        obligation.journal().quarantine();
        return Err(LiveEffectCleanupFailureV8::AppendBefore {
            _owner: obligation,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let LiveOwnedEffectCleanupAppendV8 { owner, selected: _ } = obligation;
    let ack = CleanupAckV8 { session, witness };
    match owner {
        CleanupOwnerV8::Continued(owner) => Ok(LiveCleanupAcknowledgedV8::ContinuedStarted(Box::new((*owner).acknowledge(ack.session, ack.witness)))),
        CleanupOwnerV8::ContinuedReleased(owner) => Ok(LiveCleanupAcknowledgedV8::ContinuedSettled(Box::new((*owner).acknowledge(ack.session, ack.witness)))),
        CleanupOwnerV8::Recorded(owner) => {
            let actual = LiveStartedOwnedEffectV8 { owner, ack };
            if let Err(error) = actual.validate_live() {
                return Err(LiveEffectCleanupFailureV8::Started {
                    _owner: actual,
                    error,
                });
            }
            Ok(LiveCleanupAcknowledgedV8::Started(actual))
        }
        CleanupOwnerV8::Released(owner) => {
            let LiveReleasedOwnedEffectV8 {
                pending,
                accounting,
                mut lineage,
            } = owner;
            lineage.settled = Some(ack);
            let mut actual = LiveCleanedOwnedEffectV8 {
                pending,
                accounting,
                lineage,
                observer_seal: None,
            };
            if let Err(error) = actual.lineage.validate_cleanup() {
                return Err(LiveEffectCleanupFailureV8::Cleaned {
                    _owner: actual,
                    error,
                });
            }
            if let Err(error) = actual.install_observer_seal() {
                actual.lineage.recorded.intent.journal.quarantine();
                return Err(LiveEffectCleanupFailureV8::Cleaned {
                    _owner: actual,
                    error,
                });
            }
            Ok(LiveCleanupAcknowledgedV8::Settled(actual))
        }
    }
}

#[cfg(test)]
impl LiveExecutedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_outcome_weak(
        &self,
    ) -> std::sync::Weak<[u8]> {
        self.executed.test_live_outcome_weak_v8()
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod reduce;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod failed_state;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod observer_failed_state;
