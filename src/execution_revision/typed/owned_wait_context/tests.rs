use super::*;
use crate::agent_lifecycle::iterative::effects::{EffectArgument, EffectResult, EffectScalar};
use crate::project::with_authenticated_project;
use crate::provider_adapter_sdk::{AdapterInvocationCapability, ProviderAdapter};
use crate::resumable_effects::owned_frame::v2::compile_owned_agent_wait_v8;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixture() -> Fixture {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "spx-owned-wait-context-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    std::fs::create_dir(path.join("src")).unwrap();
    std::fs::write(
        path.join("semaprax.toml"),
        include_str!("../../../../examples/offline-repair-project/semaprax.toml"),
    )
    .unwrap();
    std::fs::write(
        path.join("src/tests.spx"),
        include_str!("../../../../examples/offline-repair-project/src/tests.spx"),
    )
    .unwrap();
    let source = include_str!("../../../../examples/offline-repair-project/src/app.spx").replace(
        "    runtime_v1 {",
        "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {",
    );
    std::fs::write(
        path.join("src/app.spx"),
        format!(
            "{source}\n{}",
            r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
        ),
    )
    .unwrap();
    let app = path.join("src/app.spx");
    let parsed = crate::parse(&std::fs::read_to_string(&app).unwrap(), &app).unwrap();
    std::fs::write(&app, crate::format::canonical(&parsed)).unwrap();
    Fixture(path)
}
fn operations() -> Vec<EffectOperation> {
    ["fixture.read", "fixture.read.second"]
        .into_iter()
        .map(|id| EffectOperation {
            operation_id: id.into(),
            effect_id: "read".into(),
            arguments: vec![EffectArgument {
                argument_id: "query".into(),
                proposal_field_id: "fixture.agent.type.proposal.budget".into(),
                kind: EffectScalar::I64,
            }],
            results: vec![EffectResult {
                result_id: "value".into(),
                kind: EffectScalar::I64,
            }],
        })
        .collect()
}
fn runtime(
    project: Arc<ProjectRevision>,
    effects: EffectBudget,
    reverse_registry: bool,
    objective: &[u8],
) -> AgentRuntimeV2 {
    let root = project.program_root().unwrap();
    let (_, deployment) = migrate_agent_definition_v1(
        project.agent_definitions()[0]
            .definition()
            .canonical_source(),
        "fixture.owned.wait.runtime",
    )
    .unwrap();
    let mut registry = operations();
    if reverse_registry {
        registry.reverse();
    }
    bind_agent_runtime_v2_live(
        project,
        ProgramRootRef::V1(&root),
        root.program_root_digest(),
        "src/app.spx",
        "fixture.agent",
        "fixture.agent.type.step",
        "fixture.agent.type.proposal.sequence",
        registry,
        &deployment,
        LifecycleTask {
            objective: objective.to_vec(),
            budget: 12,
        },
        IterativeBudget {
            max_steps_per_stage: 1000,
            ..IterativeBudget::default()
        },
        effects,
    )
    .unwrap()
}
fn effects() -> EffectBudget {
    EffectBudget {
        max_calls: 3,
        max_argument_bytes: 4096,
        max_result_bytes: 4096,
        max_total_bytes: 8192,
    }
}
fn identity() -> SourceModelAdapterIdentity {
    SourceModelAdapterIdentity {
        provider_id: "fake.local".into(),
        model_id: "fake-basic".into(),
        adapter_identity: "owned-wait-inert-test".into(),
        adapter_version: "1.0.0".into(),
        provider_profile: "fixture".into(),
    }
}
fn policy(model: &SourceModelBinding) -> SourceLivePolicy {
    SourceLivePolicy {
        deployment_binding: model.digest().into(),
        response_limit: model.max_response_bytes(),
        ceiling: 3,
        reservation_units: 1,
        unit: "owned_wait_unit".into(),
        clock_domain: "owned.wait.test".into(),
        initial_millis: 0,
        deadline_millis: 1000,
        max_total_steps: 2_000_000,
        program_root: None,
    }
}
fn context(
    runtime: &AgentRuntimeV2,
    wait: Arc<CheckedOwnedAgentWaitBindingV8>,
) -> CheckedTypedOwnedWaitExecutionV8 {
    let model = runtime.source_model_binding(identity()).unwrap();
    let policy = policy(&model);
    let mut factory = || -> Box<dyn ProviderAdapter> { panic!("pure preflight dispatched") };
    let source = StreamingSourceProposalAdapter::new_bound_checkpointed(
        &mut factory,
        AdapterInvocationCapability::grant("private pure context test"),
        runtime.proposal_schema(),
        model.clone(),
        model.invocation_capability(),
        SourceProposalPolicy {
            deployment_binding: model.digest(),
            response_limit: model.max_response_bytes(),
            reservation_units: 1,
        },
    )
    .unwrap();
    runtime
        .checked_owned_wait_execution_v8(wait, &source, &policy, 1000)
        .unwrap()
}
#[test]
fn owned_wait_typed_execution_binds_real_registry_task_and_each_effect_ceiling() {
    let f = fixture();
    with_authenticated_project(&f.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let source = project
            .sources()
            .iter()
            .find(|s| s.path() == "src/app.spx")
            .unwrap();
        let wait = Arc::new(compile_owned_agent_wait_v8(
            source.source(),
            std::path::Path::new(source.path()),
            "fixture.agent",
            "fixture.agent.type.step",
        )?);
        let baseline = runtime(Arc::clone(&project), effects(), false, b"owned task");
        let c = context(&baseline, Arc::clone(&wait));
        assert!(std::ptr::eq(
            baseline.owned_wait_effects_v8(&c).unwrap(),
            &baseline.lifecycle
        ));
        let actual_limits = baseline.owned_wait_effect_limits_v8(&c).unwrap();
        assert_eq!(
            (
                actual_limits.max_calls,
                actual_limits.max_argument_bytes,
                actual_limits.max_result_bytes,
                actual_limits.max_total_bytes
            ),
            (
                baseline.effects.max_calls,
                baseline.effects.max_argument_bytes,
                baseline.effects.max_result_bytes,
                baseline.effects.max_total_bytes
            )
        );
        assert_eq!(c.evaluation_fuel(), 1000);
        assert_eq!(c.wait().binding(), wait.binding());
        assert_eq!(
            c.revision().digest(),
            baseline.execution_revision().digest()
        );
        assert_eq!(c.project().project_revision(), project.project_revision());
        use crate::agent_lifecycle::iterative::effects::plan_owned_effect_v8;
        use crate::resumable_effects::owned_frame::v2::bind_owned_wait_proposal_v8;
        use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
        let invocation = crate::live_invocation::identity::digest(b"semaprax.live-invocation.source-id.v8\0",
            serde_json::to_string(&serde_json::json!({"execution":c.ordinary().invocation(),"owned_wait_binding":wait.binding()})).unwrap().as_bytes());
        let scope = SourceCheckpointScope::new(wait.lifecycle().source_revision(), invocation, 7).unwrap();
        let proposal = |sequence: usize| {
            let document = format!(r#"{{"schema":"semaprax.agent-proposal.v1","agent_id":"fixture.agent","proposal_schema_digest":"{}","value":{{"fields":{{"fixture.agent.type.proposal.budget":"3","fixture.agent.type.proposal.urgent":false,"fixture.agent.type.proposal.sequence":"{}"}}}}}}"#,
                wait.lifecycle().proposal_schema().schema().digest(), sequence);
            let decoded = wait.lifecycle().proposal_schema().decode(&document).unwrap();
            bind_owned_wait_proposal_v8(&wait, &scope, &decoded).unwrap()
        };
        let checked = proposal(1);
        let effect_plan = plan_owned_effect_v8(&baseline, &c, &scope, &checked).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(effect_plan.operation().operation_id(), "fixture.read.second");
        assert_eq!(effect_plan.operation().effect_id(), "read");
        assert_eq!(effect_plan.argument().type_id(), effect_plan.operation().argument_type());
        assert_eq!(effect_plan.argument().payload(), b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"query\",\"3\"]]}\n");
        let accepted = b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"9\"]]}\n";
        assert_eq!(effect_plan.accepted_result(accepted).unwrap(), accepted);
        assert!(effect_plan.accepted_result(b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"09\"]]}\n").is_none());
        assert!(effect_plan.accepted_result(b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"other\",\"9\"]]}\n").is_none());
        assert!(effect_plan.accepted_result(b"{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[]}\n").is_none());
        assert_eq!(effect_plan.limits().max_calls, baseline.effects.max_calls);
        assert_eq!(baseline.effects.max_result_bytes, 4096);
        assert_eq!(effect_plan.target_limits().max_result_bytes, 1024);
        assert_eq!(effect_plan.target_limits().max_fuel, effect_plan.target_limits().max_calls);
        assert_eq!(c.ordinary().invocation(), context(&baseline, Arc::clone(&wait)).ordinary().invocation(),
            "profile intersection leaves the committed typed execution unchanged");
        assert!(effect_plan.accepted_result(&vec![b'x'; 1025]).is_none());
        let lower = runtime(Arc::clone(&project), EffectBudget { max_result_bytes: 32, ..effects() }, false, b"owned task");
        let lower_context = context(&lower, Arc::clone(&wait));
        assert!(plan_owned_effect_v8(&lower, &lower_context, &scope, &checked).is_err(), "different E cannot reuse the old scoped Proposal");
        let lower_invocation = crate::live_invocation::identity::digest(b"semaprax.live-invocation.source-id.v8\0",
            serde_json::to_string(&serde_json::json!({"execution":lower_context.ordinary().invocation(),"owned_wait_binding":wait.binding()})).unwrap().as_bytes());
        let lower_scope = SourceCheckpointScope::new(wait.lifecycle().source_revision(), lower_invocation, 7).unwrap();
        let lower_decoded = wait.lifecycle().proposal_schema().decode(checked.canonical_proposal()).unwrap();
        let lower_proposal = bind_owned_wait_proposal_v8(&wait, &lower_scope, &lower_decoded).unwrap();
        let lower_plan = plan_owned_effect_v8(&lower, &lower_context, &lower_scope, &lower_proposal).unwrap();
        assert_eq!(lower_plan.target_limits().max_result_bytes, 32);
        assert!(lower_plan.accepted_result(accepted).is_none());
        let wrong_scope = SourceCheckpointScope::new(scope.program_root(), "other", 7).unwrap();
        assert!(plan_owned_effect_v8(&baseline, &c, &wrong_scope, &checked).is_err());
        assert!(plan_owned_effect_v8(&baseline, &c, &scope, &proposal(2)).is_err());
        let e = c.ordinary().invocation();
        for change in 0..6 {
            let mut limits = effects();
            match change {
                0 => limits.max_calls += 1,
                1 => limits.max_argument_bytes += 1,
                2 => limits.max_result_bytes += 1,
                3 => limits.max_total_bytes += 1,
                _ => {}
            }
            let other = runtime(
                Arc::clone(&project),
                limits,
                change == 4,
                if change == 5 {
                    b"other task"
                } else {
                    b"owned task"
                },
            );
            assert!(
                other.owned_wait_effects_v8(&c).is_err(),
                "foreign actual runtime dimension {change}"
            );
            assert!(
                other.owned_wait_effect_limits_v8(&c).is_err(),
                "foreign ceilings dimension {change}"
            );
            assert!(plan_owned_effect_v8(&other, &c, &scope, &checked).is_err(), "foreign planner dimension {change}");
            let other = context(&other, Arc::clone(&wait));
            assert_ne!(other.ordinary().invocation(), e, "dimension {change}");
            assert_ne!(
                other.model().digest(),
                c.model().digest(),
                "dimension {change}"
            );
        }
        Ok(())
    })
    .unwrap();
}
#[test]
fn owned_wait_typed_execution_refuses_cross_runtime_and_profile_before_factory() {
    let f = fixture();
    with_authenticated_project(&f.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let source = project
            .sources()
            .iter()
            .find(|s| s.path() == "src/app.spx")
            .unwrap();
        let wait = Arc::new(compile_owned_agent_wait_v8(
            source.source(),
            std::path::Path::new(source.path()),
            "fixture.agent",
            "fixture.agent.type.step",
        )?);
        let r = runtime(Arc::clone(&project), effects(), false, b"owned task");
        let other = runtime(Arc::clone(&project), effects(), false, b"other task");
        let model = r.source_model_binding(identity())?;
        let mut p = policy(&model);
        let mut factory = || -> Box<dyn ProviderAdapter> { panic!("refused preflight dispatched") };
        let adapter = StreamingSourceProposalAdapter::new_bound_checkpointed(
            &mut factory,
            AdapterInvocationCapability::grant("private refusal test"),
            r.proposal_schema(),
            model.clone(),
            model.invocation_capability(),
            SourceProposalPolicy {
                deployment_binding: model.digest(),
                response_limit: model.max_response_bytes(),
                reservation_units: 1,
            },
        )?;
        assert!(r
            .checked_owned_wait_execution_v8(Arc::clone(&wait), &adapter, &p, 1000)
            .is_ok());
        for fuel in [0, 1001] {
            assert!(r
                .checked_owned_wait_execution_v8(Arc::clone(&wait), &adapter, &p, fuel)
                .is_err());
        }
        assert!(other
            .checked_owned_wait_execution_v8(Arc::clone(&wait), &adapter, &p, 1000)
            .is_err());
        let changed_source = source
            .source()
            .replace("sequence <= 1usize", "sequence <= 0usize");
        assert_ne!(changed_source, source.source());
        let changed_wait = Arc::new(compile_owned_agent_wait_v8(
            &changed_source,
            std::path::Path::new(source.path()),
            "fixture.agent",
            "fixture.agent.type.step",
        )?);
        assert!(r
            .checked_owned_wait_execution_v8(changed_wait, &adapter, &p, 1000)
            .is_err());
        p.program_root = Some("forbidden".into());
        assert!(r
            .checked_owned_wait_execution_v8(Arc::clone(&wait), &adapter, &p, 1000)
            .is_err());
        p.program_root = None;
        p.reservation_units += 1;
        assert!(r
            .checked_owned_wait_execution_v8(Arc::clone(&wait), &adapter, &p, 1000)
            .is_err());
        assert!(adapter.model_evidence().attempts().is_empty());
        Ok(())
    })
    .unwrap();
}

#[cfg(unix)]
fn registered_context_store(
    root: &std::path::Path,
    label: &str,
    execution: &CheckedTypedOwnedWaitExecutionV8,
    change_limit: bool,
) -> (
    crate::resumable_effects::owned_frame::SourceOwnedWaitStoreRegistrationV8,
    crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8,
) {
    use crate::resumable_effects::owned_frame::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let path = root.join(label);
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    let ordinary = execution.ordinary();
    let invocation=crate::live_invocation::identity::digest(b"semaprax.live-invocation.source-id.v8\0",
        serde_json::to_string(&serde_json::json!({"execution":ordinary.invocation(),"owned_wait_binding":execution.wait().binding()})).unwrap().as_bytes());
    let facts = FreshSourceOwnedWaitFactsV8 {
        scope: crate::resumable_effects::source_checkpoint::SourceCheckpointScope::new(
            execution.wait().lifecycle().source_revision(),
            invocation,
            7,
        )
        .unwrap(),
        execution: ordinary.invocation().into(),
        binding: execution.wait().binding().into(),
        directory_identity: (metadata.dev(), metadata.ino()),
        limits: SourceOwnedWaitLimitsV8 {
            max_steps_per_stage: ordinary.max_steps_per_stage().unwrap(),
            max_total_steps: ordinary.max_total_steps().unwrap() as u64,
            max_stages: ordinary.max_stages() as usize,
            max_attempts: ordinary.max_attempts() as usize,
            response_limit: ordinary.response_limit() + usize::from(change_limit),
        },
    };
    fresh_source_owned_wait_v8(
        prepare_fresh_source_owned_wait_v8(
            std::fs::File::open(&path).unwrap(),
            facts,
            ExplicitStoreRegistrationGrant::for_trusted_host(true).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
#[cfg(unix)]
#[test]
fn owned_wait_typed_context_joins_actual_execution_and_complete_physical_registration() {
    use crate::live_invocation::source_journal::checked_owned_wait_journal_context_v8;
    let f = fixture();
    with_authenticated_project(&f.0.join("semaprax.toml"), |snapshot| {
        let project = snapshot.retain_revision();
        let source = project
            .sources()
            .iter()
            .find(|s| s.path() == "src/app.spx")
            .unwrap();
        let wait = Arc::new(compile_owned_agent_wait_v8(
            source.source(),
            std::path::Path::new(source.path()),
            "fixture.agent",
            "fixture.agent.type.step",
        )?);
        let baseline = runtime(Arc::clone(&project), effects(), false, b"owned task");
        let e = Arc::new(context(&baseline, Arc::clone(&wait)));
        let (registration, mut lease) = registered_context_store(&f.0, "journal", &e, false);
        // A context is data-only and does not bypass independent retention ACK.
        let checked =
            checked_owned_wait_journal_context_v8(Arc::clone(&e), &lease, &registration).unwrap();
        assert_eq!(checked.binding(), wait.binding());
        assert_eq!(checked.generation(), registration.generation());
        checked.validate_lease(&lease).unwrap();
        assert_eq!(
            checked.ordinary().invocation(),
            registration.expected_facts().scope.invocation_id()
        );
        assert_ne!(checked.ordinary().invocation(), e.ordinary().invocation());
        assert!(lease.append(b"no registration ACK\n").is_err());
        lease
            .authorize_fresh_start(
                registration
                    .acknowledge_retained_by_trusted_host(true)
                    .unwrap(),
            )
            .unwrap();
        checked.validate_lease(&lease).unwrap();
        let key = crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([73; 32]);
        let state = serde_json::json!({"declaration":"fixture.agent.type.state","fields":[
            {"identity":"fixture.agent.type.state.objective","value":{"kind":"bytes","hex":"00"}},
            {"identity":"fixture.agent.type.state.budget","value":{"tag":"i64","value":10}},
            {"identity":"fixture.agent.type.state.epoch","value":{"tag":"i64","value":0}}]});
        let document = checked.test_state_document(&key, state.clone());
        assert_eq!(
            checked.test_inventory_len(&lease, &key, &document).unwrap(),
            3
        );
        assert!(checked
            .test_inventory_len(
                &lease,
                &crate::resumable_effects::source_checkpoint::SourceCheckpointKey::new([74; 32]),
                &document
            )
            .is_err());
        let mut wrong_state = state;
        wrong_state["declaration"] = serde_json::json!("wrong.state");
        let wrong_document = checked.test_state_document(&key, wrong_state);
        assert!(checked
            .test_inventory_len(&lease, &key, &wrong_document)
            .is_err());
        let other = runtime(Arc::clone(&project), effects(), true, b"changed task");
        let other_e = Arc::new(context(&other, Arc::clone(&wait)));
        assert!(checked_owned_wait_journal_context_v8(other_e, &lease, &registration).is_err());
        let (other_registration, other_lease) =
            registered_context_store(&f.0, "other-journal", &e, false);
        assert!(
            checked_owned_wait_journal_context_v8(Arc::clone(&e), &lease, &other_registration)
                .is_err()
        );
        assert!(checked.validate_lease(&other_lease).is_err());
        assert!(checked
            .test_inventory_len(&other_lease, &key, &document)
            .is_err());
        let (wrong_limit, wrong_lease) = registered_context_store(&f.0, "wrong-limit", &e, true);
        assert!(
            checked_owned_wait_journal_context_v8(Arc::clone(&e), &wrong_lease, &wrong_limit)
                .is_err()
        );
        Ok(())
    })
    .unwrap();
}
