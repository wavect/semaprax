//! Isolated source repair for an unverified baseline and the acceptance-oracle
//! guard (HN-02). A proposal may only send a bounded `source_patch`; the host
//! applies it to a scratch copy, never to the project, and the compiler alone
//! judges the result. Operations need a verified base.

use super::attempt::{self, PromptCtx};
use super::journal::Journal;
use super::pipeline::{step, Ctx, Stages};
use super::policy::{effect_tokens, law_lines};
use super::report::Report;
use super::session::{finish, loop_steps, Baseline, State};
use super::stages::*;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

const MAX_EDITS: usize = 16;
const MAX_EDIT_BYTES: usize = 64 * 1024;

fn module_of(src: &str) -> Option<String> {
    src.lines().map(str::trim).find_map(|l| {
        l.strip_prefix("module ")
            .map(|m| m.trim_end_matches(';').trim().to_string())
    })
}

/// The independent acceptance oracle: the manifest and every file declaring a
/// module the manifest lists under `tests`.
pub(super) fn oracle_files(baseline: &Baseline) -> BTreeSet<String> {
    let mut out = BTreeSet::from(["semaprax.toml".to_string()]);
    let manifest = baseline
        .get("semaprax.toml")
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let mut tests: Vec<String> = Vec::new();
    if let Some(i) = manifest.find("tests") {
        if let Some(rest) = manifest[i..].split_once('[') {
            if let Some((inner, _)) = rest.1.split_once(']') {
                tests = inner
                    .split(',')
                    .map(|t| t.trim().trim_matches('"').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
            }
        }
    }
    for (path, bytes) in baseline {
        if let Some(m) = module_of(&String::from_utf8_lossy(bytes)) {
            if tests.contains(&m) {
                out.insert(path.clone());
            }
        }
    }
    out
}

fn declaring_file(baseline: &Baseline, id: &str) -> Option<String> {
    let needle = format!("@id(\"{id}\")");
    baseline
        .iter()
        .find(|(_, b)| String::from_utf8_lossy(b).contains(&needle))
        .map(|(p, _)| p.clone())
}

/// A semantic proposal whose target lives in the oracle is refused.
pub(super) fn oracle_intent_violation(
    baseline: &Baseline,
    oracle: &BTreeSet<String>,
    p: &Proposal,
) -> Option<String> {
    for key in ["target", "destination"] {
        if let Some(id) = p.intent.get(key).and_then(Value::as_str) {
            if let Some(f) = declaring_file(baseline, id) {
                if oracle.contains(&f) {
                    return Some(format!(
                        "`{}` targets `{id}` in the acceptance oracle `{f}`; the oracle is not editable",
                        p.kind
                    ));
                }
            }
        }
    }
    None
}

fn count_map(lines: Vec<String>) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for l in lines {
        *m.entry(l).or_default() += 1;
    }
    m
}

/// Validate and apply a `source_patch` to the scratch tree. Everything is
/// computed in memory first; a refusal leaves the scratch tree unchanged.
pub(super) fn apply_patch(s: &State, patch: &Value) -> HarnessResult<Vec<String>> {
    let bad = |m: String| d("SPX-HPD117", m);
    let o = patch
        .as_object()
        .ok_or_else(|| bad("`source_patch` must be an object".into()))?;
    if o.len() != 1 {
        return Err(bad("`source_patch` has exactly one member, `edits`".into()));
    }
    let edits = o
        .get("edits")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty() && a.len() <= MAX_EDITS)
        .ok_or_else(|| bad(format!("`edits` must hold 1..={MAX_EDITS} edits")))?;
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    for e in edits {
        let m = e
            .as_object()
            .filter(|m| m.len() == 3)
            .ok_or_else(|| bad("an edit has exactly path, find, replace".into()))?;
        let get = |k: &str| {
            m.get(k)
                .and_then(Value::as_str)
                .filter(|v| v.len() <= MAX_EDIT_BYTES)
                .ok_or_else(|| {
                    bad(format!(
                        "edit `{k}` must be a string of at most {MAX_EDIT_BYTES} bytes"
                    ))
                })
        };
        let (path, find, replace) = (get("path")?, get("find")?, get("replace")?);
        let rel = std::path::Path::new(path);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(bad(format!("path `{path}` escapes the project")));
        }
        if s.oracle.contains(path) {
            return Err(d(
                "SPX-HPD114",
                format!("`{path}` is part of the acceptance oracle and is not editable"),
            ));
        }
        if !s.baseline.contains_key(path) || !path.ends_with(".spx") {
            return Err(bad(format!("`{path}` is not a baseline source file")));
        }
        if find.is_empty() {
            return Err(bad("`find` must not be empty".into()));
        }
        if !texts.contains_key(path) {
            let cur = std::fs::read_to_string(s.work.join(path))
                .map_err(|e| bad(format!("{path}: {e}")))?;
            texts.insert(path.to_string(), cur);
        }
        let text = texts.get_mut(path).expect("inserted");
        let n = text.matches(find).count();
        if n != 1 {
            return Err(bad(format!(
                "`find` must match exactly once in `{path}` (matched {n})"
            )));
        }
        *text = text.replacen(find, replace, 1);
    }
    // Protected facts relative to the exact baseline bytes.
    let mut base_effects = BTreeSet::new();
    let mut new_effects = BTreeSet::new();
    for (path, new) in &texts {
        let base = String::from_utf8_lossy(&s.baseline[path]).into_owned();
        let (bl, nl) = (count_map(law_lines(&base)), count_map(law_lines(new)));
        if bl.iter().any(|(l, c)| nl.get(l).copied().unwrap_or(0) < *c) {
            return Err(d(
                "SPX-HPD042",
                format!(
                    "the patch deletes or weakens a law (requires/ensures/invariant) in `{path}`"
                ),
            ));
        }
        base_effects.extend(effect_tokens(&base));
        new_effects.extend(effect_tokens(new));
    }
    if let Some(x) = new_effects.difference(&base_effects).next() {
        return Err(d(
            "SPX-HPD043",
            format!("the patch widens declared effects (`{x}`)"),
        ));
    }
    for (path, new) in &texts {
        std::fs::write(s.work.join(path), new)
            .map_err(|e| d("SPX-HPD070", format!("scratch write: {e}")))?;
    }
    Ok(texts.into_keys().collect())
}

fn diag_text(ds: &[super::compiler::CompilerDiagnostic]) -> String {
    ds.iter()
        .map(|x| match (&x.path, x.line) {
            (Some(p), Some(l)) => format!("{}: {} ({p}:{l})", x.code, x.message),
            _ => format!("{}: {}", x.code, x.message),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn source_items(s: &State, ds: &[super::compiler::CompilerDiagnostic]) -> Vec<ContextItem> {
    let mut paths: Vec<String> = ds.iter().filter_map(|x| x.path.clone()).collect();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        paths = s
            .baseline
            .keys()
            .filter(|p| p.ends_with(".spx"))
            .cloned()
            .collect();
    }
    paths
        .into_iter()
        .take(6)
        .filter_map(|p| {
            let text = std::fs::read_to_string(s.work.join(&p)).ok()?;
            Some(ContextItem {
                label: p,
                provenance: "host:scratch-source".into(),
                text: text.chars().take(16 * 1024).collect(),
            })
        })
        .collect()
}

pub(super) fn repair_loop(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    s: &mut State,
) -> HarnessResult<()> {
    let task = cx.cfg.task.clone();
    let mut diags = r.diagnostics.clone();
    loop {
        s.check_bounds_pub(cx)?;
        s.cancelled_pub(cx, journal)?;
        super::cost_ladder::observe(cx, &s.feedback, s.last_failures.len());
        let n = s.attempts.len() as u32 + 1;
        let view = st.command.view("diagnostics", &diag_text(&diags), 4096);
        let kept = source_items(s, &diags);
        let projected = s.project_feedback(cx, r)?;
        s.ctx_revision = "unverified-baseline".into();
        s.ctx_candidate = None;
        let pc = PromptCtx {
            revision: "unverified-baseline",
            seed: task.seed.as_deref(),
            diag_view: &view,
            kept: &kept,
            ops: &[],
            feedback: &projected,
            attempt: n,
            scratch_repair: true,
            phase: None,
        };
        s.attempts
            .push(json!({"attempt": n, "outcome": "started", "mode": "scratch-repair"}));
        let proposal = attempt::propose_step_gated(
            cx,
            st,
            journal,
            r,
            &pc,
            &format!("repair-{n}"),
            &mut |cx, j| s.admit_retry(cx, j),
        )?;
        s.cancelled_pub(cx, journal)?;
        let pdigest = sha256_plain(
            crate::json::canonical(&json!([
                proposal.kind,
                proposal.source_patch,
                proposal.intent
            ]))
            .as_bytes(),
        );
        if let Some(a) = s.attempts.last_mut() {
            a["proposal_digest"] = json!(pdigest);
            a["kind"] = json!(proposal.kind);
        }
        if !s.seen_proposals.insert(pdigest) {
            return Err(d(
                "SPX-HPD112",
                "no progress: the proposer repeated an identical proposal; stopping with all attempts counted",
            ));
        }
        if let Some(reason) = &proposal.unsupported {
            return Err(d("SPX-HPD092", format!("unsupported goal: {reason}")));
        }
        let Some(patch) = proposal.source_patch.clone() else {
            s.record_failure_pub(n, "proposal", "SPX-HPD117", "an unverified baseline accepts only a bounded `source_patch`; semantic operations need a verified base", journal)?;
            continue;
        };
        if let Err(e) = apply_patch(s, &patch) {
            s.record_failure_pub(n, "patch", e.code, &e.message, journal)?;
            continue;
        }
        s.ctx_candidate = Some("scratch-edit".into());
        let check = cx.compiler.check(&s.work)?;
        if !check.ok {
            diags = check.diagnostics.clone();
            s.record_failure_pub(n, "check", "SPX-HPD050", &diag_text(&diags), journal)?;
            continue;
        }
        let test = cx.compiler.test(&s.work)?;
        if !test.passed {
            let m = format!(
                "tests failed ({})",
                test.failure.clone().unwrap_or(test.outcome.clone())
            );
            diags = vec![];
            s.record_failure_pub(n, "tests", "SPX-HPD050", &m, journal)?;
            continue;
        }
        let revision = check.revision.clone().expect("verified");
        s.candidates += 1;
        let files: Vec<String> = s
            .baseline
            .keys()
            .filter(|p| {
                std::fs::read(s.work.join(p))
                    .map(|b| b != s.baseline[*p])
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        s.steps
            .push(json!({"step": s.steps.len() + 1, "intent": "scratch_patch",
            "candidate_revision": revision, "files": files}));
        if let Some(a) = s.attempts.last_mut() {
            a["outcome"] = json!("admitted");
            a["candidate_revision"] = json!(revision);
        }
        journal.append(
            &format!("attempt-{n}"),
            "done",
            s.attempts.last().cloned().unwrap_or(Value::Null),
        )?;
        s.feedback.clear();
        s.last_failures.clear();
        step(r, "repair", "scratch repaired; the compiler verifies it");
        if task.mode == TaskMode::Change {
            // Semantic operations only now that the candidate is a verified base.
            let ops = cx.compiler.supported_intents(&s.work, &revision)?;
            r.operations = json!({"source": "installed compiler", "kinds": ops});
            return loop_steps(
                cx,
                st,
                journal,
                r,
                s,
                revision,
                task.seed.clone(),
                ops,
                true,
            );
        }
        return finish(cx, st, journal, r, s, &revision, true);
    }
}
