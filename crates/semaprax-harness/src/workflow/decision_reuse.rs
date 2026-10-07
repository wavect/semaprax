//! MR-12: the session-owned decision cache, router readiness and the reuse
//! report around the one routing engine (`governed_decide`).
//!
//! One [`SessionDecisions`] is owned by the configured host/session (the
//! workflow keeps it in `RoutingWiring`). It holds bounded `DecisionCache`s,
//! one per authority scope: tenant/project binding, lock digest, effective
//! governance (mode, pins, remote prohibition, bypass and shadow knobs) and the
//! evidence/calibration version the session locked. Inside a scope the engine's
//! own `CacheKey` binds provider/model/checkpoint, feature, catalog and
//! policy/budget digests, the routing task and the v2 feature/candidate/
//! renderer/disclosure digests plus the adapter/profile/instance identity, so a
//! changed failure, phase, policy, candidate descriptor, credential scope or
//! model identity is a miss. Only fully validated answers are cached (the
//! engine never stores a timeout, unavailable, invalid or abstaining call), and
//! every hit is revalidated by `decide` against the live admissible set.
//!
//! Readiness is reused, never installed: a provider is `cold` until it answers
//! once in this session (`warm`); an unavailable or timed-out worker is
//! `unavailable` for the rest of the session, and a router whose observed
//! latency already exceeds the remaining router deadline is bypassed. Both
//! bypasses take the existing fallback policy (rules, or a refusal under
//! `fallback = refuse`) without a call.

use crate::contract::RequestEnvelope;
use crate::decision::{
    governed_decide, ConfiguredProvider, DecisionCache, DecisionCall, DecisionInvoker,
    FallbackMode, GateStatus, GovernedRoute, Governor, RouteContext, RouteInputs, RoutingMode,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

/// Entries per authority scope.
pub const DEFAULT_CACHE_CAP: usize = 64;
/// Authority scopes one session keeps before the oldest is dropped.
pub const MAX_SCOPES: usize = 16;

/// Router worker readiness within one session (never an installation state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// Not yet consulted in this session.
    Cold,
    /// Answered at least once in this session.
    Warm,
    /// Unavailable or timed out in this session: later routes fall back without a call.
    Unavailable,
}

impl Readiness {
    pub fn as_str(self) -> &'static str {
        match self {
            Readiness::Cold => "cold",
            Readiness::Warm => "warm",
            Readiness::Unavailable => "unavailable",
        }
    }
}

/// What one decision reused or spent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reuse {
    /// `hit`, `miss`, `bypass` or `none` (no learned provider involved).
    pub outcome: &'static str,
    pub reason: Option<String>,
    pub readiness_before: Readiness,
    pub readiness_after: Readiness,
    /// Router invocations actually sent for this decision.
    pub router_calls: u32,
    /// Wall time around the invocations minus the reported inference time.
    pub queue_ms: u64,
    /// Inference time the invoker reported.
    pub inference_ms: u64,
}

impl Reuse {
    pub fn to_json(&self) -> Value {
        json!({"cache": self.outcome, "reason": self.reason,
               "readiness": {"before": self.readiness_before.as_str(), "after": self.readiness_after.as_str()},
               "router_calls": self.router_calls, "new_api_usage": self.router_calls > 0,
               "latency": {"queue_ms": self.queue_ms, "inference_ms": self.inference_ms}})
    }
}

/// Session totals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReuseStats {
    pub decisions: u64,
    pub hits: u64,
    pub misses: u64,
    pub bypasses: u64,
    pub router_calls: u64,
}

impl ReuseStats {
    pub fn to_json(&self) -> Value {
        json!({"decisions": self.decisions, "hits": self.hits, "misses": self.misses,
               "bypasses": self.bypasses, "router_calls": self.router_calls})
    }

    /// Totals since `earlier` (a host-owned cache outlives one run).
    pub fn since(&self, earlier: &ReuseStats) -> ReuseStats {
        ReuseStats {
            decisions: self.decisions - earlier.decisions,
            hits: self.hits - earlier.hits,
            misses: self.misses - earlier.misses,
            bypasses: self.bypasses - earlier.bypasses,
            router_calls: self.router_calls - earlier.router_calls,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Worker {
    readiness: Option<Readiness>,
    /// Latency of the latest answered call (max of wall and reported).
    observed_ms: Option<u64>,
}

/// The explicitly owned, bounded decision cache and readiness of one session.
#[derive(Debug)]
pub struct SessionDecisions {
    cap: usize,
    scopes: BTreeMap<String, DecisionCache>,
    order: VecDeque<String>,
    workers: BTreeMap<String, Worker>,
    stats: ReuseStats,
}

impl Default for SessionDecisions {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_CAP)
    }
}

/// Counts invocations and splits queue from inference time.
struct Metered<'a> {
    inner: &'a mut dyn DecisionInvoker,
    calls: u32,
    wall_ms: u64,
    inference_ms: u64,
    last: Option<Readiness>,
}

impl DecisionInvoker for Metered<'_> {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall {
        let t = Instant::now();
        let out = self.inner.evaluate(request);
        self.calls += 1;
        self.wall_ms += t.elapsed().as_millis() as u64;
        self.last = Some(match &out {
            DecisionCall::Answered { elapsed_ms, .. } => {
                self.inference_ms += elapsed_ms;
                Readiness::Warm
            }
            DecisionCall::Unavailable | DecisionCall::Timeout => Readiness::Unavailable,
        });
        out
    }

    fn decision_versions(&self) -> Vec<u32> {
        self.inner.decision_versions()
    }
}

fn worker_key(p: &ConfiguredProvider<'_>) -> String {
    format!("{}|{}", p.profile.provider_id, p.profile.scope_digest())
}

impl SessionDecisions {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            scopes: BTreeMap::new(),
            order: VecDeque::new(),
            workers: BTreeMap::new(),
            stats: ReuseStats::default(),
        }
    }

    pub fn stats(&self) -> &ReuseStats {
        &self.stats
    }

    /// Cached decisions across all scopes.
    pub fn len(&self) -> usize {
        self.scopes.values().map(DecisionCache::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn readiness(&self, p: &ConfiguredProvider<'_>) -> Readiness {
        self.workers
            .get(&worker_key(p))
            .and_then(|w| w.readiness)
            .unwrap_or(Readiness::Cold)
    }

    /// The authority scope of one decision: tenant/project binding, lock,
    /// effective governance and the evidence/calibration version in force.
    pub fn scope(ctx: &RouteContext, g: &Governor, p: Option<&ConfiguredProvider<'_>>) -> String {
        let gate = p.map(|p| match &p.gate.status {
            GateStatus::Passed { evidence } => json!({"task": p.gate.task, "passed": evidence}),
            GateStatus::NotEvaluated => json!({"task": p.gate.task, "status": "not-evaluated"}),
            GateStatus::Failed => json!({"task": p.gate.task, "status": "failed"}),
        });
        json::digest(
            "semaprax.harness-decision-reuse-scope.v1",
            &json!({
                "tenant": {"project": ctx.project.id, "worktree": ctx.project.worktree, "lock": ctx.lock_digest},
                "governance": {"mode": g.cfg.mode.as_str(), "pin": g.cfg.project_pin, "allow_remote": g.cfg.user_allow_remote,
                               "cheap_bypass_micros": g.cfg.cheap_bypass_micros, "shadow_max_calls": g.cfg.shadow_max_calls,
                               "provider_mode": p.map(|p| format!("{:?}", p.mode))},
                "provider_acceptance": p.map(|p| p.profile.scope_json()),
                "evidence": {"lock": g.lock.map(|l| json!({"key": l.key_digest, "record": l.record_digest})),
                             "registry": g.registry.is_some(), "gate": gate},
            }),
        )
    }

    fn cache_for(&mut self, scope: String) -> &mut DecisionCache {
        if !self.scopes.contains_key(&scope) {
            self.order.push_back(scope.clone());
            while self.order.len() > MAX_SCOPES {
                if let Some(old) = self.order.pop_front() {
                    self.scopes.remove(&old);
                }
            }
        }
        let cap = self.cap;
        self.scopes
            .entry(scope)
            .or_insert_with(|| DecisionCache::new(cap))
    }

    /// Why a learned provider is skipped before any call, when it is.
    fn bypass(
        &self,
        g: &Governor,
        inputs: &RouteInputs,
        ctx: &RouteContext,
        p: &ConfiguredProvider<'_>,
    ) -> Option<String> {
        let pinned = g.cfg.project_pin.is_some() || matches!(g.cfg.mode, RoutingMode::Pin(_));
        if pinned || g.cfg.mode == RoutingMode::Rules {
            return None;
        }
        let w = self.workers.get(&worker_key(p))?;
        if w.readiness == Some(Readiness::Unavailable) {
            return Some(
                "router worker was unavailable earlier in this session; fallback without a call"
                    .into(),
            );
        }
        let left = inputs
            .policy
            .router_max_latency_ms
            .saturating_sub(ctx.router_ms_used);
        match w.observed_ms {
            Some(ms) if ms > left => Some(format!(
                "router predicted to miss the deadline: last observed {ms} ms > {left} ms left; fallback without a call"
            )),
            _ => None,
        }
    }

    /// `governed_decide` with this session's cache, readiness and report.
    pub fn decide(
        &mut self,
        g: &Governor,
        inputs: &RouteInputs,
        ctx: &RouteContext,
        provider: Option<&mut ConfiguredProvider<'_>>,
        live: &dyn Fn() -> RouteInputs,
    ) -> HarnessResult<(GovernedRoute, Reuse)> {
        self.stats.decisions += 1;
        let Some(p) = provider else {
            let gr = governed_decide(g, inputs, ctx, None, live, None)?;
            return Ok((gr, none_reuse(Readiness::Cold)));
        };
        let key = worker_key(p);
        let before = self.readiness(p);
        if let Some(why) = self.bypass(g, inputs, ctx, p) {
            if inputs.policy.fallback == FallbackMode::Refuse {
                return Err(HarnessDiagnostic::new(
                    "SPX-HPJ013",
                    format!("router bypassed ({why}); policy fallback is `refuse`"),
                ));
            }
            self.stats.bypasses += 1;
            let mut gr = governed_decide(g, inputs, ctx, None, live, None)?;
            gr.rules_reason = Some(why.clone());
            gr.explanation["rules_reason"] = json!(why);
            let mut r = none_reuse(before);
            r.outcome = "bypass";
            r.reason = Some(why);
            return Ok((gr, r));
        }
        let scope = Self::scope(ctx, g, Some(p));
        let cache = self.cache_for(scope.clone());
        let hits_before = cache.hits();
        let mut m = Metered {
            inner: &mut *p.invoker,
            calls: 0,
            wall_ms: 0,
            inference_ms: 0,
            last: None,
        };
        let mut wrapped = ConfiguredProvider {
            profile: p.profile.clone(),
            invoker: &mut m,
            mode: p.mode,
            gate: p.gate.clone(),
        };
        let out = governed_decide(g, inputs, ctx, Some(&mut wrapped), live, Some(cache));
        let (mode, gate) = (wrapped.mode, wrapped.gate.clone());
        drop(wrapped);
        p.mode = mode;
        p.gate = gate;
        let hit =
            self.scopes.get(&scope).map_or(0, DecisionCache::hits) > hits_before && m.calls == 0;
        let w = self.workers.entry(key).or_default();
        if let Some(r) = m.last {
            w.readiness = Some(r);
            if r == Readiness::Warm {
                w.observed_ms = Some(m.wall_ms.max(m.inference_ms));
            }
        }
        let after = w.readiness.unwrap_or(before);
        self.stats.router_calls += u64::from(m.calls);
        let gr = out?;
        let outcome = if hit {
            self.stats.hits += 1;
            "hit"
        } else if m.calls > 0 {
            self.stats.misses += 1;
            "miss"
        } else {
            "none"
        };
        let reuse = Reuse {
            outcome,
            reason: (outcome == "none")
                .then(|| gr.rules_reason.clone())
                .flatten(),
            readiness_before: before,
            readiness_after: after,
            router_calls: m.calls,
            queue_ms: m.wall_ms.saturating_sub(m.inference_ms),
            inference_ms: m.inference_ms,
        };
        Ok((gr, reuse))
    }
}

fn none_reuse(r: Readiness) -> Reuse {
    Reuse {
        outcome: "none",
        reason: None,
        readiness_before: r,
        readiness_after: r,
        router_calls: 0,
        queue_ms: 0,
        inference_ms: 0,
    }
}
