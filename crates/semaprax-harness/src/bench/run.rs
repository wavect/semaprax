//! Matrix orchestration: profiles x tasks x (cold + warm) trials, optional
//! per-cell child-process resource accounting through `/usr/bin/time`.

use super::adversarial::{run_adversarial, AdvEnv, AdvResult};
use super::arena::Arena;
use super::cell::{run_cell, CellInput};
use super::corpus::Corpus;
use super::measure::{parse_time_output, Clock};
use crate::observe::Observation;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How to re-invoke this binary for a measured cell.
#[derive(Clone, Debug)]
pub struct SelfExe {
    pub exe: PathBuf,
    /// Verb prefix: empty for `semaprax-harness`, `["harness"]` for `semaprax`.
    pub prefix: Vec<String>,
}

pub struct RunOptions<'a> {
    pub corpus: &'a Corpus,
    pub repo: PathBuf,
    pub work: PathBuf,
    pub vars: BTreeMap<String, String>,
    pub compiler: Option<PathBuf>,
    /// Empty = every profile in the corpus.
    pub profiles: Vec<String>,
    pub warm: Option<u32>,
    pub clock: &'a dyn Clock,
    /// `Some` = run each cell in a child under `/usr/bin/time` (resources measured).
    pub measure_with: Option<SelfExe>,
    pub adversarial: bool,
}

#[derive(Default)]
pub struct RunOutput {
    pub cells: Vec<Value>,
    pub events: BTreeMap<String, Vec<Observation>>,
    pub adversarial: Vec<AdvResult>,
    /// Per profile: did the stored grant change while cells ran?
    pub permission_changed: BTreeMap<String, bool>,
    pub untested_profiles: BTreeMap<String, String>,
}

fn spec_json(o: &RunOptions, profile: &str, task: &str, trial: u32, warm: bool) -> Value {
    json!({"corpus": o.corpus.dir, "work": o.work, "profile": profile, "task": task, "trial": trial, "warm": warm,
           "compiler": o.compiler, "vars": o.vars})
}

/// Child entry: run one cell from a spec file and print `{cell, events}`.
pub fn cell_from_spec(spec: &Value, clock: &dyn Clock) -> Result<Value, String> {
    let dir = PathBuf::from(spec["corpus"].as_str().ok_or("spec.corpus")?);
    let corpus = Corpus::load(&dir).map_err(|e| e.to_string())?;
    let profile = corpus
        .profile(spec["profile"].as_str().ok_or("spec.profile")?)
        .ok_or("unknown profile")?
        .clone();
    let task = corpus
        .tasks
        .iter()
        .find(|t| Some(t.id.as_str()) == spec["task"].as_str())
        .ok_or("unknown task")?;
    let vars: BTreeMap<String, String> = spec["vars"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let work = PathBuf::from(spec["work"].as_str().ok_or("spec.work")?);
    let compiler = spec["compiler"].as_str().map(PathBuf::from);
    let arena = Arena::attach(&corpus, &profile, &work, &vars, compiler);
    let (cell, events) = run_cell(&CellInput {
        corpus: &corpus,
        arena: &arena,
        task,
        trial: spec["trial"].as_u64().unwrap_or(0) as u32,
        warm: spec["warm"].as_bool().unwrap_or(false),
        clock,
    });
    Ok(
        json!({"cell": cell.to_json(), "events": events.iter().map(Observation::to_json).collect::<Vec<_>>()}),
    )
}

fn measured_cell(
    o: &RunOptions,
    me: &SelfExe,
    arena: &Arena,
    spec: &Value,
) -> Option<(Value, Vec<Observation>)> {
    let path = arena.root.join("cellspec.json");
    std::fs::write(&path, spec.to_string()).ok()?;
    let started = std::time::Instant::now();
    let out = Command::new("/usr/bin/time")
        .arg("-l")
        .arg(&me.exe)
        .args(&me.prefix)
        .args(["bench", "--cell"])
        .arg(&path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .ok()?;
    let wall = started.elapsed().as_millis() as u64;
    let _ = o;
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;
    let ru = parse_time_output(&String::from_utf8_lossy(&out.stderr));
    let mut cell = v["cell"].clone();
    cell["resources"] = json!({"measured": ru.user_ms.is_some(), "user_ms": ru.user_ms, "sys_ms": ru.sys_ms,
        "max_rss_kb": ru.max_rss_kb, "process_wall_ms": wall, "method": "/usr/bin/time -l around one cell process"});
    let events = v["events"]
        .as_array()?
        .iter()
        .filter_map(|e| Observation::from_json(e).ok())
        .collect();
    Some((cell, events))
}

pub fn run_matrix(o: &RunOptions) -> RunOutput {
    let mut out = RunOutput::default();
    let warm = o.warm.unwrap_or(o.corpus.warm);
    for p in &o.corpus.profiles {
        if !o.profiles.is_empty() && !p.baseline && !o.profiles.contains(&p.id) {
            continue;
        }
        let arena = Arena::prepare(o.corpus, p, &o.repo, &o.work, &o.vars, o.compiler.clone());
        if let Some(why) = &arena.untested {
            out.untested_profiles.insert(p.id.clone(), why.clone());
        }
        let touches = p.touches();
        for t in &o.corpus.tasks {
            let steps = t.steps();
            if !p.baseline && !steps.iter().any(|s| touches.contains(s)) {
                continue; // profile cannot change this task: not applicable, not a win
            }
            let trials = o.corpus.cold + warm;
            for trial in 0..trials {
                let is_warm = trial >= o.corpus.cold;
                if arena.untested.is_none() && !is_warm {
                    arena.reset_cold();
                }
                let input = CellInput {
                    corpus: o.corpus,
                    arena: &arena,
                    task: t,
                    trial,
                    warm: is_warm,
                    clock: o.clock,
                };
                let (cell, events) = match (&o.measure_with, &arena.untested) {
                    (Some(me), None) => {
                        let spec = spec_json(o, &p.id, &t.id, trial, is_warm);
                        match measured_cell(o, me, &arena, &spec) {
                            Some(x) => x,
                            None => {
                                let mut c = super::cell::CellResult::untested(
                                    &input,
                                    "untested: measured cell process failed to report",
                                )
                                .to_json();
                                c["resources"] = json!({"measured": false});
                                (c, vec![])
                            }
                        }
                    }
                    _ => {
                        let (c, e) = run_cell(&input);
                        let mut c = c.to_json();
                        c["resources"] =
                            json!({"measured": false, "method": "in-process (not measured)"});
                        (c, e)
                    }
                };
                out.events.entry(p.id.clone()).or_default().extend(events);
                out.cells.push(cell);
                if arena.untested.is_some() {
                    break; // one untested record per (profile, task)
                }
            }
        }
        out.permission_changed.insert(
            p.id.clone(),
            arena.untested.is_none() && arena.trust_digest() != arena.trust_before,
        );
    }
    if o.adversarial {
        out.adversarial = run_adversarial(&AdvEnv {
            corpus: o.corpus,
            repo: &o.repo,
            work: &o.work.join("_adversarial"),
            vars: &o.vars,
            compiler: o.compiler.clone(),
        });
    }
    out
}

pub fn ensure_dir(p: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(p)
}
