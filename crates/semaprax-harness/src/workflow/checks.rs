//! Authorized checks: argv arrays from `[workflow.check.<name>]`, run through
//! the host's `command_view::execute` exactly once. The authoritative exit
//! status decides pass or fail; the view (a `command.view` provider's when one
//! resolves, e.g. RTK) is only what the model-facing report shows.

use super::stages::{CommandStage, RawCommandView};
use crate::cli::Environment;
use crate::command_view::{execute, ExecOptions};
use crate::diag::HarnessDiagnostic;
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
}

impl CheckRun {
    pub fn to_json(&self) -> Value {
        json!({"name": self.name, "argv_digest": self.argv_digest, "passed": self.passed,
               "status": self.status, "status_certain": self.status_certain, "executions": self.executions,
               "view": self.view, "view_route": self.view_route, "view_provenance": self.view_provenance})
    }
}

/// Command stage that executes checks through the host's command view.
pub struct HostCommandChecks {
    pub env: Environment,
    /// Bypass any `command.view` provider (the single `--disable` switch).
    pub raw: bool,
    raw_view: RawCommandView,
}

impl HostCommandChecks {
    pub fn new(env: Environment) -> Self {
        Self {
            env,
            raw: false,
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
        let rep = execute(
            &env,
            workdir,
            &check.argv,
            &ExecOptions {
                raw: self.raw,
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
            }
        }))
    }
}
