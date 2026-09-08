//! Authority-free runtime associations derived from retained checked Project inputs.
pub mod iterative;
pub mod typed;

use crate::agent_deployment::{
    bind_agent_deployment, migrate_agent_definition_v1, BoundAgentDeployment,
};
use crate::agent_lifecycle::{
    compile_agent_lifecycle, AgentReadOperation, CompiledAgentLifecycle, LifecycleBudget,
    LifecycleRun, LifecycleTask,
};
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::project::{
    ProgramRoot, ProgramRootSegment, ProgramRootV2, ProgramRootV3, ProjectRevision,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// An already authenticated compiler root; no submitted JSON gains this type.
#[derive(Clone, Copy)]
pub enum ProgramRootRef<'a> {
    V1(&'a ProgramRoot),
    V2(&'a ProgramRootV2),
    V3(&'a ProgramRootV3),
}
impl<'a> ProgramRootRef<'a> {
    pub fn digest(self) -> &'a str {
        match self {
            Self::V1(v) => v.program_root_digest(),
            Self::V2(v) => v.program_root_v2_digest(),
            Self::V3(v) => v.program_root_v3_digest(),
        }
    }
    fn segments(self) -> &'a [ProgramRootSegment] {
        match self {
            Self::V1(v) => v.segments(),
            Self::V2(v) => v.segments(),
            Self::V3(v) => v.segments(),
        }
    }
}

/// A content-addressed association, with construction restricted to checked producers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionRoot {
    digest: String,
    json: String,
}
impl ExecutionRoot {
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn canonical_json(&self) -> &str {
        &self.json
    }
}
fn root(schema: &str, facts: serde_json::Value) -> ExecutionRoot {
    let mut value = json!({"schema": schema, "facts": facts});
    let bytes = format!(
        "{}\n",
        serde_json::to_string(&value).expect("closed root JSON")
    );
    let mut hash = Sha256::new();
    hash.update(schema.as_bytes());
    hash.update([0]);
    hash.update(bytes.as_bytes());
    let digest = format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()));
    value["digest"] = json!(digest);
    ExecutionRoot {
        digest,
        json: format!(
            "{}\n",
            serde_json::to_string(&value).expect("closed root JSON")
        ),
    }
}
fn input_digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
fn refused(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G583",
        format!("ExecutionRevision association rejected: {detail}"),
    )]
}

/// Retains the exact Project, deployed lifecycle, and invocation inputs.
/// Consumed by execution, so an evidence root can only name its actual producer.
pub struct ExecutionRevision {
    project: Arc<ProjectRevision>,
    lifecycle: CompiledAgentLifecycle,
    deployment: ExecutionRoot,
    instance: ExecutionRoot,
    revision: ExecutionRoot,
    task: LifecycleTask,
    proposal: String,
    budget: LifecycleBudget,
}
impl ExecutionRevision {
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
    ) -> Result<ExecutionEvidence> {
        let run =
            self.lifecycle
                .run(&self.task, &self.proposal, read, self.budget, cancellation)?;
        let evidence = root(
            "semaprax.evidence-root.v1",
            json!({"execution_revision": self.revision.digest(), "instance_root": self.instance.digest(), "lifecycle_evidence": run.evidence_digest()}),
        );
        Ok(ExecutionEvidence {
            run,
            evidence,
            revision: self.revision,
        })
    }
}
pub struct ExecutionEvidence {
    run: LifecycleRun,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
}
impl ExecutionEvidence {
    pub fn run(&self) -> &LifecycleRun {
        &self.run
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
}

/// Bind an exact retained source Agent and explicitly supplied deployment.
/// Root documents remain evidence and grant no host authority.
#[allow(clippy::too_many_arguments)]
pub fn bind_execution_revision(
    project: Arc<ProjectRevision>,
    program: ProgramRootRef<'_>,
    expected_program_digest: &str,
    source_path: &str,
    agent_id: &str,
    deployment_source: &str,
    task: LifecycleTask,
    proposal: &str,
    budget: LifecycleBudget,
) -> Result<ExecutionRevision> {
    if program.digest() != expected_program_digest {
        return Err(refused("stale ProgramRoot"));
    }
    let retained_root = project.program_root()?;
    // Runtime policy/target/projection nodes may be additive workspace inputs.
    // Every source-owned node must be identical to the independently derived Project.
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
            .find(|candidate| candidate.kind() == segment.kind())
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
    let bound: BoundAgentDeployment = bind_agent_deployment(&semantic, deployment_source)?;
    let lifecycle =
        compile_agent_lifecycle(source.source(), source_path, bound.runtime_v1_definition())?;
    let deployment = root(
        "semaprax.deployment-root.v1",
        json!({"program_root": program.digest(), "definition": bound.semantic_definition().digest(), "deployment": bound.deployment().digest(), "binding": bound.digest(), "source_path": source_path, "source_revision": source.source_revision(), "agent_id": agent_id, "lifecycle": lifecycle.digest()}),
    );
    if task.objective.len() > 65_536 || proposal.len() > 262_144 {
        return Err(refused("invocation byte limit"));
    }
    let instance = root(
        "semaprax.instance-root.v1",
        json!({"deployment_root": deployment.digest(), "task_digest": input_digest(&task.objective), "task_budget": task.budget, "proposal_digest": input_digest(proposal.as_bytes()), "max_steps_per_stage": budget.max_steps_per_stage}),
    );
    let revision = root(
        "semaprax.execution-revision.v1",
        json!({"program_root": program.digest(), "deployment_root": deployment.digest(), "instance_root": instance.digest(), "project_revision": project.project_revision()}),
    );
    Ok(ExecutionRevision {
        project,
        lifecycle,
        deployment,
        instance,
        revision,
        task,
        proposal: proposal.to_owned(),
        budget,
    })
}
