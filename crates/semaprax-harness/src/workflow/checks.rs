//! Authorized checks: argv arrays from `[workflow.check.<name>]`, run through
//! the host's `command_view::execute` exactly once. The authoritative exit
//! status decides pass or fail; the view (a `command.view` provider's when one
//! resolves, e.g. RTK) is only what the model-facing report shows.

use super::stages::{CommandStage, RawCommandView};
use crate::cli::Environment;
use crate::command_view::retention::StreamName;
use crate::command_view::{execute, ExecOptions, Measurement, Recovered, ViewTokenizer};
use crate::diag::HarnessDiagnostic;
use crate::json::sha256_plain;
use crate::observe::Observer;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckSpec {
    pub name: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CheckRun {
    pub name: String,
    pub argv_digest: String,
    pub passed: bool,
    pub status: String,
    pub status_certain: bool,
    pub executions: u32,
    /// Model-facing text (never used for the verdict).
    pub view: String,
    pub view_route: String,
    pub view_provenance: String,
    pub view_incomplete: bool,
    /// Retained-raw recovery: `(project id, handle)`; never re-executes the check.
    pub recovery: Option<(String, String)>,
    /// Delivered-to-model measurement of `view` (HN-12).
    pub measurement: Option<Measurement>,
}

impl CheckRun {
    pub fn to_json(&self) -> Value {
        json!({"name": self.name, "argv_digest": self.argv_digest, "passed": self.passed,
               "status": self.status, "status_certain": self.status_certain, "executions": self.executions,
               "view": self.view, "view_route": self.view_route, "view_provenance": self.view_provenance,
               "view_incomplete": self.view_incomplete,
               "recovery": self.recovery.as_ref().map(|(p, h)| json!({"project_id": p, "handle": h})),
               "measurement": self.measurement.as_ref().map(Measurement::to_json)})
    }
}

/// Command stage that executes checks through the host's command view.
pub struct HostCommandChecks {
    pub env: Environment,
    /// Bypass any `command.view` provider (the single `--disable` switch).
    pub raw: bool,
    /// Named tokenizer used to measure and guard delivered views (HN-11 helper).
    pub tokenizer: Option<ViewTokenizer>,
    raw_view: RawCommandView,
}

impl HostCommandChecks {
    pub fn new(env: Environment) -> Self {
        Self {
            env,
            raw: false,
            tokenizer: None,
            raw_view: RawCommandView,
        }
    }
}

impl CommandStage for HostCommandChecks {
    fn id(&self) -> String {
        crate::command_view::run::HOST_PROVIDER.into()
    }
    fn view(&mut self, label: &str, raw: &str, max_bytes: usize) -> String {
        self.raw_view.view(label, raw, max_bytes)
    }
    fn run_check(
        &mut self,
        check: &CheckSpec,
        workdir: &Path,
        observer: &mut Observer,
    ) -> Option<Result<CheckRun, HarnessDiagnostic>> {
        let mut env = self.env.clone();
        env.cwd = workdir.to_path_buf();
        let project_id = workdir
            .canonicalize()
            .ok()
            .map(|p| sha256_plain(p.to_string_lossy().as_bytes()));
        let rep = execute(
            &env,
            workdir,
            &check.argv,
            &ExecOptions {
                raw: self.raw,
                tokenizer: self.tokenizer.clone(),
                ..Default::default()
            },
            Some(observer),
        );
        Some(rep.map(|r| {
            let t = &r.envelope.result.termination;
            CheckRun {
                name: check.name.clone(),
                argv_digest: r.envelope.result.argv_digest.clone(),
                passed: t.certain() && t.code() == 0,
                status: t.label(),
                status_certain: t.certain(),
                executions: r.envelope.result.executions,
                view: r.envelope.view.text.clone(),
                view_route: r.envelope.view.route.clone(),
                view_provenance: r.envelope.view.provenance.clone(),
                view_incomplete: r.envelope.view.incomplete,
                recovery: project_id.zip(r.envelope.result.recovery_handle.clone()),
                measurement: r.envelope.view.measurement.clone(),
            }
        }))
    }
}

impl HostCommandChecks {
    /// Bounded raw recovery of a finished check from host retention. Reads
    /// retained bytes only; the check is never executed again.
    pub fn recover_raw(
        &self,
        run: &CheckRun,
        stream: StreamName,
        offset: u64,
        limit: u64,
    ) -> Result<Recovered, HarnessDiagnostic> {
        let (pid, handle) = run.recovery.as_ref().ok_or_else(|| {
            HarnessDiagnostic::new("SPX-HPH031", "this check run has no retained raw output")
        })?;
        crate::command_view::recover::recover_by_id(&self.env, pid, handle, stream, offset, limit)
    }
}

/// Largest check view carried into a model request (bytes).
pub const FEEDBACK_VIEW_MAX: usize = 8192;

/// From the failed-check report (`Report.checks`), the model-facing feedback for
/// the failing authorized check and a message whose digest changes with the
/// output (so a different failure is progress, an identical one is not).
pub fn check_feedback(checks: &Value, message: &str) -> (String, Option<Value>) {
    let Some(run) = checks["commands"]
        .as_array()
        .and_then(|a| a.iter().rev().find(|c| c["passed"] == false))
    else {
        return (message.to_string(), None);
    };
    let view = run["view"].as_str().unwrap_or("");
    let (text, cut) = crate::command_view::guard::bound(view, FEEDBACK_VIEW_MAX);
    let digest = sha256_plain(view.as_bytes());
    let msg = format!("{message}; output {}", &digest[7..19]);
    let m = &run["measurement"];
    let fb = json!({"check": run["name"], "status": run["status"], "status_certain": run["status_certain"],
        "route": run["view_route"], "incomplete": run["view_incomplete"].as_bool() == Some(true) || cut,
        "recovery_handle": run["recovery"]["handle"], "recovery_project_id": run["recovery"]["project_id"],
        "output": text,
        "delivered": {"basis": m["basis"], "raw_tokens": m["raw_tokens"], "delivered_tokens": m["delivered_tokens"],
                      "saved_tokens": m["saved_tokens"], "decision": m["decision"], "tokenizer": m["tokenizer"]}});
    (msg, Some(fb))
}
