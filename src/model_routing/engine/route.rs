//! `model-route/v1` request types, strict parsing, digests and the hard host
//! policy screen that runs before any decision provider is consulted.

use super::diag::{DecisionResult, Diagnostic};
use super::json;
use super::policy::RoutePolicy;
use super::registry;
use super::route_v2::{PlanDescriptor, RouteSignals};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub(crate) fn bad(code: &'static str, msg: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, msg)
}

pub(crate) fn shape<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
    code: &'static str,
) -> DecisionResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| bad(code, format!("{what} must be an object")))?;
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(bad(code, format!("unexpected member `{k}` in {what}")));
        }
    }
    for r in required {
        if !m.contains_key(*r) {
            return Err(bad(code, format!("{what} is missing `{r}`")));
        }
    }
    Ok(m)
}

pub(crate) fn text(m: &Map<String, Value>, k: &str, code: &'static str) -> DecisionResult<String> {
    match m.get(k).and_then(Value::as_str) {
        Some(s) if !s.is_empty() && s.len() <= 128 && s.is_ascii() => Ok(s.to_string()),
        _ => Err(bad(
            code,
            format!("`{k}` must be a non-empty ASCII string of at most 128 bytes"),
        )),
    }
}

pub(crate) fn uint(
    m: &Map<String, Value>,
    k: &str,
    max: u64,
    code: &'static str,
) -> DecisionResult<u64> {
    match m.get(k).and_then(Value::as_u64) {
        Some(n) if n <= max => Ok(n),
        _ => Err(bad(
            code,
            format!("`{k}` must be an integer within 0..={max}"),
        )),
    }
}

pub(crate) fn flag(m: &Map<String, Value>, k: &str, code: &'static str) -> DecisionResult<bool> {
    m.get(k)
        .and_then(Value::as_bool)
        .ok_or_else(|| bad(code, format!("`{k}` must be a boolean")))
}

macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($v:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name { $($v),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),+];
            pub fn as_str(self) -> &'static str { match self { $($name::$v => $s),+ } }
            pub fn parse(s: &str) -> Option<Self> { Self::ALL.iter().copied().find(|x| x.as_str() == s) }
        }
    };
}
pub(crate) use str_enum;

str_enum!(TaskFamily { Mechanical = "mechanical", TestsDocs = "tests_docs", LocalizedDebug = "localized_debug", SemanticLaw = "semantic_law" });
str_enum!(Confidentiality { Public = "public", Project = "project", Secret = "secret" });
str_enum!(LatencyClass { Interactive = "interactive", Batch = "batch" });

pub(crate) fn enum_of<T>(
    m: &Map<String, Value>,
    k: &str,
    parse: fn(&str) -> Option<T>,
    code: &'static str,
) -> DecisionResult<T> {
    m.get(k)
        .and_then(Value::as_str)
        .and_then(parse)
        .ok_or_else(|| bad(code, format!("`{k}` is not a member of its closed set")))
}

/// Closed task-feature set; nothing outside it is accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskFeatures {
    pub task_family: TaskFamily,
    pub estimated_context_tokens: u64,
    pub requires_structured_output: bool,
    pub requires_tools: bool,
    pub confidentiality: Confidentiality,
    pub latency_class: LatencyClass,
}

impl TaskFeatures {
    pub fn to_json(&self) -> Value {
        json!({
            "task_family": self.task_family.as_str(),
            "estimated_context_tokens": self.estimated_context_tokens,
            "requires_structured_output": self.requires_structured_output,
            "requires_tools": self.requires_tools,
            "confidentiality": self.confidentiality.as_str(),
            "latency_class": self.latency_class.as_str(),
        })
    }

    pub fn from_json(v: &Value) -> DecisionResult<Self> {
        const C: &str = "SPX-HPJ003";
        let m = shape(
            v,
            "features",
            &[
                "task_family",
                "estimated_context_tokens",
                "requires_structured_output",
                "requires_tools",
                "confidentiality",
                "latency_class",
            ],
            &[],
            C,
        )?;
        Ok(Self {
            task_family: enum_of(m, "task_family", TaskFamily::parse, C)?,
            estimated_context_tokens: uint(m, "estimated_context_tokens", 1_000_000_000, C)?,
            requires_structured_output: flag(m, "requires_structured_output", C)?,
            requires_tools: flag(m, "requires_tools", C)?,
            confidentiality: enum_of(m, "confidentiality", Confidentiality::parse, C)?,
            latency_class: enum_of(m, "latency_class", LatencyClass::parse, C)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Destination {
    Local,
    Remote { origin: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelPlan {
    pub id: String,
    pub destination: Destination,
    pub structured_output: bool,
    pub tools: bool,
    pub max_context: u64,
    pub est_cost_micros: u64,
    pub est_latency_ms: u64,
    pub strength_rank: u32,
    /// Optional MR-01 comparison descriptor; the default serializes to nothing.
    pub descriptor: PlanDescriptor,
}

impl ModelPlan {
    /// The cost estimate unless its descriptor marks it unknown: an unknown
    /// cost never ranks as the cheapest plan.
    pub fn known_cost(&self) -> Option<u64> {
        (self.descriptor.cost_basis() != super::route_v2::EstimateBasis::Unknown)
            .then_some(self.est_cost_micros)
    }

    pub fn to_json(&self) -> Value {
        let dest = match &self.destination {
            Destination::Local => json!({"kind": "local"}),
            Destination::Remote { origin } => json!({"kind": "remote", "origin": origin}),
        };
        let mut caps = Vec::new();
        if self.structured_output {
            caps.push("structured_output");
        }
        if self.tools {
            caps.push("tools");
        }
        let mut v = json!({
            "id": self.id, "destination": dest, "capabilities": caps,
            "max_context": self.max_context, "est_cost_micros": self.est_cost_micros,
            "est_latency_ms": self.est_latency_ms, "strength_rank": self.strength_rank,
        });
        if let Some(m) = v.as_object_mut() {
            self.descriptor.write(m);
        }
        v
    }

    pub fn from_json(v: &Value) -> DecisionResult<Self> {
        const C: &str = "SPX-HPJ003";
        let m = shape(
            v,
            "model plan",
            &[
                "id",
                "destination",
                "capabilities",
                "max_context",
                "est_cost_micros",
                "est_latency_ms",
                "strength_rank",
            ],
            &PlanDescriptor::MEMBERS,
            C,
        )?;
        let descriptor = PlanDescriptor::read(m, C)?;
        let d = shape(&m["destination"], "destination", &["kind"], &["origin"], C)?;
        let destination = match d["kind"].as_str() {
            Some("local") if !d.contains_key("origin") => Destination::Local,
            Some("remote") => Destination::Remote {
                origin: text(d, "origin", C)?,
            },
            _ => {
                return Err(bad(
                    C,
                    "destination kind must be `local` or `remote` with an origin",
                ))
            }
        };
        let mut caps = BTreeSet::new();
        for c in m["capabilities"]
            .as_array()
            .ok_or_else(|| bad(C, "`capabilities` must be an array"))?
        {
            match c.as_str() {
                Some(s @ ("structured_output" | "tools")) => {
                    caps.insert(s);
                }
                _ => return Err(bad(C, "unknown capability")),
            }
        }
        Ok(Self {
            id: text(m, "id", C)?,
            destination,
            structured_output: caps.contains("structured_output"),
            tools: caps.contains("tools"),
            max_context: uint(m, "max_context", 1_000_000_000_000, C)?,
            est_cost_micros: uint(m, "est_cost_micros", 1_000_000_000_000, C)?,
            est_latency_ms: uint(m, "est_latency_ms", 86_400_000, C)?,
            strength_rank: uint(m, "strength_rank", 1_000_000, C)? as u32,
            descriptor,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    pub max_cost_micros: u64,
    pub max_latency_ms: u64,
    pub max_router_calls: u32,
}

impl Budget {
    pub fn to_json(&self) -> Value {
        json!({"max_cost_micros": self.max_cost_micros, "max_latency_ms": self.max_latency_ms, "max_router_calls": self.max_router_calls})
    }

    pub fn from_json(v: &Value) -> DecisionResult<Self> {
        const C: &str = "SPX-HPJ003";
        let m = shape(
            v,
            "budget",
            &["max_cost_micros", "max_latency_ms", "max_router_calls"],
            &[],
            C,
        )?;
        Ok(Self {
            max_cost_micros: uint(m, "max_cost_micros", 1_000_000_000_000, C)?,
            max_latency_ms: uint(m, "max_latency_ms", 86_400_000, C)?,
            max_router_calls: uint(m, "max_router_calls", 1000, C)? as u32,
        })
    }
}

/// One `model-route/v1` request. The catalog is kept sorted by id so digests
/// do not depend on catalog order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteRequest {
    pub features: TaskFeatures,
    pub catalog: Vec<ModelPlan>,
    pub budget: Budget,
    /// MR-01 host signals for `model-route/v2`; unknown by default. They never
    /// enter the v1 digests.
    pub signals: RouteSignals,
}

pub const MAX_CATALOG: usize = 256;

impl RouteRequest {
    pub fn new(
        features: TaskFeatures,
        mut catalog: Vec<ModelPlan>,
        budget: Budget,
    ) -> DecisionResult<Self> {
        if catalog.len() > MAX_CATALOG {
            return Err(bad("SPX-HPJ003", "catalog exceeds 256 plans"));
        }
        catalog.sort_by(|a, b| a.id.cmp(&b.id));
        if catalog.windows(2).any(|w| w[0].id == w[1].id) {
            return Err(bad("SPX-HPJ003", "duplicate model plan id in catalog"));
        }
        Ok(Self {
            features,
            catalog,
            budget,
            signals: RouteSignals::default(),
        })
    }

    /// The same request with host routing signals attached.
    pub fn with_signals(mut self, signals: RouteSignals) -> Self {
        self.signals = signals;
        self
    }

    /// Parse `{task, features, budget, catalog?}`; the task must be an active
    /// registered one. A missing catalog is empty (supply it separately).
    pub fn from_json(v: &Value) -> DecisionResult<Self> {
        let m = shape(
            v,
            "route request",
            &["task", "features", "budget"],
            &["catalog", "policy", "lineage_id", "signals"],
            "SPX-HPJ003",
        )?;
        registry::resolve_route(m["task"].as_str().unwrap_or(""))?;
        let catalog = match m.get("catalog") {
            Some(c) => Self::catalog_from_json(c)?,
            None => Vec::new(),
        };
        let signals = match m.get("signals") {
            Some(s) => RouteSignals::from_json(s)?,
            None => RouteSignals::default(),
        };
        Ok(Self::new(
            TaskFeatures::from_json(&m["features"])?,
            catalog,
            Budget::from_json(&m["budget"])?,
        )?
        .with_signals(signals))
    }

    pub fn catalog_from_json(v: &Value) -> DecisionResult<Vec<ModelPlan>> {
        v.as_array()
            .ok_or_else(|| bad("SPX-HPJ003", "catalog must be an array"))?
            .iter()
            .map(ModelPlan::from_json)
            .collect()
    }

    pub fn features_digest(&self) -> String {
        json::digest("semaprax.decision.features.v1", &self.features.to_json())
    }

    pub fn catalog_digest(&self) -> String {
        json::digest(
            "semaprax.decision.catalog.v1",
            &Value::Array(self.catalog.iter().map(ModelPlan::to_json).collect()),
        )
    }
}

/// Result of the hard policy screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screening {
    /// Admissible plans sorted by id.
    pub admissible: Vec<ModelPlan>,
    /// `(id, reason)` for every excluded plan, sorted by id.
    pub excluded: Vec<(String, &'static str)>,
}

impl Screening {
    pub fn candidate_digest(&self) -> String {
        let ids: Vec<&str> = self.admissible.iter().map(|p| p.id.as_str()).collect();
        json::digest("semaprax.decision.candidates.v1", &json!(ids))
    }
}

/// Hard host policy filters; they run before any decision provider and no
/// provider output can widen the result.
pub fn screen(req: &RouteRequest, policy: &RoutePolicy) -> Screening {
    let f = &req.features;
    let mut admissible = Vec::new();
    let mut excluded = Vec::new();
    for p in &req.catalog {
        let why = if !policy.destination_allowed(f.confidentiality, &p.destination) {
            Some("destination not allowed for confidentiality")
        } else if f.requires_structured_output && !p.structured_output {
            Some("missing structured-output capability")
        } else if f.requires_tools && !p.tools {
            Some("missing tools capability")
        } else if f.estimated_context_tokens > p.max_context {
            Some("context too large")
        } else if p.est_cost_micros > req.budget.max_cost_micros {
            Some("unaffordable")
        } else if p.est_latency_ms > req.budget.max_latency_ms {
            Some("exceeds latency bound")
        } else {
            None
        };
        match why {
            Some(w) => excluded.push((p.id.clone(), w)),
            None => admissible.push(p.clone()),
        }
    }
    Screening {
        admissible,
        excluded,
    }
}
