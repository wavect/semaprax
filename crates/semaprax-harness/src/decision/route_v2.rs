//! `model-route/v2` (MR-01): the typed task-feature projection, host routing
//! signals and the optional candidate descriptor carried by a `ModelPlan`.
//!
//! Every signal is a host/compiler/runtime fact; nothing here is produced by a
//! model. Unknown values are explicit (`unknown`, `null`), never zero.

use super::route::{
    bad, enum_of, flag, shape, str_enum, uint, Confidentiality, LatencyClass, TaskFeatures,
};
use crate::diag::HarnessResult;
use crate::json;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

str_enum!(
    /// Who the routed step serves.
    ExecutionDomain { Development = "development", Application = "application" }
);
str_enum!(
    /// Role of the routed step in the host workflow.
    Phase { Plan = "plan", Implement = "implement", Review = "review", Repair = "repair", Turn = "turn", Handoff = "handoff", Unknown = "unknown" }
);
str_enum!(
    /// Host classification of the previous terminal failure on this lineage.
    PreviousFailure { None = "none", ParseSchema = "parse_schema", SemanticLaw = "semantic_law", ToolTransport = "tool_transport", Acceptance = "acceptance", Budget = "budget", Unknown = "unknown" }
);
str_enum!(Modality { Text = "text", Image = "image" });
str_enum!(
    /// Declared quality tier of a candidate; never a benchmark claim.
    QualityTier { Economy = "economy", Standard = "standard", Frontier = "frontier", Unknown = "unknown" }
);
str_enum!(
    /// Provenance of a cost or latency estimate.
    EstimateBasis { Measured = "measured", Configured = "configured", Unknown = "unknown" }
);
str_enum!(
    /// What the router endpoint may see of the task.
    Disclosure { MetadataOnly = "metadata_only", Excerpt = "excerpt" }
);

pub const MAX_TASK_PROFILE: usize = 64;
pub const MAX_ATTEMPT_INDEX: u32 = 64;
pub const MAX_PROGRESS: u32 = 1000;
pub const MAX_EXCERPT: usize = 1024;
pub const MAX_LABEL: usize = 64;

/// Bounded ASCII identifier/label: printable, no control characters.
pub(crate) fn printable_ascii(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| (0x20..0x7f).contains(&b))
}

/// Host routing signals beyond the v1 feature set. `Default` is the explicit
/// unknown projection: phase and previous failure `unknown`, no progress
/// counted, text input, remaining budget unknown, metadata-only disclosure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteSignals {
    pub execution_domain: ExecutionDomain,
    /// Host-declared task profile id; `None` uses the v1 task family id.
    pub task_profile: Option<String>,
    pub phase: Phase,
    pub attempt_index: u32,
    pub previous_failure: PreviousFailure,
    pub verified_progress: u32,
    pub no_progress: u32,
    pub input_modalities: BTreeSet<Modality>,
    /// `None` is unknown, never zero.
    pub remaining_budget_micros: Option<u64>,
    /// A bounded task excerpt the task asks to disclose to the router. It is
    /// sent only when the policy's independent router-disclosure rule admits it.
    pub excerpt: Option<String>,
}

impl Default for RouteSignals {
    fn default() -> Self {
        Self {
            execution_domain: ExecutionDomain::Development,
            task_profile: None,
            phase: Phase::Unknown,
            attempt_index: 0,
            previous_failure: PreviousFailure::Unknown,
            verified_progress: 0,
            no_progress: 0,
            input_modalities: [Modality::Text].into(),
            remaining_budget_micros: None,
            excerpt: None,
        }
    }
}

impl RouteSignals {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn to_json(&self) -> Value {
        json!({
            "execution_domain": self.execution_domain.as_str(),
            "task_profile": self.task_profile,
            "phase": self.phase.as_str(),
            "attempt_index": self.attempt_index,
            "previous_failure": self.previous_failure.as_str(),
            "verified_progress": self.verified_progress,
            "no_progress": self.no_progress,
            "input_modalities": self.input_modalities.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
            "remaining_budget_micros": self.remaining_budget_micros,
            "excerpt": self.excerpt,
        })
    }

    /// Parse the optional `signals` member of a route request; absent members
    /// keep their unknown defaults.
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        const C: &str = "SPX-HPJ003";
        let m = shape(
            v,
            "signals",
            &[],
            &[
                "execution_domain",
                "task_profile",
                "phase",
                "attempt_index",
                "previous_failure",
                "verified_progress",
                "no_progress",
                "input_modalities",
                "remaining_budget_micros",
                "excerpt",
            ],
            C,
        )?;
        let mut s = Self::default();
        if m.contains_key("execution_domain") {
            s.execution_domain = enum_of(m, "execution_domain", ExecutionDomain::parse, C)?;
        }
        if let Some(x) = m.get("task_profile").filter(|x| !x.is_null()) {
            s.task_profile = Some(task_profile(x.as_str().unwrap_or(""))?);
        }
        if m.contains_key("phase") {
            s.phase = enum_of(m, "phase", Phase::parse, C)?;
        }
        if m.contains_key("attempt_index") {
            s.attempt_index = uint(m, "attempt_index", MAX_ATTEMPT_INDEX.into(), C)? as u32;
        }
        if m.contains_key("previous_failure") {
            s.previous_failure = enum_of(m, "previous_failure", PreviousFailure::parse, C)?;
        }
        if m.contains_key("verified_progress") {
            s.verified_progress = uint(m, "verified_progress", MAX_PROGRESS.into(), C)? as u32;
        }
        if m.contains_key("no_progress") {
            s.no_progress = uint(m, "no_progress", MAX_PROGRESS.into(), C)? as u32;
        }
        if m.contains_key("input_modalities") {
            s.input_modalities = modalities(m, C)?;
        }
        if let Some(x) = m.get("remaining_budget_micros").filter(|x| !x.is_null()) {
            s.remaining_budget_micros =
                Some(x.as_u64().ok_or_else(|| {
                    bad(C, "`remaining_budget_micros` must be an integer or null")
                })?);
        }
        if let Some(x) = m.get("excerpt").filter(|x| !x.is_null()) {
            match x.as_str() {
                Some(e) if !e.is_empty() && e.len() <= MAX_EXCERPT => {
                    s.excerpt = Some(e.to_string())
                }
                _ => return Err(bad(C, "`excerpt` must be a string of 1..=1024 bytes")),
            }
        }
        Ok(s)
    }
}

fn task_profile(s: &str) -> HarnessResult<String> {
    let ok = !s.is_empty()
        && s.len() <= MAX_TASK_PROFILE
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-".contains(&b));
    if ok {
        Ok(s.to_string())
    } else {
        Err(bad(
            "SPX-HPJ003",
            "`task_profile` must be an ASCII id of 1..=64 bytes",
        ))
    }
}

pub(crate) fn modalities(
    m: &Map<String, Value>,
    code: &'static str,
) -> HarnessResult<BTreeSet<Modality>> {
    let arr = m
        .get("input_modalities")
        .or_else(|| m.get("modalities"))
        .and_then(Value::as_array)
        .ok_or_else(|| bad(code, "modalities must be an array"))?;
    let mut out = BTreeSet::new();
    for x in arr {
        let md = x
            .as_str()
            .and_then(Modality::parse)
            .ok_or_else(|| bad(code, "modality must be `text` or `image`"))?;
        if !out.insert(md) {
            return Err(bad(code, "duplicate modality"));
        }
    }
    if out.is_empty() {
        return Err(bad(code, "modalities must not be empty"));
    }
    Ok(out)
}

/// The closed `model-route/v2` feature projection sent to a router.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskFeaturesV2 {
    pub execution_domain: ExecutionDomain,
    pub task_profile: String,
    pub phase: Phase,
    pub attempt_index: u32,
    pub previous_failure: PreviousFailure,
    pub verified_progress: u32,
    pub no_progress: u32,
    pub estimated_context_tokens: u64,
    pub requires_structured_output: bool,
    pub requires_tools: bool,
    pub input_modalities: BTreeSet<Modality>,
    pub confidentiality: Confidentiality,
    pub latency_class: LatencyClass,
    pub remaining_budget_micros: Option<u64>,
}

impl TaskFeaturesV2 {
    /// Project the v1 features plus host signals. Signal bounds are checked
    /// here so an out-of-range host value is refused, never clamped.
    pub fn project(f: &TaskFeatures, s: &RouteSignals) -> HarnessResult<Self> {
        let profile = match &s.task_profile {
            Some(p) => task_profile(p)?,
            None => f.task_family.as_str().to_string(),
        };
        if s.attempt_index > MAX_ATTEMPT_INDEX
            || s.verified_progress > MAX_PROGRESS
            || s.no_progress > MAX_PROGRESS
            || s.input_modalities.is_empty()
        {
            return Err(bad(
                "SPX-HPJ019",
                "route signals are outside their documented bounds",
            ));
        }
        Ok(Self {
            execution_domain: s.execution_domain,
            task_profile: profile,
            phase: s.phase,
            attempt_index: s.attempt_index,
            previous_failure: s.previous_failure,
            verified_progress: s.verified_progress,
            no_progress: s.no_progress,
            estimated_context_tokens: f.estimated_context_tokens,
            requires_structured_output: f.requires_structured_output,
            requires_tools: f.requires_tools,
            input_modalities: s.input_modalities.clone(),
            confidentiality: f.confidentiality,
            latency_class: f.latency_class,
            remaining_budget_micros: s.remaining_budget_micros,
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "execution_domain": self.execution_domain.as_str(),
            "task_profile": self.task_profile,
            "phase": self.phase.as_str(),
            "attempt_index": self.attempt_index,
            "previous_failure": self.previous_failure.as_str(),
            "verified_progress": self.verified_progress,
            "no_progress": self.no_progress,
            "estimated_context_tokens": self.estimated_context_tokens,
            "requires_structured_output": self.requires_structured_output,
            "requires_tools": self.requires_tools,
            "input_modalities": self.input_modalities.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
            "confidentiality": self.confidentiality.as_str(),
            "latency_class": self.latency_class.as_str(),
            "remaining_budget_micros": self.remaining_budget_micros,
        })
    }

    /// Strict parse of the wire projection (used by validators and fixtures).
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        const C: &str = "SPX-HPA040";
        let m = shape(
            v,
            "v2 features",
            &[
                "execution_domain",
                "task_profile",
                "phase",
                "attempt_index",
                "previous_failure",
                "verified_progress",
                "no_progress",
                "estimated_context_tokens",
                "requires_structured_output",
                "requires_tools",
                "input_modalities",
                "confidentiality",
                "latency_class",
                "remaining_budget_micros",
            ],
            &[],
            C,
        )?;
        let remaining =
            match &m["remaining_budget_micros"] {
                Value::Null => None,
                x => Some(x.as_u64().ok_or_else(|| {
                    bad(C, "`remaining_budget_micros` must be an integer or null")
                })?),
            };
        let mods = m["input_modalities"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let sorted: Vec<&str> = mods.iter().filter_map(Value::as_str).collect();
        if sorted.windows(2).any(|w| w[0] >= w[1]) {
            return Err(bad(C, "`input_modalities` must be sorted and unique"));
        }
        Ok(Self {
            execution_domain: enum_of(m, "execution_domain", ExecutionDomain::parse, C)?,
            task_profile: task_profile(m["task_profile"].as_str().unwrap_or(""))
                .map_err(|_| bad(C, "`task_profile` must be an ASCII id of 1..=64 bytes"))?,
            phase: enum_of(m, "phase", Phase::parse, C)?,
            attempt_index: uint(m, "attempt_index", MAX_ATTEMPT_INDEX.into(), C)? as u32,
            previous_failure: enum_of(m, "previous_failure", PreviousFailure::parse, C)?,
            verified_progress: uint(m, "verified_progress", MAX_PROGRESS.into(), C)? as u32,
            no_progress: uint(m, "no_progress", MAX_PROGRESS.into(), C)? as u32,
            estimated_context_tokens: uint(m, "estimated_context_tokens", 1_000_000_000, C)?,
            requires_structured_output: flag(m, "requires_structured_output", C)?,
            requires_tools: flag(m, "requires_tools", C)?,
            input_modalities: modalities(m, C)?,
            confidentiality: enum_of(m, "confidentiality", Confidentiality::parse, C)?,
            latency_class: enum_of(m, "latency_class", LatencyClass::parse, C)?,
            remaining_budget_micros: remaining,
        })
    }

    pub fn digest(&self) -> String {
        json::digest("semaprax.decision.features.v2", &self.to_json())
    }
}

/// Optional host-maintained candidate descriptor (MR-01). `Default` is what a
/// v1 catalog entry means: tier unknown, configured estimates, derived label.
/// Absent members are not serialized, so v1 catalogs keep their bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanDescriptor {
    pub quality_tier: Option<QualityTier>,
    /// Host-configured comparison label (ASCII, 1..=64); `None` derives one.
    pub label: Option<String>,
    /// `None` means `configured` (the catalog figure is a configured estimate).
    pub cost_basis: Option<EstimateBasis>,
    pub latency_basis: Option<EstimateBasis>,
}

impl PlanDescriptor {
    pub const MEMBERS: [&'static str; 4] = ["quality_tier", "label", "cost_basis", "latency_basis"];

    pub fn tier(&self) -> QualityTier {
        self.quality_tier.unwrap_or(QualityTier::Unknown)
    }

    pub fn cost_basis(&self) -> EstimateBasis {
        self.cost_basis.unwrap_or(EstimateBasis::Configured)
    }

    pub fn latency_basis(&self) -> EstimateBasis {
        self.latency_basis.unwrap_or(EstimateBasis::Configured)
    }

    /// Insert the present members into a `ModelPlan` JSON object.
    pub fn write(&self, out: &mut Map<String, Value>) {
        if let Some(t) = self.quality_tier {
            out.insert("quality_tier".into(), json!(t.as_str()));
        }
        if let Some(l) = &self.label {
            out.insert("label".into(), json!(l));
        }
        if let Some(b) = self.cost_basis {
            out.insert("cost_basis".into(), json!(b.as_str()));
        }
        if let Some(b) = self.latency_basis {
            out.insert("latency_basis".into(), json!(b.as_str()));
        }
    }

    pub fn read(m: &Map<String, Value>, code: &'static str) -> HarnessResult<Self> {
        let mut d = Self::default();
        if m.contains_key("quality_tier") {
            d.quality_tier = Some(enum_of(m, "quality_tier", QualityTier::parse, code)?);
        }
        if let Some(l) = m.get("label") {
            match l.as_str() {
                Some(s) if printable_ascii(s, MAX_LABEL) => d.label = Some(s.to_string()),
                _ => return Err(bad(code, "`label` must be printable ASCII of 1..=64 bytes")),
            }
        }
        if m.contains_key("cost_basis") {
            d.cost_basis = Some(enum_of(m, "cost_basis", EstimateBasis::parse, code)?);
        }
        if m.contains_key("latency_basis") {
            d.latency_basis = Some(enum_of(m, "latency_basis", EstimateBasis::parse, code)?);
        }
        Ok(d)
    }
}
