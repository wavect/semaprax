//! Typed effect forwarding for the opt-in interpreter model wait.
use super::*;
use crate::agent_lifecycle::iterative::model_wait::SourceModelWaitBinding;
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;

impl CompiledTypedEffects {
    pub(crate) fn run_live_durable_source_with_wait(
        &self,
        request: SourceLiveRequest<'_>,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TypedEffectHandler,
        effects: EffectBudget,
        store: &mut dyn CheckpointStore,
        wait: &SourceModelWaitBinding,
        key: &SourceCheckpointKey,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        let budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let mut dispatch = LiveDispatch {
            dispatch: Dispatch {
                compiled: self,
                proposals: &[],
                handler,
                budget,
                dispatched: 0,
                arguments: 0,
                results: 0,
                failure: None,
            },
            proposal: None,
        };
        self.lifecycle.run_live_durable_with_model_wait(
            request,
            source,
            &mut dispatch,
            store,
            wait,
            key,
        )
    }
}
