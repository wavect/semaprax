//! Policy-first routing: hard screen, rules or router consultation, post-call
//! revalidation and deterministic fallback. Frozen plans come out of here.
//! The router call itself (v1 or negotiated v2) lives in `consult`.

use super::cache::DecisionCache;
use super::call::{AbstentionReason, CallMetadata, ScoreKind};
use super::consult::consult;
use super::plan::FrozenRoutePlan;
use super::policy::{FallbackMode, RoutePolicy};
use super::provider::ConfiguredProvider;
use super::registry::DecisionTask;
use super::render::{PreparedRouteV2, V2Digests};
use super::route::{bad, screen, RouteRequest, Screening};
use super::rules::{rules_choice, RULES_CHECKPOINT, RULES_PROVIDER_ID};
use crate::contract::ProjectBinding;
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
            v2: None,
        }
    }

    /// The prepared `model-route/v2` request over the screened set.
    pub fn prepare_v2(&self, scr: &Screening) -> HarnessResult<PreparedRouteV2> {
        PreparedRouteV2::prepare(&self.request, &self.policy, &scr.admissible)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Digests {
    pub features: String,
    pub catalog: String,
    pub policy: String,
    pub candidates: String,
    /// MR-01: v2 feature/candidate/renderer/disclosure digests, present only
    /// when the decision was made over a `model-route/v2` request.
    pub v2: Option<V2Digests>,
}

impl Digests {
    pub fn to_json(&self) -> Value {
        let mut v = json!({"features": self.features, "catalog": self.catalog, "policy": self.policy, "candidates": self.candidates});
        if let Some(x) = &self.v2 {
            v["v2"] = x.to_json();
        }
        v
    }

    /// The routing task these digests were taken under.
    pub fn task(&self) -> &'static str {
        if self.v2.is_some() {
            DecisionTask::ModelRouteV2.id()
        } else {
            DecisionTask::ModelRoute.id()
        }
    }
}

/// Binds a chosen model to the exact digests it was chosen under.
pub fn choice_digest(d: &Digests, provider_id: &str, checkpoint: &str, choice: &str) -> String {
    json::digest(
        "semaprax.decision.choice.v1",
        &json!({"task": d.task(), "provider": provider_id, "checkpoint": checkpoint, "digests": d.to_json(), "choice": choice}),
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
    /// MR-15: the request exceeds the provider's declared capabilities or the
    /// v2 bounds; refused before inference.
    Unsupported,
    /// MR-03: the answering identity is not the qualified one (stale evidence).
    IdentityMismatch,
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

/// What went over the wire for one decision (MR-01/02/03). Default: no router
/// call was attempted (`version` 0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WireInfo {
    /// `decision.evaluate` version used: 0 none, 1 or 2.
    pub version: u32,
    /// Negotiation fallback, disclosure or refusal explanation.
    pub note: Option<String>,
    pub rendered_digest: Option<String>,
    pub max_wire_bytes: Option<u64>,
    pub score_kind: Option<ScoreKind>,
    /// Vendor confidence as labelled metadata; never a success probability.
    pub native_confidence: Option<(f64, String)>,
    pub abstention: Option<AbstentionReason>,
    /// Typed call identity/usage of the router call that answered.
    pub call: Option<CallMetadata>,
}

impl WireInfo {
    pub fn to_json(&self) -> Value {
        let scores_are = match self.score_kind {
            Some(ScoreKind::OptionDistribution) => {
                Some("option mass (not a probability of task success)")
            }
            Some(ScoreKind::CandidateRelative) => Some("candidate-relative scores (uncalibrated)"),
            Some(ScoreKind::None) => Some("no scores (scoreless provider)"),
            None => None,
        };
        json!({
            "version": self.version, "note": self.note, "rendered_digest": self.rendered_digest,
            "max_wire_bytes": self.max_wire_bytes,
            "score_kind": self.score_kind.map(|k| k.as_str()), "scores_are": scores_are,
            "native_confidence": self.native_confidence.as_ref().map(|(v, k)| json!({"value": v, "kind": k})),
            "abstention_reason": self.abstention.map(|a| a.as_str()),
            "call": self.call.as_ref().map(CallMetadata::to_json),
        })
    }
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
    pub wire: WireInfo,
}

pub(super) struct Consulted {
    pub choice: String,
    pub cached: bool,
}

#[allow(clippy::too_many_arguments)]
fn finish(
    inputs: &RouteInputs,
    scr: &Screening,
    d: Digests,
    lineage: &str,
    choice: &str,
    source: DecisionSource,
    pid: &str,
    ck: &str,
    calls: u32,
    ms: u64,
    status: &'static str,
    wire: WireInfo,
) -> HarnessResult<RouteDecision> {
    let plan = FrozenRoutePlan::freeze(
        lineage,
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
        wire,
    })
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
    // The enablement gate is per routing task: a v1 qualification never
    // enables a v2 route and vice versa.
    let task = match provider.as_ref() {
        Some(p) if p.wire_version() == 2 => DecisionTask::ModelRouteV2.id(),
        _ => DecisionTask::ModelRoute.id(),
    };
    let f = &inputs.request.features;
    let trivial =
        scr.admissible.len() == 1 || inputs.policy.rules_only_families.contains(&f.task_family);
    let mut digests = inputs.digests(&scr);
    let status = provider.as_ref().map_or("rules", |p| p.status(task));
    let lineage = ctx.lineage_id.as_str();

    let usable = match provider {
        Some(p) if !trivial && p.enabled(task) => Some(p),
        _ => None,
    };
    let Some(p) = usable else {
        let pick = rules_choice(f, &scr.admissible, &inputs.policy)
            .map(|p| p.id.clone())
            .expect("non-empty admissible set");
        let src = if trivial {
            DecisionSource::Trivial
        } else {
            DecisionSource::Rules
        };
        return finish(
            inputs,
            &scr,
            digests,
            lineage,
            &pick,
            src,
            RULES_PROVIDER_ID,
            RULES_CHECKPOINT,
            0,
            0,
            status,
            WireInfo::default(),
        );
    };

    // Negotiation: v2 only for an adapter that negotiated it; otherwise v1
    // with an explained fallback (MR-01).
    let mut wire = WireInfo {
        version: p.wire_version(),
        ..WireInfo::default()
    };
    let mut prepared = None;
    if wire.version == 2 {
        match inputs.prepare_v2(&scr) {
            Ok(pr) => {
                digests.v2 = Some(pr.digests());
                wire.rendered_digest = Some(pr.rendered.digest.clone());
                wire.max_wire_bytes = Some(pr.max_wire_bytes);
                wire.note = pr.disclosure_note.clone();
                prepared = Some(pr);
            }
            Err(e) => {
                wire.note = Some(format!(
                    "model-route/v2 refused before dispatch: {}",
                    e.message
                ));
                return fall_back(
                    inputs,
                    &scr,
                    digests,
                    FallbackReason::Unsupported,
                    0,
                    0,
                    status,
                    lineage,
                    wire,
                );
            }
        }
    } else {
        wire.note = Some(format!(
            "`{}` did not negotiate decision.evaluate v2; model-route/v1 sent without phase, progress or candidate descriptors",
            p.profile.provider_id
        ));
    }

    let (mut calls, mut ms) = (0u32, 0u64);
    let outcome = consult(
        inputs,
        ctx,
        p,
        &scr,
        &digests,
        prepared.as_ref(),
        &mut wire,
        &mut calls,
        &mut ms,
        cache,
    );
    let reason = match outcome {
        Ok(c) => {
            // Revalidate against live state: a valid choice is not a correct one.
            let cur = live();
            let cur_scr = screen(&cur.request, &cur.policy);
            let mut cur_d = cur.digests(&cur_scr);
            if prepared.is_some() {
                cur_d.v2 = cur.prepare_v2(&cur_scr).ok().map(|p| p.digests());
            }
            if cur_d.catalog != digests.catalog
                || cur_d.policy != digests.policy
                || cur_d.features != digests.features
                || cur_d.v2 != digests.v2
            {
                return fall_back(
                    &cur,
                    &cur_scr,
                    cur_d,
                    FallbackReason::Stale,
                    calls,
                    ms,
                    status,
                    lineage,
                    wire,
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
                    lineage,
                    wire,
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
                lineage,
                &c.choice,
                src,
                &p.profile.provider_id,
                &p.profile.checkpoint,
                calls,
                ms,
                status,
                wire,
            );
        }
        Err(r) => r,
    };
    fall_back(
        inputs, &scr, digests, reason, calls, ms, status, lineage, wire,
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
    wire: WireInfo,
) -> HarnessResult<RouteDecision> {
    if cur.policy.fallback == FallbackMode::Refuse {
        return Err(bad(
            "SPX-HPJ013",
            format!("router fallback refused ({reason:?}); policy fallback is `refuse`"),
        ));
    }
    let pick = rules_choice(&cur.request.features, &scr.admissible, &cur.policy)
        .ok_or_else(|| no_safe_model(scr))?
        .id
        .clone();
    finish(
        cur,
        scr,
        d,
        lineage,
        &pick,
        DecisionSource::Fallback(reason),
        RULES_PROVIDER_ID,
        RULES_CHECKPOINT,
        calls,
        ms,
        status,
        wire,
    )
}
