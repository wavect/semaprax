//! Predeclared qualification gate (HN-16): a held-out, matched comparison of
//! the rules arm against one learned profile. A `Go` is the only way an
//! `EnablementGate` becomes `Passed`; fixture or unavailable cells never do.

use super::evidence::{EvidenceKey, EvidenceRecord, EvidenceRegistry, Origin, Outcome};
use super::provider::{EnablementGate, GateStatus};
use crate::json;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const RULES_ARM: &str = "rules";

/// Thresholds fixed before the evaluation runs; their digest is recorded.
#[derive(Clone, Debug, PartialEq)]
pub struct GateSpec {
    pub min_items: usize,
    /// Learned completion rate may trail rules by at most this much.
    pub completion_margin: f64,
    /// Required saving in total cost (router and context included), 0..=1.
    pub min_cost_saving: f64,
    /// Learned arm may add no more regressions than rules.
    pub max_extra_regressions: u32,
    /// Learned mean latency may be at most this multiple of rules.
    pub max_latency_ratio: f64,
}

impl Default for GateSpec {
    fn default() -> Self {
        Self {
            min_items: 30,
            completion_margin: 0.0,
            min_cost_saving: 0.10,
            max_extra_regressions: 0,
            max_latency_ratio: 1.5,
        }
    }
}

impl GateSpec {
    pub fn digest(&self) -> String {
        json::digest(
            "semaprax.decision.gate-spec.v1",
            &json!({"min_items": self.min_items, "completion_margin": self.completion_margin,
                    "min_cost_saving": self.min_cost_saving,
                    "max_extra_regressions": self.max_extra_regressions,
                    "max_latency_ratio": self.max_latency_ratio}),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GateDecision {
    pub go: bool,
    pub reasons: Vec<String>,
    pub metrics: Value,
    pub spec_digest: String,
    pub record_digest: String,
}

impl GateDecision {
    pub fn to_json(&self) -> Value {
        json!({"decision": if self.go { "go" } else { "no-go" }, "reasons": self.reasons,
               "metrics": self.metrics, "spec_digest": self.spec_digest,
               "record_digest": self.record_digest})
    }
}

struct Arm {
    n: usize,
    completed: usize,
    regressions: u32,
    attempts: u64,
    cost: u64,
    latency: u64,
}

fn arm(items: &[&Outcome], ceiling: u64) -> Arm {
    Arm {
        n: items.len(),
        completed: items.iter().filter(|o| o.completed).count(),
        regressions: items.iter().map(|o| o.regressions).sum(),
        attempts: items.iter().map(|o| u64::from(o.attempts)).sum(),
        cost: items.iter().map(|o| o.total_cost(ceiling)).sum(),
        latency: items.iter().map(|o| o.latency_ms.unwrap_or(0)).sum(),
    }
}

fn counted<'a>(rec: &EvidenceRecord, m: &BTreeMap<&str, &'a Outcome>) -> Vec<&'a Outcome> {
    rec.eval_items
        .iter()
        .filter_map(|i| m.get(i.as_str()).copied())
        .collect()
}

/// Evaluate one record against the predeclared spec.
pub fn evaluate(spec: &GateSpec, rec: &EvidenceRecord) -> GateDecision {
    let mut why = Vec::new();
    if rec.validate().is_err() {
        why.push("record contains an invalid outcome".to_string());
    }
    let learned_arm = rec.key.provider_id.as_str();
    let by_arm = |a: &str| -> BTreeMap<&str, &Outcome> {
        rec.outcomes
            .iter()
            .filter(|o| o.arm == a)
            .map(|o| (o.item.as_str(), o))
            .collect()
    };
    let (rules, learned) = (by_arm(RULES_ARM), by_arm(learned_arm));
    if rec.eval_items.len() < spec.min_items {
        why.push(format!(
            "{} held-out items, need {}",
            rec.eval_items.len(),
            spec.min_items
        ));
    }
    let leaked: Vec<&String> = rec.eval_items.intersection(&rec.trained_on).collect();
    if !leaked.is_empty() {
        why.push(format!(
            "{} sealed eval item(s) were trained or calibrated on",
            leaked.len()
        ));
    }
    let matched = rec
        .eval_items
        .iter()
        .all(|i| rules.contains_key(i.as_str()) && learned.contains_key(i.as_str()));
    if !matched {
        why.push(
            "arms are not matched: an eval item lacks an outcome for rules or the learned profile"
                .into(),
        );
    }
    let (r_items, l_items) = (counted(rec, &rules), counted(rec, &learned));
    if r_items
        .iter()
        .chain(&l_items)
        .any(|o| o.origin != Origin::Real)
    {
        why.push(
            "fixture or unavailable cells present: only real verified runs can unlock auto".into(),
        );
    }
    if l_items.iter().any(|o| o.attempts > rec.budget.max_attempts)
        || r_items.iter().any(|o| o.attempts > rec.budget.max_attempts)
    {
        why.push("an arm exceeded the matched attempt budget".into());
    }
    let ceiling = rec.budget.max_cost_micros;
    let (r, l) = (arm(&r_items, ceiling), arm(&l_items, ceiling));
    let rate = |a: &Arm| {
        if a.n == 0 {
            0.0
        } else {
            a.completed as f64 / a.n as f64
        }
    };
    if matched && l.n > 0 {
        if rate(&l) + spec.completion_margin < rate(&r) {
            why.push(format!(
                "completion {:.3} trails rules {:.3}",
                rate(&l),
                rate(&r)
            ));
        }
        if l.regressions > r.regressions + spec.max_extra_regressions {
            why.push(format!(
                "regressions {} exceed rules {}",
                l.regressions, r.regressions
            ));
        }
        let saving = if r.cost == 0 {
            0.0
        } else {
            1.0 - l.cost as f64 / r.cost as f64
        };
        if saving < spec.min_cost_saving {
            why.push(format!(
                "cost saving {saving:.3} below {:.3}",
                spec.min_cost_saving
            ));
        }
        if r.latency > 0 && l.latency as f64 > r.latency as f64 * spec.max_latency_ratio {
            why.push("latency above the allowed ratio".into());
        }
    }
    let m = |a: &Arm| {
        json!({"n": a.n, "completed": a.completed, "regressions": a.regressions,
               "attempts": a.attempts, "total_cost_micros": a.cost, "latency_ms": a.latency})
    };
    GateDecision {
        go: why.is_empty(),
        reasons: why,
        metrics: json!({"rules": m(&r), "learned": m(&l), "origins": origin_counts(rec)}),
        spec_digest: spec.digest(),
        record_digest: rec.digest(),
    }
}

fn origin_counts(rec: &EvidenceRecord) -> Value {
    let mut c: BTreeMap<&str, u32> = BTreeMap::new();
    for o in &rec.outcomes {
        *c.entry(o.origin.as_str()).or_default() += 1;
    }
    json!(c)
}

/// The `EnablementGate` the registry grants `key`: `Passed` only for a stored
/// record that evaluates to go; `Failed` or `NotEvaluated` otherwise.
pub fn gate_for(
    reg: &EvidenceRegistry,
    key: &EvidenceKey,
    spec: &GateSpec,
) -> (EnablementGate, Option<GateDecision>) {
    let status_gate = |status| EnablementGate {
        task: key.task.clone(),
        profile: key.provider_id.clone(),
        status,
    };
    let Some(rec) = reg.get(key) else {
        return (status_gate(GateStatus::NotEvaluated), None);
    };
    let dec = evaluate(spec, rec);
    let status = if dec.go {
        GateStatus::Passed {
            evidence: format!("evidence:{}:{}", key.digest(), dec.record_digest),
        }
    } else {
        GateStatus::Failed
    };
    (status_gate(status), Some(dec))
}

/// Smallest observed score whose at-or-above precision meets `target`
/// (with at least `min_n` samples); `None` means no calibrated threshold, so
/// the provider must not be trusted by score.
pub fn calibrate_min_confidence(samples: &[(f64, bool)], target: f64, min_n: usize) -> Option<f64> {
    let mut scores: Vec<f64> = samples.iter().map(|s| s.0).collect();
    scores.sort_by(|a, b| a.total_cmp(b));
    scores.dedup();
    scores.into_iter().find(|&t| {
        let above: Vec<&(f64, bool)> = samples.iter().filter(|s| s.0 >= t).collect();
        above.len() >= min_n
            && above.iter().filter(|s| s.1).count() as f64 / above.len() as f64 >= target
    })
}
