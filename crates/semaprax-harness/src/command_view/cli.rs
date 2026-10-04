//! `exec` and `recover` verbs.

use super::policy::Policy;
use super::retention::{valid_handle, Retention, StreamName};
use super::run::{execute, ExecOptions};
use crate::cli::{Environment, Outcome};
use crate::diag::HarnessDiagnostic;
use crate::json::{canonical, sha256_plain};
use crate::observe::sink::{JsonlFileSink, Observer, ObserverLimits};
use serde_json::json;
use std::path::PathBuf;

fn num(v: &str, what: &str) -> Result<u64, Outcome> {
    v.parse()
        .map_err(|_| Outcome::usage(format!("{what} must be a non-negative integer")))
}

pub fn exec(args: &[String], env: &Environment) -> Outcome {
    let (head, argv) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => return Outcome::usage("exec needs `-- <argv...>`"),
    };
    if argv.is_empty() {
        return Outcome::usage("exec needs a command after `--`");
    }
    let mut opts = ExecOptions::default();
    let mut project: Option<&String> = None;
    let mut obs_path: Option<PathBuf> = None;
    let mut it = head.iter();
    while let Some(a) = it.next() {
        let mut val = |what: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| Outcome::usage(format!("{what} needs a value")))
        };
        match a.as_str() {
            "--raw" => opts.raw = true,
            "--json" => opts.json = true,
            "--timeout-ms" => match val(a).and_then(|v| num(&v, a)) {
                Ok(n) => opts.timeout_ms = Some(n),
                Err(o) => return o,
            },
            "--external-owner" => match val(a) {
                Ok(v) => opts.external_owner = Some(v),
                Err(o) => return o,
            },
            "--observations" => match val(a) {
                Ok(v) => obs_path = Some(env.cwd.join(v)),
                Err(o) => return o,
            },
            "--env" => match val(a) {
                Ok(v) => match v.split_once('=') {
                    Some((k, x)) => {
                        opts.extra_env.insert(k.to_string(), x.to_string());
                    }
                    None => return Outcome::usage("--env needs KEY=VALUE"),
                },
                Err(o) => return o,
            },
            s if s.starts_with("--") => return Outcome::usage(format!("unknown option `{s}`")),
            _ if project.is_none() => project = Some(a),
            _ => return Outcome::usage("exec takes one project before `--`"),
        }
    }
    let Some(project) = project else {
        return Outcome::usage("exec needs a project");
    };
    let observer = obs_path.map(|p| match JsonlFileSink::create(&p, 1 << 20) {
        Ok(s) => Ok(Observer::new(Some(Box::new(s)), ObserverLimits::default())),
        Err(e) => Err(Outcome::refused(&HarnessDiagnostic::new(
            "SPX-HPH031",
            format!("observations file: {e}"),
        ))),
    });
    let mut observer = match observer {
        Some(Err(o)) => return o,
        Some(Ok(obs)) => Some(obs),
        None => None,
    };
    let r = execute(env, &env.cwd.join(project), argv, &opts, observer.as_mut());
    if let Some(o) = observer.as_mut() {
        o.finish();
    }
    match r {
        Ok(rep) => Outcome {
            code: rep.envelope.result.termination.code(),
            stdout: rep.display,
            stderr: String::new(),
        },
        Err(d) => Outcome::refused(&d),
    }
}

pub fn recover(args: &[String], env: &Environment) -> Outcome {
    let (mut pos, mut offset, mut limit, mut stream, mut as_json) =
        (Vec::new(), 0u64, 65536u64, StreamName::Stdout, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned();
        match a.as_str() {
            "--offset" | "--limit" => match val()
                .ok_or_else(|| Outcome::usage(format!("{a} needs a value")))
                .and_then(|v| num(&v, a))
            {
                Ok(n) => {
                    if a == "--offset" {
                        offset = n
                    } else {
                        limit = n.min(4 << 20)
                    }
                }
                Err(o) => return o,
            },
            "--stream" => match val().as_deref().and_then(StreamName::parse) {
                Some(s) => stream = s,
                None => return Outcome::usage("--stream must be stdout or stderr"),
            },
            "--json" => as_json = true,
            s if s.starts_with("--") => return Outcome::usage(format!("unknown option `{s}`")),
            _ => pos.push(a.clone()),
        }
    }
    let [project, handle] = pos.as_slice() else {
        return Outcome::usage(
            "recover <project> <handle> [--offset N] [--limit N] [--stream stdout|stderr]",
        );
    };
    let run = || -> Result<Outcome, HarnessDiagnostic> {
        let bad = |m: &str| HarnessDiagnostic::new("SPX-HPH031", m.to_string());
        let policy = Policy::load(env)?;
        let rp = policy
            .retention
            .ok_or_else(|| bad("retention is not enabled by policy"))?;
        let home = env
            .harness_home
            .as_ref()
            .ok_or_else(|| bad("no harness home"))?;
        if !valid_handle(handle) {
            return Err(HarnessDiagnostic::new(
                "SPX-HPH040",
                "recovery handle must look like `cv-<24 hex>`",
            ));
        }
        let root = env
            .cwd
            .join(project)
            .canonicalize()
            .map_err(|e| bad(&format!("project path: {e}")))?;
        let pid = sha256_plain(root.to_string_lossy().as_bytes());
        let r = Retention::open(home, &pid, &rp)?;
        let (bytes, total) = r.read(handle, stream, offset, limit)?;
        let (text, _) = super::guard::decode(&bytes);
        let next = offset + bytes.len() as u64;
        Ok(Outcome::ok(if as_json {
            format!(
                "{}\n",
                canonical(
                    &json!({"handle": handle, "offset": offset, "bytes": bytes.len(), "stream_total": total, "next_offset": if next < total { json!(next) } else { json!(null) }, "text": text})
                )
            )
        } else {
            let mut t = text;
            if next < total {
                t.push_str(&format!(
                    "\n[recover: {} of {total} bytes shown; continue with --offset {next}]\n",
                    bytes.len()
                ));
            }
            t
        }))
    };
    match run() {
        Ok(o) => o,
        Err(d) => Outcome::refused(&d),
    }
}
