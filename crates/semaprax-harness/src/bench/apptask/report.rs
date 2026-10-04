//! Aggregation of raw trial records into per-class, per-arm, per-model results,
//! matched comparisons against the native baseline, predeclared gate verdicts
//! and scoped recommendations. No pooled headline: every figure is per task
//! class and model, with its sample size and variation.

use super::super::gates::{
    MAX_ADDED_LATENCY_MS, MIN_MATCHED_CELLS, MIN_NET_BYTE_REDUCTION, QUALITY_TOLERANCE,
};
use super::super::measure::{mean, percentile, r4, stdev, wilson95};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const BASELINE_ARM: &str = "native";
pub const SUMMARY_SCHEMA: &str = "semaprax.harness-apptask-summary.v1";
pub const RECOMMENDATION_SCHEMA: &str = "semaprax.harness-bench-recommendations.v1";
/// Minimum repetitions per (task, arm, model) cell for a result to be anything but a pilot.
pub const MIN_TRIALS_PER_CELL: u64 = 10;
/// A negative control must lose at least this much accepted rate against native to count as detected.
pub const CONTROL_MIN_DROP: f64 = 0.20;

#[derive(Clone, Debug)]
pub struct T {
    pub task: String,
    pub class: String,
    pub arm: String,
    pub role: String,
    pub model: String,
    pub size: String,
    pub rep: u64,
    pub cold: bool,
    pub status: String,
    pub passed: bool,
    pub first: bool,
    pub valid_first: bool,
    pub tokens_in: f64,
    pub tokens_out: f64,
    pub prov_in: Option<f64>,
    pub prov_out: Option<f64>,
    pub cost: Option<f64>,
    pub completion_ms: f64,
    pub attempts: f64,
    pub tamper: f64,
    pub skill_delivered: bool,
    pub skill_tokens: f64,
    pub recall: Option<f64>,
    pub build_ms: f64,
    pub index_bytes: f64,
}

impl T {
    pub fn tokens(&self) -> f64 {
        self.tokens_in + self.tokens_out
    }
    pub fn ok(&self) -> bool {
        self.status == "ok"
    }
}

pub fn parse(rows: &[Value]) -> Vec<T> {
    rows.iter()
        .filter(|v| v["schema"] == super::trial::TRIAL_SCHEMA)
        .map(|v| {
            let t = &v["totals"];
            let f = |x: &Value| x.as_f64().unwrap_or(0.0);
            T {
                task: v["task"].as_str().unwrap_or("").into(),
                class: v["class"].as_str().unwrap_or("").into(),
                arm: v["arm"].as_str().unwrap_or("").into(),
                role: v["arm_role"].as_str().unwrap_or("").into(),
                model: v["model"].as_str().unwrap_or("").into(),
                size: v["size"].as_str().unwrap_or("").into(),
                rep: v["rep"].as_u64().unwrap_or(0),
                cold: v["cold"].as_bool().unwrap_or(false),
                status: v["status"].as_str().unwrap_or("error").into(),
                passed: v["passed"].as_bool().unwrap_or(false),
                first: v["passed_first_attempt"].as_bool().unwrap_or(false),
                valid_first: v["structurally_valid_first"].as_bool().unwrap_or(false),
                tokens_in: f(&t["prompt_tokens_o200k"]),
                tokens_out: f(&t["answer_tokens_o200k"]),
                prov_in: t["provider_in"].as_f64(),
                prov_out: t["provider_out"].as_f64(),
                cost: t["cost_usd"].as_f64(),
                completion_ms: f(&t["completion_ms"]),
                attempts: f(&t["attempts"]),
                tamper: f(&v["tamper_attempts"]),
                skill_delivered: v["skill"]["delivered"].as_bool().unwrap_or(false),
                skill_tokens: f(&v["skill"]["tokens_o200k"]),
                recall: v["context"]["reference_recall"].as_f64(),
                build_ms: f(&v["context"]["index_build_ms"]),
                index_bytes: f(&v["context"]["index_bytes"]),
            }
        })
        .collect()
}

fn ms(xs: &[f64]) -> Value {
    json!({"mean": mean(xs).map(r4), "sd": stdev(xs).map(r4), "n": xs.len()})
}

fn rate(k: usize, n: usize) -> Value {
    let (lo, hi) = wilson95(k as u64, n as u64);
    json!({"k": k, "n": n, "rate": (n > 0).then(|| r4(k as f64 / n as f64)), "wilson95": [r4(lo), r4(hi)]})
}

fn group_stats(ts: &[&T]) -> Value {
    let ok: Vec<&&T> = ts.iter().filter(|t| t.ok()).collect();
    let n = ok.len();
    let k = ok.iter().filter(|t| t.passed).count();
    let single: Vec<&&&T> = ok.iter().filter(|t| t.class != "maintenance").collect();
    let valid = ok.iter().filter(|t| t.valid_first).count();
    let valid_and_passed_first = single.iter().filter(|t| t.first).count();
    let col = |f: &dyn Fn(&T) -> f64| ok.iter().map(|t| f(t)).collect::<Vec<f64>>();
    let opt = |f: &dyn Fn(&T) -> Option<f64>| ok.iter().filter_map(|t| f(t)).collect::<Vec<f64>>();
    let cost: Vec<f64> = opt(&|t| t.cost);
    let completion: Vec<u64> = ok.iter().map(|t| t.completion_ms as u64).collect();
    let cost_total: f64 = cost.iter().sum();
    let recall = opt(&|t| t.recall);
    let side = |cold: bool| {
        let s: Vec<&&&T> = ok.iter().filter(|t| t.cold == cold).collect::<Vec<_>>();
        let (n, k) = (s.len(), s.iter().filter(|t| t.passed).count());
        json!({"accepted": rate(k, n), "tokens_o200k": ms(&s.iter().map(|t| t.tokens()).collect::<Vec<_>>()),
               "completion_ms": ms(&s.iter().map(|t| t.completion_ms).collect::<Vec<_>>())})
    };
    json!({
        "trials": ts.len(), "ok": n, "not_ok": ts.len() - n,
        "accepted": rate(k, n),
        "accepted_first_attempt_single_step": rate(valid_and_passed_first, single.len()),
        "structurally_valid_first_attempt": rate(valid, n),
        "tokens_o200k": {"input": ms(&col(&|t| t.tokens_in)), "output": ms(&col(&|t| t.tokens_out)), "total": ms(&col(&|t| t.tokens())), "skill_prompt_each_attempt": ms(&col(&|t| t.skill_tokens))},
        "provider_usage": {"input": ms(&opt(&|t| t.prov_in)), "output": ms(&opt(&|t| t.prov_out)), "unavailable_trials": ok.iter().filter(|t| t.prov_in.is_none()).count()},
        "cost_usd": if cost.is_empty() { json!({"status": "unavailable (local model, no billing)"}) } else {
            json!({"per_trial": ms(&cost), "total": r4(cost_total), "per_accepted": (k > 0).then(|| r4(cost_total / k as f64))}) },
        "completion_ms": {"mean": mean(&completion.iter().map(|x| *x as f64).collect::<Vec<_>>()).map(r4), "p50": percentile(&completion, 50), "p95": percentile(&completion, 95)},
        "attempts": ms(&col(&|t| t.attempts)),
        "tamper_attempts": ok.iter().map(|t| t.tamper).sum::<f64>(),
        "skill_delivered_fraction": (n > 0).then(|| r4(ok.iter().filter(|t| t.skill_delivered).count() as f64 / n as f64)),
        "retrieval": {"reference_recall": ms(&recall), "index_build_ms_cold": ms(&ok.iter().filter(|t| t.cold && t.build_ms > 0.0).map(|t| t.build_ms).collect::<Vec<_>>()),
                      "index_bytes": mean(&ok.iter().filter(|t| t.index_bytes > 0.0).map(|t| t.index_bytes).collect::<Vec<_>>())},
        "cold": side(true), "warm": side(false),
    })
}

fn per_task(ts: &[&T]) -> Value {
    let mut m: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for t in ts.iter().filter(|t| t.ok()) {
        let e = m.entry(t.task.as_str()).or_default();
        e.1 += 1;
        e.0 += t.passed as usize;
    }
    Value::Object(
        m.into_iter()
            .map(|(k, (a, b))| (k.to_string(), json!(format!("{a}/{b}"))))
            .collect(),
    )
}

/// Matched comparison of one arm against native: same task, repetition and model.
pub fn paired(arm: &[&T], base: &[&T]) -> Value {
    let idx: BTreeMap<(&str, u64), &T> = base
        .iter()
        .filter(|t| t.ok())
        .map(|t| ((t.task.as_str(), t.rep), *t))
        .collect();
    let mut dq = vec![];
    let (mut tok, mut btok, mut cost, mut bcost, mut lat, mut out, mut bout) =
        (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut tasks = BTreeSet::new();
    for t in arm.iter().filter(|t| t.ok()) {
        if let Some(b) = idx.get(&(t.task.as_str(), t.rep)) {
            dq.push(t.passed as u8 as f64 - b.passed as u8 as f64);
            tok.push(t.tokens());
            btok.push(b.tokens());
            out.push(t.tokens_out);
            bout.push(b.tokens_out);
            lat.push(t.completion_ms - b.completion_ms);
            if let (Some(c), Some(bc)) = (t.cost, b.cost) {
                cost.push(c);
                bcost.push(bc);
            }
            tasks.insert(t.task.as_str());
        }
    }
    let n = dq.len();
    let sum = |v: &[f64]| v.iter().sum::<f64>();
    let red = |a: &[f64], b: &[f64]| (!b.is_empty() && sum(b) > 0.0).then(|| 1.0 - sum(a) / sum(b));
    let token_red = red(&tok, &btok);
    let cost_red = red(&cost, &bcost);
    let lat_mean = mean(&lat);
    let dq_mean = mean(&dq);
    let enough = n as u64 >= MIN_MATCHED_CELLS;
    let g_quality = dq_mean.is_some_and(|d| d >= QUALITY_TOLERANCE - 1e-9);
    let g_cost = token_red.is_some_and(|r| r >= MIN_NET_BYTE_REDUCTION)
        && cost_red.is_none_or(|r| r >= MIN_NET_BYTE_REDUCTION);
    let g_latency = lat_mean.is_some_and(|l| l <= MAX_ADDED_LATENCY_MS);
    json!({
        "matched_cells": n, "distinct_tasks": tasks.len(),
        "accepted_delta_per_cell": dq_mean.map(r4),
        "total_tokens_o200k_reduction": token_red.map(r4), "output_tokens_o200k_reduction": red(&out, &bout).map(r4),
        "billed_cost_reduction": cost_red.map(r4), "billed_cost_matched_cells": cost.len(),
        "added_completion_ms_per_cell": lat_mean.map(r4),
        "gates": {"N_matched_at_least_10": enough, "Q_quality_no_loss": g_quality, "C_cost_reduction_20pct": g_cost, "T_latency_within_2000ms": g_latency},
    })
}

fn verdict(arm_role: &str, p: &Value, not_ok: usize) -> (&'static str, String) {
    let g = &p["gates"];
    let n = p["matched_cells"].as_u64().unwrap_or(0);
    if arm_role == "NegativeControl" {
        let d = p["accepted_delta_per_cell"].as_f64().unwrap_or(0.0);
        return if d <= -CONTROL_MIN_DROP {
            (
                "control-detected",
                format!(
                    "accepted delta {d:+.3} per matched cell: the oracle caught the stripped work"
                ),
            )
        } else {
            ("control-NOT-detected", format!("accepted delta {d:+.3}: the oracle did not separate the control; the benchmark cannot be trusted here"))
        };
    }
    if not_ok > 0 {
        return (
            "untested-cells",
            format!("{not_ok} trial(s) did not run with the real tool or model"),
        );
    }
    let all = |k: &str| g[k].as_bool().unwrap_or(false);
    if n < MIN_MATCHED_CELLS {
        return (
            "pilot-only",
            format!("{n} matched cells (< {MIN_MATCHED_CELLS}): supports no default"),
        );
    }
    if !all("Q_quality_no_loss") {
        return (
            "not-recommended",
            format!(
                "accepted delta {:+.3} per matched cell (quality loss against native)",
                p["accepted_delta_per_cell"].as_f64().unwrap_or(0.0)
            ),
        );
    }
    if all("C_cost_reduction_20pct") && all("T_latency_within_2000ms") {
        return ("qualified-scoped", "quality not worse, total tokens (skill, retrieval and retries included) down at least 20%, latency within bound".into());
    }
    ("available-no-lift", "no measurable lift on this class and model: stays available, no automatic-performance claim".into())
}

pub fn summarize(rows: &[Value], meta: &Value) -> Value {
    let ts = parse(rows);
    let mut groups: BTreeMap<(String, String, String), Vec<&T>> = BTreeMap::new();
    for t in &ts {
        groups
            .entry((t.model.clone(), t.class.clone(), t.arm.clone()))
            .or_default()
            .push(t);
    }
    let mut results = Map::new();
    let mut cmp = vec![];
    for ((model, class, arm), v) in &groups {
        let mut o = group_stats(v);
        o["per_task_accepted"] = per_task(v);
        results
            .entry(model.clone())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("obj")
            .entry(class.clone())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("obj")
            .insert(arm.clone(), o);
        if arm != BASELINE_ARM {
            if let Some(base) =
                groups.get(&(model.clone(), class.clone(), BASELINE_ARM.to_string()))
            {
                let p = paired(v, base);
                let not_ok = v.iter().filter(|t| !t.ok()).count();
                let role = v.first().map(|t| t.role.as_str()).unwrap_or("");
                let (verdict, why) = verdict(role, &p, not_ok);
                let size = v.first().map(|t| t.size.clone()).unwrap_or_default();
                cmp.push(json!({"model": model, "size": size, "class": class, "arm": arm, "role": role, "paired_vs_native": p, "verdict": verdict, "reason": why}));
            }
        }
    }
    json!({"schema": SUMMARY_SCHEMA, "meta": meta, "trials_total": ts.len(), "by_model_class_arm": results, "comparisons": cmp,
           "cascade_mixed": cascade(&ts), "cell_size_check": cell_sizes(&ts)})
}

/// Repetitions per (task, arm, model) cell; below the minimum the whole report is a pilot.
pub fn cell_sizes(ts: &[T]) -> Value {
    let mut m: BTreeMap<(&str, &str, &str), u64> = BTreeMap::new();
    for t in ts.iter().filter(|t| t.ok()) {
        *m.entry((t.task.as_str(), t.arm.as_str(), t.model.as_str()))
            .or_default() += 1;
    }
    let min = m.values().copied().min();
    let mut by_model: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for ((_, _, model), n) in &m {
        let e = by_model.entry(model).or_insert((u64::MAX, 0));
        e.0 = e.0.min(*n);
        e.1 = e.1.max(*n);
    }
    let by_model: Value = by_model
        .into_iter()
        .map(|(k, (lo, hi))| (k.to_string(), json!({"min": lo, "max": hi, "label": if lo >= MIN_TRIALS_PER_CELL { "matched-trials" } else { "pilot" }})))
        .collect::<Map<String, Value>>()
        .into();
    json!({"by_model": by_model, "min_trials_per_cell": min, "max_trials_per_cell": m.values().copied().max(), "cells": m.len(),
           "label": if min.is_some_and(|x| x >= MIN_TRIALS_PER_CELL) { "matched-trials" } else { "pilot" }, "required_for_non_pilot": MIN_TRIALS_PER_CELL})
}

type CascadeRows = BTreeMap<(String, String), Vec<(bool, f64, f64, bool)>>;

/// Derived small-then-large cascade from the paired trials of the two sizes
/// (same task, arm, repetition): the large model is asked only after the small
/// one failed the grader. A derivation, not an independent run.
pub fn cascade(ts: &[T]) -> Value {
    let mut rows: CascadeRows = BTreeMap::new();
    let mut idx: BTreeMap<(&str, &str, u64, &str), &T> = BTreeMap::new();
    for t in ts.iter().filter(|t| t.ok()) {
        idx.insert((t.task.as_str(), t.arm.as_str(), t.rep, t.size.as_str()), t);
    }
    for ((task, arm, rep, size), s) in &idx {
        if *size != "small" {
            continue;
        }
        if let Some(l) = idx.get(&(*task, *arm, *rep, "large")) {
            let esc = !s.passed;
            let pass = s.passed || l.passed;
            let tok = s.tokens() + if esc { l.tokens() } else { 0.0 };
            let usd = l.cost.unwrap_or(0.0) * esc as u8 as f64 + s.cost.unwrap_or(0.0);
            rows.entry((s.class.clone(), arm.to_string()))
                .or_default()
                .push((pass, tok, usd, esc));
        }
    }
    let mut out = vec![];
    for ((class, arm), v) in rows {
        let n = v.len();
        out.push(json!({"class": class, "arm": arm, "pairs": n, "accepted": rate(v.iter().filter(|x| x.0).count(), n),
            "escalation_rate": r4(v.iter().filter(|x| x.3).count() as f64 / n as f64),
            "mean_tokens_o200k": mean(&v.iter().map(|x| x.1).collect::<Vec<_>>()).map(r4),
            "mean_billed_usd": mean(&v.iter().map(|x| x.2).collect::<Vec<_>>()).map(r4),
            "label": "derived from paired small and large trials; not an independent run"}));
    }
    json!(out)
}

/// Scoped recommendations: one entry per (arm, class, model). A recommendation
/// is advice for a reviewer; it activates nothing.
pub fn recommendations(summary: &Value) -> Value {
    let mut entries = vec![];
    for c in summary["comparisons"].as_array().into_iter().flatten() {
        let v = c["verdict"].as_str().unwrap_or("");
        let arm = c["arm"].as_str().unwrap_or("");
        let class = c["class"].as_str().unwrap_or("");
        let qualified = v == "qualified-scoped";
        let skill_key = match arm {
            "ponytail" => Some(json!({"ponytail": "full"})),
            "caveman" => Some(json!({"caveman": "on"})),
            "ponytail+caveman" => Some(json!({"preset": "efficient-coding"})),
            _ => None,
        };
        entries.push(json!({
            "arm": arm, "task_class": class, "model": c["model"], "model_size": c["size"], "verdict": v, "reason": c["reason"],
            "matched_cells": c["paired_vs_native"]["matched_cells"], "qualified": qualified,
            "automatic_default": false,
            "hn06_skills_proposal": if qualified { skill_key.map(|k| json!({"file": "semaprax.harness.toml", "table": "skills", "keys": k, "scope": format!("projects dominated by `{class}` work; the host's per-family selection still decides per task"), "applied": false})).unwrap_or(Value::Null) } else { Value::Null },
            "hn16_evidence": "outcomes.json rows (origin real, verified by the immutable grader); consumed only through an explicit evidence import, never a hidden switch",
        }));
    }
    json!({"schema": RECOMMENDATION_SCHEMA, "entries": entries,
           "contract": "advisory only: no entry changes configuration, routing or skill state; qualified entries are inputs for the HN-05 lock, HN-06 skill preset and HN-16 evidence contracts, applied by a user or reviewer"})
}

/// Per-trial verified outcomes, shaped like the HN-16 evidence outcome (cost unknown stays null).
pub fn outcomes(rows: &[Value]) -> Value {
    let list: Vec<Value> = rows
        .iter()
        .filter(|v| v["schema"] == super::trial::TRIAL_SCHEMA && v["status"] == "ok")
        .map(|v| {
            let skill_tokens = v["skill"]["tokens_o200k"].as_u64().unwrap_or(0);
            json!({"item": format!("{}#{}", v["task"].as_str().unwrap_or(""), v["rep"]), "arm": v["arm"], "model": v["model"], "origin": "real",
                   "verified_by": "apptask-immutable-grader/v1", "completed": v["passed"], "regressions": v["tamper_attempts"],
                   "attempts": v["totals"]["attempts"], "cost_usd": v["totals"]["cost_usd"], "latency_ms": v["totals"]["completion_ms"],
                   "skill_tokens_o200k_charged": skill_tokens, "retry_owner": "host"})
        })
        .collect();
    json!({"schema": "semaprax.harness-bench-outcomes.v1", "note": "convertible to decision::Outcome (cost_usd to cost_micros); unknown cost is null, never zero", "outcomes": list})
}
