//! One proposal attempt: budget-checked routing, generation, compiler preview
//! and candidate checks. Shared by the single-shot pipeline and the HN-02
//! session so both spend one per-task ledger (HN-01, HN-11, HN-02).

use super::budget::{request_text, Fit, LedgerEntry, RequestCount};
use super::compiler::CandidatePreview;
use super::generation::{is_truncation, ResponseShape};
use super::journal::Journal;
use super::pipeline::{change_bytes, Ctx, Stages};
use super::policy::check_protected_facts;
use super::report::Report;
use super::route_signals::route_signals;
use super::spend_dispatch::Attempt;
use super::stages::*;
use crate::decision::{
    gate_attests_key, recheck_dispatch, router_output_reserve, wire_version, Budget,
    Confidentiality, ConfiguredProvider, Destination, EvidenceKey, Governor, LatencyClass,
    ModelPlan, PreparedRouteV2, ProviderMode, RouteContext, RouteInputs, RoutePolicy, RouteRequest,
    RouteSignals, RoutingConfig, RoutingMode, TaskFamily, TaskFeatures,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use crate::observe::{Availability, Role, Stage};
use crate::receipt::GenerationControls;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

/// Closed-choice router output reserve for a pool (MR-03): protocol-derived
/// from the option count, not a generation-sized constant.
pub(super) fn router_reserve(pool: &[ModelPlan]) -> u64 {
    router_output_reserve(pool.len().min(crate::decision::render::MAX_CANDIDATES_V2))
}

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn response_shape(p: &PromptCtx) -> ResponseShape {
    if p.scratch_repair {
        ResponseShape::SourceRepair
    } else {
        ResponseShape::StructuredIntent
    }
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
    /// MR-08 role request material; `None` is the single-proposer request.
    pub phase: Option<&'a super::phases::PhaseView>,
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
    use super::phases::Role;
    if let Some(v) = p.phase.filter(|v| v.role != Role::Implement) {
        // A plan/review request is its own bounded host-built document.
        return v.body.clone();
    }
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
    if let Some(v) = p.phase {
        // The compact host handoff replaces raw feedback (MR-08).
        if v.body.get("handoff").is_some() {
            if let Some(o) = prompt.as_object_mut() {
                o.remove("feedback");
            }
        }
        for (k, x) in v.body.as_object().into_iter().flatten() {
            prompt[k] = x.clone();
        }
    }
    if let Some(sp) = &cx.cfg.skill_prompt {
        if !dropped.contains("skills") {
            // Quoted data below host and compiler authority (framed by the skill service).
            prompt["skills"] = json!(sp.text);
        }
    }
    match cx.cfg.budget.generation.renderer {
        super::prompt_render::PromptRenderer::Canonical => prompt,
        super::prompt_render::PromptRenderer::OrderedV1 => {
            let ids = cx.cfg.skill_prompt.as_ref().map_or(vec![], |s| {
                if dropped.contains("skills") {
                    vec![]
                } else {
                    s.loaded.clone()
                }
            });
            super::prompt_render::render_ordered(&prompt, &ids)
        }
    }
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
            descriptor: Default::default(),
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
    /// Router calls made, decision plus shadow (what the spend record settles).
    router_calls_total: u32,
    request_text: String,
    provider: String,
    /// MR-03 typed identity/usage of the answering router call, if any.
    call: Option<crate::decision::CallMetadata>,
}

/// Routing allowance when no task cost limit is set (the earlier fixed figure).
const DEFAULT_ROUTE_ALLOWANCE_MICROS: u64 = 1_000_000;

/// Route features, budget and the router request text the host reserves
/// against. The cost allowance is the task's remaining cost budget when one is
/// set (TC-03). For a v2 router the text is the host-rendered prepared request
/// over the whole pool (MR-03: a conservative superset of what is sent).
pub(super) fn route_parts(
    task: &Task,
    catalog: &[ModelPlan],
    estimated_tokens: u64,
    allowance_micros: u64,
    router: bool,
    signals: &RouteSignals,
    wire_v2: bool,
) -> HarnessResult<(TaskFeatures, Budget, String)> {
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
        max_cost_micros: allowance_micros,
        max_latency_ms: 60_000,
        max_router_calls: u32::from(router),
    };
    let v1_text = || {
        crate::json::canonical(&json!({"features": features.to_json(),
        "catalog": catalog.iter().map(ModelPlan::to_json).collect::<Vec<_>>(), "budget": budget.to_json()}))
    };
    let text = if wire_v2 {
        RouteRequest::new(features.clone(), catalog.to_vec(), budget.clone())
            .map(|r| r.with_signals(signals.clone()))
            .and_then(|r| PreparedRouteV2::prepare(&r, &RoutePolicy::default(), &r.catalog))
            .map(|p| p.accounting_text())
            .unwrap_or_else(|_| v1_text())
    } else {
        v1_text()
    };
    Ok((features, budget, text))
}

#[allow(clippy::too_many_arguments)]
fn route_models(
    cx: &Ctx,
    task: &Task,
    catalog: Vec<ModelPlan>,
    estimated_tokens: u64,
    decision: Option<&mut super::pipeline::DecisionStage>,
    rb: &super::budget::RequestBudget,
    router_headroom_tokens: Option<u64>,
    allowance_micros: u64,
    signals: &RouteSignals,
) -> HarnessResult<Routed> {
    let wire_v2 = decision
        .as_ref()
        .is_some_and(|d| wire_version(&d.profile, &*d.invoker) == 2);
    let (features, budget, text) = route_parts(
        task,
        &catalog,
        estimated_tokens,
        allowance_micros,
        decision.is_some(),
        signals,
        wire_v2,
    )?;
    let reserve = router_reserve(&catalog);
    let request = RouteRequest::new(features, catalog, budget)?.with_signals(signals.clone());
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
        rb.count(&d.profile.provider_id, &text).admission_tokens() + reserve
    });
    let live = inputs.clone();
    // The workflow carries only the stage's gate: a learned provider is
    // consulted in `Auto` mode only when that gate attests the live key
    // (provider, weights, approved catalog); otherwise rules decide (HN-16).
    let wiring = &cx.cfg.routing;
    let live_key = decision.as_ref().map(|d| {
        EvidenceKey::live_versioned(
            &d.profile,
            &inputs.request.catalog_digest(),
            wire_version(&d.profile, &*d.invoker),
        )
    });
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
    // The session-owned cache and readiness (MR-12); hits are revalidated.
    let (gr, reuse) = cx.cfg.routing.decisions.borrow_mut().decide(
        &g,
        &inputs,
        &rctx,
        configured.as_mut(),
        &move || live.clone(),
    )?;
    let reuse_json = reuse.to_json();
    let explain = super::route_explain::explain(
        &inputs,
        &cfg,
        &gr,
        &reuse_json,
        live_key.as_ref().map(|k| k.digest()),
        router_request_tokens,
    );
    let dec = gr.decision;
    Ok(Routed {
        inputs,
        json: json!({"explain": explain, "choice": dec.choice, "provider": dec.provider_id, "router_calls": dec.router_calls,
               "status": dec.provider_status, "source": format!("{:?}", dec.source),
               "mode": gr.mode, "rules_reason": gr.rules_reason, "explanation": gr.explanation,
               "wire": dec.wire.to_json(), "reuse": reuse_json,
               "policy": {"allow_remote": cfg.user_allow_remote, "project_pin": cfg.project_pin}}),
        model: dec.choice,
        router_calls: dec.router_calls,
        router_calls_total: gr.router_calls_total,
        request_text: text,
        provider: dec.provider_id,
        call: dec.wire.call,
    })
}

/// Select a model and fit the exact serialized request to it before any
/// provider call: protected content is never truncated; optional material is
/// dropped whole; a model that cannot fit is excluded and routing repeats.
pub(super) fn route_and_fit(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    p: &PromptCtx,
    label: &str,
) -> HarnessResult<(Fit, Value, Attempt)> {
    let cfg = cx.cfg;
    let task = &cfg.task;
    let shape = response_shape(p);
    cfg.budget
        .generation
        .gate(shape, &st.proposer.generation_support())?;
    let role = p.phase.map_or(super::phases::Role::Implement, |v| v.role);
    let implement = role == super::phases::Role::Implement;
    let all = super::phases::narrow(cx, role, catalog(cx, task)?)?;
    let mut budget = cfg.budget.for_task(task);
    // Accepted output reservation: a bounded retry's cap, else the configured tier, else the budget default.
    budget.policy.output_reserve_tokens = cx.reserve_override.take().unwrap_or_else(|| {
        cfg.budget
            .generation
            .reserve_for(shape, budget.policy.output_reserve_tokens)
    });
    // Adapter-declared framing is model-visible and counted in admission.
    budget.policy.protocol_overhead_tokens = budget
        .policy
        .protocol_overhead_tokens
        .saturating_add(st.proposer.framing_overhead_tokens());
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
        let allowance = cx
            .ledger
            .spend
            .available_cost()
            .unwrap_or(DEFAULT_ROUTE_ALLOWANCE_MICROS);
        // Opt-in cost-aware ladder (TC-10): narrows the pool before rules decide
        // and bypasses a paid router whose benefit is unknown.
        let cost = if implement {
            super::cost_ladder::apply(
                cx,
                &pool,
                est,
                budget.policy.output_reserve_tokens,
                st.decision.as_ref().map(|d| &d.profile),
            )
        } else {
            None
        };
        // A router call is admitted and journaled before it can happen; when it
        // is unaffordable (or unpriced under strict money) rules decide alone.
        let signals = route_signals(task, p, cx.ledger.spend.available_cost());
        let router = if cost.is_some() || !super::phases::router_allowed(cx, role, &pool) {
            None
        } else {
            super::spend_dispatch::reserve_router(
                cx, st, journal, r, &budget, &pool, est, allowance, label, &signals,
            )?
        };
        let decision = if router.is_some() {
            st.decision.as_mut()
        } else {
            None
        };
        let mut routed = route_models(
            cx,
            task,
            cost.as_ref()
                .map_or_else(|| pool.clone(), |c| c.pool.clone()),
            est,
            decision,
            &budget,
            headroom,
            allowance,
            &signals,
        )?;
        if let Some(id) = &router {
            let model = st
                .decision
                .as_ref()
                .map_or_else(String::new, |d| d.profile.model_id.clone());
            super::spend_dispatch::settle_router(
                cx,
                journal,
                id,
                routed.router_calls_total,
                routed.call.as_ref(),
                &model,
                router_reserve(&pool),
            )?;
        }
        if routed.router_calls > 0 {
            let count = budget.count(&routed.provider, &routed.request_text);
            cx.ledger.entries.push(LedgerEntry {
                id: router.clone().unwrap_or_default(),
                label: format!("{label}-router"),
                kind: "router".into(),
                count: count.clone(),
                output_reserve: router_reserve(&pool),
                cost_micros: router
                    .as_deref()
                    .and_then(|id| cx.ledger.spend.record(id))
                    .map_or(0, |x| x.reserved_cost),
            });
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
        if implement {
            super::cost_ladder::note_model(cx, &plan.id);
        }
        if let Some(c) = &cost {
            routed.json["cost_policy"] = c.json.clone();
        }
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
            super::route_explain::set_phase(&mut routed.json, role.as_str(), &excluded);
            super::route_explain::set_deployment(&mut routed.json, &st.proposer.id(), &plan.id);
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
            let attempt = super::spend_dispatch::reserve_generation(
                cx, st, journal, &budget, &plan, &fit, label,
            )?;
            return Ok((fit, routed.json, attempt));
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
    controls: &GenerationControls,
    attempt: &Attempt,
) -> HarnessResult<Vec<u8>> {
    let side = st.proposer.side_effecting();
    let cache = super::acquire::cache_path(cx, step);
    if side {
        // A completed step is reused only through the shared artifact validator
        // (MN-01): no unchecked cache read, and a failed check is never replaced
        // by a fresh billable request.
        if let Some(rec) = journal.state(step).filter(|x| x.state == "done").cloned() {
            let checked = super::acquire::validate_done(cx, step, &rec);
            let why = if checked.is_ok() {
                "reused_not_dispatched"
            } else {
                "refused_not_dispatched"
            };
            super::spend_dispatch::release(cx, journal, attempt, why)?;
            let b = checked?;
            r.notes
                .push("proposal reused from the journal; the model was not invoked again".into());
            return Ok(b);
        }
        if journal.unfinished(step)
            || matches!(
                journal.state(step).map(|x| x.state.as_str()),
                Some("uncertain")
            )
        {
            super::spend_dispatch::release(cx, journal, attempt, "refused_not_dispatched")?;
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
        controls: controls.clone(),
    };
    let (got, receipt) = st.proposer.propose_receipted(&req);
    super::route_explain::set_generation(
        &mut r.route,
        &st.proposer.id(),
        &req.model,
        receipt.model.as_deref(),
    );
    let estimate = cx.cfg.budget.prices.estimate(&req.model, &receipt.usage);
    cx.observe_incurred_at(
        &st.proposer.id(),
        "model.generate",
        Stage::Generation,
        got.is_ok(),
        started,
        count,
        Some((&receipt, &estimate)),
    );
    cx.receipts.push(
        step,
        &st.proposer.id(),
        &req.model,
        count.admission_tokens(),
        controls.max_output_tokens.unwrap_or(0),
        &receipt,
        &estimate,
    );
    // Settle from the receipt before the step's terminal record (TC-03).
    let known = !matches!(got, Err(StageFailure::Uncertain(_)));
    super::spend_dispatch::settle_generation(cx, journal, attempt, &receipt, &estimate, known)?;
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

/// Admission of an output-cap retry by the enclosing owner (MN-04): a session
/// applies its attempt, cancellation and elapsed limits and charges the retry
/// once; the single-shot pipeline admits it unconditionally.
pub(super) type RetryGate<'g> = &'g mut dyn FnMut(&Ctx, &mut Journal) -> HarnessResult<()>;

/// Route, fit, reserve and generate on the model branch. A length-limited reply
/// is a known terminal outcome: when configured and admitted by `gate`, one new
/// attempt at the larger cap, reserved before dispatch like any other.
fn generate_with_retry(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    p: &PromptCtx,
    step: &str,
    gate: RetryGate,
) -> HarnessResult<Vec<u8>> {
    let (mut fit, route_json, mut attempt) = route_and_fit(cx, st, journal, r, p, step)?;
    r.route = route_json;
    let shape = response_shape(p);
    let mut controls = cx.cfg.budget.generation.controls(shape, fit.output_reserve);
    let mut gstep = step.to_string();
    loop {
        let model = fit.model.clone();
        match generate(
            cx,
            st,
            journal,
            fit.prompt.clone(),
            model,
            r,
            &gstep,
            &fit.count,
            &controls,
            &attempt,
        ) {
            // A length-limited reply is a known terminal outcome: one new attempt
            // at the configured larger cap, reserved before dispatch like any other.
            Err(e)
                if is_truncation(&e)
                    && gstep == step
                    && cx
                        .cfg
                        .budget
                        .generation
                        .length_retry_cap
                        .is_some_and(|c| c > fit.output_reserve) =>
            {
                // The enclosing limits are checked before routing or reservation.
                gate(cx, journal)?;
                cx.reserve_override = cx.cfg.budget.generation.length_retry_cap;
                gstep = super::acquire::retry_step(step);
                r.notes.push(format!(
                    "reply length-limited at {} output tokens; one new attempt at the larger cap",
                    fit.output_reserve
                ));
                let (f2, rj, a2) = route_and_fit(cx, st, journal, r, p, &gstep)?;
                r.route = rj;
                fit = f2;
                attempt = a2;
                controls = cx.cfg.budget.generation.controls(shape, fit.output_reserve);
            }
            other => return other,
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
    propose_step_gated(cx, st, journal, r, p, step, &mut |_, _| Ok(()))
}

/// `propose_step` whose output-cap retry is admitted by `gate` (MN-04).
pub(super) fn propose_step_gated(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    p: &PromptCtx,
    step: &str,
    gate: RetryGate,
) -> HarnessResult<Proposal> {
    let bytes = match super::acquire::local_proposal(cx, st, journal, r, step)? {
        Some(b) => b,
        None => generate_with_retry(cx, st, journal, r, p, step, gate)?,
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

/// Host-owned acceptance: `{stable_id, contains}` items are checked inside the
/// declaration the compiler binds to the stable id in the checked candidate
/// (see `acceptance`); strings are carried to the model, not verified.
pub(super) fn verify_acceptance(
    cx: &Ctx,
    root: &Path,
    revision: &str,
    items: &[Value],
) -> Result<usize, String> {
    super::acceptance::verify(cx, root, revision, items)
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
            verify_acceptance(
                cx,
                &scratch,
                &preview.candidate_revision,
                &cx.cfg.task.acceptance,
            )
            .map_err(|m| {
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
