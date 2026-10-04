//! HP-10 decision-layer tests (fixture prefix `hp-hp10`).

use crate::support::{fixture_dir, write};
use semaprax_harness::cli::Environment;
use semaprax_harness::contract::{ProjectBinding, RequestEnvelope};
use semaprax_harness::decision::*;
use serde_json::{json, Value};
use std::cell::RefCell;

type Script = Box<dyn FnMut(&RequestEnvelope) -> DecisionCall>;

struct Fx {
    calls: u32,
    lineages: Vec<Vec<String>>,
    script: Script,
}

impl Fx {
    fn new(script: impl FnMut(&RequestEnvelope) -> DecisionCall + 'static) -> Self {
        Self {
            calls: 0,
            lineages: vec![],
            script: Box::new(script),
        }
    }
    fn picks(choice: &'static str, score: f64) -> Self {
        Self::new(move |_| answer(Some(choice), &[(choice, score)], 5))
    }
}

impl DecisionInvoker for Fx {
    fn evaluate(&mut self, r: &RequestEnvelope) -> DecisionCall {
        self.calls += 1;
        self.lineages.push(r.lineage.clone());
        (self.script)(r)
    }
}

fn answer(choice: Option<&str>, scores: &[(&str, f64)], ms: u64) -> DecisionCall {
    let s: serde_json::Map<String, Value> = scores
        .iter()
        .map(|(k, v)| (k.to_string(), json!(v)))
        .collect();
    DecisionCall::Answered {
        result: json!({"choice": choice, "scores": s, "abstain": choice.is_none()}),
        elapsed_ms: ms,
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
    }
}

fn catalog() -> Vec<ModelPlan> {
    vec![
        plan(
            "strong-remote",
            Destination::Remote {
                origin: "api.example".into(),
            },
            500,
            9,
        ),
        plan("cheap-local", Destination::Local, 10, 1),
        plan("mid-local", Destination::Local, 50, 4),
    ]
}

fn features(family: TaskFamily, conf: Confidentiality) -> TaskFeatures {
    TaskFeatures {
        task_family: family,
        estimated_context_tokens: 1000,
        requires_structured_output: false,
        requires_tools: false,
        confidentiality: conf,
        latency_class: LatencyClass::Interactive,
    }
}

fn budget() -> Budget {
    Budget {
        max_cost_micros: 10_000,
        max_latency_ms: 5_000,
        max_router_calls: 2,
    }
}

fn policy() -> RoutePolicy {
    RoutePolicy {
        remote_max_confidentiality: Some(Confidentiality::Project),
        allowed_origins: ["api.example".to_string()].into(),
        ..RoutePolicy::default()
    }
}

fn inputs(family: TaskFamily, conf: Confidentiality) -> RouteInputs {
    RouteInputs {
        request: RouteRequest::new(features(family, conf), catalog(), budget()).unwrap(),
        policy: policy(),
    }
}

fn ctx() -> RouteContext {
    RouteContext {
        project: ProjectBinding {
            id: "p".into(),
            worktree: "w".into(),
            revision: "r".into(),
        },
        lock_digest: "lock".into(),
        invocation_id: "inv-1".into(),
        lineage_id: "lin-1".into(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    }
}

fn profile(id: &str) -> ProviderProfile {
    ProviderProfile {
        provider_id: id.into(),
        model_id: format!("{id}-m"),
        checkpoint: "ck1".into(),
        min_confidence: None,
        max_context_tokens: None,
        supported_families: None,
    }
}

fn explicit<'a>(fx: &'a mut Fx, p: ProviderProfile) -> ConfiguredProvider<'a> {
    let gate = EnablementGate::not_evaluated("model-route/v1", &p.provider_id);
    ConfiguredProvider {
        profile: p,
        invoker: fx,
        mode: ProviderMode::Explicit,
        gate,
    }
}

fn run(i: &RouteInputs, p: Option<&mut ConfiguredProvider>) -> HarnessResult {
    decide(i, &ctx(), p, &|| i.clone(), None)
}

type HarnessResult = semaprax_harness::diag::HarnessResult<RouteDecision>;

#[test]
fn hp_hp10_registry_is_closed_and_reserved_tasks_refuse() {
    assert!(resolve("model-route/v1").is_ok());
    assert_eq!(resolve("tool-select/v1").unwrap_err().code, "SPX-HPJ002");
    assert_eq!(resolve("context-plan/v1").unwrap_err().code, "SPX-HPJ002");
    assert_eq!(resolve("invented/v1").unwrap_err().code, "SPX-HPJ001");
    let req = json!({"task": "tool-select/v1", "features": {}, "budget": {}});
    assert_eq!(
        RouteRequest::from_json(&req).unwrap_err().code,
        "SPX-HPJ002"
    );
}

#[test]
fn hp_hp10_rules_pick_cheap_for_simple_with_zero_router_calls() {
    let i = inputs(TaskFamily::Mechanical, Confidentiality::Project);
    let mut fx = Fx::picks("strong-remote", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let d = run(&i, Some(&mut p)).unwrap();
    assert_eq!(d.choice, "cheap-local");
    assert_eq!(d.source, DecisionSource::Trivial);
    assert_eq!(d.router_calls, 0);
    drop(p);
    assert_eq!(fx.calls, 0);
}

#[test]
fn hp_hp10_rules_pick_strongest_for_configured_hard_family() {
    let d = run(
        &inputs(TaskFamily::SemanticLaw, Confidentiality::Project),
        None,
    )
    .unwrap();
    assert_eq!(d.choice, "strong-remote");
    assert_eq!(d.provider_id, RULES_PROVIDER_ID);
    assert_eq!(d.router_calls, 0);
    // Hard-family set is configuration, not code.
    let mut i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    assert_eq!(run(&i, None).unwrap().choice, "cheap-local");
    i.policy.hard_families.insert(TaskFamily::LocalizedDebug);
    assert_eq!(run(&i, None).unwrap().choice, "strong-remote");
}

#[test]
fn hp_hp10_single_admissible_needs_no_router() {
    let i = inputs(TaskFamily::SemanticLaw, Confidentiality::Secret); // remote excluded
    let mut fx = Fx::picks("mid-local", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let mut one = i.clone();
    one.request = RouteRequest::new(
        i.request.features.clone(),
        vec![plan("only", Destination::Local, 1, 1)],
        budget(),
    )
    .unwrap();
    let d = run(&one, Some(&mut p)).unwrap();
    assert_eq!(
        (d.choice.as_str(), d.source, d.router_calls),
        ("only", DecisionSource::Trivial, 0)
    );
    drop(p);
    assert_eq!(fx.calls, 0);
}

#[test]
fn hp_hp10_hard_policy_screens_before_any_provider() {
    let mut f = features(TaskFamily::LocalizedDebug, Confidentiality::Secret);
    let s = screen(
        &RouteRequest::new(f.clone(), catalog(), budget()).unwrap(),
        &policy(),
    );
    assert!(s.excluded.iter().any(|(id, _)| id == "strong-remote"));
    f.confidentiality = Confidentiality::Project;
    f.requires_tools = true;
    let mut cat = catalog();
    cat[1].tools = false; // cheap-local
    cat[2].max_context = 10; // mid-local too small
    let s = screen(
        &RouteRequest::new(f.clone(), cat, budget()).unwrap(),
        &policy(),
    );
    let ids: Vec<_> = s.admissible.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["strong-remote"]);
    // unaffordable and over latency
    let tight = Budget {
        max_cost_micros: 20,
        max_latency_ms: 400,
        max_router_calls: 1,
    };
    let s = screen(
        &RouteRequest::new(
            features(TaskFamily::Mechanical, Confidentiality::Public),
            catalog(),
            tight,
        )
        .unwrap(),
        &policy(),
    );
    assert!(s.admissible.is_empty());
    let i = RouteInputs {
        request: RouteRequest::new(
            features(TaskFamily::Mechanical, Confidentiality::Public),
            catalog(),
            Budget {
                max_cost_micros: 1,
                ..budget()
            },
        )
        .unwrap(),
        policy: policy(),
    };
    assert_eq!(run(&i, None).unwrap_err().code, "SPX-HPJ005");
    // remote origin not approved
    let mut np = policy();
    np.allowed_origins.clear();
    let s = screen(
        &RouteRequest::new(
            features(TaskFamily::Mechanical, Confidentiality::Public),
            catalog(),
            budget(),
        )
        .unwrap(),
        &np,
    );
    assert!(s.excluded.iter().any(|(id, _)| id == "strong-remote"));
}

#[test]
fn hp_hp10_provider_choice_is_used_when_valid_and_counted() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let mut fx = Fx::picks("mid-local", 0.8);
    let mut p = explicit(&mut fx, profile("learned"));
    let d = run(&i, Some(&mut p)).unwrap();
    assert_eq!(
        (d.choice.as_str(), d.source, d.router_calls),
        ("mid-local", DecisionSource::Provider, 1)
    );
    assert_eq!(d.plan.ordered[0].model_id, "mid-local");
    assert_eq!(d.provider_status, "experimental");
    drop(p);
    assert_eq!(fx.lineages[0], ["learned"]);
}

#[test]
fn hp_hp10_nonexistent_or_disallowed_choice_is_rejected_before_dispatch() {
    for bad in ["ghost-model", "strong-remote"] {
        // secret task: strong-remote exists in the catalog but is forbidden
        let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Secret);
        let mut fx = Fx::new(move |_| answer(Some(bad), &[], 5));
        let mut p = explicit(&mut fx, profile("learned"));
        let d = run(&i, Some(&mut p)).unwrap();
        assert_eq!(
            d.source,
            DecisionSource::Fallback(FallbackReason::RejectedChoice),
            "{bad}"
        );
        assert!(d
            .plan
            .ordered
            .iter()
            .all(|s| s.model_id != bad && s.destination == Destination::Local));
        assert_eq!(d.choice, "cheap-local");
    }
}

#[test]
fn hp_hp10_high_confidence_never_grants_a_disallowed_option() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Secret);
    let mut fx = Fx::picks("strong-remote", 0.99);
    let mut p = explicit(&mut fx, profile("learned"));
    p.profile.min_confidence = Some(0.5);
    let d = run(&i, Some(&mut p)).unwrap();
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::RejectedChoice)
    );
    assert_ne!(d.choice, "strong-remote");
}

#[test]
fn hp_hp10_stale_catalog_or_policy_is_rejected_after_inference() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    // catalog drops the chosen model while the router was running
    let mut live_i = i.clone();
    live_i.request = RouteRequest::new(
        i.request.features.clone(),
        catalog()
            .into_iter()
            .filter(|m| m.id != "mid-local")
            .collect(),
        budget(),
    )
    .unwrap();
    let mut fx = Fx::picks("mid-local", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let d = decide(&i, &ctx(), Some(&mut p), &|| live_i.clone(), None).unwrap();
    assert_eq!(d.source, DecisionSource::Fallback(FallbackReason::Stale));
    assert_eq!(d.choice, "cheap-local");
    assert!(d.plan.ordered.iter().all(|s| s.model_id != "mid-local"));
    // policy now forbids remote while the router chose it
    let mut live_p = i.clone();
    live_p.policy.allowed_origins.clear();
    let mut fx = Fx::picks("strong-remote", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let d = decide(&i, &ctx(), Some(&mut p), &|| live_p.clone(), None).unwrap();
    assert_eq!(d.source, DecisionSource::Fallback(FallbackReason::Stale));
    assert!(d.plan.ordered.iter().all(|s| s.model_id != "strong-remote"));
}

#[test]
fn hp_hp10_router_failures_fall_back_deterministically_within_budget() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let rules = run(&i, None).unwrap();
    let cases: Vec<(&str, Script, FallbackReason)> = vec![
        (
            "timeout",
            Box::new(|_| DecisionCall::Timeout),
            FallbackReason::Timeout,
        ),
        (
            "unavailable",
            Box::new(|_| DecisionCall::Unavailable),
            FallbackReason::Unavailable,
        ),
        (
            "abstain",
            Box::new(|_| answer(None, &[], 5)),
            FallbackReason::Abstain,
        ),
        (
            "slow",
            Box::new(|_| answer(Some("mid-local"), &[("mid-local", 0.9)], 99_999)),
            FallbackReason::Timeout,
        ),
        (
            "garbage",
            Box::new(|_| DecisionCall::Answered {
                result: json!({"choice": 7}),
                elapsed_ms: 1,
            }),
            FallbackReason::InvalidResult,
        ),
    ];
    for (name, script, reason) in cases {
        let mut fx = Fx::new(script);
        let mut p = explicit(&mut fx, profile("learned"));
        let d = run(&i, Some(&mut p)).unwrap();
        assert_eq!(d.source, DecisionSource::Fallback(reason), "{name}");
        assert_eq!(d.choice, rules.choice, "{name}");
        assert!(d.router_calls <= 2, "{name}");
    }
    // call cap exhausted: no invocation at all
    let mut c = ctx();
    c.router_calls_used = 2;
    let mut fx = Fx::picks("mid-local", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let d = decide(&i, &c, Some(&mut p), &|| i.clone(), None).unwrap();
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::CallCapExhausted)
    );
    // latency ceiling exhausted
    let mut c = ctx();
    c.router_ms_used = i.policy.router_max_latency_ms;
    let d = decide(&i, &c, Some(&mut p), &|| i.clone(), None).unwrap();
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::LatencyExhausted)
    );
    drop(p);
    assert_eq!(fx.calls, 0);
}

#[test]
fn hp_hp10_refuse_mode_and_no_safe_model_refuse() {
    let mut i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    i.policy.fallback = FallbackMode::Refuse;
    let mut fx = Fx::new(|_| DecisionCall::Timeout);
    let mut p = explicit(&mut fx, profile("learned"));
    assert_eq!(run(&i, Some(&mut p)).unwrap_err().code, "SPX-HPJ013");
    // after-inference the live catalog is empty of safe models
    let mut empty = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    empty.request = RouteRequest::new(empty.request.features.clone(), vec![], budget()).unwrap();
    let j = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let mut fx = Fx::new(|_| answer(Some("mid-local"), &[], 1));
    let mut p = explicit(&mut fx, profile("learned"));
    let err = decide(&j, &ctx(), Some(&mut p), &|| empty.clone(), None).unwrap_err();
    assert_eq!(err.code, "SPX-HPJ005");
}

#[test]
fn hp_hp10_ood_and_low_validated_confidence_use_profile_not_universal_threshold() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let mut fx = Fx::picks("mid-local", 0.4);
    let mut p = explicit(&mut fx, profile("learned"));
    p.profile.max_context_tokens = Some(500); // request has 1000
    assert_eq!(
        run(&i, Some(&mut p)).unwrap().source,
        DecisionSource::Fallback(FallbackReason::OutOfDistribution)
    );
    p.profile.max_context_tokens = None;
    p.profile.supported_families = Some([TaskFamily::SemanticLaw].into());
    assert_eq!(
        run(&i, Some(&mut p)).unwrap().source,
        DecisionSource::Fallback(FallbackReason::OutOfDistribution)
    );
    p.profile.supported_families = None;
    p.profile.min_confidence = Some(0.7);
    assert_eq!(
        run(&i, Some(&mut p)).unwrap().source,
        DecisionSource::Fallback(FallbackReason::LowConfidence)
    );
    p.profile.min_confidence = Some(0.3); // another adapter calibrates differently
    assert_eq!(
        run(&i, Some(&mut p)).unwrap().source,
        DecisionSource::Provider
    );
    p.profile.min_confidence = None; // scores ignored entirely
    assert_eq!(
        run(&i, Some(&mut p)).unwrap().source,
        DecisionSource::Provider
    );
    drop(p);
    assert_eq!(fx.calls, 3); // the two OOD cases never invoked the router
}

#[test]
fn hp_hp10_two_providers_switch_by_config_without_changing_plan_types() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let mut a = Fx::picks("mid-local", 0.9);
    let mut b = Fx::picks("strong-remote", 0.9);
    let select = |name: &str, a: &mut Fx, b: &mut Fx| -> RouteDecision {
        let (fx, id): (&mut dyn DecisionInvoker, _) = if name == "provider-a" {
            (a, "provider-a")
        } else {
            (b, "provider-b")
        };
        let p = profile(id);
        let gate = EnablementGate::not_evaluated("model-route/v1", id);
        let mut cp = ConfiguredProvider {
            profile: p,
            invoker: fx,
            mode: ProviderMode::Explicit,
            gate,
        };
        run(&i, Some(&mut cp)).unwrap()
    };
    let da: FrozenRoutePlan = select("provider-a", &mut a, &mut b).plan;
    let db: FrozenRoutePlan = select("provider-b", &mut a, &mut b).plan;
    assert_eq!((a.calls, b.calls), (1, 1));
    assert_eq!(da.ordered[0].model_id, "mid-local");
    assert_eq!(db.ordered[0].model_id, "strong-remote");
    let mut ia: Vec<_> = da.to_provider_slots().into_iter().map(|s| s.0).collect();
    let mut ib: Vec<_> = db.to_provider_slots().into_iter().map(|s| s.0).collect();
    ia.sort();
    ib.sort();
    assert_eq!(ia, ib); // same admitted set, only order differs
}

#[test]
fn hp_hp10_frozen_plan_order_and_provider_slots() {
    let d = run(
        &inputs(TaskFamily::LocalizedDebug, Confidentiality::Project),
        None,
    )
    .unwrap();
    // chosen first, then stronger ascending, then weaker
    let ids: Vec<_> = d.plan.ordered.iter().map(|s| s.model_id.as_str()).collect();
    assert_eq!(ids, ["cheap-local", "mid-local", "strong-remote"]);
    assert_eq!(
        d.plan.to_provider_slots(),
        vec![
            ("cheap-local".to_string(), true),
            ("mid-local".to_string(), true),
            ("strong-remote".to_string(), true)
        ]
    );
    assert_eq!(
        FrozenRoutePlan::from_json(&d.plan.to_json()).unwrap(),
        d.plan
    );
    assert!(
        d.plan.decision_digest.starts_with("sha256:")
            && d.plan.policy_digest.starts_with("sha256:")
    );
}

#[test]
fn hp_hp10_confidential_plan_is_never_reshuffled_or_exposed_mid_retry() {
    let d = run(
        &inputs(TaskFamily::SemanticLaw, Confidentiality::Secret),
        None,
    )
    .unwrap();
    assert!(d
        .plan
        .ordered
        .iter()
        .all(|s| s.destination == Destination::Local));
    let mut l = AttemptLedger::new(d.plan.clone(), RoutePolicy::default().lineage_budgets);
    l.begin(AttemptKind::InitialRoute, "a0").unwrap();
    for n in 0..2 {
        let g = l
            .begin(AttemptKind::TransportRetry, &format!("t{n}"))
            .unwrap();
        assert_eq!(g.slot_index, 0);
        assert_eq!(g.destination, Destination::Local);
    }
    assert_eq!(l.plan(), &d.plan); // the frozen plan never changed
    assert_eq!(
        l.begin(AttemptKind::TransportRetry, "t9").unwrap_err().code,
        "SPX-HPJ009"
    );
}

#[test]
fn hp_hp10_attempt_kinds_have_separate_budgets_and_duplicates_are_idempotent() {
    let d = run(
        &inputs(TaskFamily::LocalizedDebug, Confidentiality::Project),
        None,
    )
    .unwrap();
    let mut l = AttemptLedger::new(
        d.plan,
        LineageBudgets {
            initial_route: 1,
            reasoning_escalation: 1,
            transport_retry: 1,
        },
    );
    assert_eq!(
        l.begin(AttemptKind::TransportRetry, "x").unwrap_err().code,
        "SPX-HPJ009"
    ); // before initial
    let g0 = l.begin(AttemptKind::InitialRoute, "a0").unwrap();
    assert_eq!(g0.model_id, "cheap-local");
    assert_eq!(
        l.begin(AttemptKind::InitialRoute, "a1").unwrap_err().code,
        "SPX-HPJ009"
    );
    let t = l.begin(AttemptKind::TransportRetry, "t1").unwrap();
    assert_eq!(t.slot_index, 0);
    let dup = l.begin(AttemptKind::TransportRetry, "t1").unwrap();
    assert!(dup.duplicate && dup.slot_index == 0);
    assert_eq!(
        (l.used(AttemptKind::TransportRetry), l.duplicates()),
        (1, 1)
    );
    assert_eq!(
        l.begin(AttemptKind::InitialRoute, "t1").unwrap_err().code,
        "SPX-HPJ010"
    );
    assert_eq!(
        l.begin(AttemptKind::TransportRetry, "t2").unwrap_err().code,
        "SPX-HPJ009"
    );
    let e = l.begin(AttemptKind::ReasoningEscalation, "e1").unwrap(); // own budget still free
    assert_eq!((e.slot_index, e.model_id.as_str()), (1, "mid-local"));
    assert_eq!(
        l.begin(AttemptKind::ReasoningEscalation, "e2")
            .unwrap_err()
            .code,
        "SPX-HPJ009"
    );
}

#[test]
fn hp_hp10_unauthorized_slot_is_never_granted() {
    let mut plan = run(
        &inputs(TaskFamily::LocalizedDebug, Confidentiality::Project),
        None,
    )
    .unwrap()
    .plan;
    plan.ordered[1].authorized = false;
    let mut l = AttemptLedger::new(
        plan,
        LineageBudgets {
            initial_route: 1,
            reasoning_escalation: 3,
            transport_retry: 1,
        },
    );
    l.begin(AttemptKind::InitialRoute, "a").unwrap();
    assert_eq!(
        l.begin(AttemptKind::ReasoningEscalation, "e")
            .unwrap_err()
            .code,
        "SPX-HPJ009"
    );
}

#[test]
fn hp_hp10_record_replay_is_deterministic_and_refuses_changed_inputs() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let mut fx = Fx::picks("mid-local", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let d = run(&i, Some(&mut p)).unwrap();
    let rec = DecisionRecord::from_decision(&d);
    let rec = DecisionRecord::from_json(&rec.to_json()).unwrap();
    let again = replay(&rec, &i).unwrap();
    assert_eq!(again, d.plan);
    assert_eq!(replay(&rec, &i).unwrap().digest(), d.plan.digest());
    drop(p);
    assert_eq!(fx.calls, 1); // replay made no router call
    let mut changed = i.clone();
    changed.request = RouteRequest::new(
        i.request.features.clone(),
        catalog()
            .into_iter()
            .filter(|m| m.id != "strong-remote")
            .collect(),
        budget(),
    )
    .unwrap();
    assert_eq!(replay(&rec, &changed).unwrap_err().code, "SPX-HPJ007");
    let mut np = i.clone();
    np.policy.fallback = FallbackMode::Refuse;
    assert_eq!(replay(&rec, &np).unwrap_err().code, "SPX-HPJ007");
    let mut tampered = rec.clone();
    tampered.plan.ordered.reverse();
    assert_eq!(replay(&tampered, &i).unwrap_err().code, "SPX-HPJ007");
}

#[test]
fn hp_hp10_catalog_order_does_not_change_digests() {
    let f = features(TaskFamily::Mechanical, Confidentiality::Public);
    let a = RouteRequest::new(f.clone(), catalog(), budget()).unwrap();
    let mut rev = catalog();
    rev.reverse();
    let b = RouteRequest::new(f, rev, budget()).unwrap();
    assert_eq!(a.catalog_digest(), b.catalog_digest());
}

#[test]
fn hp_hp10_gate_controls_automatic_selection_and_status() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let task = "model-route/v1";
    for (status, expect_call, label) in [
        (
            GateStatus::NotEvaluated,
            false,
            "rules (learned provider not evaluated)",
        ),
        (
            GateStatus::Failed,
            false,
            "rules (learned provider not evaluated)",
        ),
        (
            GateStatus::Passed {
                evidence: String::new(),
            },
            false,
            "rules (learned provider not evaluated)",
        ),
        (
            GateStatus::Passed {
                evidence: "eval-record-7".into(),
            },
            true,
            "evaluated",
        ),
    ] {
        let mut fx = Fx::picks("mid-local", 0.9);
        let mut p = ConfiguredProvider {
            profile: profile("learned"),
            invoker: &mut fx,
            mode: ProviderMode::Auto,
            gate: EnablementGate {
                task: task.into(),
                profile: "learned".into(),
                status,
            },
        };
        let d = run(&i, Some(&mut p)).unwrap();
        assert_eq!(d.provider_status, label);
        assert_eq!(d.source == DecisionSource::Provider, expect_call);
        drop(p);
        assert_eq!(fx.calls, u32::from(expect_call));
    }
    // a gate for another profile does not enable this one
    let mut fx = Fx::picks("mid-local", 0.9);
    let gate = EnablementGate {
        task: task.into(),
        profile: "other".into(),
        status: GateStatus::Passed {
            evidence: "e".into(),
        },
    };
    let mut p = ConfiguredProvider {
        profile: profile("learned"),
        invoker: &mut fx,
        mode: ProviderMode::Auto,
        gate,
    };
    assert_eq!(run(&i, Some(&mut p)).unwrap().source, DecisionSource::Rules);
}

#[test]
fn hp_hp10_router_cannot_route_itself() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let mut fx = Fx::picks("mid-local", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let mut c = ctx();
    c.router_lineage = vec!["learned".into()];
    let d = decide(&i, &c, Some(&mut p), &|| i.clone(), None).unwrap();
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::RecursionBlocked)
    );
    drop(p);
    assert_eq!(fx.calls, 0);
}

#[test]
fn hp_hp10_cache_is_bounded_keyed_and_revalidated() {
    let i = inputs(TaskFamily::LocalizedDebug, Confidentiality::Project);
    let cache = RefCell::new(DecisionCache::new(1));
    let mut fx = Fx::picks("mid-local", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    let go = |p: &mut ConfiguredProvider, i: &RouteInputs| {
        decide(
            i,
            &ctx(),
            Some(p),
            &|| i.clone(),
            Some(&mut cache.borrow_mut()),
        )
        .unwrap()
    };
    assert_eq!(go(&mut p, &i).source, DecisionSource::Provider);
    assert_eq!(go(&mut p, &i).source, DecisionSource::Cache);
    assert_eq!(cache.borrow().hits(), 1);
    // different catalog digest is a different key
    let mut other = i.clone();
    other.request = RouteRequest::new(
        i.request.features.clone(),
        catalog()
            .into_iter()
            .filter(|m| m.id != "strong-remote")
            .collect(),
        budget(),
    )
    .unwrap();
    assert_eq!(go(&mut p, &other).source, DecisionSource::Provider);
    assert_eq!(cache.borrow().len(), 1); // cap 1: the first entry was evicted
    assert_eq!(go(&mut p, &i).source, DecisionSource::Provider);
    drop(p);
    assert_eq!(fx.calls, 3);
    // a cached choice that is no longer admissible is still rejected
    let mut c2 = DecisionCache::new(4);
    let mut fx = Fx::picks("mid-local", 0.9);
    let mut p = explicit(&mut fx, profile("learned"));
    decide(&i, &ctx(), Some(&mut p), &|| i.clone(), Some(&mut c2)).unwrap();
    let mut live = i.clone();
    live.policy.fallback = FallbackMode::Rules;
    live.request = RouteRequest::new(
        i.request.features.clone(),
        catalog()
            .into_iter()
            .filter(|m| m.id != "mid-local")
            .collect(),
        budget(),
    )
    .unwrap();
    let d = decide(&i, &ctx(), Some(&mut p), &|| live.clone(), Some(&mut c2)).unwrap();
    assert_eq!(d.source, DecisionSource::Fallback(FallbackReason::Stale));
}

#[test]
fn hp_hp10_policy_and_request_parse_strictly() {
    let p = RoutePolicy::from_json(&json!({"remote_max_confidentiality": "project", "allowed_origins": ["o"], "hard_families": ["localized_debug"], "fallback": "refuse"})).unwrap();
    assert_eq!(p.fallback, FallbackMode::Refuse);
    assert_eq!(RoutePolicy::from_json(&p.to_json()).unwrap(), p);
    assert_eq!(
        RoutePolicy::from_json(&json!({"bogus": 1}))
            .unwrap_err()
            .code,
        "SPX-HPJ004"
    );
    let bad =
        json!({"task": "model-route/v1", "features": {"task_family": "poetry"}, "budget": {}});
    assert_eq!(
        RouteRequest::from_json(&bad).unwrap_err().code,
        "SPX-HPJ003"
    );
}

#[test]
fn hp_hp10_cli_decide_runs_rules_and_prints_plan() {
    let dir = fixture_dir("hp-hp10");
    let task = json!({
        "task": "model-route/v1",
        "features": features(TaskFamily::SemanticLaw, Confidentiality::Project).to_json(),
        "budget": budget().to_json(),
        "policy": policy().to_json(),
    });
    write(&dir, "task.json", &task.to_string());
    let cat: Vec<Value> = catalog().iter().map(ModelPlan::to_json).collect();
    write(&dir, "catalog.json", &Value::Array(cat).to_string());
    let env = Environment {
        cwd: dir.clone(),
        ..Environment::default()
    };
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let out = cli_decide(
        &args(&["task.json", "--catalog", "catalog.json", "--json"]),
        &env,
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let v: Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["choice"], "strong-remote");
    assert_eq!(v["router_calls"], 0);
    assert_eq!(v["plan"]["ordered"].as_array().unwrap().len(), 3);
    let again = cli_decide(
        &args(&["task.json", "--catalog", "catalog.json", "--json"]),
        &env,
    );
    assert_eq!(out.stdout, again.stdout);
    let text = cli_decide(&args(&["task.json", "--catalog", "catalog.json"]), &env);
    assert!(text.stdout.contains("choice: strong-remote"));
    // no catalog: empty catalog refuses with no safe model
    let none = cli_decide(&args(&["task.json"]), &env);
    assert_eq!((none.code, none.stderr.contains("SPX-HPJ005")), (1, true));
    assert_eq!(cli_decide(&[], &env).code, 2);
    assert_eq!(cli_decide(&args(&["missing.json"]), &env).code, 1);
}
