//! `apply <project> --session <result-dir|report.json> --expected-revision <digest>
//! [--compiler p] [--json]`: the separate final-application authority for a
//! session result (HN-02). Never publishes; drift is `SPX-HPD115`.

use super::compiler::{CompilerService, SubprocessCompiler};
use super::session::{apply_result, SESSION_FILE};
use super::snapshot::Snapshot;
use crate::cli::{Environment, Outcome};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{parse_strict, JsonLimits};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAX_APPLY_RECORD_BYTES: usize = 16 * 1024 * 1024;

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
        let v = read_json(&session, "session report")?;
        PathBuf::from(
            v.pointer("/session/result/dir")
                .and_then(Value::as_str)
                .ok_or_else(|| d("SPX-HPD115", "the report carries no session result"))?,
        )
    };
    let meta = read_json(&result_dir.join(SESSION_FILE), "session result record")?;
    let object = meta.as_object().filter(|object| {
        object.len() == 4
            && [
                "schema",
                "baseline_revision",
                "baseline_files",
                "result_revision",
            ]
            .iter()
            .all(|field| object.contains_key(*field))
    });
    let object = object.ok_or_else(|| {
        d(
            "SPX-HPD115",
            "session result record has missing or unknown fields",
        )
    })?;
    if object.get("schema").and_then(Value::as_str) != Some("semaprax.harness-session-result.v1") {
        return Err(d("SPX-HPD115", "session result record schema is invalid"));
    }
    if object.get("result_revision").and_then(Value::as_str) != Some(expected.as_str()) {
        return Err(d(
            "SPX-HPD115",
            "the session result is not at the expected revision",
        ));
    }
    let snap = Snapshot::capture(&project)?;
    let baseline_revision = object
        .get("baseline_revision")
        .and_then(Value::as_str)
        .ok_or_else(|| d("SPX-HPD115", "session record baseline revision is invalid"))?;
    let baseline_files = object
        .get("baseline_files")
        .and_then(Value::as_object)
        .ok_or_else(|| d("SPX-HPD115", "session record baseline files are invalid"))?
        .iter()
        .map(|(path, digest)| {
            digest
                .as_str()
                .map(|digest| (path.clone(), digest.to_string()))
                .ok_or_else(|| {
                    d(
                        "SPX-HPD115",
                        format!("session record digest for `{path}` is invalid"),
                    )
                })
        })
        .collect::<HarnessResult<BTreeMap<_, _>>>()?;
    if baseline_revision != snap.revision || baseline_files != snap.files {
        return Err(d(
            "SPX-HPD115",
            "session result baseline inventory differs from the live authenticated project",
        ));
    }
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

fn read_json(path: &Path, what: &str) -> HarnessResult<Value> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|e| d("SPX-HPD115", format!("{what}: {e}")))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_APPLY_RECORD_BYTES as u64
    {
        return Err(d(
            "SPX-HPD115",
            format!("{what} must be an ordinary file of at most {MAX_APPLY_RECORD_BYTES} bytes"),
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| d("SPX-HPD115", format!("{what}: {e}")))?;
    parse_strict(&bytes, &JsonLimits::frame(MAX_APPLY_RECORD_BYTES))
        .map_err(|error| d("SPX-HPD115", format!("{what}: {}", error.message)))
}
