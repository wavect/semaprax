//! `apply <project> --session <result-dir|report.json> --expected-revision <digest>
//! [--compiler p] [--json]`: the separate final-application authority for a
//! session result (HN-02). Never publishes; drift is `SPX-HPD115`.

use super::compiler::{CompilerService, SubprocessCompiler};
use super::session::{apply_result, SESSION_FILE};
use super::snapshot::Snapshot;
use crate::cli::{Environment, Outcome};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

pub fn cli_apply(args: &[String], env: &Environment) -> Outcome {
    match run(args, env) {
        Ok(o) => o,
        Err(e) if e.code == "SPX-HPD080" => Outcome::usage(format!("apply: {}", e.message)),
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

fn run(args: &[String], env: &Environment) -> HarnessResult<Outcome> {
    let usage = |m: &str| d("SPX-HPD080", m);
    let (mut project, mut session, mut expected, mut compiler, mut json_out) =
        (None, None, None, None, false);
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = |n: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| usage(&format!("`{n}` needs a value")))
        };
        match x.as_str() {
            "--session" => session = Some(val("--session")?),
            "--expected-revision" => expected = Some(val("--expected-revision")?),
            "--compiler" => compiler = Some(val("--compiler")?),
            "--json" => json_out = true,
            o if o.starts_with("--") => return Err(usage(&format!("unknown option `{o}`"))),
            p if project.is_none() => project = Some(p.to_string()),
            _ => return Err(usage("expected exactly one project")),
        }
    }
    let project = abs(env, &project.ok_or_else(|| usage("missing project"))?);
    let session = abs(env, &session.ok_or_else(|| usage("missing --session"))?);
    let expected = expected.ok_or_else(|| usage("missing --expected-revision"))?;
    let exe = match (compiler, &env.compiler) {
        (Some(c), _) => abs(env, &c),
        (None, Some(c)) => c.clone(),
        _ => {
            return Err(d(
                "SPX-HPD001",
                "no compiler: pass --compiler or set SEMAPRAX_COMPILER",
            ))
        }
    };
    let result_dir = if session.is_dir() {
        session
    } else {
        let v: Value = serde_json::from_slice(
            &std::fs::read(&session)
                .map_err(|e| d("SPX-HPD115", format!("session report: {e}")))?,
        )
        .map_err(|e| d("SPX-HPD115", format!("session report is not JSON: {e}")))?;
        PathBuf::from(
            v.pointer("/session/result/dir")
                .and_then(Value::as_str)
                .ok_or_else(|| d("SPX-HPD115", "the report carries no session result"))?,
        )
    };
    let meta: Value = serde_json::from_slice(
        &std::fs::read(result_dir.join(SESSION_FILE))
            .map_err(|e| d("SPX-HPD115", format!("session result record: {e}")))?,
    )
    .map_err(|e| d("SPX-HPD115", format!("session result record: {e}")))?;
    if meta["result_revision"].as_str() != Some(expected.as_str()) {
        return Err(d(
            "SPX-HPD115",
            "the session result is not at the expected revision",
        ));
    }
    // The baseline the session was computed from: current files must still match it.
    let mut snap = Snapshot::capture(&project)?;
    snap.revision = meta["baseline_revision"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    snap.files = meta["baseline_files"]
        .as_object()
        .ok_or_else(|| d("SPX-HPD115", "session record lacks the baseline"))?
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
        .collect();
    let home = env
        .harness_home
        .clone()
        .ok_or_else(|| d("SPX-HPD080", "SEMAPRAX_HARNESS_HOME is not set"))?;
    let scratch = home.join("cache/workflow/apply");
    let svc: &dyn CompilerService = &SubprocessCompiler::new(exe, scratch)?;
    let applied = apply_result(&snap, &result_dir, &expected, svc)?;
    let out = json!({"schema": "semaprax.harness-apply.v1", "status": "applied", "revision": expected,
        "applied_files": applied, "published": false});
    Ok(Outcome {
        code: 0,
        stdout: if json_out {
            format!("{}\n", crate::json::canonical(&out))
        } else {
            format!(
                "applied {} file(s) at {expected}; nothing was published\n",
                applied.len()
            )
        },
        stderr: String::new(),
    })
}
