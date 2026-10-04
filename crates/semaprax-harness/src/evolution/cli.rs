//! `semaprax harness evolve run|promote|status` (HN-15).

use super::adapter::{Cancel, ProcessAdapter};
use super::{run, spec};
use crate::cli::{Environment, Outcome};
use crate::diag::HarnessDiagnostic;
use serde_json::Value;
use std::path::{Path, PathBuf};

const USE: &str = "usage: evolve run <experiment.json> [--json] | evolve promote <workspace> --to <skills-root> --approve | evolve status <workspace>";

fn abs(env: &Environment, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        env.cwd.join(p)
    }
}

fn refuse(d: HarnessDiagnostic) -> Outcome {
    Outcome::refused(&d)
}

pub fn cli_evolve(args: &[String], env: &Environment) -> Outcome {
    if args.iter().any(|a| a == "--auto-promote") {
        return refuse(super::w(
            "SPX-HPW009",
            "automatic promotion is disabled: promotion is a separate explicit action",
        ));
    }
    match args.first().map(String::as_str) {
        Some("run") if args.len() >= 2 => {
            let path = abs(env, &args[1]);
            let parsed = std::fs::read(&path)
                .map_err(|e| super::w("SPX-HPW001", format!("{}: {e}", path.display())))
                .and_then(|b| {
                    serde_json::from_slice::<Value>(&b)
                        .map_err(|e| super::w("SPX-HPW001", e.to_string()))
                })
                .and_then(|v| spec::parse(&v, path.parent().unwrap_or(Path::new("/"))));
            let sp = match parsed {
                Ok(s) => s,
                Err(d) => return refuse(d),
            };
            let cancel = Cancel {
                flag: Default::default(),
                file: Some(sp.workspace_root.join(&sp.id).join("CANCEL")),
            };
            let mut adapter = ProcessAdapter {
                command: sp.adapter_command.clone(),
                env: sp.adapter_env.clone(),
            };
            match run::run_experiment(&sp, &mut adapter, &cancel, &env.cwd) {
                Ok(rep) => {
                    let text = if args.iter().any(|a| a == "--json") {
                        crate::json::canonical(&rep.result) + "\n"
                    } else {
                        format!(
                            "experiment {}: {}\nreason: {}\nworkspace: {}\n",
                            sp.id,
                            rep.outcome.as_str(),
                            rep.result["reason"].as_str().unwrap_or(""),
                            rep.workspace.display()
                        )
                    };
                    let bad = matches!(
                        rep.outcome,
                        run::Outcome::Unavailable | run::Outcome::Aborted
                    );
                    Outcome {
                        code: i32::from(bad),
                        stdout: text,
                        stderr: String::new(),
                    }
                }
                Err(d) => refuse(d),
            }
        }
        Some("promote") if args.len() >= 2 => {
            let to = args
                .iter()
                .position(|a| a == "--to")
                .and_then(|i| args.get(i + 1));
            let Some(to) = to else {
                return Outcome::usage(USE);
            };
            let approve = args.iter().any(|a| a == "--approve");
            match run::promote(&abs(env, &args[1]), &abs(env, to), approve) {
                Ok(v) => Outcome::ok(crate::json::canonical(&v) + "\n"),
                Err(d) => refuse(d),
            }
        }
        Some("status") if args.len() >= 2 => {
            match std::fs::read_to_string(abs(env, &args[1]).join("evidence/result.json")) {
                Ok(s) => Outcome::ok(s),
                Err(e) => refuse(super::w("SPX-HPW001", format!("no result: {e}"))),
            }
        }
        _ => Outcome::usage(USE),
    }
}
