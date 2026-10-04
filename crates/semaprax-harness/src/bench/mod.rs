//! Development-journey benchmark contract and report (HP-17).
//!
//! `semaprax.harness-benchmark.v1`: a pinned corpus, profiles that are only
//! configuration plus adopted descriptors, per-cell metrics, predeclared gates
//! and a recommendation derived from them. Diagnostics `SPX-HPQ001..`: 001
//! read, 002 schema, 003 unknown member, 004 pin drift, 005 duplicate id, 006
//! unknown reference, 007 usage, 008 output, 010 router adapter, 011 grader
//! validation failed, 012 tokenizer unavailable (application tasks). See
//! `docs/HARNESS-BENCHMARK-V1.md`.

pub mod adversarial;
pub mod apptask;
pub mod arena;
pub mod cell;
pub mod corpus;
pub mod gates;
pub mod measure;
pub mod pilot;
pub mod report_md;
pub mod router;
pub mod run;
pub mod summary;

use crate::cli::{Environment, Outcome};
use crate::diag::HarnessDiagnostic;
use crate::observe::{JsonlFileSink, Observer, ObserverLimits};
use corpus::Corpus;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const USAGE: &str = "bench app ... (application tasks, see docs) | bench <corpus-dir> [--profile NAME]... [--out DIR] [--json] [--warm N] [--env K=V]... [--repo DIR] [--work DIR] [--label TEXT] [--no-adversarial] [--no-measure] [--pilot HOST:PORT --pilot-model NAME [--pilot-reps N] [--pilot-calls N]]";

fn q(code: &'static str, m: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, m)
}

#[derive(Default)]
struct Args {
    corpus: Option<String>,
    profiles: Vec<String>,
    out: Option<String>,
    json: bool,
    warm: Option<u32>,
    vars: Vec<(String, String)>,
    repo: Option<String>,
    work: Option<String>,
    label: Option<String>,
    adversarial: bool,
    measure: bool,
    cell: Option<String>,
    pilot: Option<String>,
    pilot_model: Option<String>,
    pilot_reps: u32,
    pilot_calls: u32,
    skill_reps: u32,
    pilot_profiles: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, HarnessDiagnostic> {
    let mut a = Args {
        adversarial: true,
        measure: true,
        pilot_reps: 2,
        pilot_calls: 60,
        skill_reps: 6,
        ..Args::default()
    };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = |n: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| q("SPX-HPQ007", format!("`{n}` needs a value")))
        };
        match x.as_str() {
            "--profile" => a.profiles.push(val(x)?),
            "--out" => a.out = Some(val(x)?),
            "--json" => a.json = true,
            "--warm" => {
                a.warm = Some(
                    val(x)?
                        .parse()
                        .map_err(|_| q("SPX-HPQ007", "--warm needs an integer"))?,
                )
            }
            "--env" => {
                let v = val(x)?;
                let (k, v) = v
                    .split_once('=')
                    .ok_or_else(|| q("SPX-HPQ007", "--env needs KEY=VALUE"))?;
                a.vars.push((k.to_string(), v.to_string()));
            }
            "--repo" => a.repo = Some(val(x)?),
            "--work" => a.work = Some(val(x)?),
            "--label" => a.label = Some(val(x)?),
            "--no-adversarial" => a.adversarial = false,
            "--no-measure" => a.measure = false,
            "--cell" => a.cell = Some(val(x)?),
            "--pilot" => a.pilot = Some(val(x)?),
            "--pilot-model" => a.pilot_model = Some(val(x)?),
            "--pilot-reps" => {
                a.pilot_reps = val(x)?
                    .parse()
                    .map_err(|_| q("SPX-HPQ007", "--pilot-reps needs an integer"))?
            }
            "--pilot-skill-reps" => {
                a.skill_reps = val(x)?
                    .parse()
                    .map_err(|_| q("SPX-HPQ007", "--pilot-skill-reps needs an integer"))?
            }
            "--pilot-profile" => a.pilot_profiles.push(val(x)?),
            "--pilot-calls" => {
                a.pilot_calls = val(x)?
                    .parse()
                    .map_err(|_| q("SPX-HPQ007", "--pilot-calls needs an integer"))?
            }
            f if f.starts_with("--") => return Err(q("SPX-HPQ007", format!("unknown flag `{f}`"))),
            p if a.corpus.is_none() => a.corpus = Some(p.to_string()),
            _ => return Err(q("SPX-HPQ007", "expected exactly one corpus directory")),
        }
    }
    Ok(a)
}

pub fn cli_bench(args: &[String], env: &Environment) -> Outcome {
    if args.first().map(String::as_str) == Some("app") {
        return apptask::cli_app(&args[1..], env);
    }
    match parse(args).and_then(|a| execute(a, env)) {
        Ok(o) => o,
        Err(e) if e.code == "SPX-HPQ007" => {
            Outcome::usage(format!("bench: {}\nusage: {USAGE}", e.message))
        }
        Err(e) => Outcome::refused(&e),
    }
}

fn abs(env: &Environment, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        env.cwd.join(p)
    }
}

/// Variables the run may forward: explicit `--env` first, then the host
/// `Environment` (only `HARNESS_*` names ever reach an adapter).
fn collect_vars(a: &Args, env: &Environment) -> BTreeMap<String, String> {
    let mut v: BTreeMap<String, String> = env
        .vars
        .iter()
        .filter(|(k, _)| k.starts_with("HARNESS_"))
        .map(|(k, x)| (k.clone(), x.clone()))
        .collect();
    for (k, x) in &a.vars {
        v.insert(k.clone(), x.clone());
    }
    v
}

fn self_exe() -> Option<run::SelfExe> {
    let exe = std::env::current_exe().ok()?;
    let name = exe.file_name()?.to_string_lossy().into_owned();
    if !Path::new("/usr/bin/time").exists() {
        return None;
    }
    // Test binaries are not the CLI; only the real binaries re-enter `bench`.
    let prefix = if name.starts_with("semaprax-harness") {
        vec![]
    } else if name == "semaprax" {
        vec!["harness".to_string()]
    } else {
        return None;
    };
    Some(run::SelfExe { exe, prefix })
}

fn execute(a: Args, env: &Environment) -> Result<Outcome, HarnessDiagnostic> {
    if let Some(spec) = &a.cell {
        // Child mode: one cell, JSON on stdout.
        let v: Value = serde_json::from_slice(
            &std::fs::read(abs(env, spec))
                .map_err(|e| q("SPX-HPQ001", format!("cell spec: {e}")))?,
        )
        .map_err(|e| q("SPX-HPQ001", format!("cell spec: {e}")))?;
        let clock = measure::SystemClock::new();
        let out = run::cell_from_spec(&v, &clock).map_err(|e| q("SPX-HPQ001", e))?;
        return Ok(Outcome::ok(out.to_string()));
    }
    let dir = abs(
        env,
        a.corpus
            .as_deref()
            .ok_or_else(|| q("SPX-HPQ007", "missing corpus directory"))?,
    );
    let corpus = Corpus::load(&dir)?;
    let repo = match &a.repo {
        Some(r) => abs(env, r),
        None => arena::locate_repo(&dir)
            .ok_or_else(|| q("SPX-HPQ007", "cannot locate the repository (use --repo)"))?,
    };
    for p in &a.profiles {
        if corpus.profile(p).is_none() {
            return Err(q("SPX-HPQ006", format!("unknown profile `{p}`")));
        }
    }
    let work = match &a.work {
        Some(w) => abs(env, w),
        None => std::env::temp_dir().join(format!("semaprax-bench-{}", &corpus.digest[7..15])),
    };
    let vars = collect_vars(&a, env);
    let clock = measure::SystemClock::new();
    let opts = run::RunOptions {
        corpus: &corpus,
        repo,
        work: work.clone(),
        vars: vars.clone(),
        compiler: env.compiler.clone(),
        profiles: a.profiles.clone(),
        warm: a.warm,
        clock: &clock,
        measure_with: if a.measure { self_exe() } else { None },
        adversarial: a.adversarial,
    };
    let out = run::run_matrix(&opts);
    let baseline = corpus
        .profiles
        .iter()
        .find(|p| p.baseline)
        .map(|p| p.id.clone())
        .unwrap_or_default();
    let warm = a.warm.unwrap_or(corpus.warm);
    let mut summary = summary::build(&out, &baseline, &corpus.digest, (corpus.cold, warm));
    let adv: Vec<Value> = out.adversarial.iter().map(|x| x.to_json()).collect();
    let gates = gates::evaluate(&out.cells, &adv, &summary, &baseline);

    let pilot = match &a.pilot {
        Some(addr) => {
            let pick = |p: &corpus::ProfileSpec| {
                p.baseline
                    || (a.pilot_profiles.is_empty() && p.touches().contains(&"context"))
                    || a.pilot_profiles.contains(&p.id)
            };
            let arenas: Vec<arena::Arena> = corpus
                .profiles
                .iter()
                .filter(|p| pick(p) && p.skills.is_empty())
                .map(|p| arena::Arena::attach(&corpus, p, &work, &vars, env.compiler.clone()))
                .filter(|ar| ar.root.join("home").is_dir())
                .collect();
            let refs: Vec<&arena::Arena> = arenas.iter().collect();
            let skill_calls = a.skill_reps * 2;
            let cfg = pilot::PilotConfig {
                addr: addr.clone(),
                model: a.pilot_model.clone().unwrap_or_default(),
                reps: a.pilot_reps,
                max_calls: a.pilot_calls.saturating_sub(skill_calls),
            };
            let mut p = pilot::run_pilot(&corpus, &refs, &cfg);
            let skill_profile = corpus.profiles.iter().find(|p| !p.skills.is_empty());
            if let (Some(sp), Some(base)) =
                (skill_profile, refs.iter().find(|r| r.profile.baseline))
            {
                let sa = arena::Arena::attach(&corpus, sp, &work, &vars, env.compiler.clone());
                p["skill_pilot"] = pilot::run_skill_pilot(&corpus, base, &sa, &cfg, a.skill_reps);
            }
            p
        }
        None => Value::Null,
    };
    summary["pilot"] = if pilot.is_null() {
        Value::Null
    } else {
        json!({"status": pilot["status"], "label": pilot["label"], "calls": pilot["calls"]})
    };
    summary["gates"] = gates.clone();

    if let Some(o) = &a.out {
        let odir = abs(env, o);
        write_outputs(
            &odir,
            &corpus,
            &out,
            &summary,
            &gates,
            &pilot,
            &work,
            &vars,
            env,
            a.label.as_deref().unwrap_or("local run"),
        )
        .map_err(|e| q("SPX-HPQ008", format!("write results: {e}")))?;
    }
    if a.json {
        return Ok(Outcome::ok(format!(
            "{}\n",
            crate::json::canonical(&summary)
        )));
    }
    let mut text = format!("benchmark {} ({} cells)\n", corpus.id, out.cells.len());
    for (p, s) in summary["profiles"].as_object().into_iter().flatten() {
        text.push_str(&format!(
            "  {p}: accepted {}/{} visible {} bytes, false negatives {}, cells ok/failed/untested {}/{}/{}\n",
            s["accepted"]["count"], s["accepted"]["n"], s["bytes"]["model_visible"], s["false_negatives"],
            s["cells"]["ok"], s["cells"]["failed"], s["cells"]["untested"],
        ));
    }
    for ad in &out.adversarial {
        text.push_str(&format!(
            "  adversarial {}: {}\n",
            ad.id,
            if ad.detected {
                "detected"
            } else if ad.untested.is_some() {
                "untested"
            } else {
                "NOT DETECTED"
            }
        ));
    }
    Ok(Outcome::ok(text))
}

#[allow(clippy::too_many_arguments)]
fn write_outputs(
    dir: &Path,
    corpus: &Corpus,
    out: &run::RunOutput,
    summary: &Value,
    gates: &Value,
    pilot: &Value,
    work: &Path,
    vars: &BTreeMap<String, String>,
    env: &Environment,
    label: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("observations"))?;
    for (p, events) in &out.events {
        let path = dir.join("observations").join(format!("{p}.jsonl"));
        let _ = std::fs::remove_file(&path);
        let sink = JsonlFileSink::create(&path, 16 << 20)?;
        let mut obs = Observer::new(
            Some(Box::new(sink)),
            ObserverLimits {
                max_events: 100_000,
            },
        );
        for e in events {
            obs.record(e.clone());
        }
        obs.finish();
    }
    let cells: String = out
        .cells
        .iter()
        .map(|c| format!("{}\n", crate::json::canonical(c)))
        .collect();
    std::fs::write(dir.join("cells.jsonl"), cells)?;
    std::fs::write(
        dir.join("summary.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(summary).unwrap_or_default()
        ),
    )?;
    if !pilot.is_null() {
        std::fs::write(
            dir.join("pilot.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(pilot).unwrap_or_default()
            ),
        )?;
    }
    let uname = std::process::Command::new("/usr/bin/uname")
        .arg("-srm")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut installs = serde_json::Map::new();
    for p in &corpus.profiles {
        if let Ok(t) = std::fs::read_to_string(work.join(&p.id).join("home/installations.json")) {
            installs.insert(
                p.id.clone(),
                serde_json::from_str(&t).unwrap_or(Value::Null),
            );
        }
    }
    let ver = |exe: Option<&String>, arg: &str| {
        exe.and_then(|e| {
            std::process::Command::new(e)
                .arg(arg)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .output()
                .ok()
        })
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let environment = json!({
        "os": uname, "label": label, "corpus_id": corpus.id, "corpus_digest": corpus.digest, "source_commit": corpus.source_commit,
        "seed": corpus.seed, "variables_provided": vars.keys().collect::<Vec<_>>(), "tool_paths": vars,
        "compiler": env.compiler, "compiler_version": ver(env.compiler.as_ref().map(|c| c.display().to_string()).as_ref(), "--version"),
        "python_version": ver(vars.get("HARNESS_PYTHON"), "--version"), "node_version": ver(vars.get("HARNESS_NODE"), "--version"),
        "adopted_installations": installs, "tokenizer": "byte-v1 only (no named tokenizer on this machine)",
    });
    std::fs::write(
        dir.join("environment.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&environment).unwrap_or_default()
        ),
    )?;
    std::fs::write(
        dir.join("REPORT.md"),
        report_md::render(label, summary, gates, pilot, &environment),
    )?;
    Ok(())
}
