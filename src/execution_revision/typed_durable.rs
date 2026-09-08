//! Store-backed direct Runtime v2 execution and evidence association.
use super::*;
use crate::agent_lifecycle::iterative::effects::{
    DurableTypedFailure, DurableTypedRun, TypedEffectHandler,
};
use crate::agent_lifecycle::CheckpointStore;

impl AgentRuntimeV2 {
    /// Execute using a caller-owned single-writer checkpoint store.
    ///
    /// `retained_checkpoint` must be an acknowledged snapshot from that trusted
    /// store. Its hashes detect drift; they do not authenticate host observations.
    /// The checked producer replays pure stages under fresh authorizations before
    /// using retained observations. An uncertain effect intent cannot redispatch.
    pub fn run_durable(
        self,
        handler: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
    ) -> std::result::Result<AgentRuntimeV2DurableEvidence, DurableTypedFailure> {
        let run = self.lifecycle.run_durable(
            &self.task,
            &self.proposals,
            handler,
            self.budget,
            self.effects,
            cancellation,
            self.revision.digest(),
            &self.program_root,
            retained_checkpoint,
            store,
            max_reserved_fuel,
        )?;
        let evidence = root(
            "semaprax.evidence-root.v4",
            json!({
                "execution_revision": self.revision.digest(),
                "instance_root": self.instance.digest(),
                "typed_effect_evidence": run.run().evidence_digest(),
                "checkpoint_digest": run.checkpoint_digest(),
                "max_reserved_fuel": max_reserved_fuel,
            }),
        );
        Ok(AgentRuntimeV2DurableEvidence {
            run,
            evidence,
            revision: self.revision,
        })
    }
}

/// Evidence from the invocation that performed or replayed the durable run.
pub struct AgentRuntimeV2DurableEvidence {
    run: DurableTypedRun,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
}
impl AgentRuntimeV2DurableEvidence {
    pub fn run(&self) -> &DurableTypedRun {
        &self.run
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
}
