//! Retained Project association for the additive iterative lifecycle.
//!
//! This producer owns the exact invocation it executes. Neither an arbitrary
//! IterativeRun nor caller-authored evidence can be joined to these roots.
use super::*;
use crate::agent_lifecycle::iterative::{
    compile_agent_lifecycle_v2, CompiledIterativeLifecycle, IterativeBudget, IterativeRun,
};

pub const MAX_ITERATIVE_PROPOSAL_BYTES: usize = 2 * 1024 * 1024;

pub struct IterativeExecutionRevision {
    project: Arc<ProjectRevision>,
    lifecycle: CompiledIterativeLifecycle,
    deployment: ExecutionRoot,
    instance: ExecutionRoot,
    revision: ExecutionRoot,
    task: LifecycleTask,
    proposals: Vec<String>,
    budget: IterativeBudget,
}
impl IterativeExecutionRevision {
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
        read: &mut dyn AgentReadOperation,
        cancellation: &AgentCancellation,
    ) -> Result<IterativeExecutionEvidence> {
        let run =
            self.lifecycle
                .run(&self.task, &self.proposals, read, self.budget, cancellation)?;
        let evidence = root(
            "semaprax.evidence-root.v2",
            json!({
                "execution_revision": self.revision.digest(),
                "instance_root": self.instance.digest(),
                "iterative_lifecycle_evidence": run.evidence_digest(),
            }),
        );
        Ok(IterativeExecutionEvidence {
            run,
            evidence,
            revision: self.revision,
        })
    }
}

pub struct IterativeExecutionEvidence {
    run: IterativeRun,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
}
impl IterativeExecutionEvidence {
    pub fn run(&self) -> &IterativeRun {
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
pub fn bind_iterative_execution_revision(
    project: Arc<ProjectRevision>,
    program: ProgramRootRef<'_>,
    expected_program_digest: &str,
    source_path: &str,
    agent_id: &str,
    step_type_id: &str,
    deployment_source: &str,
    task: LifecycleTask,
    proposals: &[String],
    budget: IterativeBudget,
) -> Result<IterativeExecutionRevision> {
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
    let lifecycle = compile_agent_lifecycle_v2(
        source.source(),
        source_path,
        bound.runtime_v1_definition(),
        step_type_id,
    )?;
    let deployment = root(
        "semaprax.deployment-root.v2",
        json!({
            "program_root": program.digest(), "definition": bound.semantic_definition().digest(),
            "deployment": bound.deployment().digest(), "binding": bound.digest(),
            "source_path": source_path, "source_revision": source.source_revision(),
            "agent_id": agent_id, "step_type_id": step_type_id, "iterative_lifecycle": lifecycle.digest(),
        }),
    );
    let proposal_digests: Vec<_> = proposals
        .iter()
        .map(|source| input_digest(source.as_bytes()))
        .collect();
    let instance = root(
        "semaprax.instance-root.v2",
        json!({
            "deployment_root": deployment.digest(), "task_digest": input_digest(&task.objective),
            "task_budget": task.budget, "proposal_digests": proposal_digests,
            "max_iterations": budget.max_iterations, "max_stages": budget.max_stages,
            "max_steps_per_stage": budget.max_steps_per_stage,
            "deployed_max_turns": deployed_turns, "deployed_max_tool_calls": deployed_calls,
            "effective_max_iterations": effective_budget.max_iterations,
        }),
    );
    let revision = root(
        "semaprax.execution-revision.v2",
        json!({
            "program_root": program.digest(), "deployment_root": deployment.digest(),
            "instance_root": instance.digest(), "project_revision": project.project_revision(),
        }),
    );
    Ok(IterativeExecutionRevision {
        project,
        lifecycle,
        deployment,
        instance,
        revision,
        task,
        proposals: proposals.to_vec(),
        budget: effective_budget,
    })
}
