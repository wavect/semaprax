use super::agent_lifecycle_v1::proposal;
use super::source_agent_lifecycle::source_module;
use semaprax::agent_deployment::{bind_agent_deployment, migrate_agent_definition_v1};
use semaprax::agent_interaction_schema::compile_agent_interaction_schema;
use semaprax::agent_lifecycle::{
    compile_source_agent_lifecycle, FixtureRead, LifecycleBudget, LifecycleStatus, LifecycleTask,
};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::execution_revision::{bind_execution_revision, ProgramRootRef};
use semaprax::live_invocation::fixture::StepClock;
use semaprax::live_invocation::fixture::{
    fixture_response, FixtureAuthorizationGate, FixtureBudgetHook, FixtureModelHandler,
    FixtureObserver, FixturePolicy, FixtureProposalDecoder,
};
use semaprax::live_invocation::{
    run_durable_policy_invocation, DurablePolicyRun, DurablePolicyRunError, ModelInvocationRequest,
};
use semaprax::live_invocation::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveInvocationId,
    LiveInvocationOutcome, LiveInvocationSeed, ModelInvocationOutcome, ModelInvokeCapability,
};
use semaprax::model_budget_policy::{
    intersect, DurablePolicyBinding, DurablePolicyBindingRefusal, ModelBudgetLimits,
    ProviderPolicy, ProviderSlot,
};
use semaprax::project::with_authenticated_project;
use semaprax::provider_adapter_sdk::{
    AdapterCapabilities, AdapterInvocationCapability, AdapterModelIdentity, AdapterPoll,
    AdapterRefusal, AdapterRequest, CancellationSemantics, EndpointPolicy, ProviderAdapter,
    StructuredOutputMode, TokenAccountingSource,
};
use std::cell::Cell;
use std::rc::Rc;

pub(super) struct Fixture(pub(super) std::path::PathBuf);
impl Fixture {
    pub(super) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "spx-execution-roots-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("src")).unwrap();
        let source = source_module(
            "fixture.agent.fn.reduce",
            r#"
@id("fixture.export.payload")
record ExportPayload { @id("fixture.export.payload.bytes") bytes: Bytes, }
@id("fixture.export.build")
fn build(input: borrow Slice<u8>) -> ExportPayload { ExportPayload { bytes: bytes_copy(input) } }
"#,
        );
        let source = source
            .replace(
                "    @id(\"fixture.agent.type.observation.tag\")\n    tag: Bytes,\n",
                "",
            )
            .replace("    let tag = [79u8, 66u8];\n", "")
            .replace("        tag: bytes_copy(array_as_slice(tag)),\n", "");
        let source = semaprax::format::canonical(&semaprax::parse(&source, "src/app.spx").unwrap());
        std::fs::write(path.join("src/app.spx"), source).unwrap();
        std::fs::write(
            path.join("src/tests.spx"),
            "module fixture.tests;\n\n@id(\"fixture.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        std::fs::write(path.join("semaprax.toml"), "schema = \"semaprax.project.v11\"\nname = \"fixture\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"fixture.agent.lifecycle\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"fixture.export.build\"]\ntests = [\"fixture.tests\"]\n").unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct IdentityProbeAdapter {
    capabilities: AdapterCapabilities,
    model: AdapterModelIdentity,
    starts: Rc<Cell<u32>>,
}

impl ProviderAdapter for IdentityProbeAdapter {
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
        self.starts.set(self.starts.get() + 1);
        Err(AdapterRefusal("fixture start refusal".to_owned()))
    }
    fn poll(&mut self) -> AdapterPoll {
        unreachable!("start always refuses")
    }
    fn cancel(&mut self, _: &str) {}
}

struct PolicyStore;
impl semaprax::agent_lifecycle::CheckpointStore for PolicyStore {
    fn commit(
        &mut self,
        _: u64,
        _: &str,
    ) -> Result<(), semaprax::agent_lifecycle::CheckpointStoreError> {
        Ok(())
    }
}

pub(super) fn probe_capabilities(provider: &str, max_context_tokens: u64) -> AdapterCapabilities {
    AdapterCapabilities {
        adapter_identity: "identity-probe".to_owned(),
        adapter_version: "1".to_owned(),
        provider_profile: provider.to_owned(),
        structured_output_modes: vec![StructuredOutputMode::RawText],
        supports_streaming: true,
        token_accounting_source: TokenAccountingSource::LocalEstimate,
        cancellation_semantics: CancellationSemantics::BestEffortRequestStop,
        retryable_failure_classes: Vec::new(),
        endpoint_policy: EndpointPolicy::HostInjected,
        max_request_bytes: 65_536,
        max_response_bytes: 65_536,
        max_context_tokens,
        max_output_tokens: 8_192,
    }
}

#[test]
fn execution_roots_bind_retained_source_and_actual_run() {
    let fixture = Fixture::new();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let foreign_fixture = Fixture::new();
        let foreign_path = foreign_fixture.0.join("src/app.spx");
        let foreign_source = std::fs::read_to_string(&foreign_path)
            .unwrap()
            .replace("fn main() -> i64\n{\n    0", "fn main() -> i64\n{\n    1");
        assert_ne!(
            foreign_source,
            std::fs::read_to_string(&foreign_path).unwrap()
        );
        std::fs::write(&foreign_path, foreign_source).unwrap();
        let foreign =
            with_authenticated_project(&foreign_fixture.0.join("semaprax.toml"), |other| {
                other.retain_revision().program_root()
            })?;
        assert_ne!(foreign.program_root_digest(), root.program_root_digest());
        let source = &project.sources()[0];
        let lifecycle =
            compile_source_agent_lifecycle(source.source(), source.path(), "fixture.agent")?;
        let (_, deployment) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let proposed = proposal(
            lifecycle.proposal_schema().schema().digest(),
            "5",
            false,
            "1",
        );
        let bind = |path: &str, expected: &str, budget: i64| {
            bind_execution_revision(
                project.clone(),
                ProgramRootRef::V1(&root),
                expected,
                path,
                "fixture.agent",
                &deployment,
                LifecycleTask {
                    objective: b"alpha".to_vec(),
                    budget,
                },
                &proposed,
                LifecycleBudget::default(),
            )
        };
        let first = bind("src/app.spx", root.program_root_digest(), 12)?;
        let same = bind("src/app.spx", root.program_root_digest(), 12)?;
        assert_eq!(first.execution_revision(), same.execution_revision());
        let different = bind("src/app.spx", root.program_root_digest(), 13)?;
        assert_ne!(first.instance_root(), different.instance_root());
        assert!(!first.instance_root().canonical_json().contains("alpha"));
        assert_eq!(
            bind("src/missing.spx", root.program_root_digest(), 12)
                .err()
                .unwrap()[0]
                .code,
            "SPX-G583"
        );
        assert_eq!(
            bind("src/app.spx", "sha256:stale", 12).err().unwrap()[0].code,
            "SPX-G583"
        );
        let foreign_error = bind_execution_revision(
            project.clone(),
            ProgramRootRef::V1(&foreign),
            foreign.program_root_digest(),
            "src/app.spx",
            "fixture.agent",
            &deployment,
            LifecycleTask {
                objective: b"alpha".to_vec(),
                budget: 12,
            },
            &proposed,
            LifecycleBudget::default(),
        )
        .err()
        .unwrap();
        assert_eq!(foreign_error[0].code, "SPX-G583");
        assert!(foreign_error[0].message.contains("source-owned segment"));
        let workspace = project.canonical_workspace_revision()?;
        let lock = semaprax::project::render_project_lock(snapshot)?;
        let association =
            root.associate_dependency_lock(snapshot, root.program_root_digest(), &lock)?;
        let interface = semaprax::project::InterfaceArtifactFacts::derive(
            project.clone(),
            project.project_revision(),
            &[semaprax::project::ImageArtifactKind::Npm],
            semaprax::project::MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let contracts = semaprax::project::ContractsAndTestsFacts::derive(
            project.clone(),
            project.project_revision(),
        )?;
        let v2 =
            semaprax::project::ProgramRootV2::derive(&workspace, &root, &interface, &association)?;
        let v3 = semaprax::project::ProgramRootV3::derive(
            &workspace,
            &root,
            &interface,
            &association,
            &contracts,
        )?;
        for program in [ProgramRootRef::V2(&v2), ProgramRootRef::V3(&v3)] {
            let bound = bind_execution_revision(
                project.clone(),
                program,
                program.digest(),
                "src/app.spx",
                "fixture.agent",
                &deployment,
                LifecycleTask {
                    objective: b"alpha".to_vec(),
                    budget: 12,
                },
                &proposed,
                LifecycleBudget::default(),
            )?;
            assert_ne!(bound.execution_revision(), first.execution_revision());
            let mut operation = FixtureRead::new(b"observed".to_vec());
            assert_eq!(
                bound
                    .run(&mut operation, &AgentCancellation::new())?
                    .run()
                    .status(),
                LifecycleStatus::Completed
            );
            assert_eq!(operation.calls(), 1);
        }
        let revision = first.execution_revision().clone();
        let mut read = FixtureRead::new(b"observed".to_vec());
        let evidence = first.run(&mut read, &AgentCancellation::new())?;
        assert_eq!(evidence.run().status(), LifecycleStatus::Completed);
        assert_eq!(read.calls(), 1);
        assert_eq!(evidence.execution_revision(), &revision);
        assert!(evidence
            .evidence_root()
            .canonical_json()
            .contains(evidence.run().evidence_digest()));
        Ok(())
    })
    .unwrap();
}

#[test]
fn durable_policy_binding_rederives_retained_execution_seed_and_schema() {
    let fixture = Fixture::new();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let (_, deployment_source) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let (semantic, _) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let deployment = bind_agent_deployment(&semantic, &deployment_source)?;
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
        let execution = bind_execution_revision(
            project.clone(),
            ProgramRootRef::V1(&root),
            root.program_root_digest(),
            "src/app.spx",
            "fixture.agent",
            &deployment_source,
            LifecycleTask {
                objective: b"alpha".to_vec(),
                budget: 12,
            },
            &proposal,
            LifecycleBudget::default(),
        )?;
        let schema = compile_agent_interaction_schema(
            &fixture.0.join("src/app.spx"),
            "fixture.agent.type.proposal",
        )?;
        let selected = deployment.model_selections();
        let policy = ProviderPolicy::new(
            selected
                .iter()
                .map(|row| ProviderSlot::authorized(row.provider_id()))
                .collect(),
        );
        let seed = LiveInvocationSeed {
            program_root: root.program_root_digest().to_owned(),
            deployment_policy: deployment.digest().to_owned(),
            task: b"alpha".to_vec(),
            budget: 12,
            interaction_schema_digest: schema.schema().digest().to_owned(),
            approved_providers: selected
                .iter()
                .map(|row| row.provider_id().to_owned())
                .collect(),
        };
        let limits = intersect(
            ModelBudgetLimits {
                max_cost_micros: 0,
                max_latency_millis: 1_000,
                ..ModelBudgetLimits::single_call_only(1, 1)
            },
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .unwrap();
        let byte_binding = DurablePolicyBinding::bind(
            &execution,
            &deployment,
            &schema,
            &seed,
            policy.clone(),
            limits,
            0,
        )
        .unwrap();

        let definition: serde_json::Value =
            serde_json::from_str(deployment.semantic_definition().canonical_json()).unwrap();
        let deployed: serde_json::Value =
            serde_json::from_str(deployment.deployment().canonical_json()).unwrap();
        let ceiling = |key: &str| {
            definition["ceilings"][key]
                .as_u64()
                .unwrap()
                .min(deployed["limits"][key].as_u64().unwrap())
        };
        let bytes = semaprax::model_budget_policy::DurableByteBudget {
            max_request_bytes: ceiling("max_provider_request_bytes"),
            max_response_bytes: ceiling("max_provider_response_bytes"),
            max_total_input_bytes: ceiling("max_total_provider_input_bytes"),
            max_total_output_bytes: ceiling("max_total_provider_output_bytes"),
        };
        assert_eq!(byte_binding.narrow_byte_budget(bytes), Ok(bytes));
        for dimension in 0..4 {
            let mut widened = bytes;
            let value = match dimension {
                0 => &mut widened.max_request_bytes,
                1 => &mut widened.max_response_bytes,
                2 => &mut widened.max_total_input_bytes,
                _ => &mut widened.max_total_output_bytes,
            };
            *value = value
                .checked_add(1)
                .expect("bounded retained fixture ceiling");
            assert!(matches!(
                byte_binding.narrow_byte_budget(widened),
                Err(DurablePolicyBindingRefusal::RetainedByteLimitMismatch { .. })
            ));
        }

        let mut swapped_seed = seed.clone();
        swapped_seed.task.push(b'!');
        assert_eq!(
            DurablePolicyBinding::bind(
                &execution,
                &deployment,
                &schema,
                &swapped_seed,
                policy.clone(),
                limits,
                0
            ),
            Err(DurablePolicyBindingRefusal::ExecutionRootMismatch),
        );
        let mut swapped_root = seed.clone();
        swapped_root.program_root = "sha256:swapped".to_owned();
        assert_eq!(
            DurablePolicyBinding::bind(
                &execution,
                &deployment,
                &schema,
                &swapped_root,
                policy.clone(),
                limits,
                0
            ),
            Err(DurablePolicyBindingRefusal::ExecutionRootMismatch),
        );
        let other_schema = compile_agent_interaction_schema(
            &fixture.0.join("src/app.spx"),
            "fixture.agent.type.observation",
        )?;
        assert_eq!(
            DurablePolicyBinding::bind(
                &execution,
                &deployment,
                &other_schema,
                &seed,
                policy.clone(),
                limits,
                0
            ),
            Err(DurablePolicyBindingRefusal::RetainedSchemaMismatch),
        );
        let calls_widened = intersect(
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
            DurablePolicyBinding::bind(
                &execution,
                &deployment,
                &schema,
                &seed,
                policy.clone(),
                calls_widened,
                0
            ),
            Err(DurablePolicyBindingRefusal::RetainedLimitMismatch {
                dimension: "max_calls"
            }),
        );
        let cost_widened = intersect(
            ModelBudgetLimits {
                max_calls: 1,
                max_retries: 0,
                max_providers: 0,
                max_context_tokens: 1,
                max_output_tokens: 1,
                max_aggregate_tokens: 2,
                max_cost_micros: 1,
                max_latency_millis: 1_000,
            },
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .unwrap();
        assert_eq!(
            DurablePolicyBinding::bind(
                &execution,
                &deployment,
                &schema,
                &seed,
                policy,
                cost_widened,
                0
            ),
            Err(DurablePolicyBindingRefusal::RetainedLimitMismatch {
                dimension: "max_cost_micros"
            }),
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn durable_policy_dispatch_binds_the_constructed_adapter_model_before_start() {
    let fixture = Fixture::new();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let (semantic, deployment_source) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let deployment = bind_agent_deployment(&semantic, &deployment_source)?;
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
        let execution = bind_execution_revision(
            project.clone(),
            ProgramRootRef::V1(&root),
            root.program_root_digest(),
            "src/app.spx",
            "fixture.agent",
            &deployment_source,
            LifecycleTask {
                objective: b"alpha".to_vec(),
                budget: 12,
            },
            &proposal,
            LifecycleBudget::default(),
        )?;
        let schema = compile_agent_interaction_schema(
            &fixture.0.join("src/app.spx"),
            "fixture.agent.type.proposal",
        )?;
        let selected = deployment.model_selections();
        let policy = ProviderPolicy::new(
            selected
                .iter()
                .map(|row| ProviderSlot::authorized(row.provider_id()))
                .collect(),
        );
        let seed = LiveInvocationSeed {
            program_root: root.program_root_digest().to_owned(),
            deployment_policy: deployment.digest().to_owned(),
            task: b"alpha".to_vec(),
            budget: 12,
            interaction_schema_digest: schema.schema().digest().to_owned(),
            approved_providers: selected
                .iter()
                .map(|row| row.provider_id().to_owned())
                .collect(),
        };
        let limits = intersect(
            ModelBudgetLimits {
                max_cost_micros: 0,
                max_latency_millis: 1_000,
                ..ModelBudgetLimits::single_call_only(1, 1)
            },
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .unwrap();
        let binding =
            DurablePolicyBinding::bind(&execution, &deployment, &schema, &seed, policy, limits, 0)
                .unwrap();
        let request = ModelInvocationRequest {
            turn: 0,
            task: seed.task.clone(),
            observation: Vec::new(),
            proposal_grammar_digest: schema.schema().digest().to_owned(),
            deployment_binding: deployment.digest().to_owned(),
            max_response_bytes: 1024,
            effective_budget: 0,
        };
        let plan = semaprax::model_budget_policy::AdapterAttemptPlan::for_compiled(
            &schema, &request, 1, 1, 0, 1,
        )
        .unwrap();
        let selected = &selected[0];
        let max_context_tokens = selected.max_context_tokens();
        let exact_starts = Rc::new(Cell::new(0));
        let exact_counter = exact_starts.clone();
        let exact_identity = AdapterModelIdentity {
            provider_id: selected.provider_id().to_owned(),
            model_id: selected.model_id().to_owned(),
            capabilities: selected.capabilities().to_vec(),
        };
        let mut exact_factory = move |provider: &str| {
            Ok(Box::new(IdentityProbeAdapter {
                capabilities: probe_capabilities(provider, max_context_tokens),
                model: exact_identity.clone(),
                starts: exact_counter.clone(),
            }) as Box<dyn ProviderAdapter>)
        };
        let mut store = PolicyStore;
        let mut classifier = semaprax::model_budget_policy::retry::ConservativeFailureClassifier;
        let mut backoff = semaprax::model_budget_policy::NoDelayBackoff;
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let _ = run_durable_policy_invocation(
            &binding,
            &schema,
            &request,
            &plan,
            &clock,
            &cancellation,
            AdapterInvocationCapability::grant("fixture"),
            &mut exact_factory,
            &mut classifier,
            &mut backoff,
            &mut store,
            None,
        );
        assert_eq!(
            exact_starts.get(),
            1,
            "exact model identity reaches adapter start"
        );

        let request_factory_calls = Rc::new(Cell::new(0));
        let request_factory_counter = request_factory_calls.clone();
        let request_identity = AdapterModelIdentity {
            provider_id: selected.provider_id().to_owned(),
            model_id: selected.model_id().to_owned(),
            capabilities: selected.capabilities().to_vec(),
        };
        let mut request_factory = move |provider: &str| {
            request_factory_counter.set(request_factory_counter.get() + 1);
            Ok(Box::new(IdentityProbeAdapter {
                capabilities: probe_capabilities(provider, max_context_tokens),
                model: request_identity.clone(),
                starts: Rc::new(Cell::new(0)),
            }) as Box<dyn ProviderAdapter>)
        };
        for changed in [
            ModelInvocationRequest {
                task: b"swapped".to_vec(),
                ..request.clone()
            },
            ModelInvocationRequest {
                proposal_grammar_digest: "sha256:swapped".to_owned(),
                ..request.clone()
            },
            ModelInvocationRequest {
                effective_budget: 13,
                ..request.clone()
            },
        ] {
            let mut request_store = PolicyStore;
            let refused = run_durable_policy_invocation(
                &binding,
                &schema,
                &changed,
                &plan,
                &clock,
                &cancellation,
                AdapterInvocationCapability::grant("fixture"),
                &mut request_factory,
                &mut classifier,
                &mut backoff,
                &mut request_store,
                None,
            );
            assert_eq!(
                refused,
                DurablePolicyRun::Refused(DurablePolicyRunError::RequestBindingMismatch)
            );
        }
        assert_eq!(
            request_factory_calls.get(),
            0,
            "swapped request fields never reach the factory"
        );

        let wrong_starts = Rc::new(Cell::new(0));
        let wrong_counter = wrong_starts.clone();
        let wrong_identity = AdapterModelIdentity {
            provider_id: selected.provider_id().to_owned(),
            model_id: "wrong-model".to_owned(),
            capabilities: selected.capabilities().to_vec(),
        };
        let mut wrong_factory = move |provider: &str| {
            Ok(Box::new(IdentityProbeAdapter {
                capabilities: probe_capabilities(provider, max_context_tokens),
                model: wrong_identity.clone(),
                starts: wrong_counter.clone(),
            }) as Box<dyn ProviderAdapter>)
        };
        let mut wrong_store = PolicyStore;
        let wrong = run_durable_policy_invocation(
            &binding,
            &schema,
            &request,
            &plan,
            &clock,
            &cancellation,
            AdapterInvocationCapability::grant("fixture"),
            &mut wrong_factory,
            &mut classifier,
            &mut backoff,
            &mut wrong_store,
            None,
        );
        assert_eq!(
            wrong_starts.get(),
            0,
            "wrong model is refused before adapter start"
        );
        assert!(
            !matches!(
                wrong,
                DurablePolicyRun::Settled(_) | DurablePolicyRun::Replayed(_)
            ),
            "a wrong model identity never publishes a response"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn frozen_execution_revision_evidence_is_byte_identical_after_a_live_fixture_run() {
    // Issue #108 requires old frozen runtime invocations and evidence to
    // retain byte-identical behavior. This exercises the retained source
    // runtime through its real bind/run APIs on both sides of one complete,
    // offline live-invocation fixture run; comparing the canonical roots and
    // lifecycle evidence digest catches any accidental cross-route mutation.
    let fixture = Fixture::new();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let root = project.program_root()?;
        let source = &project.sources()[0];
        let lifecycle =
            compile_source_agent_lifecycle(source.source(), source.path(), "fixture.agent")?;
        let (_, deployment) = migrate_agent_definition_v1(
            project.agent_definitions()[0]
                .definition()
                .canonical_source(),
            "fixture.deployment",
        )?;
        let proposed = proposal(
            lifecycle.proposal_schema().schema().digest(),
            "5",
            false,
            "1",
        );

        let run_frozen = || {
            let revision = bind_execution_revision(
                project.clone(),
                ProgramRootRef::V1(&root),
                root.program_root_digest(),
                "src/app.spx",
                "fixture.agent",
                &deployment,
                LifecycleTask {
                    objective: b"alpha".to_vec(),
                    budget: 12,
                },
                &proposed,
                LifecycleBudget::default(),
            )?;
            let mut read = FixtureRead::new(b"observed".to_vec());
            let evidence = revision.run(&mut read, &AgentCancellation::new())?;
            assert_eq!(evidence.run().status(), LifecycleStatus::Completed);
            assert_eq!(read.calls(), 1);
            Ok::<_, Vec<semaprax::diagnostic::Diagnostic>>((
                evidence.execution_revision().canonical_json().to_owned(),
                evidence.evidence_root().canonical_json().to_owned(),
                evidence.run().evidence_digest().to_owned(),
            ))
        };

        let before = run_frozen()?;

        let schema = "sha256:0000000000000000000000000000000000000000000000000000000000aa";
        let live_identity = LiveInvocationId::derive(&LiveInvocationSeed {
            program_root: root.program_root_digest().to_owned(),
            deployment_policy: "sha256:live-fixture-policy".to_owned(),
            task: b"fixture live task".to_vec(),
            budget: 10,
            interaction_schema_digest: schema.to_owned(),
            approved_providers: vec!["fixture-provider".to_owned()],
        });
        let config = LiveInvocationConfig {
            identity: &live_identity,
            task: b"fixture live task",
            deployment_binding: "sha256:live-fixture-policy",
            interaction_schema_digest: schema,
            max_turns: 1,
            max_response_bytes: 4096,
            requested_budget_per_turn: 10,
        };
        let capability = ModelInvokeCapability::grant("#108 frozen-runtime regression fixture");
        let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
            fixture_response(0, "live"),
        )]);
        let mut decoder = FixtureProposalDecoder::new(schema);
        let mut gate = FixtureAuthorizationGate::new(1);
        let mut budget = FixtureBudgetHook::new(10);
        let mut observer = FixtureObserver;
        let mut policy = FixturePolicy { total_turns: 1 };
        let mut handlers = LiveInvocationHandlers {
            capability: &capability,
            handler: &mut handler,
            decoder: &mut decoder,
            gate: &mut gate,
            budget: &mut budget,
            observer: &mut observer,
            policy: &mut policy,
            effect: None,
            sink: None,
        };
        let live = run_live_invocation(
            &config,
            Vec::new(),
            &mut handlers,
            &AgentCancellation::new(),
        )
        .expect("the deterministic live fixture completes");
        assert_eq!(live.dispatched, 1);
        assert!(matches!(live.outcome, LiveInvocationOutcome::Complete(_)));

        assert_eq!(run_frozen()?, before);
        Ok(())
    })
    .unwrap();
}

#[path = "execution_revision/workspace.rs"]
mod workspace;

#[path = "execution_revision/iterative.rs"]
mod iterative;

#[path = "execution_revision/typed.rs"]
mod typed;
