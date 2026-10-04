//! HP-17 benchmark tests (fixture prefix `hp-hp17`). No real vendor tool and no
//! model: the third-party example adapter and seeded adapters stand in. A real
//! compiler is needed for the cell tests (marked `ignore`, run with
//! `$SEMAPRAX_COMPILER` set); without one, workflow cells are `untested`.

use crate::support::{fixture_dir, repo_root};
use semaprax_harness::bench::adversarial::{run_adversarial, AdvEnv};
use semaprax_harness::bench::corpus::{tree_digest, Corpus, FAMILIES};
use semaprax_harness::bench::measure::TickClock;
use semaprax_harness::bench::run::{run_matrix, RunOptions, RunOutput};
use semaprax_harness::bench::{gates, report_md, summary};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::observe::{
    build_report, Observation, Observer, ObserverLimits, Role, Stage, TokenCount,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    repo_root().join("crates/semaprax-harness/tests/fixtures/bench")
}

fn python() -> String {
    if let Ok(p) = std::env::var("HARNESS_PYTHON") {
        return p;
    }
    let out = std::process::Command::new("/usr/bin/which")
        .arg("python3")
        .output()
        .expect("which");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn compiler() -> Option<PathBuf> {
    std::env::var_os("SEMAPRAX_COMPILER").map(PathBuf::from)
}

fn vars() -> BTreeMap<String, String> {
    BTreeMap::from([("HARNESS_PYTHON".to_string(), python())])
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if p.is_dir() {
            copy_tree(&p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap();
        }
    }
}

fn matrix(corpus: &Corpus, work: &Path, profiles: &[&str]) -> RunOutput {
    let clock = TickClock::new();
    run_matrix(&RunOptions {
        corpus,
        repo: repo_root(),
        work: work.to_path_buf(),
        vars: vars(),
        compiler: compiler(),
        profiles: profiles.iter().map(|s| s.to_string()).collect(),
        warm: Some(1),
        clock: &clock,
        measure_with: None,
        adversarial: false,
    })
}

#[test]
fn hp_hp17_corpus_schema_pins_and_refusals() {
    let c = Corpus::load(&corpus_dir()).expect("shipped corpus validates");
    let fams: std::collections::BTreeSet<&str> =
        c.tasks.iter().map(|t| t.family.as_str()).collect();
    assert_eq!(fams.len(), FAMILIES.len(), "every family has a task");
    assert!(c.tasks.iter().all(|t| !t.publish && !t.checks.is_empty()));
    assert_eq!(c.profiles.iter().filter(|p| p.baseline).count(), 1);
    for p in c.projects.values() {
        assert_eq!(tree_digest(&p.dir).unwrap(), p.digest);
    }

    // A drifted fixture is refused before any cell runs.
    let root = fixture_dir("hp-hp17").canonicalize().unwrap();
    let copy = root.join("corpus");
    copy_tree(&corpus_dir(), &copy);
    std::fs::write(
        copy.join("projects/downstream/src/lib.spx"),
        "module ledger.lib;\n",
    )
    .unwrap();
    assert_eq!(Corpus::load(&copy).unwrap_err().code, "SPX-HPQ004");

    // Unknown members, publication authority and duplicate ids are refused.
    let n = std::cell::Cell::new(0);
    let edit = |f: &dyn Fn(&mut Value)| {
        n.set(n.get() + 1);
        let dir = root.join(format!("c{}", n.get()));
        copy_tree(&corpus_dir(), &dir);
        let mut v: Value =
            serde_json::from_slice(&std::fs::read(dir.join("corpus.json")).unwrap()).unwrap();
        f(&mut v);
        std::fs::write(dir.join("corpus.json"), v.to_string()).unwrap();
        Corpus::load(&dir)
    };
    assert_eq!(
        edit(&|v| v["tasks"][0]["surprise"] = json!(1))
            .unwrap_err()
            .code,
        "SPX-HPQ003"
    );
    assert_eq!(
        edit(&|v| v["tasks"][0]["authority"]["publish"] = json!(true))
            .unwrap_err()
            .code,
        "SPX-HPQ002"
    );
    assert_eq!(
        edit(&|v| v["tasks"][1]["id"] = v["tasks"][0]["id"].clone())
            .unwrap_err()
            .code,
        "SPX-HPQ005"
    );
    assert_eq!(
        edit(&|v| v["tasks"][0]["project"] = json!("nope"))
            .unwrap_err()
            .code,
        "SPX-HPQ006"
    );
    assert_eq!(
        edit(&|v| v["tasks"][0]["family"] = json!("vibes"))
            .unwrap_err()
            .code,
        "SPX-HPQ002"
    );
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER (native .spx facts come from the real compiler)"]
fn hp_hp17_deterministic_cells_third_party_adapter_and_reconciliation() {
    let corpus = Corpus::load(&corpus_dir()).unwrap();
    let work = fixture_dir("hp-hp17").canonicalize().unwrap();
    let a = matrix(&corpus, &work.join("a"), &["native+source-index"]);
    let b = matrix(&corpus, &work.join("b"), &["native+source-index"]);
    // Injected clock: the whole result set is reproducible byte for byte.
    let canon = |o: &RunOutput| {
        o.cells
            .iter()
            .map(semaprax_harness::json::canonical)
            .collect::<Vec<_>>()
    };
    assert_eq!(a.cells.len(), b.cells.len());
    for (x, y) in a.cells.iter().zip(&b.cells) {
        let diff: Vec<String> = x
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, v)| &y[k.as_str()] != *v)
            .map(|(k, v)| format!("{k}: {v} vs {}", y[k.as_str()]))
            .collect();
        assert!(
            diff.is_empty(),
            "{} {} trial {} differs: {diff:?}",
            x["profile"],
            x["task"],
            x["trial"]
        );
    }
    assert_eq!(canon(&a), canon(&b));
    assert!(!a.cells.is_empty());

    // Same contract, no vendor branch: the third-party adapter's cells ran
    // through adopt + trust and answered `complete`.
    let ext: Vec<&Value> = a
        .cells
        .iter()
        .filter(|c| c["profile"] == "native+source-index")
        .collect();
    assert!(!ext.is_empty());
    assert!(
        ext.iter()
            .all(|c| c["status"] == "ok" && c["provider_status"] == "complete"),
        "{ext:?}"
    );
    let callers = ext
        .iter()
        .find(|c| c["task"] == "refactor-multiply-callers")
        .unwrap();
    assert_eq!(callers["accepted"], true, "{callers}");
    let base = a
        .cells
        .iter()
        .find(|c| c["profile"] == "native-only" && c["task"] == "refactor-multiply-callers")
        .unwrap();
    assert_eq!(
        base["accepted"], false,
        "native context alone cannot list callers"
    );
    assert!(a.cells.iter().any(|c| c["profile"] == "native-only"
        && c["task"] == "route-semantic-law"
        && c["accepted"] == true));

    let s = summary::build(&a, "native-only", &corpus.digest, (corpus.cold, 1));
    for p in ["native-only", "native+source-index"] {
        assert_eq!(
            s["profiles"][p]["reconciliation_mismatches"],
            json!([]),
            "{p}"
        );
        // Workflow traffic is declared unobserved, so no whole-task claim is allowed.
        assert_eq!(
            s["profiles"][p]["observation_report"]["whole_task_claim_allowed"],
            false
        );
        assert_eq!(s["profiles"][p]["bytes"]["unit"], "byte-v1");
    }
    assert_eq!(s["measurement"]["tokenizer"], "byte_only");
}

#[test]
fn hp_hp17_untested_cells_are_not_wins() {
    let corpus = Corpus::load(&corpus_dir()).unwrap();
    let work = fixture_dir("hp-hp17").canonicalize().unwrap();
    let clock = TickClock::new();
    // Tool variables absent: every tool profile is untested, with a reason.
    let out = run_matrix(&RunOptions {
        corpus: &corpus,
        repo: repo_root(),
        work,
        vars: BTreeMap::from([("HARNESS_PYTHON".to_string(), python())]),
        compiler: compiler(),
        profiles: vec![
            "native+graft".into(),
            "rtk".into(),
            "laya".into(),
            "local-efficient".into(),
        ],
        warm: Some(0),
        clock: &clock,
        measure_with: None,
        adversarial: false,
    });
    for p in ["native+graft", "rtk", "laya", "local-efficient"] {
        let cs: Vec<&Value> = out.cells.iter().filter(|c| c["profile"] == p).collect();
        assert!(!cs.is_empty(), "{p}");
        assert!(
            cs.iter()
                .all(|c| c["status"] == "untested" && c["accepted"] == false),
            "{p}"
        );
        assert!(cs[0]["reason"].as_str().unwrap().starts_with("untested:"));
    }
    assert!(out.untested_profiles["laya"].contains("resource (disk)"));
    let s = summary::build(&out, "native-only", &corpus.digest, (1, 0));
    let g = gates::evaluate(&out.cells, &[], &s, "native-only");
    for (_, scopes) in g["scopes"].as_object().unwrap() {
        for (_, sc) in scopes.as_object().unwrap() {
            assert_eq!(sc["auto_enable_eligible"], false);
        }
    }
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER (native .spx facts come from the real compiler)"]
fn hp_hp17_seeded_adversarial_cases_are_detected() {
    let corpus = Corpus::load(&corpus_dir()).unwrap();
    let work = fixture_dir("hp-hp17").canonicalize().unwrap();
    let v = vars();
    let results = run_adversarial(&AdvEnv {
        corpus: &corpus,
        repo: &repo_root(),
        work: &work,
        vars: &v,
        compiler: compiler(),
    });
    let kinds: std::collections::BTreeSet<&str> = results.iter().map(|r| r.kind.as_str()).collect();
    for k in [
        "missing_callers",
        "hidden_critical_error",
        "stale_graph",
        "wrong_router_choice",
        "double_execution",
        "law_weakening",
        "permission_widening",
    ] {
        assert!(kinds.contains(k), "seeded kind {k} present");
    }
    for r in &results {
        assert!(r.untested.is_none(), "{}: {:?}", r.id, r.untested);
        assert!(r.detected, "{} not detected: {:?}", r.id, r.evidence);
    }
}

#[test]
fn hp_hp17_stage_reconciliation_has_no_double_counting() {
    // 1000 -> 800 -> 700 is 300 fewer end to end, not 500; incurred is subtracted once.
    let mut obs = Observer::new(None, ObserverLimits::default());
    let ev = |stage, before: u64, after: u64, last: bool| {
        let mut e = Observation::new("p", "c", stage, Role::Transform, "inv-1");
        e.payload_id = Some("pay".into());
        e.before = Some(TokenCount::bytes(before));
        e.after = Some(TokenCount::bytes(after));
        e.model_visible = last;
        e
    };
    obs.record(ev(Stage::ContextSelect, 1000, 800, false));
    obs.record(ev(Stage::Compression, 800, 700, true));
    let mut inc = Observation::new("p", "c", Stage::RetrievalWrapper, Role::Incurred, "inv-2");
    inc.incurred = Some(TokenCount::bytes(100));
    obs.record(inc);
    let report = build_report(obs.events(), 0, None).json;
    let g = &report["groups"][0];
    assert_eq!(g["end_to_end_reduction"], 300);
    assert_eq!(g["net_savings"], 200);
    let cell = json!({"status": "ok", "bytes": {"baseline": 1000, "final_paired": 700}});
    assert!(summary::reconcile(&[&cell], &report).is_empty());
    // A cell set that disagrees with the observer is reported, not hidden.
    let off = json!({"status": "ok", "bytes": {"baseline": 1000, "final_paired": 650}});
    assert!(!summary::reconcile(&[&off], &report).is_empty());
}

#[test]
fn hp_hp17_gates_are_predeclared_and_decide_eligibility() {
    let doc = std::fs::read_to_string(repo_root().join("docs/HARNESS-BENCHMARK-V1.md"))
        .unwrap()
        .replace('\n', " ");
    for needle in [
        "at least 10 matched cells",
        "at least 20%",
        "at most 2000 ms",
        "No permission widening",
        "No hidden command replay",
        "zero observed failures is not proof",
    ] {
        assert!(doc.contains(needle), "doc declares `{needle}`");
    }
    assert_eq!(gates::MIN_MATCHED_CELLS, 10);
    assert_eq!(gates::MIN_NET_BYTE_REDUCTION, 0.20);
    assert_eq!(gates::MAX_ADDED_LATENCY_MS, 2000.0);
    assert_eq!(gates::QUALITY_TOLERANCE, 0.0);
    assert_eq!(gates::GATES.len(), 7);

    let cell = |profile: &str, trial: u32, vis: u64, acc: bool, lat: u64| {
        json!({"profile": profile, "family": "orientation", "task": "t", "trial": trial, "status": "ok", "accepted": acc,
               "bytes": {"model_visible": vis}, "latency_ms": lat, "false_negatives": {"count": 0}, "command": {"executions": null}})
    };
    let adv_ok = vec![
        json!({"kind": "double_execution", "detected": true, "untested": null}),
        json!({"kind": "permission_widening", "detected": true, "untested": null}),
    ];
    let s = json!({"profiles": {"cand": {"permission_changed_during_run": false}}});
    let eligible = |cells: &[Value], adv: &[Value]| {
        gates::evaluate(cells, adv, &s, "base")["scopes"]["cand"]["orientation"]
            ["auto_enable_eligible"]
            .clone()
    };
    let mk = |vis_p: u64, acc_p: bool| {
        let mut cells = vec![];
        for t in 0..10 {
            cells.push(cell("base", t, 1000, true, 10));
            cells.push(cell("cand", t, vis_p, acc_p, 20));
        }
        cells
    };
    assert_eq!(
        eligible(&mk(500, true), &adv_ok),
        true,
        "50% fewer bytes, same quality, small latency: eligible"
    );
    assert_eq!(
        eligible(&mk(900, true), &adv_ok),
        false,
        "10% saving misses G5"
    );
    assert_eq!(
        eligible(&mk(500, false), &adv_ok),
        false,
        "quality loss misses G4"
    );
    let few = vec![
        cell("base", 0, 1000, true, 10),
        cell("cand", 0, 100, true, 10),
    ];
    assert_eq!(
        eligible(&few, &adv_ok),
        false,
        "fewer than ten matched cells never passes"
    );
    let adv_bad = vec![json!({"kind": "stale_graph", "detected": false, "untested": null})];
    assert_eq!(
        eligible(&mk(500, true), &adv_bad),
        false,
        "an undetected seeded case fails G1"
    );
}

#[test]
fn hp_hp17_report_labels_and_no_vendor_special_cases() {
    let corpus = Corpus::load(&corpus_dir()).unwrap();
    let work = fixture_dir("hp-hp17").canonicalize().unwrap();
    let out = matrix(&corpus, &work, &["native+source-index", "laya"]);
    let s = summary::build(&out, "native-only", &corpus.digest, (1, 1));
    let g = gates::evaluate(&out.cells, &[], &s, "native-only");
    let env = json!({"os": "test-os"});
    let md = report_md::render("test", &s, &g, &Value::Null, &env);
    for label in [
        "byte-v1",
        "no token counts are reported",
        "Protocol-only hosted support",
        "Untested platforms",
        "Historical evidence",
        "native-only stays the default",
        "Zero observed failures is not proof",
    ] {
        assert!(md.contains(label), "report mentions `{label}`");
    }
    let pilot = json!({"status": "ran", "label": "pilot-only", "model": "m", "model_digest": "d", "calls": 3,
                       "min_trials_per_configuration": 3, "caveat": "small local model", "configurations": []});
    assert!(report_md::render("test", &s, &g, &pilot, &env).contains("pilot-only"));

    // No product or vendor name appears in benchmark core.
    fn scan(dir: &Path) {
        for f in std::fs::read_dir(dir).unwrap().flatten() {
            if f.path().is_dir() {
                scan(&f.path());
                continue;
            }
            let text = std::fs::read_to_string(f.path()).unwrap().to_lowercase();
            for name in ["graft", "graphify", "rtk", "laya"] {
                assert!(
                    !text.contains(name),
                    "{} names a vendor: {name}",
                    f.path().display()
                );
            }
        }
    }
    scan(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bench"));
}

#[test]
fn hp_hp17_cli_usage_and_unknown_profile() {
    let root = fixture_dir("hp-hp17").canonicalize().unwrap();
    let env = Environment {
        harness_home: Some(root.join("home")),
        compiler: None,
        cwd: root.clone(),
        vars: BTreeMap::new(),
    };
    let a = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(run(&a(&["bench"]), &env).code, 2);
    let c = corpus_dir();
    assert_eq!(
        run(&a(&["bench", c.to_str().unwrap(), "--bogus"]), &env).code,
        2
    );
    let o = run(
        &a(&["bench", c.to_str().unwrap(), "--profile", "nope"]),
        &env,
    );
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("SPX-HPQ006"), "{}", o.stderr);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER (native .spx facts come from the real compiler)"]
fn hp_hp17_skill_profile_adopts_root_and_compiler_still_gates() {
    let corpus = Corpus::load(&corpus_dir()).unwrap();
    let work = fixture_dir("hp-hp17").canonicalize().unwrap();
    let out = matrix(&corpus, &work, &["native+skill"]);
    let cells: Vec<&Value> = out
        .cells
        .iter()
        .filter(|c| c["profile"] == "native+skill")
        .collect();
    let reuse: Vec<&&Value> = cells
        .iter()
        .filter(|c| c["family"] == "api_reuse")
        .collect();
    assert!(!reuse.is_empty());
    for c in &reuse {
        assert_eq!(c["status"], "ok", "{c}");
        assert_eq!(c["skill"]["loaded"], json!(["reuse-before-generation"]));
        assert!(c["skill"]["prompt_bytes"].as_u64().unwrap() > 0);
        // The skill prompt is a counted incurred cost, never free.
        assert!(
            c["bytes"]["incurred"].as_u64().unwrap()
                >= c["skill"]["prompt_bytes"].as_u64().unwrap()
        );
    }
    if compiler().is_some() {
        let wf: Vec<&&Value> = cells
            .iter()
            .filter(|c| c["workflow_status"].is_string())
            .collect();
        assert!(!wf.is_empty());
        for c in &wf {
            let dep = c["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|k| k["kind"] == "no-dependency-change")
                .unwrap();
            assert_eq!(dep["pass"], true, "manifest untouched: {c}");
        }
        let bad = cells
            .iter()
            .find(|c| c["task"] == "law-bad-body-rejected")
            .unwrap();
        assert_eq!(bad["workflow_status"], "rejected");
        assert_eq!(
            bad["accepted"], true,
            "a failing candidate is still rejected by the compiler's checks"
        );
        let weak = cells
            .iter()
            .find(|c| c["task"] == "law-weakening-refused")
            .unwrap();
        assert_eq!(weak["workflow_status"], "refused");
    }
}

/// Minimal loopback HTTP stand-in for a local model server.
fn fake_model_server(answer: &'static str, calls: usize) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let h = std::thread::spawn(move || {
        for _ in 0..calls {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = vec![0u8; 65536];
            let n = s.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = if req.starts_with("GET /api/tags") {
                r#"{"models":[{"name":"fake:1b","digest":"d0"}]}"#.to_string()
            } else {
                // Reuses the API only when the prompt carries the skill block.
                let reuse = req.contains("BEGIN SKILL");
                json!({"response": if reuse { answer } else { "x * x" }}).to_string()
            };
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    (addr, h)
}

#[test]
fn hp_hp17_skill_pilot_is_labelled_pilot_only_and_loopback_only() {
    use semaprax_harness::bench::arena::Arena;
    use semaprax_harness::bench::pilot::{run_pilot, run_skill_pilot, PilotConfig};
    let corpus = Corpus::load(&corpus_dir()).unwrap();
    let work = fixture_dir("hp-hp17").canonicalize().unwrap();
    let prep = |id: &str| {
        Arena::prepare(
            &corpus,
            corpus.profile(id).unwrap(),
            &repo_root(),
            &work,
            &vars(),
            compiler(),
        )
    };
    let (base, skill) = (prep("native-only"), prep("native+skill"));
    assert!(
        base.untested.is_none() && skill.untested.is_none(),
        "{:?} {:?}",
        base.untested,
        skill.untested
    );
    // Non-loopback endpoints are refused before any request.
    let remote = PilotConfig {
        addr: "example.com:80".into(),
        model: "m".into(),
        reps: 1,
        max_calls: 1,
    };
    assert_eq!(run_pilot(&corpus, &[&base], &remote)["status"], "refused");

    let (addr, h) = fake_model_server("multiply(x, x)", 6);
    let cfg = PilotConfig {
        addr,
        model: "fake:1b".into(),
        reps: 1,
        max_calls: 6,
    };
    let r = run_skill_pilot(&corpus, &base, &skill, &cfg, 3);
    h.join().unwrap();
    assert_eq!(r["status"], "ran", "{r}");
    assert_eq!(r["label"], "pilot-only", "fewer than ten trials");
    assert_eq!(r["without_skill"], json!({"reused": 0, "n": 3}));
    assert_eq!(r["with_skill"], json!({"reused": 3, "n": 3}));
    assert!(r["skill_prompt_bytes"].as_u64().unwrap() > 0);
    // Metadata only: rows hold digests, never answer text.
    assert!(r["rows"][0].get("answer").is_none());
}

// ---- HN-17: application-task benchmark (fixture prefix `hp-hn-bench`) ----

mod hn17 {
    use super::{corpus_dir, python};
    use crate::support::{fixture_dir, repo_root};
    use semaprax_harness::bench::apptask::arms::{self, ArmSet, ContextSpec, SkillSpec, ViewSpec};
    use semaprax_harness::bench::apptask::campaign::{self, Selection};
    use semaprax_harness::bench::apptask::model::{
        Generation, HttpModel, Metered, ModelClient, ModelError, Scripted, SpendLedger,
    };
    use semaprax_harness::bench::apptask::report;
    use semaprax_harness::bench::apptask::task::{self, TaskSet, Tools};
    use semaprax_harness::bench::apptask::tokens::{Tiktoken, TokenCounter, WordCounter};
    use semaprax_harness::bench::apptask::trial::{run_trial, ModelSpec, TrialEnv, TrialKey};
    use semaprax_harness::bench::apptask::validate_tasks;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tasks_dir() -> PathBuf {
        corpus_dir().join("apptasks")
    }

    fn which(name: &str, var: &str) -> Option<PathBuf> {
        if let Ok(p) = std::env::var(var) {
            return Some(PathBuf::from(p));
        }
        let out = std::process::Command::new("/usr/bin/which")
            .arg(name)
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!s.is_empty()).then(|| PathBuf::from(s))
    }

    fn tools() -> Tools {
        let mut t = Tools {
            compiler: std::env::var_os("SEMAPRAX_COMPILER").map(PathBuf::from),
            ..Tools::default()
        }
        .with("HARNESS_PYTHON", &python());
        if let Some(n) = which("node", "HARNESS_NODE") {
            t = t.with("HARNESS_NODE", &n.display().to_string());
        }
        t
    }

    fn has_node(t: &Tools) -> bool {
        t.get("HARNESS_NODE").is_some()
    }

    fn arm_set() -> ArmSet {
        ArmSet::load(&tasks_dir()).unwrap()
    }

    fn set() -> TaskSet {
        TaskSet::load(&tasks_dir()).unwrap()
    }

    fn model(id: &str, billed: bool) -> ModelSpec {
        ModelSpec {
            id: id.into(),
            size: if billed {
                "large".into()
            } else {
                "small".into()
            },
            billed,
        }
    }

    /// Answer built from the reference solution of one step (a perfect model).
    fn reference_answer(t: &task::Task, step: usize) -> String {
        t.reference_files_of(step)
            .into_iter()
            .map(|(p, c)| {
                format!(
                    "=== FILE: {p} ===\n{}=== END FILE ===\n",
                    String::from_utf8_lossy(&c)
                )
            })
            .collect()
    }

    fn gen(text: &str, cost: Option<f64>) -> Generation {
        Generation {
            text: text.into(),
            provider_in: Some(100),
            provider_out: Some(10),
            cost_usd: cost,
            latency_ms: 5,
            ..Generation::default()
        }
    }

    fn work(tag: &str) -> PathBuf {
        fixture_dir(&format!("hp-hn-bench-{tag}"))
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn hn17_taskset_has_at_least_ten_multi_file_tasks_across_the_required_classes() {
        let s = set();
        assert!(s.tasks.len() >= 10, "{}", s.tasks.len());
        let classes: std::collections::BTreeSet<&str> =
            s.tasks.iter().map(|t| t.class.as_str()).collect();
        for c in [
            "feature",
            "refactor",
            "compile_repair",
            "failing_tests",
            "mixed_language",
            "index_reuse",
            "maintenance",
        ] {
            assert!(classes.contains(c), "missing class {c}: {classes:?}");
        }
        for t in &s.tasks {
            assert!(t.project.len() >= 4, "{} is not multi-file", t.id);
            assert!(
                !t.steps.is_empty() && t.steps.iter().all(|st| !st.grade.is_empty()),
                "{} has no grader",
                t.id
            );
        }
        assert!(
            s.tasks.iter().any(|t| t.steps.len() >= 3),
            "repeated maintenance sessions"
        );
        // Mixed-language tasks genuinely span two languages; the digest is stable and drift-sensitive.
        assert!(s
            .tasks
            .iter()
            .filter(|t| t.class == "mixed_language")
            .all(|t| t.languages.len() >= 2));
        assert_eq!(s.digest, set().digest);
        let tmp = work("digest").join("copy");
        let _ = std::fs::remove_dir_all(&tmp);
        super::copy_tree(&tasks_dir(), &tmp);
        let f = tmp.join("tasks/feature-py-shop/project/shop/pricing.py");
        std::fs::write(
            &f,
            format!("{}# drift\n", std::fs::read_to_string(&f).unwrap()),
        )
        .unwrap();
        assert_ne!(TaskSet::load(&tmp).unwrap().digest, s.digest);
    }

    #[test]
    fn hn17_every_grader_fails_the_pristine_project_and_passes_the_reference() {
        let t = tools();
        if !has_node(&t) {
            eprintln!("skipped: no node");
            return;
        }
        let rows = validate_tasks(&set(), &t, &work("validate"));
        let tested: Vec<&Value> = rows.iter().filter(|r| r["untested"].is_null()).collect();
        assert!(tested.len() >= 10, "{rows:?}");
        for r in &tested {
            assert_eq!(r["pristine_fails"], true, "{r}");
            assert_eq!(r["reference_passes"], true, "{r}");
        }
        if t.compiler.is_none() {
            let spx: Vec<_> = rows
                .iter()
                .filter(|r| r["task"] == "compile-repair-spx")
                .collect();
            assert!(
                spx.iter().all(|r| r["untested"].is_string()),
                "no compiler: untested, not passed: {spx:?}"
            );
        }
    }

    #[test]
    fn hn17_edit_protocol_parses_fences_refuses_unsafe_paths_and_drops_truncated_blocks() {
        let a = task::parse_answer("note\n=== FILE: a/b.py ===\n```python\nx = 1\n```\n=== END FILE ===\n=== FILE: ../evil ===\nz\n=== END FILE ===\n=== FILE: /abs ===\nz\n=== END ===\n=== FILE: c.py ===\ny = 2\n");
        assert_eq!(a.edits, vec![("a/b.py".to_string(), "x = 1\n".to_string())]);
        assert_eq!(
            a.unsafe_paths,
            vec!["../evil".to_string(), "/abs".to_string()]
        );
        assert_eq!(
            a.unterminated,
            vec!["c.py".to_string()],
            "a truncated block is never applied"
        );
        assert!(task::parse_answer("just prose").edits.is_empty());
        let t = set();
        let shop = t.task("feature-py-shop").unwrap();
        assert!(
            shop.is_protected("tests/test_shop.py")
                && shop.is_protected("tests/test_bulk.py")
                && shop.is_protected("semaprax.toml")
        );
        assert!(!shop.is_protected("shop/pricing.py"));
        let (ok, refused) = task::split_edits(
            shop,
            &[
                ("tests/test_shop.py".into(), "".into()),
                ("shop/pricing.py".into(), "x".into()),
            ],
        );
        assert_eq!(
            (ok.len(), refused),
            (1, vec!["tests/test_shop.py".to_string()])
        );
    }

    fn env_for<'a>(
        s: &'a TaskSet,
        t: &'a Tools,
        w: &'a std::path::Path,
        c: &'a dyn TokenCounter,
        p: &'a BTreeMap<(String, String), arms::ContextPack>,
        k: &'a BTreeMap<String, arms::SkillBlock>,
    ) -> TrialEnv<'a> {
        TrialEnv {
            tasks: s,
            tools: t,
            work: w,
            counter: c,
            packs: p,
            arm_set: Box::leak(Box::new(arm_set())),
            skills: k,
        }
    }

    fn arm(id: &str) -> arms::Arm {
        arm_set().arm(id).unwrap().clone()
    }

    #[test]
    fn hn17_perfect_model_passes_and_a_model_editing_only_tests_cannot() {
        let t = tools();
        if !has_node(&t) {
            return;
        }
        let (s, w, c, p) = (set(), work("trial"), WordCounter, BTreeMap::new());
        let sk = arms::skill_blocks(&arm_set(), &w.join("skillhome"));
        let env = env_for(&s, &t, &w, &c, &p, &sk);
        let task = s.task("feature-py-shop").unwrap();
        let key = TrialKey {
            task: task.id.clone(),
            arm: "native".into(),
            model: "m".into(),
            rep: 0,
        };
        let good = Scripted(|_: &str, _| Ok(gen(&reference_answer(task, 0), None)));
        let r = run_trial(&env, &key, &arm("native"), &model("m", false), &good);
        assert_eq!(
            (r["passed"].clone(), r["passed_first_attempt"].clone()),
            (json!(true), json!(true)),
            "{r}"
        );
        // The model rewrites every test to pass trivially and changes nothing else.
        let tamper = Scripted(|_: &str, _| {
            Ok(gen("=== FILE: tests/test_shop.py ===\nimport unittest\nclass T(unittest.TestCase):\n    def test_ok(self):\n        pass\n=== END FILE ===\n=== FILE: tests/test_bulk.py ===\nx = 1\n=== END FILE ===\n", None))
        });
        let r = run_trial(&env, &key, &arm("native"), &model("m", false), &tamper);
        assert_eq!(
            r["passed"], false,
            "an immutable grader restores the tests: {r}"
        );
        assert!(r["tamper_attempts"].as_u64().unwrap() >= 2, "{r}");
        assert_eq!(r["structurally_valid_first"], false);
    }

    #[test]
    fn hn17_negative_control_that_strips_required_work_measurably_fails_the_oracle() {
        let t = tools();
        if !has_node(&t) {
            return;
        }
        let (s, w, c, p) = (set(), work("neg"), WordCounter, BTreeMap::new());
        let sk = arms::skill_blocks(&arm_set(), &w.join("skillhome"));
        let env = env_for(&s, &t, &w, &c, &p, &sk);
        let task = s.task("feature-py-shop").unwrap();
        // A perfect model: only the harness-level lossy compression (first file block survives) differs.
        let perfect = Scripted(|_: &str, _| Ok(gen(&reference_answer(task, 0), None)));
        let mut rows = vec![];
        for a in ["native", "neg-output-keeps-first-file"] {
            for rep in 0..3 {
                let key = TrialKey {
                    task: task.id.clone(),
                    arm: a.into(),
                    model: "m".into(),
                    rep,
                };
                rows.push(run_trial(&env, &key, &arm(a), &model("m", false), &perfect));
            }
        }
        assert!(rows
            .iter()
            .filter(|r| r["arm"] == "native")
            .all(|r| r["passed"] == true));
        assert!(
            rows.iter()
                .filter(|r| r["arm"] != "native")
                .all(|r| r["passed"] == false),
            "the oracle sees the stripped work: {rows:?}"
        );
        assert!(
            rows[3]["steps"][0]["attempts"][0]["files_dropped_by_filter"]
                .as_u64()
                .unwrap()
                >= 1
        );
        let sum = report::summarize(&rows, &Value::Null);
        let c = &sum["comparisons"][0];
        assert_eq!(c["arm"], "neg-output-keeps-first-file");
        assert_eq!(
            c["paired_vs_native"]["accepted_delta_per_cell"], -1.0,
            "{c}"
        );
        assert_eq!(c["verdict"], "control-detected");
        // And the report cannot be fooled the other way: a control that scores like native is flagged.
        let mut same = rows.clone();
        for r in same.iter_mut().filter(|r| r["arm"] != "native") {
            r["passed"] = json!(true);
        }
        assert_eq!(
            report::summarize(&same, &Value::Null)["comparisons"][0]["verdict"],
            "control-NOT-detected"
        );
        // The prompt-level control is a separate arm: whether a model obeys it is itself a measurement.
        let a = arm("neg-skill-strips-work");
        assert!(
            a.role == arms::Role::NegativeControl
                && a.answer_filter.is_none()
                && sk["neg-skill-strips-work"].text.contains("ONE file")
        );
    }

    #[test]
    fn hn17_stripped_failure_output_hides_the_failure_and_the_oracle_still_fails() {
        let raw = "test_a ... ok\ntest_b ... FAIL\n\nTraceback (most recent call last):\n  File \"x.py\", line 3, in test_b\nAssertionError: 1998 != 1999\n\nFAILED (failures=1)\n";
        let stripped = arms::strip_failures(raw);
        assert!(
            stripped.contains("test_a ... ok") && stripped.trim_end().ends_with("OK"),
            "{stripped}"
        );
        assert!(
            !stripped.contains("AssertionError")
                && !stripped.contains("FAIL")
                && !stripped.contains("Traceback"),
            "{stripped}"
        );
        let t = tools();
        if !has_node(&t) {
            return;
        }
        let (s, w, c, p) = (set(), work("strip"), WordCounter, BTreeMap::new());
        let sk = arms::skill_blocks(&arm_set(), &w.join("skillhome"));
        let env = env_for(&s, &t, &w, &c, &p, &sk);
        let key = TrialKey {
            task: "failing-tests-py-ledger".into(),
            arm: "neg-stripped-failure".into(),
            model: "m".into(),
            rep: 0,
        };
        let seen = std::sync::Mutex::new(String::new());
        let m = Scripted(|prompt: &str, _| {
            *seen.lock().unwrap() = prompt.to_string();
            Ok(gen("Everything already passes; no change needed.", None))
        });
        let r = run_trial(
            &env,
            &key,
            &arm("neg-stripped-failure"),
            &model("m", false),
            &m,
        );
        assert_eq!(r["passed"], false, "{r}");
        let p = seen.lock().unwrap().clone();
        assert!(
            !p.contains("AssertionError") && p.contains("\nOK\n"),
            "the model saw a stripped view"
        );
    }

    #[test]
    fn hn17_retry_session_feeds_grader_output_back_and_counts_every_attempt() {
        let t = tools();
        if !has_node(&t) {
            return;
        }
        let (s, w, c, p) = (set(), work("retry"), WordCounter, BTreeMap::new());
        let sk = arms::skill_blocks(&arm_set(), &w.join("skillhome"));
        let env = env_for(&s, &t, &w, &c, &p, &sk);
        let task = s.task("feature-py-shop").unwrap();
        let calls = AtomicUsize::new(0);
        let m = Scripted(|prompt: &str, _| {
            Ok(if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                gen("=== FILE: shop/pricing.py ===\nfrom .models import Item\ndef line_cents(item, qty):\n    return item.unit_cents * qty\n=== END FILE ===\n", Some(0.01))
            } else {
                assert!(
                    prompt.contains("Grader result: FAILED")
                        && prompt.contains("Your previous answer"),
                    "retry prompt carries the failure"
                );
                gen(&reference_answer(task, 0), Some(0.02))
            })
        });
        let key = TrialKey {
            task: task.id.clone(),
            arm: "native".into(),
            model: "m".into(),
            rep: 1,
        };
        let r = run_trial(&env, &key, &arm("native"), &model("m", true), &m);
        assert_eq!(r["passed"], true, "{r}");
        assert_eq!(
            r["passed_first_attempt"], false,
            "success after a retry is not first-attempt success"
        );
        assert_eq!(r["totals"]["attempts"], 2);
        assert_eq!(r["totals"]["cost_usd"], 0.03, "failed attempts are charged");
        assert!(
            r["totals"]["tokens_o200k"].as_u64().unwrap() > 0 && r["totals"]["provider_in"] == 200
        );
        assert_eq!(r["steps"][0]["attempts"][0]["grade_passed"], false);
        // Metadata only: no prompt or answer text in the record.
        assert!(!r.to_string().contains("def line_cents"));
    }

    #[test]
    fn hn17_ledger_refuses_at_the_cap_and_never_overshoots() {
        let l = SpendLedger::new(0.07, 100, 0.02);
        let inner_calls = AtomicUsize::new(0);
        let inner = Scripted(|_: &str, _| {
            inner_calls.fetch_add(1, Ordering::SeqCst);
            Ok(gen("x", Some(0.02)))
        });
        let m = Metered {
            inner: &inner,
            ledger: &l,
            billed: true,
        };
        let mut refused = 0;
        for _ in 0..10 {
            match m.generate("p", 1) {
                Err(ModelError::Budget(why)) => {
                    refused += 1;
                    assert!(why.contains("cap"), "{why}");
                }
                Ok(_) => {}
                Err(e) => panic!("{e:?}"),
            }
        }
        assert!(l.spent() <= 0.07, "spent {}", l.spent());
        assert_eq!(
            inner_calls.load(Ordering::SeqCst),
            2,
            "no call is sent once the ceiling would cross the cap"
        );
        assert_eq!(refused, 8);
        assert_eq!(l.snapshot()["refused_calls"], 8);
        // In-flight reservations count: two concurrent calls cannot jointly cross the cap.
        let l2 = SpendLedger::new(0.05, 100, 0.03);
        let r1 = l2.reserve().unwrap();
        assert!(l2.reserve().is_err());
        l2.settle(r1, Some(0.001));
        assert!(l2.reserve().is_ok());
    }

    #[test]
    fn hn17_ledger_charges_failed_calls_enforces_call_cap_and_persists() {
        let path = work("ledger").join("ledger.json");
        let _ = std::fs::remove_file(&path);
        let l = SpendLedger::new(1.0, 2, 0.1).with_file(&path);
        let fail = Scripted(|_: &str, _| Err(ModelError::Failed("timeout".into())));
        let m = Metered {
            inner: &fail,
            ledger: &l,
            billed: true,
        };
        assert!(matches!(m.generate("p", 1), Err(ModelError::Failed(_))));
        assert!(
            (l.spent() - 0.1).abs() < 1e-9,
            "a failed call may still be billed upstream: reservation charged"
        );
        assert!(matches!(m.generate("p", 1), Err(ModelError::Failed(_))));
        assert!(
            matches!(m.generate("p", 1), Err(ModelError::Budget(_))),
            "call cap"
        );
        let again = SpendLedger::new(1.0, 2, 0.1).with_file(&path);
        assert_eq!(
            again.snapshot()["calls"],
            2,
            "resumes from the persisted totals"
        );
        // Local models are never charged.
        let free = Metered {
            inner: &Scripted(|_: &str, _| Ok(gen("x", None))),
            ledger: &again,
            billed: false,
        };
        assert!(free.generate("p", 1).is_ok());
    }

    #[test]
    fn hn17_http_model_reads_provider_usage_cost_and_shim_refusals() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let h = std::thread::spawn(move || {
            for body in [
                r#"{"response":"ok","usage":{"input_tokens":12,"output_tokens":3,"cache_read_input_tokens":5},"cost_usd":0.0031,"done":true}"#,
                r#"{"response":"local","prompt_eval_count":40,"eval_count":9,"done":true}"#,
                r#"{"refused":true,"reason":"shim cap"}"#,
            ] {
                let (mut s, _) = l.accept().unwrap();
                let mut buf = vec![0u8; 65536];
                let _ = s.read(&mut buf).unwrap();
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        let m = HttpModel {
            addr: addr.clone(),
            name: "m".into(),
            temperature: 0.0,
            num_ctx: 1024,
            num_predict: 64,
            timeout: std::time::Duration::from_secs(5),
        };
        let g = m.generate("p", 1).unwrap();
        assert_eq!(
            (g.provider_in, g.provider_out, g.cache_read, g.cost_usd),
            (Some(12), Some(3), Some(5), Some(0.0031))
        );
        let g = m.generate("p", 1).unwrap();
        assert_eq!(
            (g.provider_in, g.provider_out, g.cost_usd),
            (Some(40), Some(9), None),
            "local: usage reported, cost unavailable not zero"
        );
        assert!(matches!(m.generate("p", 1), Err(ModelError::Budget(_))));
        h.join().unwrap();
        let remote = HttpModel {
            addr: "example.com:80".into(),
            ..m
        };
        assert!(
            matches!(remote.generate("p", 1), Err(ModelError::Failed(_))),
            "loopback only"
        );
    }

    #[test]
    fn hn17_plan_is_rep_major_and_arms_only_run_where_they_can_change_something() {
        let s = set();
        let models = [model("small", false), model("large", true)];
        let sel = Selection {
            tasks: vec![],
            arms: vec![],
            reps: 2,
        };
        let aset = arm_set();
        let plan = campaign::plan(&s, &aset, &sel, &models);
        let first_rep1 = plan.iter().position(|k| k.rep == 1).unwrap();
        assert!(
            plan[..first_rep1].iter().all(|k| k.rep == 0)
                && plan[first_rep1..].iter().all(|k| k.rep == 1),
            "every cell gains a repetition before any gains two"
        );
        let rtk: Vec<&TrialKey> = plan
            .iter()
            .filter(|k| k.arm == "rtk-err" || k.arm == "neg-stripped-failure")
            .collect();
        assert!(
            !rtk.is_empty() && rtk.iter().all(|k| k.task.starts_with("failing-tests")),
            "views only where a failing run is shown"
        );
        let native = plan
            .iter()
            .filter(|k| k.arm == "native" && k.model == "small" && k.rep == 0)
            .count();
        assert_eq!(native, s.tasks.len());
        let core: Vec<&str> = aset
            .arms
            .iter()
            .filter(|a| a.role == arms::Role::Core)
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(
            core,
            [
                "native",
                "ponytail",
                "caveman",
                "ponytail+caveman",
                "concise"
            ]
        );
        let un = &aset.untested;
        assert!(
            un.iter()
                .any(|(id, why)| id == "router" && why.starts_with("untested"))
                && un.iter().any(|(id, _)| id == "wikiskill")
        );
    }

    #[test]
    fn hn17_resume_skips_recorded_trials_but_retries_budget_stops() {
        let p = work("resume").join("trials.jsonl");
        std::fs::write(&p, "{\"trial\":\"a|native|m|0\",\"status\":\"ok\"}\n{\"trial\":\"b|native|m|0\",\"status\":\"budget\"}\n{\"trial\":\"c|graft|m|0\",\"status\":\"untested\"}\nnot json\n").unwrap();
        let d = campaign::done_ids(&p);
        assert!(
            d.contains("a|native|m|0") && d.contains("c|graft|m|0") && !d.contains("b|native|m|0")
        );
    }

    fn row(
        task: &str,
        class: &str,
        arm: &str,
        rep: u64,
        passed: bool,
        tok: u64,
        cost: Option<f64>,
    ) -> Value {
        json!({"schema": "semaprax.harness-apptrial.v1", "trial": format!("{task}|{arm}|m|{rep}"), "task": task, "class": class, "arm": arm,
               "arm_role": if arm.starts_with("neg") { "NegativeControl" } else { "Core" }, "model": "m", "size": "large", "rep": rep, "cold": rep == 0,
               "status": "ok", "passed": passed, "passed_first_attempt": passed, "structurally_valid_first": true, "tamper_attempts": 0,
               "skill": {"delivered": arm != "native", "tokens_o200k": 0}, "context": {},
               "totals": {"prompt_tokens_o200k": tok, "answer_tokens_o200k": 10, "completion_ms": 1000, "attempts": 1, "cost_usd": cost, "provider_in": tok, "provider_out": 10}})
    }

    fn rows_for(arm_pass: impl Fn(u64) -> bool, arm_tok: u64, n: u64) -> Vec<Value> {
        let mut v = vec![];
        for rep in 0..n {
            v.push(row("t1", "feature", "native", rep, true, 1000, Some(0.01)));
            v.push(row(
                "t1",
                "feature",
                "ponytail",
                rep,
                arm_pass(rep),
                arm_tok,
                Some(0.01 * arm_tok as f64 / 1000.0),
            ));
        }
        v
    }

    #[test]
    fn hn17_verdicts_pilot_no_lift_loss_and_qualified_are_distinct_and_never_automatic() {
        let verdict = |rows: Vec<Value>| {
            report::summarize(&rows, &Value::Null)["comparisons"][0]["verdict"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(
            verdict(rows_for(|_| true, 600, 5)),
            "pilot-only",
            "fewer than ten matched cells never qualifies"
        );
        assert_eq!(verdict(rows_for(|_| true, 600, 10)), "qualified-scoped");
        assert_eq!(
            verdict(rows_for(|_| true, 1050, 10)),
            "available-no-lift",
            "no-lift keeps availability without a performance claim"
        );
        assert_eq!(
            verdict(rows_for(|r| r != 3, 600, 10)),
            "not-recommended",
            "one lost task is a quality loss"
        );
        assert_eq!(
            verdict(rows_for(|_| true, 900, 10)),
            "available-no-lift",
            "10% fewer tokens is below the 20% gate"
        );
        let sum = report::summarize(&rows_for(|_| true, 600, 10), &Value::Null);
        let recs = report::recommendations(&sum);
        let e = &recs["entries"][0];
        assert_eq!(
            (e["qualified"].clone(), e["automatic_default"].clone()),
            (json!(true), json!(false))
        );
        assert_eq!(e["hn06_skills_proposal"]["applied"], false);
        assert_eq!(
            e["hn06_skills_proposal"]["keys"],
            json!({"ponytail": "full"})
        );
        let none = report::recommendations(&report::summarize(
            &rows_for(|_| true, 1050, 10),
            &Value::Null,
        ));
        assert!(none["entries"][0]["hn06_skills_proposal"].is_null());
        // A not-run (untested) trial is never a win and blocks qualification.
        let mut rows = rows_for(|_| true, 600, 10);
        rows[1]["status"] = json!("untested");
        assert_eq!(verdict(rows), "untested-cells");
        let o = report::outcomes(&rows_for(|_| true, 600, 2));
        assert_eq!(
            o["outcomes"][0]["verified_by"],
            "apptask-immutable-grader/v1"
        );
        assert_eq!(o["outcomes"][0]["origin"], "real");
    }

    #[test]
    fn hn17_cell_size_label_is_pilot_below_ten_and_losses_stay_in_the_report() {
        let sum = report::summarize(&rows_for(|_| true, 600, 9), &Value::Null);
        assert_eq!(sum["cell_size_check"]["label"], "pilot");
        let sum = report::summarize(
            &rows_for(|r| r != 0, 1200, 10),
            &json!({"taskset_digest": "sha256:x", "tokenizer": "tiktoken:o200k_base", "identities": {}, "arms": [], "untested_arms": {}, "models_note": ""}),
        );
        assert_eq!(sum["cell_size_check"]["label"], "matched-trials");
        let text = semaprax_harness::bench::apptask::render::render(
            &sum,
            &report::recommendations(&sum),
            &json!({"taskset_digest": "sha256:x", "tokenizer": "tiktoken:o200k_base", "identities": {}, "arms": [], "untested_arms": {}, "models_note": ""}),
            &json!({"cap_usd": 15.0, "spent_usd": 1.0, "calls": 3, "refused_calls": 0}),
            "t",
        );
        assert!(
            text.contains("MATCHED-TRIALS")
                && text.contains("not-recommended")
                && text.contains("Wilson"),
            "{text}"
        );
        assert!(
            text.contains("per task class") || text.contains("task class"),
            "never pooled"
        );
        let g = &sum["by_model_class_arm"]["m"]["feature"]["ponytail"];
        assert!(
            g["accepted"]["wilson95"].is_array() && g["tokens_o200k"]["total"]["sd"].is_number(),
            "variation is reported"
        );
        assert!(
            g["cold"]["accepted"]["n"] == 1 && g["warm"]["accepted"]["n"] == 9,
            "cold and warm are separate"
        );
    }

    #[test]
    fn hn17_mixed_cascade_is_derived_and_counts_both_models_when_escalating() {
        let mut rows = vec![];
        for rep in 0..2u64 {
            let mut s = row("t1", "feature", "native", rep, rep == 1, 500, None);
            s["size"] = json!("small");
            let l = row("t1", "feature", "native", rep, true, 1000, Some(0.02));
            rows.extend([s, l]);
        }
        let c = report::cascade(&report::parse(&rows));
        assert_eq!(c[0]["pairs"], 2);
        assert_eq!(c[0]["escalation_rate"], 0.5);
        assert_eq!(c[0]["accepted"]["k"], 2);
        assert!(c[0]["label"].as_str().unwrap().contains("derived"));
        assert_eq!(c[0]["mean_billed_usd"], 0.01);
    }

    #[test]
    fn hn17_official_skills_are_delivered_byte_exact_and_the_baselines_are_distinct() {
        let upstream =
            std::fs::read_to_string(repo_root().join(
                "packages/semaprax-harness-adapters/skills/official/ponytail/v4.10.3/SKILL.md",
            ))
            .unwrap();
        let body = upstream.splitn(3, "---\n").nth(2).unwrap();
        let line = body
            .lines()
            .find(|l| {
                l.len() > 60
                    && !l.starts_with('-')
                    && !l.starts_with('#')
                    && !l.contains('|')
                    && !l.starts_with(' ')
            })
            .unwrap();
        let home = work("skills").join("home");
        let _ = std::fs::remove_dir_all(&home);
        // A second rendering into the same home (a previous run's `stop` state) must not change delivery.
        let _ = arms::skill_blocks(&arm_set(), &home);
        let blocks = arms::skill_blocks(&arm_set(), &home);
        let p = &blocks["ponytail"];
        assert!(
            p.delivered && p.ids == ["ponytail"] && p.text.contains(line.trim()),
            "delivered {} ids {:?} note {:?}",
            p.delivered,
            p.ids,
            p.note
        );
        let c = &blocks["caveman"];
        assert!(
            c.delivered && c.ids == ["caveman"] && !c.text.contains(line.trim()),
            "caveman arm must not also carry Ponytail: {:?} {:?}",
            c.ids,
            c.note
        );
        let both = &blocks["ponytail+caveman"];
        assert!(
            both.delivered
                && both.ids.len() == 2
                && both.text.len() > p.text.len()
                && both.text.len() > c.text.len()
        );
        assert!(blocks["native"].text.is_empty() && !blocks["native"].delivered);
        assert!(
            blocks["concise"].text.len() < 300,
            "the simple baseline is a short instruction"
        );
        assert!(
            blocks["neg-skill-strips-work"]
                .text
                .contains("OUTPUT-COMPRESSION")
                && blocks["neg-output-keeps-first-file"].text.is_empty()
        );
        assert_eq!(
            arm("concise").skill,
            SkillSpec::Text {
                id: "concise-baseline".into(),
                text: arm_set()
                    .arm("concise")
                    .map(|a| match &a.skill {
                        SkillSpec::Text { text, .. } => text.clone(),
                        _ => String::new(),
                    })
                    .unwrap()
            }
        );
    }

    #[test]
    fn hn17_retrieval_helpers_parse_tool_output_and_drop_preambles() {
        let aset = arm_set();
        let (t1, t2) = (&aset.retrieval["graft"], &aset.retrieval["graphify"]);
        let pre = "[x] tokens saved ≈ 21 tell the user\n\ngraft ask — \"q\"\n\n1. parse_iso · function\n   corelib/dates.py:L4-L7\n\n2. slugify\n   corelib/text.py:L1-L6\n3. again\n   corelib/dates.py:L9-L9\n";
        let cleaned = arms::clean_tool_output(t1, pre);
        assert!(cleaned.starts_with("graft ask") && !cleaned.contains("tell the user"));
        assert_eq!(
            arms::referenced_files(t1, &cleaned),
            ["corelib/dates.py", "corelib/text.py"]
        );
        let gf = "NODE slugify() [src=corelib/text.py loc=L4 community=x]\nNODE date [src= loc= community=y]\nNODE a [src=reports/summary.py loc=L1 community=z]\nNODE b [src=corelib/text.py loc=L9]\n";
        assert_eq!(
            arms::referenced_files(t2, gf),
            ["corelib/text.py", "reports/summary.py"]
        );
        let t = set();
        let shop = t.task("feature-py-shop").unwrap();
        let native = arms::native_pack(&shop.project);
        assert_eq!(native.files_in_full.len(), shop.project.len());
        // Arms are data: the arm set names retrieval and view tools only through HARNESS_* variables.
        assert!(
            matches!(arm("graft").context, ContextSpec::Retrieval { .. })
                && matches!(arm("native").context, ContextSpec::Native)
        );
        // View commands are the same argv, run once, shown differently.
        let argv = vec!["py".to_string(), "-m".into(), "unittest".into()];
        let none = Tools::default();
        assert_eq!(arms::view_argv(&ViewSpec::Raw, &argv, &none).unwrap(), argv);
        let wrap = arm("rtk-test").view;
        assert!(
            arms::view_argv(&wrap, &argv, &none).is_err(),
            "no wrapper path: unavailable, not silently raw"
        );
        let w = Tools::default().with("HARNESS_RTK", "/x/rtk");
        assert_eq!(
            arms::view_argv(&wrap, &argv, &w).unwrap()[..2],
            ["/x/rtk".to_string(), "test".to_string()]
        );
    }

    #[test]
    fn hn17_named_tokenizer_counts_when_provisioned_else_reports_unavailable() {
        let (Some(py), Some(cache)) = (
            std::env::var_os("HARNESS_TIKTOKEN_PYTHON"),
            std::env::var_os("HARNESS_TIKTOKEN_CACHE"),
        ) else {
            assert!(Tiktoken::start(
                std::path::Path::new("/nonexistent/python"),
                std::path::Path::new("/nonexistent")
            )
            .is_err());
            return;
        };
        let t = Tiktoken::start(std::path::Path::new(&py), std::path::Path::new(&cache)).unwrap();
        assert_eq!(t.count("hello world").unwrap(), 2);
        assert!(t.name().starts_with("tiktoken:o200k_base"));
        assert!(t.count("def f(x):\n    return x * 2\n").unwrap() > 5);
    }

    #[test]
    fn hn17_docs_pin_the_hn17_gates_and_keep_hp17_evidence_text() {
        let d = std::fs::read_to_string(repo_root().join("docs/HARNESS-BENCHMARK-V1.md")).unwrap();
        for needle in [
            "## HN-17",
            "MIN_TRIALS_PER_CELL",
            "`N`",
            "`Q`",
            "`C`",
            "`T`",
            "available-no-lift",
            "control-detected",
            "pilot",
        ] {
            assert!(d.contains(needle), "docs lack {needle}");
        }
        assert!(
            d.contains("## Gates (declared before any recorded result)")
                && d.contains("No named tokenizer is available on this machine"),
            "HP-17 text is intact"
        );
        assert!(d.contains(&format!(
            "{}",
            semaprax_harness::bench::gates::MIN_MATCHED_CELLS
        )));
        assert_eq!(report::MIN_TRIALS_PER_CELL, 10);
        assert_eq!(semaprax_harness::bench::gates::MIN_NET_BYTE_REDUCTION, 0.20);
    }

    #[test]
    #[ignore = "provisioned: needs HARNESS_GRAFT HARNESS_GRAPHIFY HARNESS_RTK HARNESS_NODE HARNESS_PYTHON"]
    fn hn17_real_retrieval_tools_and_command_views_produce_packs_and_views() {
        let mut t = Tools::default();
        for k in [
            "HARNESS_PYTHON",
            "HARNESS_NODE",
            "HARNESS_GRAFT",
            "HARNESS_GRAPHIFY",
            "HARNESS_RTK",
        ] {
            t = t.with(
                k,
                &std::env::var(k).unwrap_or_else(|_| panic!("{k} not set")),
            );
        }
        let (s, aset) = (set(), arm_set());
        let w = work("realtools");
        let task = s.task("reuse-py-order-row").unwrap();
        for id in ["graft", "graphify"] {
            let p = arms::retrieval_pack(
                &aset.retrieval[id],
                task,
                &task.project,
                &task.steps[0].request,
                &t,
                &w,
                "t",
            );
            assert!(p.unavailable.is_none(), "{id}: {:?}", p.unavailable);
            assert!(p.build_ms > 0 && p.index_bytes > 0 && !p.text.is_empty());
            assert!(
                p.files_in_full.iter().any(|f| f == "reports/order_row.py"),
                "the file named in the request is opened"
            );
            assert!(
                p.files_in_full.iter().any(|f| f.starts_with("corelib/")),
                "retrieval points at a helper module: {:?}",
                p.files_in_full
            );
            assert!(!p.text.contains("tell the user"), "tool preamble removed");
        }
        // The `err` view keeps the failing assertion; the `test` view keeps only the summary tail (a real, lossy view).
        let led = s.task("failing-tests-py-ledger").unwrap();
        let sb = w.join("sb-views");
        task::prepare_sandbox(&sb, &led.project).unwrap();
        let argv = t
            .expand(&led.steps[0].initial_command.as_ref().unwrap().cmd)
            .unwrap();
        for (id, keeps) in [("rtk-err", true), ("rtk-test", false)] {
            let a = arms::view_argv(&arm(id).view, &argv, &t).unwrap();
            let r = task::run_cmd(
                &a,
                &sb,
                &w.join("home"),
                &[],
                std::time::Duration::from_secs(60),
            );
            assert_eq!(
                r.combined().contains("AssertionError"),
                keeps,
                "{id}: {}",
                r.combined()
            );
        }
    }
}
