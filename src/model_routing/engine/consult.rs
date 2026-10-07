//! The router call: pre-call guards and capability preflight, the cache, one
//! `decision.evaluate` invocation (v1 or the prepared v2 request), structural
//! and semantic validation of the answer, thresholds and, for a qualified
//! (`Auto`) provider, verification of the answering identity.

use super::cache::{CacheKey, DecisionCache};
use super::call::{AbstentionReason, ResultV2, ScoreKind};
use super::json;
use super::provider::{ConfiguredProvider, DecisionCall, DecisionInvoker, ProviderMode};
use super::registry::DecisionTask;
use super::render::PreparedRouteV2;
use super::request::DecisionRequest;
use super::route::Screening;
use super::route_v2::Modality;
use super::router::{Consulted, Digests, FallbackReason, RouteContext, RouteInputs, WireInfo};
use super::wire::{self, check_against_request, Direction};
use serde_json::{json, Value};

/// Pre-call guards, the router call, and structural validation of its answer.
#[allow(clippy::too_many_arguments)]
pub(super) fn consult<I: ?Sized + DecisionInvoker>(
    inputs: &RouteInputs,
    ctx: &RouteContext,
    p: &mut ConfiguredProvider<'_, I>,
    scr: &Screening,
    d: &Digests,
    prepared: Option<&PreparedRouteV2>,
    wire: &mut WireInfo,
    calls: &mut u32,
    ms: &mut u64,
    cache: Option<&mut DecisionCache>,
) -> Result<Consulted, FallbackReason> {
    use FallbackReason as R;
    let pid = p.profile.provider_id.clone();
    // Invalid settings must not collide with JSON null in a retained cache scope.
    if [p.profile.min_confidence, p.profile.min_option_mass]
        .into_iter()
        .flatten()
        .any(|min| !min.is_finite() || !(0.0..=1.0).contains(&min))
    {
        return Err(R::LowConfidence);
    }
    if ctx.router_lineage.contains(&pid) {
        return Err(R::RecursionBlocked);
    }
    if !p.profile.covers(&inputs.request.features) {
        return Err(R::OutOfDistribution);
    }
    // MR-15: declared capabilities are checked before any inference.
    let preflight = match prepared {
        Some(pr) => p.profile.admits_request(
            pr.selection.len(),
            pr.rendered.state.len(),
            &pr.features.input_modalities,
        ),
        None => p
            .profile
            .admits_request(scr.admissible.len(), 0, &[Modality::Text].into()),
    };
    if let Err(why) = preflight {
        wire.note = Some(format!("provider capability preflight refused: {why}"));
        return Err(R::Unsupported);
    }
    let cap = inputs
        .request
        .budget
        .max_router_calls
        .min(inputs.policy.router_max_calls);
    let remaining = cap.saturating_sub(ctx.router_calls_used);
    // The router may never outlive the enclosing caller's remaining time
    // (`budget.max_latency_ms`) nor its own policy allowance.
    let latency_left = inputs
        .policy
        .router_max_latency_ms
        .min(inputs.request.budget.max_latency_ms)
        .saturating_sub(ctx.router_ms_used);
    let scope = match &d.v2 {
        Some(v2) => json::digest(
            "semaprax.decision.cache-scope.v2",
            &json!({"v2": v2.to_json(), "provider": p.profile.scope_json()}),
        ),
        None => p.profile.scope_digest(),
    };
    let key = CacheKey {
        provider_id: pid.clone(),
        model_id: p.profile.model_id.clone(),
        checkpoint: p.profile.checkpoint.clone(),
        features: d.features.clone(),
        catalog: d.catalog.clone(),
        policy: d.policy.clone(),
        schema: d.task().into(),
        scope,
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
    let (payload, version) = match prepared {
        Some(pr) => (pr.payload.clone(), 2),
        None => {
            let options: Vec<&str> = scr.admissible.iter().map(|m| m.id.as_str()).collect();
            (
                json!({"task": DecisionTask::ModelRoute.id(), "features": inputs.request.features.to_json(), "options": options}),
                1,
            )
        }
    };
    let mut lineage = ctx.router_lineage.clone();
    lineage.push(pid);
    let env = DecisionRequest {
        invocation_id: ctx.invocation_id.clone(),
        project: ctx.project.clone(),
        lock_digest: ctx.lock_digest.clone(),
        version,
        deadline_ms: latency_left.clamp(1, 600_000),
        max_result_bytes: 65_536,
        remaining_calls: remaining,
        lineage,
        payload,
    };
    env.validate().map_err(|_| R::InvalidResult)?;
    *calls += 1;
    let (result, elapsed, call) = match p.invoker.evaluate(&env) {
        DecisionCall::Unavailable => return Err(R::Unavailable),
        DecisionCall::Timeout => return Err(R::Timeout),
        DecisionCall::Answered {
            result,
            elapsed_ms,
            call,
        } => (result, elapsed_ms, call),
    };
    // Whatever the adapter reported is kept for accounting even when the
    // answer is then rejected.
    wire.call = call.clone();
    *ms += elapsed;
    if elapsed > latency_left {
        return Err(R::Timeout);
    }
    wire::validate(Direction::Result, &result).map_err(|_| R::InvalidResult)?;
    // Choices outside the admissible options (nonexistent or disallowed) fail here.
    check_against_request(&env.payload, &result).map_err(|e| {
        if e.code == "SPX-HPA043" {
            R::RejectedChoice
        } else {
            R::InvalidResult
        }
    })?;
    let choice = match prepared {
        None => v1_choice(p, &result)?,
        Some(pr) => v2_choice(p, pr, &result, call, wire)?,
    };
    if let Some(c) = cache {
        c.put(key, choice.clone());
    }
    Ok(Consulted {
        choice,
        cached: false,
    })
}

fn v1_choice<I: ?Sized>(
    p: &ConfiguredProvider<'_, I>,
    result: &Value,
) -> Result<String, FallbackReason> {
    let Some(choice) = result["choice"].as_str() else {
        return Err(FallbackReason::Abstain);
    };
    let score = result["scores"][choice].as_f64();
    for min in [p.profile.min_confidence, p.profile.min_option_mass]
        .into_iter()
        .flatten()
    {
        // v1 scores are an option distribution; a missing score fails closed.
        if !score.is_some_and(|s| s >= min) {
            return Err(FallbackReason::LowConfidence);
        }
    }
    Ok(choice.to_string())
}

fn v2_choice<I: ?Sized>(
    p: &ConfiguredProvider<'_, I>,
    pr: &PreparedRouteV2,
    result: &Value,
    call: Option<super::call::CallMetadata>,
    wire: &mut WireInfo,
) -> Result<String, FallbackReason> {
    use FallbackReason as R;
    let r = ResultV2::from_json(result).map_err(|_| R::InvalidResult)?;
    if call.as_ref().is_some_and(|c| *c != r.call) {
        return Err(R::InvalidResult);
    }
    wire.call = Some(r.call.clone());
    wire.score_kind = Some(r.score_kind);
    wire.native_confidence = r.native_confidence.zip(r.native_confidence_kind.clone());
    // A scoreless answer is valid only for a profile that declares it; the
    // host never synthesizes scores.
    if !p.profile.admits_score_kind(r.score_kind) {
        return Err(R::InvalidResult);
    }
    // Native abstention is authoritative.
    let Some(sel) = r.choice.as_deref() else {
        wire.abstention = Some(r.abstention_reason);
        return Err(R::Abstain);
    };
    let opaque = pr.opaque(sel).ok_or(R::RejectedChoice)?.to_string();
    let chosen = r.scores.as_ref().and_then(|s| s.get(sel).copied());
    if let Some(min) = p.profile.min_option_mass {
        if r.score_kind == ScoreKind::OptionDistribution && !chosen.is_some_and(|x| x >= min) {
            wire.abstention = Some(AbstentionReason::HostThreshold);
            return Err(R::LowConfidence);
        }
    }
    if let Some(min) = p.profile.min_confidence {
        // Deprecated alias: chosen score of any kind; missing fails closed.
        if !chosen.is_some_and(|x| x >= min) {
            wire.abstention = Some(AbstentionReason::HostThreshold);
            return Err(R::LowConfidence);
        }
    }
    // A qualified route is accepted only from the qualified identity.
    if p.mode == ProviderMode::Auto {
        if let Err(why) = p.profile.verify_identity(&r.call) {
            wire.note = Some(format!("qualified route not used: {why}"));
            return Err(R::IdentityMismatch);
        }
    }
    Ok(opaque)
}
