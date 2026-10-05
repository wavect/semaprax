//! MR-08: optional plan / implement / advisory-review role policies inside the
//! existing workflow (see `docs/HARNESS-WORKFLOW-V1.md`).
//!
//! Without a `[routing.phase.<role>]` table nothing here runs and the
//! single-proposer loop is unchanged. Planning and review run only when
//! `enabled` or when the task family matches the deterministic
//! `risk_families` rule. Every role routes through the one route/fit/reserve
//! path (`attempt::route_and_fit`) over its approved candidate subset, so all
//! calls share the task's one spend ledger, the endpoint policy, the privacy
//! screen and the project pin (which keeps precedence over a role pin). A plan
//! is a bounded reference; a review is bounded advisory findings against a
//! frozen candidate revision. Neither can rewrite acceptance, authorize
//! publication or claim tests passed: such members reject the artifact, and
//! `claims` are ignored. Artifacts are journaled and reused on resume.

use super::attempt::{generate, route_and_fit, PromptCtx};
use super::compiler::CandidatePreview;
use super::journal::Journal;
use super::pipeline::{Ctx, Stages};
use super::report::Report;
use super::stages::{ContextItem, TaskMode};
use crate::bridge::negotiate::Owner;
use crate::decision::{ModelPlan, PreviousFailure, RoutingMode};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{canonical, sha256_plain};
use crate::profile::config::PhaseConfig;
use serde_json::{json, Map, Value};
use std::sync::atomic::Ordering;

pub const PHASE_PROMPT_SCHEMA: &str = "semaprax.harness-phase-prompt.v1";
pub const PLAN_SCHEMA: &str = "semaprax.harness-plan.v1";
pub const REVIEW_SCHEMA: &str = "semaprax.harness-review.v1";
pub const HANDOFF_SCHEMA: &str = "semaprax.harness-handoff.v1";
const MAX_PLAN_STEPS: usize = 16;
const MAX_FINDINGS: usize = 16;
const MAX_REVIEW_SOURCE: usize = 16 * 1024;
/// Members that would let a role rewrite acceptance, claim results or authorize.
const AUTHORITY: [&str; 12] = [
    "acceptance",
    "tests_passed",
    "passed",
    "confidence",
    "done",
    "approve",
    "approved",
    "publish",
    "apply",
    "requirements",
    "authority",
    "grant",
];

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Plan,
    Implement,
    Review,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Plan => "plan",
            Role::Implement => "implement",
            Role::Review => "review",
        }
    }
}

/// Role-specific request material carried by a `PromptCtx`.
pub struct PhaseView {
    pub role: Role,
    /// The whole model-visible document for plan/review; extra members
    /// (`plan`, `handoff`) for an implementation request.
    pub body: Value,
}

/// Any role policy is configured.
pub(super) fn active(cx: &Ctx) -> bool {
    !cx.cfg.routing.phases.is_empty()
}

fn policy<'a>(cx: &'a Ctx, role: Role) -> Option<&'a PhaseConfig> {
    cx.cfg.routing.phases.get(role.as_str())
}

/// Why an optional role runs for this task, or `None` when it does not.
pub(super) fn runs(cx: &Ctx, role: Role) -> Option<String> {
    let p = policy(cx, role)?;
    if role == Role::Implement {
        return Some("implementation always runs".into());
    }
    if p.enabled {
        Some("enabled".into())
    } else if p.risk_families.contains(&cx.cfg.task.family) {
        Some(format!("risk rule: task family `{}`", cx.cfg.task.family))
    } else {
        None
    }
}

fn project_pinned(cx: &Ctx) -> bool {
    let w = &cx.cfg.routing;
    w.cfg.project_pin.is_some() || matches!(w.cfg.mode, RoutingMode::Pin(_))
}

/// Narrow the approved catalog to the role's subset and pin. A project pin
/// keeps precedence: the role policy is then not applied. The subset can only
/// remove models; one that names no approved model is refused (`SPX-HPD118`).
pub(super) fn narrow(
    cx: &Ctx,
    role: Role,
    mut pool: Vec<ModelPlan>,
) -> HarnessResult<Vec<ModelPlan>> {
    let Some(p) = policy(cx, role) else {
        return Ok(pool);
    };
    if project_pinned(cx) {
        return Ok(pool);
    }
    if let Some(models) = &p.models {
        pool.retain(|m| models.contains(&m.id));
    }
    if let Some(pin) = &p.pin {
        pool.retain(|m| &m.id == pin);
    }
    if pool.is_empty() {
        return Err(d(
            "SPX-HPD118",
            format!(
                "phase `{}`: its allowlist/pin names no approved catalog model",
                role.as_str()
            ),
        ));
    }
    Ok(pool)
}

/// May this role consult the learned router for `pool`? Without phase
/// policies the existing behavior holds. With them a router is consulted only
/// when the role's `decision = "router"` and more than one candidate remains
/// with no pin (one candidate or a deterministic rule suffices otherwise).
pub(super) fn router_allowed(cx: &Ctx, role: Role, pool: &[ModelPlan]) -> bool {
    if !active(cx) {
        return true;
    }
    match policy(cx, role) {
        None => role == Role::Implement && pool.len() > 1,
        Some(p) => p.decision == "router" && p.pin.is_none() && pool.len() > 1,
    }
}

fn acceptance_digest(cx: &Ctx) -> String {
    sha256_plain(canonical(&json!(cx.cfg.task.acceptance)).as_bytes())
}

fn log(r: &mut Report, entry: Value) {
    if r.phases.is_null() {
        r.phases = json!({
            "entries": [],
            "parent_model": "not changed: phase routing applies to Semaprax-owned worker requests only",
        });
    }
    if let Some(a) = r.phases["entries"].as_array_mut() {
        a.push(entry);
    }
}

fn last_implement_model(r: &Report) -> Option<String> {
    r.phases["entries"]
        .as_array()?
        .iter()
        .rev()
        .find(|e| e["phase"] == "implement" || e["phase"] == "repair")
        .and_then(|e| e["model"].as_str().map(str::to_string))
}

/// Reasoning escalation vs transport retry for the newest host-verified failure.
pub(super) fn retry_kind(failure: PreviousFailure) -> &'static str {
    match failure {
        PreviousFailure::None => "none",
        PreviousFailure::ParseSchema
        | PreviousFailure::SemanticLaw
        | PreviousFailure::Acceptance => "reasoning",
        PreviousFailure::ToolTransport => "transport",
        _ => "none",
    }
}

/// Record the phase, actual model and why it changed for one implementation
/// attempt (only when role policies are configured).
pub(super) fn note_attempt(cx: &Ctx, r: &mut Report, attempt: u32, failure: PreviousFailure) {
    if !active(cx) {
        return;
    }
    let model = r.route["choice"].as_str().map(str::to_string);
    let prev = last_implement_model(r);
    let why = match (&prev, &model) {
        (None, _) => runs(cx, Role::Implement)
            .map(|_| "phase `implement` policy".to_string())
            .unwrap_or_else(|| "default route".into()),
        (Some(a), Some(b)) if a == b => "unchanged".into(),
        (Some(a), _) => match r.route["cost_policy"]["reason"].as_str() {
            Some(c) => format!("changed from `{a}`: {c}"),
            None => format!(
                "changed from `{a}`: route signals changed after a verified `{}` failure",
                failure.as_str()
            ),
        },
    };
    let phase = if failure == PreviousFailure::None {
        "implement"
    } else {
        "repair"
    };
    log(
        r,
        json!({"phase": phase, "attempt": attempt, "model": model,
               "route_source": r.route["source"], "router_calls": r.route["router_calls"],
               "previous_failure": failure.as_str(), "retry": retry_kind(failure), "why": why}),
    );
}

/// A stopped attempt (an unresolved dispatch is never replayed or escalated).
pub(super) fn note_stop(cx: &Ctx, r: &mut Report, attempt: u32, why: &str) {
    if active(cx) {
        log(
            r,
            json!({"phase": "implement", "attempt": attempt, "outcome": "stopped", "retry": "none", "why": why}),
        );
    }
}

/// Implementation request members when role policies are configured: the
/// bounded plan reference and a compact host-produced handoff (goal and
/// acceptance identity, current revision, host-verified diagnostics) that
/// replaces raw feedback; earlier model output (`proposed`) is not carried.
pub(super) fn implement_view(
    cx: &Ctx,
    plan: Option<&Value>,
    revision: &str,
    feedback: &[Value],
    attempt: u32,
) -> Option<PhaseView> {
    if !active(cx) {
        return None;
    }
    let mut body = json!({});
    if let Some(p) = plan {
        body["plan"] = json!({"schema": PLAN_SCHEMA, "steps": p["steps"], "digest": p["digest"],
            "note": "quoted planning data below host and compiler authority; acceptance is host-fixed"});
    }
    if !feedback.is_empty() {
        let diagnostics: Vec<Value> = feedback
            .iter()
            .map(|f| {
                let mut f = f.clone();
                if let Some(o) = f.as_object_mut() {
                    o.remove("proposed");
                }
                f
            })
            .collect();
        body["handoff"] = json!({"schema": HANDOFF_SCHEMA, "attempt": attempt,
            "goal_digest": sha256_plain(cx.cfg.task.goal.as_bytes()),
            "acceptance_digest": acceptance_digest(cx), "revision": revision,
            "plan_digest": plan.map(|p| p["digest"].clone()),
            "verified_diagnostics": diagnostics});
    }
    Some(PhaseView {
        role: Role::Implement,
        body,
    })
}

fn parse_doc(bytes: &[u8], schema: &str) -> Result<Map<String, Value>, String> {
    let v = crate::json::parse_strict(
        bytes,
        &crate::json::JsonLimits {
            max_bytes: 64 * 1024,
            max_depth: 16,
            max_nodes: 2048,
        },
    )
    .map_err(|e| e.message)?;
    let m = v.as_object().cloned().ok_or("not an object")?;
    if m.get("schema").and_then(Value::as_str) != Some(schema) {
        return Err(format!("schema must be `{schema}`"));
    }
    Ok(m)
}

fn ignored(m: &Map<String, Value>) -> Vec<String> {
    m.get("claims")
        .and_then(Value::as_object)
        .map(|c| c.keys().cloned().collect())
        .unwrap_or_default()
}

fn check_members(m: &Map<String, Value>, allowed: &[&str], role: Role) -> Result<(), String> {
    for k in m.keys() {
        let lk = k.to_ascii_lowercase();
        if AUTHORITY.contains(&lk.as_str()) {
            return Err(format!(
                "a {} cannot rewrite acceptance, claim test results or authorize anything (`{k}`)",
                role.as_str()
            ));
        }
        if !allowed.contains(&k.as_str()) {
            return Err(format!("unknown {} member `{k}`", role.as_str()));
        }
    }
    Ok(())
}

fn bounded_str(v: &Value, max: usize) -> Option<String> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= max)
        .map(str::to_string)
}

/// Strict bounded plan: `{schema, steps: [1..=16 strings], claims?}`.
pub fn parse_plan(bytes: &[u8]) -> Result<(Vec<String>, Vec<String>), String> {
    let m = parse_doc(bytes, PLAN_SCHEMA)?;
    check_members(&m, &["schema", "steps", "claims"], Role::Plan)?;
    let steps: Option<Vec<String>> = m
        .get("steps")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty() && a.len() <= MAX_PLAN_STEPS)
        .map(|a| a.iter().map(|s| bounded_str(s, 256)).collect())
        .and_then(|v: Option<Vec<String>>| v);
    let steps = steps.ok_or("`steps` must list 1..=16 strings of at most 256 bytes")?;
    Ok((steps, ignored(&m)))
}

/// Strict bounded advisory review: `{schema, findings: [..16], claims?}`.
pub fn parse_review(bytes: &[u8]) -> Result<(Vec<Value>, Vec<String>), String> {
    let m = parse_doc(bytes, REVIEW_SCHEMA)?;
    check_members(&m, &["schema", "findings", "claims"], Role::Review)?;
    let arr = m
        .get("findings")
        .and_then(Value::as_array)
        .filter(|a| a.len() <= MAX_FINDINGS)
        .ok_or("`findings` must list at most 16 findings")?;
    let mut out = Vec::new();
    for f in arr {
        let o = f.as_object().ok_or("a finding must be an object")?;
        if o.keys()
            .any(|k| !["severity", "message", "path"].contains(&k.as_str()))
        {
            return Err("a finding carries only severity, message and path".into());
        }
        let sev = o
            .get("severity")
            .and_then(Value::as_str)
            .filter(|s| ["info", "warning", "concern"].contains(s))
            .ok_or("finding `severity` must be info, warning or concern")?;
        let msg = o
            .get("message")
            .and_then(|v| bounded_str(v, 512))
            .ok_or("finding `message` must be 1..=512 bytes")?;
        let path = match o.get("path") {
            None => None,
            Some(p) => Some(bounded_str(p, 256).ok_or("finding `path` must be 1..=256 bytes")?),
        };
        out.push(json!({"severity": sev, "message": msg, "path": path}));
    }
    Ok((out, ignored(&m)))
}

fn artifact_path(cx: &Ctx, step: &str) -> std::path::PathBuf {
    cx.cfg
        .cache_dir
        .join(format!("{}.{step}.artifact.json", cx.lineage.id))
}

/// A recorded artifact of `step` bound to `bind` (the frozen revision), or
/// `None`. A recorded artifact whose bytes no longer match is refused.
fn recorded(cx: &Ctx, journal: &Journal, step: &str, bind: &str) -> HarnessResult<Option<Value>> {
    let Some(rec) = journal.state(&format!("{step}-artifact")) else {
        return Ok(None);
    };
    if rec.state != "done" || rec.detail["bind"] != bind {
        return Ok(None);
    }
    let bytes = std::fs::read(artifact_path(cx, step)).ok();
    match bytes {
        Some(b) if rec.detail["digest"] == sha256_plain(&b) => serde_json::from_slice(&b)
            .map(Some)
            .map_err(|e| d("SPX-HPD072", format!("phase artifact `{step}`: {e}"))),
        _ => Err(d(
            "SPX-HPD072",
            format!("phase artifact `{step}` is missing or does not match its journal digest; it is not regenerated billably"),
        )),
    }
}

fn persist(
    cx: &Ctx,
    journal: &mut Journal,
    step: &str,
    bind: &str,
    art: &Value,
) -> HarnessResult<String> {
    let bytes = canonical(art);
    std::fs::write(artifact_path(cx, step), &bytes)
        .map_err(|e| d("SPX-HPD070", format!("phase artifact: {e}")))?;
    let digest = sha256_plain(bytes.as_bytes());
    journal.append(
        &format!("{step}-artifact"),
        "done",
        json!({"digest": digest, "bind": bind}),
    )?;
    Ok(digest)
}

fn cancelled(cx: &Ctx) -> HarnessResult<()> {
    if cx
        .cfg
        .cancel
        .as_ref()
        .is_some_and(|c| c.load(Ordering::SeqCst))
    {
        return Err(d(
            "SPX-HPD113",
            "session cancelled by the caller; recorded, nothing in flight is replayed",
        ));
    }
    Ok(())
}

/// One routed, reserved, journaled role generation. `Ok(None)` is a known
/// skip (budget, availability, refusal); an unresolved dispatch propagates.
fn generate_role(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    view: &PhaseView,
    revision: &str,
    ops: &[String],
    step: &str,
) -> HarnessResult<Result<(Vec<u8>, Value), String>> {
    let pc = PromptCtx {
        revision,
        seed: None,
        diag_view: "",
        kept: &[],
        ops,
        feedback: &[],
        attempt: 1,
        scratch_repair: false,
        phase: Some(view),
    };
    let saved = r.context.get("request_budget").cloned();
    let routed = route_and_fit(cx, st, journal, r, &pc, step);
    let out = match routed {
        Err(e) if e.code == "SPX-HPD072" || e.code == "SPX-HPD113" => return Err(e),
        Err(e) => Ok(Err(format!("{} {}", e.code, e.message))),
        Ok((fit, route, attempt)) => {
            let shape = super::generation::ResponseShape::StructuredIntent;
            let controls = cx.cfg.budget.generation.controls(shape, fit.output_reserve);
            let route = json!({"model": fit.model, "source": route["source"], "router_calls": route["router_calls"],
                               "reuse": route["reuse"]});
            match generate(
                cx,
                st,
                journal,
                fit.prompt.clone(),
                fit.model.clone(),
                r,
                step,
                &fit.count,
                &controls,
                &attempt,
            ) {
                Ok(b) => Ok(Ok((b, route))),
                Err(e) if e.code == "SPX-HPD072" => return Err(e),
                Err(e) => Ok(Err(format!("{} {}", e.code, e.message))),
            }
        }
    };
    if let Some(b) = saved {
        r.context["request_budget"] = b;
    }
    out
}

/// Optional planning before the first implementation attempt. Returns the
/// bounded plan reference the implementer sees, or `None`.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_plan(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    revision: &str,
    kept: &[ContextItem],
    ops: &[String],
) -> HarnessResult<Option<Value>> {
    let Some(why) = runs(cx, Role::Plan) else {
        return Ok(None);
    };
    if cx.cfg.task.mode == TaskMode::Plan {
        return Ok(None);
    }
    cancelled(cx)?;
    let step = "phase-plan";
    if let Some(plan) = recorded(cx, journal, step, revision)? {
        log(
            r,
            json!({"phase": "plan", "source": "journal", "why": why, "model_calls": 0,
                      "artifact_digest": plan["digest"], "steps": plan["steps"].as_array().map_or(0, Vec::len)}),
        );
        return Ok(Some(plan));
    }
    let context: Vec<Value> = kept
        .iter()
        .map(|i| json!({"label": i.label, "provenance": i.provenance, "text": i.text}))
        .collect();
    let task = &cx.cfg.task;
    let view = PhaseView {
        role: Role::Plan,
        body: json!({"schema": PHASE_PROMPT_SCHEMA, "phase": "plan", "revision": revision,
            "goal": task.goal, "acceptance": task.acceptance, "intents": ops, "context": context,
            "respond": {"schema": PLAN_SCHEMA, "steps": "1..=16 short strings",
                        "note": "the host fixes acceptance, runs the compiler checks and owns publication"}}),
    };
    let (bytes, route) = match generate_role(cx, st, journal, r, &view, revision, ops, step)? {
        Ok(x) => x,
        Err(why_not) => {
            log(
                r,
                json!({"phase": "plan", "source": "skipped", "why": why, "reason": why_not}),
            );
            return Ok(None);
        }
    };
    match parse_plan(&bytes) {
        Ok((steps, ignored)) => {
            let digest = sha256_plain(canonical(&json!(steps)).as_bytes());
            let plan = json!({"schema": PLAN_SCHEMA, "steps": steps, "digest": digest,
                              "revision": revision, "acceptance_digest": acceptance_digest(cx)});
            persist(cx, journal, step, revision, &plan)?;
            log(
                r,
                json!({"phase": "plan", "source": "generated", "why": why, "model": route["model"],
                          "route_source": route["source"], "router_calls": route["router_calls"],
                          "reuse": route["reuse"], "artifact_digest": digest,
                          "steps": plan["steps"].as_array().map_or(0, Vec::len), "ignored_claims": ignored}),
            );
            Ok(Some(plan))
        }
        Err(reason) => {
            log(
                r,
                json!({"phase": "plan", "source": "rejected", "why": why, "model": route["model"],
                          "reason": reason, "effect": "implementation proceeds without a plan"}),
            );
            Ok(None)
        }
    }
}

/// Optional advisory review of a compiler-verified candidate (frozen at its
/// candidate revision). Findings never approve, reject or publish anything.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_review(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    base_revision: &str,
    preview: &CandidatePreview,
    ops: &[String],
    tag: &str,
) -> HarnessResult<()> {
    let Some(why) = runs(cx, Role::Review) else {
        return Ok(());
    };
    cancelled(cx)?;
    let step = format!("phase-review-{tag}");
    let frozen = preview.candidate_revision.clone();
    let entry = |source: &str, extra: Value| {
        let mut e = json!({"phase": "review", "source": source, "why": why, "candidate_revision": frozen,
                           "advisory": true, "effect": "none: findings never approve, reject, apply or publish"});
        for (k, v) in extra.as_object().into_iter().flatten() {
            e[k] = v.clone();
        }
        e
    };
    if let Some(rev) = recorded(cx, journal, &step, &frozen)? {
        log(
            r,
            entry(
                "journal",
                json!({"model_calls": 0, "artifact_digest": rev["digest"], "findings": rev["counts"]}),
            ),
        );
        return Ok(());
    }
    let mut budget = MAX_REVIEW_SOURCE;
    let mut omitted = Vec::new();
    let changes: Vec<Value> = preview
        .source_changes
        .iter()
        .filter_map(|c| {
            if c.replacement_source.len() <= budget {
                budget -= c.replacement_source.len();
                Some(json!({"path": c.path, "source": c.replacement_source}))
            } else {
                omitted.push(c.path.clone());
                None
            }
        })
        .collect();
    let task = &cx.cfg.task;
    let view = PhaseView {
        role: Role::Review,
        body: json!({"schema": PHASE_PROMPT_SCHEMA, "phase": "review", "goal": task.goal,
            "acceptance": task.acceptance, "base_revision": base_revision, "candidate_revision": frozen,
            "changes": changes, "omitted": omitted,
            "host_checks": "the host compiler checked this candidate and its tests passed",
            "respond": {"schema": REVIEW_SCHEMA, "findings": "at most 16 {severity: info|warning|concern, message, path?}",
                        "note": "advisory only: findings cannot approve, reject or publish"}}),
    };
    let (bytes, route) = match generate_role(cx, st, journal, r, &view, base_revision, ops, &step)?
    {
        Ok(x) => x,
        Err(why_not) => {
            log(r, entry("skipped", json!({"reason": why_not})));
            return Ok(());
        }
    };
    match parse_review(&bytes) {
        Ok((findings, ignored)) => {
            let mut counts = json!({"info": 0, "warning": 0, "concern": 0});
            for f in &findings {
                let k = f["severity"].as_str().unwrap_or("info");
                counts[k] = json!(counts[k].as_u64().unwrap_or(0) + 1);
            }
            let digest = sha256_plain(canonical(&json!(findings)).as_bytes());
            let art = json!({"schema": REVIEW_SCHEMA, "candidate_revision": frozen, "findings": findings,
                             "counts": counts, "digest": digest});
            persist(cx, journal, &step, &frozen, &art)?;
            log(
                r,
                entry(
                    "generated",
                    json!({"model": route["model"], "route_source": route["source"],
                "router_calls": route["router_calls"], "reuse": route["reuse"], "artifact_digest": digest,
                "findings": counts, "ignored_claims": ignored}),
                ),
            );
        }
        Err(reason) => log(
            r,
            entry(
                "rejected",
                json!({"model": route["model"], "reason": reason}),
            ),
        ),
    }
    Ok(())
}

/// How Semaprax may affect model choice under a negotiated bridge host
/// (MR-08 item 6). A non-delegating host stays host-controlled: the report is
/// advisory and the parent agent's own model is never claimed rerouted.
pub fn parent_model_routing(owner: Owner) -> Value {
    match owner {
        Owner::Semaprax => json!({"owner": "semaprax", "mode": "delegated",
            "changes_parent_model": false,
            "scope": "the host delegated model choice; Semaprax answers through the profile's decision binding and routes its own worker requests"}),
        _ => json!({"owner": owner.as_str(), "mode": "host-controlled", "advisory_only": true,
            "changes_parent_model": false,
            "scope": "the host did not delegate model choice; phase routing applies to Semaprax-owned worker requests only and reports advice"}),
    }
}
