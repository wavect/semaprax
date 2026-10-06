//! MR-14 end-to-end routing lane (runtime consumer), run by CI with the rest
//! of `agent_runtime_v1`: committed config -> adapter negotiation -> typed
//! selection -> authorized dispatch -> report -> resume, with fixture
//! adapters only and no keys of any kind.
//!
//! It also drives an out-of-tree decision adapter (`OutOfTreeAdapter`, defined
//! only in this test crate through the public MR-15 `DecisionInvoker` trait)
//! through both runtime consumers, choice selection and model routing, with
//! no change to routing, workflow or runtime dispatch code. Fixture evidence
//! proves extensibility, not routing quality.

use std::rc::Rc;

use super::choice_examples::{attach, support_inputs, support_policy, support_turn, with_support};
use super::runtime_routing::{
    ctx, features, handlers, host, response, settling_factory, Created, MemStore,
};
use semaprax::model_routing::engine::{
    select_choice, ChoiceOutcome, ConfiguredProvider, DecisionCall, DecisionInvoker,
    DecisionRequest, EnablementGate, FixtureChoiceInvoker, ProviderMode, ProviderProfile,
    TaskFamily, CHOICE_TASK, CHOICE_WIRE_VERSION,
};
use semaprax::model_routing::runtime::{
    authorize_specialist_choice, start_routed_task, RouteReason, RouteSource, RoutedSession,
    TurnStatus, TurnVerdict,
};
use serde_json::{json, Value};

#[test]
fn routing_lane_config_negotiation_selection_dispatch_report_resume() {
    with_support(|w, config, registry, set| {
        // Config: committed host documents only. The ticket is private user
        // text; it may reach the adapter only as the disclosed excerpt and
        // never a report.
        let ticket = "please refund my invoice payment".to_owned();
        let inputs = support_inputs(config, registry, set, &ticket);
        let provider_id = config["choice"]["decision_provider"]["provider_id"]
            .as_str()
            .unwrap();

        // Negotiation: the attached adapter carries choice-select/v1 only if
        // it negotiated decision.evaluate v3; checking that is free.
        let mut inv = FixtureChoiceInvoker::default();
        assert!(inv.decision_versions().contains(&CHOICE_WIRE_VERSION));
        let mut provider = attach(provider_id, &mut inv);
        assert_eq!(provider.status(CHOICE_TASK), "experimental");
        assert_eq!(provider.invoker.calls(), 0, "negotiation makes no call");

        // Selection.
        let outcome = select_choice(&inputs, &ctx("lane"), Some(&mut provider));
        drop(provider);
        let ChoiceOutcome::Selected { selection, report } = outcome else {
            panic!("expected a selection, got {outcome:?}");
        };
        assert_eq!(selection.id(), "support.billing");
        assert_eq!(report.router_calls, 1);

        // Authorized dispatch.
        let auth = authorize_specialist_choice(&selection, &inputs, registry, set).unwrap();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(set, response(&w.schema, "1"), created.clone());
        let target = w.target(b"support request");
        let mut journal = MemStore::default();
        let outcome = {
            let mut session = RoutedSession::open(
                set,
                support_policy(registry),
                "support.lane",
                "sha256:support-instructions",
                "sha256:support-acceptance",
                2,
                None,
                &mut journal,
            )
            .unwrap();
            session
                .run_turn::<FixtureChoiceInvoker>(
                    &support_turn(auth.id()),
                    &ctx("lane"),
                    None,
                    &target,
                    handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                    &mut MemStore::default(),
                    None,
                    &mut |_| TurnVerdict::Accepted {
                        committed_state: b"replied".to_vec(),
                        progressed: true,
                        complete: true,
                        tool_results: Vec::new(),
                    },
                )
                .unwrap()
        };
        assert_eq!(
            (outcome.reason, outcome.status),
            (RouteReason::Specialist, TurnStatus::Complete)
        );
        let record = outcome.run.as_ref().unwrap().record.clone();

        // Report: decision provider and actual answering model, score
        // semantics, the selected deployment identity and the generation
        // model, router overhead. Never the private input or a token.
        let deployment = set.profile(auth.profile()).unwrap().deployment();
        let generation = &deployment.model_selections()[0];
        let call = report.wire.call.clone().unwrap();
        let doc = json!({
            "execution_domain": "runtime",
            "decision": {
                "task": CHOICE_TASK, "provider": report.provider_id,
                "provider_status": report.provider_status,
                "answering_model": call.answering_model, "wire": report.wire.to_json(),
                "admitted": report.admitted, "option_set_digest": report.option_set_digest,
                "router_calls": report.router_calls, "router_ms": report.router_ms,
            },
            "dispatch": {
                "specialist": auth.id(), "profile": auth.profile(),
                "deployment": auth.deployment_digest(),
                "generation_model": format!("{}/{}", generation.provider_id(), generation.model_id()),
                "route": record.to_json(),
            },
        });
        let text = doc.to_string();
        assert!(!text.contains("refund my invoice") && !text.contains("excerpt\":"));
        assert_eq!(doc["decision"]["answering_model"], "word-overlap-fixture");
        assert_eq!(doc["dispatch"]["generation_model"], "fake.local/fake-basic");
        assert_eq!(
            doc["dispatch"]["route"]["deployment"],
            auth.deployment_digest()
        );
        assert!(doc["decision"]["wire"]
            .to_string()
            .contains("not a probability"));
        assert_eq!(created.borrow().as_slice(), ["fake.local"]);

        // Resume: the completed turn replays from the journal with no route,
        // adapter or decision call.
        let (doc, generation) = journal.latest();
        let mut store = MemStore::default();
        let resumed =
            RoutedSession::resume(set, support_policy(registry), &doc, generation, &mut store)
                .unwrap();
        let replay = resumed.replay_turn(0).unwrap();
        assert!(replay.replayed);
        assert_eq!(
            (replay.profile.as_str(), replay.status),
            (auth.profile(), TurnStatus::Complete)
        );
        assert_eq!(created.borrow().len(), 1, "no adapter on resume");
        assert_eq!(inv.calls(), 1, "no decision call on resume");
    });
}

/// An additional decision adapter that lives outside the tree: it only
/// implements the public `DecisionInvoker` contract. It always picks the
/// first offered option (v1 model route or v3 choice) and reports its own
/// identity in the call metadata.
struct OutOfTreeAdapter {
    calls: u32,
}

impl DecisionInvoker for OutOfTreeAdapter {
    fn evaluate(&mut self, request: &DecisionRequest) -> DecisionCall {
        self.calls += 1;
        let p = &request.payload;
        let options: Vec<&str> = p["options"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let first = options[0];
        let result = if request.version == CHOICE_WIRE_VERSION {
            let scores: serde_json::Map<String, Value> = options
                .iter()
                .map(|o| {
                    (
                        o.to_string(),
                        json!(if *o == first {
                            0.8
                        } else {
                            0.2 / (options.len() as f64 - 1.0)
                        }),
                    )
                })
                .collect();
            json!({
                "choice": first, "abstain": false, "abstention_reason": "none",
                "scores": scores, "score_kind": "option_distribution",
                "native_confidence": null, "native_confidence_kind": null, "calibration_id": null,
                "call": {"adapter": "example/out-of-tree@0.1.0", "requested_model": "first-option",
                         "answering_model": "first-option", "checkpoint": "oot-1",
                         "identity_kind": "local_declared", "rendered_digest": p["rendered"]["digest"],
                         "wire_bytes": semaprax::model_routing::engine::json::canonical(&p["rendered"]).len(),
                         "usage": {"input_tokens": null, "output_tokens": null, "basis": "unknown"},
                         "billing": "local"},
            })
        } else {
            json!({"choice": first, "scores": {first: 0.8}, "abstain": false})
        };
        DecisionCall::Answered {
            result,
            elapsed_ms: 1,
            call: None,
        }
    }

    fn decision_versions(&self) -> Vec<u32> {
        vec![1, CHOICE_WIRE_VERSION]
    }
}

fn out_of_tree<'a>(
    inv: &'a mut OutOfTreeAdapter,
    task: &str,
) -> ConfiguredProvider<'a, OutOfTreeAdapter> {
    ConfiguredProvider {
        profile: ProviderProfile {
            provider_id: "example.out-of-tree".into(),
            model_id: "first-option".into(),
            checkpoint: "oot-1".into(),
            ..ProviderProfile::default()
        },
        invoker: inv,
        mode: ProviderMode::Explicit,
        gate: EnablementGate::not_evaluated(task, "example.out-of-tree"),
    }
}

#[test]
fn an_out_of_tree_adapter_serves_both_runtime_consumers_unchanged() {
    with_support(|w, config, registry, set| {
        // Consumer 1: runtime choice selection.
        let inputs = support_inputs(config, registry, set, "anything at all");
        let mut inv = OutOfTreeAdapter { calls: 0 };
        let out = select_choice(
            &inputs,
            &ctx("oot"),
            Some(&mut out_of_tree(&mut inv, CHOICE_TASK)),
        );
        let sel = out.selection().expect("selected").clone();
        assert_eq!(sel.id(), "support.billing");
        assert_eq!(
            out.report().wire.call.as_ref().unwrap().adapter,
            "example/out-of-tree@0.1.0"
        );
        authorize_specialist_choice(&sel, &inputs, registry, set).unwrap();

        // Consumer 2: runtime model routing over the approved profiles.
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(set, response(&w.schema, "1"), created.clone());
        let mut store = MemStore::default();
        let routed = start_routed_task(
            set,
            &features(TaskFamily::LocalizedDebug),
            &ctx("oot-route"),
            Some(&mut out_of_tree(&mut inv, "model-route/v1")),
            &w.target(b"t"),
            handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
            &mut store,
        )
        .unwrap();
        assert_eq!(routed.record.source(), RouteSource::Provider);
        assert_eq!(routed.record.decision_provider(), "example.out-of-tree");
        assert_eq!(inv.calls, 2);
    });
}
