//! Bounded multi-step session (HN-02): context -> propose -> compiler preview
//! -> check -> feedback -> revised proposal, with whole-task bounds. Work happens
//! in a private scratch copy; the project is never written here. An unverified
//! baseline is repaired by a bounded scratch patch (`session_repair`) before any
//! semantic operation. Publication stays a separate authority.

use super::attempt::{self, PromptCtx};
use super::journal::Journal;
use super::pipeline::{
    expand_context, follow_up_context, gather_context, present_and_publish, step, Ctx, Stages,
};
use super::report::Report;
use super::session_repair as repair;
use super::snapshot::Snapshot;
use super::stages::*;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cooperative cancellation shared with the caller (checked between steps).
pub const SESSION_FILE: &str = "semaprax.harness-session.json";

pub type CancelFlag = Arc<AtomicBool>;

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Whole-task bounds across every attempt of the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionBounds {
    pub max_attempts: u32,
    pub max_candidates: u32,
    pub max_tool_calls: u32,
    pub max_elapsed_ms: u64,
    pub max_tokens: Option<u64>,
    /// Admitted steps (verified candidates) before the session must stop.
    pub max_steps: u32,
}

impl Default for SessionBounds {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            max_candidates: 8,
            max_tool_calls: 400,
            max_elapsed_ms: 600_000,
            max_tokens: None,
            max_steps: 4,
        }
    }
}

impl SessionBounds {
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        let bad = |m: String| d("SPX-HPD081", m);
        let o = v
            .as_object()
            .ok_or_else(|| bad("`session` must be an object".into()))?;
        let mut b = Self::default();
        for (k, x) in o {
            let n = x
                .as_u64()
                .ok_or_else(|| bad(format!("`session.{k}` must be a number")))?;
            match k.as_str() {
                "max_attempts" => b.max_attempts = n as u32,
                "max_candidates" => b.max_candidates = n as u32,
                "max_tool_calls" => b.max_tool_calls = n as u32,
                "max_elapsed_ms" => b.max_elapsed_ms = n,
                "max_tokens" => b.max_tokens = Some(n),
                "max_steps" => b.max_steps = n as u32,
                _ => return Err(bad(format!("unknown session member `{k}`"))),
            }
        }
        Ok(b)
    }
    pub fn to_json(&self) -> Value {
        json!({"max_attempts": self.max_attempts, "max_candidates": self.max_candidates,
               "max_tool_calls": self.max_tool_calls, "max_elapsed_ms": self.max_elapsed_ms,
               "max_tokens": self.max_tokens, "max_steps": self.max_steps})
    }
}

/// Exact baseline bytes of every snapshotted file, captured once.
pub(super) type Baseline = BTreeMap<String, Vec<u8>>;

pub(super) struct State {
    pub bounds: SessionBounds,
    pub work: PathBuf,
    pub baseline: Baseline,
    pub oracle: BTreeSet<String>,
    pub attempts: Vec<Value>,
    pub steps: Vec<Value>,
    pub feedback: Vec<Value>,
    /// Check output delivered to the model in feedback (HN-12), separate from report compaction.
    pub delivered: Vec<Value>,
    pub candidates: u32,
    pub seen_proposals: BTreeSet<String>,
    pub last_failures: Vec<String>,
    pub commands_start: usize,
    pub result: Value,
}

impl State {
    fn to_json(&self, cx: &Ctx) -> Value {
        json!({"bounds": self.bounds.to_json(), "attempts_spent": self.attempts.len(),
               "candidates_admitted": self.candidates, "attempts": self.attempts, "steps": self.steps,
               "tool_calls": cx.compiler.commands().len().saturating_sub(self.commands_start),
               "reserved_tokens": cx.ledger.reserved_tokens(), "delivered_to_model": self.delivered_json(),
               "result": self.result})
    }

    /// Delivered-to-model savings of check output carried in feedback. Counts the
    /// view the next request actually contained; unavailable counts claim nothing.
    fn delivered_json(&self) -> Value {
        let tokens: Vec<i64> = self
            .delivered
            .iter()
            .filter_map(|d| d["saved_tokens"].as_i64())
            .collect();
        json!({"entries": self.delivered,
               "saved_tokens": if !self.delivered.is_empty() && tokens.len() == self.delivered.len() { json!(tokens.iter().sum::<i64>()) } else { Value::Null },
               "note": "tokens the model was shown, not post-run report compaction (checks.commands)"})
    }

    pub(super) fn check_bounds_pub(&self, cx: &Ctx) -> HarnessResult<()> {
        self.check_bounds(cx)
    }
    pub(super) fn cancelled_pub(&self, cx: &Ctx, j: &mut Journal) -> HarnessResult<()> {
        cancelled(cx, j)
    }
    pub(super) fn record_failure_pub(
        &mut self,
        n: u32,
        stage: &str,
        code: &str,
        message: &str,
        j: &mut Journal,
    ) -> HarnessResult<()> {
        self.record_failure(n, stage, code, message, j)
    }

    fn check_bounds(&self, cx: &Ctx) -> HarnessResult<()> {
        let b = &self.bounds;
        let over = |what: &str| {
            d(
                "SPX-HPD111",
                format!(
                    "session bound exhausted: {what}; {} attempt(s) were spent and remain counted",
                    self.attempts.len()
                ),
            )
        };
        if self.attempts.len() as u32 >= b.max_attempts {
            return Err(over(&format!("max_attempts {}", b.max_attempts)));
        }
        if self.candidates >= b.max_candidates {
            return Err(over(&format!("max_candidates {}", b.max_candidates)));
        }
        if cx
            .compiler
            .commands()
            .len()
            .saturating_sub(self.commands_start) as u32
            >= b.max_tool_calls
        {
            return Err(over(&format!("max_tool_calls {}", b.max_tool_calls)));
        }
        if cx.started.elapsed().as_millis() as u64 >= b.max_elapsed_ms {
            return Err(over(&format!("max_elapsed_ms {}", b.max_elapsed_ms)));
        }
        if b.max_tokens
            .is_some_and(|m| cx.ledger.reserved_tokens() >= m)
        {
            return Err(over("max_tokens"));
        }
        Ok(())
    }

    fn record_failure(
        &mut self,
        n: u32,
        stage: &str,
        code: &str,
        message: &str,
        journal: &mut Journal,
    ) -> HarnessResult<()> {
        let digest = sha256_plain(format!("{code}\n{message}").as_bytes());
        self.feedback
            .push(json!({"attempt": n, "stage": stage, "code": code, "message": message}));
        // Keep the model-visible history bounded: the newest 4 failures.
        if self.feedback.len() > 4 {
            self.feedback.remove(0);
        }
        self.last_failures.push(digest.clone());
        if let Some(a) = self.attempts.last_mut() {
            a["outcome"] = json!("rejected");
            a["stage"] = json!(stage);
            a["code"] = json!(code);
            a["diagnostic_digest"] = json!(digest);
        }
        journal.append(
            &format!("attempt-{n}"),
            "done",
            self.attempts.last().cloned().unwrap_or(Value::Null),
        )?;
        let k = self.last_failures.len();
        if k >= 3 && self.last_failures[k - 3..].iter().all(|x| *x == digest) {
            return Err(d("SPX-HPD112", "no progress: the same diagnostic digest three attempts in a row; stopping with all attempts counted"));
        }
        Ok(())
    }
}

fn read_baseline(snapshot: &Snapshot) -> HarnessResult<Baseline> {
    let mut out = Baseline::new();
    for (path, digest) in &snapshot.files {
        let bytes = std::fs::read(snapshot.root.join(path))
            .map_err(|e| d("SPX-HPD004", format!("baseline {path}: {e}")))?;
        if &sha256_plain(&bytes) != digest {
            return Err(d(
                "SPX-HPD005",
                format!("stale revision: `{path}` changed while capturing the baseline"),
            ));
        }
        out.insert(path.clone(), bytes);
    }
    Ok(out)
}

fn start(cx: &mut Ctx, journal: &mut Journal, r: &mut Report) -> HarnessResult<State> {
    let cfg = cx.cfg;
    let bounds = cfg.task.session.clone().unwrap_or_default();
    let baseline = read_baseline(&cfg.snapshot)?;
    let oracle = repair::oracle_files(&baseline);
    let work = cfg.cache_dir.join(format!("work-{}", cx.lineage.id));
    attempt::copy_project(&cfg.snapshot.root, &work)?;
    let work = work
        .canonicalize()
        .map_err(|e| d("SPX-HPD070", format!("work: {e}")))?;
    if journal.state("session").is_none() {
        journal.append(
            "session",
            "begin",
            json!({
            "task": cx.lineage.task_digest, "lock": cx.lineage.lock_digest,
            "toolchain": sha256_plain(cx.compiler.version().as_bytes()),
            "baseline": cfg.snapshot.revision,
            "skills": cfg.skill_prompt.as_ref().map(|s| sha256_plain(s.text.as_bytes())),
            "bounds": bounds.to_json()}),
        )?;
    }
    r.notes
        .push("session work happens in a private scratch copy; the project is not written".into());
    Ok(State {
        bounds,
        work,
        baseline,
        oracle,
        attempts: vec![],
        steps: vec![],
        feedback: vec![],
        delivered: vec![],
        candidates: 0,
        seen_proposals: BTreeSet::new(),
        last_failures: vec![],
        commands_start: cx.compiler.commands().len(),
        result: Value::Null,
    })
}

fn cancelled(cx: &Ctx, journal: &mut Journal) -> HarnessResult<()> {
    if cx
        .cfg
        .cancel
        .as_ref()
        .is_some_and(|c| c.load(Ordering::SeqCst))
    {
        journal.append("session", "cancelled", json!({}))?;
        return Err(d(
            "SPX-HPD113",
            "session cancelled by the caller; recorded, nothing in flight is replayed",
        ));
    }
    Ok(())
}

const MIGRATING: [&str; 4] = [
    "rename_declaration",
    "change_function_signature",
    "move_declaration",
    "add_record_field",
];

/// Session over a verified base (the project or a verified scratch repair).
pub(super) fn run_session(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    revision: &str,
    seed: Option<String>,
    ops: Vec<String>,
) -> HarnessResult<()> {
    let mut s = start(cx, journal, r)?;
    let res = loop_steps(
        cx,
        st,
        journal,
        r,
        &mut s,
        revision.to_string(),
        seed,
        ops,
        false,
    );
    r.session = s.to_json(cx);
    res
}

/// Unverified baseline: bounded scratch source repair, then (change mode) steps.
pub(super) fn repair_unverified(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
) -> HarnessResult<()> {
    let mut s = start(cx, journal, r)?;
    let res = repair::repair_loop(cx, st, journal, r, &mut s);
    r.session = s.to_json(cx);
    res
}

#[allow(clippy::too_many_arguments)]
pub(super) fn loop_steps(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    s: &mut State,
    mut revision: String,
    seed: Option<String>,
    ops: Vec<String>,
    after_repair: bool,
) -> HarnessResult<()> {
    let task = cx.cfg.task.clone();
    let host_items = task.acceptance.iter().filter(|a| a.is_object()).count();
    let work = s.work.clone();
    let diag_view = String::new();
    let mut need_context = true;
    let mut kept: Vec<ContextItem> = Vec::new();
    loop {
        s.check_bounds(cx)?;
        cancelled(cx, journal)?;
        if s.steps.len() as u32 >= s.bounds.max_steps {
            return Err(d("SPX-HPD111", format!("session bound exhausted: max_steps {} without satisfying the acceptance criteria", s.bounds.max_steps)));
        }
        if need_context {
            let query: String = task.goal.chars().take(256).collect();
            kept = gather_context(cx, st, r, &work, &seed, query)?.0;
            need_context = false;
        }
        let n = s.attempts.len() as u32 + 1;
        let stepname = format!("gen-{n}");
        let pc = PromptCtx {
            revision: &revision,
            seed: seed.as_deref(),
            diag_view: &diag_view,
            kept: &kept,
            ops: &ops,
            feedback: &s.feedback,
            attempt: n,
            scratch_repair: false,
        };
        s.attempts.push(json!({"attempt": n, "outcome": "started"}));
        let proposal = attempt::propose_step(cx, st, journal, r, &pc, &stepname)?;
        cancelled(cx, journal)?;
        let pdigest = sha256_plain(
            crate::json::canonical(&json!([proposal.kind, proposal.intent, proposal.done]))
                .as_bytes(),
        );
        if let Some(a) = s.attempts.last_mut() {
            a["proposal_digest"] = json!(pdigest);
            a["kind"] = json!(proposal.kind);
        }
        if !s.seen_proposals.insert(pdigest.clone()) {
            if let Some(a) = s.attempts.last_mut() {
                a["outcome"] = json!("repeated");
            }
            journal.append(
                &format!("attempt-{n}"),
                "done",
                s.attempts.last().cloned().unwrap_or(Value::Null),
            )?;
            return Err(d("SPX-HPD112", "no progress: the proposer repeated an identical proposal; stopping with all attempts counted"));
        }
        // Terminal and non-intent proposals.
        if proposal.done {
            let ok = verify_acceptance_all(cx, &work, &task.acceptance, host_items);
            match ok {
                Ok(()) if !s.steps.is_empty() => {
                    return finish(cx, st, journal, r, s, &revision, after_repair)
                }
                Ok(()) => {
                    s.record_failure(
                        n,
                        "done",
                        "SPX-HPD116",
                        "the proposer reported done before any admitted step",
                        journal,
                    )?;
                }
                Err(m) => s.record_failure(n, "acceptance", "SPX-HPD116", &m, journal)?,
            }
            continue;
        }
        if let Err(e) = attempt::require_intent(&proposal, &ops) {
            if e.code == "SPX-HPD092" {
                journal.append(
                    &format!("attempt-{n}"),
                    "done",
                    json!({"outcome": "unsupported"}),
                )?;
                return Err(e);
            }
            s.record_failure(n, "proposal", e.code, &e.message, journal)?;
            continue;
        }
        // The acceptance oracle is not editable by a proposal.
        if let Some(m) = repair::oracle_intent_violation(&s.baseline, &s.oracle, &proposal) {
            s.record_failure(n, "oracle", "SPX-HPD114", &m, journal)?;
            continue;
        }
        cx.cfg.snapshot.verify_current()?;
        let (change, preview) = match attempt::validate_step(cx, &work, &revision, &proposal, r) {
            Ok(x) => x,
            Err(e)
                if matches!(
                    e.code,
                    "SPX-HPD040"
                        | "SPX-HPD041"
                        | "SPX-HPD042"
                        | "SPX-HPD043"
                        | "SPX-HPD044"
                        | "SPX-HPD005"
                ) =>
            {
                s.record_failure(n, "preview", e.code, &e.message, journal)?;
                refine_context(cx, st, r, &work, &seed, &e.message, &mut kept);
                continue;
            }
            Err(e) => return Err(e),
        };
        s.candidates += 1;
        let touched: Vec<&String> = preview
            .source_changes
            .iter()
            .map(|c| &c.path)
            .filter(|p| s.oracle.contains(*p))
            .collect();
        if !touched.is_empty() && !MIGRATING.contains(&proposal.kind.as_str()) {
            let m = format!("`{}` would edit the acceptance oracle ({}); only compiler caller migrations may touch it", proposal.kind, touched.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "));
            s.record_failure(n, "oracle", "SPX-HPD114", &m, journal)?;
            continue;
        }
        match attempt::candidate_checks(cx, st.command, &work, &n.to_string(), &preview, r, false) {
            Ok(c) => r.checks = c,
            Err(e) if e.code == "SPX-HPD050" => {
                let (msg, fb) = super::checks::check_feedback(&r.checks, &e.message);
                s.record_failure(n, "checks", e.code, &msg, journal)?;
                refine_context(cx, st, r, &work, &seed, &msg, &mut kept);
                if let Some(fb) = fb {
                    let mut d = fb["delivered"].clone();
                    d["attempt"] = json!(n);
                    d["check"] = fb["check"].clone();
                    s.delivered.push(d);
                    if let Some(last) = s.feedback.last_mut() {
                        last["check_output"] = fb;
                    }
                }
                continue;
            }
            Err(e) => return Err(e),
        }
        // Admitted step: apply the compiler-produced sources to the scratch tree only.
        let capsule = cx.compiler.candidate_export(&work, &change)?;
        let capsule_path = cx.cfg.cache_dir.join(format!(
            "{}.step{}.capsule.json",
            cx.lineage.id,
            s.steps.len() + 1
        ));
        std::fs::write(&capsule_path, &capsule.bytes)
            .map_err(|e| d("SPX-HPD070", format!("capsule: {e}")))?;
        for c in &preview.source_changes {
            std::fs::write(work.join(&c.path), &c.replacement_source)
                .map_err(|e| d("SPX-HPD070", format!("work write: {e}")))?;
        }
        let first = s.steps.is_empty();
        s.steps.push(json!({"step": s.steps.len() + 1, "intent": proposal.kind, "base_revision": revision,
            "candidate_revision": preview.candidate_revision, "capsule_digest": capsule.candidate_digest,
            "capsule_base_is_project_baseline": first && !after_repair,
            "files": preview.source_changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>()}));
        if let Some(a) = s.attempts.last_mut() {
            a["outcome"] = json!("admitted");
            a["candidate_revision"] = json!(preview.candidate_revision);
        }
        journal.append(
            &format!("attempt-{n}"),
            "done",
            s.attempts.last().cloned().unwrap_or(Value::Null),
        )?;
        revision = preview.candidate_revision.clone();
        s.last_failures.clear();
        s.feedback.clear();
        need_context = true;
        step(
            r,
            "step",
            &format!("{} admitted ({})", s.steps.len(), proposal.kind),
        );
        // Complete when the host-verified acceptance holds; without host-verified
        // criteria the first admitted step completes (the proposer may say `done` earlier).
        if host_items > 0 {
            if verify_acceptance_all(cx, &work, &task.acceptance, host_items).is_ok() {
                return finish(cx, st, journal, r, s, &revision, after_repair);
            }
        } else {
            return finish(cx, st, journal, r, s, &revision, after_repair);
        }
    }
}

/// HN-13: after a failed candidate, exactly one focused provider follow-up
/// named by the failure's own identifiers; when it adds nothing, a continuation
/// handle whose path the failure names is expanded (no provider call). Never
/// fatal: a stage without plans, a spent call bound or provider trouble only
/// leaves the context unchanged and is reported.
fn refine_context(
    cx: &mut Ctx,
    st: &mut Stages,
    r: &mut Report,
    work: &Path,
    seed: &Option<String>,
    failure: &str,
    kept: &mut Vec<ContextItem>,
) {
    let added = follow_up_context(cx, st, r, work, seed, failure, kept).unwrap_or(false);
    if added {
        return;
    }
    let mut handles: Vec<String> = r.context["plan"]["retrieval"]["continuation"]
        .as_array()
        .into_iter()
        .chain(r.context["plan"]["follow_up"]["report"]["continuation"].as_array())
        .flatten()
        .filter_map(|h| h.as_str().map(str::to_string))
        .collect();
    handles.sort();
    handles.dedup();
    for h in handles {
        let names =
            crate::context::plan::parse_handle(&h).is_some_and(|p| failure.contains(&p.path));
        if names {
            match expand_context(cx, st, work, &h, r, failure, kept) {
                Ok(n) if n > 0 => {
                    r.context["plan"]["expanded_handles"] = json!(r.context["plan"]
                        ["expanded_handles"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .chain([json!(h)])
                        .collect::<Vec<_>>());
                    step(r, "context-expand", "added");
                    return;
                }
                Ok(_) => {}
                Err(e) => r.notes.push(format!(
                    "context expansion refused: {} {}",
                    e.code, e.message
                )),
            }
        }
    }
}

fn verify_acceptance_all(
    cx: &Ctx,
    work: &Path,
    items: &[Value],
    host_items: usize,
) -> Result<(), String> {
    match attempt::verify_acceptance(cx, work, items) {
        Ok(n) if n == host_items => Ok(()),
        Ok(_) => Err("acceptance unmet".into()),
        Err(m) => Err(m),
    }
}

/// Final verification of the scratch result by the compiler, then presentation.
pub(super) fn finish(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    s: &mut State,
    revision: &str,
    after_repair: bool,
) -> HarnessResult<()> {
    let check = cx.compiler.check(&s.work)?;
    let test = cx.compiler.test(&s.work)?;
    if !check.ok || check.revision.as_deref() != Some(revision) || !test.passed {
        return Err(d(
            "SPX-HPD050",
            "final scratch result failed independent verification",
        ));
    }
    let files = changed_files(&s.baseline, &s.work);
    let result_dir = cx.cfg.cache_dir.join(format!("result-{}", cx.lineage.id));
    attempt::copy_project(&s.work, &result_dir)?;
    // Binds the result to its exact baseline so `apply` can detect drift later.
    let meta = json!({"schema": "semaprax.harness-session-result.v1", "baseline_revision": cx.cfg.snapshot.revision,
        "baseline_files": cx.cfg.snapshot.files, "result_revision": revision});
    std::fs::write(result_dir.join(SESSION_FILE), crate::json::canonical(&meta))
        .map_err(|e| d("SPX-HPD070", format!("session result: {e}")))?;
    s.result = json!({"revision": revision, "dir": result_dir.to_string_lossy(), "changed_files": files,
        "kind": if after_repair { "scratch-repair-then-change" } else { "semantic-steps" },
        "apply": "separate authority: workflow::apply_result (drift-checked); nothing was written to the project"});
    r.compiler_revision = Some(revision.to_string());
    step(
        r,
        "session",
        &format!(
            "{} admitted step(s), {} attempt(s)",
            s.steps.len(),
            s.attempts.len()
        ),
    );
    // A single step against the project baseline has an ordinary capsule: publication
    // stays behind the existing host policy. Multi-step results are not one transaction.
    let single_publishable = s.steps.len() == 1 && !after_repair;
    if single_publishable {
        let kind = s.steps[0]["intent"].as_str().unwrap_or("").to_string();
        let path = cx
            .cfg
            .cache_dir
            .join(format!("{}.step1.capsule.json", cx.lineage.id));
        let bytes = std::fs::read(&path).map_err(|e| d("SPX-HPD070", format!("capsule: {e}")))?;
        let capsule = super::compiler::Capsule {
            candidate_digest: s.steps[0]["capsule_digest"].as_str().unwrap_or("").into(),
            base_revision: s.steps[0]["base_revision"].as_str().unwrap_or("").into(),
            candidate_project_revision: revision.into(),
            bytes,
        };
        return present_and_publish(cx, journal, r, &capsule, &kind);
    }
    r.status = "candidate-ready";
    r.notes.push("session result verified in scratch; it spans several steps or a scratch repair, so no single capsule is publishable; apply it through the drift-checked `apply_result` authority".into());
    let _ = st;
    Ok(())
}

pub(super) fn changed_files(baseline: &Baseline, work: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    for (path, bytes) in baseline {
        if let Ok(now) = std::fs::read(work.join(path)) {
            if &now != bytes {
                out.push(json!({"path": path, "digest": sha256_plain(&now)}));
            }
        }
    }
    out
}

/// Final source application: a separate authority from publication. Refuses
/// (`SPX-HPD115`) when the project drifted from the captured baseline, or when
/// the scratch result no longer verifies at `expected_revision`. All files are
/// staged before any is renamed into place.
pub fn apply_result(
    snapshot: &Snapshot,
    result_dir: &Path,
    expected_revision: &str,
    compiler: &dyn super::compiler::CompilerService,
) -> HarnessResult<Vec<String>> {
    snapshot.verify_current().map_err(|e| {
        d(
            "SPX-HPD115",
            format!("source drift rejects final application: {}", e.message),
        )
    })?;
    let check = compiler.check(result_dir)?;
    if !check.ok || check.revision.as_deref() != Some(expected_revision) {
        return Err(d(
            "SPX-HPD115",
            "the scratch result does not verify at the expected revision",
        ));
    }
    if !compiler.test(result_dir)?.passed {
        return Err(d("SPX-HPD115", "the scratch result's tests do not pass"));
    }
    let mut staged = Vec::new();
    for (path, digest) in &snapshot.files {
        let new = std::fs::read(result_dir.join(path))
            .map_err(|e| d("SPX-HPD115", format!("result {path}: {e}")))?;
        if &sha256_plain(&new) != digest {
            let target = snapshot.root.join(path);
            let tmp = target.with_extension("spx.harness-tmp");
            std::fs::write(&tmp, &new)
                .map_err(|e| d("SPX-HPD070", format!("stage {path}: {e}")))?;
            staged.push((path.clone(), tmp, target));
        }
    }
    let mut applied = Vec::new();
    for (path, tmp, target) in staged {
        std::fs::rename(&tmp, &target)
            .map_err(|e| d("SPX-HPD070", format!("apply {path}: {e}")))?;
        applied.push(path);
    }
    Ok(applied)
}
