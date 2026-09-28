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
        assert_eq!(c.evaluation_fuel(), 1000);
        assert_eq!(c.wait().binding(), wait.binding());
        assert_eq!(
            c.revision().digest(),
            baseline.execution_revision().digest()
        );
        assert_eq!(c.project().project_revision(), project.project_revision());
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
