//! Decision gates. The thresholds below are declared in
//! `docs/HARNESS-BENCHMARK-V1.md` BEFORE any result existed and are not tuned
//! afterwards; a test pins the document and these constants together.
//!
//! A gate verdict is "passed on this evidence", never a proof of safety: zero
//! observed failures over n trials bounds nothing about unseen inputs.

use super::measure::r4;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// Minimum matched, independent trials per (profile, task family) scope.
pub const MIN_MATCHED_CELLS: u64 = 10;
/// Task-quality tolerance: accepted rate may not drop at all (points per cell).
pub const QUALITY_TOLERANCE: f64 = 0.0;
/// Required reduction in model-visible bytes (incl. incurred) vs baseline.
pub const MIN_NET_BYTE_REDUCTION: f64 = 0.20;
/// Largest tolerated added end-to-end latency per cell (cold included).
pub const MAX_ADDED_LATENCY_MS: f64 = 2000.0;

pub const GATES: [(&str, &str); 7] = [
    ("G1", "zero protected-fact loss: every seeded adversarial case detected and zero false negatives in measured cells"),
    ("G2", "no permission widening: stored grants unchanged during cells and the widening case detected"),
    ("G3", "no hidden command replay: every command cell shows exactly one execution and the double-execution case is detected"),
    ("G4", "task quality: accepted-rate delta vs baseline >= -0.0 over >= 10 matched cells"),
    ("G5", "net cost: model-visible bytes including incurred requests fall >= 20% vs baseline"),
    ("G6", "latency: added end-to-end latency (cold and warm trials together) <= 2000 ms per cell"),
    ("G7", "tested: the scope ran with the real tool (no untested or failed cell)"),
];

fn u(v: &Value, path: &[&str]) -> f64 {
    path.iter().fold(v, |a, k| &a[*k]).as_f64().unwrap_or(0.0)
}

fn gate(id: &str, pass: bool, detail: String) -> Value {
    json!({"id": id, "pass": pass, "detail": detail})
}

/// Evaluate every non-baseline profile per task family.
pub fn evaluate(cells: &[Value], adversarial: &[Value], summary: &Value, baseline: &str) -> Value {
    let adv_ok = |kind: &str| {
        adversarial
            .iter()
            .filter(|a| a["kind"] == kind)
            .all(|a| a["detected"] == true && a["untested"].is_null())
    };
    let adv_all = !adversarial.is_empty()
        && adversarial
            .iter()
            .all(|a| a["detected"] == true && a["untested"].is_null());
    let mut out = Map::new();
    let profiles: BTreeSet<&str> = cells
        .iter()
        .filter_map(|c| c["profile"].as_str())
        .filter(|p| *p != baseline)
        .collect();
    for p in profiles {
        let psum = &summary["profiles"][p];
        let mut scopes = Map::new();
        let fams: BTreeSet<&str> = cells
            .iter()
            .filter(|c| c["profile"] == p)
            .filter_map(|c| c["family"].as_str())
            .collect();
        for fam in fams {
            let prof: Vec<&Value> = cells
                .iter()
                .filter(|c| c["profile"] == p && c["family"] == fam)
                .collect();
            let key = |c: &Value| format!("{}|{}", c["task"].as_str().unwrap_or(""), c["trial"]);
            let base: std::collections::BTreeMap<String, &Value> = cells
                .iter()
                .filter(|c| {
                    c["profile"] == baseline && c["family"] == fam && c["status"] != "untested"
                })
                .map(|c| (key(c), c))
                .collect();
            let (mut n, mut acc, mut vis_p, mut vis_b, mut lat) = (0u64, 0.0, 0.0, 0.0, 0.0);
            let mut fneg = 0.0;
            let (mut bad_status, mut cmd_bad) = (0u64, 0u64);
            for c in &prof {
                if c["status"] != "ok" {
                    bad_status += 1;
                    continue;
                }
                fneg += u(c, &["false_negatives", "count"]);
                if c["command"]["executions"].as_u64().is_some_and(|e| e != 1) {
                    cmd_bad += 1;
                }
                if let Some(b) = base.get(&key(c)) {
                    n += 1;
                    acc += (c["accepted"] == true) as i32 as f64
                        - (b["accepted"] == true) as i32 as f64;
                    vis_p += u(c, &["bytes", "model_visible"]);
                    vis_b += u(b, &["bytes", "model_visible"]);
                    lat += u(c, &["latency_ms"]) - u(b, &["latency_ms"]);
                }
            }
            let nf = n.max(1) as f64;
            let reduction = if vis_b > 0.0 {
                1.0 - vis_p / vis_b
            } else {
                0.0
            };
            let perm = psum["permission_changed_during_run"] == true;
            let gates = vec![
                gate(
                    "G1",
                    adv_all && fneg == 0.0,
                    format!(
                        "adversarial all detected: {adv_all}; false negatives in scope: {fneg}"
                    ),
                ),
                gate(
                    "G2",
                    !perm && adv_ok("permission_widening"),
                    format!("grants changed during run: {perm}"),
                ),
                gate(
                    "G3",
                    cmd_bad == 0 && adv_ok("double_execution"),
                    format!("command cells with executions != 1: {cmd_bad}"),
                ),
                gate(
                    "G4",
                    n >= MIN_MATCHED_CELLS && acc / nf >= -QUALITY_TOLERANCE,
                    format!(
                        "matched cells {n}; accepted delta per cell {:+.4}",
                        acc / nf
                    ),
                ),
                gate(
                    "G5",
                    n >= MIN_MATCHED_CELLS && reduction >= MIN_NET_BYTE_REDUCTION,
                    format!(
                        "visible-byte reduction {:+.1}% over {n} matched cells",
                        reduction * 100.0
                    ),
                ),
                gate(
                    "G6",
                    n >= MIN_MATCHED_CELLS && lat / nf <= MAX_ADDED_LATENCY_MS,
                    format!("added latency {:+.0} ms per matched cell", lat / nf),
                ),
                gate(
                    "G7",
                    bad_status == 0 && !prof.is_empty(),
                    format!("{bad_status} of {} cells untested or failed", prof.len()),
                ),
            ];
            let eligible = gates.iter().all(|g| g["pass"] == true);
            scopes.insert(
                fam.to_string(),
                json!({"matched_cells": n, "accepted_delta_per_cell": r4(acc / nf), "visible_byte_reduction": r4(reduction),
                       "added_latency_ms_per_cell": r4(lat / nf), "gates": gates, "auto_enable_eligible": eligible}),
            );
        }
        out.insert(p.to_string(), Value::Object(scopes));
    }
    json!({"gates_declared": GATES.iter().map(|(i, d)| json!({"id": i, "rule": d})).collect::<Vec<_>>(),
           "adversarial_all_detected": adv_all, "scopes": out,
           "caveat": "A passed gate is evidence on this corpus and these trials only; zero observed failures is not proof of safety."})
}
