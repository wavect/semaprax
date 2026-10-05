//! Runtime boundary tests: no provider means rules and no invoker call; a
//! provider's strings never become anything but a host-admitted id.

use super::super::call::CallMetadata;
use super::super::policy::RoutePolicy;
use super::super::provider::{
    ConfiguredProvider, DecisionCall, DecisionInvoker, EnablementGate, ProviderMode,
    ProviderProfile,
};
use super::super::request::{DecisionRequest, ProjectBinding};
use super::super::route::{
    Budget, Confidentiality, Destination, LatencyClass, ModelPlan, RouteRequest, TaskFamily,
    TaskFeatures,
};
use super::super::router::{DecisionSource, FallbackReason, RouteContext, RouteInputs};
use super::*;
use serde_json::{json, Value};

/// Counts calls and answers from a script; it has no transport of any kind.
struct Counting {
    calls: u32,
    answer: Value,
}

impl DecisionInvoker for Counting {
    fn evaluate(&mut self, request: &DecisionRequest) -> DecisionCall {
        self.calls += 1;
        assert!(request.validate().is_ok());
        DecisionCall::Answered {
            result: self.answer.clone(),
            elapsed_ms: 3,
            call: None::<CallMetadata>,
        }
    }
}

fn plan(id: &str, dest: Destination, cost: u64, rank: u32) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: dest,
        structured_output: true,
        tools: true,
        max_context: 100_000,
        est_cost_micros: cost,
        est_latency_ms: 500,
        strength_rank: rank,
        descriptor: Default::default(),
    }
}

/// Host-admitted candidates (stand-ins for pre-bound deployment selections).
fn admitted() -> Vec<ModelPlan> {
    vec![
        plan(
            "dep-remote",
            Destination::Remote {
                origin: "api.example".into(),
            },
            500,
            9,
        ),
        plan("dep-cheap", Destination::Local, 10, 1),
        plan("dep-mid", Destination::Local, 50, 4),
    ]
}

fn inputs(family: TaskFamily) -> RouteInputs {
    let features = TaskFeatures {
        task_family: family,
        estimated_context_tokens: 1_000,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let budget = Budget {
        max_cost_micros: 1_000,
        max_latency_ms: 10_000,
        max_router_calls: 1,
    };
    RouteInputs {
        request: RouteRequest::new(features, admitted(), budget).unwrap(),
        policy: RoutePolicy::default(),
    }
}

fn ctx() -> RouteContext {
    RouteContext {
        project: ProjectBinding {
            id: "proj".into(),
            worktree: "wt".into(),
            revision: "rev".into(),
        },
        lock_digest: "lock".into(),
        invocation_id: "inv-1".into(),
        lineage_id: "lin-1".into(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    }
}

fn explicit<'a>(inv: &'a mut Counting) -> ConfiguredProvider<'a, Counting> {
    ConfiguredProvider {
        profile: ProviderProfile {
            provider_id: "fixture-router".into(),
            model_id: "m".into(),
            checkpoint: "c1".into(),
            ..ProviderProfile::default()
        },
        invoker: inv,
        mode: ProviderMode::Explicit,
        gate: EnablementGate::not_evaluated("model-route/v1", "fixture-router"),
    }
}

#[test]
fn no_provider_is_rules_with_zero_router_calls() {
    let i = inputs(TaskFamily::LocalizedDebug);
    let r = recommend_static(&i, &ctx()).unwrap();
    assert_eq!(r.source(), DecisionSource::Rules);
    assert_eq!(r.router_calls(), 0);
    assert_eq!(r.decision().wire.version, 0);
    // Identical to the generic entry with no provider and to `decide`.
    let again = recommend::<dyn DecisionInvoker>(&i, &ctx(), None, None).unwrap();
    assert_eq!(r, again);
}

#[test]
fn attached_but_trivial_family_never_calls_the_invoker() {
    let mut inv = Counting {
        calls: 0,
        answer: json!({"choice": "dep-mid", "scores": {"dep-mid": 1.0}, "abstain": false}),
    };
    let i = inputs(TaskFamily::Mechanical);
    let r = recommend(&i, &ctx(), Some(&mut explicit(&mut inv)), None).unwrap();
    assert_eq!(r.source(), DecisionSource::Trivial);
    assert_eq!(inv.calls, 0);
}

#[test]
fn provider_choice_is_a_host_admitted_id_and_maps_back_by_reference() {
    let mut inv = Counting {
        calls: 0,
        answer: json!({"choice": "dep-mid", "scores": {"dep-mid": 0.9}, "abstain": false}),
    };
    let i = inputs(TaskFamily::LocalizedDebug);
    let r = recommend(&i, &ctx(), Some(&mut explicit(&mut inv)), None).unwrap();
    assert_eq!(inv.calls, 1);
    assert_eq!(r.source(), DecisionSource::Provider);
    let host = admitted();
    let picked = r.select(&host, |p| p.id.as_str()).unwrap();
    assert!(std::ptr::eq(picked, &host[2]));
    let order: Vec<&str> = r
        .ordered(&host, |p| p.id.as_str())
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(order[0], "dep-mid");
    assert!(order.iter().all(|id| host.iter().any(|p| p.id == *id)));
}

#[test]
fn provider_strings_cannot_mint_a_deployment_grant_or_transport() {
    let host = admitted();
    let cases = [
        // A choice that names something the host never admitted.
        (
            json!({"choice": "attacker-deployment", "scores": {"attacker-deployment": 1.0}, "abstain": false}),
            FallbackReason::RejectedChoice,
        ),
        // Extra authority-shaped members are refused by the closed result shape.
        (
            json!({"choice": "dep-remote", "scores": {"dep-remote": 1.0}, "abstain": false,
                   "grant": {"tools": ["shell"]}, "endpoint": "https://evil.example", "deployment": "dep-x"}),
            FallbackReason::InvalidResult,
        ),
    ];
    for (answer, why) in cases {
        let mut inv = Counting { calls: 0, answer };
        let i = inputs(TaskFamily::LocalizedDebug);
        let r = recommend(&i, &ctx(), Some(&mut explicit(&mut inv)), None).unwrap();
        assert_eq!(inv.calls, 1);
        assert_eq!(r.source(), DecisionSource::Fallback(why));
        // The fallback is the rules pick over the host's own candidates.
        assert_eq!(r.choice(), recommend_static(&i, &ctx()).unwrap().choice());
        assert!(r.select(&host, |p| p.id.as_str()).is_some());
        assert!(r
            .plan()
            .ordered
            .iter()
            .all(|s| host.iter().any(|p| p.id == s.model_id)));
        let text = format!("{:?}", r.decision());
        assert!(!text.contains("attacker") && !text.contains("evil.example"));
    }
}

#[test]
fn confidentiality_screen_holds_at_the_boundary() {
    // Secret work never reaches a remote candidate whatever a provider says.
    let mut inv = Counting {
        calls: 0,
        answer: json!({"choice": "dep-remote", "scores": {"dep-remote": 1.0}, "abstain": false}),
    };
    let mut i = inputs(TaskFamily::LocalizedDebug);
    i.request.features.confidentiality = Confidentiality::Secret;
    let r = recommend(&i, &ctx(), Some(&mut explicit(&mut inv)), None).unwrap();
    assert_ne!(r.choice(), "dep-remote");
    assert!(r.plan().ordered.iter().all(|s| s.model_id != "dep-remote"));
}
