//! MR-09: runtime model routing over host-approved deployment profiles,
//! exercised through the real bind/execution-root/durable-policy path.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::agent_lifecycle_v1::proposal;
use super::execution_revision::{probe_capabilities, Fixture};
use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_interaction_schema::{
    compile_agent_interaction_schema, CompiledInteractionSchema,
};
use semaprax::agent_lifecycle::{
    compile_source_agent_lifecycle, CheckpointStore, CheckpointStoreError, LifecycleBudget,
    LifecycleTask,
};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::execution_revision::{bind_execution_revision, ProgramRootRef};
use semaprax::live_invocation::fixture::StepClock;
use semaprax::live_invocation::DurablePolicyRun;
use semaprax::model_budget_policy::{
    intersect, DurablePolicyBindingRefusal, EffectiveModelBudget, ModelBudgetLimits,
    ProviderPolicy, ProviderSlot,
};
use semaprax::model_routing::engine::{
    Confidentiality, ConfiguredProvider, DecisionCall, DecisionInvoker, DecisionRequest,
    Destination, EnablementGate, LatencyClass, Modality, ProjectBinding, ProviderMode,
    ProviderProfile, RouteContext, RoutePolicy, TaskFamily,
};
use semaprax::model_routing::runtime::{
    bind_routed_invocation, resume_routed_invocation, route_new_invocation, start_routed_task,
    ApprovedProfileSet, InvocationTarget, ProfileModel, ProfileSpec, RouteSource,
    RoutedRunHandlers, RuntimeFeatures, RuntimeRoutingError, ENVELOPE_SCHEMA,
};
use semaprax::project::{with_authenticated_project, ProgramRoot, ProjectRevision};
use semaprax::provider_adapter_sdk::fixture_adapters::usage;
use semaprax::provider_adapter_sdk::{
    AdapterCapabilities, AdapterEvent, AdapterInvocationCapability, AdapterModelIdentity,
    AdapterPoll, AdapterRefusal, AdapterRequest, AdapterSettlement, ProviderAdapter,
};

/// The second approved profile: same semantic definition, another concrete
/// provider/model and deployment identity.
pub(super) fn strong_deployment(fast: &str) -> String {
    fast.replace(
        "\"deployment_id\":\"fixture.deployment\"",
        "\"deployment_id\":\"fixture.deployment.strong\"",
    )
    .replace("fake.local", "other.local")
    .replace("fake-basic", "other-basic")
    .replace(
        "\"quality_tier\":\"basic\"",
        "\"quality_tier\":\"standard\"",
    )
}

pub(super) fn limits() -> EffectiveModelBudget {
    intersect(
        ModelBudgetLimits {
            max_cost_micros: 0,
            max_latency_millis: 1_000,
            ..ModelBudgetLimits::single_call_only(1, 1)
        },
        ModelBudgetLimits::unbounded(),
        ModelBudgetLimits::unbounded(),
    )
    .unwrap()
}

pub(super) fn spec(id: &str, deployment: &str, cost: u64, rank: u32) -> ProfileSpec {
    ProfileSpec {
        id: id.to_owned(),
        deployment_source: deployment.to_owned(),
        provider_policy: None,
        limits: limits(),
        model: ProfileModel {
            alias: format!("{id}-model"),
            destination: Destination::Local,
            structured_output: true,
            tools: true,
            modalities: [Modality::Text].into(),
            max_context: 4096,
            est_cost_micros: cost,
            est_latency_ms: 100,
            strength_rank: rank,
        },
    }
}

pub(super) fn features(family: TaskFamily) -> RuntimeFeatures {
    RuntimeFeatures {
        task_family: family,
        estimated_context_tokens: 1,
        requires_structured_output: true,
        requires_tools: false,
        required_modalities: BTreeSet::new(),
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
        remaining_cost_micros: 1_000,
        remaining_latency_ms: 10_000,
        max_router_calls: 1,
        operator_pin: None,
    }
}

pub(super) fn ctx(invocation: &str) -> RouteContext {
    RouteContext {
        project: ProjectBinding {
            id: "fixture".into(),
            worktree: "wt".into(),
            revision: "rev".into(),
        },
        lock_digest: "lock".into(),
        invocation_id: invocation.into(),
        lineage_id: format!("{invocation}/lineage"),
        router_lineage: Vec::new(),
        router_calls_used: 0,
        router_ms_used: 0,
    }
}

/// A scripted decision adapter: counts calls and answers from a script. It
/// has no transport; it stands in for an experimental learned router.
pub(super) struct ScriptedRouter {
    pub(super) calls: u32,
    pub(super) answer: Option<serde_json::Value>,
}

impl DecisionInvoker for ScriptedRouter {
    fn evaluate(&mut self, request: &DecisionRequest) -> DecisionCall {
        self.calls += 1;
        assert!(request.validate().is_ok());
        match &self.answer {
            Some(result) => DecisionCall::Answered {
                result: result.clone(),
                elapsed_ms: 2,
                call: None,
            },
            None => DecisionCall::Unavailable,
        }
    }
}

pub(super) fn router(
    inv: &mut ScriptedRouter,
    mode: ProviderMode,
) -> ConfiguredProvider<'_, ScriptedRouter> {
    ConfiguredProvider {
        profile: ProviderProfile {
            provider_id: "fixture-router".into(),
            model_id: "router".into(),
            checkpoint: "c1".into(),
            ..ProviderProfile::default()
        },
        invoker: inv,
        mode,
        gate: EnablementGate::not_evaluated("model-route/v1", "fixture-router"),
    }
}

pub(super) fn choose(id: &str) -> serde_json::Value {
    serde_json::json!({"choice": id, "scores": {id: 0.9}, "abstain": false})
}

pub(super) fn response(schema: &CompiledInteractionSchema, sequence: &str) -> Vec<u8> {
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"fixture.agent.type.proposal\",\"schema_digest\":\"{}\",\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.budget\":\"5\",\"fixture.agent.type.proposal.urgent\":false,\"fixture.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n",
        schema.schema().digest()
    )
    .into_bytes()
}

/// Settles with one scripted response under the exact selected identity.
struct SettlingAdapter {
    capabilities: AdapterCapabilities,
    model: AdapterModelIdentity,
    script: Vec<AdapterPoll>,
}

impl ProviderAdapter for SettlingAdapter {
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }
    fn model_identity(&self) -> Option<&AdapterModelIdentity> {
        Some(&self.model)
    }
    fn start(
        &mut self,
        _: &AdapterInvocationCapability,
        _: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        Ok(())
    }
    fn poll(&mut self) -> AdapterPoll {
        self.script.remove(0)
    }
    fn cancel(&mut self, _: &str) {}
}

/// Records every provider the factory constructs an adapter for.
pub(super) type Created = Rc<RefCell<Vec<String>>>;

pub(super) fn settling_factory(
    set: &ApprovedProfileSet,
    reply: Vec<u8>,
    created: Created,
) -> impl FnMut(
    &str,
)
    -> Result<Box<dyn ProviderAdapter>, semaprax::model_budget_policy::AdapterFactoryRefusal> {
    let identities: Vec<_> = set
        .profiles()
        .iter()
        .flat_map(|p| p.deployment().model_selections())
        .collect();
    move |provider: &str| {
        created.borrow_mut().push(provider.to_owned());
        let selected = identities
            .iter()
            .find(|row| row.provider_id() == provider)
            .expect("factory only receives approved providers");
        Ok(Box::new(SettlingAdapter {
            capabilities: probe_capabilities(provider, selected.max_context_tokens()),
            model: AdapterModelIdentity {
                provider_id: selected.provider_id().to_owned(),
                model_id: selected.model_id().to_owned(),
                capabilities: selected.capabilities().to_vec(),
            },
            script: vec![
                AdapterPoll::Event(AdapterEvent::Delta(reply.clone())),
                AdapterPoll::Event(AdapterEvent::Completed),
                AdapterPoll::Settled(AdapterSettlement {
                    response_bytes: reply.clone(),
                    usage: usage(1, 1, 0),
                }),
            ],
        }) as Box<dyn ProviderAdapter>)
    }
}

#[derive(Default)]
pub(super) struct MemStore {
    pub(super) commits: Vec<(u64, String)>,
}

impl CheckpointStore for MemStore {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.commits.push((generation, document.to_owned()));
        Ok(())
    }
}

impl MemStore {
    pub(super) fn latest(&self) -> (String, u64) {
        let (g, d) = self.commits.last().unwrap();
        (d.clone(), *g)
    }
}

/// The retained project world shared by the routing tests.
pub(super) struct World {
    pub(super) project: std::sync::Arc<ProjectRevision>,
    pub(super) root: ProgramRoot,
    pub(super) semantic: String,
    pub(super) fast: String,
    pub(super) strong: String,
    pub(super) schema: CompiledInteractionSchema,
    pub(super) proposal: String,
}

pub(super) fn with_world(test: impl FnOnce(&World)) {
    let fixture = Fixture::new();
    with_world_at(&fixture.0, test);
}

/// The routing world of the authenticated project rooted at `dir`.
pub(super) fn with_world_at(dir: &std::path::Path, test: impl FnOnce(&World)) {
    with_authenticated_project(&dir.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let (semantic, fast) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let lifecycle = compile_source_agent_lifecycle(
            project.sources()[0].source(),
            project.sources()[0].path(),
            "fixture.agent",
        )?;
        let proposal = proposal(
            lifecycle.proposal_schema().schema().digest(),
            "5",
            false,
            "1",
        );
        let schema = compile_agent_interaction_schema(
            &dir.join("src/app.spx"),
            "fixture.agent.type.proposal",
        )?;
        let strong = strong_deployment(&fast);
        test(&World {
            project,
            root,
            semantic,
            fast,
            strong,
            schema,
            proposal,
        });
        Ok(())
    })
    .unwrap();
}

impl World {
    pub(super) fn set(&self) -> ApprovedProfileSet {
        ApprovedProfileSet::approve(
            &self.semantic,
            vec![
                spec("fast", &self.fast, 10, 1),
                spec("strong", &self.strong, 50, 9),
            ],
            RoutePolicy::default(),
        )
        .unwrap()
    }

    pub(super) fn target(&self, task: &[u8]) -> InvocationTarget<'_> {
        InvocationTarget {
            project: self.project.clone(),
            program: ProgramRootRef::V1(&self.root),
            expected_program_digest: self.root.program_root_digest(),
            source_path: "src/app.spx",
            agent_id: "fixture.agent",
            task: LifecycleTask {
                objective: task.to_vec(),
                budget: 12,
            },
            proposal: &self.proposal,
            lifecycle_budget: LifecycleBudget::default(),
            schema: &self.schema,
            started_at_millis: 0,
            turn: 0,
            observation: Vec::new(),
            max_response_bytes: 1024,
            effective_budget: 0,
            plan_context_tokens: 1,
            plan_output_tokens: 1,
            plan_cost_micros: 0,
            plan_max_polls: 8,
        }
    }
}

/// Runs `f` with fresh explicit host handlers over `factory`.
pub(super) fn handlers<'h>(
    clock: &'h StepClock,
    cancellation: &'h AgentCancellation,
    factory: &'h mut dyn semaprax::model_budget_policy::ProviderAdapterFactory,
    classifier: &'h mut semaprax::model_budget_policy::retry::ConservativeFailureClassifier,
    backoff: &'h mut semaprax::model_budget_policy::NoDelayBackoff,
) -> RoutedRunHandlers<'h> {
    RoutedRunHandlers {
        clock,
        cancellation,
        capability: AdapterInvocationCapability::grant("routing fixture"),
        factory,
        classifier,
        backoff,
    }
}

macro_rules! host {
    ($clock:ident, $cancel:ident, $classifier:ident, $backoff:ident) => {
        let $clock = semaprax::live_invocation::fixture::StepClock::new(0);
        let $cancel = semaprax::agent_runtime::AgentCancellation::new();
        let mut $classifier = semaprax::model_budget_policy::retry::ConservativeFailureClassifier;
        let mut $backoff = semaprax::model_budget_policy::NoDelayBackoff;
    };
}
pub(super) use host;

#[test]
fn two_tasks_select_different_approved_profiles_through_the_real_adapter_path() {
    with_world(|w| {
        let set = w.set();
        assert_eq!(set.profiles().len(), 2);
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut runs = Vec::new();
        for (family, task) in [
            (TaskFamily::LocalizedDebug, b"debug task".as_slice()),
            (TaskFamily::SemanticLaw, b"law task".as_slice()),
        ] {
            let mut store = MemStore::default();
            let target = w.target(task);
            let run = start_routed_task::<dyn DecisionInvoker>(
                &set,
                &features(family),
                &ctx("task"),
                None,
                &target,
                handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut store,
            )
            .unwrap();
            assert_eq!(run.run, DurablePolicyRun::Settled(response(&w.schema, "1")));
            assert_eq!(run.record.router_calls(), 0);
            // The route record is committed before any adapter (generation 0)
            // and stays attached to every policy-journal generation.
            assert_eq!(store.commits[0].0, 0);
            assert!(store.commits[0].1.contains(ENVELOPE_SCHEMA));
            assert!(store.commits[0].1.contains("\"policy\":null"));
            assert!(store
                .commits
                .iter()
                .all(|(_, d)| d.contains(&run.record.digest())));
            // The roots are those of the selected concrete deployment.
            let selected = set.profile(run.record.profile()).unwrap();
            let expected = bind_execution_revision(
                w.project.clone(),
                ProgramRootRef::V1(&w.root),
                w.root.program_root_digest(),
                "src/app.spx",
                "fixture.agent",
                selected.deployment_source(),
                LifecycleTask {
                    objective: task.to_vec(),
                    budget: 12,
                },
                &w.proposal,
                LifecycleBudget::default(),
            )
            .unwrap();
            assert_eq!(run.deployment_root, expected.deployment_root().digest());
            assert_eq!(run.instance_root, expected.instance_root().digest());
            assert_eq!(run.record.deployment(), selected.deployment_digest());
            runs.push(run);
        }
        assert_eq!(runs[0].record.profile(), "fast");
        assert_eq!(runs[0].record.source(), RouteSource::Rules);
        assert_eq!(runs[1].record.profile(), "strong");
        assert_ne!(runs[0].deployment_root, runs[1].deployment_root);
        assert_ne!(runs[0].invocation, runs[1].invocation);
        assert_eq!(created.borrow().as_slice(), ["fake.local", "other.local"]);
    });
}

#[test]
fn incompatible_profiles_are_rejected_before_model_construction() {
    with_world(|w| {
        let approve = |specs: Vec<ProfileSpec>| {
            ApprovedProfileSet::approve(&w.semantic, specs, RoutePolicy::default())
                .err()
                .expect("refused")
        };
        // Wrong semantic definition: the deployment names another digest.
        let marker = "\"definition_digest\":\"";
        let at = w.fast.find(marker).unwrap() + marker.len();
        let foreign = format!(
            "{}sha256:{}{}",
            &w.fast[..at],
            "0".repeat(64),
            &w.fast[at + 71..]
        );
        let wrong = approve(vec![spec("fast", &foreign, 10, 1)]);
        assert!(
            matches!(&wrong, RuntimeRoutingError::Deployment { code, message, .. }
            if code == "SPX-G556" && message.ends_with("definition_digest"))
        );
        // Expanded capability and tool grants.
        for (from, to, field) in [
            (
                "\"granted_capabilities\":[\"tool.read\"]",
                "\"granted_capabilities\":[\"tool.read\",\"tool.write\"]",
                "granted_capabilities",
            ),
            (
                "\"allowed_tool_ids\":[\"fixture.read\"]",
                "\"allowed_tool_ids\":[\"fixture.read\",\"fixture.write\"]",
                "allowed_tool_ids",
            ),
            ("\"max_turns\":2", "\"max_turns\":3", "limits"),
        ] {
            let widened = w.strong.replacen(from, to, 1);
            let refused = approve(vec![
                spec("fast", &w.fast, 10, 1),
                spec("strong", &widened, 50, 9),
            ]);
            assert!(
                matches!(&refused, RuntimeRoutingError::Deployment { profile, message, .. }
                if profile == "strong" && message.ends_with(field)),
                "{field}: {refused:?}"
            );
        }
        // Mismatched provider order.
        let mut reordered = spec("fast", &w.fast, 10, 1);
        reordered.provider_policy = Some(ProviderPolicy::new(vec![ProviderSlot::authorized(
            "other.local",
        )]));
        assert_eq!(
            approve(vec![reordered]),
            RuntimeRoutingError::PolicyBinding {
                profile: "fast".into(),
                refusal: DurablePolicyBindingRefusal::ProviderOrderMismatch
            }
        );
        // Excessive limits.
        let mut excessive = spec("fast", &w.fast, 10, 1);
        excessive.limits = intersect(
            ModelBudgetLimits {
                max_calls: 3,
                max_cost_micros: 0,
                max_latency_millis: 1_000,
                ..ModelBudgetLimits::unbounded()
            },
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .unwrap();
        assert_eq!(
            approve(vec![excessive]),
            RuntimeRoutingError::PolicyBinding {
                profile: "fast".into(),
                refusal: DurablePolicyBindingRefusal::RetainedLimitMismatch {
                    dimension: "max_calls"
                }
            }
        );
        // Unknown profile: a pin naming nothing approved, and a retained
        // route whose deployment this set does not approve. Neither reaches
        // the factory.
        let set = w.set();
        let mut pinned = features(TaskFamily::LocalizedDebug);
        pinned.operator_pin = Some("nope".into());
        assert_eq!(
            route_new_invocation::<dyn DecisionInvoker>(&set, &pinned, &ctx("x"), None).err(),
            Some(RuntimeRoutingError::UnknownPin("nope".into()))
        );
        let only_fast = ApprovedProfileSet::approve(
            &w.semantic,
            vec![spec("fast", &w.fast, 10, 1)],
            RoutePolicy::default(),
        )
        .unwrap();
        let mut strong_pin = features(TaskFamily::LocalizedDebug);
        strong_pin.operator_pin = Some("strong".into());
        let record =
            route_new_invocation::<dyn DecisionInvoker>(&set, &strong_pin, &ctx("x"), None)
                .unwrap()
                .into_record();
        let target = w.target(b"task");
        assert!(matches!(
            bind_routed_invocation(&only_fast, record, &target).err(),
            Some(RuntimeRoutingError::ProfileNotApproved { .. })
        ));
    });
}

#[test]
fn replay_makes_zero_router_calls_and_keeps_the_profile_after_catalog_drift() {
    with_world(|w| {
        let set = w.set();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut inv = ScriptedRouter {
            calls: 0,
            answer: Some(choose("strong")),
        };
        let mut store = MemStore::default();
        let target = w.target(b"routed task");
        let first = start_routed_task(
            &set,
            &features(TaskFamily::LocalizedDebug),
            &ctx("task"),
            Some(&mut router(&mut inv, ProviderMode::Explicit)),
            &target,
            handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
            &mut store,
        )
        .unwrap();
        assert_eq!(inv.calls, 1);
        assert_eq!(first.record.profile(), "strong");
        assert_eq!(first.record.source(), RouteSource::Provider);
        let (envelope, generation) = store.latest();

        // Catalog drift: aliases renamed, estimates changed so rules (and the
        // router, if asked) would now prefer `fast`.
        let mut fast = spec("fast", &w.fast, 1, 1);
        fast.model.alias = "renamed-fast".into();
        let mut strong = spec("strong", &w.strong, 900, 2);
        strong.model.alias = "renamed-strong".into();
        let drifted =
            ApprovedProfileSet::approve(&w.semantic, vec![fast, strong], RoutePolicy::default())
                .unwrap();
        assert_ne!(drifted.digest(), set.digest());
        let replay_created: Created = Rc::default();
        let mut replay_factory =
            settling_factory(&drifted, response(&w.schema, "1"), replay_created.clone());
        let replayed = resume_routed_invocation(
            &drifted,
            &envelope,
            generation,
            &target,
            handlers(
                &clock,
                &cancel,
                &mut replay_factory,
                &mut classifier,
                &mut backoff,
            ),
            &mut store,
        )
        .unwrap();
        assert_eq!(
            replayed.run,
            DurablePolicyRun::Replayed(response(&w.schema, "1"))
        );
        assert_eq!(replayed.record, first.record);
        assert_eq!(replayed.record.profile(), "strong");
        assert_eq!(replayed.deployment_root, first.deployment_root);
        assert_eq!(inv.calls, 1, "resume makes no router call");
        assert!(
            replay_created.borrow().is_empty(),
            "replay constructs no adapter"
        );

        // Revocation stops further work; it never rebinds to another profile.
        let revoked = ApprovedProfileSet::approve(
            &w.semantic,
            vec![spec("fast", &w.fast, 10, 1)],
            RoutePolicy::default(),
        )
        .unwrap();
        assert!(matches!(
            resume_routed_invocation(
                &revoked,
                &envelope,
                generation,
                &target,
                handlers(
                    &clock,
                    &cancel,
                    &mut replay_factory,
                    &mut classifier,
                    &mut backoff
                ),
                &mut store,
            )
            .err(),
            Some(RuntimeRoutingError::ProfileNotApproved { .. })
        ));
        assert!(replay_created.borrow().is_empty());
    });
}

#[test]
fn an_inadmissible_pin_refuses_and_never_falls_back() {
    with_world(|w| {
        let set = w.set();
        let route = |f: &RuntimeFeatures| {
            route_new_invocation::<dyn DecisionInvoker>(&set, f, &ctx("pin"), None)
        };
        let mut pinned = features(TaskFamily::LocalizedDebug);
        pinned.operator_pin = Some("strong".into());
        let ok = route(&pinned).unwrap();
        assert_eq!(ok.profile().id(), "strong", "rules alone would pick `fast`");
        assert_eq!(ok.record().source(), RouteSource::Pin);
        assert_eq!(ok.record().router_calls(), 0);

        let mut unaffordable = pinned.clone();
        unaffordable.remaining_cost_micros = 20;
        assert_eq!(
            route(&unaffordable).err(),
            Some(RuntimeRoutingError::InadmissiblePin {
                profile: "strong".into(),
                reason: "unaffordable".into()
            })
        );
        let mut image = pinned.clone();
        image.required_modalities = [Modality::Image].into();
        assert_eq!(
            route(&image).err(),
            Some(RuntimeRoutingError::InadmissiblePin {
                profile: "strong".into(),
                reason: "missing `image` modality".into()
            })
        );
        // Without the pin the same request routes to the cheaper admissible
        // profile, so the refusal above is not a capacity failure.
        unaffordable.operator_pin = None;
        assert_eq!(route(&unaffordable).unwrap().profile().id(), "fast");
    });
}

#[test]
fn an_unavailable_router_takes_an_explained_rules_fallback_and_no_admissible_profile_fails_first() {
    with_world(|w| {
        let set = w.set();
        let mut down = ScriptedRouter {
            calls: 0,
            answer: None,
        };
        let routed = route_new_invocation(
            &set,
            &features(TaskFamily::LocalizedDebug),
            &ctx("fallback"),
            Some(&mut router(&mut down, ProviderMode::Explicit)),
        )
        .unwrap();
        assert_eq!(down.calls, 1);
        assert_eq!(routed.record().source(), RouteSource::Fallback);
        assert_eq!(routed.profile().id(), "fast");
        let why = routed.record().explanation().unwrap();
        assert!(
            why.contains("fixture-router") && why.contains("Unavailable"),
            "{why}"
        );
        assert!(why.contains("rules fallback chose `fast`"), "{why}");

        // A learned router that is not evaluated is never consulted (Auto).
        let mut unevaluated = ScriptedRouter {
            calls: 0,
            answer: Some(choose("strong")),
        };
        let auto = route_new_invocation(
            &set,
            &features(TaskFamily::LocalizedDebug),
            &ctx("auto"),
            Some(&mut router(&mut unevaluated, ProviderMode::Auto)),
        )
        .unwrap();
        assert_eq!(unevaluated.calls, 0);
        assert_eq!(auto.record().source(), RouteSource::Rules);

        // Nothing admissible: refused before any adapter or paid generation.
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut broke = features(TaskFamily::LocalizedDebug);
        broke.remaining_cost_micros = 1;
        let mut store = MemStore::default();
        let target = w.target(b"task");
        let refused = start_routed_task::<dyn DecisionInvoker>(
            &set,
            &broke,
            &ctx("broke"),
            None,
            &target,
            handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
            &mut store,
        )
        .err()
        .unwrap();
        assert_eq!(
            refused,
            RuntimeRoutingError::NoAdmissibleProfile {
                excluded: vec![
                    ("fast".into(), "unaffordable".into()),
                    ("strong".into(), "unaffordable".into())
                ]
            }
        );
        assert!(created.borrow().is_empty());
        assert!(
            store.commits.is_empty(),
            "no route record for a refused task"
        );
    });
}

#[test]
fn a_static_single_deployment_route_is_trivial_and_binds_the_unrouted_policy_bytes() {
    with_world(|w| {
        let single = ApprovedProfileSet::approve(
            &w.semantic,
            vec![spec("only", &w.fast, 10, 1)],
            RoutePolicy::default(),
        )
        .unwrap();
        let mut inv = ScriptedRouter {
            calls: 0,
            answer: Some(choose("only")),
        };
        let routed = route_new_invocation(
            &single,
            &features(TaskFamily::LocalizedDebug),
            &ctx("static"),
            Some(&mut router(&mut inv, ProviderMode::Explicit)),
        )
        .unwrap();
        assert_eq!(
            inv.calls, 0,
            "one admissible profile never consults a router"
        );
        assert_eq!(routed.record().source(), RouteSource::Trivial);
        let target = w.target(b"alpha");
        let bound = bind_routed_invocation(&single, routed.into_record(), &target).unwrap();

        // The same binding the unrouted static path builds, byte for byte.
        let deployment =
            semaprax::agent_deployment::bind_agent_deployment(&w.semantic, &w.fast).unwrap();
        let execution = bind_execution_revision(
            w.project.clone(),
            ProgramRootRef::V1(&w.root),
            w.root.program_root_digest(),
            "src/app.spx",
            "fixture.agent",
            &w.fast,
            LifecycleTask {
                objective: b"alpha".to_vec(),
                budget: 12,
            },
            &w.proposal,
            LifecycleBudget::default(),
        )
        .unwrap();
        let selected = deployment.model_selections();
        let seed = semaprax::live_invocation::LiveInvocationSeed {
            program_root: w.root.program_root_digest().to_owned(),
            deployment_policy: deployment.digest().to_owned(),
            task: b"alpha".to_vec(),
            budget: 12,
            interaction_schema_digest: w.schema.schema().digest().to_owned(),
            approved_providers: selected
                .iter()
                .map(|r| r.provider_id().to_owned())
                .collect(),
        };
        let unrouted = semaprax::model_budget_policy::DurablePolicyBinding::bind(
            &execution,
            &deployment,
            &w.schema,
            &seed,
            ProviderPolicy::new(
                selected
                    .iter()
                    .map(|r| ProviderSlot::authorized(r.provider_id()))
                    .collect(),
            ),
            limits(),
            0,
        )
        .unwrap();
        assert_eq!(bound.binding().digest(), unrouted.digest());
        assert_eq!(
            bound.execution().execution_revision(),
            execution.execution_revision()
        );
    });
}
