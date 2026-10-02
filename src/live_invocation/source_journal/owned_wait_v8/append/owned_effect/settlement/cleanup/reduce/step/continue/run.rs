//! One consuming second-turn composition. This is private runtime plumbing,
//! not a public session or a restoration route from journal evidence.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler;
use crate::cleanup_plan::FinalizeAction;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{
    LiveOwnedContinuedStartAppendV8, LiveOwnedObserveSettlementAppendV8, LiveSettledObserveV8,
};
use crate::live_invocation::source_journal::SourceTerminalEvidenceInput;
use crate::provider_adapter_sdk::StreamingSourceProposalAdapter;

// A private, methodless trait preserves the concrete owner-bearing failure
// without making any extraction, downcast, retry, or cleanup entry available.
trait RetainedFailure {}
impl<T> RetainedFailure for T {}

/// Must be retained by the enclosing runtime while its obligation is pending.
/// Dropping this holder performs no language cleanup or effect retry. Public
/// execution still requires a runtime-owned quarantine lifetime contract.
#[must_use = "retain the quarantine while its physical obligation is pending"]
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedRunQuarantineV8<'j> {
    phase: &'static str,
    _owner: Box<dyn RetainedFailure + 'j>,
}
impl ContinuedRunQuarantineV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn phase(&self) -> &'static str {
        self.phase
    }
}

fn quarantine<'j, T: 'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    phase: &'static str,
    owner: T,
) -> ContinuedRunQuarantineV8<'j> {
    journal.quarantine();
    ContinuedRunQuarantineV8 {
        phase,
        _owner: Box::new(owner),
    }
}

// The driver carries only heap owners between consuming operations. Keep the
// call boundary even in optimized builds so Result temporaries cannot combine
// into a frame spanning the entire second turn.
#[inline(never)]
pub(super) fn run_phase<T, E>(run: impl FnOnce() -> Result<T, E>) -> Result<Box<T>, E> {
    run().map(Box::new)
}

fn acknowledge_observe<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: Box<LiveOwnedObserveSettlementAppendV8<'j>>,
) -> Result<Box<LiveSettledObserveV8<'j>>, ContinuedRunQuarantineV8<'j>> {
    let appended = run_phase(|| {
        let session = match journal.begin_session() {
            Ok(session) => session,
            Err(error) => return Err(quarantine(journal, "observe-session", (owner, error))),
        };
        session
            .append_owned_observe_settlement(*owner)
            .map_err(|owner| quarantine(journal, "observe-append", owner))
    })?;
    run_phase(|| {
        (*appended)
            .advance_observe_settlement()
            .map_err(|owner| quarantine(journal, "observe-advance", owner))
    })
}

fn acknowledge_start<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: Box<LiveOwnedContinuedStartAppendV8<'j>>,
) -> Result<Box<LiveContinuedStartPhaseV8<'j>>, ContinuedRunQuarantineV8<'j>> {
    let appended = run_phase(|| {
        let session = match journal.begin_session() {
            Ok(session) => session,
            Err(error) => return Err(quarantine(journal, "start-session", (owner, error))),
        };
        session
            .append_owned_continued_start(*owner)
            .map_err(|owner| quarantine(journal, "start-append", owner))
    })?;
    run_phase(|| {
        (*appended)
            .advance_continued_start()
            .map_err(|owner| quarantine(journal, "start-advance", owner))
    })
}

/// Consumes the actual first Continue Step and completes one further turn.
/// Each dispatch, source entry and release is delegated to its existing fixed
/// ACK join. No row, receipt or caller-provided store constructs the owner.
/// Success exports only the terminal-ACK-bound canonical Report projection.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn finish_second_turn_v8<'j>(
    moved: LiveMovedStepV8<'j>,
    adapter: &mut StreamingSourceProposalAdapter<'_>,
    handler: &mut dyn TargetHostHandler,
    mut observe_cleanup: impl FnMut(&FinalizeAction),
) -> Result<ContinuedRunOutcomeV8<'j>, ContinuedRunQuarantineV8<'j>> {
    let journal = moved.journal();
    let admission = (|| {
        moved.validate_live()?;
        let context = journal.context();
        let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        if !context.fold().cumulative_initialization
            || execution.ordinary().max_iterations() != 2
            || moved.kind() != "continue"
        {
            return Err(SourceJournalError::Binding);
        }
        let current = journal.begin_session()?;
        let (_, _, turn, _) = current.inventory.continuation_facts()?;
        if turn != 0 {
            return Err(SourceJournalError::Order);
        }
        Ok(())
    })();
    if let Err(error) = admission {
        return Err(quarantine(journal, "admission", (moved, error)));
    }

    macro_rules! join {
        ($operation:expr, $phase:literal) => {
            run_phase(|| $operation.map_err(|owner| quarantine(journal, $phase, owner)))?
        };
    }
    let moved = Box::new(moved);
    let observed = join!(advance_live_owned_continue_v8(journal, *moved), "continue");
    let selected = join!((*observed).prepare_observe_settlement(), "observe-select");
    let settled = acknowledge_observe(journal, selected)?;
    match settled.failed() {
        Ok(true) => return Ok(ContinuedRunOutcomeV8::FailedObserve(*settled)),
        Ok(false) => {}
        Err(error) => return Err(quarantine(journal, "observe-outcome", (settled, error))),
    }
    let selected = join!((*settled).prepare_turn_observed(), "turn-observed-select");
    let settled = acknowledge_observe(journal, selected)?;
    let carried = join!((*settled).into_continued_wait(), "observe-carry");
    let created = join!((*carried).prepare_start_created(), "start-created");
    let created = acknowledge_start(journal, created)?;
    let reserved = join!((*created).prepare_start_reservation(), "start-reservation");
    let reserved = acknowledge_start(journal, reserved)?;
    let prepared = join!(
        advance_live_owned_continued_start_v8(journal, *reserved),
        "start"
    );
    let model = join!(
        advance_live_owned_continued_model_v8(journal, *prepared, adapter),
        "model-intent"
    );
    let model = join!(
        advance_live_owned_continued_dispatch_v8(journal, *model, adapter),
        "model-dispatch"
    );
    let model = join!(
        advance_live_owned_continued_resume_v8(journal, *model),
        "resume"
    );
    let completed = join!(
        advance_live_owned_continued_completed_v8(journal, *model),
        "completed"
    );
    let authorized = join!(
        advance_live_owned_continued_authorize_v8(journal, *completed),
        "authorize"
    );
    let effect = join!(
        advance_live_owned_continued_effect_v8(journal, *authorized),
        "effect"
    );
    let activated = join!(
        advance_live_owned_continued_intent_v8(journal, *effect),
        "effect-intent"
    );
    let recorded = join!(
        advance_live_owned_continued_effect_dispatch_v8(journal, *activated, handler),
        "effect-dispatch"
    );
    let cleanup = join!(
        advance_live_owned_continued_cleanup_v8(journal, *recorded, &mut observe_cleanup),
        "decision-cleanup"
    );
    if cleanup.failed_target() {
        return crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::failed_state::continued::stop_failed_continued_state(journal, cleanup, observe_cleanup)
            .map(ContinuedRunOutcomeV8::FailedEffectStopped)
            .map_err(|owner| quarantine(journal, "continued-failed-effect-cleanup", owner));
    }
    let reserved = join!(
        advance_live_owned_continued_reduce_v8(journal, *cleanup),
        "reduce-reservation"
    );
    let evaluated = join!((*reserved).evaluate(), "reduce");
    let selected = join!((*evaluated).prepare_step(), "step-select");
    let appended = run_phase(|| {
        let session = match journal.begin_session() {
            Ok(session) => session,
            Err(error) => return Err(quarantine(journal, "step-session", (selected, error))),
        };
        session
            .append_owned_step(*selected)
            .map_err(|owner| quarantine(journal, "step-append", owner))
    })?;
    let acknowledged = join!((*appended).advance_step(), "step-advance");
    let staged = run_phase(|| match *acknowledged {
        LiveStepAcknowledgedV8::Continued(staged) => Ok(staged),
        owner => Err(quarantine(journal, "step-shape", owner)),
    })?;
    // Stage counts come from the authenticated current prefix. The initial
    // composition deliberately emits the existing omitted-detail projection,
    // not caller-asserted stage evidence or a fabricated completed-stage count.
    let evidence = match journal.begin_session().and_then(|session| {
        let (_, stages, turn, attempt, _) = session.inventory.step_reduce_facts()?;
        if (turn, attempt) != (1, 0) {
            return Err(SourceJournalError::Order);
        }
        Ok(SourceTerminalEvidenceInput {
            completed_stages: stages,
            omitted_stage_rows: stages,
            stage_rows: Vec::new(),
            checked_run_evidence: None,
        })
    }) {
        Ok(evidence) => evidence,
        Err(error) => return Err(quarantine(journal, "terminal-evidence", (staged, error))),
    };
    (*staged)
        .finish_complete_projection(journal, observe_cleanup, evidence)
        .map(ContinuedRunOutcomeV8::Complete)
        .map_err(|owner| quarantine(journal, "terminal", owner))
}

impl<'j> LiveMovedStepV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn finish_second_turn(
        self,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn TargetHostHandler,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<ContinuedRunOutcomeV8<'j>, ContinuedRunQuarantineV8<'j>> {
        finish_second_turn_v8(self, adapter, handler, observe)
    }
}
