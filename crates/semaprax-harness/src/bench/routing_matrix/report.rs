//! The MR-13 run manifest (`semaprax.harness-routing-matrix.v1`): pins,
//! budgets, hardware/backend, the real/fixture/unavailable matrix, verifier
//! identities, billable usage, cold/warm latency, fallback/escalation rates,
//! calibration (raw option calibration kept apart from downstream success),
//! gate decisions and activation. Deterministic for a given input set.

use super::registry::{ArmKind, Registry, RouterPrice};
use super::run::{latency_split, Cell, MatrixRun};
use super::{Split, Stratum, TaskSet, MANIFEST_SCHEMA};
use crate::decision::evidence::Origin;
use crate::decision::qualify::DomainGateSpec;
use crate::decision::route_v2::ExecutionDomain;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const CLAIMS: [&str; 4] = [
    "results cover only the sampled tasks, strata, partitions and the declared hardware/backend",
    "fixture cells are contract fixtures, never runs; unavailable cells are neither successes nor zero-cost",
    "option-mass calibration is reported apart from, and is not, a probability of downstream task success",
    "a no-go keeps rules active; no saving is claimed for an arm that did not qualify",
];

/// Upper bound of billable spend for a real run of this matrix: every
/// (item, catalog model) at the matched cost ceiling plus every learned
/// router call at its reserved ceiling.
pub fn cost_ceiling_micros(reg: &Registry, tasks: &TaskSet) -> u64 {
    let models = tasks.catalog.as_array().map_or(0, Vec::len) as u64;
    let items = tasks.items.len() as u64;
    let routers: u64 = reg
        .learned
        .iter()
        .map(|l| match l.router_price {
            RouterPrice::NonBilled => 0,
            RouterPrice::Priced {
                max_call_micros, ..
            } => max_call_micros,
        })
        .sum();
    items.saturating_mul(
        models
            .saturating_mul(tasks.matched.max_cost_micros)
            .saturating_add(routers),
    )
}

fn origin_counts(cells: &[&Cell]) -> Value {
    let n = |o: Origin| cells.iter().filter(|c| c.origin == o).count();
    json!({"real": n(Origin::Real), "fixture": n(Origin::Fixture), "unavailable": n(Origin::Unavailable)})
}

fn rate(num: usize, den: usize) -> Value {
    if den == 0 {
        Value::Null
    } else {
        json!(num as f64 / den as f64)
    }
}

/// Metrics over one arm's cells. Total cost per accepted task includes
/// failures, retries and router overhead; it is `null` when any executed
/// cell's cost is unknown or nothing was accepted.
pub fn metrics(cells: &[&Cell]) -> Value {
    let ex: Vec<&&Cell> = cells
        .iter()
        .filter(|c| c.origin != Origin::Unavailable)
        .collect();
    let accepted = ex
        .iter()
        .filter(|c| c.completed && c.regressions == 0)
        .count();
    let unknown = ex.iter().filter(|c| c.cost_micros.is_none()).count();
    let total: u64 = ex
        .iter()
        .map(|c| c.cost_micros.unwrap_or(0) + c.router.micros)
        .sum();
    let router: u64 = ex.iter().map(|c| c.router.micros).sum();
    json!({
        "cells": cells.len(), "executed": ex.len(), "accepted": accepted,
        "origins": origin_counts(cells),
        "total_cost_micros": total, "router_cost_micros": router, "unknown_cost_cells": unknown,
        "cost_per_accepted_task_micros": (unknown == 0 && accepted > 0).then(|| total as f64 / accepted as f64),
        "failed_attempts": ex.iter().map(|c| c.failed_attempts).sum::<u32>(),
        "gateway_retries": ex.iter().map(|c| c.gateway_retries).sum::<u32>(),
        "fallback_rate": rate(ex.iter().filter(|c| c.fallback).count(), ex.len()),
        "escalation_rate": rate(ex.iter().filter(|c| c.attempts > 1).count(), ex.len()),
        "latency": latency_split(cells),
        "unreconciled_cells": ex.iter().filter(|c| !c.errors.is_empty()).count(),
        "forged_origin_cells": ex.iter().filter(|c| c.forged).count(),
    })
}

fn cell_json(c: &Cell) -> Value {
    json!({"item": c.item, "domain": c.domain.as_str(), "partition": c.partition,
           "stratum": c.stratum.as_str(), "split": c.split.as_str(), "arm": c.arm,
           "model": c.model, "origin": c.origin.as_str(), "verified_by": c.verified_by,
           "completed": c.completed, "regressions": c.regressions, "attempts": c.attempts,
           "failed_attempts": c.failed_attempts, "cost_micros": c.cost_micros,
           "router": c.router.to_json(), "latency_ms": c.latency_ms, "cold": c.cold,
           "cache": c.cache, "source": c.source, "shadow": c.shadow,
           "rules_choice": c.rules_choice, "chosen_score": c.chosen_score,
           "forged": c.forged, "errors": c.errors, "unavailable": c.unavailable})
}

pub fn gate_spec_json(s: &DomainGateSpec) -> Value {
    json!({"domain": s.domain.as_str(), "version": s.version, "reviewed": s.reviewed,
           "min_items": s.spec.min_items, "completion_margin": s.spec.completion_margin,
           "min_cost_saving": s.spec.min_cost_saving,
           "max_extra_regressions": s.spec.max_extra_regressions,
           "max_latency_ratio": s.spec.max_latency_ratio, "digest": s.digest()})
}

pub struct ManifestInputs<'a> {
    pub reg: &'a Registry,
    pub tasks: &'a TaskSet,
    pub specs: &'a [DomainGateSpec],
    pub mode: &'a str,
    pub hardware: &'a str,
    pub pins: &'a BTreeMap<String, String>,
    pub sessions: &'a Value,
}

pub fn manifest(run: &MatrixRun, m: &ManifestInputs) -> Value {
    let domains: BTreeSet<ExecutionDomain> = m.tasks.items.iter().map(|i| i.domain).collect();
    let (mut matrix, mut met, mut shadow, mut active) =
        (Map::new(), Map::new(), Map::new(), Map::new());
    for d in &domains {
        let (mut dm, mut dmet, mut dsh) = (Map::new(), Map::new(), Map::new());
        for arm in run.arms.iter().filter(|a| a.domains.contains(d)) {
            let mine: Vec<&Cell> = run
                .cells
                .iter()
                .filter(|c| c.domain == *d && c.arm == arm.id)
                .collect();
            let eval: Vec<&Cell> = mine
                .iter()
                .copied()
                .filter(|c| c.split == Split::Eval)
                .collect();
            let mut by_stratum = Map::new();
            for s in Stratum::ALL {
                let cs: Vec<&Cell> = eval.iter().copied().filter(|c| c.stratum == *s).collect();
                if !cs.is_empty() {
                    by_stratum.insert(s.as_str().into(), origin_counts(&cs));
                }
            }
            let calib: Vec<&Cell> = mine
                .iter()
                .copied()
                .filter(|c| c.split == Split::Calibration)
                .collect();
            dm.insert(
                arm.id.clone(),
                json!({"status": if arm.unavailable.is_some() { "unavailable" } else { "available" },
                       "reason": arm.unavailable, "eval": origin_counts(&eval),
                       "calibration": origin_counts(&calib), "eval_by_stratum": by_stratum}),
            );
            let gated: Vec<&Cell> = eval.iter().copied().filter(|c| !c.shadow).collect();
            let mut by_part = Map::new();
            let parts: BTreeSet<&str> = gated.iter().map(|c| c.partition.as_str()).collect();
            for p in parts {
                let cs: Vec<&Cell> = gated.iter().copied().filter(|c| c.partition == p).collect();
                by_part.insert(p.into(), metrics(&cs));
            }
            let mut mm = metrics(&gated);
            mm["by_partition"] = Value::Object(by_part);
            dmet.insert(arm.id.clone(), mm);
            if matches!(arm.kind, ArmKind::Learned(_)) {
                let sh: Vec<&Cell> = eval.iter().copied().filter(|c| c.shadow).collect();
                let agrees = sh
                    .iter()
                    .filter(|c| c.model.is_some() && c.model == c.rules_choice)
                    .count();
                dsh.insert(
                    arm.id.clone(),
                    json!({"cells": sh.len(), "agrees_with_rules": agrees, "metrics": metrics(&sh),
                           "note": "shadow recommendations never routed; automatic routing is prohibited for these strata"}),
                );
            }
        }
        matrix.insert(d.as_str().into(), Value::Object(dm));
        met.insert(d.as_str().into(), Value::Object(dmet));
        shadow.insert(d.as_str().into(), Value::Object(dsh));
        let lock = run.stores.get(d).and_then(|s| s.active().cloned());
        active.insert(
            d.as_str().into(),
            match lock {
                Some(l) => json!({"mode": "qualified-auto", "key_digest": l.key_digest, "record_digest": l.record_digest}),
                None => json!({"mode": "rules", "reason": "no profile qualified for this domain; rules stay active"}),
            },
        );
    }
    let usage = |o: Origin| {
        let cs: Vec<&Cell> = run.cells.iter().filter(|c| c.origin == o).collect();
        // Shared executions are counted once per (item, model).
        let mut seen = BTreeSet::new();
        let (mut i, mut r, mut out, mut billed, mut unknown) = (0u64, 0u64, 0u64, 0u64, 0usize);
        for c in &cs {
            if !seen.insert((c.item.clone(), c.model.clone())) {
                continue;
            }
            i += c.tokens.input;
            r += c.tokens.cache_read;
            out += c.tokens.output;
            match c.cost_micros {
                Some(x) => billed += x,
                None => unknown += 1,
            }
        }
        let router: u64 = cs.iter().map(|c| c.router.micros).sum();
        json!({"executions": seen.len(), "input_tokens": i, "cache_read_tokens": r,
               "output_tokens": out, "generation_micros": billed, "router_micros": router,
               "unknown_cost_executions": unknown})
    };
    let gate: Vec<Value> = run
        .decisions
        .iter()
        .map(|d| {
            json!({"domain": d.domain.as_str(), "arm": d.arm, "status": d.status, "reasons": d.reasons,
                   "key": d.key.as_ref().map(|k| k.to_json()),
                   "key_digest": d.key.as_ref().map(|k| k.digest()),
                   "spec_digest": d.spec_digest,
                   "record_digest": d.decision.as_ref().map(|x| x.record_digest.clone()),
                   "metrics": d.decision.as_ref().map(|x| x.metrics.clone())})
        })
        .collect();
    let mut pins = json!({
        "registry": m.reg.digest, "tasks": m.tasks.digest, "seal": run.seal_digest,
        "candidate_revision": m.tasks.candidate_revision, "renderer_revision": m.tasks.renderer_revision,
        "executor": {"identity": run.executor, "class": run.executor_class.as_str()},
        "gate_specs": m.specs.iter().map(gate_spec_json).collect::<Vec<_>>(),
        "learned_profiles": m.reg.learned.iter().map(|l| json!({
            "arm": l.arm_id, "adapter": l.adapter.label(), "descriptor_digest": l.descriptor_digest,
            "model_profile_digest": l.model_profile.digest(), "instance": l.instance.instance_id,
            "decision_versions": l.versions})).collect::<Vec<_>>()});
    for (k, v) in m.pins {
        pins[k.as_str()] = json!(v);
    }
    json!({
        "schema": MANIFEST_SCHEMA,
        "mode": m.mode,
        "decision": run.overall(),
        "active": active,
        "pins": pins,
        "budgets": {"matched": {"max_cost_micros": m.tasks.matched.max_cost_micros,
                                "max_attempts": m.tasks.matched.max_attempts},
                    "real_run_cost_ceiling_micros": cost_ceiling_micros(m.reg, m.tasks)},
        "hardware": m.hardware,
        "adapter_sessions": m.sessions,
        "arms": run.arms.iter().map(|a| a.to_json()).collect::<Vec<_>>(),
        "excluded_decision_entries": m.reg.excluded,
        "verifiers": m.tasks.verifiers.values().map(|v| json!({"id": v.id, "kind": v.kind,
                     "revision": v.revision, "label": v.label()})).collect::<Vec<_>>(),
        "matrix": matrix,
        "metrics_eval": met,
        "shadow": shadow,
        "billable_usage": {"real": usage(Origin::Real),
                           "fixture": {"note": "fixture usage is a contract figure, never billable spend", "figures": usage(Origin::Fixture)}},
        "calibration": run.calibration,
        "gate": gate,
        "claims": CLAIMS,
        "cells": run.cells.iter().map(cell_json).collect::<Vec<_>>(),
    })
}

/// The compact decision document recorded beside the manifest.
pub fn gate_decision(manifest: &Value) -> Value {
    json!({"schema": "semaprax.harness-routing-gate-decision.v1",
           "decision": manifest["decision"], "active": manifest["active"],
           "gate": manifest["gate"], "gate_specs": manifest["pins"]["gate_specs"],
           "seal": manifest["pins"]["seal"], "matrix": manifest["matrix"],
           "claims": manifest["claims"]})
}
