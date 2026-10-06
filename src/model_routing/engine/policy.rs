//! Project-defined routing policy: destination rules, task-family classes,
//! fallback mode and router ceilings. Digested into every decision.

use super::diag::DecisionResult;
use super::json;
use super::route::{bad, enum_of, shape, str_enum, uint, Confidentiality, Destination, TaskFamily};
use serde_json::{json, Value};
use std::collections::BTreeSet;

str_enum!(FallbackMode { Rules = "rules", Refuse = "refuse" });

/// Per-lineage event budgets. Initial route, reasoning escalation and
/// transport retry are separate events and never share a counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineageBudgets {
    pub initial_route: u32,
    pub reasoning_escalation: u32,
    pub transport_retry: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutePolicy {
    /// Highest confidentiality a remote destination may receive; `None`
    /// forbids remote destinations entirely. `secret` is always local-only.
    pub remote_max_confidentiality: Option<Confidentiality>,
    /// Remote origins the host approves (empty means none).
    pub allowed_origins: BTreeSet<String>,
    /// Families that route to the strongest admissible model.
    pub hard_families: BTreeSet<TaskFamily>,
    /// Families rules always decide without consulting a router.
    pub rules_only_families: BTreeSet<TaskFamily>,
    pub fallback: FallbackMode,
    /// Router latency ceiling across a lineage.
    pub router_max_latency_ms: u64,
    /// Router call ceiling (the request budget may only lower it).
    pub router_max_calls: u32,
    pub lineage_budgets: LineageBudgets,
    /// MR-01 routing-disclosure rule for the router endpoint itself: the
    /// highest confidentiality whose bounded task excerpt a router may see.
    /// `None` (default) is metadata-only routing. Independent of the remote
    /// generation approval above.
    pub router_excerpt_max_confidentiality: Option<Confidentiality>,
}

impl Default for RoutePolicy {
    fn default() -> Self {
        Self {
            remote_max_confidentiality: None,
            allowed_origins: BTreeSet::new(),
            hard_families: [TaskFamily::SemanticLaw].into(),
            rules_only_families: [TaskFamily::Mechanical, TaskFamily::TestsDocs].into(),
            fallback: FallbackMode::Rules,
            router_max_latency_ms: 2_000,
            router_max_calls: 1,
            lineage_budgets: LineageBudgets {
                initial_route: 1,
                reasoning_escalation: 1,
                transport_retry: 2,
            },
            router_excerpt_max_confidentiality: None,
        }
    }
}

impl RoutePolicy {
    pub fn destination_allowed(&self, conf: Confidentiality, dest: &Destination) -> bool {
        match dest {
            Destination::Local => true,
            Destination::Remote { origin } => {
                conf != Confidentiality::Secret
                    && self
                        .remote_max_confidentiality
                        .is_some_and(|max| conf <= max)
                    && self.allowed_origins.contains(origin)
            }
        }
    }

    pub fn to_json(&self) -> Value {
        let fam = |s: &BTreeSet<TaskFamily>| s.iter().map(|f| f.as_str()).collect::<Vec<_>>();
        let mut v = json!({
            "remote_max_confidentiality": self.remote_max_confidentiality.map(|c| c.as_str()),
            "allowed_origins": self.allowed_origins,
            "hard_families": fam(&self.hard_families),
            "rules_only_families": fam(&self.rules_only_families),
            "fallback": self.fallback.as_str(),
            "router_max_latency_ms": self.router_max_latency_ms,
            "router_max_calls": self.router_max_calls,
            "lineage_budgets": {
                "initial_route": self.lineage_budgets.initial_route,
                "reasoning_escalation": self.lineage_budgets.reasoning_escalation,
                "transport_retry": self.lineage_budgets.transport_retry,
            },
        });
        // Present only when set, so the default policy keeps its v1 digest.
        if let Some(c) = self.router_excerpt_max_confidentiality {
            v["router_excerpt_max_confidentiality"] = json!(c.as_str());
        }
        v
    }

    pub fn digest(&self) -> String {
        json::digest("semaprax.decision.policy.v1", &self.to_json())
    }

    /// Parse a policy; every member is optional and defaults conservatively.
    pub fn from_json(v: &Value) -> DecisionResult<Self> {
        const C: &str = "SPX-HPJ004";
        let m = shape(
            v,
            "policy",
            &[],
            &[
                "remote_max_confidentiality",
                "allowed_origins",
                "hard_families",
                "rules_only_families",
                "fallback",
                "router_max_latency_ms",
                "router_max_calls",
                "lineage_budgets",
                "router_excerpt_max_confidentiality",
            ],
            C,
        )?;
        let mut p = Self::default();
        if m.contains_key("router_excerpt_max_confidentiality") {
            p.router_excerpt_max_confidentiality = Some(enum_of(
                m,
                "router_excerpt_max_confidentiality",
                Confidentiality::parse,
                C,
            )?);
        }
        if let Some(x) = m.get("remote_max_confidentiality") {
            p.remote_max_confidentiality = match x {
                Value::Null => None,
                _ => Some(enum_of(
                    m,
                    "remote_max_confidentiality",
                    Confidentiality::parse,
                    C,
                )?),
            };
        }
        let strs = |k: &str| -> DecisionResult<Option<Vec<String>>> {
            match m.get(k) {
                None => Ok(None),
                Some(a) => a
                    .as_array()
                    .ok_or_else(|| bad(C, format!("`{k}` must be an array")))?
                    .iter()
                    .map(|s| {
                        s.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| bad(C, format!("`{k}` holds a non-string")))
                    })
                    .collect::<DecisionResult<Vec<_>>>()
                    .map(Some),
            }
        };
        if let Some(o) = strs("allowed_origins")? {
            p.allowed_origins = o.into_iter().collect();
        }
        for (k, slot) in [("hard_families", 0), ("rules_only_families", 1)] {
            if let Some(list) = strs(k)? {
                let set = list
                    .iter()
                    .map(|s| {
                        TaskFamily::parse(s)
                            .ok_or_else(|| bad(C, format!("unknown task family `{s}`")))
                    })
                    .collect::<DecisionResult<BTreeSet<_>>>()?;
                if slot == 0 {
                    p.hard_families = set
                } else {
                    p.rules_only_families = set
                }
            }
        }
        if m.contains_key("fallback") {
            p.fallback = enum_of(m, "fallback", FallbackMode::parse, C)?;
        }
        if m.contains_key("router_max_latency_ms") {
            p.router_max_latency_ms = uint(m, "router_max_latency_ms", 600_000, C)?;
        }
        if m.contains_key("router_max_calls") {
            p.router_max_calls = uint(m, "router_max_calls", 1000, C)? as u32;
        }
        if let Some(b) = m.get("lineage_budgets") {
            let bm = shape(
                b,
                "lineage_budgets",
                &["initial_route", "reasoning_escalation", "transport_retry"],
                &[],
                C,
            )?;
            p.lineage_budgets = LineageBudgets {
                initial_route: uint(bm, "initial_route", 16, C)? as u32,
                reasoning_escalation: uint(bm, "reasoning_escalation", 16, C)? as u32,
                transport_retry: uint(bm, "transport_retry", 64, C)? as u32,
            };
        }
        Ok(p)
    }
}
