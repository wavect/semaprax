//! Cohort metrics and the promotion rule for profile arms (TC-12).
//!
//! Primary metric: total billed spend over the cohort divided by independently
//! accepted tasks. It is undefined (`null`), never zero, when nothing is
//! accepted or any dispatched attempt has unknown cost. A profile is only
//! recommended when quality, privacy and total-cost gates all pass on a
//! complete, real, pinned cohort; incomplete or fixture evidence is
//! inconclusive and a failed gate is no-go, and both leave defaults unchanged.
//! Evidence is registered in the HN-16 `EvidenceRegistry` under a key built from
//! the model, tool, task-set and profile pins; the lookup uses the live pins, so
//! drift makes the evidence `NotEvaluated` and the defaults return.

use super::profile_arms::{Criterion, Pins, BASELINE};
use crate::bench::measure::{mean, percentile, r4};
use crate::decision::evidence::{
    EvidenceKey, EvidenceRecord, EvidenceRegistry, MatchedBudget, Origin, Outcome, RetryOwner,
};
use crate::decision::provider::GateStatus;
use crate::decision::qualify::{evaluate, gate_for, RULES_ARM};
use crate::json::digest;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const QUALIFICATION_SCHEMA: &str = "semaprax.harness-profile-qualification.v1";
const TASK_ID: &str = "cost-profile/v1";

fn item(r: &Value) -> String {
    format!("{}|{}", r["task"].as_str().unwrap_or(""), r["rep"])
}

fn rate(k: usize, n: usize) -> Option<f64> {
    (n > 0).then(|| r4(k as f64 / n as f64))
}

/// Metrics of one arm over its cohort rows.
pub fn metrics(rows: &[&Value]) -> Value {
    let n = rows.len();
    let accepted = rows.iter().filter(|r| r["accepted"] == true).count();
    let first = rows.iter().filter(|r| r["first_pass"] == true).count();
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    let mut caches: BTreeMap<String, usize> = BTreeMap::new();
    for r in rows {
        *outcomes
            .entry(r["outcome"].as_str().unwrap_or("error").into())
            .or_default() += 1;
        *caches
            .entry(r["cache"]["provider"].as_str().unwrap_or("none").into())
            .or_default() += 1;
    }
    let dispatched: Vec<&&Value> = rows
        .iter()
        .filter(|r| r["spend"]["dispatched"].as_u64().unwrap_or(0) > 0)
        .collect();
    let unknown = dispatched
        .iter()
        .filter(|r| r["spend"]["complete"] != true)
        .count();
    let per_trial: Vec<u64> = dispatched
        .iter()
        .filter_map(|r| r["spend"]["micros"].as_u64())
        .collect();
    let total = (unknown == 0 && !dispatched.is_empty()).then(|| per_trial.iter().sum::<u64>());
    let cpa = match total {
        Some(t) if accepted > 0 => Some(r4(t as f64 / accepted as f64)),
        _ => None,
    };
    let lat: Vec<u64> = rows
        .iter()
        .filter_map(|r| r["latency_ms"].as_u64())
        .collect();
    let att: Vec<f64> = rows
        .iter()
        .map(|r| r["attempt_count"].as_u64().unwrap_or(0) as f64)
        .collect();
    let estimated = dispatched
        .iter()
        .filter(|r| {
            r["spend"]["basis"]
                .as_array()
                .is_some_and(|b| b.iter().any(|x| x == "estimate"))
        })
        .count();
    json!({"trials": n, "accepted": accepted, "acceptance_rate": rate(accepted, n),
           "first_pass_success": rate(first, n), "outcomes": outcomes, "provider_cache_states": caches,
           "total_spend_micros": total, "spend_unknown_trials": unknown,
           "not_dispatched_trials": n - dispatched.len(), "estimated_cost_trials": estimated,
           "spend_per_accepted_micros": cpa,
           "spend_per_accepted_note": "null = undefined (no accepted task or unknown cost), never zero",
           "trial_spend_micros": {"p50": percentile(&per_trial, 50), "p90": percentile(&per_trial, 90), "max": per_trial.iter().max()},
           "attempts": {"total": att.iter().sum::<f64>(), "mean": mean(&att).map(r4)},
           "latency_ms": {"mean": mean(&lat.iter().map(|x| *x as f64).collect::<Vec<_>>()).map(r4), "p50": percentile(&lat, 50), "p95": percentile(&lat, 95)},
           "tamper_attempts": rows.iter().map(|r| r["tamper_attempts"].as_u64().unwrap_or(0)).sum::<u64>()})
}

fn gate(id: &str, pass: bool, detail: String) -> Value {
    json!({"id": id, "pass": pass, "detail": detail})
}

/// True when no string anywhere in `v` contains one of `forbidden` (task text,
/// reference solutions): records hold counts, digests and verdicts only.
pub fn content_free(v: &Value, forbidden: &[String]) -> bool {
    match v {
        Value::String(s) => !forbidden
            .iter()
            .any(|f| !f.is_empty() && s.contains(f.as_str())),
        Value::Array(a) => a.iter().all(|x| content_free(x, forbidden)),
        Value::Object(o) => o.values().all(|x| content_free(x, forbidden)),
        _ => true,
    }
}

fn key_for(
    arm: &str,
    arm_digest: &str,
    base_digest: &str,
    pins: &Pins,
    c: &Criterion,
) -> EvidenceKey {
    EvidenceKey {
        task: TASK_ID.into(),
        provider_id: arm.into(),
        weights_digest: pins.model.clone(),
        catalog_digest: digest(
            "semaprax.harness-profile-pins.v1",
            &json!({"tools": pins.tools, "taskset": pins.taskset, "profile": arm_digest, "baseline": base_digest}),
        ),
        normalization: "cost-profile/v1/receipt-cost.v1".into(),
        distribution: c.digest(),
    }
}

fn outcome(r: &Value, arm: &str, ceiling: u64) -> Outcome {
    let accepted = r["accepted"] == true;
    let origin = match (r["origin"].as_str(), r["outcome"].as_str()) {
        (_, Some("unavailable")) | (_, Some("not_applicable")) => Origin::Unavailable,
        (Some("real"), _) => Origin::Real,
        _ => Origin::Fixture,
    };
    Outcome {
        item: item(r),
        arm: arm.into(),
        model: r["model"].as_str().unwrap_or("").into(),
        origin,
        verified_by: "immutable-grader".into(),
        completed: accepted,
        regressions: r["tamper_attempts"].as_u64().unwrap_or(0) as u32,
        attempts: r["attempt_count"].as_u64().unwrap_or(0) as u32,
        cost_micros: r["spend"]["micros"]
            .as_u64()
            .or_else(|| (r["spend"]["dispatched"].as_u64().unwrap_or(0) == 0).then_some(0))
            .or((!accepted).then_some(ceiling)),
        latency_ms: r["latency_ms"].as_u64(),
        router_cost_micros: 0,
        context_cost_micros: 0,
        retry_owner: RetryOwner::Host,
    }
}

fn is_clean(r: &Value) -> bool {
    matches!(r["outcome"].as_str(), Some("accepted") | Some("failed"))
        && (r["spend"]["dispatched"].as_u64().unwrap_or(0) == 0 || r["spend"]["complete"] == true)
}

/// Qualify every non-baseline arm of a declared campaign. `declared` is the
/// parsed `campaign.json`; `live` are the pins as they are now; `forbidden`
/// are strings no record may contain.
pub fn qualify(
    declared: &Value,
    criterion: &Criterion,
    rows: &[Value],
    live: &Pins,
    forbidden: &[String],
) -> Value {
    let pins = Pins {
        model: declared["pins"]["model"].as_str().unwrap_or("").into(),
        tools: declared["pins"]["tools"].as_str().unwrap_or("").into(),
        taskset: declared["pins"]["taskset"].as_str().unwrap_or("").into(),
    };
    let by_arm = |a: &str| -> Vec<&Value> { rows.iter().filter(|r| r["arm"] == a).collect() };
    let arm_digest = |a: &str| -> String {
        declared["arms"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|x| x["id"] == a)
            .and_then(|x| x["profile_digest"].as_str())
            .unwrap_or("")
            .into()
    };
    let base_rows = by_arm(BASELINE);
    let base_items: BTreeSet<String> = base_rows.iter().map(|r| item(r)).collect();
    let base_m = metrics(&base_rows);
    let base_digest = arm_digest(BASELINE);
    let drift: Vec<&str> = [
        ("model", pins.model != live.model),
        ("tools", pins.tools != live.tools),
        ("taskset", pins.taskset != live.taskset),
    ]
    .iter()
    .filter(|(_, d)| *d)
    .map(|(n, _)| *n)
    .collect();
    let mut out = vec![];
    for a in declared["arms"].as_array().into_iter().flatten() {
        let id = a["id"].as_str().unwrap_or("");
        if id == BASELINE {
            continue;
        }
        let ar = by_arm(id);
        let m = metrics(&ar);
        let mut inconclusive: Vec<String> = vec![];
        if a["availability"]["state"] != "available" {
            inconclusive.push(format!("arm unavailable: {}", a["availability"]["reason"]));
        }
        let arm_items: BTreeSet<String> = ar.iter().map(|r| item(r)).collect();
        if base_items.is_empty() || arm_items != base_items {
            inconclusive.push("arm cohort does not match the defaults cohort".into());
        }
        if base_items.len() < criterion.gate.min_items {
            inconclusive.push(format!(
                "{} matched items, the declared criterion needs {}",
                base_items.len(),
                criterion.gate.min_items
            ));
        }
        for r in ar.iter().chain(base_rows.iter()) {
            if !is_clean(r) {
                inconclusive.push(format!(
                    "trial {} is {} (unavailable, untested, aborted or unknown cost)",
                    r["trial"].as_str().unwrap_or("?"),
                    r["outcome"].as_str().unwrap_or("?")
                ));
                break;
            }
        }
        if ar
            .iter()
            .chain(base_rows.iter())
            .any(|r| r["origin"] != "real")
        {
            inconclusive.push("offline fixture evidence cannot support a promotion".into());
        }
        if ar
            .iter()
            .chain(base_rows.iter())
            .any(|r| r["path"] != "production-harness")
        {
            inconclusive.push("only the production-harness path validates HostModel wire changes; the raw model loop does not".into());
        }
        if ar
            .iter()
            .any(|r| r["profile_digest"] != a["profile_digest"])
        {
            inconclusive
                .push("trials were recorded under a different profile configuration".into());
        }
        let ceiling = rows
            .iter()
            .filter_map(|r| r["spend"]["micros"].as_u64())
            .max()
            .unwrap_or(0);
        let max_attempts = rows
            .iter()
            .filter_map(|r| r["attempt_count"].as_u64())
            .max()
            .unwrap_or(1) as u32;
        let key = key_for(
            id,
            a["profile_digest"].as_str().unwrap_or(""),
            &base_digest,
            &pins,
            criterion,
        );
        let live_key = key_for(
            id,
            a["profile_digest"].as_str().unwrap_or(""),
            &base_digest,
            live,
            criterion,
        );
        let mut gates = vec![];
        let mut registry = EvidenceRegistry::default();
        let mut record_digest = Value::Null;
        let mut decision = if inconclusive.is_empty() {
            "no-go"
        } else {
            "inconclusive"
        };
        let mut reasons: Vec<String> = inconclusive.clone();
        if inconclusive.is_empty() {
            let mut outcomes: Vec<Outcome> = base_rows
                .iter()
                .map(|r| outcome(r, RULES_ARM, ceiling))
                .collect();
            outcomes.extend(ar.iter().map(|r| outcome(r, id, ceiling)));
            let rec = EvidenceRecord {
                key: key.clone(),
                budget: MatchedBudget {
                    max_cost_micros: ceiling,
                    max_attempts,
                },
                eval_items: base_items.clone(),
                trained_on: BTreeSet::new(),
                outcomes,
                calibration: None,
            };
            record_digest = json!(rec.digest());
            let _ = registry.register(rec.clone());
            let ev = evaluate(&criterion.gate, &rec);
            gates.push(gate(
                "quality+cost+latency (HN-16 gate)",
                ev.go,
                ev.reasons.join("; "),
            ));
            let fp_b = base_m["first_pass_success"].as_f64().unwrap_or(0.0);
            let fp_a = m["first_pass_success"].as_f64().unwrap_or(0.0);
            gates.push(gate(
                "first-pass non-inferiority",
                fp_a + criterion.first_pass_margin >= fp_b,
                format!(
                    "first-pass {fp_a:.3} vs defaults {fp_b:.3}, margin {}",
                    criterion.first_pass_margin
                ),
            ));
            let (cb, ca) = (
                base_m["spend_per_accepted_micros"].as_f64(),
                m["spend_per_accepted_micros"].as_f64(),
            );
            gates.push(match (cb, ca) {
                (Some(b), Some(c)) => gate(
                    "total cost per accepted task",
                    c <= b * (1.0 - criterion.gate.min_cost_saving),
                    format!(
                        "{c:.1} vs defaults {b:.1} micro-units, required saving {}",
                        criterion.gate.min_cost_saving
                    ),
                ),
                _ => gate(
                    "total cost per accepted task",
                    false,
                    "undefined (no accepted task)".into(),
                ),
            });
            let private = !criterion.require_privacy
                || (m["tamper_attempts"] == 0 && ar.iter().all(|r| content_free(r, forbidden)));
            gates.push(gate(
                "privacy",
                private,
                "no oracle tampering; records carry no task or answer text".into(),
            ));
            let pass = gates.iter().all(|g| g["pass"] == true);
            decision = if pass { "go" } else { "no-go" };
            if !pass {
                reasons = gates
                    .iter()
                    .filter(|g| g["pass"] != true)
                    .map(|g| {
                        format!(
                            "{}: {}",
                            g["id"].as_str().unwrap_or(""),
                            g["detail"].as_str().unwrap_or("")
                        )
                    })
                    .collect();
            }
        }
        // Drift: the registry only grants the gate to the key the evidence was recorded under.
        let granted = matches!(
            gate_for(&registry, &live_key, &criterion.gate).0.status,
            GateStatus::Passed { .. }
        );
        if decision == "go" && !granted {
            decision = "invalidated";
            reasons = vec![format!(
                "pins drifted since the evidence was recorded: {}",
                drift.join(", ")
            )];
        }
        out.push(json!({"arm": id, "decision": decision, "reasons": reasons, "metrics": m, "gates": gates,
                        "evidence_key": key.to_json(), "evidence_key_digest": key.digest(), "record_digest": record_digest}));
    }
    let best = out
        .iter()
        .filter(|v| v["decision"] == "go")
        .min_by(|a, b| {
            let c = |v: &Value| {
                v["metrics"]["spend_per_accepted_micros"]
                    .as_f64()
                    .unwrap_or(f64::MAX)
            };
            c(a).total_cmp(&c(b))
        })
        .map(|v| v["arm"].clone());
    json!({"schema": QUALIFICATION_SCHEMA, "criterion_digest": criterion.digest(),
           "baseline": {"arm": BASELINE, "metrics": base_m}, "arms": out,
           "recommendation": match &best {
               Some(a) => json!({"profile": a, "action": "recommend-scoped-default", "note": "recommendation only; nothing is activated without explicit approval"}),
               None => json!({"profile": null, "action": "leave-defaults-unchanged"}),
           },
           "paid_qualification": "unrun unless the cohort origin is real; fixture evidence never promotes"})
}
