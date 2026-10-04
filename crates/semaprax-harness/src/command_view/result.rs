//! The authoritative command result and the separate model-facing view. The
//! result is built only from what the host observed; a provider can influence
//! `ModelView` text and nothing else.

use super::executor::Termination;
use crate::json::{canonical, sha256_plain};
use serde_json::{json, Value};

pub const ENVELOPE_SCHEMA: &str = "semaprax.harness-command-envelope.v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamRecord {
    pub bytes: u64,
    pub digest: String,
    /// The stream ended and every byte is retained for recovery.
    pub retained_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandResult {
    pub argv: Vec<String>,
    pub argv_digest: String,
    /// Set only for an accepted pre-execution wrapper plan.
    pub effective_argv: Option<Vec<String>>,
    /// Absolute path actually executed.
    pub executable: String,
    pub cwd: String,
    /// Sorted names of the environment variables the command received.
    pub env_grant: Vec<String>,
    pub termination: Termination,
    pub stdout: StreamRecord,
    pub stderr: StreamRecord,
    pub recovery_handle: Option<String>,
    /// Always 1: the host executes exactly once.
    pub executions: u32,
    pub lineage: Vec<String>,
}

pub fn argv_digest(argv: &[String]) -> String {
    sha256_plain(canonical(&json!(argv)).as_bytes())
}

impl CommandResult {
    pub fn certain(&self) -> bool {
        self.termination.certain()
    }

    pub fn to_json(&self) -> Value {
        let s = |r: &StreamRecord| json!({"bytes": r.bytes, "digest": r.digest, "retained_complete": r.retained_complete});
        json!({
            "argv": self.argv, "argv_digest": self.argv_digest, "effective_argv": self.effective_argv,
            "executable": self.executable, "cwd": self.cwd, "env_grant": self.env_grant,
            "status": self.termination.label(), "status_certain": self.certain(),
            "stdout": s(&self.stdout), "stderr": s(&self.stderr),
            "recovery_handle": self.recovery_handle, "executions": self.executions, "lineage": self.lineage,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelView {
    pub text: String,
    pub lossless: bool,
    pub omissions: u64,
    /// Provider id, or `semaprax.host/raw` when the host produced the text.
    pub provenance: String,
    pub recovery_handle: Option<String>,
    /// The view is not a complete account: truncated, a critical line was
    /// missing, or the command's own status is uncertain.
    pub incomplete: bool,
    /// Route taken: `provider`, `raw`, `wrapper`.
    pub route: String,
    pub notes: Vec<String>,
    /// Delivered-to-model measurement (HN-12); never a billing figure.
    pub measurement: Option<super::measure::Measurement>,
}

impl ModelView {
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "text": self.text, "lossless": self.lossless, "omissions": self.omissions, "provenance": self.provenance,
            "recovery_handle": self.recovery_handle, "incomplete": self.incomplete, "route": self.route, "notes": self.notes,
        });
        if let Some(m) = &self.measurement {
            v["measurement"] = m.to_json();
        }
        v
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub result: CommandResult,
    pub view: ModelView,
}

impl Envelope {
    pub fn to_json(&self) -> Value {
        json!({"schema": ENVELOPE_SCHEMA, "result": self.result.to_json(), "view": self.view.to_json()})
    }

    /// The exact text a model sees in human mode.
    pub fn render(&self) -> String {
        let r = &self.result;
        let v = &self.view;
        let mut out = format!(
            "[exec] status={} certain={} stdout={}B stderr={}B executions={}\n[view] route={} provenance={} lossless={} omissions={} incomplete={}",
            r.termination.label(),
            yn(r.certain()),
            r.stdout.bytes,
            r.stderr.bytes,
            r.executions,
            v.route,
            v.provenance,
            yn(v.lossless),
            v.omissions,
            yn(v.incomplete),
        );
        if let Some(h) = &v.recovery_handle {
            out.push_str(&format!(" recovery={h}"));
        }
        out.push('\n');
        for n in &v.notes {
            out.push_str(&format!("[note] {n}\n"));
        }
        out.push_str(&v.text);
        if !v.text.ends_with('\n') {
            out.push('\n');
        }
        out
    }
}

fn yn(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}
