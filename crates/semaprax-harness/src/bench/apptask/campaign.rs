//! Campaign planning and execution: rep-major order (so a stopped or capped
//! campaign leaves every cell with the same number of repetitions), bounded
//! per-model workers, resumable append-only `trials.jsonl`, and the spend
//! ledger as the authority over billed models.

use super::arms::{self, Arm, ArmSet, ContextPack};
use super::model::{Metered, ModelClient, SpendLedger};
use super::task::{TaskSet, Tools};
use super::tokens::TokenCounter;
use super::trial::{run_trial, ModelSpec, TrialEnv, TrialKey};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

pub struct ModelConn {
    pub spec: ModelSpec,
    pub client: Box<dyn ModelClient>,
    pub workers: usize,
}

pub struct Selection {
    pub tasks: Vec<String>,
    pub arms: Vec<String>,
    pub reps: u32,
}

pub fn selected_arms<'a>(set: &'a ArmSet, sel: &Selection) -> Vec<&'a Arm> {
    set.arms
        .iter()
        .filter(|a| sel.arms.is_empty() || sel.arms.contains(&a.id))
        .collect()
}

/// Rep-major, then task, arm, model: every cell gains one repetition at a time.
pub fn plan(
    tasks: &TaskSet,
    arm_set: &ArmSet,
    sel: &Selection,
    models: &[ModelSpec],
) -> Vec<TrialKey> {
    let arms = selected_arms(arm_set, sel);
    let mut out = vec![];
    for rep in 0..sel.reps {
        for t in tasks
            .tasks
            .iter()
            .filter(|t| sel.tasks.is_empty() || sel.tasks.contains(&t.id))
        {
            for a in arms.iter().filter(|a| a.applies_to(&t.class)) {
                for m in models {
                    out.push(TrialKey {
                        task: t.id.clone(),
                        arm: a.id.clone(),
                        model: m.id.clone(),
                        rep,
                    });
                }
            }
        }
    }
    out
}

/// Cold-build the retrieval pack of every (task, retrieval tool) the selection uses.
pub fn build_packs(
    tasks: &TaskSet,
    arm_set: &ArmSet,
    sel: &Selection,
    tools: &Tools,
    work: &Path,
) -> BTreeMap<(String, String), ContextPack> {
    let used: BTreeSet<String> = selected_arms(arm_set, sel)
        .iter()
        .filter_map(|a| a.retrieval_tool().map(String::from))
        .collect();
    let mut out = BTreeMap::new();
    for t in tasks
        .tasks
        .iter()
        .filter(|t| sel.tasks.is_empty() || sel.tasks.contains(&t.id))
    {
        for id in &used {
            if let Some(tool) = arm_set.retrieval.get(id) {
                out.insert(
                    (t.id.clone(), id.clone()),
                    arms::retrieval_pack(
                        tool,
                        t,
                        &t.project,
                        &t.steps[0].request,
                        tools,
                        work,
                        "cold",
                    ),
                );
            }
        }
    }
    out
}

pub fn done_ids(path: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| matches!(v["status"].as_str(), Some("ok") | Some("untested")))
        .filter_map(|v| v["trial"].as_str().map(String::from))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn execute(
    tasks: &TaskSet,
    tools: &Tools,
    work: &Path,
    counter: &dyn TokenCounter,
    arm_set: &ArmSet,
    packs: &BTreeMap<(String, String), ContextPack>,
    skills: &BTreeMap<String, arms::SkillBlock>,
    models: &[ModelConn],
    ledger: &SpendLedger,
    keys: Vec<TrialKey>,
    out: &Path,
) -> Result<(usize, usize), String> {
    let done = done_ids(out);
    let todo: Vec<TrialKey> = keys
        .into_iter()
        .filter(|k| !done.contains(&k.id()))
        .collect();
    let total = todo.len();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out)
        .map_err(|e| format!("open {}: {e}", out.display()))?;
    let sink = Mutex::new(file);
    let halt = AtomicBool::new(false);
    let finished = AtomicUsize::new(0);
    let env = TrialEnv {
        tasks,
        tools,
        work,
        counter,
        packs,
        arm_set,
        skills,
    };
    std::thread::scope(|s| {
        for m in models {
            let q: Mutex<VecDeque<TrialKey>> = Mutex::new(
                todo.iter()
                    .filter(|k| k.model == m.spec.id)
                    .cloned()
                    .collect(),
            );
            let q = std::sync::Arc::new(q);
            for _ in 0..m.workers.max(1) {
                let q = q.clone();
                let (env, sink, halt, finished) = (&env, &sink, &halt, &finished);
                s.spawn(move || {
                    let metered = Metered {
                        inner: &*m.client,
                        ledger,
                        billed: m.spec.billed,
                    };
                    loop {
                        if halt.load(Ordering::SeqCst) {
                            break;
                        }
                        let Some(key) = q.lock().ok().and_then(|mut g| g.pop_front()) else {
                            break;
                        };
                        let Some(arm) = arm_set.arm(&key.arm) else {
                            continue;
                        };
                        let rec = run_trial(env, &key, arm, &m.spec, &metered);
                        if rec["status"] == "budget" {
                            halt.store(true, Ordering::SeqCst);
                        }
                        if let Ok(mut f) = sink.lock() {
                            let _ = writeln!(f, "{}", crate::json::canonical(&rec));
                            let _ = f.flush();
                        }
                        let n = finished.fetch_add(1, Ordering::SeqCst) + 1;
                        if n % 10 == 0 {
                            eprintln!(
                                "apptask: {n}/{total} trials, spent USD {:.4}",
                                ledger.spent()
                            );
                        }
                    }
                });
            }
        }
    });
    Ok((finished.load(Ordering::SeqCst), total))
}
