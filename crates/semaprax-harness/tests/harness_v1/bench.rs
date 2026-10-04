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
    // Injected clock: the whole result set is reproducible byte for byte,
    // except `disk_bytes`, which measures real provider cache files whose
    // contents (refresh timings) may differ by a few bytes between runs; it
    // is compared within a small tolerance below.
    let strip = |c: &Value| {
        let mut c = c.clone();
        c.as_object_mut().unwrap().remove("disk_bytes");
        c
    };
    let canon = |o: &RunOutput| {
        o.cells
            .iter()
            .map(|c| semaprax_harness::json::canonical(&strip(c)))
            .collect::<Vec<_>>()
    };
    assert_eq!(a.cells.len(), b.cells.len());
    for (x, y) in a.cells.iter().zip(&b.cells) {
        let diff: Vec<String> = x
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, v)| k.as_str() != "disk_bytes" && &y[k.as_str()] != *v)
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
    for (x, y) in a.cells.iter().zip(&b.cells) {
        let (dx, dy) = (
            x["disk_bytes"].as_u64().unwrap(),
            y["disk_bytes"].as_u64().unwrap(),
        );
        assert!(dx.abs_diff(dy) <= 64, "disk_bytes {dx} vs {dy}");
    }
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

#[path = "bench/hn17.rs"]
mod hn17;

// ---- TC-12: profile-arm campaign, offline ----

#[path = "bench/tc12.rs"]
mod tc12;
