//! Application-task benchmark (HN-17): complete multi-file tasks graded by
//! immutable oracles, run against compared arms on real models, with spend
//! capped by a ledger. Extends the HP-17 benchmark: same `bench` verb, same
//! statistics and gate constants, no parallel telemetry. Diagnostics `SPX-HPQ0xx`
//! continue the HP-17 numbering from 011.
//!
//! `bench app validate <tasks>` proves every grader fails on the pristine project and
//! passes on the reference; `bench app run <tasks> ...` runs the campaign;
//! `bench app report <out>` aggregates the raw trials.

pub mod arms;
pub mod cache_state;
pub mod campaign;
pub mod model;
pub mod production;
pub mod profile_arms;
pub mod profile_campaign;
pub mod profile_cli;
pub mod profile_qualify;
pub mod render;
pub mod report;
pub mod task;
pub mod tokens;
pub mod trial;

use crate::cli::{Environment, Outcome};
use crate::diag::HarnessDiagnostic;
use model::{HttpModel, SpendLedger};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use task::{TaskSet, Tools};

pub const APP_USAGE: &str = "bench app validate <tasks> [--env K=V]... [--work DIR] | bench app run <tasks> --out DIR --work DIR --reps N --model id=ID,name=NAME,addr=HOST:PORT,size=small|large[,billed=1][,workers=N][,predict=N][,ctx=N][,temp=X]... [--task ID]... [--arm ID]... [--cap-usd X] [--max-calls N] [--ident K=V]... [--env K=V]... [--dry-run] | bench app run <tasks> --profile-arms all|ID,ID --max-usd N [--out DIR --work DIR --reps N --model ...] (capped profile campaign) | bench app qualify <out> [--ident model=ID --ident tools=DIGEST --ident taskset=DIGEST] | bench app report <out> [--label TEXT]";

fn q(code: &'static str, m: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, m)
}

fn abs(env: &Environment, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        env.cwd.join(p)
    }
}

/// One validation row per step: the grader must fail the pristine state and pass the reference.
pub fn validate_tasks(set: &TaskSet, tools: &Tools, work: &Path) -> Vec<Value> {
    let mut rows = vec![];
    let home = work.join("home");
    for t in &set.tasks {
        let mut state = t.project.clone();
        for (i, s) in t.steps.iter().enumerate() {
            let sb = work.join("validate").join(&t.id);
            let pristine =
                task::prepare_sandbox(&sb, &state).map(|_| task::grade(&sb, &home, t, i, tools));
            let mut with_ref = state.clone();
            for (k, v) in t.reference_files_of(i) {
                with_ref.insert(k, String::from_utf8_lossy(&v).into_owned());
            }
            let reference =
                task::prepare_sandbox(&sb, &with_ref).map(|_| task::grade(&sb, &home, t, i, tools));
            let untested = pristine
                .as_ref()
                .ok()
                .and_then(|g| g.untested.clone())
                .or_else(|| reference.as_ref().ok().and_then(|g| g.untested.clone()));
            rows.push(json!({"task": t.id, "step": s.id, "class": t.class,
                "pristine_fails": pristine.as_ref().is_ok_and(|g| !g.passed && g.untested.is_none()),
                "reference_passes": reference.as_ref().is_ok_and(|g| g.passed),
                "untested": untested}));
            state = with_ref;
            let _ = std::fs::remove_dir_all(&sb);
        }
    }
    rows
}

pub fn validation_ok(rows: &[Value]) -> bool {
    !rows.is_empty()
        && rows
            .iter()
            .all(|r| r["pristine_fails"] == true && r["reference_passes"] == true)
}

#[derive(Default)]
struct Args {
    cmd: String,
    dir: Option<String>,
    out: Option<String>,
    work: Option<String>,
    reps: u32,
    tasks: Vec<String>,
    arms: Vec<String>,
    models: Vec<BTreeMap<String, String>>,
    cap: f64,
    max_calls: u64,
    vars: BTreeMap<String, String>,
    idents: BTreeMap<String, String>,
    label: Option<String>,
    dry: bool,
    profile_arms: Option<String>,
    max_usd: Option<f64>,
}

fn kv(s: &str) -> BTreeMap<String, String> {
    s.split(',')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn parse(args: &[String]) -> Result<Args, HarnessDiagnostic> {
    let mut a = Args {
        reps: 10,
        cap: 15.0,
        max_calls: 5000,
        ..Args::default()
    };
    let mut it = args.iter();
    a.cmd = it
        .next()
        .cloned()
        .ok_or_else(|| q("SPX-HPQ007", "missing app subcommand"))?;
    while let Some(x) = it.next() {
        let mut val = |n: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| q("SPX-HPQ007", format!("`{n}` needs a value")))
        };
        let num = |n: &str, v: String| {
            v.parse::<f64>()
                .map_err(|_| q("SPX-HPQ007", format!("`{n}` needs a number")))
        };
        match x.as_str() {
            "--out" => a.out = Some(val(x)?),
            "--work" => a.work = Some(val(x)?),
            "--reps" => a.reps = num(x, val(x)?)? as u32,
            "--task" => a.tasks.push(val(x)?),
            "--arm" => a.arms.push(val(x)?),
            "--model" => a.models.push(kv(&val(x)?)),
            "--cap-usd" => a.cap = num(x, val(x)?)?,
            "--max-calls" => a.max_calls = num(x, val(x)?)? as u64,
            "--label" => a.label = Some(val(x)?),
            "--dry-run" => a.dry = true,
            "--profile-arms" => a.profile_arms = Some(val(x)?),
            "--max-usd" => a.max_usd = Some(num(x, val(x)?)?),
            "--env" | "--ident" => {
                let v = val(x)?;
                let (k, v) = v
                    .split_once('=')
                    .ok_or_else(|| q("SPX-HPQ007", format!("`{x}` needs KEY=VALUE")))?;
                if x == "--env" {
                    a.vars.insert(k.into(), v.into())
                } else {
                    a.idents.insert(k.into(), v.into())
                };
            }
            f if f.starts_with("--") => return Err(q("SPX-HPQ007", format!("unknown flag `{f}`"))),
            p if a.dir.is_none() => a.dir = Some(p.to_string()),
            _ => return Err(q("SPX-HPQ007", "unexpected extra argument")),
        }
    }
    Ok(a)
}

pub fn cli_app(args: &[String], env: &Environment) -> Outcome {
    match parse(args).and_then(|a| exec(a, env)) {
        Ok(o) => o,
        Err(e) if e.code == "SPX-HPQ007" => {
            Outcome::usage(format!("bench app: {}\nusage: {APP_USAGE}", e.message))
        }
        Err(e) => Outcome::refused(&e),
    }
}

fn version_of(exe: Option<&PathBuf>, arg: &str, extra_path: Option<&Path>) -> Value {
    let path = match extra_path {
        Some(d) => format!("{}:/usr/bin:/bin", d.display()),
        None => "/usr/bin:/bin".to_string(),
    };
    exe.and_then(|e| {
        std::process::Command::new(e)
            .arg(arg)
            .env_clear()
            .env("PATH", path)
            .output()
            .ok()
    })
    .map(|o| {
        Value::String(
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .to_string(),
        )
    })
    .unwrap_or(Value::Null)
}

fn exec(a: Args, env: &Environment) -> Result<Outcome, HarnessDiagnostic> {
    let dir = abs(
        env,
        a.dir
            .as_deref()
            .ok_or_else(|| q("SPX-HPQ007", "missing directory"))?,
    );
    let mut vars: BTreeMap<String, String> = env
        .vars
        .iter()
        .filter(|(k, _)| k.starts_with("HARNESS_"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    vars.extend(a.vars.clone());
    let tools = Tools::from_vars(&vars, env.compiler.clone());
    match a.cmd.as_str() {
        "report" => {
            return write_report(&dir, a.label.as_deref().unwrap_or("local run")).map(Outcome::ok)
        }
        "qualify" => {
            let (declared, _) = profile_arms::load(&dir).map_err(|e| q("SPX-HPQ001", e))?;
            let pin = |k: &str| {
                a.idents
                    .get(k)
                    .cloned()
                    .unwrap_or_else(|| declared["pins"][k].as_str().unwrap_or("").to_string())
            };
            let live = profile_arms::Pins {
                model: pin("model"),
                tools: pin("tools"),
                taskset: pin("taskset"),
            };
            return profile_cli::qualify_dir(&dir, &live)
                .map(Outcome::ok)
                .map_err(|e| q("SPX-HPQ008", e));
        }
        "validate" | "run" => {}
        other => return Err(q("SPX-HPQ007", format!("unknown app subcommand `{other}`"))),
    }
    let set = TaskSet::load(&dir).map_err(|e| q("SPX-HPQ001", e))?;
    let arm_set = arms::ArmSet::load(&dir).map_err(|e| q("SPX-HPQ001", e))?;
    let work = a
        .work
        .as_deref()
        .map(|w| abs(env, w))
        .unwrap_or_else(|| std::env::temp_dir().join("semaprax-apptask"));
    let _ = std::fs::create_dir_all(&work);
    if a.cmd == "validate" {
        let rows = validate_tasks(&set, &tools, &work);
        let ok = validation_ok(&rows);
        let mut text = format!(
            "task set {} ({} tasks, digest {})\n",
            dir.display(),
            set.tasks.len(),
            set.digest
        );
        for r in &rows {
            text.push_str(&format!(
                "  {} {}: pristine fails {}, reference passes {}{}\n",
                r["task"].as_str().unwrap_or(""),
                r["step"].as_str().unwrap_or(""),
                r["pristine_fails"],
                r["reference_passes"],
                r["untested"]
                    .as_str()
                    .map(|u| format!(" (untested: {u})"))
                    .unwrap_or_default()
            ));
        }
        return Ok(if ok {
            Outcome::ok(text)
        } else {
            Outcome::refused(&q(
                "SPX-HPQ011",
                format!("grader validation failed\n{text}"),
            ))
        });
    }
    let out = abs(
        env,
        a.out
            .as_deref()
            .ok_or_else(|| q("SPX-HPQ007", "run needs --out"))?,
    );
    std::fs::create_dir_all(&out).map_err(|e| q("SPX-HPQ008", e.to_string()))?;
    let sel = campaign::Selection {
        tasks: a.tasks.clone(),
        arms: a.arms.clone(),
        reps: a.reps,
    };
    let mut specs = vec![];
    let mut conns = vec![];
    let mut raw_models: Vec<(trial::ModelSpec, HttpModel)> = vec![];
    for m in &a.models {
        let get = |k: &str| m.get(k).cloned();
        let id = get("id").ok_or_else(|| q("SPX-HPQ007", "--model needs id="))?;
        let spec = trial::ModelSpec {
            id: id.clone(),
            size: get("size").unwrap_or_else(|| "small".into()),
            billed: get("billed").as_deref() == Some("1"),
        };
        let client = HttpModel {
            addr: get("addr").ok_or_else(|| q("SPX-HPQ007", "--model needs addr="))?,
            name: get("name").ok_or_else(|| q("SPX-HPQ007", "--model needs name="))?,
            temperature: get("temp").and_then(|x| x.parse().ok()).unwrap_or(0.7),
            num_ctx: get("ctx").and_then(|x| x.parse().ok()).unwrap_or(16384),
            num_predict: get("predict").and_then(|x| x.parse().ok()).unwrap_or(3000),
            timeout: Duration::from_secs(
                get("timeout").and_then(|x| x.parse().ok()).unwrap_or(300),
            ),
        };
        specs.push(spec.clone());
        raw_models.push((spec.clone(), client.clone()));
        conns.push(campaign::ModelConn {
            spec,
            client: Box::new(client),
            workers: get("workers").and_then(|x| x.parse().ok()).unwrap_or(1),
        });
    }
    if let Some(psel) = &a.profile_arms {
        let max_usd = a.max_usd.ok_or_else(|| {
            q(
                "SPX-HPQ007",
                "--profile-arms needs an explicit --max-usd cap",
            )
        })?;
        let ids: Vec<String> = psel.split(',').map(|x| x.trim().to_string()).collect();
        let mut idents = serde_json::Map::new();
        for (var, p) in &tools.vars {
            idents.insert(var.to_lowercase(), json!(p.display().to_string()));
        }
        idents.insert(
            "compiler".into(),
            version_of(tools.compiler.as_ref(), "--version", None),
        );
        let words = tokens::WordCounter;
        let tik;
        let counter: &dyn tokens::TokenCounter = if a.dry {
            &words
        } else {
            let py = vars
                .get("HARNESS_TIKTOKEN_PYTHON")
                .map(PathBuf::from)
                .ok_or_else(|| {
                    q(
                        "SPX-HPQ007",
                        "run needs --env HARNESS_TIKTOKEN_PYTHON and HARNESS_TIKTOKEN_CACHE",
                    )
                })?;
            let cache = vars
                .get("HARNESS_TIKTOKEN_CACHE")
                .map(PathBuf::from)
                .ok_or_else(|| q("SPX-HPQ007", "run needs HARNESS_TIKTOKEN_CACHE"))?;
            tik = tokens::Tiktoken::start(&py, &cache).map_err(|e| q("SPX-HPQ012", e))?;
            &tik
        };
        return profile_cli::run_profiles(
            &set,
            &arm_set,
            &tools,
            &work,
            &out,
            a.reps,
            &ids,
            &a.tasks,
            max_usd,
            a.max_calls,
            &raw_models,
            &Value::Object(idents),
            a.dry,
            counter,
        )
        .map(Outcome::ok)
        .map_err(|e| q("SPX-HPQ008", e));
    }
    let keys = campaign::plan(&set, &arm_set, &sel, &specs);
    if a.dry {
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for k in &keys {
            *by.entry(k.model.clone()).or_default() += 1;
        }
        return Ok(Outcome::ok(format!(
            "planned {} trials: {by:?}\n",
            keys.len()
        )));
    }
    let py = vars.get("HARNESS_TIKTOKEN_PYTHON").map(PathBuf::from).ok_or_else(|| q("SPX-HPQ007", "run needs --env HARNESS_TIKTOKEN_PYTHON and HARNESS_TIKTOKEN_CACHE (named tokenizer)"))?;
    let cache = vars
        .get("HARNESS_TIKTOKEN_CACHE")
        .map(PathBuf::from)
        .ok_or_else(|| q("SPX-HPQ007", "run needs HARNESS_TIKTOKEN_CACHE"))?;
    let counter = tokens::Tiktoken::start(&py, &cache).map_err(|e| q("SPX-HPQ012", e))?;
    let ledger = SpendLedger::new(a.cap, a.max_calls, 0.02).with_file(&out.join("ledger.json"));
    let packs = campaign::build_packs(&set, &arm_set, &sel, &tools, &work);
    let blocks = arms::skill_blocks(&arm_set, &work.join("skillhome"));
    let skill_ident = |arm: &str| {
        blocks
            .get(arm)
            .map(|b| json!({"ids": b.ids, "bytes": b.text.len(), "delivered": b.delivered}))
    };
    let mut identities = serde_json::Map::new();
    for (var, p) in &tools.vars {
        identities.insert(var.to_lowercase(), json!({"path": p.display().to_string(), "version": version_of(Some(p), "--version", tools.get("HARNESS_NODE").and_then(|n| n.parent()))}));
    }
    identities.insert(
        "compiler".into(),
        version_of(tools.compiler.as_ref(), "--version", None),
    );
    identities.insert(
        "tokenizer".into(),
        json!(tokens::TokenCounter::name(&counter)),
    );
    identities.insert(
        "skill_blocks".into(),
        json!(arm_set
            .arms
            .iter()
            .filter_map(|a| skill_ident(&a.id).map(|v| (a.id.clone(), v)))
            .collect::<BTreeMap<_, _>>()),
    );
    for (k, v) in &a.idents {
        identities.insert(k.clone(), json!(v));
    }
    for (pid, p) in &packs {
        if let Some(why) = &p.unavailable {
            identities.insert(format!("pack-unavailable:{}:{}", pid.0, pid.1), json!(why));
        }
    }
    let meta = json!({"taskset_digest": set.digest, "tokenizer": tokens::TokenCounter::name(&counter), "identities": identities,
        "arms": arm_set.arms.iter().map(|x| json!({"id": x.id, "role": format!("{:?}", x.role), "label": x.label})).collect::<Vec<_>>(),
        "untested_arms": arm_set.untested.iter().cloned().collect::<BTreeMap<_, _>>(),
        "models": a.models, "reps_requested": a.reps, "cap_usd": a.cap,
        "models_note": a.models.iter().map(|m| format!("{} ({}, {})", m.get("id").cloned().unwrap_or_default(), m.get("name").cloned().unwrap_or_default(), m.get("size").cloned().unwrap_or_default())).collect::<Vec<_>>().join("; ")});
    std::fs::write(
        out.join("environment.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&meta).unwrap_or_default()
        ),
    )
    .map_err(|e| q("SPX-HPQ008", e.to_string()))?;
    let (done, total) = campaign::execute(
        &set,
        &tools,
        &work,
        &counter,
        &arm_set,
        &packs,
        &blocks,
        &conns,
        &ledger,
        keys,
        &out.join("trials.jsonl"),
    )
    .map_err(|e| q("SPX-HPQ008", e))?;
    let rep = write_report(&out, a.label.as_deref().unwrap_or("local run"))?;
    Ok(Outcome::ok(format!(
        "ran {done} of {total} planned trials; spent USD {}\n{rep}",
        ledger.snapshot()["spent_usd"]
    )))
}

pub fn write_report(out: &Path, label: &str) -> Result<String, HarnessDiagnostic> {
    let rows: Vec<Value> = std::fs::read_to_string(out.join("trials.jsonl"))
        .map_err(|e| q("SPX-HPQ001", format!("trials.jsonl: {e}")))?
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let read = |n: &str| {
        std::fs::read_to_string(out.join(n))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .unwrap_or(Value::Null)
    };
    let (meta, ledger) = (read("environment.json"), read("ledger.json"));
    let summary = report::summarize(&rows, &meta);
    let recs = report::recommendations(&summary);
    let pretty = |v: &Value| format!("{}\n", serde_json::to_string_pretty(v).unwrap_or_default());
    let w = |n: &str, t: String| {
        std::fs::write(out.join(n), t).map_err(|e| q("SPX-HPQ008", format!("{n}: {e}")))
    };
    w("summary.json", pretty(&summary))?;
    w("recommendations.json", pretty(&recs))?;
    w("outcomes.json", pretty(&report::outcomes(&rows)))?;
    w(
        "REPORT.md",
        render::render(&summary, &recs, &meta, &ledger, label),
    )?;
    Ok(format!(
        "wrote report for {} trials to {}\n",
        rows.len(),
        out.display()
    ))
}
