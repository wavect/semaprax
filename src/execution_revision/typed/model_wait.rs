//! Additive checked model-wait selection for standalone Direct Runtime v2.
use super::*;
pub use crate::agent_lifecycle::iterative::model_wait::SourceModelWaitBinding;

impl AgentRuntimeV2 {
    pub fn source_model_wait_binding(
        &self,
        wrapper_id: &str,
        evaluation_fuel: usize,
    ) -> Result<SourceModelWaitBinding> {
        if evaluation_fuel > self.budget.max_steps_per_stage {
            return Err(refused("source.model_wait_stage_fuel"));
        }
        self.lifecycle
            .source_lifecycle()
            .model_wait_binding(wrapper_id, evaluation_fuel)
    }
}
