//! Profile arms and the predeclared campaign (TC-12). An arm is the current
//! defaults, one opt-in cost policy, or the combined candidate. The bounded
//! screening roster is `defaults + one arm per policy + combined`; there is no
//! combinatorial sweep. Policies whose lane has not landed are `Unavailable`
//! and report so instead of silently running as defaults.
//!
//! The campaign file (`campaign.json`) stores the non-inferiority criterion
//! before any trial runs; qualification reads only that file.

use crate::decision::qualify::GateSpec;
use crate::json::{canonical, digest};
use serde_json::{json, Map, Value};
use std::path::Path;

pub const BASELINE: &str = "defaults";
pub const CAMPAIGN_SCHEMA: &str = "semaprax.harness-profile-campaign.v1";
pub const COST_DOMAIN: &str = "semaprax.harness-cost-profile.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Policy {
    CompactSkills,
    ContextTarget,
    FeedbackAllowance,
    Tiers,
    CavemanView,
    PromptRenderer,
    SpendLedger,
    Routing,
}

pub const POLICIES: [Policy; 8] = [
    Policy::CompactSkills,
    Policy::ContextTarget,
    Policy::FeedbackAllowance,
    Policy::Tiers,
    Policy::CavemanView,
    Policy::PromptRenderer,
    Policy::SpendLedger,
    Policy::Routing,
];

impl Policy {
    pub fn id(self) -> &'static str {
        match self {
            Self::CompactSkills => "compact-skills",
            Self::ContextTarget => "context-target",
            Self::FeedbackAllowance => "feedback-allowance",
            Self::Tiers => "tiers",
            Self::CavemanView => "caveman-view",
            Self::PromptRenderer => "prompt-renderer",
            Self::SpendLedger => "spend-ledger",
            Self::Routing => "cost-aware-routing",
        }
    }

    /// `Some(reason)` while the owning lane has not landed.
    pub fn unavailable(self) -> Option<&'static str> {
        match self {
            Self::SpendLedger => Some("durable spend ledger (TC-03) has not landed"),
            Self::Routing => Some("cost-aware routing (TC-10) has not landed"),
            _ => None,
        }
    }

    /// The profile configuration this policy turns on (declared constants, part of the pin).
    pub fn overlay(self) -> Vec<(&'static str, Value)> {
        match self {
            Self::CompactSkills => vec![("skills.cost_profile", json!("compact"))],
            Self::ContextTarget => vec![("budget.context_target_bytes", json!(4096))],
            Self::FeedbackAllowance => vec![("budget.feedback_max_tokens", json!(600))],
            Self::Tiers => vec![
                ("budget.intent_cap", json!(1024)),
                ("budget.repair_cap", json!(2048)),
            ],
            Self::CavemanView => vec![("command_view.adapter", json!("caveman"))],
            Self::PromptRenderer => vec![
                ("budget.prompt_renderer", json!("ordered-v1")),
                ("budget.model_prompt_cache", json!("supported")),
            ],
            Self::SpendLedger => vec![("spend.ledger", json!("durable"))],
            Self::Routing => vec![("routing.cost_aware", json!(true))],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProfileArm {
    pub id: String,
    pub label: String,
    pub policies: Vec<Policy>,
    /// Arm of `arms.json` whose skill/context/view the raw trial loop applies.
    pub base_arm: String,
    /// Policies requested but not runnable (kept for the record).
    pub omitted: Vec<(Policy, &'static str)>,
    /// `Some(reason)`: the arm cannot run and reports `unavailable`.
    pub unavailable: Option<String>,
}

impl ProfileArm {
    pub fn overlay(&self) -> Map<String, Value> {
        self.policies
            .iter()
            .flat_map(|p| p.overlay())
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    /// Digest pin of the arm's exact configuration.
    pub fn profile_digest(&self) -> String {
        digest(COST_DOMAIN, &Value::Object(self.overlay()))
    }

    pub fn has(&self, p: Policy) -> bool {
        self.policies.contains(&p)
    }

    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "label": self.label,
               "policies": self.policies.iter().map(|p| p.id()).collect::<Vec<_>>(),
               "overlay": self.overlay(), "profile_digest": self.profile_digest(),
               "base_arm": self.base_arm,
               "omitted_unavailable": self.omitted.iter().map(|(p, w)| json!({"policy": p.id(), "reason": w})).collect::<Vec<_>>(),
               "availability": match &self.unavailable { Some(w) => json!({"state": "unavailable", "reason": w}), None => json!({"state": "available"}) }})
    }
}

/// Defaults, each policy alone, and the combined candidate (every available policy).
pub fn screening_roster(base_arm: &str) -> Vec<ProfileArm> {
    let mut v = vec![ProfileArm {
        id: BASELINE.into(),
        label: "current defaults (no opt-in policy)".into(),
        policies: vec![],
        base_arm: base_arm.into(),
        omitted: vec![],
        unavailable: None,
    }];
    for p in POLICIES {
        v.push(ProfileArm {
            id: p.id().into(),
            label: format!("{} alone", p.id()),
            policies: if p.unavailable().is_some() {
                vec![]
            } else {
                vec![p]
            },
            base_arm: base_arm.into(),
            omitted: vec![],
            unavailable: p.unavailable().map(String::from),
        });
    }
    let (on, off): (Vec<Policy>, Vec<Policy>) =
        POLICIES.iter().partition(|p| p.unavailable().is_none());
    v.push(ProfileArm {
        id: "combined".into(),
        label: "combined candidate: every available policy".into(),
        policies: on,
        base_arm: base_arm.into(),
        omitted: off
            .into_iter()
            .filter_map(|p| p.unavailable().map(|w| (p, w)))
            .collect(),
        unavailable: None,
    });
    v
}

/// Predeclared criterion. Stored before running; never read from arguments afterwards.
#[derive(Clone, Debug, PartialEq)]
pub struct Criterion {
    /// Quality, completion margin, minimum saving, regressions and latency ratio.
    pub gate: GateSpec,
    /// First-pass success may trail the defaults by at most this much.
    pub first_pass_margin: f64,
    /// Candidate spend per accepted task must be at most `1 - min_cost_saving` of the defaults'.
    pub require_privacy: bool,
}

impl Criterion {
    /// The existing quality gate (`QUALITY_TOLERANCE`: no acceptance drop) plus the
    /// HN-16 cost/latency defaults, declared up front.
    pub fn predeclared() -> Self {
        let mut gate = GateSpec::default();
        gate.completion_margin = crate::bench::gates::QUALITY_TOLERANCE;
        Self {
            gate,
            first_pass_margin: 0.10,
            require_privacy: true,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({"min_items": self.gate.min_items, "acceptance_margin": self.gate.completion_margin,
               "min_cost_saving": self.gate.min_cost_saving,
               "max_extra_regressions": self.gate.max_extra_regressions,
               "max_latency_ratio": self.gate.max_latency_ratio,
               "first_pass_margin": self.first_pass_margin, "require_privacy": self.require_privacy,
               "primary_metric": "total billed spend / independently accepted tasks (undefined, never zero, when nothing is accepted or any cost is unknown)"})
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let mut gate = GateSpec::default();
        gate.min_items = v["min_items"].as_u64()? as usize;
        gate.completion_margin = v["acceptance_margin"].as_f64()?;
        gate.min_cost_saving = v["min_cost_saving"].as_f64()?;
        gate.max_extra_regressions = v["max_extra_regressions"].as_u64()? as u32;
        gate.max_latency_ratio = v["max_latency_ratio"].as_f64()?;
        Some(Self {
            gate,
            first_pass_margin: v["first_pass_margin"].as_f64()?,
            require_privacy: v["require_privacy"].as_bool()?,
        })
    }

    pub fn digest(&self) -> String {
        digest("semaprax.harness-profile-criterion.v1", &self.to_json())
    }
}

/// Identity pins every trial and the evidence are keyed to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pins {
    pub model: String,
    /// Digest of the tool identities (compiler, graders' interpreters, adapter).
    pub tools: String,
    pub taskset: String,
}

impl Pins {
    pub fn to_json(&self) -> Value {
        json!({"model": self.model, "tools": self.tools, "taskset": self.taskset})
    }
}

#[derive(Clone, Debug)]
pub struct CampaignSpec {
    pub id: String,
    pub pins: Pins,
    pub arms: Vec<ProfileArm>,
    pub tasks: Vec<String>,
    pub reps: u32,
    pub criterion: Criterion,
    pub max_usd: f64,
    pub max_calls: u64,
}

impl CampaignSpec {
    pub fn to_json(&self) -> Value {
        json!({"schema": CAMPAIGN_SCHEMA, "id": self.id, "pins": self.pins.to_json(),
               "baseline": BASELINE, "arms": self.arms.iter().map(ProfileArm::to_json).collect::<Vec<_>>(),
               "cohort": {"tasks": self.tasks, "reps": self.reps},
               "criterion": self.criterion.to_json(), "criterion_digest": self.criterion.digest(),
               "caps": {"max_usd": self.max_usd, "max_calls": self.max_calls},
               "paid_qualification": "unrun"})
    }
}

/// Write `campaign.json` before any trial. An existing file must carry the same
/// criterion, arms and pins: a criterion is never chosen after results exist.
pub fn declare(dir: &Path, spec: &CampaignSpec) -> Result<(), String> {
    let path = dir.join("campaign.json");
    let new = spec.to_json();
    if let Ok(t) = std::fs::read_to_string(&path) {
        let old: Value = serde_json::from_str(&t).map_err(|e| format!("campaign.json: {e}"))?;
        for k in ["criterion_digest", "arms", "pins", "cohort"] {
            if canonical(&old[k]) != canonical(&new[k]) {
                return Err(format!(
                    "campaign.json already declares a different `{k}`; a declared campaign is immutable"
                ));
            }
        }
        return Ok(());
    }
    std::fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&new).unwrap_or_default()
        ),
    )
    .map_err(|e| format!("campaign.json: {e}"))
}

/// The declared campaign with its criterion verified against the stored digest.
pub fn load(dir: &Path) -> Result<(Value, Criterion), String> {
    let t = std::fs::read_to_string(dir.join("campaign.json"))
        .map_err(|e| format!("campaign.json: {e}"))?;
    let v: Value = serde_json::from_str(&t).map_err(|e| format!("campaign.json: {e}"))?;
    let c = Criterion::from_json(&v["criterion"]).ok_or("campaign.json: bad criterion")?;
    if v["criterion_digest"].as_str() != Some(&c.digest()) {
        return Err("campaign.json: criterion does not match its declared digest".into());
    }
    Ok((v, c))
}
