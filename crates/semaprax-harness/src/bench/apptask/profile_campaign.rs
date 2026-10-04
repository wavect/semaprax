//! Profile-arm campaign runner (TC-12). Drives the existing task set and
//! immutable graders (`run_trial`) for each profile arm through an `ArmBackend`
//! that supplies a `TrialClient`: the production-harness model path
//! (`ProductionClient`) or the benchmark's raw loop (`RawClient`), and labels
//! each record with which one ran. Every planned trial is retained, including
//! unavailable, untested, failed and budget-aborted ones; none becomes a
//! success or a zero cost.

use super::cache_state::{self, CacheState, CacheTracker, RepoCache};
use super::model::{Metered, SpendLedger};
use super::production::{Attempt, TrialClient};
use super::profile_arms::{CampaignSpec, Policy, ProfileArm};
use super::trial::{run_trial, ModelSpec, TrialEnv, TrialKey};
use crate::receipt::Usage;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

pub const PROFILE_TRIAL_SCHEMA: &str = "semaprax.harness-profile-trial.v1";

/// Supplies the model client of one trial. `Err(reason)` is a missing model
/// or an unavailable tool: the trial is retained as `unavailable`.
pub trait ArmBackend: Sync {
    /// `real` (live provider) or `fixture` (deterministic local fake): only
    /// `real` evidence can support a promotion.
    fn origin(&self, model: &ModelSpec) -> &'static str;
    fn client<'a>(
        &'a self,
        arm: &ProfileArm,
        model: &ModelSpec,
        tracker: &'a CacheTracker,
        key: &TrialKey,
    ) -> Result<Box<dyn TrialClient + 'a>, String>;
}

/// Spend of one trial: unknown stays unknown; no dispatch is not zero spend.
pub struct Spend {
    pub micros: Option<u64>,
    pub complete: bool,
    pub dispatched: usize,
    pub basis: Vec<&'static str>,
}

pub fn spend_of(attempts: &[Attempt]) -> Spend {
    let d: Vec<&Attempt> = attempts.iter().filter(|a| a.dispatched).collect();
    let complete = !d.is_empty() && d.iter().all(|a| a.cost_micros.is_some());
    let mut basis: Vec<&'static str> = d.iter().map(|a| a.cost_basis).collect();
    basis.sort();
    basis.dedup();
    Spend {
        micros: complete.then(|| d.iter().filter_map(|a| a.cost_micros).sum()),
        complete,
        dispatched: d.len(),
        basis,
    }
}

fn base(
    key: &TrialKey,
    task_class: &str,
    arm: &ProfileArm,
    model: &ModelSpec,
    origin: &str,
) -> Value {
    json!({"schema": PROFILE_TRIAL_SCHEMA, "trial": key.id(), "task": key.task, "class": task_class,
           "arm": arm.id, "model": model.id, "rep": key.rep, "origin": origin,
           "profile_digest": arm.profile_digest(), "accepted": false, "first_pass": false,
           "attempts": [], "spend": {"micros": null, "complete": false, "dispatched": 0},
           "cache": {"provider": "none", "repo": if key.rep == 0 { "cold" } else { "warm" }}})
}

fn status_record(mut v: Value, status: &str, outcome: &str, reason: &str) -> Value {
    v["status"] = json!(status);
    v["outcome"] = json!(outcome);
    v["reason"] = json!(reason);
    v
}

/// Fold a finished apptask trial record and its attempt log into the profile record.
pub fn wrap(
    key: &TrialKey,
    class: &str,
    arm: &ProfileArm,
    model: &ModelSpec,
    origin: &str,
    path: &str,
    attempts: &[Attempt],
    observations: usize,
    overlays: Value,
    trial: Value,
) -> Value {
    let mut v = base(key, class, arm, model, origin);
    let st = trial["status"].as_str().unwrap_or("error").to_string();
    let accepted = st == "ok" && trial["passed"] == true;
    let outcome = match st.as_str() {
        "ok" if accepted => "accepted",
        "ok" => "failed",
        "budget" => "budget_aborted",
        "untested" => "untested",
        _ => "error",
    };
    let sp = spend_of(attempts);
    let states: Vec<CacheState> = attempts
        .iter()
        .filter(|a| a.dispatched)
        .map(|a| a.cache)
        .collect();
    v["status"] = json!(st);
    v["outcome"] = json!(outcome);
    v["reason"] = trial["reason"].clone();
    v["accepted"] = json!(accepted);
    v["first_pass"] = json!(trial["passed_first_attempt"] == true && accepted);
    v["path"] = json!(path);
    v["attempts"] = json!(attempts.iter().map(Attempt::to_json).collect::<Vec<_>>());
    v["attempt_count"] = json!(attempts.iter().filter(|a| a.dispatched).count());
    v["spend"] = json!({"micros": sp.micros, "complete": sp.complete, "dispatched": sp.dispatched,
                        "basis": sp.basis});
    v["cache"] = json!({"provider": cache_state::trial_label(&states),
                        "repo": (if key.rep == 0 { RepoCache::Cold } else { RepoCache::Warm }).as_str()});
    v["latency_ms"] = trial["totals"]["completion_ms"].clone();
    v["local_overhead_ms"] = trial["totals"]["harness_ms"].clone();
    v["tamper_attempts"] = trial["tamper_attempts"].clone();
    v["observations"] = json!(observations);
    v["overlays"] = overlays;
    v["pins"] = json!({"model": attempts.iter().find_map(|a| a.model_pin.clone())});
    v["trial_record"] = trial;
    v
}

/// Which overlays took effect in this trial and which could not, with the reason.
pub fn overlay_states(arm: &ProfileArm, path: &str, reports: &[Value]) -> Value {
    let mut v: Vec<Value> = vec![];
    for p in &arm.policies {
        let wire = matches!(
            p,
            Policy::Tiers | Policy::FeedbackAllowance | Policy::PromptRenderer
        );
        v.push(if wire && path != super::production::PATH_PRODUCTION {
            json!({"policy": p.id(), "state": "not-applicable",
                   "reason": "the raw model loop has no HostModel wire path; run through the production adapter"})
        } else {
            let mut o = json!({"policy": p.id(), "state": "applied"});
            if *p == Policy::FeedbackAllowance {
                o["projections"] = json!(reports.len());
            }
            o
        });
    }
    for (p, why) in &arm.not_applicable {
        v.push(json!({"policy": p.id(), "state": "not-applicable", "reason": why}));
    }
    for (p, why) in &arm.omitted {
        v.push(json!({"policy": p.id(), "state": "unavailable", "reason": why}));
    }
    json!(v)
}

pub struct RunSummary {
    pub recorded: usize,
    pub aborted: bool,
}

/// Run the campaign sequentially (bounded, rep-major) and append to `trials.jsonl`.
pub fn run(
    env: &TrialEnv,
    spec: &CampaignSpec,
    models: &[ModelSpec],
    backend: &dyn ArmBackend,
    ledger: &SpendLedger,
    out: &Path,
) -> Result<RunSummary, String> {
    let file = out.join("trials.jsonl");
    let done: BTreeSet<String> = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| v["trial"].as_str().map(String::from))
        .collect();
    let mut sink = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|e| format!("trials.jsonl: {e}"))?;
    let tracker = CacheTracker::default();
    let compact_blocks = super::arms::skill_blocks_with(
        env.arm_set,
        &env.work.join("skillhome-compact"),
        crate::skills::cost_profile::CostPolicy::compact(),
    );
    let (mut recorded, mut aborted) = (0usize, false);
    let mut emit = |v: &Value| {
        let _ = writeln!(sink, "{}", crate::json::canonical(v));
        let _ = sink.flush();
    };
    for rep in 0..spec.reps {
        for tid in &spec.tasks {
            let Some(task) = env.tasks.task(tid) else {
                continue;
            };
            for arm in &spec.arms {
                let Some(base_arm) = env.arm_set.arm(&arm.base_arm) else {
                    continue;
                };
                let mut eff = base_arm.clone();
                if let Some(v) = arm.view_arm.as_deref().and_then(|a| env.arm_set.arm(a)) {
                    eff.view = v.view.clone();
                }
                let base_arm = &eff;
                let arm_env = TrialEnv {
                    tasks: env.tasks,
                    tools: env.tools,
                    work: env.work,
                    counter: env.counter,
                    packs: env.packs,
                    arm_set: env.arm_set,
                    skills: if arm.has(Policy::CompactSkills) {
                        &compact_blocks
                    } else {
                        env.skills
                    },
                };
                if !base_arm.applies_to(&task.class) {
                    continue;
                }
                for m in models {
                    let key = TrialKey {
                        task: tid.clone(),
                        arm: arm.id.clone(),
                        model: m.id.clone(),
                        rep,
                    };
                    if done.contains(&key.id()) {
                        continue;
                    }
                    let b = base(&key, &task.class, arm, m, backend.origin(m));
                    let rec = if aborted {
                        status_record(
                            b,
                            "not_run",
                            "budget_aborted",
                            "campaign halted by the spend cap",
                        )
                    } else if let Some(why) = &arm.unavailable {
                        status_record(
                            b,
                            "unavailable",
                            if why.starts_with("not-applicable") {
                                "not_applicable"
                            } else {
                                "unavailable"
                            },
                            why,
                        )
                    } else {
                        match backend.client(arm, m, &tracker, &key) {
                            Err(why) => status_record(b, "unavailable", "unavailable", &why),
                            Ok(client) => {
                                let metered = Metered {
                                    inner: client.model(),
                                    ledger,
                                    billed: m.billed,
                                };
                                let trial = run_trial(&arm_env, &key, base_arm, m, &metered);
                                let attempts = client.take_attempts();
                                if trial["status"] == "budget" {
                                    aborted = true;
                                }
                                wrap(
                                    &key,
                                    &task.class,
                                    arm,
                                    m,
                                    backend.origin(m),
                                    client.path(),
                                    &attempts,
                                    client.observations().len(),
                                    overlay_states(arm, client.path(), &client.overlay_reports()),
                                    trial,
                                )
                            }
                        }
                    };
                    emit(&rec);
                    recorded += 1;
                }
            }
        }
    }
    Ok(RunSummary { recorded, aborted })
}

/// Attempts of a production pipeline run, from its report: every receipted
/// generation (first = generator, later = recovery) and every router call
/// (cost unknown: router spend is not receipted). A run that reused a cached
/// or scripted proposal reserved nothing and yields no attempts.
pub fn attempts_from_report(report: &Value) -> Vec<Attempt> {
    let mut out = vec![];
    for e in report["task_ledger"]["entries"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if e["kind"] == "router" {
            out.push(Attempt {
                role: "router",
                dispatched: true,
                ok: true,
                model_pin: None,
                usage: None,
                cost_micros: None,
                cost_basis: "unknown",
                cache: CacheState::Unknown,
                finish: "router".into(),
                latency_ms: 0,
                request_bytes: 0,
                request_tokens: None,
                prefix_identity: None,
            });
        }
    }
    let tracker = CacheTracker::default();
    for (i, e) in report["usage_receipts"]["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let r = &e["receipt"];
        let observed = r["availability"]["state"] == "observed";
        let usage = observed.then(|| Usage::from_json(&r["usage"]));
        let micros = |v: &Value| v.as_u64();
        let (cost, basis) = match (
            micros(&r["cost"]["provider_reported_micros"]),
            micros(&r["cost"]["estimated"]["micros"]),
        ) {
            (Some(m), _) => (Some(m), "provider"),
            (None, Some(m)) => (Some(m), "estimate"),
            _ => (None, "unknown"),
        };
        let model = e["requested_model"].as_str().unwrap_or("");
        out.push(Attempt {
            role: if i == 0 { "generator" } else { "recovery" },
            dispatched: true,
            ok: r["finish"] == "complete",
            model_pin: r["returned_model"]
                .as_str()
                .filter(|m| *m != "unknown")
                .map(String::from),
            cache: usage
                .as_ref()
                .map_or(CacheState::Unknown, |u| tracker.classify(model, u)),
            usage,
            cost_micros: cost,
            cost_basis: basis,
            finish: r["finish"].as_str().unwrap_or("unknown").into(),
            latency_ms: 0,
            request_bytes: 0,
            request_tokens: e["preflight_input_tokens"].as_u64(),
            prefix_identity: None,
        });
    }
    out
}
