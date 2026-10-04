//! `semaprax harness conformance <descriptor> [--suite ..] [--upstream ..]
//! [--runtime ..] [--hostile-runtime ..] [--isolation none|restricted]
//! [--env K=V]... [--json]`.

use super::{run, Options, SUITES};
use crate::cli::{Environment, Outcome};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::path::PathBuf;

fn usage(m: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPP001", m)
}

fn parse(args: &[String], env: &Environment) -> HarnessResult<(Options, bool)> {
    let mut o = Options::default();
    let (mut json, mut desc) = (false, None);
    let abs = |what: &str, v: &str| {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            Ok(p)
        } else {
            Err(usage(format!("{what} must be an absolute path")))
        }
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |what: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| usage(format!("`{what}` needs a value")))
        };
        match a.as_str() {
            "--json" => json = true,
            "--suite" => {
                let s = val("--suite")?;
                if !SUITES.contains(&s.as_str()) {
                    return Err(usage(format!(
                        "unknown suite `{s}` (one of {})",
                        SUITES.join(", ")
                    )));
                }
                o.suites.push(s);
            }
            "--upstream" => o.upstream = Some(abs("--upstream", &val("--upstream")?)?),
            "--runtime" => o.runtime = Some(abs("--runtime", &val("--runtime")?)?),
            "--hostile-runtime" => {
                o.hostile_runtime = Some(abs("--hostile-runtime", &val("--hostile-runtime")?)?)
            }
            "--hostile" => o.hostile_dir = Some(abs("--hostile", &val("--hostile")?)?),
            "--isolation" => match val("--isolation")?.as_str() {
                "none" => o.restricted = false,
                "restricted" => o.restricted = true,
                _ => return Err(usage("--isolation is `none` or `restricted`")),
            },
            "--env" => {
                let kv = val("--env")?;
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| usage("--env needs NAME=VALUE"))?;
                o.forward_env.insert(k.into(), v.into());
            }
            x if x.starts_with("--") => return Err(usage(format!("unknown option `{x}`"))),
            x => {
                if desc.replace(x.to_string()).is_some() {
                    return Err(usage("expected exactly one descriptor"));
                }
            }
        }
    }
    let d = desc.ok_or_else(|| usage("expected a descriptor path"))?;
    o.descriptor = env.cwd.join(d);
    Ok((o, json))
}

pub fn cli_conformance(args: &[String], env: &Environment) -> Outcome {
    let (opts, json) = match parse(args, env) {
        Ok(x) => x,
        Err(e) => return Outcome::usage(e.message),
    };
    match run(&opts, env) {
        Ok(r) => Outcome {
            code: i32::from(r.verdict() == super::Verdict::Fail),
            stdout: if json {
                r.render_json()
            } else {
                r.render_human()
            },
            stderr: String::new(),
        },
        Err(e) => Outcome::refused(&e),
    }
}
