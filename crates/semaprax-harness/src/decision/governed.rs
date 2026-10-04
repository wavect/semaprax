//! Governed routing (HN-16): the four modes around the one routing engine,
//! `decide`. Pins, remote prohibition, evidence-keyed enablement, shadow
//! recommendations, budget headroom and rollback live here; there is no second
//! engine, and every learned answer is still revalidated by `decide`.

use super::cache::DecisionCache;
use super::evidence::{EvidenceKey, EvidenceRegistry};
use super::provider::{ConfiguredProvider, GateStatus, ProviderMode};
use super::qualify::{gate_for, GateSpec};
use super::route::{screen, Destination, ModelPlan, RouteRequest};
use super::router::{decide, RouteContext, RouteDecision, RouteInputs};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutingMode {
    /// Policy rules only (the default).
    Rules,
    /// The user pinned one model.
    Pin(String),
    /// A learned provider chosen by the user, visibly experimental.
    Experimental,
    /// A learned provider only while its live evidence key is qualified.
    QualifiedAuto,
}

impl RoutingMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoutingMode::Rules => "rules",
            RoutingMode::Pin(_) => "pin",
            RoutingMode::Experimental => "experimental",
            RoutingMode::QualifiedAuto => "qualified-auto",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingConfig {
    pub mode: RoutingMode,
    /// Project pin; wins over every mode and over a user pin.
    pub project_pin: Option<String>,
    /// User prohibition on remote routing, honored in every mode.
    pub user_allow_remote: bool,
    /// Skip the router when every admissible model costs at most this.
    pub cheap_bypass_micros: Option<u64>,
    /// Router calls a shadow recommendation may spend (0 disables shadows).
    pub shadow_max_calls: u32,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            mode: RoutingMode::Rules,
            project_pin: None,
            user_allow_remote: true,
            cheap_bypass_micros: None,
            shadow_max_calls: 0,
        }
    }
}

/// The qualified profile a session started under; later updates never change it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionLock {
    pub key_digest: String,
    pub record_digest: String,
}

/// Active and previous qualified profiles. A rollback restores the previous
/// one for new sessions; existing `SessionLock`s are owned copies.
#[derive(Default, Debug)]
pub struct ProfileStore {
    active: Option<SessionLock>,
    previous: Option<SessionLock>,
}

impl ProfileStore {
    pub fn install(&mut self, p: SessionLock) {
        self.previous = self.active.take();
        self.active = Some(p);
    }

    pub fn active(&self) -> Option<&SessionLock> {
        self.active.as_ref()
    }

    pub fn lock_session(&self) -> Option<SessionLock> {
        self.active.clone()
    }

    /// Restore the previous qualified profile (or rules when none).
    pub fn rollback(&mut self) {
        self.active = self.previous.take();
    }
}

/// Live verified outcomes of the active learned profile against its
/// qualification baseline.
#[derive(Clone, Debug)]
pub struct DriftMonitor {
    pub baseline_completion: f64,
    pub tolerance: f64,
    pub min_samples: usize,
    window: Vec<bool>,
}

impl DriftMonitor {
    pub fn new(baseline_completion: f64, tolerance: f64, min_samples: usize) -> Self {
        Self {
            baseline_completion,
            tolerance,
            min_samples,
            window: vec![],
        }
    }

    pub fn record(&mut self, completed: bool) {
        self.window.push(completed);
    }

    pub fn drifted(&self) -> bool {
        self.window.len() >= self.min_samples
            && (self.window.iter().filter(|c| **c).count() as f64 / self.window.len() as f64)
                + self.tolerance
                < self.baseline_completion
    }

    /// Roll the store back when drift is detected.
    pub fn enforce(&self, store: &mut ProfileStore) -> bool {
        let d = self.drifted();
        if d {
            store.rollback();
        }
        d
    }
}

pub struct Governor<'a> {
    pub cfg: &'a RoutingConfig,
    pub registry: Option<&'a EvidenceRegistry>,
    pub spec: &'a GateSpec,
    pub lock: Option<&'a SessionLock>,
    /// Tokens left in the task ledger for router spend; `None` is unbounded.
    pub router_headroom_tokens: Option<u64>,
    /// Tokens one router request would reserve (request plus output reserve).
    pub router_request_tokens: u64,
}

#[derive(Clone, Debug)]
pub struct GovernedRoute {
    pub decision: RouteDecision,
    pub mode: &'static str,
    /// Why a learned provider was not consulted, when one was configured.
    pub rules_reason: Option<String>,
    pub shadow: Option<Value>,
    /// Router calls the caller must charge to the task ledger (decision plus shadow).
    pub router_calls_total: u32,
    pub explanation: Value,
}

fn without_remote(
    i: &RouteInputs,
    allow_remote: bool,
    only: Option<&str>,
) -> HarnessResult<RouteInputs> {
    let cat: Vec<ModelPlan> = i
        .request
        .catalog
        .iter()
        .filter(|p| allow_remote || p.destination == Destination::Local)
        .filter(|p| only.is_none_or(|o| p.id == o))
        .cloned()
        .collect();
    Ok(RouteInputs {
        request: RouteRequest::new(i.request.features.clone(), cat, i.request.budget.clone())?,
        policy: i.policy.clone(),
    })
}

/// Recheck the chosen model against the final serialized request before
/// dispatch: privacy, structured output, tools and final prompt capacity.
pub fn recheck_dispatch(
    i: &RouteInputs,
    choice: &str,
    final_required_tokens: u64,
) -> HarnessResult<()> {
    let plan = i.request.catalog.iter().find(|p| p.id == choice);
    let Some(plan) = plan else {
        return Err(HarnessDiagnostic::new(
            "SPX-HPJ018",
            format!("`{choice}` is not in the approved set"),
        ));
    };
    let mut f = i.request.features.clone();
    f.estimated_context_tokens = final_required_tokens;
    let req = RouteRequest::new(f, vec![plan.clone()], i.request.budget.clone())?;
    let s = screen(&req, &i.policy);
    match s.excluded.first() {
        Some((_, why)) => Err(HarnessDiagnostic::new(
            "SPX-HPJ018",
            format!("`{choice}` failed the pre-dispatch recheck: {why}"),
        )),
        None => Ok(()),
    }
}

pub fn governed_decide(
    g: &Governor,
    inputs: &RouteInputs,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_>>,
    live: &dyn Fn() -> RouteInputs,
    cache: Option<&mut DecisionCache>,
) -> HarnessResult<GovernedRoute> {
    let allow = g.cfg.user_allow_remote;
    let inputs = without_remote(inputs, allow, None)?;
    let live_f = |only: Option<&str>| {
        let (cur, only) = (live(), only.map(str::to_string));
        without_remote(&cur, allow, only.as_deref()).unwrap_or(cur)
    };
    let pin = g.cfg.project_pin.clone().or(match &g.cfg.mode {
        RoutingMode::Pin(m) => Some(m.clone()),
        _ => None,
    });
    let finish = |decision: RouteDecision,
                  mode,
                  why: Option<String>,
                  shadow: Option<Value>,
                  extra: u32,
                  ev: Value| {
        let explanation = json!({"mode": mode, "choice": decision.choice, "source": format!("{:?}", decision.source),
            "provider": decision.provider_id, "checkpoint": decision.checkpoint, "status": decision.provider_status,
            "rules_reason": why, "evidence": ev, "shadow": shadow,
            "router_calls": decision.router_calls + extra, "digests": decision.digests.to_json()});
        let total = decision.router_calls + extra;
        GovernedRoute {
            decision,
            mode,
            rules_reason: why,
            shadow,
            router_calls_total: total,
            explanation,
        }
    };
    if let Some(p) = pin {
        let scr = screen(&inputs.request, &inputs.policy);
        if !scr.admissible.iter().any(|m| m.id == p) {
            return Err(HarnessDiagnostic::new(
                "SPX-HPJ016",
                format!("pinned model `{p}` is not admissible under policy; pins never fall back"),
            ));
        }
        let only = without_remote(&inputs, allow, Some(&p))?;
        let pc = p.clone();
        let d = decide(&only, ctx, None, &|| live_f(Some(&pc)), cache)?;
        return Ok(finish(
            d,
            "pin",
            Some("pinned".into()),
            None,
            0,
            Value::Null,
        ));
    }

    let mut evidence = Value::Null;
    let mut off: Option<String> = None;
    let mut use_provider = false;
    match (&g.cfg.mode, provider.as_deref()) {
        (RoutingMode::Rules, _) => off = Some("mode rules".into()),
        (_, None) => off = Some("no provider configured".into()),
        _ => use_provider = true,
    }
    let scr = screen(&inputs.request, &inputs.policy);
    if use_provider {
        if let Some(t) = g.cfg.cheap_bypass_micros {
            if scr.admissible.iter().all(|m| m.est_cost_micros <= t) {
                use_provider = false;
                off = Some("cheap-task bypass".into());
            }
        }
    }
    if use_provider {
        if let Some(h) = g.router_headroom_tokens {
            if h < g.router_request_tokens {
                use_provider = false;
                off =
                    Some("router budget exhausted: task budget cannot cover a router call".into());
            }
        }
    }
    let mut provider = provider;
    if use_provider && g.cfg.mode == RoutingMode::QualifiedAuto {
        let p = provider.as_deref_mut().expect("provider present");
        let key = EvidenceKey::live(&p.profile, &inputs.request.catalog_digest());
        let kd = key.digest();
        let verdict = match (g.registry, g.lock) {
            (None, _) => Err("no evidence registry".to_string()),
            (_, None) => Err("no qualified profile locked for this session".to_string()),
            (_, Some(l)) if l.key_digest != kd => {
                Err("live profile differs from the session's qualified profile (weights, catalog, normalization or distribution changed)".into())
            }
            (Some(r), _) => match gate_for(r, &key, g.spec) {
                (gate, Some(dec)) if matches!(gate.status, GateStatus::Passed { .. }) => {
                    evidence = json!({"key": key.to_json(), "key_digest": kd, "record_digest": dec.record_digest, "spec_digest": dec.spec_digest});
                    p.gate = gate;
                    p.mode = ProviderMode::Auto;
                    Ok(())
                }
                (_, Some(dec)) => Err(format!("evidence not qualified: {}", dec.reasons.join("; "))),
                (_, None) => Err("no evidence record for the live key".into()),
            },
        };
        if let Err(e) = verdict {
            use_provider = false;
            off = Some(e);
        }
    } else if use_provider {
        provider.as_deref_mut().expect("provider present").mode = ProviderMode::Explicit;
    }
    let mode = g.cfg.mode.as_str();
    if use_provider {
        let d = decide(&inputs, ctx, provider, &|| live_f(None), cache)?;
        return Ok(finish(d, mode, None, None, 0, evidence));
    }
    // Rules decide; an explicit shadow budget may still evaluate a recommendation.
    let (mut shadow, mut spent) = (None, 0u32);
    if g.cfg.shadow_max_calls > 0 && g.cfg.mode != RoutingMode::Rules {
        if let Some(p) = provider {
            p.mode = ProviderMode::Explicit;
            let mut si = inputs.clone();
            si.request.budget.max_router_calls = g.cfg.shadow_max_calls;
            let shadow_live = || {
                let mut c = live_f(None);
                c.request.budget.max_router_calls = g.cfg.shadow_max_calls;
                c
            };
            let sd = decide(&si, ctx, Some(p), &shadow_live, None)?;
            spent = sd.router_calls;
            shadow = Some(
                json!({"recommended": sd.choice, "source": format!("{:?}", sd.source),
                "router_calls": sd.router_calls, "changes_route": false}),
            );
        }
    }
    let d = decide(&inputs, ctx, None, &|| live_f(None), None)?;
    if let Some(s) = shadow.as_mut() {
        s["agrees"] = json!(s["recommended"] == json!(d.choice));
    }
    Ok(finish(d, mode, off, shadow, spent, evidence))
}

/// True when `gate` is a `Passed` gate issued by `qualify::gate_for` for
/// exactly `key` (the live profile and catalog). A bare `Passed` string from
/// anywhere else does not unlock automatic selection in the workflow.
pub fn gate_attests_key(gate: &super::provider::EnablementGate, key: &EvidenceKey) -> bool {
    gate.task == key.task
        && gate.profile == key.provider_id
        && matches!(&gate.status, GateStatus::Passed { evidence }
            if evidence.starts_with(&format!("evidence:{}:", key.digest())))
}
