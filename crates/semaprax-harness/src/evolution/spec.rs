//! Experiment specification (`semaprax.evolution-experiment.v1`) parsing.

use super::w;
use crate::diag::HarnessResult;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SPEC_SCHEMA: &str = "semaprax.evolution-experiment.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    /// Sanitized raw trace copies are deleted when the experiment ends.
    ExperimentOnly,
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Split {
    Train,
    Validation,
    Test,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: String,
    pub split: Split,
    pub prompt: String,
    /// Sealed: only ever sent to the adapter for the `train` split.
    pub expected: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Caps {
    pub iterations: u64,
    pub model_calls: u64,
    pub seconds: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct GateCfg {
    /// Largest allowed drop in passed test tasks (0 = none).
    pub max_test_regression: u64,
    pub max_skill_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub id: String,
    pub family: String,
    pub adapter_command: Vec<String>,
    pub adapter_env: BTreeMap<String, String>,
    pub workspace_root: PathBuf,
    pub traces: Vec<PathBuf>,
    pub retention: Retention,
    pub parent_name: String,
    pub parent_dir: PathBuf,
    pub protected: Vec<PathBuf>,
    pub tasks: Vec<Task>,
    pub caps: Caps,
    pub gate: GateCfg,
    pub min_traces: usize,
}

fn bad(msg: impl Into<String>) -> crate::diag::HarnessDiagnostic {
    w("SPX-HPW001", msg)
}

fn s<'a>(v: &'a Value, key: &str) -> HarnessResult<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("`{key}` must be a string")))
}

fn n(v: &Value, key: &str, default: u64) -> HarnessResult<u64> {
    match v.get(key) {
        None => Ok(default),
        Some(x) => x
            .as_u64()
            .ok_or_else(|| bad(format!("`{key}` must be a non-negative integer"))),
    }
}

fn abs(base: &Path, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

pub fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Parse a spec; relative paths resolve against `base`.
pub fn parse(v: &Value, base: &Path) -> HarnessResult<Spec> {
    if v.get("schema").and_then(Value::as_str) != Some(SPEC_SCHEMA) {
        return Err(bad(format!("`schema` must be `{SPEC_SCHEMA}`")));
    }
    if v.get("auto_promote")
        .is_some_and(|x| x != &Value::Bool(false))
    {
        return Err(w(
            "SPX-HPW009",
            "automatic promotion is disabled: promotion is a separate explicit action",
        ));
    }
    let id = s(v, "id")?.to_string();
    if !id_ok(&id) {
        return Err(bad("`id` must be 1-64 chars of [a-z0-9-]"));
    }
    let adapter = v.get("adapter").ok_or_else(|| bad("missing `adapter`"))?;
    let command: Vec<String> = adapter
        .get("command")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if command.is_empty() || !Path::new(&command[0]).is_absolute() {
        return Err(bad("`adapter.command` needs an absolute executable path"));
    }
    let mut adapter_env = BTreeMap::new();
    if let Some(o) = adapter.get("env").and_then(Value::as_object) {
        for (k, val) in o {
            adapter_env.insert(
                k.clone(),
                val.as_str()
                    .ok_or_else(|| bad("`adapter.env` values must be strings"))?
                    .to_string(),
            );
        }
    }
    let consent = v.get("consent").ok_or_else(|| {
        w(
            "SPX-HPW003",
            "no consent: list the consented trace files under `consent.traces`",
        )
    })?;
    let traces: Vec<PathBuf> = consent
        .get("traces")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|p| abs(base, p)))
                .collect()
        })
        .unwrap_or_default();
    if traces.is_empty() {
        return Err(w("SPX-HPW003", "`consent.traces` is empty"));
    }
    let retention = match s(consent, "retention")? {
        "experiment-only" => Retention::ExperimentOnly,
        "keep" => Retention::Keep,
        o => return Err(bad(format!("unknown retention `{o}`"))),
    };
    let parent = v.get("parent").ok_or_else(|| bad("missing `parent`"))?;
    let mut tasks = Vec::new();
    for t in v
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("missing `tasks`"))?
    {
        let split = match s(t, "split")? {
            "train" => Split::Train,
            "validation" => Split::Validation,
            "test" => Split::Test,
            o => return Err(bad(format!("unknown split `{o}`"))),
        };
        tasks.push(Task {
            id: s(t, "id")?.to_string(),
            split,
            prompt: s(t, "prompt")?.to_string(),
            expected: s(t, "expected")?.to_string(),
        });
    }
    tasks.sort_by(|a, b| a.id.cmp(&b.id));
    if tasks.windows(2).any(|p| p[0].id == p[1].id) {
        return Err(bad("duplicate task id"));
    }
    for sp in [Split::Validation, Split::Test] {
        if !tasks.iter().any(|t| t.split == sp) {
            return Err(bad(
                "held-out `validation` and `test` splits are both required",
            ));
        }
    }
    let caps = v.get("caps").cloned().unwrap_or(Value::Null);
    let gate = v.get("gate").cloned().unwrap_or(Value::Null);
    let protected = v
        .get("protected")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|p| abs(base, p)))
                .collect()
        })
        .unwrap_or_default();
    Ok(Spec {
        id,
        family: s(v, "family")?.to_string(),
        adapter_command: command,
        adapter_env,
        workspace_root: abs(base, s(v, "workspace_root")?),
        traces,
        retention,
        parent_name: s(parent, "name")?.to_string(),
        parent_dir: abs(base, s(parent, "dir")?),
        protected,
        tasks,
        caps: Caps {
            iterations: n(&caps, "max_iterations", 1)?,
            model_calls: n(&caps, "max_model_calls", 32)?,
            seconds: n(&caps, "max_seconds", 120)?,
        },
        gate: GateCfg {
            max_test_regression: n(&gate, "max_test_regression", 0)?,
            max_skill_bytes: n(&gate, "max_skill_bytes", 4096)? as usize,
        },
        min_traces: n(v, "min_traces", 2)? as usize,
    })
}
