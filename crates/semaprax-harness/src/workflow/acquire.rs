//! Local proposal acquisition (TC-09): a completed same-lineage journal
//! proposal or an explicit host-supplied scripted proposal is taken before any
//! routing, fitting or reservation. A local hit makes zero router and model
//! calls and reserves nothing; the bytes are still untrusted and go through
//! the same parse, protected-fact, preview, test and approval path as a
//! generated proposal. A completed record whose artifact cannot be validated
//! is refused, never answered by a fresh billable replay.

use super::journal::{Journal, Record};
use super::pipeline::{Ctx, Stages};
use super::report::Report;
use super::stages::*;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use crate::observe::{Availability, Role, Stage};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;

/// Bound on a stored proposal artifact (same as the scripted-file bound).
pub(super) const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024;

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Where the proposal artifact of `step` lives for this lineage.
pub(super) fn cache_path(cx: &Ctx, step: &str) -> PathBuf {
    cx.cfg.cache_dir.join(if step == "generate" {
        format!("{}.proposal.json", cx.lineage.id)
    } else {
        format!("{}.{step}.proposal.json", cx.lineage.id)
    })
}

/// Identity recorded with a completed generation.
pub(super) fn done_detail(cx: &Ctx, bytes: &[u8], incurred: Value) -> Value {
    json!({
        "digest": sha256_plain(bytes),
        "bytes": bytes.len(),
        "lineage": cx.lineage.id,
        "task": cx.lineage.task_digest,
        "lock": cx.lineage.lock_digest,
        "revision": cx.lineage.project.revision,
        "incurred": incurred,
    })
}

fn refuse(step: &str, why: String) -> HarnessDiagnostic {
    d(
        "SPX-HPD072",
        format!("uncertain: the completed proposal for `{step}` failed validation ({why}); it is not replayed, supply --proposal or change the task"),
    )
}

fn validate_done(cx: &Ctx, step: &str, rec: &Record) -> HarnessResult<Vec<u8>> {
    let path = cache_path(cx, step);
    let meta = std::fs::metadata(&path).map_err(|_| refuse(step, "artifact missing".into()))?;
    if meta.len() > MAX_ARTIFACT_BYTES {
        return Err(refuse(step, "artifact exceeds the size bound".into()));
    }
    let bytes =
        std::fs::read(&path).map_err(|e| refuse(step, format!("artifact unreadable: {e}")))?;
    let det = &rec.detail;
    if det["digest"].as_str() != Some(sha256_plain(&bytes).as_str()) {
        return Err(refuse(step, "digest mismatch".into()));
    }
    if let Some(n) = det["bytes"].as_u64() {
        if n != bytes.len() as u64 {
            return Err(refuse(step, "size mismatch".into()));
        }
    }
    let l = cx.lineage;
    for (k, want) in [
        ("lineage", l.id.as_str()),
        ("task", l.task_digest.as_str()),
        ("lock", l.lock_digest.as_str()),
        ("revision", l.project.revision.as_str()),
    ] {
        if let Some(have) = det[k].as_str() {
            if have != want {
                return Err(refuse(step, format!("{k} identity mismatch")));
            }
        }
    }
    Ok(bytes)
}

/// The local decision. `Ok(Some(bytes))` is a true local hit (no router, no
/// model, no reservation); `Ok(None)` means a fresh model request is needed.
pub(super) fn local_proposal(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    r: &mut Report,
    step: &str,
) -> HarnessResult<Option<Vec<u8>>> {
    let started = Instant::now();
    let (source, bytes, historical) = if st.proposer.local_source() {
        let req = ProposalRequest {
            lineage: cx.lineage,
            prompt: Value::Null,
            model: String::new(),
        };
        let b = st.proposer.propose(&req).map_err(|f| match f {
            StageFailure::Unavailable(x) => d(
                "SPX-HPD090",
                format!("no proposal available: {} {}", x.code, x.message),
            ),
            StageFailure::Refused(x) | StageFailure::Uncertain(x) => x,
        })?;
        ("scripted", b, Value::Null)
    } else if st.proposer.side_effecting() {
        match journal.state(step) {
            Some(rec) if rec.state == "done" => {
                let rec = rec.clone();
                let b = validate_done(cx, step, &rec)?;
                ("journal", b, rec.detail["incurred"].clone())
            }
            Some(rec) if matches!(rec.state.as_str(), "begin" | "uncertain") => {
                // Non-replayable: refuse before any routing or reservation.
                return Err(d("SPX-HPD072", "uncertain: a model generation in this lineage began without a recorded result; it is not replayed, supply --proposal or change the task"));
            }
            _ => return Ok(None),
        }
    } else {
        return Ok(None);
    };
    if source == "journal" {
        journal.append(
            &format!("{step}.local-reuse"),
            "done",
            json!({"digest": sha256_plain(&bytes)}),
        )?;
    }
    cx.observe(
        &st.proposer.id(),
        "proposal.local_reuse",
        Stage::Generation,
        Role::Local,
        Availability::Available,
        true,
        started,
    );
    r.route = json!({"choice": "local-proposal", "source": source, "router_calls": 0});
    r.context["proposal_acquisition"] = json!({
        "source": source,
        "router_calls": 0,
        "model_calls": 0,
        "new_reservations": 0,
        "historical_incurred": historical,
    });
    r.notes.push(if source == "journal" {
        "proposal reused from the journal before routing; the model was not invoked again"
            .to_string()
    } else {
        "scripted proposal taken before routing; no router or model call".to_string()
    });
    Ok(Some(bytes))
}
