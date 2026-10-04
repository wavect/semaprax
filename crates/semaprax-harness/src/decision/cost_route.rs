//! TC-10: opt-in routing by qualified total task cost and a bounded
//! escalation ladder. Pure and deterministic: no model call, no clock, no I/O,
//! no training. The existing rules routing stays the default; this module only
//! (a) estimates one attempt from the fitted prompt, (b) compares complete-task
//! strategies from already-registered evidence, and (c) classifies what a known
//! terminal failure may trigger next.

use super::evidence::{EvidenceRecord, Origin, Outcome};
use super::route::{Destination, ModelPlan, TaskFeatures};
use crate::receipt::{PriceBook, Pricing};
use crate::workflow::spend::{call_bound, Billing};
use serde_json::{json, Value};

/// What is known about prompt caching before dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheState {
    /// Nothing confirmed: every input token is priced at the dearest input category.
    Conservative,
    /// A cache miss that writes `write_tokens` of the input to the cache.
    Miss { write_tokens: u64 },
    /// A cache read of `read_tokens` confirmed by the provider. The read tokens
    /// are cheaper but still occupy context: capacity is never discounted.
    ConfirmedRead { read_tokens: u64 },
}

/// Estimate of one attempt. API billing and local runtime latency stay apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptEstimate {
    /// Billed upper bound; `None` is unknown and never a zero-cost winner.
    pub billed_micros: Option<u64>,
    pub billing: Billing,
    pub basis: &'static str,
    /// Context tokens the request occupies (cache state does not change this).
    pub context_tokens: u64,
    /// Catalog latency, reported beside (never summed into) billing.
    pub latency_ms: u64,
    pub local: bool,
}

impl AttemptEstimate {
    pub fn to_json(&self) -> Value {
        json!({"billed_micros": self.billed_micros, "basis": self.basis,
               "context_tokens": self.context_tokens, "latency_ms": self.latency_ms,
               "local": self.local})
    }
}

/// Bound or estimate of one attempt for `plan` (reuses TC-03 `call_bound` for
/// the conservative state; a cache state only lowers it when its category is priced).
pub fn attempt_estimate(
    prices: &PriceBook,
    plan: &ModelPlan,
    input_tokens: u64,
    output_cap: u64,
    cache: CacheState,
) -> AttemptEstimate {
    let bound = call_bound(prices, &plan.id, input_tokens, output_cap, 1);
    let mut e = AttemptEstimate {
        billed_micros: bound.micros,
        billing: bound.billing,
        basis: "conservative_bound",
        context_tokens: input_tokens,
        latency_ms: plan.est_latency_ms,
        local: plan.destination == Destination::Local,
    };
    let Some(Pricing::Rates {
        input: Some(input),
        cache_read,
        cache_write,
        output: Some(output),
        ..
    }) = prices.record_for(&plan.id).map(|r| &r.pricing)
    else {
        return e;
    };
    let (read, write, cache_rate, basis) = match (cache, cache_read, cache_write) {
        (CacheState::ConfirmedRead { read_tokens }, Some(r), _) => {
            (read_tokens.min(input_tokens), 0, *r, "confirmed_cache_read")
        }
        (CacheState::Miss { write_tokens }, _, Some(w)) => {
            (0, write_tokens.min(input_tokens), *w, "cache_write_miss")
        }
        (CacheState::Conservative, ..) => return e,
        // An unpriced cache category is never assumed cheaper than the bound.
        _ => {
            e.basis = "cache_category_unpriced";
            return e;
        }
    };
    let plain = input_tokens - read - write;
    let total = plain as u128 * *input as u128
        + (read + write) as u128 * cache_rate as u128
        + output_cap as u128 * *output as u128;
    if let (Ok(n), Some(cap)) = (u64::try_from(total.div_ceil(1_000_000)), e.billed_micros) {
        // A confirmed split never reports more than the conservative bound.
        e.billed_micros = Some(n.min(cap));
        e.basis = basis;
    }
    e
}

/// Completed-task statistics of one strategy (model) from real, verified evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrategyStats {
    pub model: String,
    pub tasks: u32,
    pub accepted: u32,
    /// Accepted on the first attempt.
    pub first_try: u32,
    /// Accepted after at least one failed attempt.
    pub recovered: u32,
    /// Total known billed cost of every trial, failures included; `None` when
    /// any trial's price is unknown (incomplete pricing).
    pub total_cost_micros: Option<u64>,
}

impl StrategyStats {
    pub fn from_outcomes(model: &str, outcomes: &[Outcome]) -> Self {
        let mine: Vec<&Outcome> = outcomes
            .iter()
            .filter(|o| o.model == model && o.origin == Origin::Real)
            .collect();
        let total = mine.iter().try_fold(0u64, |t, o| {
            o.cost_micros?;
            t.checked_add(o.total_cost(0))
        });
        let ok = |o: &&&Outcome| o.completed && !o.verified_by.is_empty() && o.regressions == 0;
        Self {
            model: model.into(),
            tasks: mine.len() as u32,
            accepted: mine.iter().filter(ok).count() as u32,
            first_try: mine.iter().filter(ok).filter(|o| o.attempts <= 1).count() as u32,
            recovered: mine.iter().filter(ok).filter(|o| o.attempts > 1).count() as u32,
            total_cost_micros: total,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({"model": self.model, "tasks": self.tasks, "accepted": self.accepted,
               "first_try": self.first_try, "recovered": self.recovered,
               "total_cost_micros": self.total_cost_micros})
    }

    /// Why this strategy cannot be compared, else `None`.
    pub fn disqualified(&self, min_tasks: u32) -> Option<&'static str> {
        if self.tasks < min_tasks {
            Some("insufficient evidence")
        } else if self.accepted == 0 {
            Some("no accepted tasks")
        } else if self.total_cost_micros.is_none() {
            Some("incomplete pricing in evidence")
        } else {
            None
        }
    }
}

/// Total cost per accepted task, compared exactly by cross-multiplication.
fn cheaper(a: &StrategyStats, b: &StrategyStats) -> std::cmp::Ordering {
    let (ca, cb) = (
        a.total_cost_micros.unwrap_or(u64::MAX) as u128,
        b.total_cost_micros.unwrap_or(u64::MAX) as u128,
    );
    (ca * b.accepted as u128).cmp(&(cb * a.accepted as u128))
}

pub const DEFAULT_MIN_TASKS: u32 = 5;

pub struct ChooseInputs<'a> {
    /// Approved ladder (weakest first) of the task family.
    pub ladder: &'a [String],
    /// Models the hard policy screen left admissible.
    pub pool: &'a [ModelPlan],
    pub features: &'a TaskFeatures,
    pub input_tokens: u64,
    pub output_cap: u64,
    pub prices: &'a PriceBook,
    pub cache: CacheState,
    /// Remaining task cost; `None` is unbounded.
    pub allowance_micros: Option<u64>,
    /// Evidence registered for the LIVE key (`None`: absent or stale after drift).
    pub evidence: Option<&'a EvidenceRecord>,
    pub min_tasks: u32,
    /// An explicit pin is honoured exactly: cost routing steps aside.
    pub pinned: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// The qualified winner; `None` keeps the existing rules policy.
    pub model: Option<String>,
    /// Ladder models that passed every hard filter (the conservative pool).
    pub eligible: Vec<String>,
    pub reason: String,
    pub excluded: Vec<Value>,
    pub estimate: Option<AttemptEstimate>,
    pub stats: Vec<StrategyStats>,
}

impl Choice {
    pub fn to_json(&self) -> Value {
        json!({"model": self.model, "eligible": self.eligible, "reason": self.reason,
               "excluded": self.excluded, "attempt_estimate": self.estimate.as_ref().map(AttemptEstimate::to_json),
               "strategies": self.stats.iter().map(StrategyStats::to_json).collect::<Vec<_>>()})
    }
}

/// Hard filters (admissibility, capability, context, known price, allowance)
/// for a ladder model; `Err` carries the reason.
pub fn hard_filter(name: &str, c: &ChooseInputs) -> Result<AttemptEstimate, String> {
    let plan = c
        .pool
        .iter()
        .find(|p| p.id == name)
        .ok_or("not admissible under policy or catalog")?;
    if c.features.requires_structured_output && !plan.structured_output {
        return Err("lacks structured output".into());
    }
    if c.features.requires_tools && !plan.tools {
        return Err("lacks tools".into());
    }
    if plan.max_context < c.input_tokens {
        return Err("context capacity too small".into());
    }
    let e = attempt_estimate(c.prices, plan, c.input_tokens, c.output_cap, c.cache);
    let Some(b) = e.billed_micros else {
        return Err("unknown price".into());
    };
    if c.allowance_micros.is_some_and(|a| b > a) {
        return Err("attempt bound exceeds the remaining task cost".into());
    }
    Ok(e)
}

/// Choose the starting rung by qualified total cost per accepted task.
pub fn choose_start(c: &ChooseInputs) -> Choice {
    let mut out = Choice {
        model: None,
        eligible: vec![],
        reason: String::new(),
        excluded: vec![],
        estimate: None,
        stats: vec![],
    };
    if c.pinned {
        out.reason = "explicit pin honoured; cost routing steps aside".into();
        return out;
    }
    let mut ranked: Vec<(StrategyStats, AttemptEstimate)> = vec![];
    for name in c.ladder {
        let est = match hard_filter(name, c) {
            Ok(x) => x,
            Err(why) => {
                out.excluded.push(json!({"model": name, "reason": why}));
                continue;
            }
        };
        out.eligible.push(name.clone());
        let Some(rec) = c.evidence else {
            out.excluded
                .push(json!({"model": name, "reason": "no fresh evidence for the live key"}));
            continue;
        };
        let st = StrategyStats::from_outcomes(name, &rec.outcomes);
        out.stats.push(st.clone());
        if let Some(why) = st.disqualified(c.min_tasks) {
            out.excluded.push(json!({"model": name, "reason": why}));
            continue;
        }
        ranked.push((st, est));
    }
    // Stable sort: ties keep ladder order.
    ranked.sort_by(|a, b| cheaper(&a.0, &b.0));
    match ranked.into_iter().next() {
        Some((st, est)) => {
            out.reason = format!(
                "lowest total cost per accepted task ({} micros over {} accepted of {} tasks)",
                st.total_cost_micros.unwrap_or(0),
                st.accepted,
                st.tasks
            );
            out.model = Some(st.model);
            out.estimate = Some(est);
        }
        None => {
            out.reason = "insufficient qualified evidence; existing rules policy decides".into();
        }
    }
    out
}

/// The next ladder rung above `current` that passes every hard filter.
pub fn next_rung(c: &ChooseInputs, current: &str) -> Result<(String, AttemptEstimate), String> {
    let at = c
        .ladder
        .iter()
        .position(|m| m == current)
        .ok_or("current model is not on the ladder")?;
    let mut why = "no stronger rung".to_string();
    for name in &c.ladder[at + 1..] {
        match hard_filter(name, c) {
            Ok(e) => return Ok((name.clone(), e)),
            Err(w) => why = format!("`{name}`: {w}"),
        }
    }
    Err(why)
}

/// A known terminal failure, classified from the failure record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureClass {
    /// Transport or dispatch outcome unknown: nothing may be retried on a paid route.
    Uncertain,
    /// The provider or host refused authority.
    Authorization,
    /// A deterministic toolchain or host dependency is missing.
    MissingDependency(String),
    /// A named context fact was missing from the request.
    MissingContext(String),
    /// The reply was cut off by the output cap.
    Truncated,
    /// The candidate was rejected by a deterministic validator.
    Rejected,
}

pub fn classify_failure(stage: &str, code: &str, message: &str) -> FailureClass {
    let m = message.to_ascii_lowercase();
    let first: String = message
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect();
    if code == "SPX-HPD072" || m.starts_with("uncertain") || m.contains("outcome unknown") {
        FailureClass::Uncertain
    } else if [
        "not authorized",
        "unauthorized",
        "permission denied",
        "forbidden",
        "refused by policy",
    ]
    .iter()
    .any(|k| m.contains(k))
    {
        FailureClass::Authorization
    } else if [
        "toolchain",
        "command not found",
        "no such file or directory",
    ]
    .iter()
    .any(|k| m.contains(k))
    {
        FailureClass::MissingDependency(first)
    } else if m.contains("length-limited") || m.contains("truncat") {
        FailureClass::Truncated
    } else if matches!(stage, "preview" | "checks" | "check" | "patch")
        && ["unresolved", "unknown name", "undefined"]
            .iter()
            .any(|k| m.contains(k))
    {
        FailureClass::MissingContext(first)
    } else {
        FailureClass::Rejected
    }
}

/// What a known failure may trigger next. Context expansion and a larger cap
/// change the request; an escalation changes the model. They stay distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NextAction {
    /// Focused context expansion (TC-05 path); the model is unchanged.
    ExpandContext,
    /// Larger output cap (TC-02 bounded retry); the model is unchanged.
    LargerOutputCap,
    /// One move to the next approved rung.
    Escalate,
    /// Same model: a relevant input (the failure feedback) changed.
    RetryChangedInput,
    Stop(String),
}

impl NextAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExpandContext => "expand_context",
            Self::LargerOutputCap => "larger_output_cap",
            Self::Escalate => "escalate",
            Self::RetryChangedInput => "retry_changed_input",
            Self::Stop(_) => "stop",
        }
    }
}

pub struct ActionContext {
    pub escalations_used: u32,
    pub max_escalations: u32,
    pub pinned: bool,
    pub can_expand_context: bool,
    pub can_grow_cap: bool,
    /// A next rung exists, passed the hard filters and fits the remaining spend.
    pub next_rung_ok: bool,
    /// Evidence justifies reusing the model although nothing relevant changed.
    pub evidence_justifies_retry: bool,
    pub input_changed: bool,
}

pub fn next_action(class: &FailureClass, a: &ActionContext) -> NextAction {
    match class {
        FailureClass::Uncertain => {
            NextAction::Stop("uncertain request: no automatic paid retry".into())
        }
        FailureClass::Authorization => NextAction::Stop("authorization refusal".into()),
        FailureClass::MissingDependency(n) => {
            NextAction::Stop(format!("missing deterministic dependency: {n}"))
        }
        FailureClass::Truncated if a.can_grow_cap => NextAction::LargerOutputCap,
        FailureClass::MissingContext(_) if a.can_expand_context => NextAction::ExpandContext,
        _ if !a.pinned && a.escalations_used < a.max_escalations && a.next_rung_ok => {
            NextAction::Escalate
        }
        _ if a.input_changed || a.evidence_justifies_retry => NextAction::RetryChangedInput,
        _ => NextAction::Stop("no relevant input changed and no rung remains".into()),
    }
}

/// A paid router is consulted only when its known expected benefit exceeds its
/// known cost; an unknown benefit or cost never pays.
pub fn router_pays(expected_benefit_micros: Option<u64>, router_cost_micros: Option<u64>) -> bool {
    matches!((expected_benefit_micros, router_cost_micros), (Some(b), Some(c)) if b > c)
}

#[cfg(test)]
mod tests;
