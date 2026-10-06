//! MR-01 / MR-15 `model-route/v2` routing tests: the typed feature
//! projection, the host renderer, negotiation, disclosure, cache/evidence
//! binding and capability preflight (fixture-invoker based; the out-of-tree
//! adapter lives in `decision_adapter`). Shared helpers are reused by
//! `decision_v2_scores` (MR-02 / MR-03).

use semaprax_harness::contract::{ProjectBinding, RequestEnvelope};
use semaprax_harness::decision::*;
use serde_json::{json, Value};

pub(crate) type Answer = Box<dyn FnMut(&Value) -> Value>;

/// A fixture decision adapter: negotiated versions, every payload it saw, and
/// a scripted answer computed from the request payload.
pub(crate) struct V2 {
    pub versions: Vec<u32>,
    pub seen: Vec<Value>,
    pub answer: Answer,
}

impl V2 {
    pub fn new(answer: impl FnMut(&Value) -> Value + 'static) -> Self {
        Self {
            versions: vec![1, 2],
            seen: vec![],
            answer: Box::new(answer),
        }
    }

    /// Picks the option whose host label contains `needle`.
    pub fn by_label(needle: &'static str) -> Self {
        Self::new(move |req| {
            let pick = label_pick(req, needle);
            answer(
                req,
                Some(&pick),
                "option_distribution",
                call(req, "fx-1", "mutable_service"),
            )
        })
    }
}

impl DecisionInvoker for V2 {
    fn evaluate(&mut self, r: &RequestEnvelope) -> DecisionCall {
        self.seen.push(r.payload.clone());
        let result = (self.answer)(&r.payload);
        DecisionCall::Answered {
            call: result
                .get("call")
                .and_then(|c| CallMetadata::from_json(c).ok()),
            result,
            elapsed_ms: 1,
        }
    }

    fn decision_versions(&self) -> Vec<u32> {
        self.versions.clone()
    }
}

pub(crate) fn label_pick(req: &Value, needle: &str) -> String {
    let labels = req["rendered"]["option_labels"].as_object().unwrap();
    labels
        .iter()
        .find(|(_, l)| l.as_str().unwrap().contains(needle))
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "m0".into())
}

pub(crate) fn call(req: &Value, answering: &str, kind: &str) -> Value {
    json!({"adapter": "org.example/fixture@0.1.0", "requested_model": "fx", "answering_model": answering,
           "checkpoint": null, "identity_kind": kind, "rendered_digest": req["rendered"]["digest"],
           "wire_bytes": 100,
           "usage": {"input_tokens": 50, "output_tokens": null, "basis": "provider_reported"},
           "billing": "api"})
}

/// A v2 result choosing `choice` with mass 0.9 (or null scores for `none`).
pub(crate) fn answer(req: &Value, choice: Option<&str>, kind: &str, call: Value) -> Value {
    let opts: Vec<String> = req["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o.as_str().unwrap().to_string())
        .collect();
    let scores = if kind == "none" {
        Value::Null
    } else {
        let rest = 0.1 / (opts.len().max(2) - 1) as f64;
        let m: serde_json::Map<String, Value> = opts
            .iter()
            .map(|o| {
                let v = if Some(o.as_str()) == choice.or(Some("m0")) {
                    0.9
                } else {
                    rest
                };
                (o.clone(), json!(v))
            })
            .collect();
        Value::Object(m)
    };
    json!({"choice": choice, "abstain": choice.is_none(),
           "abstention_reason": if choice.is_none() { "native" } else { "none" },
           "scores": scores, "score_kind": kind, "native_confidence": null,
           "native_confidence_kind": null, "calibration_id": null, "call": call})
}

pub(crate) fn plan(id: &str, tier: Option<QualityTier>, ctx: u64, cost: u64) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: true,
        max_context: ctx,
        est_cost_micros: cost,
        est_latency_ms: 500,
        strength_rank: 1,
        descriptor: PlanDescriptor {
            quality_tier: tier,
            ..PlanDescriptor::default()
        },
    }
}

pub(crate) fn catalog(ids: [&str; 3]) -> Vec<ModelPlan> {
    vec![
        plan(ids[0], Some(QualityTier::Frontier), 200_000, 1200),
        plan(ids[1], Some(QualityTier::Economy), 32_000, 100),
        plan(ids[2], Some(QualityTier::Standard), 64_000, 400),
    ]
}

pub(crate) fn features() -> TaskFeatures {
    TaskFeatures {
        task_family: TaskFamily::LocalizedDebug,
        estimated_context_tokens: 1000,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    }
}

pub(crate) fn inputs_with(cat: Vec<ModelPlan>, signals: RouteSignals) -> RouteInputs {
    let budget = Budget {
        max_cost_micros: 10_000,
        max_latency_ms: 5_000,
        max_router_calls: 4,
    };
    RouteInputs {
        request: RouteRequest::new(features(), cat, budget)
            .unwrap()
            .with_signals(signals),
        policy: RoutePolicy {
            router_max_calls: 4,
            ..RoutePolicy::default()
        },
    }
}

pub(crate) fn inputs(signals: RouteSignals) -> RouteInputs {
    inputs_with(catalog(["alpha", "beta", "gamma"]), signals)
}

pub(crate) fn ctx() -> RouteContext {
    RouteContext {
        project: ProjectBinding {
            id: "p".into(),
            worktree: "w".into(),
            revision: "r".into(),
        },
        lock_digest: "lock".into(),
        invocation_id: "inv-mr".into(),
        lineage_id: "lin-mr".into(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    }
}

pub(crate) const PID: &str = "org.example/fixture";

pub(crate) fn legacy() -> ProviderProfile {
    ProviderProfile {
        provider_id: PID.into(),
        model_id: "fx".into(),
        checkpoint: "ck".into(),
        ..ProviderProfile::default()
    }
}

pub(crate) fn model_profile(extra: Value) -> ModelProfile {
    let mut v =
        json!({"profile_id": "fx-default", "model": "fx-1", "identity_kind": "mutable_service"});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    ModelProfile::from_json(&v).unwrap()
}

pub(crate) fn configured(extra: Value, instance: &str) -> ProviderProfile {
    ProviderProfile::configured(
        AdapterIdentity {
            provider_id: PID.into(),
            adapter_version: "0.1.0".into(),
        },
        model_profile(extra),
        InstanceConfig {
            instance_id: instance.into(),
            endpoint: None,
            secret_refs: vec![],
        },
    )
}

pub(crate) fn run_with(
    i: &RouteInputs,
    inv: &mut V2,
    profile: ProviderProfile,
    mode: ProviderMode,
    cache: Option<&mut DecisionCache>,
) -> RouteDecision {
    let task = if inv.versions.contains(&2) {
        "model-route/v2"
    } else {
        "model-route/v1"
    };
    let gate = EnablementGate {
        task: task.into(),
        profile: profile.provider_id.clone(),
        status: GateStatus::Passed {
            evidence: "evidence:test".into(),
        },
    };
    let mut p = ConfiguredProvider {
        profile,
        invoker: inv,
        mode,
        gate,
    };
    decide(i, &ctx(), Some(&mut p), &|| i.clone(), cache).unwrap()
}

pub(crate) fn run(i: &RouteInputs, inv: &mut V2) -> RouteDecision {
    run_with(i, inv, legacy(), ProviderMode::Explicit, None)
}

fn repair_signals() -> RouteSignals {
    RouteSignals {
        phase: Phase::Repair,
        attempt_index: 2,
        previous_failure: PreviousFailure::SemanticLaw,
        no_progress: 2,
        ..RouteSignals::default()
    }
}

fn implement_signals() -> RouteSignals {
    RouteSignals {
        phase: Phase::Implement,
        previous_failure: PreviousFailure::None,
        ..RouteSignals::default()
    }
}

#[test]
fn hp_mr01_v2_features_expose_phase_and_failure_the_v1_features_cannot() {
    let (a, b) = (inputs(implement_signals()), inputs(repair_signals()));
    // The old six-field projection is identical...
    assert_eq!(a.request.features_digest(), b.request.features_digest());
    let scr = |i: &RouteInputs| screen(&i.request, &i.policy);
    let (pa, pb) = (
        a.prepare_v2(&scr(&a)).unwrap(),
        b.prepare_v2(&scr(&b)).unwrap(),
    );
    // ...the v2 feature digests are not.
    assert_ne!(pa.digests().features, pb.digests().features);
    let mut inv = V2::by_label("frontier");
    let (da, db) = (run(&a, &mut inv), run(&b, &mut inv));
    assert_eq!(inv.seen.len(), 2, "a counting adapter sees both requests");
    assert_eq!(inv.seen[0]["task"], "model-route/v2");
    assert_eq!(inv.seen[0]["features"]["phase"], "implement");
    assert_eq!(inv.seen[1]["features"]["phase"], "repair");
    assert_eq!(inv.seen[1]["features"]["previous_failure"], "semantic_law");
    let state = inv.seen[1]["rendered"]["state"].as_str().unwrap();
    assert!(state.contains("phase=repair") && state.contains("attempt_index=2"));
    assert_ne!(da.digests.v2, db.digests.v2);
    assert_eq!(da.wire.version, 2);
    // Unknown signals are explicit, never zero.
    let u = inputs(RouteSignals::default());
    let pu = u.prepare_v2(&scr(&u)).unwrap();
    assert_eq!(pu.payload["features"]["phase"], "unknown");
    assert_eq!(
        pu.payload["features"]["remaining_budget_micros"],
        Value::Null
    );
    assert!(pu
        .rendered
        .state
        .contains("remaining_budget_micros=unknown"));
}

#[test]
fn hp_mr01_renamed_opaque_ids_keep_the_selection_information() {
    for ids in [["alpha", "beta", "gamma"], ["zz-9", "aa-1", "mm-5"]] {
        let i = inputs_with(catalog(ids), implement_signals());
        let mut inv = V2::by_label("frontier");
        let d = run(&i, &mut inv);
        assert_eq!(d.source, DecisionSource::Provider);
        assert_eq!(d.choice, ids[0], "maps back to exactly the frontier plan");
        let wire = semaprax_harness::json::canonical(&inv.seen[0]);
        for id in ids {
            assert!(!wire.contains(id), "opaque id `{id}` leaked onto the wire");
        }
        let n = inv.seen[0]["candidates"].as_array().unwrap().len();
        assert_eq!(n, 3);
    }
}

#[test]
fn hp_mr01_unknown_estimates_stay_unknown_and_labels_grant_nothing() {
    let mut unknown = plan("delta", None, 64_000, 0);
    unknown.descriptor.cost_basis = Some(EstimateBasis::Unknown);
    let mut remote = plan("omega", Some(QualityTier::Frontier), 900_000, 1);
    remote.destination = Destination::Remote {
        origin: "api.example".into(),
    };
    remote.descriptor.label = Some("frontier, tools, best".into());
    let mut cat = catalog(["alpha", "beta", "gamma"]);
    cat.extend([unknown, remote]);
    let i = inputs_with(cat, implement_signals());
    let scr = screen(&i.request, &i.policy);
    let pr = i.prepare_v2(&scr).unwrap();
    let delta = pr
        .candidates
        .iter()
        .find(|c| pr.opaque(&c.id) == Some("delta"))
        .unwrap();
    assert_eq!(delta.est_cost_micros.value, None);
    assert_eq!(delta.quality_tier, QualityTier::Unknown);
    assert!(pr.rendered.state.contains("est_cost_micros=unknown"));
    // The forbidden remote plan is not a candidate whatever its label says.
    assert!(pr.selection.iter().all(|(_, o)| o != "omega"));
    // Unknown cost never wins as the cheapest under rules.
    let rules = decide(&i, &ctx(), None, &|| i.clone(), None).unwrap();
    assert_ne!(rules.choice, "delta");
    // A foreign selection id cannot widen the admitted set.
    let mut inv = V2::new(|req| {
        answer(
            req,
            Some("m9"),
            "candidate_relative",
            call(req, "fx-1", "mutable_service"),
        )
    });
    let d = run(&i, &mut inv);
    assert!(matches!(
        d.source,
        DecisionSource::Fallback(FallbackReason::RejectedChoice)
    ));
}

#[test]
fn hp_mr01_metadata_only_requests_carry_no_source_or_credentials() {
    let planted = "fn leak() { read(\"/home/u/.ssh/id_rsa\") } sk-live-0123456789abcdef";
    let mut s = implement_signals();
    s.excerpt = Some(planted.into());
    let i = inputs(s.clone());
    let mut inv = V2::by_label("frontier");
    let d = run(&i, &mut inv);
    let wire = semaprax_harness::json::canonical(&inv.seen[0]);
    assert_eq!(inv.seen[0]["disclosure"], "metadata_only");
    assert!(inv.seen[0].get("excerpt").is_none());
    for needle in ["fn leak", "/home/u", "sk-live", "id_rsa"] {
        assert!(!wire.contains(needle), "`{needle}` reached the router");
        let report = semaprax_harness::json::canonical(&d.wire.to_json());
        assert!(!report.contains(needle), "`{needle}` reached the report");
        let rec = semaprax_harness::json::canonical(&DecisionRecord::from_decision(&d).to_json());
        assert!(!rec.contains(needle), "`{needle}` reached the record");
    }
    assert!(d.wire.note.as_deref().unwrap().contains("excerpt withheld"));
    // An independent routing-disclosure policy admits a bounded excerpt...
    s.excerpt = Some("rename the helper and keep its callers".into());
    let mut allowed = inputs(s.clone());
    allowed.policy.router_excerpt_max_confidentiality = Some(Confidentiality::Project);
    let mut inv = V2::by_label("frontier");
    run(&allowed, &mut inv);
    assert_eq!(inv.seen[0]["disclosure"], "excerpt");
    assert_eq!(
        inv.seen[0]["excerpt"],
        "rename the helper and keep its callers"
    );
    assert!(inv.seen[0]["rendered"]["state"]
        .as_str()
        .unwrap()
        .contains("rename the helper"));
    // ...but never for a credential-looking excerpt, and approving remote
    // generation alone does not admit one.
    s.excerpt = Some("sk-live-0123456789abcdef".into());
    let mut cred = inputs(s.clone());
    cred.policy.router_excerpt_max_confidentiality = Some(Confidentiality::Project);
    let scr = screen(&cred.request, &cred.policy);
    assert_eq!(
        cred.prepare_v2(&scr).unwrap().disclosure,
        Disclosure::MetadataOnly
    );
    s.excerpt = Some("rename the helper".into());
    let mut gen_only = inputs(s);
    gen_only.policy.remote_max_confidentiality = Some(Confidentiality::Project);
    let scr = screen(&gen_only.request, &gen_only.policy);
    assert_eq!(
        gen_only.prepare_v2(&scr).unwrap().disclosure,
        Disclosure::MetadataOnly
    );
}

#[test]
fn hp_mr01_v1_only_adapter_gets_the_unchanged_v1_payload_with_an_explained_fallback() {
    let i = inputs(repair_signals());
    let mut inv =
        V2::new(|_| json!({"choice": "alpha", "scores": {"alpha": 0.9}, "abstain": false}));
    inv.versions = vec![1];
    let d = run(&i, &mut inv);
    let p = &inv.seen[0];
    let keys: Vec<&String> = p.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["features", "options", "task"]);
    assert_eq!(p["task"], "model-route/v1");
    assert_eq!(p["options"], json!(["alpha", "beta", "gamma"]));
    assert_eq!(d.source, DecisionSource::Provider);
    assert_eq!(d.wire.version, 1);
    assert!(d
        .wire
        .note
        .as_deref()
        .unwrap()
        .contains("did not negotiate decision.evaluate v2"));
    assert_eq!(d.digests.v2, None);
    assert!(d.digests.to_json().get("v2").is_none());
    // A v1 catalog entry keeps its exact JSON members.
    let m = plan("alpha", None, 10, 1);
    let keys: Vec<String> = m.to_json().as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        [
            "capabilities",
            "destination",
            "est_cost_micros",
            "est_latency_ms",
            "id",
            "max_context",
            "strength_rank"
        ]
    );
    // A v2 result to a v1 request is refused, not reinterpreted.
    let mut wrong = V2::new(|req| answer(req, Some("alpha"), "option_distribution", json!(null)));
    wrong.versions = vec![1];
    let d = run(&i, &mut wrong);
    assert!(matches!(
        d.source,
        DecisionSource::Fallback(FallbackReason::InvalidResult)
    ));
}

#[test]
fn hp_mr01_role_catalog_disclosure_and_policy_changes_invalidate_cache_and_qualification() {
    let mut cache = DecisionCache::new(16);
    let base = inputs(implement_signals());
    let mut inv = V2::by_label("frontier");
    let go = |i: &RouteInputs, inv: &mut V2, c: &mut DecisionCache| {
        run_with(i, inv, legacy(), ProviderMode::Explicit, Some(c)).source
    };
    assert_eq!(go(&base, &mut inv, &mut cache), DecisionSource::Provider);
    assert_eq!(go(&base, &mut inv, &mut cache), DecisionSource::Cache);
    let role = inputs(repair_signals());
    assert_eq!(go(&role, &mut inv, &mut cache), DecisionSource::Provider);
    let mut cat = catalog(["alpha", "beta", "gamma"]);
    cat[1].descriptor.label = Some("economy, tools, quick edits".into());
    let relabel = inputs_with(cat, implement_signals());
    assert_eq!(go(&relabel, &mut inv, &mut cache), DecisionSource::Provider);
    let mut s = implement_signals();
    s.excerpt = Some("rename the helper".into());
    let mut disclose = inputs(s);
    disclose.policy.router_excerpt_max_confidentiality = Some(Confidentiality::Project);
    assert_eq!(
        go(&disclose, &mut inv, &mut cache),
        DecisionSource::Provider
    );
    let mut pol = base.clone();
    pol.policy.router_max_latency_ms = 1_999;
    assert_eq!(go(&pol, &mut inv, &mut cache), DecisionSource::Provider);
    assert_eq!(inv.seen.len(), 5);
    // v1 and v2 never share a cache entry...
    let mut v1 =
        V2::new(|_| json!({"choice": "alpha", "scores": {"alpha": 0.9}, "abstain": false}));
    v1.versions = vec![1];
    assert_eq!(
        run_with(
            &base,
            &mut v1,
            legacy(),
            ProviderMode::Explicit,
            Some(&mut cache)
        )
        .source,
        DecisionSource::Provider
    );
    // ...nor a qualification key.
    let cat = base.request.catalog_digest();
    assert_ne!(
        EvidenceKey::live_versioned(&legacy(), &cat, 1).digest(),
        EvidenceKey::live_versioned(&legacy(), &cat, 2).digest()
    );
    assert_eq!(
        EvidenceKey::live_versioned(&legacy(), &cat, 1),
        EvidenceKey::live(&legacy(), &cat),
        "v1 keys keep their bytes"
    );
    // Replay of a v2 decision re-derives the v2 digests and refuses drift.
    let d = run(&base, &mut inv);
    let rec = DecisionRecord::from_json(&DecisionRecord::from_decision(&d).to_json()).unwrap();
    assert!(replay(&rec, &base).is_ok());
    let e = replay(&rec, &role).unwrap_err();
    assert_eq!(e.code, "SPX-HPJ007");
}

#[test]
fn hp_mr15_malformed_profiles_and_over_capability_requests_refuse_before_inference() {
    for bad in [
        json!({"profile_id": "p", "model": "m", "extra": 1}),
        json!({"profile_id": "p", "model": "m", "renderer": "semaprax.route-render.v9"}),
        json!({"profile_id": "p", "model": "m", "max_options": 1}),
        json!({"profile_id": "p", "model": "m", "score_kind": "none", "scoreless": false}),
        json!({"profile_id": "p", "model": "m", "identity_kind": "immutable_checkpoint"}),
        json!({"profile_id": "p", "model": "has spaces"}),
        json!({"profile_id": "p", "model": "m", "modalities": ["audio"]}),
    ] {
        assert_eq!(
            ModelProfile::from_json(&bad).unwrap_err().code,
            "SPX-HPJ020",
            "{bad}"
        );
    }
    let i = inputs(implement_signals());
    let mut inv = V2::by_label("frontier");
    let d = run_with(
        &i,
        &mut inv,
        configured(json!({"max_options": 2}), "a"),
        ProviderMode::Explicit,
        None,
    );
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::Unsupported)
    );
    assert_eq!(d.router_calls, 0);
    let mut s = implement_signals();
    s.input_modalities = [Modality::Image, Modality::Text].into();
    let d = run_with(
        &inputs(s),
        &mut inv,
        configured(json!({}), "a"),
        ProviderMode::Explicit,
        None,
    );
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::Unsupported)
    );
    assert!(d.wire.note.as_deref().unwrap().contains("image"));
    assert!(inv.seen.is_empty(), "nothing was sent to the adapter");
    // More admitted candidates than the v2 task bound: refused, not truncated.
    let many: Vec<ModelPlan> = (0..17)
        .map(|n| plan(&format!("p{n:02}"), None, 10_000, 5))
        .collect();
    let d = run(&inputs_with(many, implement_signals()), &mut inv);
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::Unsupported)
    );
    assert!(inv.seen.is_empty());
}

#[test]
fn hp_mr15_two_profiles_and_instances_of_one_adapter_never_share_cache_or_evidence() {
    let a = configured(
        json!({"profile_id": "small", "model": "fx-small"}),
        "inst-a",
    );
    let b = configured(
        json!({"profile_id": "large", "model": "fx-large"}),
        "inst-a",
    );
    let a2 = configured(
        json!({"profile_id": "small", "model": "fx-small"}),
        "inst-b",
    );
    assert_eq!(a.provider_id, b.provider_id, "one adapter");
    assert_ne!(a.scope_digest(), b.scope_digest());
    assert_ne!(
        a.scope_digest(),
        a2.scope_digest(),
        "instances are distinct scopes"
    );
    let cat = inputs(implement_signals()).request.catalog_digest();
    let keys: Vec<String> = [&a, &b, &a2]
        .iter()
        .map(|p| EvidenceKey::live_versioned(p, &cat, 2).digest())
        .collect();
    assert!(keys[0] != keys[1] && keys[0] != keys[2] && keys[1] != keys[2]);
    let i = inputs(implement_signals());
    let mut cache = DecisionCache::new(8);
    let mut inv = V2::by_label("frontier");
    for p in [&a, &b, &a2] {
        let d = run_with(
            &i,
            &mut inv,
            p.clone(),
            ProviderMode::Explicit,
            Some(&mut cache),
        );
        assert_eq!(d.source, DecisionSource::Provider);
    }
    let d = run_with(
        &i,
        &mut inv,
        a.clone(),
        ProviderMode::Explicit,
        Some(&mut cache),
    );
    assert_eq!(
        d.source,
        DecisionSource::Cache,
        "same profile and instance hits"
    );
    assert_eq!(inv.seen.len(), 3);
}
