//! A physical later completion joins the existing authorization pipeline.
//! History is moved from the live wrappers; journal replay cannot construct it.
use super::resume::{PreparedHistoryV8, ResumedModelOwnerV8};
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedStartSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedResumedWaitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LaterModelHistoryV8<'j> {
    history: PreparedHistoryV8<'j>,
    request: CheckedOwnedModelRequestV8,
    ordinal: u32,
    dispatched: Option<OwnedModelSettlementV8>,
    acks: Vec<ModelAckV8<'j>>,
}
impl<'j> LaterModelHistoryV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn new(
        observation: CheckedOwnedWaitObservationV8,
        observe_acks: Vec<ObserveSettlementAckV8<'j>>,
        start_acks: Vec<(
            AppendSessionV8<'j>,
            VerifiedOwnedContinuedStartSuccessorV8<'j>,
        )>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedPreparedSuccessorV8<'j>,
        wait: String,
        request: CheckedOwnedModelRequestV8,
        ordinal: u32,
        dispatched: Option<OwnedModelSettlementV8>,
        acks: Vec<(
            AppendSessionV8<'j>,
            VerifiedOwnedContinuedModelSuccessorV8<'j>,
        )>,
    ) -> Self {
        Self {
            history: PreparedHistoryV8 {
                observation,
                _observe_acks: observe_acks,
                _start_acks: start_acks
                    .into_iter()
                    .map(|(session, witness)| ContinuedStartAckV8 { session, witness })
                    .collect(),
                session,
                _witness: witness,
                wait,
            },
            request,
            ordinal,
            dispatched,
            acks: acks
                .into_iter()
                .map(|(session, witness)| ModelAckV8 { session, witness })
                .collect(),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn join(
        self,
        owner: ContinuedResumedWaitV8<'j>,
        proposal: CheckedOwnedWaitProposalV8,
        resume_session: AppendSessionV8<'j>,
        resume_witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedContinuedModelSuccessorV8<'j>,
    ) -> LiveContinuedModelV8<'j> {
        let Self {
            history,
            request,
            ordinal,
            dispatched,
            mut acks,
        } = self;
        acks.push(ModelAckV8 {
            session: resume_session,
            witness: resume_witness,
        });
        acks.push(ModelAckV8 { session, witness });
        LiveContinuedModelV8 {
            owner: ModelOwnerV8::Resumed(ResumedModelOwnerV8 { owner, history }),
            request,
            ordinal,
            acks,
            dispatched,
            proposal: Some(proposal),
        }
    }
}
