//! MA-03/07/08/09 bridge journal tests (fixture prefix `hp-jma`): single-writer
//! claims, host-assigned default step identity, strict optional controls and
//! visible terminal-journal failures. A local counted model adapter records
//! every dispatch in `count`; nothing here calls a paid model.

use crate::support::{fixture_dir, write};
use semaprax_harness::bridge::inflight::Invoker;
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::host::{CancelToken, HostConfig, IsolationBackend};
use semaprax_harness::profile::resolve::current_platform;
use semaprax_harness::workflow::journal::{Fault, FaultHook};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

const ID: &str = "org.example/hostile";

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

struct World {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    env: Environment,
}

impl World {
    fn new() -> World {
        let root = fixture_dir("hp-jma").canonicalize().unwrap();
        let ex = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/semaprax-harness-adapters/examples/hostile-python");
        let count = root.join("count");
        let src = std::fs::read_to_string(ex.join("adapter.py"))
            .unwrap()
            .replace(
                "    if kind == \"decision.evaluate\":",
                &format!(
                    "    if kind == \"model.generate\":\n        open(\"{}\", \"a\").write(\"x\")\n        return {{\"model\": p.get(\"model\"), \"output_base64\": \"aGk=\", \"usage\": {{\"input_bytes\": 4, \"output_bytes\": 2}}}}\n    if kind == \"decision.evaluate\":",
                    count.display()
                ),
            )
            .replace(
                "\"decision.evaluate\": [\"evaluate\"]}",
                "\"decision.evaluate\": [\"evaluate\"], \"model.generate\": [\"generate\"]}",
            );
        write(&root, "adapter/adapter.py", &src);
        let mut d: Value =
            serde_json::from_slice(&std::fs::read(ex.join("harness-provider.json")).unwrap())
                .unwrap();
        d["platforms"]
            .as_array_mut()
            .unwrap()
            .push(json!(current_platform()));
        d["capabilities"].as_array_mut().unwrap().push(
            json!({"kind": "model.generate", "version": 1, "required": false, "operations": ["generate"]}),
        );
        d["resources"]["invoke_timeout_ms"] = json!(120_000);
        d["resources"]["handshake_timeout_ms"] = json!(10_000);
        write(&root, "adapter/harness-provider.json", &d.to_string());
        let home = root.join("home");
        let project = root.join("project");
        std::fs::create_dir_all(&home).unwrap();
        write(&project, "src/lib.rs", "pub fn a() {}\n");
        write(
            &project,
            "semaprax.harness.toml",
            &format!("schema = \"semaprax.harness-config.v1\"\n[capability.\"model.generate\"]\nmode = \"auto\"\nprovider = \"{ID}\"\n"),
        );
        let env = Environment {
            harness_home: Some(home.clone()),
            compiler: None,
            cwd: root.clone(),
            vars: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        };
        let py = std::env::var_os("HARNESS_PYTHON")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                    .map(|d| d.join("python3"))
                    .find(|p| p.is_file())
            })
            .expect("python3 not found");
        let desc = root.join("adapter/harness-provider.json");
        let o = run(
            &s(&[
                "adopt",
                desc.to_str().unwrap(),
                "--runtime",
                py.to_str().unwrap(),
            ]),
            &env,
        );
        assert_eq!(o.code, 0, "adopt: {}", o.stderr);
        let o = run(&s(&["trust", ID]), &env);
        assert_eq!(o.code, 0, "trust: {}", o.stderr);
        World {
            root,
            home,
            project,
            env,
        }
    }

    fn invoker(&self) -> Invoker {
        Invoker::new(&self.env, &self.project, HostConfig::default())
    }

    fn dispatches(&self) -> usize {
        std::fs::read_to_string(self.root.join("count")).map_or(0, |t| t.len())
    }

    fn journal_text(&self) -> String {
        let dir = self.home.join("cache").join("bridge");
        std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".journal.jsonl"))
            .map(|e| std::fs::read_to_string(e.path()).unwrap())
            .collect()
    }
}

fn gen_params(step: Option<&str>) -> Value {
    let mut p = json!({"capability":"model.generate","operation":"generate",
        "payload":{"model":"m.x","input_base64":"aGk=","max_output_bytes":16}});
    if let Some(st) = step {
        p["step"] = json!(st);
    }
    p
}

fn code(e: &semaprax_harness::diag::HarnessDiagnostic) -> &str {
    e.code
}

#[test]
fn ma07_omitted_step_gets_a_unique_persisted_identity_per_call_and_session() {
    let w = World::new();
    let inv = w.invoker();
    let tok = CancelToken::new();
    let a = inv.run(&gen_params(None), &tok).unwrap();
    let b = inv.run(&gen_params(None), &tok).unwrap();
    assert_eq!(
        (a["state"].as_str(), b["state"].as_str()),
        (Some("completed"), Some("completed"))
    );
    assert_ne!(a["step"], b["step"]);
    // A restarted session (fresh Invoker, counter reset) never collides.
    let c = w.invoker().run(&gen_params(None), &tok).unwrap();
    assert_eq!(c["state"], "completed", "{c}");
    assert_ne!(c["step"], a["step"]);
    assert_eq!(w.dispatches(), 3);
    let text = w.journal_text();
    for r in [&a, &b, &c] {
        let st = r["step"].as_str().unwrap();
        assert!(
            text.contains(&format!("\"step\":\"{st}\"")),
            "{st} not journaled"
        );
    }
}

#[test]
fn ma07_explicit_step_stays_replay_protected_across_restarts() {
    let w = World::new();
    let tok = CancelToken::new();
    let a = w.invoker().run(&gen_params(Some("gen-1")), &tok).unwrap();
    assert_eq!(a["step"], "gen-1");
    assert_eq!(w.dispatches(), 1);
    for inv in [w.invoker(), w.invoker()] {
        let e = inv.run(&gen_params(Some("gen-1")), &tok).unwrap_err();
        assert_eq!(code(&e), "SPX-HPN015");
    }
    assert_eq!(w.dispatches(), 1);
    // A different explicit step is independent.
    assert!(w.invoker().run(&gen_params(Some("gen-2")), &tok).is_ok());
    assert_eq!(w.dispatches(), 2);
}

#[test]
fn ma03_independent_invokers_race_for_one_step_and_exactly_one_dispatches() {
    let w = Arc::new(World::new());
    let n = 6;
    let barrier = Arc::new(Barrier::new(n));
    let results: Vec<Result<Value, String>> = (0..n)
        .map(|_| {
            let (w, b) = (w.clone(), barrier.clone());
            std::thread::spawn(move || {
                let inv = w.invoker();
                b.wait();
                inv.run(&gen_params(Some("race")), &CancelToken::new())
                    .map_err(|e| e.code.to_string())
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    let ok = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(ok, 1, "{results:?}");
    assert!(results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .all(|c| c == "SPX-HPN015"));
    assert_eq!(w.dispatches(), 1);
    let text = w.journal_text();
    assert_eq!(text.matches("\"state\":\"begin\"").count(), 1, "{text}");
    let seqs: Vec<u64> = text
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["seq"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(seqs, vec![1, 2], "{text}");
}

#[test]
fn ma08_malformed_optional_controls_fail_before_preparation_and_journal() {
    let w = World::new();
    let tok = CancelToken::new();
    let bad: Vec<(&str, Value)> = vec![
        ("isolation", json!(true)),
        ("isolation", json!(false)),
        ("isolation", json!(null)),
        ("isolation", json!(1)),
        ("isolation", json!([])),
        ("isolation", json!({})),
        ("isolation", json!("optional")),
        ("deadline_ms", json!(-1)),
        ("deadline_ms", json!(1.5)),
        ("deadline_ms", json!("100")),
        ("deadline_ms", json!(true)),
        ("deadline_ms", json!(null)),
        ("deadline_ms", json!(0)),
        ("deadline_ms", json!(600_001)),
        ("step", json!(1)),
        ("step", json!(null)),
        ("step", json!("")),
        ("step", json!("has space")),
        ("step", json!("x".repeat(129))),
        ("step", json!("new\nline")),
    ];
    for (field, v) in bad {
        let mut p = gen_params(None);
        p[field] = v.clone();
        let e = w.invoker().run(&p, &tok).unwrap_err();
        assert_eq!(code(&e), "SPX-HPN005", "{field}={v}: {e}");
    }
    assert_eq!(w.dispatches(), 0);
    assert!(w.journal_text().is_empty());
    assert!(
        !w.home.join("cache").join("bridge").exists(),
        "no journal was opened"
    );
    assert!(
        !w.home.join("cache").join("adapters").exists(),
        "nothing was prepared"
    );
    // Positive controls: absence keeps defaults; boundaries are accepted.
    for (field, v) in [
        ("deadline_ms", json!(1)),
        ("deadline_ms", json!(600_000)),
        ("step", json!("a")),
    ] {
        let mut p = gen_params(None);
        p[field] = v;
        // The 1 ms deadline may expire in the provider; it must at least pass parsing.
        match w.invoker().run(&p, &tok) {
            Ok(_) => {}
            Err(e) => assert_ne!(code(&e), "SPX-HPN005", "{e}"),
        }
    }
    assert!(w.invoker().run(&gen_params(None), &tok).is_ok());
}

#[test]
fn ma08_present_required_isolation_is_still_enforced_not_defaulted() {
    let w = World::new();
    let cfg = HostConfig {
        backend: IsolationBackend::unavailable(),
        ..HostConfig::default()
    };
    let inv = Invoker::new(&w.env, &w.project, cfg);
    let mut p = gen_params(None);
    p["isolation"] = json!("required");
    let e = inv.run(&p, &CancelToken::new()).unwrap_err();
    assert_eq!(code(&e), "SPX-HPC003", "{e}");
    assert_eq!(w.dispatches(), 0);
}

fn fault_on(state: &'static str, f: Fault) -> FaultHook {
    Arc::new(move |st| (st == state).then_some(f))
}

#[test]
fn ma09_failed_done_append_is_visible_and_never_retried_or_replayable() {
    for fault in [Fault::Write, Fault::Sync, Fault::Torn] {
        let w = World::new();
        let tok = CancelToken::new();
        let inv = w.invoker();
        inv.set_journal_fault(Some(fault_on("done", fault)));
        let r = inv.run(&gen_params(Some("s")), &tok).unwrap();
        // Provider outcome and durability outcome stay distinct.
        assert_eq!(r["state"], "completed", "{fault:?}: {r}");
        assert_eq!(r["durable"], false, "{fault:?}: {r}");
        assert_eq!(r["journal_error"]["code"], "SPX-HPD070", "{r}");
        assert_eq!(r["step"], "s");
        assert_eq!(w.dispatches(), 1);
        // Same session: the step is still refused and nothing is re-dispatched.
        inv.set_journal_fault(None);
        let e = inv.run(&gen_params(Some("s")), &tok).unwrap_err();
        assert!(
            matches!(code(&e), "SPX-HPN015" | "SPX-HPD070"),
            "{fault:?}: {e}"
        );
        // After restart: what reached storage decides, and it fails closed.
        let e = w.invoker().run(&gen_params(Some("s")), &tok).unwrap_err();
        assert!(
            matches!(code(&e), "SPX-HPN015" | "SPX-HPD070"),
            "{fault:?}: {e}"
        );
        assert_eq!(w.dispatches(), 1, "{fault:?}: provider was re-invoked");
        let text = w.journal_text();
        assert!(text.contains("\"state\":\"begin\""), "{text}");
        if fault == Fault::Write {
            assert!(!text.contains("\"state\":\"done\""), "{text}");
        }
    }
}

#[test]
fn ma09_healthy_settlement_has_no_journal_error_fields() {
    let w = World::new();
    let r = w
        .invoker()
        .run(&gen_params(Some("ok")), &CancelToken::new())
        .unwrap();
    assert_eq!(r["state"], "completed");
    assert!(
        r.get("durable").is_none() && r.get("journal_error").is_none(),
        "{r}"
    );
    assert!(w.journal_text().contains("\"state\":\"done\""));
}
