//! One proposal attempt: budget-checked routing, generation, compiler preview
//! and candidate checks. Shared by the single-shot pipeline and the HN-02
//! session so both spend one per-task ledger (HN-01, HN-11, HN-02).

use super::budget::{request_text, Fit, LedgerEntry, RequestCount};
use super::compiler::CandidatePreview;
use super::journal::Journal;
use super::pipeline::{change_bytes, Ctx, Stages};
use super::policy::check_protected_facts;
use super::report::Report;
use super::stages::*;
use crate::decision::{
    gate_attests_key, governed_decide, recheck_dispatch, Budget, Confidentiality,
    ConfiguredProvider, Destination, EvidenceKey, Governor, LatencyClass, ModelPlan, ProviderMode,
    RouteContext, RouteInputs, RoutePolicy, RouteRequest, RoutingConfig, RoutingMode, TaskFamily,
    TaskFeatures,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use crate::observe::{Availability, Role, Stage};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

pub(super) const ROUTER_OUTPUT_RESERVE: u64 = 256;

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Everything the model-visible prompt is built from for one attempt.
pub(super) struct PromptCtx<'a> {
    pub revision: &'a str,
    pub seed: Option<&'a str>,
    pub diag_view: &'a str,
    pub kept: &'a [ContextItem],
    pub ops: &'a [String],
    /// Exact compiler/check output of earlier attempts (HN-02).
    pub feedback: &'a [Value],
    pub attempt: u32,
    /// Extra framing for an unverified-baseline scratch repair.
    pub scratch_repair: bool,
}

fn item_label(i: usize, it: &ContextItem) -> String {
    format!("ctx#{i}:{}", it.label)
}

/// Optional material in drop order: external context items last-first, then the
/// whole skill section. Native compiler facts and instructions are protected.
fn optional_labels(p: &PromptCtx, has_skills: bool) -> Vec<String> {
    let mut v: Vec<String> = p
        .kept
        .iter()
        .enumerate()
        .filter(|(_, it)| it.provenance.starts_with("external:"))
        .map(|(i, it)| item_label(i, it))
        .rev()
        .collect();
    if has_skills {
        v.push("skills".into());
    }
    v
}

fn build_prompt(cx: &Ctx, p: &PromptCtx, dropped: &BTreeSet<String>) -> Value {
    let task = &cx.cfg.task;
    let context: Vec<Value> = p
        .kept
        .iter()
        .enumerate()
        .filter(|(i, it)| !dropped.contains(&item_label(*i, it)))
        .map(|(_, i)| json!({"label": i.label, "provenance": i.provenance, "text": i.text}))
        .collect();
    let mut prompt = json!({
        "schema": "semaprax.harness-prompt.v1", "revision": p.revision, "goal": task.goal,
        "seed": p.seed, "diagnostics": p.diag_view, "intents": p.ops, "context": context,
    });
    if task.schema_version == 2 {
        prompt["mode"] = json!(task.mode.as_str());
        prompt["task_family"] = json!(task.family);
        prompt["acceptance"] = json!(task.acceptance);
        prompt["attempt"] = json!(p.attempt);
    }
    if !p.feedback.is_empty() {
        prompt["feedback"] = json!(p.feedback);
    }
    if p.scratch_repair {
        prompt["scratch_repair"] = json!(true);
    }
    if let Some(sp) = &cx.cfg.skill_prompt {
        if !dropped.contains("skills") {
            // Quoted data below host and compiler authority (framed by the skill service).
            prompt["skills"] = json!(sp.text);
        }
    }
    prompt
}

/// The model catalog of the task (explicit models, the machine-local binding or
/// the local default), screened by the `[model]` policy.
fn catalog(cx: &Ctx, task: &Task) -> HarnessResult<Vec<ModelPlan>> {
    let catalog = match (&task.models, &cx.cfg.model_plans) {
        (Some(m), _) => RouteRequest::catalog_from_json(m)?,
        (None, Some(p)) => p.clone(),
        (None, None) => vec![ModelPlan {
            id: "workflow-default".into(),
            destination: Destination::Local,
            structured_output: true,
            tools: false,
            max_context: 1_000_000,
            est_cost_micros: 0,
            est_latency_ms: 1000,
            strength_rank: 1,
        }],
    };
    for m in &catalog {
        crate::endpoint::check_policy(
            cx.cfg.endpoint_policy,
            &crate::endpoint::AttemptOwnership::direct(),
            &m.destination,
        )?;
    }
    Ok(catalog)
}

struct Routed {
    inputs: RouteInputs,
    json: Value,
    model: String,
    router_calls: u32,
    request_text: String,
    provider: String,
}

fn route_models(
    cx: &Ctx,
    task: &Task,
    catalog: Vec<ModelPlan>,
    estimated_tokens: u64,
    decision: Option<&mut super::pipeline::DecisionStage>,
    rb: &super::budget::RequestBudget,
    router_headroom_tokens: Option<u64>,
) -> HarnessResult<Routed> {
    let family = TaskFamily::parse(&task.family).ok_or_else(|| {
        d(
            "SPX-HPD081",
            format!("unknown task_family `{}`", task.family),
        )
    })?;
    let features = TaskFeatures {
        task_family: family,
        estimated_context_tokens: estimated_tokens,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let budget = Budget {
        max_cost_micros: 1_000_000,
        max_latency_ms: 60_000,
        max_router_calls: u32::from(decision.is_some()),
    };
    let text = crate::json::canonical(&json!({"features": features.to_json(),
        "catalog": catalog.iter().map(ModelPlan::to_json).collect::<Vec<_>>(), "budget": budget.to_json()}));
    let request = RouteRequest::new(features, catalog, budget)?;
    let mut policy = RoutePolicy::default();
    if cx.cfg.routing.approve_remote {
        // The project explicitly allows remote routing: the origins of the
        // (already endpoint-policy-checked) catalog are approved for project data.
        policy.remote_max_confidentiality = Some(Confidentiality::Project);
        policy.allowed_origins = request
            .catalog
            .iter()
            .filter_map(|m| match &m.destination {
                Destination::Remote { origin } => Some(origin.clone()),
                Destination::Local => None,
            })
            .collect();
    }
    let inputs = RouteInputs { request, policy };
    let rctx = RouteContext {
        project: cx.lineage.project.clone(),
        lock_digest: cx.lineage.lock_digest.clone(),
        invocation_id: format!("route-{}", cx.lineage.id),
        lineage_id: cx.lineage.id.clone(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    };
    let router_request_tokens = decision.as_ref().map_or(0, |d| {
        rb.count(&d.profile.provider_id, &text).admission_tokens() + ROUTER_OUTPUT_RESERVE
    });
    let live = inputs.clone();
    // The workflow carries only the stage's gate: a learned provider is
    // consulted in `Auto` mode only when that gate attests the live key
    // (provider, weights, approved catalog); otherwise rules decide (HN-16).
    let wiring = &cx.cfg.routing;
    let live_key = decision
        .as_ref()
        .map(|d| EvidenceKey::live(&d.profile, &inputs.request.catalog_digest()));
    let mode = if wiring.explicit_mode || wiring.cfg.project_pin.is_some() {
        // `[routing]` decides: the project's mode and pin, never the provider's own.
        wiring.cfg.mode.clone()
    } else {
        match (decision.as_ref(), &live_key) {
            (None, _) | (_, None) => RoutingMode::Rules,
            (Some(d), _) if d.mode == ProviderMode::Explicit => RoutingMode::Experimental,
            (Some(d), Some(key)) => {
                if gate_attests_key(&d.gate, key) {
                    RoutingMode::Experimental
                } else {
                    RoutingMode::Rules
                }
            }
        }
    };
    let lock = if mode == RoutingMode::QualifiedAuto {
        live_key.as_ref().and_then(|k| wiring.lock_for(k))
    } else {
        None
    };
    let mut configured = decision.map(|d| ConfiguredProvider {
        profile: d.profile.clone(),
        invoker: &mut *d.invoker,
        mode: d.mode,
        gate: d.gate.clone(),
    });
    let cfg = RoutingConfig {
        mode,
        ..wiring.cfg.clone()
    };
    let g = Governor {
        cfg: &cfg,
        registry: wiring.registry.as_ref(),
        spec: &wiring.spec,
        lock: lock.as_ref(),
        router_headroom_tokens,
        router_request_tokens,
    };
    let gr = governed_decide(
        &g,
        &inputs,
        &rctx,
        configured.as_mut(),
        &move || live.clone(),
        None,
    )?;
    let dec = gr.decision;
    Ok(Routed {
        inputs,
        json: json!({"choice": dec.choice, "provider": dec.provider_id, "router_calls": dec.router_calls,
               "status": dec.provider_status, "source": format!("{:?}", dec.source),
               "mode": gr.mode, "rules_reason": gr.rules_reason, "explanation": gr.explanation,
               "policy": {"allow_remote": cfg.user_allow_remote, "project_pin": cfg.project_pin}}),
        model: dec.choice,
        router_calls: dec.router_calls,
        request_text: text,
        provider: dec.provider_id,
    })
}

/// Select a model and fit the exact serialized request to it before any
/// provider call: protected content is never truncated; optional material is
/// dropped whole; a model that cannot fit is excluded and routing repeats.
pub(super) fn route_and_fit(
    cx: &mut Ctx,
    st: &mut Stages,
    r: &mut Report,
    p: &PromptCtx,
    label: &str,
) -> HarnessResult<(Fit, Value)> {
    let cfg = cx.cfg;
    let task = &cfg.task;
    let all = catalog(cx, task)?;
    let budget = cfg.budget.for_task(task);
    let optional = optional_labels(p, cfg.skill_prompt.is_some());
    let mut excluded: Vec<Value> = Vec::new();
    let mut pool = all.clone();
    loop {
        if pool.is_empty() {
            return Err(d(
                "SPX-HPD100",
                format!(
                    "no model can take this request: the protected content (compiler facts, diagnostics, instructions, output reserve) exceeds every candidate's context or the router found none admissible; tried {}",
                    json!(excluded)
                ),
            ));
        }
        let est = {
            let build = |dr: &BTreeSet<String>| build_prompt(cx, p, dr);
            budget.floor_estimate(&pool, &optional, &build)
        };
        pool.retain(|m| m.max_context >= est);
        if pool.is_empty() {
            excluded.push(json!({"reason": "no candidate context covers the protected floor", "floor_tokens": est}));
            continue; // the empty pool is refused above, explained
        }
        let started = Instant::now();
        let headroom = budget
            .policy
            .max_task_tokens
            .map(|m| m.saturating_sub(cx.ledger.reserved_tokens() + est));
        let routed = route_models(
            cx,
            task,
            pool.clone(),
            est,
            st.decision.as_mut(),
            &budget,
            headroom,
        )?;
        if routed.router_calls > 0 {
            let count = budget.count(&routed.provider, &routed.request_text);
            cx.ledger.reserve(
                &budget.policy,
                LedgerEntry {
                    label: format!("{label}-router"),
                    kind: "router".into(),
                    count: count.clone(),
                    output_reserve: ROUTER_OUTPUT_RESERVE,
                    cost_micros: 0,
                },
            )?;
            cx.observe_incurred(
                &routed.provider,
                "decision.evaluate",
                Stage::Decision,
                &count,
            );
        }
        let fell_back = routed.json["source"]
            .as_str()
            .is_some_and(|s| s.starts_with("Fallback"));
        cx.observe(
            routed.json["provider"].as_str().unwrap_or(""),
            "decision.evaluate",
            Stage::Decision,
            Role::Local,
            if fell_back {
                Availability::Fallback
            } else {
                Availability::Available
            },
            true,
            started,
        );
        let plan = pool
            .iter()
            .find(|m| m.id == routed.model)
            .expect("router chose a catalog model")
            .clone();
        let fit = {
            let build = |dr: &BTreeSet<String>| build_prompt(cx, p, dr);
            budget.fit(&plan, &optional, &build)
        };
        if fit.fits {
            // Pre-dispatch recheck of the chosen model against the final serialized request.
            if let Err(e) = recheck_dispatch(&routed.inputs, &plan.id, fit.required_tokens) {
                if cfg.routing.cfg.project_pin.is_some()
                    || matches!(cfg.routing.cfg.mode, RoutingMode::Pin(_))
                {
                    return Err(e);
                }
                excluded.push(json!({"model": plan.id, "recheck": e.message}));
                pool.retain(|m| m.id != plan.id);
                continue;
            }
            let mut bj = fit.to_json(&budget.policy);
            bj["rerouted_from"] = json!(excluded);
            r.context["request_budget"] = bj;
            if !fit.dropped.is_empty() {
                r.notes.push(format!(
                    "request repacked for `{}`: dropped optional {}",
                    fit.model,
                    fit.dropped.join(", ")
                ));
            }
            cx.ledger.reserve(
                &budget.policy,
                LedgerEntry {
                    label: label.into(),
                    kind: "generation".into(),
                    count: fit.count.clone(),
                    output_reserve: budget.policy.output_reserve_tokens,
                    cost_micros: plan.est_cost_micros,
                },
            )?;
            return Ok((fit, routed.json));
        }
        excluded.push(
            json!({"model": plan.id, "required_tokens": fit.required_tokens,
            "max_context": fit.max_context, "protected_floor_tokens": fit.floor_tokens,
            "basis": fit.count.to_json()["admission_basis"]}),
        );
        pool.retain(|m| m.id != plan.id);
    }
}

/// Generate one proposal under journal step `step`. A side-effecting generation
/// that began without a result is never replayed.
#[allow(clippy::too_many_arguments)]
pub(super) fn generate(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    prompt: Value,
    model: String,
    r: &mut Report,
    step: &str,
    count: &RequestCount,
) -> HarnessResult<Vec<u8>> {
    let side = st.proposer.side_effecting();
    let cache = cx.cfg.cache_dir.join(if step == "generate" {
        format!("{}.proposal.json", cx.lineage.id)
    } else {
        format!("{}.{step}.proposal.json", cx.lineage.id)
    });
    if side {
        if matches!(journal.state(step).map(|x| x.state.as_str()), Some("done")) {
            if let Ok(b) = std::fs::read(&cache) {
                r.notes.push(
                    "proposal reused from the journal; the model was not invoked again".into(),
                );
                return Ok(b);
            }
        }
        if journal.unfinished(step)
            || matches!(
                journal.state(step).map(|x| x.state.as_str()),
                Some("uncertain")
            )
        {
            return Err(d("SPX-HPD072", "uncertain: a model generation in this lineage began without a recorded result; it is not replayed, supply --proposal or change the task"));
        }
        journal.append(
            step,
            "begin",
            json!({"provider": st.proposer.id(), "request_digest": sha256_plain(request_text(&prompt).as_bytes())}),
        )?;
    }
    let started = Instant::now();
    let req = ProposalRequest {
        lineage: cx.lineage,
        prompt,
        model,
    };
    let got = st.proposer.propose(&req);
    cx.observe_incurred_at(
        &st.proposer.id(),
        "model.generate",
        Stage::Generation,
        got.is_ok(),
        started,
        count,
    );
    match got {
        Ok(b) => {
            if side {
                std::fs::write(&cache, &b)
                    .map_err(|e| d("SPX-HPD070", format!("proposal cache: {e}")))?;
                let detail = super::acquire::done_detail(cx, &b, count.to_json());
                journal.append(step, "done", detail)?;
            }
            Ok(b)
        }
        Err(StageFailure::Uncertain(x)) => {
            journal.append(step, "uncertain", json!({}))?;
            Err(d(
                "SPX-HPD072",
                format!(
                    "uncertain: model outcome unknown, not retried ({})",
                    x.message
                ),
            ))
        }
        Err(StageFailure::Refused(x)) => {
            if side {
                journal.append(step, "refused", json!({"code": x.code}))?;
            }
            Err(x)
        }
        Err(StageFailure::Unavailable(x)) => {
            if side {
                journal.append(step, "refused", json!({"code": x.code}))?;
            }
            Err(d(
                "SPX-HPD090",
                format!("no proposal available: {} {}", x.code, x.message),
            ))
        }
    }
}

/// Route, fit, reserve, generate and parse one proposal.
pub(super) fn propose_step(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    p: &PromptCtx,
    step: &str,
) -> HarnessResult<Proposal> {
    let bytes = match super::acquire::local_proposal(cx, st, journal, r, step)? {
        Some(b) => b,
        None => {
            let (fit, route_json) = route_and_fit(cx, st, r, p, step)?;
            r.route = route_json;
            let model = fit.model.clone();
            generate(
                cx,
                st,
                journal,
                fit.prompt.clone(),
                model,
                r,
                step,
                &fit.count,
            )?
        }
    };
    let v2 = cx.cfg.task.schema_version == 2;
    let proposal = parse_proposal(&bytes).map_err(|e| {
        if v2 && e.code == "SPX-HPD031" && e.message.starts_with("unsupported change kind") {
            d("SPX-HPD092", e.message)
        } else {
            e
        }
    })?;
    if let Some(c) = proposal.claims.as_object() {
        r.ignored_claims = c.keys().cloned().collect();
    }
    Ok(proposal)
}

/// Classify a proposal that carries no intent for the single-shot path.
pub(super) fn require_intent(p: &Proposal, ops: &[String]) -> HarnessResult<()> {
    if let Some(reason) = &p.unsupported {
        return Err(d(
            "SPX-HPD092",
            format!(
                "unsupported goal: the proposer reports no admitted operation fits ({reason}); installed operations: {}",
                ops.join(", ")
            ),
        ));
    }
    match p.kind.as_str() {
        "done" => Err(d("SPX-HPD030", "the proposal states the goal is done but carries no change")),
        "source_patch" => Err(d(
            "SPX-HPD031",
            "`source_patch`: scratch source edits exist only for an unverified baseline in a session",
        )),
        k if !ops.iter().any(|o| o == k) => Err(d(
            "SPX-HPD092",
            format!(
                "unsupported goal: change kind `{k}` is not admitted by the installed compiler; installed operations: {}",
                ops.join(", ")
            ),
        )),
        _ => Ok(()),
    }
}

/// Validate through the compiler's candidate operation, same revision.
pub(super) fn validate_step(
    cx: &mut Ctx,
    root: &Path,
    revision: &str,
    proposal: &Proposal,
    r: &mut Report,
) -> HarnessResult<(Vec<u8>, CandidatePreview)> {
    let change = change_bytes(revision, &proposal.intent);
    let preview = cx.compiler.candidate_preview(root, &change)?;
    check_protected_facts(root, revision, &proposal.kind, &preview)?;
    r.candidate = json!({"intent": proposal.kind, "base_revision": preview.base_revision, "candidate_revision": preview.candidate_revision,
                         "changed_files": preview.source_changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
                         "preview_digest": preview.digest});
    Ok((change, preview))
}

fn spx_files(dir: &Path, out: &mut Vec<std::path::PathBuf>, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut es: Vec<_> = rd.flatten().collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        match e.file_type() {
            Ok(t) if t.is_dir() && depth < 16 && !name.starts_with('.') && name != "target" => {
                spx_files(&p, out, depth + 1)
            }
            Ok(t) if t.is_file() && name.ends_with(".spx") => out.push(p),
            _ => {}
        }
    }
}

/// Host-owned acceptance: `{stable_id, contains}` items are checked against the
/// compiler-verified candidate sources (the declaration carrying `@id("..")`);
/// strings are carried to the model, not verified. A proposal cannot influence it.
pub(super) fn verify_acceptance(_cx: &Ctx, root: &Path, items: &[Value]) -> Result<usize, String> {
    let mut files = Vec::new();
    spx_files(root, &mut files, 0);
    let texts: Vec<String> = files
        .iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect();
    let mut verified = 0;
    for it in items.iter().filter_map(Value::as_object) {
        let (id, needle) = (
            it["stable_id"].as_str().unwrap_or(""),
            it["contains"].as_str().unwrap_or(""),
        );
        let marker = format!("@id(\"{id}\")");
        // The declaration site, not an import that names the same stable id.
        let decl = texts.iter().find_map(|t| {
            t.match_indices(&marker).find_map(|(i, _)| {
                let rest = &t[i + marker.len()..];
                (!rest.trim_start().starts_with("from "))
                    .then(|| rest.split("@id(").next().unwrap_or(rest).to_string())
            })
        });
        match decl {
            None => return Err(format!("acceptance unmet: `{id}` is not declared")),
            Some(d) if d.contains(needle) => verified += 1,
            Some(_) => {
                return Err(format!(
                    "acceptance unmet: `{id}` does not contain `{needle}`"
                ))
            }
        }
    }
    Ok(verified)
}

fn copy_tree(from: &Path, to: &Path, depth: usize) -> HarnessResult<()> {
    let io = |e: std::io::Error| d("SPX-HPD070", format!("scratch copy: {e}"));
    std::fs::create_dir_all(to).map_err(io)?;
    for e in std::fs::read_dir(from).map_err(io)? {
        let e = e.map_err(io)?;
        let name = e.file_name().to_string_lossy().into_owned();
        let t = e.file_type().map_err(io)?;
        if t.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" || depth > 16 {
                continue;
            }
            copy_tree(&e.path(), &to.join(&name), depth + 1)?;
        } else if t.is_file() && !name.starts_with("semaprax.harness") {
            std::fs::copy(e.path(), to.join(&name)).map_err(io)?;
        }
    }
    Ok(())
}

pub(super) fn copy_project(from: &Path, to: &Path) -> HarnessResult<()> {
    let _ = std::fs::remove_dir_all(to);
    copy_tree(from, to, 0)
}

/// Materialize the compiler-produced candidate sources over a private copy of
/// `base_root` (never the project) and have the compiler check and test it.
/// Returns the check summary; the verdict is the compiler's.
pub(super) fn candidate_checks(
    cx: &mut Ctx,
    command: &mut dyn CommandStage,
    base_root: &Path,
    tag: &str,
    preview: &CandidatePreview,
    r: &mut Report,
    acceptance: bool,
) -> HarnessResult<Value> {
    let scratch = cx
        .cfg
        .cache_dir
        .join(format!("scratch-{}-{tag}", cx.lineage.id));
    copy_project(base_root, &scratch)?;
    for c in &preview.source_changes {
        std::fs::write(scratch.join(&c.path), &c.replacement_source)
            .map_err(|e| d("SPX-HPD070", format!("scratch write: {e}")))?;
    }
    let scratch = scratch
        .canonicalize()
        .map_err(|e| d("SPX-HPD070", format!("scratch: {e}")))?;
    let out = (|| {
        let check = cx.compiler.check(&scratch)?;
        if !check.ok {
            let first = check
                .diagnostics
                .first()
                .map(|x| format!("{} {}", x.code, x.message))
                .unwrap_or_default();
            return Err(d(
                "SPX-HPD050",
                format!("candidate rejected: the compiler's check failed ({first})"),
            ));
        }
        if check.revision.as_deref() != Some(preview.candidate_revision.as_str()) {
            return Err(d(
                "SPX-HPD041",
                "candidate check revision differs from the previewed candidate revision",
            ));
        }
        let test = cx.compiler.test(&scratch)?;
        if test.project_revision != preview.candidate_revision {
            return Err(d(
                "SPX-HPD041",
                "candidate test revision differs from the previewed candidate revision",
            ));
        }
        if !test.passed {
            r.checks = json!({"check": "verified", "tests": "failed", "report_digest": test.report_digest});
            return Err(d(
                "SPX-HPD050",
                format!(
                    "candidate rejected: tests failed ({})",
                    test.failure.clone().unwrap_or_else(|| test.outcome.clone())
                ),
            ));
        }
        // A session judges acceptance after each admitted step, not on partial work.
        let acceptance = if acceptance {
            verify_acceptance(cx, &scratch, &cx.cfg.task.acceptance).map_err(|m| {
                r.checks = json!({"check": "verified", "tests": "passed", "acceptance": "unmet"});
                d("SPX-HPD050", format!("candidate rejected: {m}"))
            })?
        } else {
            0
        };
        let mut runs: Vec<Value> = Vec::new();
        let selected: Vec<_> = cx
            .cfg
            .checks
            .iter()
            .filter(|c| {
                cx.cfg
                    .task
                    .checks
                    .as_ref()
                    .is_none_or(|n| n.contains(&c.name))
            })
            .cloned()
            .collect();
        if !selected.is_empty() {
            // Authorized checks see the project's own configuration (scope, mode).
            let cfg_file = cx
                .cfg
                .snapshot
                .root
                .join(crate::profile::config::CONFIG_FILE);
            if cfg_file.is_file() {
                let _ = std::fs::copy(&cfg_file, scratch.join(crate::profile::config::CONFIG_FILE));
            }
            for c in &selected {
                let run = match command.run_check(c, &scratch, &mut *cx.observer) {
                    None => {
                        return Err(d(
                            "SPX-HPD051",
                            format!("authorized check `{}` cannot run: the command stage executes no checks", c.name),
                        ))
                    }
                    Some(Err(e)) => {
                        return Err(d(
                            "SPX-HPD051",
                            format!("authorized check `{}` was refused: {} {}", c.name, e.code, e.message),
                        ))
                    }
                    Some(Ok(run)) => run,
                };
                runs.push(run.to_json());
                if !run.passed {
                    r.checks = json!({"check": "verified", "tests": "passed", "commands": runs});
                    return Err(d(
                        "SPX-HPD050",
                        format!(
                            "candidate rejected: authorized check `{}` failed ({})",
                            c.name, run.status
                        ),
                    ));
                }
            }
        }
        Ok(
            json!({"check": "verified", "tests": "passed", "candidate_revision": preview.candidate_revision, "report_digest": test.report_digest, "commands": runs, "acceptance_verified": acceptance}),
        )
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    out
}
