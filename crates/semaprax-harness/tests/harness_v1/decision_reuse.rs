//! MR-12: the session-owned decision cache, readiness and reuse report
//! (`workflow::decision_reuse`) over fixture invokers, plus one cross-language
//! end-to-end cell against the bundled laya-local adapter. Fixture prefix `hp-mr12`.

use crate::decision_v2::{answer, call, catalog, ctx, inputs, inputs_with, label_pick, V2};
use semaprax_harness::contract::RequestEnvelope;
use semaprax_harness::decision::*;
use semaprax_harness::diag::HarnessResult;
use semaprax_harness::workflow::decision_reuse::{Readiness, Reuse, SessionDecisions};
use serde_json::json;

fn rc(mode: RoutingMode) -> RoutingConfig {
    RoutingConfig {
        mode,
        ..RoutingConfig::default()
    }
}

fn instance(id: &str, secrets: &[&str]) -> InstanceConfig {
    InstanceConfig {
        instance_id: id.into(),
        endpoint: None,
        secret_refs: secrets.iter().map(|s| s.to_string()).collect(),
    }
}

fn profile(model: &str, secrets: &[&str]) -> ProviderProfile {
    ProviderProfile::configured(
        AdapterIdentity {
            provider_id: crate::decision_v2::PID.into(),
            adapter_version: "0.1.0".into(),
        },
        crate::decision_v2::model_profile(json!({"model": model})),
        instance("inst", secrets),
    )
}

/// One governed decision through the session cache.
fn go(
    sd: &mut SessionDecisions,
    cfg: &RoutingConfig,
    i: &RouteInputs,
    c: &RouteContext,
    prof: &ProviderProfile,
    inv: &mut dyn DecisionInvoker,
    live: &dyn Fn() -> RouteInputs,
) -> HarnessResult<(GovernedRoute, Reuse)> {
    let spec = GateSpec::default();
    let g = Governor {
        cfg,
        registry: None,
        spec: &spec,
        lock: None,
        router_headroom_tokens: None,
        router_request_tokens: 0,
    };
    let mut p = ConfiguredProvider {
        profile: prof.clone(),
        invoker: inv,
        mode: ProviderMode::Explicit,
        gate: EnablementGate::not_evaluated("model-route/v2", &prof.provider_id),
    };
    sd.decide(&g, i, c, Some(&mut p), live)
}

#[test]
fn mr12_repeated_identical_decisions_make_one_router_inference_and_report_zero_new_usage() {
    let mut sd = SessionDecisions::default();
    let cfg = rc(RoutingMode::Experimental);
    let (i, c, p) = (inputs(RouteSignals::default()), ctx(), profile("fx-1", &[]));
    let mut inv = V2::by_label("economy");
    let (a, ra) = go(&mut sd, &cfg, &i, &c, &p, &mut inv, &|| i.clone()).unwrap();
    assert_eq!(
        (a.decision.source, ra.outcome, ra.router_calls),
        (DecisionSource::Provider, "miss", 1)
    );
    assert_eq!(
        (ra.readiness_before, ra.readiness_after),
        (Readiness::Cold, Readiness::Warm)
    );
    for _ in 0..3 {
        let (b, rb) = go(&mut sd, &cfg, &i, &c, &p, &mut inv, &|| i.clone()).unwrap();
        assert_eq!(
            (b.decision.source, rb.outcome, rb.router_calls),
            (DecisionSource::Cache, "hit", 0)
        );
        assert_eq!(b.decision.choice, a.decision.choice);
        assert!(
            b.decision.wire.call.is_none(),
            "no new call metadata, no new usage"
        );
        assert_eq!(rb.to_json()["new_api_usage"], false);
        assert_eq!(b.router_calls_total, 0);
    }
    assert_eq!(inv.seen.len(), 1, "at most one actual router inference");
    let s = sd.stats();
    assert_eq!(
        (s.decisions, s.hits, s.misses, s.router_calls),
        (4, 3, 1, 1)
    );
}

#[test]
fn mr12_changed_failure_role_policy_candidates_credentials_identity_or_tenant_never_reuse() {
    let mut sd = SessionDecisions::default();
    let cfg = rc(RoutingMode::Experimental);
    let base = inputs(RouteSignals::default());
    let (c, p) = (ctx(), profile("fx-1", &["SECRET_A"]));
    let mut inv = V2::by_label("economy");
    go(&mut sd, &cfg, &base, &c, &p, &mut inv, &|| base.clone()).unwrap();
    let mut variants: Vec<(&str, RouteInputs, RouteContext, ProviderProfile)> = vec![];
    let sig = |f: &dyn Fn(&mut RouteSignals)| {
        let mut s = RouteSignals::default();
        f(&mut s);
        inputs(s)
    };
    variants.push((
        "failure",
        sig(&|s| s.previous_failure = PreviousFailure::SemanticLaw),
        c.clone(),
        p.clone(),
    ));
    variants.push((
        "role",
        sig(&|s| s.phase = Phase::Review),
        c.clone(),
        p.clone(),
    ));
    let mut pol = base.clone();
    pol.policy.router_max_latency_ms += 1;
    variants.push(("policy", pol, c.clone(), p.clone()));
    let mut cat = catalog(["alpha", "beta", "gamma"]);
    cat[2].descriptor.quality_tier = Some(QualityTier::Economy);
    variants.push((
        "candidates",
        inputs_with(cat, RouteSignals::default()),
        c.clone(),
        p.clone(),
    ));
    variants.push((
        "credential scope",
        base.clone(),
        c.clone(),
        profile("fx-1", &["SECRET_B"]),
    ));
    variants.push((
        "model identity",
        base.clone(),
        c.clone(),
        profile("fx-2", &["SECRET_A"]),
    ));
    let mut tenant = c.clone();
    tenant.project.id = "other-project".into();
    variants.push(("tenant", base.clone(), tenant, p.clone()));
    for (n, (what, i, cx, pr)) in variants.iter().enumerate() {
        let (_, r) = go(&mut sd, &cfg, i, cx, pr, &mut inv, &|| i.clone()).unwrap();
        assert_eq!((r.outcome, r.router_calls), ("miss", 1), "{what}");
        assert_eq!(inv.seen.len(), n + 2, "{what}");
    }
    // The original request still hits.
    let (_, r) = go(&mut sd, &cfg, &base, &c, &p, &mut inv, &|| base.clone()).unwrap();
    assert_eq!(r.outcome, "hit");
}

#[test]
fn mr12_a_cached_choice_that_is_no_longer_admissible_is_screened_out_before_generation() {
    let mut sd = SessionDecisions::default();
    let cfg = rc(RoutingMode::Experimental);
    let (i, c, p) = (inputs(RouteSignals::default()), ctx(), profile("fx-1", &[]));
    let mut inv = V2::by_label("economy");
    let (a, _) = go(&mut sd, &cfg, &i, &c, &p, &mut inv, &|| i.clone()).unwrap();
    assert_eq!(a.decision.choice, "beta");
    // The cached destination was revoked between decisions: the hit is
    // revalidated against the live set and the forbidden model never comes back.
    let revoked = inputs_with(
        catalog(["alpha", "beta", "gamma"])
            .into_iter()
            .filter(|m| m.id != "beta")
            .collect(),
        RouteSignals::default(),
    );
    let (b, r) = go(&mut sd, &cfg, &i, &c, &p, &mut inv, &|| revoked.clone()).unwrap();
    assert_eq!((r.outcome, r.router_calls), ("hit", 0));
    assert_ne!(b.decision.choice, "beta");
    assert!(
        matches!(b.decision.source, DecisionSource::Fallback(_)),
        "{:?}",
        b.decision.source
    );
    assert!(revoked
        .request
        .catalog
        .iter()
        .any(|m| m.id == b.decision.choice));
    assert!(
        recheck_dispatch(&revoked, "beta", 10).is_err(),
        "and the dispatch recheck refuses it"
    );
}

struct Dead(u32, DecisionCall);
impl DecisionInvoker for Dead {
    fn evaluate(&mut self, _r: &RequestEnvelope) -> DecisionCall {
        self.0 += 1;
        match &self.1 {
            DecisionCall::Timeout => DecisionCall::Timeout,
            _ => DecisionCall::Unavailable,
        }
    }
    fn decision_versions(&self) -> Vec<u32> {
        vec![1, 2]
    }
}

/// Answers after a reported `ms` of inference.
struct Slow(u32, u64);
impl DecisionInvoker for Slow {
    fn evaluate(&mut self, r: &RequestEnvelope) -> DecisionCall {
        self.0 += 1;
        let pick = label_pick(&r.payload, "economy");
        let result = answer(
            &r.payload,
            Some(&pick),
            "option_distribution",
            call(&r.payload, "fx-1", "mutable_service"),
        );
        DecisionCall::Answered {
            call: CallMetadata::from_json(&result["call"]).ok(),
            result,
            elapsed_ms: self.1,
        }
    }
    fn decision_versions(&self) -> Vec<u32> {
        vec![1, 2]
    }
}

#[test]
fn mr12_dead_worker_deadline_and_refuse_policy_take_the_bounded_fallback_path() {
    let cfg = rc(RoutingMode::Experimental);
    let (i, c, p) = (inputs(RouteSignals::default()), ctx(), profile("fx-1", &[]));
    // A timed-out worker: one call, a rules fallback, nothing cached, and the
    // rest of the session falls back without calling it again.
    let mut sd = SessionDecisions::default();
    let mut dead = Dead(0, DecisionCall::Timeout);
    let (a, ra) = go(&mut sd, &cfg, &i, &c, &p, &mut dead, &|| i.clone()).unwrap();
    assert_eq!(
        a.decision.source,
        DecisionSource::Fallback(FallbackReason::Timeout)
    );
    assert_eq!(
        (ra.router_calls, ra.readiness_after),
        (1, Readiness::Unavailable)
    );
    assert!(sd.is_empty(), "an uncertain evaluation is never cached");
    let (b, rb) = go(&mut sd, &cfg, &i, &c, &p, &mut dead, &|| i.clone()).unwrap();
    assert_eq!((rb.outcome, rb.router_calls, dead.0), ("bypass", 0, 1));
    assert!(b.rules_reason.unwrap().contains("unavailable"));
    // Under `fallback = refuse` the bypass refuses instead of choosing.
    let mut strict = i.clone();
    strict.policy.fallback = FallbackMode::Refuse;
    let e = go(&mut sd, &cfg, &strict, &c, &p, &mut dead, &|| {
        strict.clone()
    })
    .unwrap_err();
    assert_eq!(e.code, "SPX-HPJ013");
    assert_eq!(dead.0, 1);

    // A router observed slower than the remaining deadline is bypassed.
    let mut sd = SessionDecisions::default();
    let mut slow = Slow(0, 3_000);
    let (_, r) = go(&mut sd, &cfg, &i, &c, &p, &mut slow, &|| i.clone()).unwrap();
    assert_eq!((r.outcome, r.inference_ms), ("miss", 3_000));
    let mut later = c.clone();
    later.router_ms_used = i.policy.router_max_latency_ms - 1_000;
    let other = inputs(RouteSignals {
        attempt_index: 1,
        ..RouteSignals::default()
    });
    let (d, r) = go(&mut sd, &cfg, &other, &later, &p, &mut slow, &|| {
        other.clone()
    })
    .unwrap();
    assert_eq!((r.outcome, slow.0), ("bypass", 1));
    assert!(r.reason.unwrap().contains("predicted to miss the deadline"));
    assert_eq!(d.decision.router_calls, 0);
    // Deadline exhausted: the engine's own latency guard, no call.
    let mut gone = c.clone();
    gone.router_ms_used = i.policy.router_max_latency_ms;
    let mut sd = SessionDecisions::default();
    let mut fresh = V2::by_label("economy");
    let (d, r) = go(&mut sd, &cfg, &i, &gone, &p, &mut fresh, &|| i.clone()).unwrap();
    assert_eq!(
        d.decision.source,
        DecisionSource::Fallback(FallbackReason::LatencyExhausted)
    );
    assert_eq!((r.router_calls, fresh.seen.len()), (0, 0));
}

#[test]
fn mr12_rules_only_and_one_candidate_paths_keep_zero_router_calls() {
    let (i, c, p) = (inputs(RouteSignals::default()), ctx(), profile("fx-1", &[]));
    let mut sd = SessionDecisions::default();
    let mut inv = V2::by_label("economy");
    let (d, r) = go(
        &mut sd,
        &rc(RoutingMode::Rules),
        &i,
        &c,
        &p,
        &mut inv,
        &|| i.clone(),
    )
    .unwrap();
    assert_eq!(
        (d.decision.source, r.outcome, r.router_calls),
        (DecisionSource::Rules, "none", 0)
    );
    let one = inputs_with(
        catalog(["alpha", "beta", "gamma"])[..1].to_vec(),
        RouteSignals::default(),
    );
    let (d, r) = go(
        &mut sd,
        &rc(RoutingMode::Experimental),
        &one,
        &c,
        &p,
        &mut inv,
        &|| one.clone(),
    )
    .unwrap();
    assert_eq!(
        (d.decision.source, r.router_calls),
        (DecisionSource::Trivial, 0)
    );
    assert!(inv.seen.is_empty());
    let pinned = RoutingConfig {
        project_pin: Some("gamma".into()),
        ..rc(RoutingMode::Experimental)
    };
    let (d, r) = go(&mut sd, &pinned, &i, &c, &p, &mut inv, &|| i.clone()).unwrap();
    assert_eq!((d.decision.choice.as_str(), r.router_calls), ("gamma", 0));
    assert!(inv.seen.is_empty());
}

#[path = "decision_reuse_e2e.rs"]
mod e2e;
