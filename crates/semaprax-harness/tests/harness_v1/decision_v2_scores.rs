//! MR-02 score semantics and MR-03 call identity/usage for `model-route/v2`
//! (fixture invokers from `decision_v2`).

use crate::decision_v2::*;
use semaprax_harness::decision::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn with(req: &Value, f: impl Fn(&mut Value)) -> Value {
    let mut v = answer(
        req,
        Some(&label_pick(req, "frontier")),
        "option_distribution",
        call(req, "fx-1", "mutable_service"),
    );
    f(&mut v);
    v
}

fn source(d: &RouteDecision) -> DecisionSource {
    d.source
}

#[test]
fn hp_mr02_option_mass_and_native_confidence_are_separate_measures() {
    let i = inputs(RouteSignals::default());
    // High chosen-option mass, low vendor confidence (a Laya-like answer).
    let laya = |req: &Value| {
        with(req, |v| {
            v["native_confidence"] = json!(0.2);
            v["native_confidence_kind"] = json!("laya.confidence");
        })
    };
    let mut p = legacy();
    p.min_option_mass = Some(0.8);
    let mut inv = V2::new(laya);
    let d = run_with(&i, &mut inv, p.clone(), ProviderMode::Explicit, None);
    assert_eq!(source(&d), DecisionSource::Provider);
    assert_eq!(
        d.wire.native_confidence,
        Some((0.2, "laya.confidence".into()))
    );
    let shown = d.wire.to_json();
    assert_eq!(
        shown["scores_are"],
        "option mass (not a probability of task success)"
    );
    // Raising the host mass threshold does not touch the native measure...
    p.min_option_mass = Some(0.95);
    let d = run_with(&i, &mut inv, p.clone(), ProviderMode::Explicit, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::LowConfidence)
    );
    assert_eq!(d.wire.abstention, Some(AbstentionReason::HostThreshold));
    // ...and a native abstention is authoritative whatever the host allows.
    p.min_option_mass = None;
    let mut abstains = V2::new(|req| {
        answer(
            req,
            None,
            "option_distribution",
            call(req, "fx-1", "mutable_service"),
        )
    });
    let d = run_with(&i, &mut abstains, p, ProviderMode::Explicit, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::Abstain)
    );
    assert_eq!(d.wire.abstention, Some(AbstentionReason::Native));
}

#[test]
fn hp_mr02_equal_numbers_of_different_score_kinds_do_not_share_a_threshold() {
    let i = inputs(RouteSignals::default());
    let mut p = legacy();
    p.min_option_mass = Some(0.95);
    // 0.9 candidate-relative (Mini Jev style): the mass threshold does not apply.
    let mut rel = V2::new(|req| {
        let pick = label_pick(req, "frontier");
        answer(
            req,
            Some(&pick),
            "candidate_relative",
            call(req, "fx-1", "mutable_service"),
        )
    });
    let d = run_with(&i, &mut rel, p.clone(), ProviderMode::Explicit, None);
    assert_eq!(source(&d), DecisionSource::Provider);
    assert_eq!(d.wire.score_kind, Some(ScoreKind::CandidateRelative));
    // The same 0.9 as an option distribution is held to it.
    let mut dist = V2::by_label("frontier");
    let d = run_with(&i, &mut dist, p.clone(), ProviderMode::Explicit, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::LowConfidence)
    );
    // The deprecated alias keeps its documented behavior for any score kind.
    p.min_option_mass = None;
    p.min_confidence = Some(0.95);
    let d = run_with(&i, &mut rel, p, ProviderMode::Explicit, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::LowConfidence)
    );
    // Profiles declaring different score semantics are different qualification keys.
    let cat = i.request.catalog_digest();
    let jev = configured(
        json!({"profile_id": "jev", "score_kind": "option_distribution"}),
        "x",
    );
    let mini = configured(
        json!({"profile_id": "mini", "score_kind": "candidate_relative"}),
        "x",
    );
    assert_ne!(
        EvidenceKey::live_versioned(&jev, &cat, 2),
        EvidenceKey::live_versioned(&mini, &cat, 2)
    );
    // A declared option_distribution profile refuses candidate-relative answers.
    let d = run_with(&i, &mut rel, jev, ProviderMode::Explicit, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::InvalidResult)
    );
}

#[test]
fn hp_mr02_malformed_scores_and_metadata_remain_rejected() {
    let i = inputs(RouteSignals::default());
    let scr = screen(&i.request, &i.policy);
    let pr = i.prepare_v2(&scr).unwrap();
    let req = &pr.payload;
    let ok = with(req, |_| {});
    assert!(ResultV2::from_json(&ok).is_ok());
    let opts = pr.options();
    let check = |v: &Value| {
        ResultV2::from_json(v)
            .and_then(|r| r.check_against(&opts, &pr.rendered.digest, pr.max_wire_bytes))
    };
    assert!(check(&ok).is_ok());
    let cases: Vec<(&str, Value)> = vec![
        (
            "out of range",
            with(req, |v| v["scores"]["m1"] = json!(1.5)),
        ),
        ("negative", with(req, |v| v["scores"]["m1"] = json!(-0.1))),
        (
            "missing option",
            with(req, |v| {
                v["scores"].as_object_mut().unwrap().remove("m1");
            }),
        ),
        (
            "foreign option",
            with(req, |v| v["scores"]["m7"] = json!(0.0)),
        ),
        ("non-argmax", with(req, |v| v["choice"] = json!("m1"))),
        (
            "not normalized",
            with(req, |v| v["scores"]["m1"] = json!(0.5)),
        ),
        (
            "kind without scores",
            with(req, |v| v["scores"] = Value::Null),
        ),
        (
            "scores without kind",
            with(req, |v| v["score_kind"] = json!("none")),
        ),
        (
            "confidence without kind",
            with(req, |v| v["native_confidence"] = json!(0.3)),
        ),
        (
            "prose calibration id",
            with(req, |v| v["calibration_id"] = json!("my calibration")),
        ),
        (
            "unknown member",
            with(req, |v| v["probability_of_success"] = json!(0.9)),
        ),
        (
            "abstain reason mismatch",
            with(req, |v| v["abstention_reason"] = json!("native")),
        ),
        (
            "string score",
            with(req, |v| v["scores"]["m1"] = json!("0.05")),
        ),
    ];
    for (what, v) in cases {
        assert!(check(&v).is_err(), "{what} was accepted");
    }
}

#[test]
fn hp_mr02_scoreless_needs_declared_support_and_is_never_given_scores() {
    let i = inputs(RouteSignals::default());
    let scoreless = |req: &Value| {
        let pick = label_pick(req, "frontier");
        answer(
            req,
            Some(&pick),
            "none",
            call(req, "fx-1", "local_declared"),
        )
    };
    let mut inv = V2::new(scoreless);
    // Undeclared: refused rather than trusted.
    let d = run_with(&i, &mut inv, legacy(), ProviderMode::Explicit, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::InvalidResult)
    );
    let declared = configured(
        json!({"score_kind": "none", "identity_kind": "local_declared"}),
        "x",
    );
    let d = run_with(&i, &mut inv, declared.clone(), ProviderMode::Explicit, None);
    assert_eq!(source(&d), DecisionSource::Provider);
    assert_eq!(d.choice, "alpha");
    assert_eq!(d.wire.score_kind, Some(ScoreKind::None));
    assert_eq!(
        d.wire.to_json()["scores_are"],
        "no scores (scoreless provider)"
    );
    // A missing score still fails a deprecated-alias threshold closed.
    let mut strict = declared;
    strict.min_confidence = Some(0.5);
    let d = run_with(&i, &mut inv, strict, ProviderMode::Explicit, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::LowConfidence)
    );
}

fn outcome(item: &str, arm: &str, cost: u64) -> Outcome {
    Outcome {
        item: item.into(),
        arm: arm.into(),
        model: "alpha".into(),
        origin: Origin::Real,
        verified_by: "scripted-grader".into(),
        completed: true,
        regressions: 0,
        attempts: 1,
        cost_micros: Some(cost),
        latency_ms: Some(100),
        router_cost_micros: 0,
        context_cost_micros: 0,
        retry_owner: RetryOwner::Host,
    }
}

fn record(key: EvidenceKey, calibration: Option<Calibration>) -> EvidenceRecord {
    let items: BTreeSet<String> = (0..4).map(|i| format!("e{i}")).collect();
    let mut outcomes = vec![];
    for i in &items {
        outcomes.push(outcome(i, RULES_ARM, 100));
        outcomes.push(outcome(i, &key.provider_id, 50));
    }
    EvidenceRecord {
        key,
        budget: MatchedBudget {
            max_cost_micros: 1000,
            max_attempts: 2,
        },
        eval_items: items,
        trained_on: BTreeSet::new(),
        outcomes,
        calibration,
    }
}

#[test]
fn hp_mr02_unknown_calibration_never_passes_a_confidence_gate() {
    let cat = inputs(RouteSignals::default()).request.catalog_digest();
    let scored = configured(json!({}), "x");
    let key = EvidenceKey::live_versioned(&scored, &cat, 2);
    let outcome_gate = GateSpec {
        min_items: 4,
        ..GateSpec::default()
    };
    let confidence_gate = GateSpec {
        basis: GateBasis::CalibratedConfidence {
            min_success_estimate: 0.8,
        },
        ..outcome_gate.clone()
    };
    assert_ne!(outcome_gate.digest(), confidence_gate.digest());
    let uncal = record(key.clone(), None);
    assert!(evaluate(&outcome_gate, &uncal).go);
    let d = evaluate(&confidence_gate, &uncal);
    assert!(!d.go);
    assert!(d.reasons[0].contains("unknown calibration"));
    let cal = |key_digest: String, est: f64| Calibration {
        calibration_id: "cal-1".into(),
        score_kind: ScoreKind::OptionDistribution,
        key_digest,
        success_estimate: est,
    };
    assert!(
        evaluate(
            &confidence_gate,
            &record(key.clone(), Some(cal(key.digest(), 0.9)))
        )
        .go
    );
    assert!(
        !evaluate(
            &confidence_gate,
            &record(key.clone(), Some(cal(key.digest(), 0.5)))
        )
        .go
    );
    // A calibration fit on another profile/renderer regime does not transfer.
    let other = EvidenceKey::live_versioned(&scored, &cat, 1).digest();
    assert!(!evaluate(&confidence_gate, &record(key, Some(cal(other, 0.99)))).go);
    // A scoreless provider qualifies only under the outcome-based gate.
    let sl = configured(json!({"score_kind": "none"}), "x");
    let sk = EvidenceKey::live_versioned(&sl, &cat, 2);
    assert!(evaluate(&outcome_gate, &record(sk.clone(), None)).go);
    assert!(!evaluate(&confidence_gate, &record(sk, None)).go);
}

#[test]
fn hp_mr03_a_qualified_route_requires_the_qualified_answering_identity() {
    let i = inputs(RouteSignals::default());
    let qualified = configured(
        json!({"model": "jev-2026-09", "identity_kind": "mutable_service"}),
        "x",
    );
    let answering = |model: &'static str| {
        V2::new(move |req| {
            let pick = label_pick(req, "frontier");
            answer(
                req,
                Some(&pick),
                "option_distribution",
                call(req, model, "mutable_service"),
            )
        })
    };
    let mut cache = DecisionCache::new(4);
    let mut drifted = answering("jev-2026-10");
    let d = run_with(
        &i,
        &mut drifted,
        qualified.clone(),
        ProviderMode::Auto,
        Some(&mut cache),
    );
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::IdentityMismatch)
    );
    assert!(d.wire.note.as_deref().unwrap().contains("jev-2026-10"));
    assert_eq!(cache.len(), 0, "a mismatched identity is never cached");
    // Explicit (experimental) use records the identity but makes no claim.
    let d = run_with(
        &i,
        &mut drifted,
        qualified.clone(),
        ProviderMode::Explicit,
        None,
    );
    assert_eq!(source(&d), DecisionSource::Provider);
    assert_eq!(
        d.wire.call.as_ref().unwrap().answering_model.as_deref(),
        Some("jev-2026-10")
    );
    let mut same = answering("jev-2026-09");
    let d = run_with(
        &i,
        &mut same,
        qualified,
        ProviderMode::Auto,
        Some(&mut cache),
    );
    assert_eq!(source(&d), DecisionSource::Provider);
    // An unknown identity cannot be verified at all.
    let unknown = configured(json!({"identity_kind": "unknown"}), "x");
    let mut any = answering("fx-1");
    let d = run_with(&i, &mut any, unknown, ProviderMode::Auto, None);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::IdentityMismatch)
    );
}

#[test]
fn hp_mr03_mutable_aliases_and_immutable_checkpoints_are_distinct() {
    let alias = model_profile(json!({"model": "jev-latest", "identity_kind": "mutable_service"}));
    let ckpt = model_profile(
        json!({"model": "laya-7b", "identity_kind": "immutable_checkpoint", "checkpoint": "laya-7b-r42"}),
    );
    assert_eq!(alias.checkpoint_label(), "mutable_service:jev-latest");
    assert_eq!(ckpt.checkpoint_label(), "laya-7b-r42");
    let mk = |p: ModelProfile| {
        ProviderProfile::configured(
            AdapterIdentity {
                provider_id: PID.into(),
                adapter_version: "0.1.0".into(),
            },
            p,
            InstanceConfig {
                instance_id: "x".into(),
                endpoint: None,
                secret_refs: vec![],
            },
        )
    };
    let (pa, pc) = (mk(alias), mk(ckpt));
    assert_ne!(pa.checkpoint, pc.checkpoint);
    let c = |model: Option<&str>, ck: Option<&str>, kind: IdentityKind| CallMetadata {
        adapter: "a@1".into(),
        requested_model: None,
        answering_model: model.map(String::from),
        checkpoint: ck.map(String::from),
        identity_kind: kind,
        rendered_digest: format!("sha256:{}", "0".repeat(64)),
        wire_bytes: 1,
        usage: Usage {
            input_tokens: None,
            output_tokens: None,
            basis: UsageBasis::Unknown,
        },
        billing: Billing::Local,
    };
    assert!(pc
        .verify_identity(&c(
            None,
            Some("laya-7b-r42"),
            IdentityKind::ImmutableCheckpoint
        ))
        .is_ok());
    assert!(pc
        .verify_identity(&c(
            Some("laya-7b"),
            Some("laya-7b-r43"),
            IdentityKind::ImmutableCheckpoint
        ))
        .is_err());
    // A mutable service cannot claim reproducibility it lacks.
    assert!(pc
        .verify_identity(&c(Some("laya-7b"), None, IdentityKind::MutableService))
        .is_err());
    assert!(pa
        .verify_identity(&c(Some("jev-latest"), None, IdentityKind::MutableService))
        .is_ok());
}

#[test]
fn hp_mr03_prepared_digest_and_wire_bound_are_enforced_and_descriptions_raise_the_bound() {
    let i = inputs(RouteSignals::default());
    let mut forged = V2::new(|req| {
        with(req, |v| {
            v["call"]["rendered_digest"] = json!(format!("sha256:{}", "a".repeat(64)))
        })
    });
    let d = run(&i, &mut forged);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::InvalidResult)
    );
    assert!(d.router_calls == 1, "the dispatched call still counts");
    let mut oversized = V2::new(|req| {
        let max = req["max_wire_bytes"].as_u64().unwrap();
        with(req, move |v| v["call"]["wire_bytes"] = json!(max + 1))
    });
    let d = run(&i, &mut oversized);
    assert_eq!(
        source(&d),
        DecisionSource::Fallback(FallbackReason::InvalidResult)
    );
    // Captured request digest equals what the host prepared and recorded.
    let mut inv = V2::by_label("frontier");
    let d = run(&i, &mut inv);
    assert_eq!(
        inv.seen[0]["rendered"]["digest"].as_str(),
        d.wire.rendered_digest.as_deref()
    );
    let call = d.wire.call.as_ref().unwrap();
    assert_eq!(
        Some(call.rendered_digest.as_str()),
        d.wire.rendered_digest.as_deref()
    );
    // Longer option descriptions change the prepared bytes and the bound.
    let scr = screen(&i.request, &i.policy);
    let short = i.prepare_v2(&scr).unwrap();
    let mut cat = catalog(["alpha", "beta", "gamma"]);
    for p in &mut cat {
        p.descriptor.label = Some("a considerably longer comparison label text".into());
    }
    let j = inputs_with(cat, RouteSignals::default());
    let long = j.prepare_v2(&screen(&j.request, &j.policy)).unwrap();
    assert!(long.accounting_text().len() > short.accounting_text().len());
    assert!(long.rendered.model_visible_bytes() > short.rendered.model_visible_bytes());
    assert_ne!(long.rendered.digest, short.rendered.digest);
}

#[test]
fn hp_mr03_call_identity_is_journaled_and_replayed_without_inference() {
    let i = inputs(RouteSignals::default());
    let mut inv = V2::by_label("frontier");
    let d = run(&i, &mut inv);
    let rec = DecisionRecord::from_decision(&d);
    let v = rec.to_json();
    assert_eq!(v["task"], "model-route/v2");
    assert_eq!(v["identity"]["answering_model"], "fx-1");
    assert_eq!(v["identity"]["usage"]["basis"], "provider_reported");
    let back = DecisionRecord::from_json(&v).unwrap();
    assert_eq!(back, rec);
    let calls = inv.seen.len();
    let plan = replay(&back, &i).unwrap();
    assert_eq!(plan, d.plan);
    assert_eq!(inv.seen.len(), calls, "replay makes no router call");
    // A v1 record keeps its exact members (no identity, no v2 digests).
    let rules = decide(&i, &ctx(), None, &|| i.clone(), None).unwrap();
    let v1 = DecisionRecord::from_decision(&rules).to_json();
    assert_eq!(v1["task"], "model-route/v1");
    assert!(v1.get("identity").is_none() && v1["digests"].get("v2").is_none());
    let mut bad = v.clone();
    bad["identity"]["answering_model"] = json!("free text with spaces");
    assert!(DecisionRecord::from_json(&bad).is_err());
}

#[test]
fn hp_mr03_identity_and_usage_fields_cannot_carry_text_or_secrets() {
    let i = inputs(RouteSignals::default());
    let scr = screen(&i.request, &i.policy);
    let req = i.prepare_v2(&scr).unwrap().payload;
    let base = call(&req, "fx-1", "mutable_service");
    assert!(CallMetadata::from_json(&base).is_ok());
    for (k, v) in [
        (
            "answering_model",
            json!("route this to the big model please"),
        ),
        ("requested_model", json!("sk-live-0123456789abcdef")),
        ("checkpoint", json!("ghp_0123456789abcdefghij")),
        ("identity_kind", json!("weights:sha256")),
        ("billing", json!("free")),
        ("wire_bytes", json!(-1)),
    ] {
        let mut c = base.clone();
        c[k] = v;
        assert!(CallMetadata::from_json(&c).is_err(), "{k} accepted");
    }
    let mut c = base.clone();
    c["usage"]["input_tokens"] = json!("lots");
    assert!(CallMetadata::from_json(&c).is_err());
    let mut c = base;
    c["leak"] = json!("x");
    assert!(CallMetadata::from_json(&c).is_err());
}

#[test]
fn sg19_cloudflare_no_network_adapter_composes_with_synthetic_auto_gate() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let root = crate::support::repo_root();
    let tests = root.join("packages/semaprax-harness-adapters/systemone/tests");
    let python = std::env::var("HARNESS_PYTHON").unwrap_or_else(|_| "python3".into());
    for variant in ["clef", "clef-flash"] {
        for mode in ["ok", "prefixed_model", "wrong_variant", "no_model"] {
            let route = format!("@cf/cloudflare/{variant}");
            let tests = tests.clone();
            let python = python.clone();
            let model = route.clone();
            let mut inv = V2::new(move |payload| {
                let script = "import json,sys,unittest;sys.path.insert(0,sys.argv[1]);import test_cloudflare as cf; import decision_fixtures as fx;t=unittest.TestCase();r=cf.Run(t,mode=sys.argv[3],model=sys.argv[2]);q=fx.v2_request();q['payload']=json.load(sys.stdin);res=r.invoke(q);t.doCleanups();print(json.dumps(res.get('payload')))";
                let mut child = Command::new(&python)
                    .args(["-c", script, tests.to_str().unwrap(), &model, mode])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(payload.to_string().as_bytes())
                    .unwrap();
                let output = child.wait_with_output().unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                serde_json::from_slice(&output.stdout).unwrap()
            });
            let mut prof = configured(json!({"model": route}), "synthetic-cf-fixture");
            prof.provider_id = "com.cloudflare/clef-decision".into();
            let d = run_with(
                &inputs(RouteSignals::default()),
                &mut inv,
                prof,
                ProviderMode::Auto,
                None,
            );
            if ["ok", "prefixed_model"].contains(&mode) {
                assert_eq!(d.source, DecisionSource::Provider);
                assert_eq!(
                    d.wire.call.as_ref().unwrap().answering_model.as_deref(),
                    Some(route.as_str())
                );
                assert_eq!(d.wire.call.as_ref().unwrap().usage.input_tokens, Some(212));
            } else {
                assert_eq!(
                    d.source,
                    DecisionSource::Fallback(FallbackReason::InvalidResult)
                );
            }
        }
    }
}
