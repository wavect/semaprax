//! MR-07 parity: identical admitted inputs produce identical decisions and
//! rejection reasons through the harness path (`decision::decide` over the
//! envelope invoker) and the runtime boundary (`semaprax_decision_core::
//! boundary::recommend` over a core-native invoker). Both paths run the one
//! decision core; this pins that the harness adaptation adds no behavior.

use super::decision_v2::{answer, call, ctx, inputs, label_pick, legacy, V2};
use semaprax_decision_core::boundary::{recommend, recommend_static};
use semaprax_decision_core::request::DecisionRequest;
use semaprax_harness::decision::*;
use semaprax_harness::diag::HarnessDiagnostic;
use serde_json::{json, Value};

/// The same scripted adapter seen through the core's MR-15 trait directly.
struct CoreFx(V2);

impl semaprax_decision_core::DecisionInvoker for CoreFx {
    fn evaluate(&mut self, r: &DecisionRequest) -> DecisionCall {
        r.validate().expect("the core validates before dispatch");
        self.0.seen.push(r.payload.clone());
        let result = (self.0.answer)(&r.payload);
        DecisionCall::Answered {
            call: result
                .get("call")
                .and_then(|c| CallMetadata::from_json(c).ok()),
            result,
            elapsed_ms: 1,
        }
    }

    fn decision_versions(&self) -> Vec<u32> {
        self.0.versions.clone()
    }
}

fn gate(versions: &[u32]) -> EnablementGate {
    let task = if versions.contains(&2) {
        "model-route/v2"
    } else {
        "model-route/v1"
    };
    EnablementGate {
        task: task.into(),
        profile: legacy().provider_id,
        status: GateStatus::Passed {
            evidence: "evidence:test".into(),
        },
    }
}

type Script = fn() -> V2;

/// Run one scripted adapter through both paths and return both outcomes plus
/// the payloads each invoker saw.
#[allow(clippy::type_complexity)]
fn both(
    i: &RouteInputs,
    script: Script,
) -> (
    Result<RouteDecision, HarnessDiagnostic>,
    Result<RouteDecision, HarnessDiagnostic>,
    Vec<Value>,
    Vec<Value>,
) {
    let mut h = script();
    let g = gate(&h.versions);
    let mut hp = ConfiguredProvider {
        profile: legacy(),
        invoker: &mut h,
        mode: ProviderMode::Explicit,
        gate: g.clone(),
    };
    let harness = decide(i, &ctx(), Some(&mut hp), &|| i.clone(), None);
    let mut c = CoreFx(script());
    let mut cp = semaprax_decision_core::ConfiguredProvider {
        profile: legacy(),
        invoker: &mut c,
        mode: ProviderMode::Explicit,
        gate: g,
    };
    let runtime = recommend(i, &ctx(), Some(&mut cp), None).map(|r| r.into_decision());
    (harness, runtime, h.seen, c.0.seen)
}

fn v2_label() -> V2 {
    V2::by_label("economy")
}

fn v1_only() -> V2 {
    let mut v = V2::new(|req| {
        let first = req["options"][1].as_str().unwrap().to_string();
        json!({"choice": first, "scores": {first.clone(): 0.8}, "abstain": false})
    });
    v.versions = vec![1];
    v
}

fn out_of_range() -> V2 {
    V2::new(|req| {
        answer(
            req,
            Some("m9"),
            "option_distribution",
            call(req, "fx-1", "mutable_service"),
        )
    })
}

fn authority_members() -> V2 {
    V2::new(|req| {
        let mut a = answer(
            req,
            Some(&label_pick(req, "frontier")),
            "option_distribution",
            call(req, "fx-1", "mutable_service"),
        );
        a["grant"] = json!({"tools": ["shell"]});
        a
    })
}

fn abstains() -> V2 {
    V2::new(|req| {
        answer(
            req,
            None,
            "option_distribution",
            call(req, "fx-1", "mutable_service"),
        )
    })
}

#[test]
fn mr07_rules_path_is_identical_through_harness_and_runtime_boundary() {
    let i = inputs(RouteSignals::default());
    let harness = decide(&i, &ctx(), None, &|| i.clone(), None).unwrap();
    let runtime = recommend_static(&i, &ctx()).unwrap().into_decision();
    assert_eq!(harness, runtime);
    assert_eq!(runtime.source, DecisionSource::Rules);
    assert_eq!(runtime.router_calls, 0);
}

#[test]
fn mr07_provider_decisions_and_rejections_match_across_paths() {
    let i = inputs(RouteSignals::default());
    let cases: [(Script, DecisionSource); 5] = [
        (v2_label, DecisionSource::Provider),
        (v1_only, DecisionSource::Provider),
        (
            out_of_range,
            DecisionSource::Fallback(FallbackReason::RejectedChoice),
        ),
        (
            authority_members,
            DecisionSource::Fallback(FallbackReason::InvalidResult),
        ),
        (abstains, DecisionSource::Fallback(FallbackReason::Abstain)),
    ];
    for (script, want) in cases {
        let (harness, runtime, hs, rs) = both(&i, script);
        let (harness, runtime) = (harness.unwrap(), runtime.unwrap());
        assert_eq!(harness, runtime, "decision diverged for {want:?}");
        assert_eq!(runtime.source, want);
        assert_eq!(hs, rs, "the adapters saw different payloads for {want:?}");
        assert_eq!(hs.len(), 1);
        // A recommendation names only admitted candidates.
        assert!(runtime.plan.ordered.iter().all(|s| i
            .request
            .catalog
            .iter()
            .any(|m| m.id == s.model_id)));
    }
}

#[test]
fn mr07_refusals_carry_the_same_code_and_message() {
    // Fallback `refuse` turns a rejected answer into an error on both paths.
    let mut i = inputs(RouteSignals::default());
    i.policy.fallback = FallbackMode::Refuse;
    let (harness, runtime, _, _) = both(&i, out_of_range);
    let (h, r) = (harness.unwrap_err(), runtime.unwrap_err());
    assert_eq!(h, r);
    assert_eq!(h.code, "SPX-HPJ013");
    // No admissible model: the hard screen refuses before any provider call.
    let mut j = inputs(RouteSignals::default());
    j.request.features.estimated_context_tokens = 900_000;
    let (harness, runtime, hs, rs) = both(&j, v2_label);
    assert_eq!(harness.unwrap_err(), runtime.unwrap_err());
    assert!(hs.is_empty() && rs.is_empty());
}

#[test]
fn mr07_runtime_recommendations_replay_through_the_harness_record() {
    let i = inputs(RouteSignals::default());
    let (_, runtime, _, _) = both(&i, v2_label);
    let d = runtime.unwrap();
    let record = DecisionRecord::from_decision(&d);
    let parsed = DecisionRecord::from_json(&record.to_json()).unwrap();
    assert_eq!(replay(&parsed, &i).unwrap(), d.plan);
}
