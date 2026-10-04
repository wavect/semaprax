//! Conformance report model: cases, suites and the deterministic
//! `semaprax.harness-conformance-report.v1` document.

use crate::json::canonical;
use serde_json::{json, Value};

pub const REPORT_SCHEMA: &str = "semaprax.harness-conformance-report.v1";
/// Every report carries this literal: a run is evidence, never a support claim.
pub const SUPPORT_DECISION: &str = "not-a-support-decision";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Unverified,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unverified => "unverified",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub name: String,
    pub verdict: Verdict,
    pub evidence: Value,
}

/// A failed check: message plus structured evidence.
#[derive(Clone, Debug)]
pub struct Fail(pub String, pub Value);

pub fn fail(msg: impl Into<String>) -> Fail {
    Fail(msg.into(), Value::Null)
}

pub fn fail_with(msg: impl Into<String>, evidence: Value) -> Fail {
    Fail(msg.into(), evidence)
}

impl Case {
    pub fn pass(name: &str, evidence: Value) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Pass,
            evidence,
        }
    }
    pub fn unverified(name: &str, why: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Unverified,
            evidence: json!({"reason": why.into()}),
        }
    }
    pub fn failed(name: &str, f: Fail) -> Self {
        Self {
            name: name.into(),
            verdict: Verdict::Fail,
            evidence: json!({"reason": f.0, "detail": f.1}),
        }
    }
}

/// Run one check; `Err` becomes a failed case.
pub fn check(name: &str, f: impl FnOnce() -> Result<Value, Fail>) -> Case {
    match f() {
        Ok(ev) => Case::pass(name, ev),
        Err(e) => Case::failed(name, e),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suite {
    pub name: String,
    /// `adapter` (the adapter under test behaved) or `host` (the host refused
    /// a hostile fixture correctly).
    pub subject: &'static str,
    pub delegated: Option<&'static str>,
    pub cases: Vec<Case>,
}

impl Suite {
    pub fn new(name: &str, subject: &'static str, cases: Vec<Case>) -> Self {
        Self {
            name: name.into(),
            subject,
            delegated: None,
            cases,
        }
    }

    pub fn verdict(&self) -> Verdict {
        if self.cases.iter().any(|c| c.verdict == Verdict::Fail) {
            Verdict::Fail
        } else if self.cases.iter().any(|c| c.verdict == Verdict::Pass) {
            Verdict::Pass
        } else {
            Verdict::Unverified
        }
    }

    fn to_json(&self) -> Value {
        let cases: Vec<Value> = self
            .cases
            .iter()
            .map(|c| json!({"name": c.name, "verdict": c.verdict.as_str(), "evidence": c.evidence}))
            .collect();
        let mut v = json!({"name": self.name, "subject": self.subject, "verdict": self.verdict().as_str(), "cases": cases});
        if let Some(d) = self.delegated {
            v["delegated"] = json!(d);
        }
        v
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub subject: Value,
    pub inactive: Vec<Value>,
    pub suites: Vec<Suite>,
}

impl Report {
    fn count(&self, v: Verdict) -> usize {
        self.suites
            .iter()
            .flat_map(|s| &s.cases)
            .filter(|c| c.verdict == v)
            .count()
    }

    /// `fail` when any case failed, else `pass` when something passed.
    pub fn verdict(&self) -> Verdict {
        if self.count(Verdict::Fail) > 0 {
            Verdict::Fail
        } else if self.count(Verdict::Pass) > 0 {
            Verdict::Pass
        } else {
            Verdict::Unverified
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "schema": REPORT_SCHEMA,
            "support_decision": SUPPORT_DECISION,
            "subject": self.subject,
            "inactive_capabilities": self.inactive,
            "suites": self.suites.iter().map(Suite::to_json).collect::<Vec<_>>(),
            "summary": {"pass": self.count(Verdict::Pass), "fail": self.count(Verdict::Fail),
                        "unverified": self.count(Verdict::Unverified)},
            "verdict": self.verdict().as_str(),
        })
    }

    pub fn render_json(&self) -> String {
        format!("{}\n", canonical(&self.to_json()))
    }

    pub fn render_human(&self) -> String {
        let mut out = format!(
            "conformance report for {} ({}) - {}\n",
            self.subject["provider_id"].as_str().unwrap_or("?"),
            SUPPORT_DECISION,
            self.verdict().as_str()
        );
        for s in &self.suites {
            out.push_str(&format!(
                "suite {} [{}]: {}\n",
                s.name,
                s.subject,
                s.verdict().as_str()
            ));
            for c in &s.cases {
                out.push_str(&format!("  {:<10} {}\n", c.verdict.as_str(), c.name));
                if c.verdict != Verdict::Pass {
                    if let Some(r) = c.evidence["reason"].as_str() {
                        out.push_str(&format!("             {r}\n"));
                    }
                }
            }
        }
        out
    }
}
