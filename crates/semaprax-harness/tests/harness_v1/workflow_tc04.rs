//! TC-04: ordered prompt rendering and provider cache boundaries. A capture
//! stage builds the wire payload with `HostModel::request_payload_cached` (the
//! code `HostModel::propose` runs) and answers with a canned proposal; no
//! network and no paid call. Live cache receipts are TC-12.

use super::*;
use semaprax_harness::contract::{validate_payload, CapabilityKind, Direction};
use semaprax_harness::decision::{Destination, ModelPlan};
use semaprax_harness::receipt::{GenerationSupport, Support};
use semaprax_harness::workflow::budget::BudgetPolicy;
use semaprax_harness::workflow::generation::GenerationPolicy;
use semaprax_harness::workflow::prompt_render::{
    prefix_identity, render_ordered, rendered_text, PrefixBinding, PromptRenderer,
};

const CHANGED: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    x + 0\n}\n";

struct Cap {
    wire: RefCell<Vec<Value>>,
    support: Support,
}

struct CapRef<'a>(&'a Cap);

impl ProposalStage for CapRef<'_> {
    fn id(&self) -> String {
        "org.example/capture-model".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        let p = HostModel::request_payload_cached(r, &self.id(), self.0.support);
        validate_payload(
            CapabilityKind::ModelGenerate,
            "generate",
            Direction::Request,
            &p,
        )
        .expect("valid wire request");
        self.0.wire.borrow_mut().push(p);
        Ok(json!({"schema": "semaprax.harness-proposal.v1",
                  "intent": {"kind": "replace_function_body", "target": "t.f"}})
        .to_string()
        .into_bytes())
    }
    fn calls(&self) -> u32 {
        self.0.wire.borrow().len() as u32
    }
    fn side_effecting(&self) -> bool {
        true
    }
}

fn decode(s: &str) -> String {
    let v = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    } as u32;
    let b: Vec<u8> = s.bytes().filter(|c| *c != b'=').collect();
    let mut out = Vec::new();
    for c in b.chunks(4) {
        let n = c
            .iter()
            .enumerate()
            .fold(0u32, |a, (i, x)| a | v(*x) << (18 - 6 * i));
        for i in 0..(c.len() - 1) {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    String::from_utf8(out).unwrap()
}

fn task() -> Task {
    Task {
        schema_version: 2,
        mode: TaskMode::Change,
        goal: "rename f and keep behavior".into(),
        seed: Some("t.f".into()),
        models: Some(json!([ModelPlan {
            id: "m-a".into(),
            destination: Destination::Local,
            structured_output: true,
            tools: false,
            max_context: 1_000_000,
            est_cost_micros: 0,
            est_latency_ms: 10,
            strength_rank: 1,
        }
        .to_json()])),
        budget: Some(BudgetPolicy {
            output_reserve_tokens: 100,
            protocol_overhead_tokens: 20,
            ..Default::default()
        }),
        ..Task::default()
    }
}

/// One run; returns (report, the wire payloads).
fn run_with_renderer(r: PromptRenderer, support: Support) -> (Report, Vec<Value>) {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut cfg = config(&e, task(), None);
    cfg.budget.generation = GenerationPolicy {
        renderer: r,
        ..Default::default()
    };
    let cap = Cap {
        wire: RefCell::default(),
        support,
    };
    let mut native = NativeContext::new(&fake);
    let mut p = CapRef(&cap);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let rep = run(
        &cfg,
        &fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    );
    let w = cap.wire.borrow().clone();
    (rep, w)
}

fn legacy(attempt: u32, feedback: &str) -> Value {
    json!({"schema": "semaprax.harness-prompt.v1", "revision": "r1", "goal": "g", "seed": "t.f",
           "diagnostics": "d", "intents": ["replace_function_body"],
           "context": [{"label": "src", "provenance": "native", "text": "fn f() {}"}],
           "mode": "change", "task_family": "bugfix", "acceptance": {"tests": ["t"]},
           "attempt": attempt, "feedback": [feedback], "skills": "skill text"})
}

fn binding<'a>(project: &'a str, lock: &'a str, model: &'a str) -> PrefixBinding<'a> {
    PrefixBinding {
        provider: "p",
        model,
        project,
        worktree: "w",
        lock_digest: lock,
    }
}

#[test]
fn tc04_attempts_differing_in_number_and_errors_share_a_byte_identical_prefix_but_not_the_request()
{
    let skills = vec!["s@1".to_string()];
    let a = render_ordered(&legacy(1, ""), &skills);
    let b = render_ordered(&legacy(2, "error[E0308] mismatched types"), &skills);
    let (ta, tb) = (rendered_text(&a).unwrap(), rendered_text(&b).unwrap());
    let prefix = |p: &Value| {
        p["segments"].as_array().unwrap()[..2]
            .iter()
            .map(|s| s["text"].as_str().unwrap())
            .collect::<String>()
    };
    assert_eq!(prefix(&a), prefix(&b));
    assert!(ta.starts_with(&prefix(&a)) && tb.starts_with(&prefix(&b)));
    assert_ne!(sha256_plain(ta.as_bytes()), sha256_plain(tb.as_bytes()));
    let bd = binding("proj", "lock", "m");
    assert_eq!(prefix_identity(&a, &bd), prefix_identity(&b, &bd));
    // The changing facts live in the suffix, never in the prefix.
    assert!(tb.contains("attempt: 2") && tb.contains("mismatched types"));
    assert!(!prefix(&b).contains("mismatched types") && !prefix(&b).contains("attempt"));
}

#[test]
fn tc04_skill_source_schema_authority_model_and_project_each_change_the_prefix_identity() {
    let skills = vec!["s@1".to_string()];
    let base = render_ordered(&legacy(1, ""), &skills);
    let id = |p: &Value, b: &PrefixBinding| prefix_identity(p, b).unwrap();
    let b0 = binding("proj", "lock", "m");
    let base_id = id(&base, &b0);
    let mut seen = vec![base_id.clone()];
    let mut skill_text = legacy(1, "");
    skill_text["skills"] = json!("other skill text");
    let mut source = legacy(1, "");
    source["context"][0]["text"] = json!("fn f() { 1 }");
    let mut accept = legacy(1, "");
    accept["acceptance"] = json!({"tests": ["t", "u"]});
    let mut shape = legacy(1, "");
    shape["scratch_repair"] = json!(true); // a different response schema
    for p in [&skill_text, &source, &accept, &shape] {
        seen.push(id(&render_ordered(p, &skills), &b0));
    }
    seen.push(id(
        &render_ordered(&legacy(1, ""), &["s@2".to_string()]),
        &b0,
    ));
    seen.push(id(&base, &binding("proj", "lock2", "m"))); // authority/lock boundary
    seen.push(id(&base, &binding("proj", "lock", "m2"))); // model
    seen.push(id(&base, &binding("proj2", "lock", "m"))); // cross-project isolation
    let n = seen.len();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), n, "every change must give a distinct identity");
    // A revision change alone does not.
    let mut rev = legacy(1, "");
    rev["revision"] = json!("r2");
    assert_eq!(id(&render_ordered(&rev, &skills), &b0), base_id);
    // A canonical prompt has no prefix identity.
    assert!(prefix_identity(&legacy(1, ""), &b0).is_none());
}

fn payload_input(w: &Value) -> String {
    decode(w["input_base64"].as_str().unwrap())
}

#[test]
fn tc04_the_capture_adapter_sees_the_boundary_only_for_supported_models_and_budgets_count_the_rendered_request(
) {
    let (r, w) = run_with_renderer(PromptRenderer::OrderedV1, Support::Supported);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let seg = &w[0]["segments"];
    assert_eq!(seg["renderer"], "ordered-v1");
    assert_eq!(seg["cache_boundary_after"], "task");
    let text = payload_input(&w[0]);
    let total: u64 = seg["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["bytes"].as_u64().unwrap())
        .sum();
    assert_eq!(total as usize, text.len());
    // Budgets count the rendered request, not the canonical JSON.
    assert_eq!(r.context["request_budget"]["request_bytes"], text.len());

    for support in [Support::Unsupported, Support::Unknown] {
        let (r, w) = run_with_renderer(PromptRenderer::OrderedV1, support);
        assert_eq!(r.status, "candidate-ready");
        assert!(
            w[0].get("segments").is_none(),
            "no boundary for {support:?}"
        );
        // Cold one-off / undeclared: the same self-contained rendered text, uncached.
        assert_eq!(payload_input(&w[0]), text);
    }
    let _ = GenerationSupport::default();
}

#[test]
fn tc04_the_default_rendering_is_the_unchanged_canonical_json_and_has_no_segments() {
    let (r, w) = run_with_renderer(PromptRenderer::Canonical, Support::Supported);
    assert_eq!(r.status, "candidate-ready");
    assert!(w[0].get("segments").is_none());
    let text = payload_input(&w[0]);
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["schema"], "semaprax.harness-prompt.v1");
    assert_eq!(semaprax_harness::json::canonical(&v), text);
    assert_eq!(r.context["request_budget"]["request_bytes"], text.len());
}

#[test]
fn tc04_the_stateless_fallback_carries_every_fact_of_the_canonical_request() {
    let (_, canon) = run_with_renderer(PromptRenderer::Canonical, Support::Unknown);
    let (_, ordered) = run_with_renderer(PromptRenderer::OrderedV1, Support::Unknown);
    let legacy: Value = serde_json::from_str(&payload_input(&canon[0])).unwrap();
    let text = payload_input(&ordered[0]);
    let mut checked = 0;
    for (k, v) in legacy.as_object().unwrap() {
        if matches!(k.as_str(), "schema" | "scratch_repair") || v.is_null() {
            continue;
        }
        let needle = semaprax_harness::json::canonical(v);
        assert!(
            text.contains(&format!("{k}: {needle}")),
            "missing fact `{k}`: {needle}"
        );
        checked += 1;
    }
    assert!(checked >= 6, "{legacy}");
}

#[test]
fn tc04_the_segments_member_is_closed_and_legacy_requests_stay_valid() {
    let ok = |p: &Value| {
        validate_payload(
            CapabilityKind::ModelGenerate,
            "generate",
            Direction::Request,
            p,
        )
    };
    let input = "aGVsbG8="; // 5 bytes
    let base = json!({"model": "m-a", "input_base64": input, "max_output_bytes": 10});
    assert!(ok(&base).is_ok(), "legacy shape");
    let seg = |bytes: u64, after: &str| {
        json!({"renderer": "ordered-v1", "prefix_identity": "sha256:ab",
        "items": [{"id": "task", "bytes": bytes}], "cache_boundary_after": after})
    };
    let mut p = base.clone();
    p["segments"] = seg(5, "task");
    assert!(ok(&p).is_ok());
    p["segments"] = seg(6, "task");
    assert!(ok(&p).is_err(), "sizes must sum to the input");
    p["segments"] = seg(5, "nope");
    assert!(ok(&p).is_err(), "boundary must name an item");
    p["segments"] = seg(5, "task");
    p["segments"]["extra"] = json!(1);
    assert!(ok(&p).is_err(), "closed schema");
}

#[test]
fn tc04_no_cache_fixture_reports_no_cache_savings_and_renderer_config_is_opt_in() {
    let (r, w) = run_with_renderer(PromptRenderer::OrderedV1, Support::Unsupported);
    assert!(w[0].get("segments").is_none());
    let blob = r.context.to_string();
    assert!(
        !blob.contains("cache_savings"),
        "no savings claim without a provider receipt"
    );
    // The capture stage returns no usage, so no cache tier is reported as read.
    let u = &r.context["usage_receipts"]["entries"][0]["receipt"]["usage"];
    assert!(!u["cache_read"].as_u64().is_some_and(|n| n > 0), "{u}");
    assert_eq!(PromptRenderer::default(), PromptRenderer::Canonical);
    assert_eq!(
        GenerationPolicy::default().renderer,
        PromptRenderer::Canonical
    );
}
