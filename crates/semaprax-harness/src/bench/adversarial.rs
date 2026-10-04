//! Seeded adversarial cases. Each case plants one defect (a lying provider, a
//! stale index, a hostile router, a weakening candidate) and the benchmark must
//! DETECT it. Ground truth always comes from an independent read of the
//! fixture files, never from the component under test.
//!
//! A case that is not detected is a protected-fact loss and fails gate G1.

use super::arena::Arena;
use super::corpus::{AdversarialSpec, Corpus, Install, ProfileSpec};
use super::router::{route, ScriptedInvoker};
use crate::cli::run;
use crate::command_view::{execute, ExecOptions};
use crate::json::sha256_plain;
use crate::workflow::compiler::{CandidatePreview, SourceChange};
use crate::workflow::policy::{check_protected_facts, source_digest, REQUIREMENTS};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ADV_DESC: &str = "adapters/adversarial/harness-provider.json";
const ADV_ID: &str = "org.example/bench-adversarial";

#[derive(Clone, Debug, PartialEq)]
pub struct AdvResult {
    pub id: String,
    pub kind: String,
    /// The harness caught the seeded defect.
    pub detected: bool,
    /// Why the case could not run here (never counted as detected).
    pub untested: Option<String>,
    pub evidence: Vec<String>,
}

impl AdvResult {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "kind": self.kind, "detected": self.detected,
               "untested": self.untested, "evidence": self.evidence})
    }
}

pub struct AdvEnv<'a> {
    pub corpus: &'a Corpus,
    pub repo: &'a Path,
    pub work: &'a Path,
    pub vars: &'a BTreeMap<String, String>,
    pub compiler: Option<PathBuf>,
}

fn install(cap: &str) -> Install {
    Install {
        descriptor: ADV_DESC.into(),
        provider: ADV_ID.into(),
        capability: cap.into(),
        upstream_env: None,
        strip_upstream: false,
    }
}

fn arena(e: &AdvEnv, spec: &AdversarialSpec, caps: &[&str]) -> Arena {
    let p = ProfileSpec {
        id: format!("adv-{}", spec.id),
        baseline: false,
        description: "seeded adversarial arena".into(),
        installs: caps.iter().map(|c| install(c)).collect(),
        router: None,
        requires_env: vec!["HARNESS_PYTHON".into()],
        command_view_policy: Some(json!({"min_bytes": 64})),
        skip: None,
        skills: vec![],
    };
    Arena::prepare(e.corpus, &p, e.repo, e.work, e.vars, e.compiler.clone())
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn result(spec: &AdversarialSpec, detected: bool, evidence: Vec<String>) -> AdvResult {
    AdvResult {
        id: spec.id.clone(),
        kind: spec.kind.clone(),
        detected,
        untested: None,
        evidence,
    }
}

fn untested(spec: &AdversarialSpec, why: &str) -> AdvResult {
    AdvResult {
        id: spec.id.clone(),
        kind: spec.kind.clone(),
        detected: false,
        untested: Some(why.into()),
        evidence: vec![],
    }
}

fn ctx_doc(a: &Arena, project: &Path, args: &[&str]) -> Value {
    let mut v = s(&["context", project.to_str().unwrap_or("")]);
    v.extend(s(args));
    v.push("--json".into());
    let o = run(&v, &a.env);
    serde_json::from_str(o.stdout.trim())
        .unwrap_or_else(|_| json!({"_error": o.stderr.lines().next().unwrap_or("")}))
}

/// Lines (path:line) of `.spx` files that mention `word` outside its definition.
fn callers_on_disk(project: &Path, word: &str) -> Vec<String> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(project.join("src")) {
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for f in files {
            let name = f
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let text = std::fs::read_to_string(&f).unwrap_or_default();
            if name.ends_with(".spx")
                && text
                    .lines()
                    .any(|l| l.contains(&format!("{word}(")) && !l.contains("fn "))
            {
                out.push(format!("src/{name}"));
            }
        }
    }
    out
}

/// A provider that skipped files reports "no references"; the host must not
/// turn that into a claim of absence.
fn missing_callers(e: &AdvEnv, spec: &AdversarialSpec) -> AdvResult {
    let a = arena(e, spec, &["context.repository"]);
    if let Some(why) = &a.untested {
        return untested(spec, why);
    }
    let project = a.projects[&spec.project].clone();
    let truth = callers_on_disk(&project, "multiply");
    let _ = std::fs::write(project.join(".bench-mode"), "incomplete-no-refs");
    let doc = ctx_doc(
        &a,
        &project,
        &[
            "calculator.multiply",
            "--references",
            "--symbol",
            "multiply",
            "--max-bytes",
            "4096",
        ],
    );
    let absence = doc["references"]["definitive_absence"].as_bool();
    let exhaustive = doc["references"]["exhaustive"].as_bool();
    let status = doc["providers"][0]["status"]
        .as_str()
        .unwrap_or("?")
        .to_string();
    let diag = doc["providers"][0]["diagnostics"][0]["code"]
        .as_str()
        .unwrap_or("none")
        .to_string();
    // Second leg: an honest partial index (no absence claim) must also never yield definitive absence.
    let _ = std::fs::write(project.join(".bench-mode"), "partial-honest");
    let doc2 = ctx_doc(
        &a,
        &project,
        &[
            "calculator.multiply",
            "--references",
            "--symbol",
            "multiply",
            "--max-bytes",
            "4096",
        ],
    );
    let status2 = doc2["providers"][0]["status"]
        .as_str()
        .unwrap_or("?")
        .to_string();
    let absence2 = doc2["references"]["definitive_absence"].as_bool();
    let detected = !truth.is_empty()
        && absence == Some(false)
        && exhaustive != Some(true)
        && absence2 == Some(false)
        && status2 == "partial";
    result(
        spec,
        detected,
        vec![
            format!("lying leg: provider diagnostic `{diag}`; honest-partial leg: status `{status2}`, definitive_absence={absence2:?}"),
            format!("ground truth callers on disk: {truth:?}"),
            format!("provider claimed no_references with an incomplete index (status {status})"),
            format!("host definitive_absence={absence:?} exhaustive={exhaustive:?}"),
        ],
    )
}

/// One failing line in repetitive output must survive a provider view that
/// drops it, or the view must say it is incomplete.
fn hidden_critical_error(e: &AdvEnv, spec: &AdversarialSpec) -> AdvResult {
    let a = arena(e, spec, &["command.view"]);
    if let Some(why) = &a.untested {
        return untested(spec, why);
    }
    let project = a.projects[&spec.project].clone();
    let counter = a.root.join("counter-hidden");
    let argv = vec![
        a.env.vars["HARNESS_PYTHON"].clone(),
        project.join("noisy_test_run.py").display().to_string(),
        "--counter".into(),
        counter.display().to_string(),
        "view-drop".into(),
    ];
    match execute(&a.env, &project, &argv, &ExecOptions::default(), None) {
        Ok(rep) => {
            let v = &rep.envelope.view;
            let kept = rep.display.contains("case_173 ... FAILED");
            let detected = kept && v.incomplete && v.route == "provider";
            result(
                spec,
                detected,
                vec![
                    format!("provider view said `all tests passed` with lossless=true; route={} incomplete={}", v.route, v.incomplete),
                    format!("critical failing line present in model display: {kept}"),
                ],
            )
        }
        Err(d) => result(spec, false, vec![format!("exec refused: {d}")]),
    }
}

/// An index frozen before an edit must not present its old spans as current.
fn stale_graph(e: &AdvEnv, spec: &AdversarialSpec) -> AdvResult {
    let a = arena(e, spec, &["context.repository"]);
    if let Some(why) = &a.untested {
        return untested(spec, why);
    }
    let project = a.projects[&spec.project].clone();
    let _ = std::fs::write(project.join(".bench-mode"), "stale");
    let q = ["calculator.multiply", "--max-bytes", "8192"];
    let before = ctx_doc(&a, &project, &q);
    let core = project.join("src/core.spx");
    let old = std::fs::read_to_string(&core).unwrap_or_default();
    let edited = old.replace("left * right", "right * left");
    let _ = std::fs::write(&core, &edited);
    let after = ctx_doc(&a, &project, &q);
    let current: Vec<String> = edited.lines().map(str::to_string).collect();
    let mut offered_stale = 0;
    let mut claimed_current = 0;
    for it in after["external"].as_array().into_iter().flatten() {
        let (path, line) = (
            it["path"].as_str().unwrap_or(""),
            it["span"]["start_line"].as_u64().unwrap_or(0) as usize,
        );
        let text = it["text"].as_str().unwrap_or("");
        if path == "src/core.spx"
            && line >= 1
            && current.get(line - 1).map(String::as_str) != Some(text)
        {
            offered_stale += 1;
            if it["verified"].as_bool() == Some(true) {
                claimed_current += 1;
            }
        }
    }
    let detected = offered_stale > 0 && claimed_current == 0;
    let n_before = before["external"].as_array().map_or(0, Vec::len);
    result(
        spec,
        detected,
        vec![
            format!("index frozen at first query ({n_before} items), then src/core.spx was edited"),
            format!("stale spans offered after the edit: {offered_stale}; presented as verified: {claimed_current}"),
            format!(
                "after the edit the host reported provider status `{}` ({} external items, {} offered, document {}, error {})",
                after["providers"][0]["status"].as_str().unwrap_or("?"),
                after["external"].as_array().map_or(0, Vec::len),
                after["providers"][0]["offered_items"],
                if after["_error"].is_null() { "present" } else { "missing" },
                after["_error"].as_str().unwrap_or("none")
            ),
        ],
    )
}

/// A router that names a forbidden or unknown option must not decide the route.
fn wrong_router_choice(_e: &AdvEnv, spec: &AdversarialSpec) -> AdvResult {
    let request = json!({
        "task": "model-route/v1",
        "features": {"task_family": "localized_debug", "estimated_context_tokens": 4000, "requires_structured_output": false,
                     "requires_tools": false, "confidentiality": "project", "latency_class": "interactive"},
        "budget": {"max_cost_micros": 1000000, "max_latency_ms": 60000, "max_router_calls": 1},
        "catalog": [
            {"id": "m-local", "destination": {"kind": "local"}, "capabilities": [], "max_context": 100000,
             "est_cost_micros": 10, "est_latency_ms": 1000, "strength_rank": 1},
            {"id": "m-mid", "destination": {"kind": "local"}, "capabilities": [], "max_context": 100000,
             "est_cost_micros": 30, "est_latency_ms": 1000, "strength_rank": 2},
            {"id": "m-small", "destination": {"kind": "local"}, "capabilities": [], "max_context": 1000,
             "est_cost_micros": 1, "est_latency_ms": 100, "strength_rank": 1}
        ]
    });
    let spec_r = super::corpus::RouterSpec {
        descriptor: "-".into(),
        runtime_env: "-".into(),
        env: BTreeMap::new(),
        provider_id: "org.example/hostile-router".into(),
        model_id: "scripted".into(),
        checkpoint: "seeded".into(),
    };
    let mut ev = vec![];
    let mut all_safe = true;
    for (label, choice) in [
        ("option screened out by the context-size policy", "m-small"),
        ("option outside the offered set", "m-ghost"),
    ] {
        let mut inv = ScriptedInvoker {
            answer: json!({"choice": choice, "scores": {"m-local": 0.1, "m-mid": 0.2, "m-small": 0.9}, "abstain": false}),
            calls: 0,
        };
        let out = route(&request, Some((&mut inv, &spec_r)), "adv-router");
        match out {
            Ok(o) => {
                let safe = o.choice != "m-small"
                    && o.choice != "m-ghost"
                    && o.source.starts_with("Fallback");
                all_safe &= safe;
                ev.push(format!(
                    "router named `{choice}` ({label}); final choice `{}` via {}",
                    o.choice, o.source
                ));
            }
            Err(d) => {
                all_safe = false;
                ev.push(format!(
                    "{label}: decision errored instead of falling back: {d}"
                ));
            }
        }
    }
    result(spec, all_safe, ev)
}

/// A failing or crashing view provider must never cause the command to run twice.
fn double_execution(e: &AdvEnv, spec: &AdversarialSpec) -> AdvResult {
    let a = arena(e, spec, &["command.view"]);
    if let Some(why) = &a.untested {
        return untested(spec, why);
    }
    let project = a.projects[&spec.project].clone();
    let mut ok = true;
    let mut ev = vec![];
    for mode in ["view-crash", "view-fail"] {
        let counter = a.root.join(format!("counter-{mode}"));
        let _ = std::fs::remove_file(&counter);
        let argv = vec![
            a.env.vars["HARNESS_PYTHON"].clone(),
            project.join("noisy_test_run.py").display().to_string(),
            "--counter".into(),
            counter.display().to_string(),
            mode.into(),
        ];
        let rep = execute(&a.env, &project, &argv, &ExecOptions::default(), None);
        let runs = std::fs::read_to_string(&counter)
            .ok()
            .and_then(|t| t.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let route = rep
            .as_ref()
            .map(|r| r.envelope.view.route.clone())
            .unwrap_or_else(|d| d.code.to_string());
        ok &= runs == 1 && rep.is_ok();
        ev.push(format!(
            "{mode}: executions on disk = {runs}; fallback route = {route}"
        ));
    }
    result(spec, ok, ev)
}

/// A candidate that deletes an `ensures` or widens `uses` must be refused by
/// the host's protected-fact check, independent of any model claim.
fn law_weakening(e: &AdvEnv, spec: &AdversarialSpec) -> AdvResult {
    let a = arena(e, spec, &[]);
    let project = a.projects[&spec.project].clone();
    let base = std::fs::read(project.join("src/lib.spx")).unwrap_or_default();
    let base_src = String::from_utf8_lossy(&base).into_owned();
    let preview = |src: String| CandidatePreview {
        base_revision: "sha256:r".into(),
        candidate_revision: sha256_plain(src.as_bytes()),
        source_changes: vec![SourceChange {
            path: "src/lib.spx".into(),
            base_digest: source_digest(&base),
            candidate_digest: sha256_plain(src.as_bytes()),
            replacement_source: src,
        }],
        requirements: REQUIREMENTS.iter().map(|x| x.to_string()).collect(),
        change_requirements: vec![REQUIREMENTS.iter().map(|x| x.to_string()).collect()],
        unresolved_holes: 0,
        tests_state: "not_run".into(),
        digest: "sha256:d".into(),
    };
    let weakened = base_src.replace("    ensures result == price * qty\n", "");
    let widened = base_src.replace(
        "{\n    price + qty",
        "    uses { clock.read }\n{\n    price + qty",
    );
    let mut ev = vec![];
    let mut ok = true;
    for (label, src, code) in [
        ("deleted `ensures`", weakened, "SPX-HPD042"),
        ("added `uses { clock.read }`", widened, "SPX-HPD043"),
    ] {
        match check_protected_facts(&project, "sha256:r", "replace_function_body", &preview(src)) {
            Err(d) if d.code == code => ev.push(format!("{label}: refused with {}", d.code)),
            other => {
                ok = false;
                ev.push(format!("{label}: NOT refused as {code}: {other:?}"));
            }
        }
    }
    // With a compiler, the whole workflow also refuses a proposal that tries to
    // pass its own (empty) requirements.
    if a.env.compiler.is_some() {
        let o = run(
            &s(&[
                "run",
                project.to_str().unwrap_or(""),
                "--proposal",
                e.corpus
                    .dir
                    .join("proposals/weak-requirements.json")
                    .to_str()
                    .unwrap_or(""),
                "--json",
            ]),
            &a.env,
        );
        let v: Value = serde_json::from_str(o.stdout.trim()).unwrap_or(Value::Null);
        let code = v["refusals"][0]["code"].as_str().unwrap_or("-").to_string();
        ok &= code == "SPX-HPD032";
        ev.push(format!(
            "compiler workflow: weak requirements proposal refused with {code}"
        ));
    } else {
        ev.push("compiler workflow leg untested: no compiler configured".into());
    }
    result(spec, ok, ev)
}

/// A descriptor widened after approval must not run on the old grant.
fn permission_widening(e: &AdvEnv, spec: &AdversarialSpec) -> AdvResult {
    let a = arena(e, spec, &["context.repository"]);
    if let Some(why) = &a.untested {
        return untested(spec, why);
    }
    let project = a.projects[&spec.project].clone();
    let base = ctx_doc(
        &a,
        &project,
        &["calculator.multiply", "--max-bytes", "4096"],
    );
    let before = base["providers"][0]["status"]
        .as_str()
        .unwrap_or("?")
        .to_string();
    let grant_before = a.trust_digest();
    let desc = super::arena::staged_descriptor(&a.root, ADV_DESC);
    let mut d: Value =
        serde_json::from_slice(&std::fs::read(&desc).unwrap_or_default()).unwrap_or(Value::Null);
    d["permissions"]["network"] = json!(["loopback:*"]);
    let _ = std::fs::write(&desc, d.to_string());
    let after = ctx_doc(
        &a,
        &project,
        &["calculator.multiply", "--max-bytes", "4096"],
    );
    let status = after["providers"][0]["status"]
        .as_str()
        .unwrap_or("?")
        .to_string();
    let detected = (before == "complete" || before == "partial")
        && !(status == "complete" || status == "partial")
        && grant_before == a.trust_digest();
    result(
        spec,
        detected,
        vec![
            format!("provider status before widening: {before}; after the descriptor requested network: {status}"),
            "the stored grant was not modified by the run".into(),
        ],
    )
}

pub fn run_adversarial(e: &AdvEnv) -> Vec<AdvResult> {
    e.corpus
        .adversarial
        .iter()
        .map(|spec| match spec.kind.as_str() {
            "missing_callers" => missing_callers(e, spec),
            "hidden_critical_error" => hidden_critical_error(e, spec),
            "stale_graph" => stale_graph(e, spec),
            "wrong_router_choice" => wrong_router_choice(e, spec),
            "double_execution" => double_execution(e, spec),
            "law_weakening" => law_weakening(e, spec),
            "permission_widening" => permission_widening(e, spec),
            other => untested(spec, &format!("unknown adversarial kind `{other}`")),
        })
        .collect()
}
