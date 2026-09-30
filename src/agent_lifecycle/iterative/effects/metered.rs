//! Opt-in observed semantic work on the ordinary public target driver.
use super::*;
use crate::agent_lifecycle::authorization::{target_protocol::TargetHostHandler, StageBackend};
use crate::agent_lifecycle::iterative::driver::ProposalSource;
use crate::interpreter::retained_call::{
    PreparedRetainedCall, RetainedCallEvaluation, SemanticWork,
};
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

/// A fresh durable run plus the exact observed semantic work for its stages.
///
/// The existing durable checkpoint schema does not authenticate semantic-work
/// receipts. Until it does, resuming a checkpoint through this route is
/// refused before store access or target dispatch rather than producing
/// incomplete evidence for historical stages.
pub struct MeteredDurableTypedRun {
    run: DurableTypedRun,
    observations: Vec<StageSemanticObservation>,
    observations_complete: bool,
    evidence: String,
    digest: String,
}
impl MeteredDurableTypedRun {
    pub fn run(&self) -> &DurableTypedRun {
        &self.run
    }
    pub fn observations(&self) -> &[StageSemanticObservation] {
        &self.observations
    }
    pub fn observations_complete(&self) -> bool {
        self.observations_complete
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    pub fn evidence_digest(&self) -> &str {
        &self.digest
    }
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

/// One selected target evaluation of a retained call used by a durable
/// migration. The binding identifies the held target and the exact typed
/// registry; the evaluation keeps instruction steps separate from the
/// backend-neutral semantic-work receipt.
pub(crate) struct MeteredTargetRetainedCall {
    pub(crate) evaluation: RetainedCallEvaluation,
    pub(crate) execution_binding: String,
}

impl CompiledTypedEffects {
    /// Run a fresh durable typed-effect lifecycle with a selected semantic
    /// meter. The checkpoint retains its existing reservation and cleanup
    /// accounting; this additive receipt describes only stages observed by
    /// this invocation. Resumption is refused until the checkpoint format
    /// binds prior semantic receipts.
    #[allow(clippy::too_many_arguments)]
    pub fn run_durable_metered_with_backend(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        selected: TargetStageBackend<'_>,
        semantic_fuel_limit: u64,
    ) -> Result<MeteredDurableTypedRun, DurableTypedFailure> {
        if let Some(checkpoint) = retained_checkpoint {
            return Err(durable::semantic_refusal(
                "semantic_work.recovery_unsupported",
                checkpoint,
            ));
        }
        if !cancellation.is_cancelled() && !(1..=1_000_000).contains(&semantic_fuel_limit) {
            return Err(durable::semantic_refusal("semantic_work.fuel_limit", ""));
        }
        let selected = self.durable_backend(selected, None)?;
        let observations = RefCell::new(Vec::new());
        let backend = StageBackend::Metered {
            backend: &selected,
            fuel_limit: semantic_fuel_limit,
            observations: &observations,
        };
        let run = self.run_durable_inner(
            task,
            proposals,
            handler,
            stages,
            effects,
            cancellation,
            execution_revision_digest,
            program_root_digest,
            None,
            store,
            max_reserved_fuel,
            None,
            Some(backend),
        )?;
        let observations = observations.into_inner();
        let observations_complete = observations.len() == run.run().lifecycle().stages().len();
        let mut document = serde_json::json!({
            "schema": "semaprax.agent-durable-semantic-work.v1",
            "checkpoint_digest": run.checkpoint_digest(),
            "semantic_fuel_limit": semantic_fuel_limit,
            "observations_complete": observations_complete,
            "observed_stage_count": observations.len(),
            "committed_stage_count": run.run().lifecycle().stages().len(),
            "stages": observations.iter().map(|observation| serde_json::json!({
                "function": observation.function_id(),
                "fuel_used": observation.work().fuel_used,
                "fuel_limit": observation.work().fuel_limit,
                "exhausted": observation.work().exhausted,
            })).collect::<Vec<_>>(),
        });
        document.sort_all_objects();
        let evidence = format!("{document}\n");
        let digest = digest(
            b"semaprax.agent-durable-semantic-work.v1\0",
            evidence.as_bytes(),
        );
        Ok(MeteredDurableTypedRun {
            run,
            observations,
            observations_complete,
            evidence,
            digest,
        })
    }

    /// Refuse a selected migration target before that migration reserves fuel
    /// or invokes a compiler/runtime. This is intentionally separate from
    /// execution so migration's existing reservation accounting stays exact.
    pub(crate) fn validate_target_retained_call_metered(
        &self,
        program: &crate::hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        selected: TargetStageBackend<'_>,
        semantic_fuel_limit: u64,
    ) -> Result<(), Vec<Diagnostic>> {
        let _ = self.selected_target_backend(selected)?;
        crate::agent_lifecycle::authorization::StageSemanticProfile::admit(
            program,
            prepared.function_id(),
            semantic_fuel_limit,
        )
        .map_err(|error| vec![error])?;
        Ok(())
    }

    /// Execute one already-admitted migration call through the same sealed
    /// target dispatcher as Agent stages. A selected target is always
    /// semantically metered; there is no unmetered or interpreter fallback.
    pub(crate) fn execute_target_retained_call_metered(
        &self,
        program: &crate::hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        selected: TargetStageBackend<'_>,
        semantic_fuel_limit: u64,
    ) -> Result<MeteredTargetRetainedCall, Vec<Diagnostic>> {
        let backend = self.selected_target_backend(selected)?;
        let target_binding = self.target_execution_binding(backend);
        let execution_binding = digest(
            b"semaprax.agent-migration-target-execution.v1\0",
            format!("{target_binding}\0{semantic_fuel_limit}").as_bytes(),
        );
        let evaluation = crate::agent_lifecycle::authorization::dispatch_on_metered(
            backend,
            program,
            prepared,
            arguments,
            max_steps,
            semantic_fuel_limit,
            None,
        )?;
        Ok(MeteredTargetRetainedCall {
            evaluation,
            execution_binding,
        })
    }

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
