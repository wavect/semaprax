//! The runnable routed-agent example (`examples/routed-agent-project`): two
//! approved model profiles, an explicit pin, rules mode and an experimental
//! decision provider, through the real routed runtime path.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::runtime_routing::{
    ctx, features, handlers, host, limits, response, settling_factory, with_world_at, Created,
    MemStore, ScriptedRouter,
};
use semaprax::live_invocation::DurablePolicyRun;
use semaprax::model_routing::engine::{
    Confidentiality, ConfiguredProvider, DecisionInvoker, Destination, EnablementGate, Modality,
    ProviderMode, ProviderProfile, RoutePolicy, TaskFamily,
};
use semaprax::model_routing::runtime::{
    start_routed_task, ApprovedProfileSet, ProfileModel, ProfileSpec, RouteReason, RouteSource,
    RoutedSession, SessionPolicy, TurnRequest, TurnStatus, TurnVerdict,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/routed-agent-project")
}

/// The host's own reader for its routing configuration.
fn profiles(config: &serde_json::Value) -> Vec<ProfileSpec> {
    config["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| ProfileSpec {
            id: p["id"].as_str().unwrap().to_owned(),
            deployment_source: std::fs::read_to_string(
                root().join(p["deployment"].as_str().unwrap()),
            )
            .unwrap(),
            provider_policy: None,
            limits: limits(),
            model: ProfileModel {
                alias: p["alias"].as_str().unwrap().to_owned(),
                destination: Destination::Local,
                structured_output: true,
                tools: true,
                modalities: [Modality::Text].into(),
                max_context: p["max_context"].as_u64().unwrap(),
                est_cost_micros: p["est_cost_micros"].as_u64().unwrap(),
                est_latency_ms: p["est_latency_ms"].as_u64().unwrap(),
                strength_rank: p["strength_rank"].as_u64().unwrap() as u32,
            },
        })
        .collect()
}

#[test]
fn routed_agent_example_runs_rules_pin_and_experimental_provider_tasks() {
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("routing.json")).unwrap())
            .unwrap();
    with_world_at(&root(), |w| {
        let specs = profiles(&config);
        // The committed deployments are exactly the two profiles of the
        // project's own definition.
        assert_eq!(specs[0].deployment_source, w.fast);
        assert_eq!(specs[1].deployment_source, w.strong);
        let set = ApprovedProfileSet::approve(&w.semantic, specs, RoutePolicy::default()).unwrap();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let target = w.target(b"example task");
        let mut run = |f, provider: Option<&mut ConfiguredProvider<'_, ScriptedRouter>>| {
            let mut store = MemStore::default();
            start_routed_task(
                &set,
                &f,
                &ctx("example"),
                provider,
                &target,
                handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut store,
            )
            .unwrap()
        };

        // Rules mode: zero router calls.
        let rules = run(features(TaskFamily::LocalizedDebug), None);
        assert_eq!(rules.record.source(), RouteSource::Rules);
        assert_eq!(rules.record.profile(), "fast");

        // Explicit operator pin.
        let mut pinned = features(TaskFamily::LocalizedDebug);
        pinned.operator_pin = config["operator_pin"].as_str().map(str::to_owned);
        let pin = run(pinned, None);
        assert_eq!(pin.record.source(), RouteSource::Pin);
        assert_eq!(pin.record.profile(), "strong");

        // Experimental decision provider (deterministic fixture invoker).
        let provider_id = config["decision_provider"]["provider_id"].as_str().unwrap();
        assert_eq!(config["decision_provider"]["mode"], "experimental");
        let mut inv = ScriptedRouter {
            calls: 0,
            answer: Some(
                serde_json::json!({"choice": "strong", "scores": {"strong": 0.8}, "abstain": false}),
            ),
        };
        let mut provider = ConfiguredProvider {
            profile: ProviderProfile {
                provider_id: provider_id.into(),
                model_id: "fixture".into(),
                checkpoint: "v1".into(),
                ..ProviderProfile::default()
            },
            invoker: &mut inv,
            mode: ProviderMode::Explicit,
            gate: EnablementGate::not_evaluated("model-route/v1", provider_id),
        };
        let learned = run(features(TaskFamily::LocalizedDebug), Some(&mut provider));
        assert_eq!(learned.record.source(), RouteSource::Provider);
        assert_eq!(learned.record.profile(), "strong");
        assert_eq!(learned.record.router_calls(), 1);
        for r in [&rules, &pin, &learned] {
            assert_eq!(r.run, DurablePolicyRun::Settled(response(&w.schema, "1")));
        }
        assert_eq!(
            created.borrow().as_slice(),
            ["fake.local", "other.local", "other.local"]
        );

        // Two turns re-routed at the durable boundary.
        let mut journal = MemStore::default();
        let mut session = RoutedSession::open(
            &set,
            SessionPolicy {
                role_profiles: BTreeMap::from([
                    ("draft".to_owned(), BTreeSet::from(["fast".to_owned()])),
                    ("review".to_owned(), BTreeSet::from(["strong".to_owned()])),
                ]),
                specialists: Vec::new(),
                escalation: None,
                max_delegation_depth: 0,
                max_turns: 2,
                confidentiality: Confidentiality::Project,
            },
            "example.session",
            "sha256:instructions",
            "sha256:acceptance",
            4,
            None,
            &mut journal,
        )
        .unwrap();
        let mut statuses = Vec::new();
        for (role, complete) in [("draft", false), ("review", true)] {
            let outcome = session
                .run_turn::<dyn DecisionInvoker>(
                    &TurnRequest {
                        role: role.into(),
                        specialist: None,
                        next_stage_capabilities: Vec::new(),
                        features: features(TaskFamily::LocalizedDebug),
                        reservation: 2,
                    },
                    &ctx("example"),
                    None,
                    &target,
                    handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                    &mut MemStore::default(),
                    None,
                    &mut |_| TurnVerdict::Accepted {
                        committed_state: role.as_bytes().to_vec(),
                        progressed: true,
                        complete,
                        tool_results: Vec::new(),
                    },
                )
                .unwrap();
            statuses.push((outcome.profile, outcome.reason, outcome.status));
        }
        assert_eq!(
            statuses,
            [
                ("fast".into(), RouteReason::Initial, TurnStatus::Continue),
                ("strong".into(), RouteReason::Reroute, TurnStatus::Complete)
            ]
        );
    });
}
