//! Pinned workload corpus `semaprax.harness-benchmark-corpus.v1`.
//!
//! The corpus fixes requested behavior, authority, acceptance checks, required
//! facts and budget once, so every profile runs the identical task. Every
//! project directory is pinned by a tree digest; a drifted fixture is refused
//! before any cell runs (`SPX-HPQ004`).

use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{parse_strict, sha256_plain, JsonLimits};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CORPUS_SCHEMA: &str = "semaprax.harness-benchmark-corpus.v1";
pub const BENCH_SCHEMA: &str = "semaprax.harness-benchmark.v1";

pub const FAMILIES: [&str; 6] = [
    "orientation",
    "api_reuse",
    "mechanical_refactor",
    "failing_test_diagnosis",
    "multi_file_repair",
    "law_effect_change",
];

pub(crate) fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn schema_err(msg: impl Into<String>) -> HarnessDiagnostic {
    bad("SPX-HPQ002", msg)
}

/// Closed object: exactly `required` plus any of `optional`.
pub(crate) fn closed<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
) -> HarnessResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| schema_err(format!("{what} must be an object")))?;
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(bad("SPX-HPQ003", format!("{what}: unknown member `{k}`")));
        }
    }
    for k in required {
        if !m.contains_key(*k) {
            return Err(schema_err(format!("{what}: missing member `{k}`")));
        }
    }
    Ok(m)
}

pub(crate) fn text(m: &Map<String, Value>, k: &str, what: &str) -> HarnessResult<String> {
    match m.get(k).and_then(Value::as_str) {
        Some(s) if !s.is_empty() && s.len() <= 4096 => Ok(s.to_string()),
        _ => Err(schema_err(format!(
            "{what}: `{k}` must be a non-empty string"
        ))),
    }
}

pub(crate) fn strings(m: &Map<String, Value>, k: &str, what: &str) -> HarnessResult<Vec<String>> {
    let Some(v) = m.get(k) else { return Ok(vec![]) };
    v.as_array()
        .ok_or_else(|| schema_err(format!("{what}: `{k}` must be an array")))?
        .iter()
        .map(|x| {
            x.as_str()
                .map(str::to_string)
                .ok_or_else(|| schema_err(format!("{what}: `{k}` must hold strings")))
        })
        .collect()
}

pub(crate) fn uint(m: &Map<String, Value>, k: &str, what: &str) -> HarnessResult<u64> {
    m.get(k)
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_err(format!("{what}: `{k}` must be an unsigned integer")))
}

fn rel_path(s: &str, what: &str) -> HarnessResult<()> {
    let p = Path::new(s);
    if p.is_absolute()
        || p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        || s.contains('\\')
    {
        return Err(schema_err(format!(
            "{what}: `{s}` must be a relative path without `..`"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub dir: PathBuf,
    pub digest: String,
}

/// One acceptance check. A task is accepted only if every check passes.
#[derive(Clone, Debug, PartialEq)]
pub enum Check {
    /// Every required fact appears in the model-visible context.
    Facts,
    /// `symbol` has the listed callers; the context must list them and must
    /// not claim definitive absence.
    References {
        symbol: String,
        callers: Vec<String>,
    },
    /// Run `argv` once through the host; each critical line must survive.
    Command {
        argv: Vec<String>,
        critical: Vec<String>,
    },
    /// Compiler-judged workflow run with a pinned proposal.
    Workflow {
        proposal: String,
        task: Option<String>,
        expect_status: String,
    },
    /// Route decision must be in `allowed` and never in `forbidden`.
    Route {
        request: Value,
        allowed: Vec<String>,
        forbidden: Vec<String>,
    },
}

impl Check {
    pub fn kind(&self) -> &'static str {
        match self {
            Check::Facts => "facts",
            Check::References { .. } => "references",
            Check::Command { .. } => "command",
            Check::Workflow { .. } => "workflow",
            Check::Route { .. } => "route",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: String,
    pub family: String,
    pub project: String,
    pub request: String,
    /// Declared authority; the benchmark never grants more than `read`.
    pub write: String,
    pub publish: bool,
    pub query: Option<String>,
    pub max_bytes: usize,
    pub baseline_files: Vec<String>,
    pub required_facts: Vec<String>,
    pub checks: Vec<Check>,
    pub max_visible_bytes: u64,
    pub max_calls: u64,
    /// Local-model pilot question and the strings a correct answer contains.
    pub question: Option<(String, Vec<String>)>,
}

impl Task {
    pub fn has_check(&self, kind: &str) -> bool {
        self.checks.iter().any(|c| c.kind() == kind)
    }
    /// Steps the task exercises: context, command, route, workflow.
    pub fn steps(&self) -> Vec<&'static str> {
        let mut s = vec![];
        if self.query.is_some() {
            s.push("context");
        }
        for (k, n) in [
            ("command", "command"),
            ("route", "route"),
            ("workflow", "workflow"),
        ] {
            if self.has_check(k) {
                s.push(n);
            }
        }
        s
    }
}

#[derive(Clone, Debug)]
pub struct Install {
    pub descriptor: String,
    pub provider: String,
    pub capability: String,
    pub upstream_env: Option<String>,
    /// Remove the descriptor's `upstream` block from the staged copy (adapters
    /// that are their own upstream, e.g. the bundled source index).
    pub strip_upstream: bool,
}

#[derive(Clone, Debug)]
pub struct RouterSpec {
    pub descriptor: String,
    pub runtime_env: String,
    pub env: BTreeMap<String, String>,
    pub provider_id: String,
    pub model_id: String,
    pub checkpoint: String,
}

/// A profile is configuration only: adopted descriptors plus capability
/// selections. Benchmark code never branches on a product name.
#[derive(Clone, Debug)]
pub struct ProfileSpec {
    pub id: String,
    pub baseline: bool,
    pub description: String,
    pub installs: Vec<Install>,
    pub router: Option<RouterSpec>,
    pub requires_env: Vec<String>,
    pub command_view_policy: Option<Value>,
    /// Declared reason this profile is skipped on this machine (recorded, not hidden).
    pub skip: Option<String>,
}

impl ProfileSpec {
    /// Capability kinds this profile changes relative to builtins.
    pub fn touches(&self) -> Vec<&'static str> {
        let mut t = vec![];
        for i in &self.installs {
            match i.capability.as_str() {
                "context.repository" => t.push("context"),
                "command.view" => t.push("command"),
                _ => {}
            }
        }
        if self.router.is_some() {
            t.push("route");
        }
        t.sort();
        t.dedup();
        t
    }
}

#[derive(Clone, Debug)]
pub struct AdversarialSpec {
    pub id: String,
    pub kind: String,
    pub project: String,
}

#[derive(Clone, Debug)]
pub struct Corpus {
    pub dir: PathBuf,
    pub id: String,
    pub seed: u64,
    pub cold: u32,
    pub warm: u32,
    pub source_commit: String,
    pub projects: BTreeMap<String, Project>,
    pub tasks: Vec<Task>,
    pub profiles: Vec<ProfileSpec>,
    pub adversarial: Vec<AdversarialSpec>,
    pub digest: String,
}

/// Deterministic digest of a directory tree (sorted relative paths + content).
pub fn tree_digest(dir: &Path) -> HarnessResult<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> std::io::Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            if e.file_type()?.is_dir() {
                walk(base, &p, out)?;
            } else {
                let rel = p
                    .strip_prefix(base)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, sha256_plain(&std::fs::read(&p)?)));
            }
        }
        Ok(())
    }
    let mut rows = vec![];
    walk(dir, dir, &mut rows)
        .map_err(|e| bad("SPX-HPQ001", format!("read {}: {e}", dir.display())))?;
    let mut text = String::new();
    for (p, d) in rows {
        text.push_str(&format!("{p}\0{d}\n"));
    }
    Ok(sha256_plain(text.as_bytes()))
}

const LIMITS: JsonLimits = JsonLimits {
    max_bytes: 4 << 20,
    max_depth: 32,
    max_nodes: 200_000,
};

fn parse_check(v: &Value, task: &str) -> HarnessResult<Check> {
    let what = format!("task `{task}` check");
    let kind = v.get("kind").and_then(Value::as_str).unwrap_or("");
    Ok(match kind {
        "facts" => {
            closed(v, &what, &["kind"], &[])?;
            Check::Facts
        }
        "references" => {
            let m = closed(v, &what, &["kind", "symbol", "callers"], &[])?;
            Check::References {
                symbol: text(m, "symbol", &what)?,
                callers: strings(m, "callers", &what)?,
            }
        }
        "command" => {
            let m = closed(v, &what, &["kind", "argv", "critical"], &[])?;
            Check::Command {
                argv: strings(m, "argv", &what)?,
                critical: strings(m, "critical", &what)?,
            }
        }
        "workflow" => {
            let m = closed(v, &what, &["kind", "proposal", "expect_status"], &["task"])?;
            Check::Workflow {
                proposal: text(m, "proposal", &what)?,
                task: m.get("task").and_then(Value::as_str).map(str::to_string),
                expect_status: text(m, "expect_status", &what)?,
            }
        }
        "route" => {
            let m = closed(v, &what, &["kind", "request", "allowed", "forbidden"], &[])?;
            Check::Route {
                request: m["request"].clone(),
                allowed: strings(m, "allowed", &what)?,
                forbidden: strings(m, "forbidden", &what)?,
            }
        }
        other => return Err(schema_err(format!("{what}: unknown kind `{other}`"))),
    })
}

fn parse_task(v: &Value) -> HarnessResult<Task> {
    let m = closed(
        v,
        "task",
        &[
            "id",
            "family",
            "project",
            "request",
            "authority",
            "acceptance",
            "required_facts",
            "budget",
        ],
        &["query", "max_bytes", "baseline_files", "question"],
    )?;
    let id = text(m, "id", "task")?;
    let what = format!("task `{id}`");
    let family = text(m, "family", &what)?;
    if !FAMILIES.contains(&family.as_str()) {
        return Err(schema_err(format!("{what}: unknown family `{family}`")));
    }
    let a = closed(&m["authority"], &what, &["read", "write", "publish"], &[])?;
    let write = text(a, "write", &what)?;
    if write != "none" && write != "candidate" {
        return Err(schema_err(format!(
            "{what}: authority.write must be `none` or `candidate`"
        )));
    }
    let publish = a["publish"]
        .as_bool()
        .ok_or_else(|| schema_err(format!("{what}: authority.publish must be a bool")))?;
    if publish {
        return Err(schema_err(format!(
            "{what}: the benchmark never grants publication authority"
        )));
    }
    let b = closed(
        &m["budget"],
        &what,
        &["max_visible_bytes", "max_calls"],
        &[],
    )?;
    let checks = m["acceptance"]
        .as_array()
        .ok_or_else(|| schema_err(format!("{what}: acceptance must be an array")))?
        .iter()
        .map(|c| parse_check(c, &id))
        .collect::<HarnessResult<Vec<_>>>()?;
    if checks.is_empty() {
        return Err(schema_err(format!(
            "{what}: needs at least one acceptance check"
        )));
    }
    let baseline_files = strings(m, "baseline_files", &what)?;
    for f in &baseline_files {
        rel_path(f, &what)?;
    }
    let question = match m.get("question") {
        None => None,
        Some(q) => {
            let q = closed(q, &what, &["ask", "expect"], &[])?;
            Some((text(q, "ask", &what)?, strings(q, "expect", &what)?))
        }
    };
    let t = Task {
        id: id.clone(),
        family,
        project: text(m, "project", &what)?,
        request: text(m, "request", &what)?,
        write,
        publish,
        query: m.get("query").and_then(Value::as_str).map(str::to_string),
        max_bytes: m.get("max_bytes").and_then(Value::as_u64).unwrap_or(8192) as usize,
        baseline_files,
        required_facts: strings(m, "required_facts", &what)?,
        checks,
        max_visible_bytes: uint(b, "max_visible_bytes", &what)?,
        max_calls: uint(b, "max_calls", &what)?,
        question,
    };
    if t.has_check("facts") && (t.query.is_none() || t.required_facts.is_empty()) {
        return Err(schema_err(format!(
            "{what}: a `facts` check needs a query and required_facts"
        )));
    }
    if t.has_check("references") && t.query.is_none() {
        return Err(schema_err(format!(
            "{what}: a `references` check needs a query"
        )));
    }
    Ok(t)
}

fn parse_profile(v: &Value) -> HarnessResult<ProfileSpec> {
    let m = closed(
        v,
        "profile",
        &["id", "description"],
        &[
            "baseline",
            "installs",
            "router",
            "requires_env",
            "command_view_policy",
            "skip",
        ],
    )?;
    let id = text(m, "id", "profile")?;
    let what = format!("profile `{id}`");
    let mut installs = vec![];
    for i in m
        .get("installs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let im = closed(
            i,
            &what,
            &["descriptor", "provider", "capability"],
            &["upstream_env", "strip_upstream"],
        )?;
        let cap = text(im, "capability", &what)?;
        if !["context.repository", "command.view"].contains(&cap.as_str()) {
            return Err(schema_err(format!(
                "{what}: install capability `{cap}` is not benchmarked"
            )));
        }
        installs.push(Install {
            descriptor: text(im, "descriptor", &what)?,
            provider: text(im, "provider", &what)?,
            capability: cap,
            upstream_env: im
                .get("upstream_env")
                .and_then(Value::as_str)
                .map(str::to_string),
            strip_upstream: im
                .get("strip_upstream")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    let router = match m.get("router") {
        None => None,
        Some(r) => {
            let rm = closed(
                r,
                &what,
                &[
                    "descriptor",
                    "runtime_env",
                    "provider_id",
                    "model_id",
                    "checkpoint",
                ],
                &["env"],
            )?;
            let mut env = BTreeMap::new();
            for (k, x) in rm
                .get("env")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                env.insert(k.clone(), x.as_str().unwrap_or("").to_string());
            }
            Some(RouterSpec {
                descriptor: text(rm, "descriptor", &what)?,
                runtime_env: text(rm, "runtime_env", &what)?,
                env,
                provider_id: text(rm, "provider_id", &what)?,
                model_id: text(rm, "model_id", &what)?,
                checkpoint: text(rm, "checkpoint", &what)?,
            })
        }
    };
    Ok(ProfileSpec {
        id,
        baseline: m.get("baseline").and_then(Value::as_bool).unwrap_or(false),
        description: text(m, "description", &what)?,
        installs,
        router,
        requires_env: strings(m, "requires_env", &what)?,
        command_view_policy: m.get("command_view_policy").cloned(),
        skip: m.get("skip").and_then(Value::as_str).map(str::to_string),
    })
}

impl Corpus {
    /// Load `<dir>/corpus.json`, validate the schema and verify every pin.
    pub fn load(dir: &Path) -> HarnessResult<Corpus> {
        let path = dir.join("corpus.json");
        let bytes = std::fs::read(&path)
            .map_err(|e| bad("SPX-HPQ001", format!("read {}: {e}", path.display())))?;
        let v = parse_strict(&bytes, &LIMITS)?;
        let m = closed(
            &v,
            "corpus",
            &[
                "schema",
                "id",
                "seed",
                "trials",
                "pin",
                "projects",
                "tasks",
                "profiles",
                "adversarial",
            ],
            &[],
        )?;
        if m["schema"] != CORPUS_SCHEMA {
            return Err(schema_err(format!(
                "corpus schema must be `{CORPUS_SCHEMA}`"
            )));
        }
        let tm = closed(&m["trials"], "trials", &["cold", "warm"], &[])?;
        let pin = closed(&m["pin"], "pin", &["source_commit", "projects"], &[])?;
        let pins = pin["projects"]
            .as_object()
            .ok_or_else(|| schema_err("pin.projects must be an object"))?;
        let mut projects = BTreeMap::new();
        for (id, p) in m["projects"]
            .as_object()
            .ok_or_else(|| schema_err("projects must be an object"))?
        {
            let pm = closed(p, "project", &["path"], &[])?;
            let rel = text(pm, "path", "project")?;
            rel_path(&rel, "project path")?;
            let pdir = dir.join(&rel);
            let actual = tree_digest(&pdir)?;
            let pinned = pins.get(id).and_then(Value::as_str).unwrap_or("");
            if actual != pinned {
                return Err(bad(
                    "SPX-HPQ004",
                    format!("project `{id}` drifted from its pin (pinned `{pinned}`, actual `{actual}`)"),
                ));
            }
            projects.insert(
                id.clone(),
                Project {
                    id: id.clone(),
                    dir: pdir,
                    digest: actual,
                },
            );
        }
        let mut tasks: Vec<Task> = vec![];
        for t in m["tasks"]
            .as_array()
            .ok_or_else(|| schema_err("tasks must be an array"))?
        {
            let t = parse_task(t)?;
            if tasks.iter().any(|x| x.id == t.id) {
                return Err(bad("SPX-HPQ005", format!("duplicate task id `{}`", t.id)));
            }
            if !projects.contains_key(&t.project) {
                return Err(bad(
                    "SPX-HPQ006",
                    format!("task `{}` names unknown project `{}`", t.id, t.project),
                ));
            }
            tasks.push(t);
        }
        let mut profiles: Vec<ProfileSpec> = vec![];
        for p in m["profiles"]
            .as_array()
            .ok_or_else(|| schema_err("profiles must be an array"))?
        {
            let p = parse_profile(p)?;
            if profiles.iter().any(|x| x.id == p.id) {
                return Err(bad(
                    "SPX-HPQ005",
                    format!("duplicate profile id `{}`", p.id),
                ));
            }
            profiles.push(p);
        }
        if profiles.iter().filter(|p| p.baseline).count() != 1 {
            return Err(schema_err("exactly one profile must be the baseline"));
        }
        let mut adversarial = vec![];
        for a in m["adversarial"]
            .as_array()
            .ok_or_else(|| schema_err("adversarial must be an array"))?
        {
            let am = closed(a, "adversarial", &["id", "kind", "project"], &[])?;
            let project = text(am, "project", "adversarial")?;
            if !projects.contains_key(&project) {
                return Err(bad(
                    "SPX-HPQ006",
                    format!("adversarial case names unknown project `{project}`"),
                ));
            }
            adversarial.push(AdversarialSpec {
                id: text(am, "id", "adversarial")?,
                kind: text(am, "kind", "adversarial")?,
                project,
            });
        }
        let families: std::collections::BTreeSet<&str> =
            tasks.iter().map(|t| t.family.as_str()).collect();
        for f in FAMILIES {
            if !families.contains(f) {
                return Err(schema_err(format!("corpus has no task of family `{f}`")));
            }
        }
        Ok(Corpus {
            dir: dir.to_path_buf(),
            id: text(m, "id", "corpus")?,
            seed: uint(m, "seed", "corpus")?,
            cold: uint(tm, "cold", "trials")? as u32,
            warm: uint(tm, "warm", "trials")? as u32,
            source_commit: text(pin, "source_commit", "pin")?,
            projects,
            tasks,
            profiles,
            adversarial,
            digest: sha256_plain(&bytes),
        })
    }

    pub fn profile(&self, id: &str) -> Option<&ProfileSpec> {
        self.profiles.iter().find(|p| p.id == id)
    }
}
