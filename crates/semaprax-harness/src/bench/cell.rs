//! One benchmark cell: a (task, profile, trial) run through the same host
//! verbs and library entry points a user's workflow uses.
//!
//! Steps: `context` (the `context` verb), `command` (`command_view::execute`,
//! exactly one host execution), `route` (`decision::decide`) and `workflow`
//! (the `run` verb, judged by the compiler). Acceptance is deterministic:
//! compiler/host verdicts and string presence, never a model's opinion.

use super::arena::{staged_descriptor, Arena};
use super::corpus::{Check, Corpus, Task};
use super::measure::Clock;
use super::router::{route, AdapterInvoker};
use crate::cli::{run, Outcome};
use crate::command_view::{execute, ExecOptions};
use crate::json::sha256_plain;
use crate::observe::event::{Observation, Role, Stage, TokenCount, Warmth};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct CellInput<'a> {
    pub corpus: &'a Corpus,
    pub arena: &'a Arena,
    pub task: &'a Task,
    pub trial: u32,
    pub warm: bool,
    pub clock: &'a dyn Clock,
}

#[derive(Clone, Debug, Default)]
pub struct CellResult {
    pub task: String,
    pub family: String,
    pub profile: String,
    pub trial: u32,
    pub warm: bool,
    /// `ok`, `failed` (ran, a provider or step broke) or `untested`.
    pub status: String,
    pub reason: Option<String>,
    pub accepted: bool,
    pub checks: Vec<(String, bool, String)>,
    pub facts_total: u32,
    pub facts_found: u32,
    pub facts_missing: Vec<String>,
    pub false_negatives: u32,
    pub false_negative_notes: Vec<String>,
    pub visible_bytes: u64,
    pub baseline_bytes: u64,
    pub final_paired_bytes: u64,
    pub incurred_bytes: u64,
    pub calls: u32,
    pub retries: u32,
    pub detail_retrievals: u32,
    pub latency_ms: u64,
    pub step_ms: BTreeMap<String, u64>,
    pub command_execs: Option<u32>,
    pub command_route: Option<String>,
    pub command_provider: Option<String>,
    pub route_choice: Option<String>,
    pub route_source: Option<String>,
    pub provider_status: Option<String>,
    pub workflow_status: Option<String>,
    pub permission_changed: bool,
    pub disk_bytes: u64,
}

impl CellResult {
    pub fn to_json(&self) -> Value {
        let checks: Vec<Value> = self
            .checks
            .iter()
            .map(|(k, p, n)| json!({"kind": k, "pass": p, "note": n}))
            .collect();
        json!({
            "task": self.task, "family": self.family, "profile": self.profile, "trial": self.trial,
            "temperature": if self.warm { "warm" } else { "cold" },
            "status": self.status, "reason": self.reason, "accepted": self.accepted, "checks": checks,
            "facts": {"total": self.facts_total, "found": self.facts_found, "missing": self.facts_missing},
            "false_negatives": {"count": self.false_negatives, "notes": self.false_negative_notes},
            "bytes": {"model_visible": self.visible_bytes, "baseline": self.baseline_bytes,
                      "final_paired": self.final_paired_bytes, "incurred": self.incurred_bytes, "unit": "byte-v1"},
            "calls": self.calls, "retries": self.retries, "detail_retrievals": self.detail_retrievals,
            "latency_ms": self.latency_ms, "step_ms": self.step_ms,
            "command": {"executions": self.command_execs, "route": self.command_route, "provider": self.command_provider},
            "route": {"choice": self.route_choice, "source": self.route_source},
            "provider_status": self.provider_status, "workflow_status": self.workflow_status,
            "permission_changed": self.permission_changed, "disk_bytes": self.disk_bytes,
        })
    }

    pub fn untested(i: &CellInput, why: &str) -> Self {
        Self {
            task: i.task.id.clone(),
            family: i.task.family.clone(),
            profile: i.arena.profile.id.clone(),
            trial: i.trial,
            warm: i.warm,
            status: "untested".into(),
            reason: Some(why.to_string()),
            ..Self::default()
        }
    }
}

fn verb(a: &Arena, args: &[String]) -> Outcome {
    run(args, &a.env)
}

fn st(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn first_line(o: &Outcome) -> String {
    o.stderr
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(200)
        .collect()
}

/// Strings that count as "seen by the model" for one context document: the
/// rendered document plus the decoded item texts and paths.
fn visible_corpus(doc_text: &str, doc: &Value) -> String {
    let mut s = doc_text.to_string();
    for k in ["native", "external"] {
        for it in doc[k].as_array().into_iter().flatten() {
            for f in ["text", "path"] {
                if let Some(t) = it[f].as_str() {
                    s.push('\n');
                    s.push_str(t);
                }
            }
        }
    }
    s
}

struct Ids<'a> {
    i: &'a CellInput<'a>,
    n: std::cell::Cell<u32>,
}

impl Ids<'_> {
    fn ev(&self, cap: &str, provider: &str, stage: Stage, role: Role) -> Observation {
        let n = self.n.get() + 1;
        self.n.set(n);
        let id = format!(
            "inv-{}-{}-{}-{n}",
            self.i.arena.profile.id, self.i.task.id, self.i.trial
        );
        let mut o = Observation::new(provider, cap, stage, role, &id);
        o.source_revision = self.i.corpus.digest.chars().take(23).collect();
        o.config_revision = self.i.arena.profile.id.chars().take(64).collect();
        o.warmth = if self.i.warm {
            Warmth::Warm
        } else {
            Warmth::Cold
        };
        o
    }
    fn payload(&self, step: &str) -> String {
        format!("{}#{}:{}", self.i.task.id, self.i.trial, step)
    }
}

/// Run one cell. Never panics on tool failure: the failure is the result.
pub fn run_cell(i: &CellInput) -> (CellResult, Vec<Observation>) {
    let mut r = CellResult {
        task: i.task.id.clone(),
        family: i.task.family.clone(),
        profile: i.arena.profile.id.clone(),
        trial: i.trial,
        warm: i.warm,
        status: "ok".into(),
        ..CellResult::default()
    };
    let mut obs = vec![];
    if let Some(why) = &i.arena.untested {
        return (CellResult::untested(i, why), obs);
    }
    let ids = Ids {
        i,
        n: std::cell::Cell::new(0),
    };
    let project = i.arena.projects[&i.task.project].clone();
    let t = i.task;
    let mut facts_text = String::new();
    let mut context_ok = true;

    if t.query.is_some() {
        context_ok = context_step(i, &ids, &project, &mut r, &mut obs, &mut facts_text);
    }
    for c in &t.checks {
        match c {
            Check::Facts => {
                r.facts_total = t.required_facts.len() as u32;
                r.facts_missing = t
                    .required_facts
                    .iter()
                    .filter(|f| !facts_text.contains(f.as_str()))
                    .cloned()
                    .collect();
                r.facts_found = r.facts_total - r.facts_missing.len() as u32;
                let pass = context_ok && r.facts_missing.is_empty();
                r.checks.push((
                    "facts".into(),
                    pass,
                    format!("{}/{} required facts visible", r.facts_found, r.facts_total),
                ));
            }
            Check::References { .. } => {} // judged inside context_step
            Check::Command { argv, critical } => {
                command_step(i, &ids, &project, argv, critical, &mut r, &mut obs)
            }
            Check::Route {
                request,
                allowed,
                forbidden,
            } => route_step(i, &ids, request, allowed, forbidden, &mut r, &mut obs),
            Check::Workflow {
                proposal,
                task,
                expect_status,
            } => workflow_step(
                i,
                &project,
                proposal,
                task.as_deref(),
                expect_status,
                &mut r,
            ),
        }
    }
    r.latency_ms = r.step_ms.values().sum();
    r.permission_changed = i.arena.trust_digest() != i.arena.trust_before;
    r.disk_bytes = i.arena.disk_bytes();
    r.accepted = r.status == "ok" && !r.checks.is_empty() && r.checks.iter().all(|(_, p, _)| *p);
    (r, obs)
}

fn context_step(
    i: &CellInput,
    ids: &Ids,
    project: &std::path::Path,
    r: &mut CellResult,
    obs: &mut Vec<Observation>,
    facts_text: &mut String,
) -> bool {
    let t = i.task;
    let query = t.query.clone().expect("checked");
    let refs = t.checks.iter().find_map(|c| match c {
        Check::References { symbol, callers } => Some((symbol.clone(), callers.clone())),
        _ => None,
    });
    let mut args = st(&[
        "context",
        project.to_str().unwrap_or(""),
        &query,
        "--max-bytes",
        &t.max_bytes.to_string(),
        "--json",
    ]);
    if let Some((sym, _)) = &refs {
        args.extend(st(&["--references", "--symbol", sym]));
    }
    let t0 = i.clock.now_ms();
    let o = verb(i.arena, &args);
    let ms = i.clock.now_ms().saturating_sub(t0);
    r.step_ms.insert("context".into(), ms);
    r.calls += 1;
    let doc: Value = serde_json::from_str(o.stdout.trim()).unwrap_or(Value::Null);
    if o.code != 0 || doc.is_null() {
        r.status = "failed".into();
        r.reason = Some(format!("context verb failed: {}", first_line(&o)));
        return false;
    }
    r.visible_bytes += o.stdout.len() as u64;
    *facts_text = visible_corpus(o.stdout.trim(), &doc);

    // Provider health: a required external provider that did not answer makes
    // the cell a failure of that profile, never a silent native-only result.
    let expected = i
        .arena
        .profile
        .installs
        .iter()
        .find(|x| x.capability == "context.repository");
    let mut provider_label = "semaprax.compiler".to_string();
    if let Some(inst) = expected {
        provider_label = inst.provider.clone();
        let p = doc["providers"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["provider_id"] == inst.provider.as_str());
        let status = p
            .and_then(|p| p["status"].as_str())
            .unwrap_or("absent")
            .to_string();
        r.provider_status = Some(status.clone());
        if !(status == "complete" || status == "partial") {
            let diag = p
                .and_then(|p| p["diagnostics"][0]["code"].as_str())
                .unwrap_or("none");
            r.status = "failed".into();
            r.reason = Some(format!(
                "provider `{}` status `{status}` ({diag})",
                inst.provider
            ));
        }
    }

    // Detail retrieval: when required facts are missing and items were omitted
    // by budget, a model would ask once more with a larger budget. The extra
    // bytes are incurred and counted.
    let mut doc = doc;
    if t.has_check("facts")
        && t.required_facts
            .iter()
            .any(|f| !facts_text.contains(f.as_str()))
        && doc["omitted"]["count"].as_u64().unwrap_or(0) > 0
    {
        let mut a2 = args.clone();
        if let Some(p) = a2.iter().position(|a| a == "--max-bytes") {
            a2[p + 1] = (t.max_bytes * 4).to_string();
        }
        let t1 = i.clock.now_ms();
        let o2 = verb(i.arena, &a2);
        *r.step_ms.entry("context".into()).or_default() += i.clock.now_ms().saturating_sub(t1);
        r.calls += 1;
        r.detail_retrievals += 1;
        if let Ok(d2) = serde_json::from_str::<Value>(o2.stdout.trim()) {
            r.visible_bytes += o2.stdout.len() as u64;
            r.incurred_bytes += o2.stdout.len() as u64;
            facts_text.push('\n');
            facts_text.push_str(&visible_corpus(o2.stdout.trim(), &d2));
            let mut e = ids.ev(
                "context.repository",
                &provider_label,
                Stage::RetrievalWrapper,
                Role::Incurred,
            );
            e.incurred = Some(TokenCount::bytes(o2.stdout.len() as u64));
            e.latency_ms = ms;
            obs.push(e);
            doc = d2;
        }
    }

    // Paired payload: unassisted baseline (whole baseline files) vs the context
    // document the model actually receives.
    let baseline: u64 = t
        .baseline_files
        .iter()
        .map(|f| std::fs::metadata(project.join(f)).map_or(0, |m| m.len()))
        .sum();
    if baseline > 0 {
        let first = o.stdout.len() as u64;
        r.baseline_bytes += baseline;
        r.final_paired_bytes += first;
        let mut e = ids.ev(
            "context.repository",
            &provider_label,
            Stage::ContextSelect,
            Role::Transform,
        );
        e.payload_id = Some(ids.payload("context"));
        e.before = Some(TokenCount::bytes(baseline));
        e.after = Some(TokenCount::bytes(first));
        e.model_visible = true;
        e.latency_ms = ms;
        e.before_digest = Some(sha256_plain(
            format!("baseline:{}:{}", t.id, baseline).as_bytes(),
        ));
        e.after_digest = Some(sha256_plain(o.stdout.as_bytes()));
        obs.push(e);
    }
    // Local-only cost: cold index/model load versus warm query.
    let mut l = ids.ev(
        "context.repository",
        &provider_label,
        if i.warm {
            Stage::ContextSelect
        } else {
            Stage::IndexBuild
        },
        Role::Local,
    );
    l.latency_ms = ms;
    obs.push(l);

    if let Some((_, callers)) = refs {
        let abs = doc["references"]["definitive_absence"]
            .as_bool()
            .unwrap_or(false);
        let missing: Vec<&String> = callers
            .iter()
            .filter(|c| !facts_text.contains(c.as_str()))
            .collect();
        if abs && !callers.is_empty() {
            r.false_negatives += 1;
            r.false_negative_notes
                .push("claimed definitive absence of references while callers exist".into());
        }
        r.checks.push((
            "references".into(),
            !abs && missing.is_empty(),
            format!(
                "{} of {} callers visible; definitive_absence={abs}",
                callers.len() - missing.len(),
                callers.len()
            ),
        ));
    }
    true
}

fn expand(argv: &[String], i: &CellInput, counter: &std::path::Path) -> Vec<String> {
    let python = i
        .arena
        .env
        .vars
        .get("HARNESS_PYTHON")
        .cloned()
        .unwrap_or_else(|| "/usr/bin/python3".into());
    argv.iter()
        .map(|a| {
            a.replace("{python}", &python)
                .replace("{counter}", &counter.display().to_string())
                .replace(
                    "{project}",
                    &i.arena.projects[&i.task.project].display().to_string(),
                )
                .replace(
                    "{projects}",
                    &i.arena.root.join("projects").display().to_string(),
                )
        })
        .collect()
}

fn command_step(
    i: &CellInput,
    ids: &Ids,
    project: &std::path::Path,
    argv: &[String],
    critical: &[String],
    r: &mut CellResult,
    obs: &mut Vec<Observation>,
) {
    let dir = i.arena.root.join("counters");
    let _ = std::fs::create_dir_all(&dir);
    let counter = dir.join(format!("{}-{}", i.task.id, i.trial));
    let _ = std::fs::remove_file(&counter);
    let argv = expand(argv, i, &counter);
    let t0 = i.clock.now_ms();
    let rep = execute(&i.arena.env, project, &argv, &ExecOptions::default(), None);
    r.step_ms
        .insert("command".into(), i.clock.now_ms().saturating_sub(t0));
    r.calls += 1;
    let rep = match rep {
        Ok(rep) => rep,
        Err(d) => {
            r.status = "failed".into();
            r.reason = Some(format!("command step refused: {d}"));
            r.checks
                .push(("command".into(), false, "host refused the command".into()));
            return;
        }
    };
    let execs = std::fs::read_to_string(&counter)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);
    r.command_execs = Some(execs);
    let v = &rep.envelope.view;
    r.command_route = Some(v.route.clone());
    r.command_provider = Some(v.provenance.clone());
    let display = rep.display;
    r.visible_bytes += display.len() as u64;
    if let Some(inst) = i
        .arena
        .profile
        .installs
        .iter()
        .find(|x| x.capability == "command.view")
    {
        if v.notes
            .iter()
            .any(|n| n.contains("provider unavailable") || n.contains("provider failed"))
        {
            r.status = "failed".into();
            r.reason = Some(format!(
                "provider `{}` failed during the view: {}",
                inst.provider,
                v.notes.join("; ").chars().take(160).collect::<String>()
            ));
        }
    }
    let raw_bytes = rep.envelope.result.stdout.bytes + rep.envelope.result.stderr.bytes;
    let mut e = ids.ev(
        "command.view",
        &v.provenance,
        Stage::CommandView,
        Role::Transform,
    );
    e.payload_id = Some(ids.payload("command"));
    e.before = Some(TokenCount::bytes(raw_bytes));
    e.after = Some(TokenCount::bytes(display.len() as u64));
    e.model_visible = true;
    e.latency_ms = r.step_ms["command"];
    e.before_digest = Some(rep.envelope.result.stdout.digest.clone());
    e.after_digest = Some(sha256_plain(display.as_bytes()));
    obs.push(e);
    r.baseline_bytes += raw_bytes;
    r.final_paired_bytes += display.len() as u64;

    // Critical lines must survive; recovery is one counted detail retrieval.
    let mut missing: Vec<&String> = critical
        .iter()
        .filter(|c| !display.contains(c.as_str()))
        .collect();
    if !missing.is_empty() {
        if let Some(h) = &v.recovery_handle {
            let rec = verb(
                i.arena,
                &st(&[
                    "recover",
                    project.to_str().unwrap_or(""),
                    h,
                    "--limit",
                    "4194304",
                    "--json",
                ]),
            );
            r.calls += 1;
            r.detail_retrievals += 1;
            r.visible_bytes += rec.stdout.len() as u64;
            r.incurred_bytes += rec.stdout.len() as u64;
            let mut ie = ids.ev(
                "command.view",
                &v.provenance,
                Stage::RetrievalWrapper,
                Role::Incurred,
            );
            ie.incurred = Some(TokenCount::bytes(rec.stdout.len() as u64));
            obs.push(ie);
            missing.retain(|c| !rec.stdout.contains(c.as_str()));
        }
    }
    if !missing.is_empty() && !v.incomplete {
        r.false_negatives += 1;
        r.false_negative_notes
            .push("critical line absent from a view that claims completeness".into());
    }
    r.checks.push((
        "command".into(),
        missing.is_empty() && execs == 1,
        format!(
            "{} critical missing; executions={execs}; route={}",
            missing.len(),
            v.route
        ),
    ));
}

fn route_step(
    i: &CellInput,
    ids: &Ids,
    request: &Value,
    allowed: &[String],
    forbidden: &[String],
    r: &mut CellResult,
    obs: &mut Vec<Observation>,
) {
    let t0 = i.clock.now_ms();
    let lineage = format!("{}-{}-{}", i.arena.profile.id, i.task.id, i.trial);
    let out = match &i.arena.profile.router {
        None => route(request, None, &lineage),
        Some(spec) => {
            let desc: PathBuf = staged_descriptor(&i.arena.root, &spec.descriptor);
            match AdapterInvoker::spawn(spec, &desc, &i.arena.env) {
                Ok(mut inv) => {
                    let o = route(request, Some((&mut inv, spec)), &lineage);
                    o.map(|mut o| {
                        o.request_bytes = inv.request_bytes;
                        o
                    })
                }
                Err(d) => Err(d),
            }
        }
    };
    let ms = i.clock.now_ms().saturating_sub(t0);
    r.step_ms.insert("route".into(), ms);
    r.calls += 1;
    match out {
        Err(d) => {
            r.status = "failed".into();
            r.reason = Some(format!("route step failed: {d}"));
            r.checks
                .push(("route".into(), false, "decision refused".into()));
        }
        Ok(o) => {
            let ok = allowed.contains(&o.choice) && !forbidden.contains(&o.choice);
            if forbidden.contains(&o.choice) {
                r.false_negatives += 1;
                r.false_negative_notes
                    .push(format!("route chose forbidden option `{}`", o.choice));
            }
            if o.router_calls > 0 {
                r.incurred_bytes += o.request_bytes;
                let mut e = ids.ev(
                    "decision.evaluate",
                    &o.provider_id,
                    Stage::Decision,
                    Role::Incurred,
                );
                e.incurred = Some(TokenCount::bytes(o.request_bytes));
                e.latency_ms = ms;
                obs.push(e);
            } else {
                let mut e = ids.ev(
                    "decision.evaluate",
                    &o.provider_id,
                    Stage::Decision,
                    Role::Local,
                );
                e.latency_ms = ms;
                obs.push(e);
            }
            r.checks.push((
                "route".into(),
                ok,
                format!(
                    "choice `{}` via {} ({} router calls)",
                    o.choice, o.source, o.router_calls
                ),
            ));
            r.route_choice = Some(o.choice);
            r.route_source = Some(o.source);
        }
    }
}

fn workflow_step(
    i: &CellInput,
    project: &std::path::Path,
    proposal: &str,
    task: Option<&str>,
    expect: &str,
    r: &mut CellResult,
) {
    if i.arena.env.compiler.is_none() {
        r.checks
            .push(("workflow".into(), false, "no compiler configured".into()));
        r.status = "untested".into();
        r.reason = Some("untested: no compiler (set SEMAPRAX_COMPILER)".into());
        return;
    }
    let mut args = st(&[
        "run",
        project.to_str().unwrap_or(""),
        "--proposal",
        i.corpus.dir.join(proposal).to_str().unwrap_or(""),
        "--json",
    ]);
    if let Some(tk) = task {
        args.extend(st(&[
            "--task",
            i.corpus.dir.join(tk).to_str().unwrap_or(""),
        ]));
    }
    let t0 = i.clock.now_ms();
    let o = verb(i.arena, &args);
    r.step_ms
        .insert("workflow".into(), i.clock.now_ms().saturating_sub(t0));
    r.calls += 1;
    let v: Value = serde_json::from_str(o.stdout.trim()).unwrap_or(Value::Null);
    let status = v["status"].as_str().unwrap_or("").to_string();
    // The verdict the model receives: status, refusals and check results only.
    let verdict = json!({"status": v["status"], "refusals": v["refusals"], "checks": v["checks"]});
    r.visible_bytes += verdict.to_string().len() as u64;
    r.workflow_status = Some(status.clone());
    r.checks.push((
        "workflow".into(),
        status == expect,
        format!("compiler-judged status `{status}`, expected `{expect}`"),
    ));
}
