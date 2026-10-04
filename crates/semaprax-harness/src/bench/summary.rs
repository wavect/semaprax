//! Machine summary: per-profile and per-task statistics, matched comparisons
//! against the baseline, and the stage reconciliation check.
//!
//! Headline = end-to-end paired payload bytes (baseline vs final model-visible
//! envelope) from `observe::build_report`; incurred requests are subtracted
//! once. Nothing here averages incompatible percentages: every figure is a
//! sum of bytes over an explicit cell set, with n shown.

use super::measure::{mean, percentile, r4, stdev, wilson95};
use super::run::RunOutput;
use crate::observe::{build_report, HostTraffic, Observation};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

fn u(v: &Value, path: &[&str]) -> u64 {
    path.iter().fold(v, |a, k| &a[*k]).as_u64().unwrap_or(0)
}

fn cells_of<'a>(cells: &'a [Value], profile: &str) -> Vec<&'a Value> {
    cells.iter().filter(|c| c["profile"] == profile).collect()
}

fn lat(cs: &[&Value], warm: bool) -> Value {
    let xs: Vec<u64> = cs
        .iter()
        .filter(|c| c["status"] != "untested" && (c["temperature"] == "warm") == warm)
        .map(|c| u(c, &["latency_ms"]))
        .collect();
    json!({"n": xs.len(), "p50": percentile(&xs, 50), "p95": percentile(&xs, 95),
           "stdev": stdev(&xs.iter().map(|x| *x as f64).collect::<Vec<_>>()).map(r4)})
}

/// Check that stage-level figures reconcile with the end-to-end headline and
/// with an independent sum over the raw cells. Returns human-readable mismatches.
pub fn reconcile(cells: &[&Value], report: &Value) -> Vec<String> {
    let mut bad = vec![];
    let cell_base: u64 = cells
        .iter()
        .filter(|c| c["status"] != "untested")
        .map(|c| u(c, &["bytes", "baseline"]))
        .sum();
    let cell_final: u64 = cells
        .iter()
        .filter(|c| c["status"] != "untested")
        .map(|c| u(c, &["bytes", "final_paired"]))
        .sum();
    for g in report["groups"].as_array().into_iter().flatten() {
        let e2e = g["end_to_end_reduction"].as_i64().unwrap_or(0);
        let stage_sum: i64 = g["stage_local"].as_object().map_or(0, |m| {
            m.values()
                .map(|s| s["reduction"].as_i64().unwrap_or(0))
                .sum()
        });
        if stage_sum != e2e {
            bad.push(format!(
                "stage-local reductions sum to {stage_sum} but end-to-end is {e2e}"
            ));
        }
        let (b, f) = (
            g["baseline"].as_i64().unwrap_or(0),
            g["final"].as_i64().unwrap_or(0),
        );
        if b - f != e2e {
            bad.push(format!("baseline {b} - final {f} != end-to-end {e2e}"));
        }
        if b as u64 != cell_base || f as u64 != cell_final {
            bad.push(format!(
                "observer totals ({b}, {f}) differ from raw cell sums ({cell_base}, {cell_final})"
            ));
        }
        let net = g["net_savings"].as_i64().unwrap_or(0);
        let inc = g["incurred"]["total"].as_i64().unwrap_or(0);
        if net != e2e - inc {
            bad.push(format!("net {net} != end-to-end {e2e} - incurred {inc}"));
        }
    }
    bad
}

fn profile_summary(cells: &[&Value], events: &[Observation], workflow_runs: u64) -> Value {
    let run: Vec<&&Value> = cells.iter().filter(|c| c["status"] != "untested").collect();
    let n = run.len() as u64;
    let accepted = run.iter().filter(|c| c["accepted"] == true).count() as u64;
    let (lo, hi) = wilson95(accepted, n);
    let visible: u64 = run.iter().map(|c| u(c, &["bytes", "model_visible"])).sum();
    let incurred: u64 = run.iter().map(|c| u(c, &["bytes", "incurred"])).sum();
    let (ft, ff): (u64, u64) = run.iter().fold((0, 0), |a, c| {
        (
            a.0 + u(c, &["facts", "total"]),
            a.1 + u(c, &["facts", "found"]),
        )
    });
    let fneg: u64 = run
        .iter()
        .map(|c| u(c, &["false_negatives", "count"]))
        .sum();
    let sum = |k: &str| run.iter().map(|c| u(c, &[k])).sum::<u64>();
    let res = |k: &str| -> Value {
        let xs: Vec<u64> = run
            .iter()
            .filter_map(|c| c["resources"][k].as_u64())
            .collect();
        if xs.is_empty() {
            Value::Null
        } else {
            json!({"sum": xs.iter().sum::<u64>(), "max": xs.iter().max(), "cells": xs.len()})
        }
    };
    let report = build_report(
        events,
        0,
        Some(HostTraffic {
            observed: events.len() as u64,
            unobserved: workflow_runs,
        }),
    )
    .json;
    let cmd_exec_bad = run
        .iter()
        .filter(|c| c["command"]["executions"].as_u64().is_some_and(|e| e != 1))
        .count();
    let provider_used = run
        .iter()
        .filter(|c| c["command"]["route"] == "provider")
        .count();
    let cmd_cells = run
        .iter()
        .filter(|c| c["command"]["route"].is_string())
        .count();
    let by_status = |s: &str| cells.iter().filter(|c| c["status"] == s).count();
    json!({
        "cells": {"total": cells.len(), "ok": by_status("ok"), "failed": by_status("failed"), "untested": by_status("untested")},
        "accepted": {"count": accepted, "n": n, "rate": r4(accepted as f64 / n.max(1) as f64), "wilson95": [r4(lo), r4(hi)]},
        "critical_fact_retention": if ft == 0 { Value::Null } else { json!(r4(ff as f64 / ft as f64)) },
        "false_negatives": fneg,
        "bytes": {"unit": "byte-v1", "model_visible": visible, "incurred": incurred,
                  "per_accepted_task": if accepted == 0 { Value::Null } else { json!(r4(visible as f64 / accepted as f64)) },
                  "note": "byte-only: no named tokenizer is available on this machine"},
        "calls": sum("calls"), "retries": sum("retries"), "detail_retrievals": sum("detail_retrievals"),
        "latency_ms": {"warm": lat(cells, true), "cold": lat(cells, false)},
        "resources": {"user_ms": res("user_ms"), "sys_ms": res("sys_ms"), "max_rss_kb": res("max_rss_kb"),
                      "disk_bytes": run.iter().map(|c| u(c, &["disk_bytes"])).max()},
        "command": {"cells": cmd_cells, "provider_route_cells": provider_used, "executions_not_one": cmd_exec_bad},
        "observation_report": report,
        "reconciliation_mismatches": reconcile(cells, &report),
    })
}

/// Matched comparison of one profile against the baseline on the (task, trial)
/// pairs both ran. Positive `visible_bytes_delta` = the profile showed more.
fn matched(base: &[&Value], prof: &[&Value]) -> Value {
    let key = |c: &Value| format!("{}|{}", c["task"].as_str().unwrap_or(""), c["trial"]);
    let bm: BTreeMap<String, &Value> = base
        .iter()
        .filter(|c| c["status"] != "untested")
        .map(|c| (key(c), *c))
        .collect();
    let (mut n, mut d_acc, mut d_vis, mut d_lat) = (0u64, 0i64, 0i64, 0i64);
    let mut tasks = BTreeSet::new();
    for c in prof.iter().filter(|c| c["status"] != "untested") {
        if let Some(b) = bm.get(&key(c)) {
            n += 1;
            tasks.insert(c["task"].as_str().unwrap_or("").to_string());
            d_acc += (c["accepted"] == true) as i64 - (b["accepted"] == true) as i64;
            d_vis +=
                u(c, &["bytes", "model_visible"]) as i64 - u(b, &["bytes", "model_visible"]) as i64;
            d_lat += u(c, &["latency_ms"]) as i64 - u(b, &["latency_ms"]) as i64;
        }
    }
    let m = |x: i64| {
        if n == 0 {
            Value::Null
        } else {
            json!(r4(x as f64 / n as f64))
        }
    };
    json!({"matched_cells": n, "tasks": tasks, "accepted_delta_per_cell": m(d_acc),
           "visible_bytes_delta_per_cell": m(d_vis), "latency_ms_delta_per_cell": m(d_lat)})
}

/// Two explicit acceptance cells: the development workflow itself querying an
/// external context provider (workflow_context), and an adopted skill.
fn acceptance_cells(cells: &[Value]) -> Value {
    let mut ctx = vec![];
    let mut skill = serde_json::Map::new();
    for c in cells
        .iter()
        .filter(|c| c["status"] == "ok" && c["trial"] == 1)
    {
        let w = &c["workflow_context"];
        if w.is_object()
            && w["providers"].as_array().is_some_and(|p| {
                p.iter()
                    .any(|x| !x.as_str().unwrap_or("").starts_with("semaprax/"))
            })
        {
            ctx.push(json!({"profile": c["profile"], "task": c["task"], "workflow_context": w,
                            "context_step_visible_bytes": c["bytes"]["final_paired"], "accepted": c["accepted"]}));
        }
    }
    for c in cells
        .iter()
        .filter(|c| c["status"] == "ok" && c["skill"].is_object())
    {
        let e = skill.entry(c["profile"].as_str().unwrap_or("").to_string()).or_insert_with(|| json!({
            "cells": 0, "skill_cells": 0, "prompt_bytes": null, "loaded": null, "workflow_cells": 0, "workflow_accepted": 0,
            "manifest_unchanged_everywhere": true}));
        e["cells"] = json!(e["cells"].as_u64().unwrap_or(0) + 1);
        if c["skill"]["prompt_bytes"].is_u64() {
            e["skill_cells"] = json!(e["skill_cells"].as_u64().unwrap_or(0) + 1);
            e["prompt_bytes"] = c["skill"]["prompt_bytes"].clone();
            e["loaded"] = c["skill"]["loaded"].clone();
        }
        if c["workflow_status"].is_string() {
            e["workflow_cells"] = json!(e["workflow_cells"].as_u64().unwrap_or(0) + 1);
            e["workflow_accepted"] = json!(
                e["workflow_accepted"].as_u64().unwrap_or(0) + (c["accepted"] == true) as u64
            );
            if c["checks"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|k| k["kind"] == "no-dependency-change" && k["pass"] != true)
            {
                e["manifest_unchanged_everywhere"] = json!(false);
            }
        }
    }
    json!({"workflow_external_context": ctx, "skills": skill})
}

pub fn build(out: &RunOutput, baseline: &str, corpus_digest: &str, trials: (u32, u32)) -> Value {
    let profiles: BTreeSet<String> = out
        .cells
        .iter()
        .filter_map(|c| c["profile"].as_str().map(str::to_string))
        .collect();
    let base_cells = cells_of(&out.cells, baseline);
    let mut per = Map::new();
    let mut cmp = Map::new();
    let mut by_task = Map::new();
    for p in &profiles {
        let cs = cells_of(&out.cells, p);
        let wf = cs
            .iter()
            .filter(|c| c["workflow_status"].is_string())
            .count() as u64;
        let empty = vec![];
        let ev = out.events.get(p).unwrap_or(&empty);
        let mut s = profile_summary(&cs, ev, wf);
        s["permission_changed_during_run"] =
            json!(out.permission_changed.get(p).copied().unwrap_or(false));
        s["untested_reason"] = out
            .untested_profiles
            .get(p)
            .map_or(Value::Null, |r| json!(r));
        per.insert(p.clone(), s);
        if p != baseline {
            cmp.insert(p.clone(), matched(&base_cells, &cs));
        }
        let mut tasks = Map::new();
        let ids: BTreeSet<&str> = cs.iter().filter_map(|c| c["task"].as_str()).collect();
        for t in ids {
            let tc: Vec<&&Value> = cs
                .iter()
                .filter(|c| c["task"] == t && c["status"] != "untested")
                .collect();
            let vis: Vec<f64> = tc
                .iter()
                .map(|c| u(c, &["bytes", "model_visible"]) as f64)
                .collect();
            tasks.insert(t.to_string(), json!({
                "family": cs.iter().find(|c| c["task"] == t).map_or(Value::Null, |c| c["family"].clone()),
                "n": tc.len(), "accepted": tc.iter().filter(|c| c["accepted"] == true).count(),
                "visible_bytes_mean": mean(&vis).map(r4), "visible_bytes_stdev": stdev(&vis).map(r4),
                "status": cs.iter().filter(|c| c["task"] == t).map(|c| c["status"].as_str().unwrap_or("")).collect::<BTreeSet<_>>(),
                "reason": cs.iter().find(|c| c["task"] == t && c["reason"].is_string()).map_or(Value::Null, |c| c["reason"].clone()),
            }));
        }
        by_task.insert(p.clone(), Value::Object(tasks));
    }
    json!({
        "schema": "semaprax.harness-benchmark-summary.v1",
        "corpus_digest": corpus_digest,
        "baseline_profile": baseline,
        "trials": {"cold": trials.0, "warm": trials.1},
        "measurement": {"unit": "byte-v1", "tokenizer": "byte_only",
                        "named_tokenizer": "unavailable on this machine; no token counts are reported"},
        "profiles": per,
        "vs_baseline": cmp,
        "per_task": by_task,
        "acceptance_cells": acceptance_cells(&out.cells),
        "adversarial": out.adversarial.iter().map(|a| a.to_json()).collect::<Vec<_>>(),
    })
}
