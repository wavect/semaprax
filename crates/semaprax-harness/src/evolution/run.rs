//! Experiment driver: ingest -> evolve (adapter) -> evaluate (host graders) ->
//! gate -> result. Promotion is the separate [`promote`] action.

use super::adapter::{Adapter, AdapterError, Cancel};
use super::gate::{self, Score, Scores};
use super::spec::{Retention, Spec, Split};
use super::{derived, protected, trace, w, RESULT_SCHEMA};
use crate::contract::payload::evolve::validate_payload;
use crate::contract::payload::Direction;
use crate::diag::HarnessResult;
use crate::json::{canonical, sha256_plain};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NoAction,
    Rejected,
    Accepted,
    Unavailable,
    Aborted,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoAction => "no-action",
            Self::Rejected => "rejected",
            Self::Accepted => "accepted",
            Self::Unavailable => "unavailable",
            Self::Aborted => "aborted",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub outcome: Outcome,
    pub result: Value,
    pub workspace: PathBuf,
}

struct St<'a> {
    spec: &'a Spec,
    ws: PathBuf,
    res: Map<String, Value>,
    before: BTreeMap<String, String>,
    calls: u64,
    iterations: u64,
    derived: Option<String>,
}

impl St<'_> {
    fn finish(
        mut self,
        mut outcome: Outcome,
        reason: &str,
        code: Option<&str>,
    ) -> HarnessResult<Report> {
        let mut reason = reason.to_string();
        let mut code = code.map(String::from);
        let after = hash_protected(self.spec).unwrap_or_default();
        if after != self.before {
            if let Some(n) = &self.derived {
                derived::rollback(&self.ws, n);
            }
            self.res.remove("derived");
            outcome = Outcome::Aborted;
            reason = "protected content changed during the run; derived skill rolled back".into();
            code = Some("SPX-HPW008".into());
        }
        self.res.insert("schema".into(), json!(RESULT_SCHEMA));
        self.res.insert("experiment".into(), json!(self.spec.id));
        self.res.insert("family".into(), json!(self.spec.family));
        self.res.insert("outcome".into(), json!(outcome.as_str()));
        self.res.insert("reason".into(), json!(reason));
        self.res.insert("code".into(), json!(code));
        self.res.insert(
            "usage".into(),
            json!({"iterations": self.iterations, "model_calls": self.calls,
                "caps": {"max_iterations": self.spec.caps.iterations,
                    "max_model_calls": self.spec.caps.model_calls,
                    "max_seconds": self.spec.caps.seconds}}),
        );
        self.res.insert(
            "protected".into(),
            json!({"before": self.before, "after": after, "unchanged": after == self.before}),
        );
        self.res.insert(
            "promotion".into(),
            json!({"status": "not-promoted", "auto_promotion": "disabled"}),
        );
        if self.spec.retention == Retention::ExperimentOnly {
            let _ = std::fs::remove_dir_all(self.ws.join("raw"));
        }
        let result = Value::Object(self.res);
        std::fs::write(
            self.ws.join("evidence/result.json"),
            canonical(&result) + "\n",
        )
        .map_err(|e| w("SPX-HPW001", format!("result: {e}")))?;
        Ok(Report {
            outcome,
            result,
            workspace: self.ws,
        })
    }
}

fn hash_protected(spec: &Spec) -> HarnessResult<BTreeMap<String, String>> {
    let mut m = BTreeMap::new();
    for p in spec.protected.iter().chain([&spec.parent_dir]) {
        m.insert(p.display().to_string(), protected::digest_path(p)?);
    }
    let graders = Value::Array(
        spec.tasks
            .iter()
            .map(|t| json!({"id": t.id, "split": format!("{:?}", t.split), "prompt": t.prompt, "expected": t.expected}))
            .collect(),
    );
    m.insert(
        "graders".into(),
        sha256_plain(canonical(&graders).as_bytes()),
    );
    Ok(m)
}

fn caps_json(spec: &Spec) -> Value {
    json!({"max_iterations": spec.caps.iterations, "max_model_calls": spec.caps.model_calls, "max_seconds": spec.caps.seconds})
}

fn map_err(e: AdapterError) -> (Outcome, String, &'static str) {
    match e {
        AdapterError::Unavailable(r) => (
            Outcome::Unavailable,
            format!("evolution backend unavailable: {r}"),
            "SPX-HPW004",
        ),
        AdapterError::Protocol(r) => (
            Outcome::Aborted,
            format!("adapter protocol violation: {r}"),
            "SPX-HPW005",
        ),
        AdapterError::Timeout => (Outcome::Aborted, "time cap exceeded".into(), "SPX-HPW006"),
        AdapterError::Cancelled => (Outcome::Aborted, "cancelled".into(), "SPX-HPW007"),
    }
}

/// Run one isolated experiment. `cwd` is refused as a workspace location.
pub fn run_experiment(
    spec: &Spec,
    adapter: &mut dyn Adapter,
    cancel: &Cancel,
    cwd: &Path,
) -> HarnessResult<Report> {
    let ws = spec.workspace_root.join(&spec.id);
    std::fs::create_dir_all(&ws).map_err(|e| w("SPX-HPW002", format!("workspace: {e}")))?;
    let ws = ws
        .canonicalize()
        .map_err(|e| w("SPX-HPW002", e.to_string()))?;
    let mut guarded: Vec<PathBuf> = spec.protected.clone();
    guarded.push(spec.parent_dir.clone());
    guarded.push(cwd.to_path_buf());
    for g in guarded {
        if let Ok(g) = g.canonicalize() {
            if protected::overlaps(&ws, &g) {
                return Err(w(
                    "SPX-HPW002",
                    format!(
                        "workspace {} overlaps protected path {}",
                        ws.display(),
                        g.display()
                    ),
                ));
            }
        }
    }
    let before = hash_protected(spec)?;
    for d in ["derived", "evidence"] {
        let _ = std::fs::remove_dir_all(ws.join(d));
    }
    for d in ["raw", "wiki", "evidence"] {
        std::fs::create_dir_all(ws.join(d)).map_err(|e| w("SPX-HPW002", e.to_string()))?;
    }
    let parent_digest = derived::digest_of(&spec.parent_dir)?;
    let mut st = St {
        spec,
        ws: ws.clone(),
        res: Map::new(),
        before,
        calls: 0,
        iterations: 0,
        derived: None,
    };
    st.res.insert("parent".into(), json!({"name": spec.parent_name, "digest": parent_digest, "dir": spec.parent_dir.display().to_string()}));
    let deadline = Instant::now() + Duration::from_secs(spec.caps.seconds);

    let ing = trace::ingest(spec);
    st.res.insert("traces".into(), ing.summary());
    st.res.insert("trace_files".into(), json!(ing.files));
    if !ing.unreadable.is_empty() {
        return Err(w(
            "SPX-HPW003",
            format!("consented trace unreadable: {}", ing.unreadable.join(", ")),
        ));
    }
    let trace_digest = ing.digest();
    st.res.insert("trace_digest".into(), json!(trace_digest));
    std::fs::write(ws.join("raw/traces.jsonl"), ing.jsonl())
        .map_err(|e| w("SPX-HPW002", e.to_string()))?;
    if ing.records.len() < spec.min_traces {
        let r = format!(
            "only {} recurrent in-family records (need {})",
            ing.records.len(),
            spec.min_traces
        );
        return st.finish(Outcome::NoAction, &r, None);
    }
    if cancel.is_set() {
        return st.finish(Outcome::Aborted, "cancelled", Some("SPX-HPW007"));
    }

    // ---- evolve ----
    let train: Vec<Value> = spec
        .tasks
        .iter()
        .filter(|t| t.split == Split::Train)
        .map(|t| json!({"id": t.id, "prompt": t.prompt, "expected": t.expected}))
        .collect();
    let req = json!({
        "experiment": spec.id, "family": spec.family, "trace_path": "raw/traces.jsonl",
        "trace_digest": trace_digest, "parent": {"name": spec.parent_name, "digest": parent_digest},
        "caps": caps_json(spec), "train_tasks": train,
    });
    if validate_payload("evolve", Direction::Request, &req).is_err() {
        return st.finish(
            Outcome::Aborted,
            "evolve request failed contract validation",
            Some("SPX-HPW005"),
        );
    }
    if spec.caps.iterations == 0 {
        return st.finish(
            Outcome::Aborted,
            "iteration cap is zero",
            Some("SPX-HPW006"),
        );
    }
    st.iterations = 1;
    let out = match adapter.call(&req, &ws, deadline, cancel) {
        Ok(v) => v,
        Err(e) => {
            let (o, r, c) = map_err(e);
            return st.finish(o, &r, Some(c));
        }
    };
    if let Err(d) = validate_payload("evolve", Direction::Result, &out) {
        return st.finish(
            Outcome::Aborted,
            &format!("adapter protocol violation: {}", d.message),
            Some("SPX-HPW005"),
        );
    }
    st.calls += out["model_calls"].as_u64().unwrap_or(0);
    let mut wiki = Vec::new();
    for e in out["wiki"].as_array().into_iter().flatten() {
        let p = e["path"].as_str().unwrap_or("");
        let ok = p.starts_with("wiki/")
            && std::fs::read(ws.join(p))
                .is_ok_and(|b| Some(sha256_plain(&b).as_str()) == e["digest"].as_str());
        if !ok {
            return st.finish(
                Outcome::Aborted,
                &format!("wiki entry `{p}` is outside wiki/ or does not match its digest"),
                Some("SPX-HPW005"),
            );
        }
        wiki.push(e.clone());
    }
    st.res.insert("wiki".into(), json!(wiki));
    if out["iterations"].as_u64().unwrap_or(0) > spec.caps.iterations
        || st.calls > spec.caps.model_calls
    {
        return st.finish(
            Outcome::Aborted,
            "iteration or model-call cap exceeded by the evolution backend",
            Some("SPX-HPW006"),
        );
    }
    let Some(cand) = out.get("candidate") else {
        let r = out["no_action_reason"]
            .as_str()
            .unwrap_or("no candidate")
            .to_string();
        return st.finish(Outcome::NoAction, &r, None);
    };

    // ---- candidate ----
    let body = cand["body"].as_str().unwrap_or("");
    let Some(name) = derived::derived_name(cand["name"].as_str().unwrap_or("")) else {
        return st.finish(
            Outcome::Aborted,
            "candidate name is empty",
            Some("SPX-HPW011"),
        );
    };
    if body.trim_start().starts_with("---") || body.trim().is_empty() {
        return st.finish(
            Outcome::Aborted,
            "candidate body is empty or carries front matter",
            Some("SPX-HPW011"),
        );
    }
    let md = derived::skill_md(
        &name,
        cand["description"].as_str().unwrap_or(""),
        &spec.parent_name,
        &parent_digest,
        &spec.family,
        body,
    );
    let dir = derived::write_candidate(&ws, &name, &md)?;
    st.derived = Some(name.clone());
    let body_digest = sha256_plain(body.as_bytes());
    st.res.insert(
        "candidate".into(),
        json!({"name": name, "body_digest": body_digest, "bytes": body.len()}),
    );

    // ---- evaluate with host-owned graders ----
    let skill = json!({"name": name, "body": md});
    let mut scores = Scores::default();
    for (split, with) in [
        (Split::Validation, false),
        (Split::Validation, true),
        (Split::Test, false),
        (Split::Test, true),
    ] {
        let mut sc = Score::default();
        for t in spec.tasks.iter().filter(|t| t.split == split) {
            if cancel.is_set() {
                derived::rollback(&ws, &name);
                st.derived = None;
                return st.finish(Outcome::Aborted, "cancelled", Some("SPX-HPW007"));
            }
            if st.calls >= spec.caps.model_calls {
                derived::rollback(&ws, &name);
                st.derived = None;
                return st.finish(
                    Outcome::Aborted,
                    "model-call cap exhausted before evaluation completed",
                    Some("SPX-HPW006"),
                );
            }
            let mut r = json!({"task": {"id": t.id, "prompt": t.prompt}});
            if with {
                r["skill"] = skill.clone();
            }
            let res = adapter
                .call(&r, &ws, deadline, cancel)
                .map_err(map_err)
                .and_then(|v| {
                    validate_payload("solve", Direction::Result, &v)
                        .map(|_| v)
                        .map_err(|d| {
                            (
                                Outcome::Aborted,
                                format!("adapter protocol violation: {}", d.message),
                                "SPX-HPW005",
                            )
                        })
                });
            let v = match res {
                Ok(v) => v,
                Err((o, rs, c)) => {
                    derived::rollback(&ws, &name);
                    st.derived = None;
                    return st.finish(o, &rs, Some(c));
                }
            };
            st.calls += v["model_calls"].as_u64().unwrap_or(0).max(1);
            sc.total += 1;
            sc.passed += u64::from(gate::grade(v["answer"].as_str().unwrap_or(""), &t.expected));
        }
        match (split, with) {
            (Split::Validation, false) => scores.validation_base = sc,
            (Split::Validation, true) => scores.validation_cand = sc,
            (Split::Test, false) => scores.test_base = sc,
            _ => scores.test_cand = sc,
        }
    }
    st.res.insert("scores".into(), gate::to_json(&scores));
    if st.calls > spec.caps.model_calls {
        derived::rollback(&ws, &name);
        st.derived = None;
        return st.finish(
            Outcome::Aborted,
            "model-call cap exceeded during evaluation",
            Some("SPX-HPW006"),
        );
    }

    // ---- gate ----
    match gate::decide(&scores, body.len(), &spec.gate) {
        Err(why) => {
            derived::rollback(&ws, &name);
            st.derived = None;
            let ev = format!(
                "# Rejected candidate `{name}`\n\nFamily: {}\nBody digest: {body_digest}\nReasons: {}\n\n## Rejected body\n\n{body}\n",
                spec.family,
                why.join("; ")
            );
            let rel = format!("wiki/negative-evidence/{}.md", &body_digest[7..19]);
            let _ = std::fs::create_dir_all(ws.join("wiki/negative-evidence"));
            let _ = std::fs::write(ws.join(&rel), ev);
            st.res.insert("negative_evidence".into(), json!(rel));
            let r = why.join("; ");
            st.finish(Outcome::Rejected, &r, None)
        }
        Ok(()) => {
            let prov = json!({
                "schema": derived::PROVENANCE_SCHEMA, "identity": "artifact-v2",
                "parent": {"name": spec.parent_name, "digest": parent_digest},
                "sources": {"trace_digest": trace_digest, "wiki": wiki},
                "scope": {"family": spec.family},
                "evaluation": {"scores": gate::to_json(&scores), "gate": {"max_test_regression": spec.gate.max_test_regression, "max_skill_bytes": spec.gate.max_skill_bytes}},
            });
            let digest = derived::finalize(&dir, &prov)?;
            st.res.insert("derived".into(), json!({"name": name, "digest": digest, "identity": "artifact-v2", "dir": format!("derived/{name}")}));
            st.finish(
                Outcome::Accepted,
                "candidate passed the held-out gate; promotion proposal only",
                None,
            )
        }
    }
}

/// Explicit promotion of an accepted experiment's derived skill into `target_root`.
pub fn promote(ws: &Path, target_root: &Path, approve: bool) -> HarnessResult<Value> {
    let raw = std::fs::read(ws.join("evidence/result.json"))
        .map_err(|e| w("SPX-HPW009", format!("no experiment result: {e}")))?;
    let r: Value = serde_json::from_slice(&raw).map_err(|e| w("SPX-HPW009", e.to_string()))?;
    if r["outcome"] != "accepted" {
        return Err(w(
            "SPX-HPW009",
            "only an accepted experiment can be promoted",
        ));
    }
    if !approve {
        return Err(w(
            "SPX-HPW009",
            "promotion needs explicit approval (--approve)",
        ));
    }
    let name = r["derived"]["name"].as_str().unwrap_or("");
    let want = r["derived"]["digest"].as_str().unwrap_or("");
    let src = ws.join("derived").join(name);
    if derived::digest_of(&src)? != want {
        return Err(w(
            "SPX-HPW008",
            "derived skill no longer matches its recorded identity",
        ));
    }
    let parent = Path::new(r["parent"]["dir"].as_str().unwrap_or(""));
    if derived::digest_of(parent)? != r["parent"]["digest"].as_str().unwrap_or("") {
        return Err(w("SPX-HPW008", "parent snapshot changed since evaluation"));
    }
    let dst = target_root.join(name);
    if dst.exists() {
        return Err(w("SPX-HPW009", format!("{} already exists", dst.display())));
    }
    derived::copy_bundle(&src, &dst)?;
    if derived::digest_of(&dst)? != want {
        let _ = std::fs::remove_dir_all(&dst);
        return Err(w(
            "SPX-HPW008",
            "promoted copy differs from the evaluated bundle",
        ));
    }
    let rec = json!({"schema": "semaprax.evolution-promotion.v1", "name": name, "digest": want, "target": dst.display().to_string(), "parent_digest": r["parent"]["digest"]});
    let _ = std::fs::write(ws.join("evidence/promotion.json"), canonical(&rec) + "\n");
    Ok(rec)
}
