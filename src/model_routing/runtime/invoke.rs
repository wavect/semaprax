//! Binding a routed choice to real execution roots and running it through the
//! existing durable policy kernel, with the route record attached.

use std::sync::Arc;

use super::envelope::{Envelope, EnvelopeStore};
use super::error::RuntimeRoutingError;
use super::features::RuntimeFeatures;
use super::profiles::ApprovedProfileSet;
use super::record::RouteRecord;
use super::select::route_new_invocation;
use crate::agent_interaction_schema::CompiledInteractionSchema;
use crate::agent_lifecycle::{CheckpointStore, LifecycleBudget, LifecycleTask};
use crate::agent_runtime::AgentCancellation;
use crate::execution_revision::{bind_execution_revision, ExecutionRevision, ProgramRootRef};
use crate::live_invocation::{
    run_durable_policy_invocation, DurablePolicyRun, InvocationClock, LiveInvocationSeed,
    ModelInvocationRequest,
};
use crate::model_budget_policy::{
    AdapterAttemptPlan, DurablePolicyBinding, FailureClassifier, ProviderAdapterFactory,
    RetryBackoff,
};
use crate::model_routing::engine::{ConfiguredProvider, DecisionInvoker, RouteContext};
use crate::project::ProjectRevision;
use crate::provider_adapter_sdk::AdapterInvocationCapability;

/// Everything an invocation needs besides the route: the retained project
/// and agent, the task, the compiled proposal schema and the attempt shape.
#[derive(Clone)]
pub struct InvocationTarget<'p> {
    pub project: Arc<ProjectRevision>,
    pub program: ProgramRootRef<'p>,
    pub expected_program_digest: &'p str,
    pub source_path: &'p str,
    pub agent_id: &'p str,
    pub task: LifecycleTask,
    /// The lifecycle proposal bound into the instance root.
    pub proposal: &'p str,
    pub lifecycle_budget: LifecycleBudget,
    pub schema: &'p CompiledInteractionSchema,
    pub started_at_millis: i64,
    pub turn: u32,
    pub observation: Vec<u8>,
    pub max_response_bytes: usize,
    /// Per-call budget units; at most `task.budget`.
    pub effective_budget: i64,
    pub plan_context_tokens: u64,
    pub plan_output_tokens: u64,
    pub plan_cost_micros: i64,
    pub plan_max_polls: usize,
}

/// A routed choice bound to the roots of its selected deployment. Built
/// before any adapter; holds no adapter, credential or transport.
pub struct BoundRoutedInvocation {
    record: RouteRecord,
    profile: String,
    execution: ExecutionRevision,
    binding: DurablePolicyBinding,
    request: ModelInvocationRequest,
    plan: AdapterAttemptPlan,
}

impl BoundRoutedInvocation {
    pub fn record(&self) -> &RouteRecord {
        &self.record
    }
    pub fn profile_id(&self) -> &str {
        &self.profile
    }
    pub fn execution(&self) -> &ExecutionRevision {
        &self.execution
    }
    pub fn binding(&self) -> &DurablePolicyBinding {
        &self.binding
    }
    pub fn request(&self) -> &ModelInvocationRequest {
        &self.request
    }

    fn head(&self) -> Envelope {
        Envelope {
            record: self.record.clone(),
            binding: self.binding.digest().to_owned(),
            invocation: self.binding.invocation().digest().to_owned(),
            deployment_root: self.binding.deployment_root().to_owned(),
            instance_root: self.binding.instance_root().to_owned(),
            policy: None,
        }
    }
}

/// The explicit host authority one run consumes.
pub struct RoutedRunHandlers<'h> {
    pub clock: &'h dyn InvocationClock,
    pub cancellation: &'h AgentCancellation,
    pub capability: AdapterInvocationCapability,
    pub factory: &'h mut dyn ProviderAdapterFactory,
    pub classifier: &'h mut dyn FailureClassifier,
    pub backoff: &'h mut dyn RetryBackoff,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutedRun {
    pub record: RouteRecord,
    pub run: DurablePolicyRun,
    pub deployment_root: String,
    pub instance_root: String,
    pub invocation: String,
}

/// Derives execution, instance and invocation roots from the recorded
/// deployment and binds the durable policy (exact provider order, retained
/// limits) before any adapter exists. A recorded deployment the current set
/// no longer approves refuses; it is never rebound to another profile.
pub fn bind_routed_invocation(
    set: &ApprovedProfileSet,
    record: RouteRecord,
    target: &InvocationTarget<'_>,
) -> Result<BoundRoutedInvocation, RuntimeRoutingError> {
    let profile = set.by_deployment(record.deployment()).ok_or_else(|| {
        RuntimeRoutingError::ProfileNotApproved {
            deployment: record.deployment().to_owned(),
        }
    })?;
    if record.definition() != set.definition_digest() {
        return Err(RuntimeRoutingError::RecordMismatch(
            "route record names another semantic definition".into(),
        ));
    }
    let execution = bind_execution_revision(
        target.project.clone(),
        target.program,
        target.expected_program_digest,
        target.source_path,
        target.agent_id,
        profile.deployment_source(),
        target.task.clone(),
        target.proposal,
        target.lifecycle_budget,
    )
    .map_err(|d| RuntimeRoutingError::execution(&d))?;
    let seed = LiveInvocationSeed {
        program_root: target.program.digest().to_owned(),
        deployment_policy: profile.deployment_digest().to_owned(),
        task: target.task.objective.clone(),
        budget: target.task.budget,
        interaction_schema_digest: target.schema.schema().digest().to_owned(),
        approved_providers: profile
            .deployment()
            .model_selections()
            .iter()
            .map(|row| row.provider_id().to_owned())
            .collect(),
    };
    let binding = DurablePolicyBinding::bind(
        &execution,
        profile.deployment(),
        target.schema,
        &seed,
        profile.provider_policy().clone(),
        profile.limits(),
        target.started_at_millis,
    )
    .map_err(|refusal| RuntimeRoutingError::PolicyBinding {
        profile: profile.id().to_owned(),
        refusal,
    })?;
    let request = ModelInvocationRequest {
        turn: target.turn,
        task: seed.task.clone(),
        observation: target.observation.clone(),
        proposal_grammar_digest: target.schema.schema().digest().to_owned(),
        deployment_binding: profile.deployment_digest().to_owned(),
        max_response_bytes: target.max_response_bytes,
        effective_budget: target.effective_budget,
    };
    let plan = AdapterAttemptPlan::for_compiled(
        target.schema,
        &request,
        target.plan_context_tokens,
        target.plan_output_tokens,
        target.plan_cost_micros,
        target.plan_max_polls,
    )
    .map_err(|refusal| RuntimeRoutingError::Execution {
        code: "attempt_plan".into(),
        message: format!("{refusal:?}"),
    })?;
    Ok(BoundRoutedInvocation {
        record,
        profile: profile.id().to_owned(),
        execution,
        binding,
        request,
        plan,
    })
}

fn kernel(
    bound: &BoundRoutedInvocation,
    schema: &CompiledInteractionSchema,
    handlers: RoutedRunHandlers<'_>,
    store: &mut EnvelopeStore<'_>,
    recovered: Option<(&str, u64)>,
) -> RoutedRun {
    let run = run_durable_policy_invocation(
        &bound.binding,
        schema,
        &bound.request,
        &bound.plan,
        handlers.clock,
        handlers.cancellation,
        handlers.capability,
        handlers.factory,
        handlers.classifier,
        handlers.backoff,
        store,
        recovered,
    );
    RoutedRun {
        record: bound.record.clone(),
        run,
        deployment_root: bound.binding.deployment_root().to_owned(),
        instance_root: bound.binding.instance_root().to_owned(),
        invocation: bound.binding.invocation().digest().to_owned(),
    }
}

/// Commits the route (generation 0) before any adapter exists, then runs the
/// unchanged durable policy kernel with every journal commit enveloped.
pub fn run_routed_invocation(
    bound: &BoundRoutedInvocation,
    schema: &CompiledInteractionSchema,
    handlers: RoutedRunHandlers<'_>,
    store: &mut dyn CheckpointStore,
) -> Result<RoutedRun, RuntimeRoutingError> {
    let head = bound.head();
    store
        .commit(0, &head.render())
        .map_err(|_| RuntimeRoutingError::Checkpoint)?;
    let mut store = EnvelopeStore { inner: store, head };
    Ok(kernel(bound, schema, handlers, &mut store, None))
}

/// One new task: route, bind the selected deployment's roots, record, run.
/// Every refusal before `run_routed_invocation` happens before generation.
pub fn start_routed_task<I: ?Sized + DecisionInvoker>(
    set: &ApprovedProfileSet,
    features: &RuntimeFeatures,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_, I>>,
    target: &InvocationTarget<'_>,
    handlers: RoutedRunHandlers<'_>,
    store: &mut dyn CheckpointStore,
) -> Result<RoutedRun, RuntimeRoutingError> {
    let routed = route_new_invocation(set, features, ctx, provider)?;
    let bound = bind_routed_invocation(set, routed.into_record(), target)?;
    run_routed_invocation(&bound, target.schema, handlers, store)
}

/// Resumes from the latest retained envelope. The recorded route is reused
/// with zero router calls (no decision provider can even be supplied); the
/// recorded binding must rebind byte for byte, so catalog metadata or alias
/// drift cannot move an in-flight invocation to another deployment.
pub fn resume_routed_invocation(
    set: &ApprovedProfileSet,
    envelope: &str,
    generation: u64,
    target: &InvocationTarget<'_>,
    handlers: RoutedRunHandlers<'_>,
    store: &mut dyn CheckpointStore,
) -> Result<RoutedRun, RuntimeRoutingError> {
    let retained = Envelope::parse(envelope)?;
    let bound = bind_routed_invocation(set, retained.record.clone(), target)?;
    let mut head = bound.head();
    if head.binding != retained.binding
        || head.invocation != retained.invocation
        || head.deployment_root != retained.deployment_root
        || head.instance_root != retained.instance_root
    {
        return Err(RuntimeRoutingError::RecordMismatch(
            "retained binding does not rebind exactly".into(),
        ));
    }
    head.policy = None;
    let mut store = EnvelopeStore { inner: store, head };
    let recovered = retained.policy.as_deref().map(|doc| (doc, generation));
    Ok(kernel(
        &bound,
        target.schema,
        handlers,
        &mut store,
        recovered,
    ))
}
