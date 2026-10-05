//! TC-05 opt-in context target through the real pipeline (fake compiler,
//! scripted proposer, no network). Fixture prefix `hp-tc05`.

use super::*;
use semaprax_harness::workflow::context_target::TargetConfig;

/// Native facts plus many external slices; one slice names the task identifier.
#[derive(Default)]
struct Bulk {
    fillers: usize,
    follow_ups: u32,
    follow_up_text: Option<&'static str>,
    /// Extra external slices `(label, text)` returned by the initial collection.
    extra: Vec<(&'static str, &'static str)>,
    handle: Option<String>,
    expands: u32,
}

impl ContextStage for Bulk {
    fn id(&self) -> String {
        "org.example/bulk-stage".into()
    }
    fn plans(&self) -> bool {
        true
    }
    fn collect(&mut self, r: &ContextRequest) -> Result<ContextPacket, StageFailure> {
        let mut items = vec![ContextItem {
            label: "native:t.f".into(),
            provenance: COMPILER_VERIFIED.into(),
            text: "native facts".into(),
        }];
        if r.external != ExternalContext::Never {
            for i in 0..self.fillers {
                items.push(ContextItem {
                    label: format!("web/filler{i}.ts:1-1"),
                    provenance: "external:structural".into(),
                    text: format!("unrelated filler {i} {}", "z".repeat(200)),
                });
            }
            for (l, t) in &self.extra {
                items.push(ContextItem {
                    label: (*l).into(),
                    provenance: "external:structural".into(),
                    text: (*t).into(),
                });
            }
            items.push(ContextItem {
                label: "web/hit.ts:1-1".into(),
                provenance: "external:structural".into(),
                text: "HIT-SLICE uses rename".into(),
            });
        }
        Ok(ContextPacket {
            provider: self.id(),
            items,
            complete: true,
        })
    }
    fn calls(&self) -> u32 {
        1 + self.follow_ups
    }
    fn expand(&mut self, _r: &ContextRequest, _h: &str) -> Result<ContextPacket, StageFailure> {
        self.expands += 1;
        Ok(packet(
            &self.id(),
            "src/a.ts:1-2",
            "external:inferred",
            "EXPANDED-SLICE",
        ))
    }
    fn take_plan_report(&mut self) -> Option<Value> {
        self.handle.as_ref().map(|h| json!({"continuation": [h]}))
    }
    fn follow_up(
        &mut self,
        _r: &ContextRequest,
        _f: &str,
    ) -> Result<Option<ContextPacket>, StageFailure> {
        self.follow_ups += 1;
        Ok(match (self.follow_ups, self.follow_up_text) {
            (1, Some(t)) => Some(packet(
                &self.id(),
                "web/api.ts:5-9",
                "external:structural",
                t,
            )),
            _ => None,
        })
    }
}

fn drive(cfg: &RunConfig, stage: &mut Bulk, script: &Script) -> Report {
    let fake = Fake::new(CHANGED);
    let mut prop = ScriptRef(script);
    let mut view = RawCommandView;
    run(
        cfg,
        &fake,
        Stages {
            decision: None,
            native: stage,
            external: None,
            proposer: &mut prop,
            command: &mut view,
        },
        &mut Observer::new(None, ObserverLimits::default()),
    )
}

fn script(items: Vec<Value>) -> Script {
    Script {
        items,
        prompts: RefCell::default(),
        calls: Cell::new(0),
    }
}

#[test]
fn hp_tc05_localized_task_under_a_small_target_passes_the_same_checks_as_the_default() {
    let e = setup(FIXED);
    let run_one = |target: Option<TargetConfig>| {
        let mut cfg = session_cfg(&e);
        cfg.context_target = target;
        let s = script(vec![body(json!({}))]);
        let mut st = Bulk {
            fillers: 40,
            ..Default::default()
        };
        let r = drive(&cfg, &mut st, &s);
        let prompt = s.prompts.borrow()[0].to_string();
        (r, prompt)
    };
    let (base, base_prompt) = run_one(None);
    let (small, small_prompt) = run_one(Some(TargetConfig {
        initial: 400,
        max_escalations: 1,
    }));
    assert_eq!(base.status, "candidate-ready", "{:?}", base.refusals);
    assert_eq!(small.status, base.status, "{:?}", small.refusals);
    assert_eq!(small.checks, base.checks);
    assert!(small_prompt.len() < base_prompt.len());
    assert!(small_prompt.contains("HIT-SLICE") && small_prompt.contains("native facts"));
    assert!(!small_prompt.contains("unrelated filler 39"));
    // Default report is unchanged; the opt-in report carries the audit, not the prompt.
    assert!(base.context.get("target").is_none());
    assert!(!small_prompt.contains("over target"));
    let t = &small.context["target"];
    assert_eq!(t["exhaustive"], false);
    assert!(t["omitted"].as_array().unwrap().len() >= 30);
    assert!(
        small.context["used_bytes"].as_u64().unwrap()
            < base.context["used_bytes"].as_u64().unwrap()
    );
}

#[test]
fn hp_tc05_incomplete_initial_retrieval_escalates_through_the_follow_up_path_within_the_bound() {
    let e = setup(FIXED);
    let failing = || {
        script(vec![
            body(json!({"fake_refuse": "SPX-G225 unknown field on `Gadget`"})),
            body(json!({})),
        ])
    };
    let mut cfg = session_cfg(&e);
    cfg.context_target = Some(TargetConfig {
        initial: 60,
        max_escalations: 1,
    });
    let s = failing();
    let mut st = Bulk {
        fillers: 0,
        follow_up_text: Some("FOLLOWUP-FACT about Gadget"),
        ..Default::default()
    };
    let r = drive(&cfg, &mut st, &s);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(st.follow_ups, 1);
    let p = s.prompts.borrow();
    assert!(!p[0].to_string().contains("FOLLOWUP-FACT"));
    assert!(p[1].to_string().contains("FOLLOWUP-FACT"));
    let t = &r.context["target"];
    assert_eq!(t["escalations"].as_array().unwrap().len(), 1);
    assert_eq!(t["current"], 120);
    assert!(t["escalations"][0]["why"]
        .as_str()
        .unwrap()
        .starts_with("validation-failure:"));

    // Bound of zero: no escalation, the refusal is reported, search stays bounded.
    let mut cfg = session_cfg(&e);
    cfg.context_target = Some(TargetConfig {
        initial: 60,
        max_escalations: 0,
    });
    let s = failing();
    let mut st = Bulk {
        fillers: 0,
        follow_up_text: Some("FOLLOWUP-FACT about Gadget"),
        ..Default::default()
    };
    let r = drive(&cfg, &mut st, &s);
    assert!(st.follow_ups <= 1);
    assert!(r.context["target"]["escalations"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(r.context["target"]["escalation_refused"].is_string());
    assert!(!s
        .prompts
        .borrow()
        .iter()
        .any(|p| p.to_string().contains("FOLLOWUP-FACT")));
}

#[test]
fn hp_tc05_multi_file_rename_keeps_every_required_reference_and_is_not_exhaustive() {
    let e = setup(FIXED);
    let mut cfg = session_cfg(&e);
    cfg.task.goal = "rename `Gadget` to `Widget` in every file".into();
    cfg.context_target = Some(TargetConfig {
        initial: 300,
        max_escalations: 1,
    });
    let s = script(vec![body(json!({}))]);
    let mut st = Bulk {
        fillers: 30,
        extra: vec![
            ("web/use1.ts:1-1", "const a = new Gadget(1); // pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad"),
            ("web/use2.ts:1-1", "import { Gadget } from './g'; // pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad pad"),
            ("required:contract.law", "law: Gadget ids are stable; padding padding padding padding padding padding padding padding padding"),
        ],
        ..Default::default()
    };
    let r = drive(&cfg, &mut st, &s);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let p = s.prompts.borrow()[0].to_string();
    for must in ["web/use1.ts", "web/use2.ts", "required:contract.law"] {
        assert!(p.contains(must), "required reference {must} kept");
    }
    assert!(!p.contains("unrelated filler 29"));
    let t = &r.context["target"];
    assert_eq!(t["exhaustive"], false);
    assert!(t["required_refs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "Gadget"));
    assert!(!t["omitted"].as_array().unwrap().is_empty());
    assert!(t["chosen"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["reason"].as_str().unwrap().contains("required")));
}

struct Words;
impl semaprax_harness::observe::Tokenizer for Words {
    fn name(&self) -> &str {
        "fake-words"
    }
    fn fingerprint(&self) -> &str {
        "fp-w"
    }
    fn count(&self, t: &str) -> usize {
        t.split_whitespace().count()
    }
}

#[test]
fn hp_tc05_named_tokenizer_selects_token_units_and_unknown_falls_back_to_labelled_bytes() {
    let e = setup(FIXED);
    let models = json!([semaprax_harness::decision::ModelPlan {
        id: "m-1".into(),
        destination: semaprax_harness::decision::Destination::Local,
        structured_output: true,
        tools: false,
        max_context: 100_000,
        est_cost_micros: 1,
        est_latency_ms: 1,
        strength_rank: 1,
        descriptor: Default::default(),
    }
    .to_json()]);
    let go_one = |with_tok: bool| {
        let mut cfg = session_cfg(&e);
        cfg.task.models = Some(models.clone());
        cfg.budget.map =
            semaprax_harness::workflow::budget::ModelTokenizerMap::empty().with("m-", "fake-words");
        if with_tok {
            cfg.budget.tokenizers.add(Box::new(Words));
        }
        cfg.context_target = Some(TargetConfig {
            initial: 40,
            max_escalations: 1,
        });
        let s = script(vec![body(json!({}))]);
        let mut st = Bulk {
            fillers: 10,
            ..Default::default()
        };
        drive(&cfg, &mut st, &s)
    };
    let named = go_one(true);
    assert_eq!(named.status, "candidate-ready", "{:?}", named.refusals);
    assert_eq!(named.context["target"]["unit"], "tokens:fake-words");
    let fallback = go_one(false);
    assert_eq!(
        fallback.context["target"]["unit"],
        "bytes:byte-policy-upper-bound"
    );
}

#[test]
fn hp_tc05_handle_expansion_goes_through_the_target_merge_and_escalates() {
    let e = setup(FIXED);
    let mut cfg = session_cfg(&e);
    cfg.context_target = Some(TargetConfig {
        initial: 60,
        max_escalations: 1,
    });
    let s = script(vec![
        body(json!({"fake_refuse": "SPX-G225 mismatch in src/a.ts"})),
        body(json!({})),
    ]);
    let mut st = Bulk {
        handle: Some("ctx:org.example/bulk-stage:src/a.ts#1-2@sha256:00".into()),
        ..Default::default()
    };
    let r = drive(&cfg, &mut st, &s);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(st.expands, 1);
    assert!(s.prompts.borrow()[1].to_string().contains("EXPANDED-SLICE"));
    assert_eq!(
        r.context["target"]["escalations"].as_array().unwrap().len(),
        1
    );
}
