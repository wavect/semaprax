//! Consuming first-turn bridge. Every failed join keeps its exact physical
//! predecessor in runtime custody; no serialized row can select this entry.
use super::super::authorize::authorize_live_actor_v8;
use super::super::effect::authorization::{
    cleanup::{LiveCleanupAcknowledgedV8, LiveFailedOwnedEffectV8, LiveOutcomeV8},
    step::LiveStepAcknowledgedV8,
    LiveEffectSettlementAcknowledgedV8,
};
use super::super::effect::prepare_live_effect_ready_v8;
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler;
use crate::resumable_effects::CapabilityPolicy;

trait RetainedFailure {}
impl<T> RetainedFailure for T {}

pub(super) struct RunQuarantineV8<'j> {
    phase: &'static str,
    _owner: Box<dyn RetainedFailure + 'j>,
}
pub(super) enum RunOutcomeV8<'j> {
    Complete(serde_json::Value),
    FailedEffect(LiveFailedOwnedEffectV8<'j>),
    FailedObserve(LiveSettledObserveV8<'j>),
}
impl RunQuarantineV8<'_> {
    pub(super) fn phase(&self) -> &'static str {
        self.phase
    }
}
fn quarantine<'j, T: 'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    phase: &'static str,
    owner: T,
) -> RunQuarantineV8<'j> {
    journal.quarantine();
    RunQuarantineV8 {
        phase,
        _owner: Box::new(owner),
    }
}

// Each phase returns its physical successor on the heap before the next phase
// starts. Keeping this call boundary prevents unoptimized builds from retaining
// every large consuming Result temporary in one driver stack frame.
#[inline(never)]
fn run_phase<'j, T>(
    run: impl FnOnce() -> Result<T, RunQuarantineV8<'j>>,
) -> Result<Box<T>, RunQuarantineV8<'j>> {
    run().map(Box::new)
}

pub(super) fn finish_run<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    completed: CompletedLiveOwnedRunV8<'j>,
    policy: &'j CapabilityPolicy,
    adapter: &mut StreamingSourceProposalAdapter<'_>,
    handler: &mut dyn TargetHostHandler,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<RunOutcomeV8<'j>, RunQuarantineV8<'j>> {
    macro_rules! join {
        ($operation:expr, $phase:literal) => {
            run_phase(|| $operation.map_err(|owner| quarantine(journal, $phase, owner)))?
        };
    }
    // A refused current-session acquisition must retain the still-unconsumed
    // append obligation, just like a refused physical append or ACK advance.
    macro_rules! ack {
        ($owner:expr, $append:ident, $advance:ident, $phase:literal) => {{
            let owner = $owner;
            let appended = run_phase(|| {
                let session = match journal.begin_session() {
                    Ok(session) => session,
                    Err(error) => return Err(quarantine(journal, $phase, (owner, error))),
                };
                session
                    .$append(*owner)
                    .map_err(|owner| quarantine(journal, $phase, owner))
            })?;
            join!((*appended).$advance(), $phase)
        }};
    }
    macro_rules! shape {
        ($value:expr, $case:path, $phase:literal) => {
            run_phase(|| match *$value {
                $case(owner) => Ok(owner),
                owner => Err(quarantine(journal, $phase, owner)),
            })?
        };
    }
    let completed = Box::new(completed);
    let staged = join!(authorize_live_actor_v8(*completed), "first-authorize");
    let ready = join!(prepare_live_effect_ready_v8(*staged, policy), "first-ready");
    let consumed = ack!(ready, append_owned_effect, advance_ready, "first-ready");
    let held = ack!(
        consumed,
        append_owned_authorization_consumed,
        reserve_owned_reduce,
        "first-consumed"
    );
    let prepared = join!((*held).advance_authorization(), "first-effect");
    let intent = join!((*prepared).prepare_intent(), "first-intent");
    let activated = ack!(
        intent,
        append_owned_effect_intent,
        advance_intent,
        "first-intent"
    );
    let dispatched = join!((*activated).dispatch(handler), "first-dispatch");
    let selected = join!((*dispatched).prepare_settlement(), "first-settlement");
    let settled = shape!(
        ack!(
            selected,
            append_owned_effect_settlement,
            advance_settlement,
            "first-settlement"
        ),
        LiveEffectSettlementAcknowledgedV8::Settled,
        "first-settlement-shape"
    );
    let selected = join!((*settled).prepare_recorded(), "first-recorded");
    let recorded = shape!(
        ack!(
            selected,
            append_owned_effect_settlement,
            advance_settlement,
            "first-recorded"
        ),
        LiveEffectSettlementAcknowledgedV8::Recorded,
        "first-recorded-shape"
    );
    let selected = join!((*recorded).prepare_cleanup(), "first-cleanup-started");
    let started = shape!(
        ack!(
            selected,
            append_owned_effect_cleanup,
            advance_cleanup,
            "first-cleanup-started"
        ),
        LiveCleanupAcknowledgedV8::Started,
        "first-cleanup-started-shape"
    );
    let released = join!(
        (*started).release_decision(&mut observe),
        "first-cleanup-release"
    );
    let selected = join!((*released).prepare_settled(), "first-cleanup-receipt");
    let cleaned = shape!(
        ack!(
            selected,
            append_owned_effect_cleanup,
            advance_cleanup,
            "first-cleanup-receipt"
        ),
        LiveCleanupAcknowledgedV8::Settled,
        "first-cleanup-receipt-shape"
    );
    let executed = match *join!((*cleaned).advance_outcome(), "first-outcome") {
        LiveOutcomeV8::Executed(owner) => Box::new(owner),
        // The actual failed target has a distinct checked State cleanup and
        // sticky Stop tail. Preserve that owner type in runtime custody.
        LiveOutcomeV8::Failed(owner) => return Ok(RunOutcomeV8::FailedEffect(owner)),
    };
    let selected = join!((*executed).prepare_reduce(), "first-reduce");
    let evaluated = ack!(
        selected,
        append_owned_reduce_reservation,
        advance_reduce,
        "first-reduce"
    );
    let selected = join!((*evaluated).prepare_step(), "first-step");
    let staged = shape!(
        ack!(selected, append_owned_step, advance_step, "first-step"),
        LiveStepAcknowledgedV8::Staged,
        "first-step-shape"
    );
    let selected = join!((*staged).prepare_cleanup(), "first-step-cleanup");
    let staged = shape!(
        ack!(
            selected,
            append_owned_step,
            advance_step,
            "first-step-cleanup"
        ),
        LiveStepAcknowledgedV8::Staged,
        "first-step-cleanup-shape"
    );
    let released = join!((*staged).release(&mut observe), "first-step-release");
    let selected = join!((*released).prepare_receipt(), "first-step-receipt");
    let released = shape!(
        ack!(
            selected,
            append_owned_step,
            advance_step,
            "first-step-receipt"
        ),
        LiveStepAcknowledgedV8::Released,
        "first-step-receipt-shape"
    );
    let ready = join!((*released).into_ready(), "first-step-ready");
    let selected = join!((*ready).prepare_transfer(), "first-step-transfer");
    let ready = shape!(
        ack!(
            selected,
            append_owned_step,
            advance_step,
            "first-step-transfer"
        ),
        LiveStepAcknowledgedV8::Ready,
        "first-step-transfer-shape"
    );
    let moved = join!((*ready).move_fields(), "first-step-move");
    let selected = join!((*moved).prepare_completed(), "first-step-completed");
    let moved = shape!(
        ack!(
            selected,
            append_owned_step,
            advance_step,
            "first-step-completed"
        ),
        LiveStepAcknowledgedV8::Moved,
        "first-step-completed-shape"
    );
    let selected = join!((*moved).prepare_transition(), "first-transition");
    let moved = shape!(
        ack!(
            selected,
            append_owned_step,
            advance_step,
            "first-transition"
        ),
        LiveStepAcknowledgedV8::Moved,
        "first-transition-shape"
    );
    // The successor independently checks actual Continue, turn zero, and the
    // cumulative two-turn ceiling before another reservation or dispatch.
    (*moved)
        .finish_second_turn(adapter, handler, observe)
        .map(|outcome| {
            use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::ContinuedRunOutcomeV8;
            match outcome {
                ContinuedRunOutcomeV8::Complete(projection) => RunOutcomeV8::Complete(projection),
                ContinuedRunOutcomeV8::FailedObserve(owner) => RunOutcomeV8::FailedObserve(owner),
            }
        })
        .map_err(|owner| quarantine(journal, owner.phase(), owner))
}
