//! `bench routing-matrix`: plan, fixture-lane or real-lane MR-13 runs.
//!
//! `--plan` discovers arms and prints the real-run cost ceiling without
//! executing anything. The fixture lane binds no adapter session, so every
//! learned arm is `unavailable` and its cells say why. The real lane refuses
//! unless the registry declares an executor whose requirements are met, at
//! least two generation profiles are available, and `--max-usd` covers the
//! printed ceiling; learned arms are bound only when their adapter starts.

use super::exec::{load_fixture, CellExecutor, CommandExecutor};
use super::registry::{ArmKind, Registry};
use super::report::{cost_ceiling_micros, gate_decision, manifest, ManifestInputs};
use super::run::{run, Lane};
use super::{closed, domain_of, q, text, TaskSet};
use crate::bench::corpus::RouterSpec;
use crate::bench::router::AdapterInvoker;
use crate::cli::{Environment, Outcome};
use crate::decision::qualify::{DomainGateSpec, GateBasis, GateSpec};
use crate::decision::DecisionInvoker;
use crate::diag::HarnessDiagnostic;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const GATE_SPEC_SCHEMA: &str = "semaprax.harness-routing-gate-spec.v1";
pub const USAGE: &str = "bench routing-matrix --registry FILE --tasks FILE --gate-spec FILE (--plan | --fixture-outcomes FILE --out DIR | --real --max-usd X --out DIR) [--repo DIR] [--hardware TEXT] [--pin K=V]... [--env K=V]...";

/// Parse the reviewed per-domain gate specs (only the outcome basis here).
pub fn gate_specs_from_json(v: &Value) -> Result<Vec<DomainGateSpec>, HarnessDiagnostic> {
    let m = closed(v, "gate spec", &["schema", "specs"], &["description"])?;
    if m["schema"] != GATE_SPEC_SCHEMA {
        return Err(q(
            "SPX-HPQ002",
            format!("gate spec schema must be `{GATE_SPEC_SCHEMA}`"),
        ));
    }
    let mut out = Vec::new();
    for s in m["specs"].as_array().into_iter().flatten() {
        let sm = closed(
            s,
            "domain gate spec",
            &[
                "domain",
                "version",
                "reviewed",
                "min_items",
                "completion_margin",
                "min_cost_saving",
                "max_extra_regressions",
                "max_latency_ratio",
            ],
            &[],
        )?;
        let f = |k: &str| {
            sm[k]
                .as_f64()
                .ok_or_else(|| q("SPX-HPQ002", format!("gate spec `{k}` must be a number")))
        };
        let u = |k: &str| {
            sm[k]
                .as_u64()
                .ok_or_else(|| q("SPX-HPQ002", format!("gate spec `{k}` must be an integer")))
        };
        let d = DomainGateSpec {
            domain: domain_of(sm["domain"].as_str().unwrap_or(""))?,
            version: text(sm, "version", "gate spec")?,
            reviewed: text(sm, "reviewed", "gate spec")?,
            spec: GateSpec {
                basis: GateBasis::Outcome,
                min_items: u("min_items")? as usize,
                completion_margin: f("completion_margin")?,
                min_cost_saving: f("min_cost_saving")?,
                max_extra_regressions: u("max_extra_regressions")? as u32,
                max_latency_ratio: f("max_latency_ratio")?,
            },
        };
        if out.iter().any(|x: &DomainGateSpec| x.domain == d.domain) {
            return Err(q("SPX-HPQ005", "one gate spec per domain"));
        }
        out.push(d);
    }
    Ok(out)
}

fn read(p: &Path) -> Result<Value, HarnessDiagnostic> {
    let b = std::fs::read(p).map_err(|e| q("SPX-HPQ001", format!("{}: {e}", p.display())))?;
    serde_json::from_slice(&b).map_err(|e| q("SPX-HPQ001", format!("{}: {e}", p.display())))
}

#[derive(Default)]
struct Args {
    registry: Option<String>,
    tasks: Option<String>,
    spec: Option<String>,
    fixture: Option<String>,
    out: Option<String>,
    repo: Option<String>,
    hardware: Option<String>,
    max_usd: Option<f64>,
    plan: bool,
    real: bool,
    pins: BTreeMap<String, String>,
    vars: Vec<(String, String)>,
}

fn parse(args: &[String]) -> Result<Args, HarnessDiagnostic> {
    let u = |m: String| q("SPX-HPQ007", m);
    let mut a = Args::default();
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or_else(|| u(format!("`{x}` needs a value")))
        };
        let kv = |s: String| {
            s.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .ok_or_else(|| u(format!("`{x}` takes K=V")))
        };
        match x.as_str() {
            "--registry" => a.registry = Some(val()?),
            "--tasks" => a.tasks = Some(val()?),
            "--gate-spec" => a.spec = Some(val()?),
            "--fixture-outcomes" => a.fixture = Some(val()?),
            "--out" => a.out = Some(val()?),
            "--repo" => a.repo = Some(val()?),
            "--hardware" => a.hardware = Some(val()?),
            "--max-usd" => {
                a.max_usd = Some(
                    val()?
                        .parse()
                        .map_err(|_| u("`--max-usd` takes a number".into()))?,
                )
            }
            "--plan" => a.plan = true,
            "--real" => a.real = true,
            "--pin" => {
                let (k, v) = kv(val()?)?;
                a.pins.insert(k, v);
            }
            "--env" => {
                let p = kv(val()?)?;
                a.vars.push(p);
            }
            f => return Err(u(format!("unknown argument `{f}`"))),
        }
    }
    let modes = [a.plan, a.real, a.fixture.is_some()]
        .iter()
        .filter(|x| **x)
        .count();
    if a.registry.is_none() || a.tasks.is_none() || a.spec.is_none() || modes != 1 {
        return Err(u("needs --registry, --tasks, --gate-spec and exactly one of --plan, --fixture-outcomes, --real".into()));
    }
    if !a.plan && a.out.is_none() {
        return Err(u("needs --out".into()));
    }
    Ok(a)
}

pub fn cli(args: &[String], env: &Environment) -> Outcome {
    match parse(args).and_then(|a| execute(a, env)) {
        Ok(o) => o,
        Err(e) if e.code == "SPX-HPQ007" => Outcome::usage(format!(
            "bench routing-matrix: {}\nusage: {USAGE}",
            e.message
        )),
        Err(e) => Outcome::refused(&e),
    }
}

fn execute(a: Args, env: &Environment) -> Result<Outcome, HarnessDiagnostic> {
    let abs = |p: &str| -> PathBuf {
        let p = Path::new(p);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            env.cwd.join(p)
        }
    };
    let repo = a
        .repo
        .as_deref()
        .map(abs)
        .unwrap_or_else(|| env.cwd.clone());
    let reg = Registry::load(&abs(a.registry.as_deref().unwrap_or("")), &repo)?;
    let tasks = TaskSet::from_json(&read(&abs(a.tasks.as_deref().unwrap_or("")))?)?;
    let specs = gate_specs_from_json(&read(&abs(a.spec.as_deref().unwrap_or("")))?)?;
    let mut env = env.clone();
    for (k, v) in &a.vars {
        env.vars.insert(k.clone(), v.clone());
    }
    if a.plan || a.real {
        // Deliberate forwarding: only variable names the approved registry
        // declares (plus the adapter runtime), never the whole environment.
        for k in reg
            .required_vars()
            .into_iter()
            .chain(["HARNESS_PYTHON".to_string()])
        {
            if let (false, Ok(v)) = (env.vars.contains_key(&k), std::env::var(&k)) {
                env.vars.insert(k, v);
            }
        }
    }
    let ceiling = cost_ceiling_micros(&reg, &tasks);
    let usd = ceiling as f64 / 1_000_000.0;
    if a.plan {
        let arms = super::registry::discover(&reg, &tasks, &env, &Default::default(), false);
        let plan = json!({"real_run_cost_ceiling_micros": ceiling, "real_run_cost_ceiling_usd": usd,
            "arms": arms.iter().map(|x| x.to_json()).collect::<Vec<_>>(),
            "note": "learned arms show `no live adapter session` until a real run starts their adapters"});
        return Ok(Outcome::ok(format!(
            "cost ceiling: {ceiling} micros (~{usd:.2} USD)\n{}\n",
            serde_json::to_string_pretty(&plan).unwrap_or_default()
        )));
    }
    let out = abs(a.out.as_deref().unwrap_or(""));
    let hardware = a
        .hardware
        .clone()
        .unwrap_or_else(|| "unspecified".to_string());
    let mut sessions = serde_json::Map::new();
    let mut adapters: Vec<(String, AdapterInvoker)> = Vec::new();
    let mut fixture;
    let mut command;
    let executor: &mut dyn CellExecutor = if a.real {
        let Some(decl) = reg.executor.clone() else {
            return Err(q(
                "SPX-HPQ007",
                "a real run needs a registry `executor` (no fixture ever counts as a real run)",
            ));
        };
        if let Some(why) = decl.requires.unmet(&env) {
            return Err(q("SPX-HPQ007", format!("real executor unavailable: {why}")));
        }
        let available = super::registry::discover(&reg, &tasks, &env, &Default::default(), false)
            .iter()
            .filter(|x| matches!(x.kind, ArmKind::Fixed(_)) && x.unavailable.is_none())
            .count();
        if available < 2 {
            return Err(q("SPX-HPQ007", format!("a real matrix needs at least two available generation profiles; {available} available")));
        }
        match a.max_usd {
            Some(m) if m >= usd => {}
            _ => {
                return Err(q("SPX-HPQ007", format!("refused: the real-run cost ceiling is ~{usd:.2} USD ({ceiling} micros); pass --max-usd at or above it")))
            }
        }
        for l in &reg.learned {
            if let Some(why) = l.requires.unmet(&env) {
                sessions.insert(l.arm_id.clone(), json!({"bound": false, "reason": why}));
                continue;
            }
            let mut vars = BTreeMap::from([(
                "SEMAPRAX_HARNESS_MODEL_PROFILE".to_string(),
                l.model_profile.to_json().to_string(),
            )]);
            if let Some(e) = &l.instance.endpoint {
                vars.insert("SEMAPRAX_HARNESS_CFG_ENDPOINT".into(), e.clone());
            }
            for k in l.requires.env.iter() {
                if let Some(v) = env.vars.get(k) {
                    vars.insert(k.clone(), v.clone());
                }
            }
            let spec = RouterSpec {
                descriptor: l.descriptor.display().to_string(),
                runtime_env: "HARNESS_PYTHON".into(),
                env: vars,
                provider_id: l.adapter.provider_id.clone(),
                model_id: l.model_profile.model.clone(),
                checkpoint: l.model_profile.checkpoint_label(),
            };
            match AdapterInvoker::spawn(&spec, &l.descriptor, &env) {
                Ok(inv) => {
                    sessions.insert(l.arm_id.clone(), json!({"bound": true}));
                    adapters.push((l.arm_id.clone(), inv));
                }
                Err(e) => {
                    sessions.insert(
                        l.arm_id.clone(),
                        json!({"bound": false, "reason": e.message}),
                    );
                }
            }
        }
        command = CommandExecutor {
            decl,
            cwd: repo.clone(),
            vars: env.vars.clone(),
        };
        &mut command
    } else {
        fixture = load_fixture(&abs(a.fixture.as_deref().unwrap_or("")))?;
        for l in &reg.learned {
            sessions.insert(
                l.arm_id.clone(),
                json!({"bound": false, "reason": "fixture lane: no adapter is started and no network or paid call is made"}),
            );
        }
        &mut fixture
    };
    let invokers: BTreeMap<String, &mut dyn DecisionInvoker> = adapters
        .iter_mut()
        .map(|(k, v)| (k.clone(), v as &mut dyn DecisionInvoker))
        .collect();
    let r = run(&reg, &tasks, &specs, &env, Lane { executor, invokers })?;
    let sessions = Value::Object(sessions);
    let doc = manifest(
        &r,
        &ManifestInputs {
            reg: &reg,
            tasks: &tasks,
            specs: &specs,
            mode: if a.real { "real" } else { "fixture" },
            hardware: &hardware,
            pins: &a.pins,
            sessions: &sessions,
        },
    );
    std::fs::create_dir_all(&out)
        .map_err(|e| q("SPX-HPQ008", format!("{}: {e}", out.display())))?;
    let write = |name: &str, v: &Value| {
        let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
        s.push('\n');
        std::fs::write(out.join(name), s).map_err(|e| q("SPX-HPQ008", format!("{name}: {e}")))
    };
    write("run-manifest.json", &doc)?;
    write("gate-decision.json", &gate_decision(&doc))?;
    Ok(Outcome::ok(format!(
        "routing matrix ({}): decision {}; {} cells; manifest {}\n",
        doc["mode"].as_str().unwrap_or(""),
        r.overall(),
        r.cells.len(),
        out.join("run-manifest.json").display()
    )))
}
