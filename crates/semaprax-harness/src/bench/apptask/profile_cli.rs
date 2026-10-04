//! `bench app run <tasks> --profile-arms ... --max-usd N` and
//! `bench app qualify <out>`: the capped profile campaign (TC-12).
//!
//! The command declares `campaign.json` (arms, pins, caps and the
//! non-inferiority criterion) before the first trial, then runs the screening
//! roster through the benchmark's raw model loop (`path: raw-model-loop`).
//! That path never promotes a profile: only trials recorded through
//! `ProductionClient` (`path: production-harness`, library API) can. The
//! spend ledger refuses any call that could cross `--max-usd`, and the
//! remaining cohort is retained as `budget_aborted`.

use super::cache_state::CacheTracker;
use super::model::{HttpModel, ModelClient, SpendLedger};
use super::production::{RawClient, TrialClient};
use super::profile_arms::{self, CampaignSpec, Criterion, Pins};
use super::profile_campaign::{self, ArmBackend};
use super::trial::{ModelSpec, TrialEnv, TrialKey};
use super::{arms, campaign, task, tokens};
use crate::json::canonical;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub struct RawBackend {
    pub clients: BTreeMap<String, Box<dyn ModelClient>>,
}

impl ArmBackend for RawBackend {
    fn origin(&self, m: &ModelSpec) -> &'static str {
        if m.billed {
            "real"
        } else {
            "fixture"
        }
    }
    fn client<'a>(
        &'a self,
        _arm: &profile_arms::ProfileArm,
        model: &ModelSpec,
        tracker: &'a CacheTracker,
        _key: &TrialKey,
    ) -> Result<Box<dyn TrialClient + 'a>, String> {
        let c = self
            .clients
            .get(&model.id)
            .ok_or_else(|| format!("model `{}` is not configured", model.id))?;
        Ok(Box::new(RawClient::new(
            &**c,
            tracker,
            &model.id,
            model.billed,
        )))
    }
}

pub fn pins_for(set: &task::TaskSet, models: &[ModelSpec], identities: &Value) -> Pins {
    Pins {
        model: models
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>()
            .join("+"),
        tools: super::production::pin(&canonical(identities)),
        taskset: set.digest.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_profiles(
    set: &task::TaskSet,
    arm_set: &arms::ArmSet,
    tools: &task::Tools,
    work: &Path,
    out: &Path,
    reps: u32,
    arms_sel: &[String],
    tasks_sel: &[String],
    max_usd: f64,
    max_calls: u64,
    models: &[(ModelSpec, HttpModel)],
    identities: &Value,
    dry: bool,
    counter: &dyn tokens::TokenCounter,
) -> Result<String, String> {
    let specs: Vec<ModelSpec> = models.iter().map(|(s, _)| s.clone()).collect();
    let mut roster = profile_arms::screening_roster("native");
    if !(arms_sel.is_empty() || arms_sel.iter().any(|a| a == "all")) {
        roster.retain(|a| a.id == profile_arms::BASELINE || arms_sel.contains(&a.id));
    }
    let tasks: Vec<String> = set
        .tasks
        .iter()
        .filter(|t| tasks_sel.is_empty() || tasks_sel.contains(&t.id))
        .map(|t| t.id.clone())
        .collect();
    let spec = CampaignSpec {
        id: format!("profile-{}", &set.digest[7..19]),
        pins: pins_for(set, &specs, identities),
        arms: roster,
        tasks,
        reps,
        criterion: Criterion::predeclared(),
        max_usd,
        max_calls,
    };
    if dry {
        return Ok(format!(
            "planned {} arms x {} tasks x {} reps x {} models; cap USD {max_usd}; nothing was run\n",
            spec.arms.len(),
            spec.tasks.len(),
            reps,
            specs.len()
        ));
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    profile_arms::declare(out, &spec)?;
    let ledger = SpendLedger::new(max_usd, max_calls, 0.02).with_file(&out.join("ledger.json"));
    let sel = campaign::Selection {
        tasks: vec![],
        arms: vec![],
        reps,
    };
    let packs = campaign::build_packs(set, arm_set, &sel, tools, work);
    let blocks = arms::skill_blocks(arm_set, &work.join("skillhome"));
    let env = TrialEnv {
        tasks: set,
        tools,
        work,
        counter,
        packs: &packs,
        arm_set,
        skills: &blocks,
    };
    let backend = RawBackend {
        clients: models
            .iter()
            .map(|(s, c)| (s.id.clone(), Box::new(c.clone()) as Box<dyn ModelClient>))
            .collect(),
    };
    let s = profile_campaign::run(&env, &spec, &specs, &backend, &ledger, out)?;
    Ok(format!(
        "recorded {} profile trials ({}); spent USD {}; run `bench app qualify {}`\n",
        s.recorded,
        if s.aborted {
            "halted by the spend cap"
        } else {
            "complete"
        },
        ledger.snapshot()["spent_usd"],
        out.display()
    ))
}

/// `bench app qualify <out>`: read `campaign.json` and `trials.jsonl`, write `qualification.json`.
pub fn qualify_dir(out: &Path, live: &Pins) -> Result<String, String> {
    let (declared, criterion) = profile_arms::load(out)?;
    let rows: Vec<Value> = std::fs::read_to_string(out.join("trials.jsonl"))
        .map_err(|e| format!("trials.jsonl: {e}"))?
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|v: &Value| v["schema"] == profile_campaign::PROFILE_TRIAL_SCHEMA)
        .collect();
    let q = super::profile_qualify::qualify(&declared, &criterion, &rows, live, &[]);
    std::fs::write(
        out.join("qualification.json"),
        format!("{}\n", serde_json::to_string_pretty(&q).unwrap_or_default()),
    )
    .map_err(|e| e.to_string())?;
    Ok(format!(
        "recommendation: {}\n",
        json!(q["recommendation"]["action"])
    ))
}
