//! Opt-in observed semantic work on the ordinary public target driver.
use super::*;
use crate::agent_lifecycle::authorization::{target_protocol::TargetHostHandler, StageBackend};
use crate::agent_lifecycle::iterative::driver::ProposalSource;
use crate::interpreter::retained_call::SemanticWork;
use std::cell::RefCell;

/// One settled stage's observed work, in lifecycle execution order.
/// Instruction steps are intentionally absent; interpreter finalizers are None.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageSemanticObservation {
    pub(crate) function_id: crate::hir::DeclarationId,
    pub(crate) work: SemanticWork,
}
impl StageSemanticObservation {
    pub fn function_id(&self) -> &str {
        self.function_id.as_str()
    }
    pub fn work(&self) -> &SemanticWork {
        &self.work
    }
}

/// A target run plus bounded, observed semantic work and its exact evidence.
/// The ordinary run retains its existing reservation accounting and wire.
pub struct MeteredTargetEffectRun {
    run: TargetEffectRun,
    observations: Vec<StageSemanticObservation>,
    evidence: String,
    digest: String,
}
impl MeteredTargetEffectRun {
    pub fn run(&self) -> &TargetEffectRun {
        &self.run
    }
    pub fn observations(&self) -> &[StageSemanticObservation] {
        &self.observations
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    pub fn evidence_digest(&self) -> &str {
        &self.digest
    }
}

impl CompiledTypedEffects {
    /// Run the public target lifecycle with Agent Stage Semantic Work v1.
    ///
    /// Each stage admits its reachable metered profile before target execution.
    /// Unsupported constructs and limits outside 1..=1_000_000 fail closed.
    /// Cancellation retains precedence. No stage falls back to unmetered work.
    /// The limit and selected held host are bound into every target grant.
    #[allow(clippy::too_many_arguments)]
    pub fn run_target_live_metered(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TargetHostHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        selected: TargetStageBackend<'_>,
        semantic_fuel_limit: u64,
    ) -> Result<MeteredTargetEffectRun, Vec<Diagnostic>> {
        if !cancellation.is_cancelled() && !(1..=1_000_000).contains(&semantic_fuel_limit) {
            return Err(vec![crate::agent_lifecycle::stages::invariant(
                "semantic_work.fuel_limit",
            )]);
        }
        let selected = self.selected_target_backend(selected)?;
        let observations = RefCell::new(Vec::new());
        let backend = StageBackend::Metered {
            backend: &selected,
            fuel_limit: semantic_fuel_limit,
            observations: &observations,
        };
        let binding = self.target_execution_binding(backend);
        let run = self.run_target_live_inner(
            task,
            source,
            handler,
            stages,
            effects,
            cancellation,
            backend,
            Some(binding.clone()),
        )?;
        let observations = observations.into_inner();
        if observations.len() != run.lifecycle().stages().len()
            || observations
                .iter()
                .zip(run.lifecycle().stages())
                .any(|(observation, stage)| observation.function_id() != stage.function_id())
        {
            return Err(error("target.semantic_work.stage_count"));
        }
        let rows = observations
            .iter()
            .map(|observation| {
                let work = &observation.work;
                let events = work.finalizer_events.as_ref().map(|events| {
                    events
                        .iter()
                        .map(|event| {
                            serde_json::json!([event.function.as_str(), event.liveness_flag])
                        })
                        .collect::<Vec<_>>()
                });
                serde_json::json!({
                    "function": observation.function_id(),
                    "fuel_used": work.fuel_used,
                    "fuel_limit": work.fuel_limit,
                    "exhausted": work.exhausted,
                    "finalizer_events": events,
                })
            })
            .collect::<Vec<_>>();
        let mut document = serde_json::json!({
            "schema": "semaprax.agent-target-semantic-work.v1",
            "execution_binding": binding,
            "target_evidence": run.evidence_digest(),
            "semantic_fuel_limit": semantic_fuel_limit,
            "stages": rows,
        });
        document.sort_all_objects();
        let evidence = format!("{document}\n");
        let digest = digest(
            b"semaprax.agent-target-semantic-work.v1\0",
            evidence.as_bytes(),
        );
        Ok(MeteredTargetEffectRun {
            run,
            observations,
            evidence,
            digest,
        })
    }
}

#[cfg(test)]
mod tests;
