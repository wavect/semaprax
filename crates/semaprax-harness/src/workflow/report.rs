//! The run report: deterministic JSON plus a human rendering of the same data.
//! It never carries goal text, prompts, provider payloads or secrets.

use super::compiler::CompilerDiagnostic;
use crate::diag::HarnessDiagnostic;
use serde_json::{json, Value};

pub const RUN_SCHEMA: &str = "semaprax.harness-run.v1";
/// Additive report for `semaprax.harness-task.v2` runs (new statuses, task and
/// operation blocks). v1 reports keep their members and meanings.
pub const RUN_SCHEMA_V2: &str = "semaprax.harness-run.v2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderUse {
    pub capability: String,
    pub provider: String,
    /// selected, fallback, disabled, unavailable (from the resolved profile).
    pub state: String,
    pub invoked: u32,
}

#[derive(Clone, Debug)]
pub struct Report {
    /// published, approved-candidate-ready, rejected, refused, uncertain,
    /// diagnosed, no-repair-needed; v2 adds unchanged-repair-baseline, planned,
    /// candidate-ready, unsupported-goal, exhausted, no-progress, cancelled.
    pub status: &'static str,
    pub lineage: String,
    pub revision: String,
    pub compiler_revision: Option<String>,
    pub diagnostics: Vec<CompilerDiagnostic>,
    pub refusals: Vec<HarnessDiagnostic>,
    pub notes: Vec<String>,
    pub steps: Vec<(String, String)>,
    pub providers: Vec<ProviderUse>,
    pub composition: Value,
    pub context: Value,
    pub route: Value,
    pub candidate: Value,
    pub checks: Value,
    pub approval: Value,
    pub publication: Value,
    pub ignored_claims: Vec<String>,
    pub compiler_commands: Vec<String>,
    pub external_calls: u32,
    /// 2 when the run used a v2 task (selects the v2 report schema).
    pub schema_version: u8,
    pub task: Value,
    /// Compiler candidate operations advertised for this run.
    pub operations: Value,
    pub session: Value,
}

impl Report {
    pub fn new(lineage: &str, revision: &str) -> Self {
        Report {
            status: "refused",
            lineage: lineage.into(),
            revision: revision.into(),
            compiler_revision: None,
            diagnostics: vec![],
            refusals: vec![],
            notes: vec![],
            steps: vec![],
            providers: vec![],
            composition: Value::Null,
            context: Value::Null,
            route: Value::Null,
            candidate: Value::Null,
            checks: Value::Null,
            approval: Value::Null,
            publication: Value::Null,
            ignored_claims: vec![],
            compiler_commands: vec![],
            external_calls: 0,
            schema_version: 1,
            task: Value::Null,
            operations: Value::Null,
            session: Value::Null,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self.status {
            "published"
            | "approved-candidate-ready"
            | "no-repair-needed"
            | "candidate-ready"
            | "planned"
            | "unchanged-repair-baseline" => 0,
            _ => 1,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut v = self.to_json_v1();
        if self.schema_version == 2 {
            v["schema"] = json!(RUN_SCHEMA_V2);
            v["task"] = self.task.clone();
            v["operations"] = self.operations.clone();
            v["session"] = self.session.clone();
        }
        v
    }

    fn to_json_v1(&self) -> Value {
        json!({
            "schema": RUN_SCHEMA,
            "status": self.status,
            "lineage": self.lineage,
            "revision": self.revision,
            "compiler_revision": self.compiler_revision,
            "diagnostics": self.diagnostics.iter().map(CompilerDiagnostic::to_json).collect::<Vec<_>>(),
            "refusals": self.refusals.iter().map(HarnessDiagnostic::json).collect::<Vec<_>>(),
            "notes": self.notes,
            "steps": self.steps.iter().map(|(s, o)| json!({"step": s, "outcome": o})).collect::<Vec<_>>(),
            "providers": self.providers.iter().map(|p| json!({"capability": p.capability, "provider": p.provider, "state": p.state, "invoked": p.invoked})).collect::<Vec<_>>(),
            "composition": self.composition,
            "context": self.context,
            "route": self.route,
            "candidate": self.candidate,
            "checks": self.checks,
            "approval": self.approval,
            "publication": self.publication,
            "ignored_provider_claims": self.ignored_claims,
            "compiler_commands": self.compiler_commands,
            "external_provider_calls": self.external_calls,
        })
    }

    pub fn to_text(&self) -> String {
        let mut s = format!(
            "run {} revision {}\nstatus: {}\n",
            self.lineage, self.revision, self.status
        );
        for d in &self.diagnostics {
            s.push_str(&format!("compiler diagnostic {}: {}\n", d.code, d.message));
        }
        for (step, outcome) in &self.steps {
            s.push_str(&format!("step {step}: {outcome}\n"));
        }
        for p in &self.providers {
            s.push_str(&format!(
                "provider {} {} ({}), invoked {}\n",
                p.capability, p.provider, p.state, p.invoked
            ));
        }
        for c in &self.ignored_claims {
            s.push_str(&format!("ignored provider claim: {c}\n"));
        }
        for n in &self.notes {
            s.push_str(&format!("note: {n}\n"));
        }
        for r in &self.refusals {
            s.push_str(&format!("{r}\n"));
        }
        if !self.approval.is_null() {
            s.push_str(&format!("approval: {}\n", self.approval));
        }
        if !self.publication.is_null() {
            s.push_str(&format!("publication: {}\n", self.publication));
        }
        s
    }
}
