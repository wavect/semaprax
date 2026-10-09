//! HN-15 gated skill evolution: fixture-adapter outcome tests (no real
//! evolution backend ran here; see docs/HARNESS-EVOLUTION-V1.md). Fixture prefix `hp-hn15`.

use crate::support::*;
use semaprax_harness::cli::{self, Environment};
use semaprax_harness::contract::payload::evolve::validate_payload;
use semaprax_harness::contract::payload::Direction::{Request, Result as Res};
use semaprax_harness::evolution::protected::digest_path;
use semaprax_harness::evolution::{
    self, run_experiment, Adapter, AdapterError, Cancel, Outcome, ProcessAdapter,
};
use semaprax_harness::json::sha256_plain;
use semaprax_harness::skills::{ApprovedRoot, SkillCatalogConfig, SkillService};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Instant;

const FAMILY: &str = "compiler-diagnostic/SPX-0001";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Good,
    Harmful,
    NoAction,
    Unavailable,
    ManyCalls,
    CancelInEvolve,
    Tamper,
}

struct Fixture {
    mode: Mode,
    requests: Vec<Value>,
    cancel: Cancel,
    tamper: PathBuf,
}

impl Adapter for Fixture {
    fn call(
        &mut self,
        req: &Value,
        ws: &Path,
        _d: Instant,
        _c: &Cancel,
    ) -> Result<Value, AdapterError> {
        self.requests.push(req.clone());
        if self.mode == Mode::Unavailable {
            return Err(AdapterError::Unavailable("no backend".into()));
        }
        if req.get("family").is_some() {
            let page = "# SPX-0001\nMissing capability: declare it in the effect row.\n";
            let p = ws.join("wiki/patterns");
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("spx-0001.md"), page).unwrap();
            if self.mode == Mode::Tamper {
                std::fs::write(&self.tamper, "tampered").unwrap();
            }
            if self.mode == Mode::CancelInEvolve {
                self.cancel
                    .flag
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
            let mut out = json!({
                "wiki": [{"path": "wiki/patterns/spx-0001.md", "digest": sha256_plain(page.as_bytes())}],
                "model_calls": if self.mode == Mode::ManyCalls { 999 } else { 2 },
                "iterations": 1,
            });
            if self.mode == Mode::NoAction {
                out["no_action_reason"] = json!("no improvement over the baseline is expected");
            } else {
                out["candidate"] = json!({"name": "Declare Effects", "description": "Repair SPX-0001", "body": "When SPX-0001 appears, declare the effect in the signature."});
            }
            return Ok(out);
        }
        let id = req["task"]["id"].as_str().unwrap();
        let with = req.get("skill").is_some();
        let ok = if with {
            self.mode != Mode::Harmful
        } else {
            ["v1", "t1", "t2"].contains(&id)
        };
        Ok(
            json!({"answer": if ok { format!("fix-{id}") } else { "wrong".into() }, "model_calls": 1}),
        )
    }
}

struct Rig {
    dir: PathBuf,
    parent: PathBuf,
    app: PathBuf,
    traces: PathBuf,
}

impl Rig {
    fn new(prefix: &str) -> Self {
        let dir = fixture_dir(prefix);
        let parent =
            repo_root().join("packages/semaprax-harness-adapters/skills/official/ponytail/v4.10.3");
        let app = dir.join("app");
        write(&app, "src/main.spx", "module app\n");
        let line = |id: &str| {
            json!({"schema": "semaprax.evolution-trace.v1", "task_id": id, "family": FAMILY, "kind": "check", "outcome": "fail", "diagnostic_code": "SPX-0001", "message": "missing capability", "repair": "declare the effect"}).to_string()
        };
        let traces = write(
            &dir,
            "traces.jsonl",
            &format!("{}\n{}\n{}\n", line("run-a"), line("run-b"), line("run-c")),
        );
        Self {
            dir,
            parent,
            app,
            traces,
        }
    }

    fn spec_json(&self, retention: &str) -> Value {
        let task = |id: &str, split: &str| json!({"id": id, "split": split, "prompt": format!("repair {id}"), "expected": format!("fix-{id}")});
        json!({
            "schema": "semaprax.evolution-experiment.v1", "id": "exp-1", "family": FAMILY,
            "adapter": {"command": ["/bin/sh", "-c", "exit 1"]},
            "workspace_root": self.dir.join("work"),
            "consent": {"traces": [self.traces], "retention": retention},
            "parent": {"name": "ponytail", "dir": self.parent},
            "protected": [self.app],
            "tasks": [task("tr1", "train"), task("v1", "validation"), task("v2", "validation"), task("v3", "validation"), task("t1", "test"), task("t2", "test")],
            "caps": {"max_iterations": 1, "max_model_calls": 40, "max_seconds": 30},
        })
    }

    fn run(&self, mode: Mode, spec: Value) -> (evolution::Report, Fixture) {
        let sp = evolution::spec::parse(&spec, &self.dir).unwrap();
        let mut fx = Fixture {
            mode,
            requests: vec![],
            cancel: Cancel::default(),
            tamper: self.app.join("src/main.spx"),
        };
        let cancel = fx.cancel.clone();
        let rep = run_experiment(&sp, &mut fx, &cancel, &self.dir.join("elsewhere")).unwrap();
        (rep, fx)
    }
}

fn code(r: Result<evolution::Report, semaprax_harness::diag::HarnessDiagnostic>) -> &'static str {
    r.expect_err("expected a refusal").code
}

#[test]
fn hp_hn15_accepted_candidate_has_derived_identity_and_unchanged_protected_bytes() {
    let rig = Rig::new("hp-hn15-acc");
    let parent_before = digest_path(&rig.parent).unwrap();
    let app_before = digest_path(&rig.app).unwrap();
    let (rep, fx) = rig.run(Mode::Good, rig.spec_json("keep"));
    assert_eq!(rep.outcome, Outcome::Accepted, "{}", rep.result);
    let r = &rep.result;
    let d = &r["derived"];
    assert_eq!(d["identity"], "artifact-v2");
    let dir = rep.workspace.join("derived/derived-declare-effects");
    assert_eq!(
        d["digest"],
        semaprax_harness::evolution::derived::digest_of(&dir).unwrap()
    );
    assert_ne!(
        d["digest"], r["parent"]["digest"],
        "derived identity must differ from the parent"
    );
    let prov: Value =
        serde_json::from_slice(&std::fs::read(dir.join("provenance.json")).unwrap()).unwrap();
    assert_eq!(prov["parent"]["digest"], r["parent"]["digest"]);
    assert_eq!(prov["sources"]["trace_digest"], r["trace_digest"]);
    assert_eq!(prov["scope"]["family"], FAMILY);
    assert_eq!(
        prov["evaluation"]["scores"]["validation"]["candidate"]["passed"],
        3
    );
    assert_eq!(r["promotion"]["status"], "not-promoted");
    assert_eq!(r["promotion"]["auto_promotion"], "disabled");
    assert!(rep.workspace.join("wiki/patterns/spx-0001.md").is_file());
    assert_eq!(r["protected"]["unchanged"], true);
    assert_eq!(digest_path(&rig.parent).unwrap(), parent_before);
    assert_eq!(digest_path(&rig.app).unwrap(), app_before);
    // Sealed held-out answers never reach the adapter; train answers may.
    let sent = fx
        .requests
        .iter()
        .map(|q| q.to_string())
        .collect::<String>();
    assert!(sent.contains("fix-tr1"));
    for held in ["fix-v1", "fix-v2", "fix-v3", "fix-t1", "fix-t2"] {
        assert!(!sent.contains(held), "{held} leaked");
    }
}

#[test]
fn hp_hn15_harmful_candidate_is_rejected_and_only_the_derived_skill_rolls_back() {
    let rig = Rig::new("hp-hn15-harm");
    let (rep, _) = rig.run(Mode::Harmful, rig.spec_json("keep"));
    assert_eq!(rep.outcome, Outcome::Rejected, "{}", rep.result);
    assert!(rep.result["reason"]
        .as_str()
        .unwrap()
        .contains("test regressed"));
    assert!(!rep
        .workspace
        .join("derived/derived-declare-effects")
        .exists());
    assert!(
        rep.workspace.join("wiki/patterns/spx-0001.md").is_file(),
        "wiki must be retained"
    );
    assert!(
        rep.workspace.join("raw/traces.jsonl").is_file(),
        "source evidence must be retained"
    );
    let neg = rep.result["negative_evidence"].as_str().unwrap();
    assert!(std::fs::read_to_string(rep.workspace.join(neg))
        .unwrap()
        .contains("Rejected candidate"));
    assert_eq!(rep.result["protected"]["unchanged"], true);
}

#[test]
fn hp_hn15_no_action_when_backend_declines_or_evidence_is_not_recurrent() {
    let rig = Rig::new("hp-hn15-noact");
    let (rep, fx) = rig.run(Mode::NoAction, rig.spec_json("keep"));
    assert_eq!(rep.outcome, Outcome::NoAction);
    assert!(
        rep.result["wiki"].as_array().unwrap().len() == 1,
        "wiki update still recorded"
    );
    assert!(!rep
        .workspace
        .join("derived")
        .join("derived-declare-effects")
        .exists());
    assert!(!fx.requests.is_empty());
    let mut s = rig.spec_json("keep");
    s["min_traces"] = json!(10);
    let (rep, fx) = rig.run(Mode::Good, s);
    assert_eq!(rep.outcome, Outcome::NoAction);
    assert!(
        fx.requests.is_empty(),
        "no backend call without recurrent evidence"
    );
}

#[test]
fn hp_hn15_absent_backend_is_unavailable_on_the_real_process_path() {
    let rig = Rig::new("hp-hn15-unav");
    let mut s = rig.spec_json("keep");
    s["adapter"]["command"] = json!(["/nonexistent/wikiskill-adapter"]);
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    let mut a = ProcessAdapter {
        command: sp.adapter_command.clone(),
        env: sp.adapter_env.clone(),
    };
    let rep = run_experiment(&sp, &mut a, &Cancel::default(), &rig.dir.join("elsewhere")).unwrap();
    assert_eq!(rep.outcome, Outcome::Unavailable);
    assert_eq!(rep.result["code"], "SPX-HPW004");
    // The adapter itself may report unavailability (exit 69).
    s["adapter"]["command"] = json!([
        "/bin/sh",
        "-c",
        "cat >/dev/null; echo '{\"unavailable\":\"no local model backend\"}'; exit 69"
    ]);
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    let mut a = ProcessAdapter {
        command: sp.adapter_command.clone(),
        env: sp.adapter_env.clone(),
    };
    let rep = run_experiment(&sp, &mut a, &Cancel::default(), &rig.dir.join("elsewhere")).unwrap();
    assert_eq!(rep.outcome, Outcome::Unavailable);
    assert!(rep.result["reason"]
        .as_str()
        .unwrap()
        .contains("no local model backend"));
}

#[test]
fn hp_hn15_real_process_adapter_speaks_the_protocol_and_time_cap_aborts() {
    let rig = Rig::new("hp-hn15-proc");
    let mut s = rig.spec_json("keep");
    s["adapter"]["command"] = json!(["/bin/sh", "-c", "cat >/dev/null; echo '{\"wiki\":[],\"model_calls\":1,\"iterations\":1,\"no_action_reason\":\"nothing to change\"}'"]);
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    let mut a = ProcessAdapter {
        command: sp.adapter_command.clone(),
        env: sp.adapter_env.clone(),
    };
    let rep = run_experiment(&sp, &mut a, &Cancel::default(), &rig.dir.join("elsewhere")).unwrap();
    assert_eq!(rep.outcome, Outcome::NoAction);
    s["adapter"]["command"] = json!(["/bin/sh", "-c", "exec sleep 20"]);
    s["caps"]["max_seconds"] = json!(1);
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    let mut a = ProcessAdapter {
        command: sp.adapter_command.clone(),
        env: sp.adapter_env.clone(),
    };
    let t = Instant::now();
    let rep = run_experiment(&sp, &mut a, &Cancel::default(), &rig.dir.join("elsewhere")).unwrap();
    assert!(t.elapsed().as_secs() < 10);
    assert_eq!(
        (rep.outcome, rep.result["code"].as_str()),
        (Outcome::Aborted, Some("SPX-HPW006"))
    );
    // A marker file cancels a running adapter.
    s["caps"]["max_seconds"] = json!(30);
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    let cancel = Cancel {
        flag: Default::default(),
        file: Some(rig.dir.join("CANCEL")),
    };
    std::fs::write(rig.dir.join("CANCEL"), "").unwrap();
    let mut a = ProcessAdapter {
        command: sp.adapter_command.clone(),
        env: sp.adapter_env.clone(),
    };
    let rep = run_experiment(&sp, &mut a, &cancel, &rig.dir.join("elsewhere")).unwrap();
    assert_eq!(
        (rep.outcome, rep.result["code"].as_str()),
        (Outcome::Aborted, Some("SPX-HPW007"))
    );
}

#[test]
fn hp_hn15_spend_caps_and_cancellation_abort_truthfully() {
    let rig = Rig::new("hp-hn15-caps");
    let (rep, _) = rig.run(Mode::ManyCalls, rig.spec_json("keep"));
    assert_eq!(
        (rep.outcome, rep.result["code"].as_str()),
        (Outcome::Aborted, Some("SPX-HPW006"))
    );
    assert!(rep.workspace.join("wiki/patterns/spx-0001.md").is_file());
    let mut s = rig.spec_json("keep");
    s["caps"]["max_model_calls"] = json!(5);
    let (rep, _) = rig.run(Mode::Good, s);
    assert_eq!(
        (rep.outcome, rep.result["code"].as_str()),
        (Outcome::Aborted, Some("SPX-HPW006"))
    );
    assert!(
        !rep.workspace
            .join("derived/derived-declare-effects")
            .exists(),
        "partial evaluation never leaves a skill"
    );
    let mut s = rig.spec_json("keep");
    s["caps"]["max_iterations"] = json!(0);
    let (rep, _) = rig.run(Mode::Good, s);
    assert_eq!(rep.outcome, Outcome::Aborted);
    let (rep, _) = rig.run(Mode::CancelInEvolve, rig.spec_json("keep"));
    assert_eq!(
        (rep.outcome, rep.result["code"].as_str()),
        (Outcome::Aborted, Some("SPX-HPW007"))
    );
    assert!(!rep
        .workspace
        .join("derived/derived-declare-effects")
        .exists());
}

#[test]
fn hp_hn15_protected_change_aborts_and_rolls_back() {
    let rig = Rig::new("hp-hn15-tamper");
    let (rep, _) = rig.run(Mode::Tamper, rig.spec_json("keep"));
    assert_eq!(
        (rep.outcome, rep.result["code"].as_str()),
        (Outcome::Aborted, Some("SPX-HPW008"))
    );
    assert_eq!(rep.result["protected"]["unchanged"], false);
    assert!(!rep
        .workspace
        .join("derived/derived-declare-effects")
        .exists());
    assert!(rep.result.get("derived").is_none());
}

#[test]
fn hp_hn15_workspace_must_not_overlap_protected_paths() {
    let rig = Rig::new("hp-hn15-overlap");
    let mut s = rig.spec_json("keep");
    s["workspace_root"] = json!(rig.app.join("inside"));
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    let mut fx = Fixture {
        mode: Mode::Good,
        requests: vec![],
        cancel: Cancel::default(),
        tamper: rig.app.join("x"),
    };
    assert_eq!(
        code(run_experiment(
            &sp,
            &mut fx,
            &Cancel::default(),
            &rig.dir.join("elsewhere")
        )),
        "SPX-HPW002"
    );
    assert!(fx.requests.is_empty());
    s["workspace_root"] = json!(rig.dir.join("work"));
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    assert_eq!(
        code(run_experiment(
            &sp,
            &mut fx,
            &Cancel::default(),
            &rig.dir.join("work/exp-1")
        )),
        "SPX-HPW002"
    );
}

#[test]
fn hp_hn15_ingestion_admits_only_public_in_family_evidence() {
    let rig = Rig::new("hp-hn15-ingest");
    let rec = |extra: Value| {
        let mut v = json!({"schema": "semaprax.evolution-trace.v1", "task_id": "run-x", "family": FAMILY, "kind": "check", "outcome": "fail", "message": "m", "cwd": "/Users/someone/private-repo"});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v.to_string()
    };
    let report = json!({"schema": "semaprax.harness-run.v2", "status": "failed", "lineage": "wf-1", "diagnostics": [{"code": "SPX-0001", "message": "missing capability", "path": "/secret/path", "line": 3}], "context": {"hidden": "dropped"}}).to_string();
    let obs = json!({"schema": "semaprax.harness-observation.v1", "provider": "p", "stage": "generation"}).to_string();
    let lines = [
        rec(json!({})),
        rec(json!({"reasoning": "private chain of thought"})),
        rec(json!({"message": "key sk-live-123"})),
        rec(json!({"task_id": "v1"})),
        rec(json!({"message": "answer is fix-v2"})),
        rec(json!({"family": "other"})),
        report,
        obs,
        "not json".to_string(),
    ];
    write(&rig.dir, "mixed.jsonl", &lines.join("\n"));
    let mut s = rig.spec_json("keep");
    s["consent"]["traces"] = json!([rig.dir.join("mixed.jsonl")]);
    let sp = evolution::spec::parse(&s, &rig.dir).unwrap();
    let ing = evolution::trace::ingest(&sp);
    assert_eq!(ing.records.len(), 2, "{:?}", ing.summary());
    assert_eq!(ing.out_of_family, 1);
    assert_eq!(ing.observation_events, 1);
    assert_eq!(ing.refused["forbidden-member:reasoning"], 1);
    assert_eq!(ing.refused["secret-like-value"], 1);
    assert_eq!(ing.refused["heldout-or-sealed-content"], 2);
    assert_eq!(ing.refused["invalid-json"], 1);
    let text = ing.jsonl();
    for leaked in [
        "/Users/someone",
        "/secret/path",
        "dropped",
        "chain of thought",
    ] {
        assert!(
            !text.contains(leaked),
            "{leaked} leaked into ingested traces"
        );
    }
    // No consent, no ingestion.
    s["consent"] = json!({});
    assert_eq!(
        evolution::spec::parse(&s, &rig.dir).err().unwrap().code,
        "SPX-HPW003"
    );
}

#[test]
fn hp_hn15_experiment_only_retention_removes_raw_copies_but_keeps_wiki_and_evidence() {
    let rig = Rig::new("hp-hn15-retain");
    let (rep, _) = rig.run(Mode::Good, rig.spec_json("experiment-only"));
    assert!(!rep.workspace.join("raw").exists());
    assert!(rep.workspace.join("wiki/patterns/spx-0001.md").is_file());
    assert!(rep.workspace.join("evidence/result.json").is_file());
    assert!(rep.result["trace_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
}

#[test]
fn hp_hn15_promotion_is_explicit_and_the_accepted_skill_loads_in_a_later_task() {
    let rig = Rig::new("hp-hn15-promote");
    let (rep, _) = rig.run(Mode::Good, rig.spec_json("keep"));
    let live = rig.dir.join("everyday-skills");
    assert_eq!(
        evolution::promote(&rep.workspace, &live, false)
            .err()
            .unwrap()
            .code,
        "SPX-HPW009"
    );
    assert!(!live.exists(), "nothing is promoted without approval");
    let mut auto = rig.spec_json("keep");
    auto["auto_promote"] = json!(true);
    assert_eq!(
        evolution::spec::parse(&auto, &rig.dir).err().unwrap().code,
        "SPX-HPW009"
    );
    evolution::promote(&rep.workspace, &live, true).unwrap();
    // A later, separate task loads the promoted skill from its own approved root.
    let cfg = SkillCatalogConfig {
        enabled: true,
        select: vec!["derived-declare-effects".into()],
        ..Default::default()
    };
    let mut svc = SkillService::new(
        vec![ApprovedRoot {
            path: live.clone(),
            origin: "promoted".into(),
            approved_digest: None,
        }],
        cfg,
    );
    let p = svc.render_prompt(&[]);
    let digest = rep.result["derived"]["digest"].as_str().unwrap();
    assert_eq!(
        p.loaded,
        vec![("derived-declare-effects".to_string(), digest.to_string())]
    );
    assert!(p.text.contains("declare the effect in the signature"));
    assert!(
        p.text
            .contains(rep.result["parent"]["digest"].as_str().unwrap()),
        "parent digest visible"
    );
    assert!(p.text.contains("Derived skill, not an official snapshot"));
    // Rejected experiments cannot be promoted.
    let rig2 = Rig::new("hp-hn15-promote2");
    let (bad, _) = rig2.run(Mode::Harmful, rig2.spec_json("keep"));
    assert_eq!(
        evolution::promote(&bad.workspace, &rig2.dir.join("live"), true)
            .err()
            .unwrap()
            .code,
        "SPX-HPW009"
    );
}

#[test]
fn hp_hn15_cli_verb_runs_refuses_auto_promote_and_reports_unavailable() {
    let rig = Rig::new("hp-hn15-cli");
    let mut s = rig.spec_json("keep");
    s["adapter"]["command"] = json!(["/nonexistent/adapter"]);
    let spec = write(&rig.dir, "exp.json", &s.to_string());
    let env = Environment {
        cwd: rig.dir.join("elsewhere"),
        ..Default::default()
    };
    let out = cli::run(
        &[
            "evolve".into(),
            "run".into(),
            spec.display().to_string(),
            "--json".into(),
        ],
        &env,
    );
    assert_eq!(out.code, 1);
    assert!(
        out.stdout.contains("\"outcome\":\"unavailable\""),
        "{out:?}"
    );
    let out = cli::run(
        &[
            "evolve".into(),
            "run".into(),
            spec.display().to_string(),
            "--auto-promote".into(),
        ],
        &env,
    );
    assert!(out.stderr.contains("SPX-HPW009"));
    let out = cli::run(
        &[
            "evolve".into(),
            "status".into(),
            rig.dir.join("work/exp-1").display().to_string(),
        ],
        &env,
    );
    assert_eq!(out.code, 0);
    assert!(cli::VERBS.iter().any(|(n, _)| *n == "evolve"));
}

#[test]
fn hp_hn15_skill_evolve_v1_payloads_are_closed() {
    let d = "sha256:".to_string() + &"a".repeat(64);
    let ok = json!({"experiment": "e", "family": "f", "trace_path": "raw/traces.jsonl", "trace_digest": d,
        "parent": {"name": "p", "digest": d}, "caps": {"max_iterations": 1, "max_model_calls": 2, "max_seconds": 3},
        "train_tasks": [{"id": "a", "prompt": "p", "expected": "x"}]});
    assert!(validate_payload("evolve", Request, &ok).is_ok());
    let mut bad = ok.clone();
    bad["trace_path"] = json!("../escape");
    assert_eq!(
        validate_payload("evolve", Request, &bad)
            .err()
            .unwrap()
            .code,
        "SPX-HPA041"
    );
    let leak = json!({"task": {"id": "a", "prompt": "p", "expected": "x"}});
    assert_eq!(
        validate_payload("solve", Request, &leak)
            .err()
            .unwrap()
            .code,
        "SPX-HPA040"
    );
    let both = json!({"wiki": [], "model_calls": 0, "iterations": 1});
    assert!(
        validate_payload("evolve", Res, &both).is_err(),
        "candidate or no_action_reason required"
    );
    assert_eq!(
        validate_payload("promote", Request, &json!({}))
            .err()
            .unwrap()
            .code,
        "SPX-HPA046"
    );
}

#[test]
fn sg16_process_deadline_covers_request_write_and_inherited_stdout() {
    use std::time::Duration;
    let rig = Rig::new("hp-hn15-sg16");
    // The child requests cancellation at the exercised I/O phase, rather
    // than racing a sleeping host thread against the timeout deadline.
    for (timeout_script, cancellation_script) in [
        ("sleep 20", "touch SG16-CANCEL; sleep 20"),
        (
            "cat >/dev/null; sleep 20 & echo '{}'",
            "cat >/dev/null; (touch SG16-CANCEL; sleep 20) & echo '{}'",
        ),
        (
            "cat >/dev/null; sleep 20",
            "cat >/dev/null; touch SG16-CANCEL; sleep 20",
        ),
    ] {
        for marker_cancel in [false, true] {
            let marker = rig.dir.join("SG16-CANCEL");
            let _ = std::fs::remove_file(&marker);
            let cancel = Cancel {
                flag: Default::default(),
                file: marker_cancel.then(|| marker.clone()),
            };
            let script = if marker_cancel {
                cancellation_script
            } else {
                timeout_script
            };
            let mut adapter = ProcessAdapter {
                command: vec!["/bin/sh".into(), "-c".into(), script.into()],
                env: [("PATH".into(), "/usr/bin:/bin".into())].into(),
            };
            let start = Instant::now();
            let result = adapter.call(
                &json!({"prompt": "x".repeat(263_150)}),
                &rig.dir,
                // Cancellation has its own watchdog; the strict timeout
                // cases still exercise the original 150 ms deadline.
                start
                    + if marker_cancel {
                        Duration::from_secs(20)
                    } else {
                        Duration::from_millis(150)
                    },
                &cancel,
            );
            assert_eq!(
                result,
                Err(if marker_cancel {
                    AdapterError::Cancelled
                } else {
                    AdapterError::Timeout
                })
            );
            assert!(start.elapsed() < Duration::from_secs(2));
            if marker_cancel {
                assert!(marker.exists(), "the running child requested cancellation");
            }
        }
    }
    let mut adapter = ProcessAdapter {
        command: vec![
            "/bin/sh".into(),
            "-c".into(),
            "touch should-not-spawn".into(),
        ],
        env: Default::default(),
    };
    assert_eq!(
        adapter.call(&json!({}), &rig.dir, Instant::now(), &Cancel::default()),
        Err(AdapterError::Timeout)
    );
    assert!(!rig.dir.join("should-not-spawn").exists());
}
