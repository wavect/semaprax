//! Policy-first routing: hard screen, rules or router consultation, post-call
//! revalidation and deterministic fallback. Frozen plans come out of here.

use super::cache::{CacheKey, DecisionCache};
use super::plan::FrozenRoutePlan;
use super::policy::{FallbackMode, RoutePolicy};
use super::provider::{ConfiguredProvider, DecisionCall};
use super::registry::DecisionTask;
use super::route::{bad, screen, RouteRequest, Screening};
use super::rules::{rules_choice, RULES_CHECKPOINT, RULES_PROVIDER_ID};
use crate::contract::payload::check_against_request;
use crate::contract::{
    validate_payload, CapabilityKind, CapabilityRef, Direction, ProjectBinding, RequestEnvelope,
};
use crate::diag::HarnessResult;
use crate::json;
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct RouteInputs {
    pub request: RouteRequest,
    pub policy: RoutePolicy,
}

impl RouteInputs {
    /// Policy digest covers the policy and the request budget.
    pub fn policy_digest(&self) -> String {
        json::digest(
            "semaprax.decision.policy-budget.v1",
            &json!({"policy": self.policy.to_json(), "budget": self.request.budget.to_json()}),
        )
    }

    pub fn digests(&self, scr: &Screening) -> Digests {
        Digests {
            features: self.request.features_digest(),
            catalog: self.request.catalog_digest(),
            policy: self.policy_digest(),
            candidates: scr.candidate_digest(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Digests {
    pub features: String,
    pub catalog: String,
    pub policy: String,
    pub candidates: String,
}

impl Digests {
    pub fn to_json(&self) -> Value {
        json!({"features": self.features, "catalog": self.catalog, "policy": self.policy, "candidates": self.candidates})
    }
}

/// Binds a chosen model to the exact digests it was chosen under.
pub fn choice_digest(d: &Digests, provider_id: &str, checkpoint: &str, choice: &str) -> String {
    json::digest(
        "semaprax.decision.choice.v1",
        &json!({"task": DecisionTask::ModelRoute.id(), "provider": provider_id, "checkpoint": checkpoint, "digests": d.to_json(), "choice": choice}),
    )
}

#[derive(Clone, Debug)]
pub struct RouteContext {
    pub project: ProjectBinding,
    pub lock_digest: String,
    pub invocation_id: String,
    pub lineage_id: String,
    /// Router ids already on this decision lineage (recursion guard).
    pub router_lineage: Vec<String>,
    pub router_calls_used: u32,
    pub router_ms_used: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackReason {
    Abstain,
    Unavailable,
    Timeout,
    OutOfDistribution,
    LowConfidence,
    Stale,
    RejectedChoice,
    InvalidResult,
    CallCapExhausted,
    LatencyExhausted,
    RecursionBlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionSource {
    /// Rules chosen by policy (no router selected or enabled).
    Rules,
    /// One admissible plan or a rules-only family: zero router calls.
    Trivial,
    Cache,
    Provider,
    Fallback(FallbackReason),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RouteDecision {
    pub choice: String,
    pub source: DecisionSource,
    /// Identity that produced `choice` (rules for rules/fallback).
    pub provider_id: String,
    pub checkpoint: String,
    pub provider_status: &'static str,
    pub router_calls: u32,
    pub router_ms: u64,
    pub digests: Digests,
    pub plan: FrozenRoutePlan,
}

struct Consulted {
    choice: String,
    cached: bool,
}

/// Decide a route. `live` returns the current inputs and is evaluated after
/// any router inference so a stale decision is rejected, not trusted.
pub fn decide(
    inputs: &RouteInputs,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_>>,
    live: &dyn Fn() -> RouteInputs,
    cache: Option<&mut DecisionCache>,
) -> HarnessResult<RouteDecision> {
    let scr = screen(&inputs.request, &inputs.policy);
    if scr.admissible.is_empty() {
        return Err(no_safe_model(&scr));
    }
    let task = DecisionTask::ModelRoute.id();
    let f = &inputs.request.features;
    let trivial =
        scr.admissible.len() == 1 || inputs.policy.rules_only_families.contains(&f.task_family);
    let digests = inputs.digests(&scr);
    let status = provider.as_ref().map_or("rules", |p| p.status(task));
    let finish = |inputs: &RouteInputs,
                  scr: &Screening,
                  d: Digests,
                  choice: &str,
                  source,
                  pid: &str,
                  ck: &str,
                  calls,
                  ms|
     -> HarnessResult<RouteDecision> {
        let plan = FrozenRoutePlan::freeze(
            &ctx.lineage_id,
            choice,
            &scr.admissible,
            choice_digest(&d, pid, ck, choice),
            inputs.policy_digest(),
        )?;
        Ok(RouteDecision {
            choice: choice.to_string(),
            source,
            provider_id: pid.into(),
            checkpoint: ck.into(),
            provider_status: status,
            router_calls: calls,
            router_ms: ms,
            digests: d,
            plan,
        })
    };
    let rules_pick = |scr: &Screening, i: &RouteInputs| {
        rules_choice(&i.request.features, &scr.admissible, &i.policy).map(|p| p.id.clone())
    };

    let usable = match provider {
        Some(p) if !trivial && p.enabled(task) => Some(p),
        _ => None,
    };
    let Some(p) = usable else {
        let pick = rules_pick(&scr, inputs).expect("non-empty admissible set");
        let src = if trivial {
            DecisionSource::Trivial
        } else {
            DecisionSource::Rules
        };
        return finish(
            inputs,
            &scr,
            digests,
            &pick,
            src,
            RULES_PROVIDER_ID,
            RULES_CHECKPOINT,
            0,
            0,
        );
    };

    let (mut calls, mut ms) = (0u32, 0u64);
    let outcome = consult(inputs, ctx, p, &scr, &digests, &mut calls, &mut ms, cache);
    let reason = match outcome {
        Ok(c) => {
            // Revalidate against live state: a valid choice is not a correct one.
            let cur = live();
            let cur_scr = screen(&cur.request, &cur.policy);
            let cur_d = cur.digests(&cur_scr);
            if cur_d.catalog != digests.catalog
                || cur_d.policy != digests.policy
                || cur_d.features != digests.features
            {
                return fall_back(
                    &cur,
                    &cur_scr,
                    cur_d,
                    FallbackReason::Stale,
                    calls,
                    ms,
                    status,
                    &ctx.lineage_id,
                );
            }
            if !cur_scr.admissible.iter().any(|m| m.id == c.choice) {
                return fall_back(
                    &cur,
                    &cur_scr,
                    cur_d,
                    FallbackReason::RejectedChoice,
                    calls,
                    ms,
                    status,
                    &ctx.lineage_id,
                );
            }
            let src = if c.cached {
                DecisionSource::Cache
            } else {
                DecisionSource::Provider
            };
            return finish(
                &cur,
                &cur_scr,
                cur_d,
                &c.choice,
                src,
                &p.profile.provider_id,
                &p.profile.checkpoint,
                calls,
                ms,
            );
        }
        Err(r) => r,
    };
    fall_back(
        inputs,
        &scr,
        digests,
        reason,
        calls,
        ms,
        status,
        &ctx.lineage_id,
    )
}

fn no_safe_model(scr: &Screening) -> crate::diag::HarnessDiagnostic {
    let why: Vec<String> = scr
        .excluded
        .iter()
        .map(|(i, r)| format!("{i}: {r}"))
        .collect();
    bad(
        "SPX-HPJ005",
        format!("no admissible model remains ({})", why.join("; ")),
    )
}

#[allow(clippy::too_many_arguments)]
fn fall_back(
    cur: &RouteInputs,
    scr: &Screening,
    d: Digests,
    reason: FallbackReason,
    calls: u32,
    ms: u64,
    status: &'static str,
    lineage: &str,
) -> HarnessResult<RouteDecision> {
    if cur.policy.fallback == FallbackMode::Refuse {
        return Err(bad(
            "SPX-HPJ013",
            format!("router fallback refused ({reason:?}); policy fallback is `refuse`"),
        ));
    }
    let pick = rules_choice(&cur.request.features, &scr.admissible, &cur.policy)
        .ok_or_else(|| no_safe_model(scr))?;
    let plan = FrozenRoutePlan::freeze(
        lineage,
        &pick.id,
        &scr.admissible,
        choice_digest(&d, RULES_PROVIDER_ID, RULES_CHECKPOINT, &pick.id),
        cur.policy_digest(),
    )?;
    Ok(RouteDecision {
        choice: pick.id.clone(),
        source: DecisionSource::Fallback(reason),
        provider_id: RULES_PROVIDER_ID.into(),
        checkpoint: RULES_CHECKPOINT.into(),
        provider_status: status,
        router_calls: calls,
        router_ms: ms,
        digests: d,
        plan,
    })
}

/// Pre-call guards, the router call, and structural validation of its answer.
#[allow(clippy::too_many_arguments)]
fn consult(
    inputs: &RouteInputs,
    ctx: &RouteContext,
    p: &mut ConfiguredProvider<'_>,
    scr: &Screening,
    d: &Digests,
    calls: &mut u32,
    ms: &mut u64,
    cache: Option<&mut DecisionCache>,
) -> Result<Consulted, FallbackReason> {
    use FallbackReason as R;
    let pid = p.profile.provider_id.clone();
    if ctx.router_lineage.contains(&pid) {
        return Err(R::RecursionBlocked);
    }
    if !p.profile.covers(&inputs.request.features) {
        return Err(R::OutOfDistribution);
    }
    let cap = inputs
        .request
        .budget
        .max_router_calls
        .min(inputs.policy.router_max_calls);
    let remaining = cap.saturating_sub(ctx.router_calls_used);
    let latency_left = inputs
        .policy
        .router_max_latency_ms
        .saturating_sub(ctx.router_ms_used);
    let key = CacheKey {
        provider_id: pid.clone(),
        model_id: p.profile.model_id.clone(),
        checkpoint: p.profile.checkpoint.clone(),
        features: d.features.clone(),
        catalog: d.catalog.clone(),
        policy: d.policy.clone(),
    };
    let mut cache = cache;
    if let Some(c) = cache.as_deref_mut() {
        if let Some(choice) = c.get(&key) {
            return Ok(Consulted {
                choice,
                cached: true,
            });
        }
    }
    if remaining == 0 {
        return Err(R::CallCapExhausted);
    }
    if latency_left == 0 {
        return Err(R::LatencyExhausted);
    }
    let options: Vec<&str> = scr.admissible.iter().map(|m| m.id.as_str()).collect();
    let payload = json!({"task": DecisionTask::ModelRoute.id(), "features": inputs.request.features.to_json(), "options": options});
    let mut lineage = ctx.router_lineage.clone();
    lineage.push(pid);
    let env = RequestEnvelope {
        invocation_id: ctx.invocation_id.clone(),
        project: ctx.project.clone(),
        lock_digest: ctx.lock_digest.clone(),
        capability: CapabilityRef {
            kind: CapabilityKind::DecisionEvaluate,
            version: 1,
        },
        operation: "evaluate".into(),
        deadline_ms: latency_left.clamp(1, 600_000),
        max_result_bytes: 65_536,
        remaining_calls: remaining,
        lineage,
        payload,
    };
    env.validate().map_err(|_| R::InvalidResult)?;
    *calls += 1;
    let (result, elapsed) = match p.invoker.evaluate(&env) {
        DecisionCall::Unavailable => return Err(R::Unavailable),
        DecisionCall::Timeout => return Err(R::Timeout),
        DecisionCall::Answered { result, elapsed_ms } => (result, elapsed_ms),
    };
    *ms += elapsed;
    if elapsed > latency_left {
        return Err(R::Timeout);
    }
    validate_payload(
        CapabilityKind::DecisionEvaluate,
        "evaluate",
        Direction::Result,
        &result,
    )
    .map_err(|_| R::InvalidResult)?;
    // Choices outside the admissible options (nonexistent or disallowed) fail here.
    check_against_request(CapabilityKind::DecisionEvaluate, &env.payload, &result)
        .map_err(|_| R::RejectedChoice)?;
    let Some(choice) = result["choice"].as_str() else {
        return Err(R::Abstain);
    };
    if let Some(min) = p.profile.min_confidence {
        // Missing score fails closed; confidence is never a grant.
        if !result["scores"][choice].as_f64().is_some_and(|s| s >= min) {
            return Err(R::LowConfidence);
        }
    }
    if let Some(c) = cache {
        c.put(key, choice.to_string());
    }
    Ok(Consulted {
        choice: choice.to_string(),
        cached: false,
    })
}
