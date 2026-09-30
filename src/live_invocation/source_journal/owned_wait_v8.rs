//! Private v8 journal proof data and fixed physical append adapter.
//! Authenticated inventory carries no owner restoration or effect authority.
mod append;
pub(crate) use append::{HeldOwnedWaitStoreV8, SourceOwnedWaitJournalV8};
mod candidate;
mod capacity;
mod checked_context;
mod fold;
mod inventory;
pub(crate) use inventory::CheckedCumulativeEffectPrefixV8;
mod model;
mod ready_commitment;
mod reduce_fold;
mod reduce_inventory;
mod reduce_model;
mod reduce_wire;
pub(crate) use ready_commitment::owned_wait_ready_commitment_v8;
#[cfg(test)]
mod tests;
mod wire;

use super::{SourceInvocationBinding, SourceJournalEntry, SourceJournalError};
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;
use serde_json::Value;

pub(super) const SCHEMA: &str = "semaprax.live-invocation.source-persisted-journal.v8";
pub(super) const RECORD_DOMAIN: &[u8] = b"semaprax.live-invocation.source-record.v8\0";
const MAX_DEPTH: usize = 24;

/// Inert body only; even authenticated decoding cannot construct a runtime owner.
#[derive(Clone, Debug, Eq, PartialEq)]
enum EntryV8 {
    Ordinary(SourceJournalEntry),
    Owned(model::OwnedBodyV8),
}

/// Expected chain facts borrowed from the independently checked context.
/// There is deliberately no public or production context factory in this slice.
pub(super) struct ExpectedRowV8<'a> {
    pub invocation: &'a str,
    pub generation: &'a str,
    pub seq: u32,
    pub prev_mac: &'a str,
    pub ordinary: &'a SourceInvocationBinding,
}

/// The closed typed inventory binder creates this after actual carrier replay.
/// Ready and effect commitments are inert; legacy cleanup remains refused.
pub(super) struct ValidatedEntryV8 {
    entry: EntryV8,
    observation: Option<crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8>,
}

/// Sealed expected facts, to be constructed by the actual typed binding/store
/// join. Neither codec bytes nor generic callbacks can mint this context.
pub(super) struct FoldContextV8 {
    ordinary: SourceInvocationBinding,
    initialized_task: Option<Value>,
    cumulative_initialization: bool,
    created: model::OwnedBodyV8,
    plan_digest: String,
    cleanup_plan_digest: String,
    signature: Value,
    helper: String,
    authorize: String,
    granted: String,
    refused: String,
    refused_cleanup_empty: bool,
    maxima_templates: capacity::MaximaTemplateCacheV8,
    reduce_templates: capacity::ReduceTemplateCacheV8,
    future_templates: capacity::FutureTemplateCacheV8,
    checked_reduce: Result<
        std::sync::Arc<crate::resumable_effects::owned_frame::v2::CheckedOwnedReduceV2>,
        SourceJournalError,
    >,
    checked_binding:
        std::sync::Arc<crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8>,
}

impl FoldContextV8 {
    // Immutable compiler proof only. Deferred refusal remains at its old use sites.
    fn checked_reduce(
        &self,
    ) -> Result<
        &std::sync::Arc<crate::resumable_effects::owned_frame::v2::CheckedOwnedReduceV2>,
        SourceJournalError,
    > {
        let plan = self.checked_reduce.as_ref().map_err(|error| *error)?;
        if plan.binding() != self.checked_binding.binding()
            || !plan.helper().same_helper(self.checked_binding.helper())
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(plan)
    }
}

pub(crate) use checked_context::{
    checked_owned_wait_journal_context_v8, CheckedOwnedWaitJournalContextV8,
};

mod live_upstream;
pub(crate) use live_upstream::effect::authorization::LiveReadyPromotionPermitV8;
pub(crate) use live_upstream::{
    LiveInitializePermitV8, LiveObservePermitV8, LiveWaitStartPermitV8,
};

pub(crate) use live_upstream::model::{LiveModelIntentPermitV8, LiveWaitResumePermitV8};

pub(crate) use live_upstream::authorize::{LiveAuthorizePermitV8, LiveStateTransferPermitV8};
pub(crate) use live_upstream::effect::authorization::{
    LiveEffectAuthorizationPermitV8, LiveEffectIntentPermitV8,
};

pub(crate) use live_upstream::effect::authorization::cleanup::LiveEffectDecisionCleanupPermitV8;

pub(crate) use live_upstream::effect::authorization::cleanup::reduce::LiveReduceEvaluationPermitV8;

pub(crate) use live_upstream::effect::authorization::step::{
    FixedOwnedStepAppendPermitV8, LiveOwnedReduceCleanupPermitV8, LiveOwnedStepTransferPermitV8,
};

pub(crate) use live_upstream::effect::authorization::failed_state::LiveFailedEffectStateCleanupPermitV8;

pub(crate) use live_upstream::effect::authorization::step::r#continue::LiveContinueObservePermitV8;

#[cfg(test)]
pub(crate) use live_upstream::test_initial_observe_entry_v8;

pub(crate) use live_upstream::effect::authorization::observer_failed_state::state::LiveObserverFailedStateCleanupPermitV8;

pub(crate) use live_upstream::LiveFailedObserveStateCleanupPermitV8;

pub(crate) use live_upstream::{
    LiveContinuedModelIntentPermitV8, LiveContinuedModelRequestOriginV8,
    LiveContinuedWaitResumePermitV8,
};

pub(crate) use live_upstream::{
    LiveContinuedAuthorizePermitV8, LiveContinuedStateTransferPermitV8,
};

pub(crate) use live_upstream::LiveContinuedReadyPromotionPermitV8;

pub(crate) use live_upstream::LiveContinuedEffectAuthorizationPermitV8;

pub(crate) use live_upstream::LiveContinuedIntentPermitV8;

pub(crate) use live_upstream::LiveContinuedSettlementPermitV8;

pub(crate) use live_upstream::LiveContinuedDecisionCleanupPermitV8;
