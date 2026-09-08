//! Direct Runtime v2 consumption of the checked typed iterative product.
//!
//! This producer owns the exact invocation it executes. Neither an arbitrary
//! TypedEffectRun nor caller-authored evidence can be joined to these roots.
use super::*;
use crate::agent_lifecycle::iterative::IterativeBudget;

use crate::agent_lifecycle::iterative::effects::{
    compile_typed_effects, CompiledTypedEffects, EffectBudget, EffectOperation, TypedEffectHandler,
    TypedEffectRun,
};

pub const MAX_ITERATIVE_PROPOSAL_BYTES: usize = 2 * 1024 * 1024;

pub struct AgentRuntimeV2 {
    project: Arc<ProjectRevision>,
    program_root: String,
    lifecycle: CompiledTypedEffects,
    deployment: ExecutionRoot,
    instance: ExecutionRoot,
    revision: ExecutionRoot,
    task: LifecycleTask,
    proposals: Vec<String>,
    budget: IterativeBudget,
    effects: EffectBudget,
}
impl AgentRuntimeV2 {
    pub fn deployment_root(&self) -> &ExecutionRoot {
        &self.deployment
    }
    pub fn instance_root(&self) -> &ExecutionRoot {
        &self.instance
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
    pub fn project_revision(&self) -> &ProjectRevision {
        &self.project
    }
    pub fn run(
        self,
        read: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> Result<AgentRuntimeV2Evidence> {
        let run = self.lifecycle.run(
            &self.task,
            &self.proposals,
            read,
            self.budget,
            self.effects,
            cancellation,
        )?;
        let evidence = root(
            "semaprax.evidence-root.v3",
            json!({
                "execution_revision": self.revision.digest(),
                "instance_root": self.instance.digest(),
                "typed_effect_evidence": run.evidence_digest(),
            }),
        );
        Ok(AgentRuntimeV2Evidence {
            run,
            evidence,
            revision: self.revision,
        })
    }
}

pub struct AgentRuntimeV2Evidence {
    run: TypedEffectRun,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
}
impl AgentRuntimeV2Evidence {
    pub fn run(&self) -> &TypedEffectRun {
        &self.run
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
}

/// Compile the deployed Step reducer directly from the selected retained source.
/// The proposal inventory is positional, including unused suffix entries; limits
/// and exact submitted bytes are all committed before any execution occurs.
#[allow(clippy::too_many_arguments)]
pub fn bind_agent_runtime_v2(
    project: Arc<ProjectRevision>,
    program: ProgramRootRef<'_>,
    expected_program_digest: &str,
    source_path: &str,
    agent_id: &str,
    step_type_id: &str,
    selector_field_id: &str,
    operations: Vec<EffectOperation>,
    deployment_source: &str,
    task: LifecycleTask,
    proposals: &[String],
    budget: IterativeBudget,
    effects: EffectBudget,
) -> Result<AgentRuntimeV2> {
    let total = proposals
        .iter()
        .try_fold(0usize, |total, proposal| total.checked_add(proposal.len()));
    if task.objective.len() > 65_536
        || proposals.len() > 4096
        || proposals.iter().any(|proposal| proposal.len() > 262_144)
        || total.is_none_or(|total| total > MAX_ITERATIVE_PROPOSAL_BYTES)
        || budget.max_iterations > 4096
        || budget.max_stages > 12_289
    {
        return Err(refused("iterative invocation capacity"));
    }
    if program.digest() != expected_program_digest {
        return Err(refused("stale ProgramRoot"));
    }
    let retained_root = project.program_root()?;
    for kind in [
        "source_projection",
        "semantic_program",
        "stable_identity_index",
        "dependency_closure",
        "contracts_and_tests",
        "agent_definitions",
    ] {
        let segment = retained_root
            .segment(kind)
            .ok_or_else(|| refused("retained Project source segment missing"))?;
        if program
            .segments()
            .iter()
            .find(|candidate| candidate.kind() == kind)
            != Some(segment)
        {
            return Err(refused(
                "ProgramRoot source-owned segment differs from retained Project",
            ));
        }
    }
    let source = project
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .ok_or_else(|| refused("source path is not retained by Project"))?;
    let checked = crate::check(source.source(), source_path)?;
    let declaration = checked
        .agents
        .iter()
        .find(|agent| agent.stable_id == agent_id)
        .ok_or_else(|| refused("selected Agent is not in retained source"))?;
    let source_definition = crate::project::compile_source_agent_declaration(declaration)?;
    if !project.agent_definitions().iter().any(|definition| {
        definition.definition().canonical_source()
            == source_definition.definition().canonical_source()
    }) {
        return Err(refused("Agent definition differs from retained Project"));
    }
    let selected = crate::agent_deployment::compile_agent_deployment(deployment_source)?;
    let (semantic, _) = migrate_agent_definition_v1(
        source_definition.definition().canonical_source(),
        selected.deployment_id(),
    )?;
    let bound = bind_agent_deployment(&semantic, deployment_source)?;
    let deployed: serde_json::Value = serde_json::from_str(bound.canonical_json())
        .map_err(|_| refused("iterative deployment limits"))?;
    let limit = |key: &str| -> Result<usize> {
        deployed["effective"]["limits"][key]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| refused("iterative deployment limits"))
    };
    let deployed_turns = limit("max_turns")?;
    let deployed_calls = limit("max_tool_calls")?;
    let effective_budget = IterativeBudget {
        max_iterations: budget
            .max_iterations
            .min(deployed_turns)
            .min(deployed_calls),
        ..budget
    };
    let lifecycle = compile_typed_effects(
        source.source(),
        source_path,
        &bound,
        step_type_id,
        selector_field_id,
        operations,
    )?;
    let deployment = root(
        "semaprax.deployment-root.v3",
        json!({
            "program_root": program.digest(), "definition": bound.semantic_definition().digest(),
            "deployment": bound.deployment().digest(), "binding": bound.digest(),
            "source_path": source_path, "source_revision": source.source_revision(),
            "agent_id": agent_id, "step_type_id": step_type_id, "typed_registry": lifecycle.digest(),
        }),
    );
    let proposal_digests: Vec<_> = proposals
        .iter()
        .map(|source| input_digest(source.as_bytes()))
        .collect();
    let instance = root(
        "semaprax.instance-root.v3",
        json!({
            "deployment_root": deployment.digest(), "task_digest": input_digest(&task.objective),
            "task_budget": task.budget, "proposal_digests": proposal_digests,
            "max_iterations": budget.max_iterations, "max_stages": budget.max_stages,
            "max_steps_per_stage": budget.max_steps_per_stage,
            "deployed_max_turns": deployed_turns, "deployed_max_tool_calls": deployed_calls,
            "effective_max_iterations": effective_budget.max_iterations,
            "max_effect_calls": effects.max_calls, "max_argument_bytes": effects.max_argument_bytes,
            "max_result_bytes": effects.max_result_bytes, "max_total_bytes": effects.max_total_bytes,
        }),
    );
    let revision = root(
        "semaprax.execution-revision.v3",
        json!({
            "program_root": program.digest(), "deployment_root": deployment.digest(),
            "instance_root": instance.digest(), "project_revision": project.project_revision(),
        }),
    );
    Ok(AgentRuntimeV2 {
        program_root: program.digest().to_owned(),
        project,
        lifecycle,
        deployment,
        instance,
        revision,
        task,
        proposals: proposals.to_vec(),
        budget: effective_budget,
        effects,
    })
}

#[path = "typed_durable.rs"]
mod durable;
pub use durable::AgentRuntimeV2DurableEvidence;

#[path = "typed_migration.rs"]
pub(crate) mod migration;
pub use migration::{
    migrate_suspended_agent_runtime_v2, resume_migrated_agent_runtime_v2,
    AgentRuntimeV2MigrationEvidence, AgentRuntimeV2MigrationFailure, DurableMigrationFailure,
    MigratedAgentRuntimeV2, ResumedMigratedAgentRuntimeV2,
};
