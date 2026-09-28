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
use crate::agent_runtime_v2::source_model::SourceModelContract;
use crate::agent_runtime_v2::{
    SourceModelAdapterIdentity, SourceModelBinding, SourceModelEvidence, SourceModelPolicyBinding,
};
use crate::model_budget_policy::ModelBudgetLimits;
use crate::provider_adapter_sdk::StreamingSourceProposalAdapter;

pub const MAX_ITERATIVE_PROPOSAL_BYTES: usize = 2 * 1024 * 1024;

#[path = "typed/model_wait.rs"]
mod model_wait;
#[path = "typed/owned_wait_context.rs"]
mod owned_wait_context;
pub use model_wait::SourceModelWaitBinding;
pub(crate) use owned_wait_context::CheckedTypedOwnedWaitExecutionV8;
#[cfg(test)]
pub(crate) use owned_wait_context::TestProspectiveReduceLimitV8;

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
    source_model: SourceModelContract,
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

    /// Consume the live source-proposal route. This is only admitted for a
    /// runtime bound without a frozen proposal inventory, so no caller can
    /// silently replace committed proposal bytes with provider output.
    pub fn run_live(
        self,
        source: &mut dyn crate::agent_lifecycle::iterative::driver::ProposalSource,
        read: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> Result<AgentRuntimeV2Evidence> {
        if !self.proposals.is_empty() {
            return Err(refused(
                "live source runtime cannot replace a bound proposal inventory",
            ));
        }
        let run = self.lifecycle.run_live(
            &self.task,
            source,
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

    pub fn proposal_schema(&self) -> &crate::agent_proposal::CompiledAgentProposalSchema {
        self.lifecycle.proposal_schema()
    }

    /// Derives facts for one explicit streaming adapter selection. The caller
    /// supplies this binding and its derived token to the adapter constructor;
    /// neither value creates a provider nor a transport capability.
    pub fn source_model_binding(
        &self,
        adapter: SourceModelAdapterIdentity,
    ) -> Result<SourceModelBinding> {
        self.source_model
            .bind(self.deployment.digest(), self.instance.digest(), adapter)
            .map_err(refused)
    }

    /// Narrows the retained source/deployment policy for one invocation. The
    /// supplied invocation ceiling can never widen either retained ceiling.
    pub fn source_model_policy_binding(
        &self,
        binding: &SourceModelBinding,
        invocation_policy: ModelBudgetLimits,
    ) -> Result<SourceModelPolicyBinding> {
        if !binding.runtime_matches(
            self.deployment.digest(),
            self.instance.digest(),
            self.lifecycle.proposal_schema().source_revision(),
            self.lifecycle.proposal_schema().schema().digest(),
        ) {
            return Err(refused("source.model_binding"));
        }
        binding.policy_binding(invocation_policy).map_err(refused)
    }

    /// Consumes the bound streaming source-model route. It is additive to the
    /// ordinary `run_live` API, which remains the compatibility seam for an
    /// arbitrary caller-owned proposal source.
    pub fn run_live_bound_model(
        self,
        source: &mut StreamingSourceProposalAdapter<'_>,
        read: &mut dyn TypedEffectHandler,
        cancellation: &AgentCancellation,
    ) -> std::result::Result<AgentRuntimeV2ModelEvidence, AgentRuntimeV2ModelFailure> {
        // The source retains its binding internally. The runtime derives the
        // same source/schema/root facts before it permits the first factory
        // call; adapter identity comparison itself happens after construction
        // and before `ProviderAdapter::start`.
        if !self.proposals.is_empty() {
            return Err(AgentRuntimeV2ModelFailure::new(
                refused("live source runtime cannot replace a bound proposal inventory"),
                SourceModelEvidence::default(),
                &self,
                None,
                None,
                "preflight.proposal_inventory",
            ));
        }
        if !source.model_evidence().attempts().is_empty() {
            return Err(AgentRuntimeV2ModelFailure::new(
                refused("source.model_evidence_reused"),
                SourceModelEvidence::default(),
                &self,
                None,
                None,
                "preflight.evidence_reused",
            ));
        }
        if source.model_binding().is_none_or(|binding| {
            !binding.runtime_matches(
                self.deployment.digest(),
                self.instance.digest(),
                self.lifecycle.proposal_schema().source_revision(),
                self.lifecycle.proposal_schema().schema().digest(),
            )
        }) {
            return Err(AgentRuntimeV2ModelFailure::new(
                refused("source.model_binding"),
                SourceModelEvidence::default(),
                &self,
                None,
                None,
                "preflight.binding",
            ));
        }
        let binding_digest = source
            .model_binding()
            .expect("checked source model binding")
            .digest()
            .to_owned();
        let policy_digest = source
            .model_policy_binding()
            .map(|policy| policy.digest().to_owned());
        let run = self.lifecycle.run_live(
            &self.task,
            source,
            read,
            self.budget,
            self.effects,
            cancellation,
        );
        match run {
            Ok(run) => {
                let model_evidence = source.model_evidence().clone();
                let evidence = model_evidence_root(
                    &self,
                    run.evidence_digest(),
                    &model_evidence,
                    Some(&binding_digest),
                    policy_digest.as_deref(),
                    "completed",
                );
                Ok(AgentRuntimeV2ModelEvidence {
                    run,
                    model_evidence,
                    evidence,
                    revision: self.revision,
                })
            }
            Err(diagnostics) => Err(AgentRuntimeV2ModelFailure::new(
                diagnostics,
                source.model_evidence().clone(),
                &self,
                Some(&binding_digest),
                policy_digest.as_deref(),
                "lifecycle_failed",
            )),
        }
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

/// Evidence for one successfully completed bound streaming model route.
pub struct AgentRuntimeV2ModelEvidence {
    run: TypedEffectRun,
    model_evidence: SourceModelEvidence,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
}
impl AgentRuntimeV2ModelEvidence {
    pub fn run(&self) -> &TypedEffectRun {
        &self.run
    }
    pub fn model_evidence(&self) -> &SourceModelEvidence {
        &self.model_evidence
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
}

/// A bound streaming source failure retains redacted model-attempt evidence.
/// The source adapter never returns raw prompt or response bytes through it.
pub struct AgentRuntimeV2ModelFailure {
    diagnostics: Vec<Diagnostic>,
    model_evidence: SourceModelEvidence,
    evidence: ExecutionRoot,
    revision: ExecutionRoot,
}
impl AgentRuntimeV2ModelFailure {
    fn new(
        diagnostics: Vec<Diagnostic>,
        model_evidence: SourceModelEvidence,
        runtime: &AgentRuntimeV2,
        binding_digest: Option<&str>,
        policy_digest: Option<&str>,
        status: &'static str,
    ) -> Self {
        let evidence = model_evidence_root(
            runtime,
            "",
            &model_evidence,
            binding_digest,
            policy_digest,
            status,
        );
        Self {
            diagnostics,
            model_evidence,
            evidence,
            revision: runtime.revision.clone(),
        }
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn model_evidence(&self) -> &SourceModelEvidence {
        &self.model_evidence
    }
    pub fn evidence_root(&self) -> &ExecutionRoot {
        &self.evidence
    }
    pub fn execution_revision(&self) -> &ExecutionRoot {
        &self.revision
    }
}

fn model_evidence_root(
    runtime: &AgentRuntimeV2,
    typed_effect_evidence: &str,
    model_evidence: &SourceModelEvidence,
    binding_digest: Option<&str>,
    policy_digest: Option<&str>,
    status: &str,
) -> ExecutionRoot {
    let facts = if let Some(policy_digest) = policy_digest {
        json!({
            "execution_revision": runtime.revision.digest(),
            "instance_root": runtime.instance.digest(),
            "typed_effect_evidence": if typed_effect_evidence.is_empty() { None } else { Some(typed_effect_evidence) },
            "source_model_binding": binding_digest,
            "source_model_policy": policy_digest,
            "source_model_evidence": model_evidence.digest(),
            "source_model_status": status,
        })
    } else {
        // Preserve the established v4 bytes for the unpriced constructor.
        json!({
            "execution_revision": runtime.revision.digest(),
            "instance_root": runtime.instance.digest(),
            "typed_effect_evidence": if typed_effect_evidence.is_empty() { None } else { Some(typed_effect_evidence) },
            "source_model_binding": binding_digest,
            "source_model_evidence": model_evidence.digest(),
            "source_model_status": status,
        })
    };
    root("semaprax.evidence-root.v4", facts)
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
    bind_runtime(
        project,
        program,
        expected_program_digest,
        source_path,
        agent_id,
        step_type_id,
        selector_field_id,
        operations,
        deployment_source,
        task,
        proposals,
        budget,
        effects,
        false,
    )
}

/// Bind Direct Runtime v2 for a source selected at run time. Its empty frozen
/// proposal inventory is intentional and enforced by [`AgentRuntimeV2::run_live`].
#[allow(clippy::too_many_arguments)]
pub fn bind_agent_runtime_v2_live(
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
    budget: IterativeBudget,
    effects: EffectBudget,
) -> Result<AgentRuntimeV2> {
    bind_runtime(
        project,
        program,
        expected_program_digest,
        source_path,
        agent_id,
        step_type_id,
        selector_field_id,
        operations,
        deployment_source,
        task,
        &[],
        budget,
        effects,
        false,
    )
}

/// Bind deterministic roles from the exact retained Project dependency closure.
#[allow(clippy::too_many_arguments)]
pub fn bind_linked_agent_runtime_v2(
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
    bind_runtime(
        project,
        program,
        expected_program_digest,
        source_path,
        agent_id,
        step_type_id,
        selector_field_id,
        operations,
        deployment_source,
        task,
        proposals,
        budget,
        effects,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn bind_runtime(
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
    linked: bool,
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
    let checked = if linked {
        crate::parse(source.source(), std::path::Path::new(source_path))
            .map_err(|error| vec![error])?
    } else {
        crate::check(source.source(), source_path)?
    };
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
    let deployed_model_request_bytes = limit("max_provider_request_bytes")?;
    let deployed_model_response_bytes = limit("max_provider_response_bytes")?;
    let effective_budget = IterativeBudget {
        max_iterations: budget
            .max_iterations
            .min(deployed_turns)
            .min(deployed_calls),
        ..budget
    };
    let lifecycle = if linked {
        let linked_program = project.linked_agent_program(
            source_path,
            agent_id,
            source_definition.definition().canonical_source(),
        )?;
        crate::agent_lifecycle::iterative::effects::compile_linked_typed_effects(
            linked_program,
            &bound,
            step_type_id,
            selector_field_id,
            operations,
        )?
    } else {
        compile_typed_effects(
            source.source(),
            source_path,
            &bound,
            step_type_id,
            selector_field_id,
            operations,
        )?
    };
    let deployment = root(
        "semaprax.deployment-root.v3",
        json!({
            "program_root": program.digest(), "definition": bound.semantic_definition().digest(),
            "deployment": bound.deployment().digest(), "binding": bound.digest(),
            "source_path": source_path, "source_revision": source.source_revision(),
            "agent_id": agent_id, "step_type_id": step_type_id, "typed_registry": lifecycle.digest(),
        }),
    );
    let source_model = SourceModelContract::new(
        bound.deployment().digest(),
        bound.digest(),
        lifecycle.proposal_schema().source_revision(),
        lifecycle.proposal_schema().schema().digest(),
        bound.granted_capabilities(),
        bound.required_model_capabilities(),
        bound.model_selections(),
        deployed_model_request_bytes,
        deployed_model_response_bytes,
        &semantic,
        deployment_source,
    )
    .map_err(refused)?;
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
        source_model,
    })
}

#[path = "typed_durable.rs"]
mod durable;
pub use durable::{
    AgentRuntimeV2DurableEvidence, AgentRuntimeV2DurableModelEvidence,
    AgentRuntimeV2DurableModelFailure, AgentRuntimeV2DurableModelWaitEvidence,
    PreparedAgentRuntimeV2SourceMigration,
};

#[path = "typed_migration.rs"]
pub(crate) mod migration;
pub use migration::{
    migrate_suspended_agent_runtime_v2, resume_migrated_agent_runtime_v2,
    AgentRuntimeV2MigrationEvidence, AgentRuntimeV2MigrationFailure, DurableMigrationFailure,
    MigratedAgentRuntimeV2, ResumedMigratedAgentRuntimeV2,
};

#[cfg(test)]
pub(crate) use owned_wait_context::TestContinuedAuthorizeV8;
