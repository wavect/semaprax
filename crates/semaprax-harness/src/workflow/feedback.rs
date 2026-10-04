//! TC-06: deterministic model-facing projection of repair feedback.
//!
//! The raw failures (complete messages, check output, digests, delivered-token
//! accounting) stay in `State` and the journal. This module derives what the
//! next request carries: the latest failure exactly, plus a compact record of
//! already-rejected fixes, inside a labelled **byte** allowance. Accounting-only
//! fields never reach the prompt; they are returned in the report instead.
//! Pure and deterministic: no model call, no clock, no I/O.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use serde_json::{json, Map, Value};

/// Bytes the projected feedback array may use, and the approved ceiling the
/// indispensable current failure may enlarge to before the session stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedbackPolicy {
    pub max_bytes: usize,
    pub hard_max_bytes: usize,
}

impl Default for FeedbackPolicy {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024,
            hard_max_bytes: 64 * 1024,
        }
    }
}

pub struct Projection {
    pub entries: Vec<Value>,
    pub report: Value,
}

/// Longest first line kept for a superseded diagnostic.
const HISTORY_LINE: usize = 120;

fn s<'a>(e: &'a Value, k: &str) -> &'a str {
    e[k].as_str().unwrap_or("")
}

/// Drops the accounting suffix `; output <12 hex>` that `check_feedback` adds.
fn strip_output_digest(m: &str) -> &str {
    match m.rfind("; output ") {
        Some(i) if m.len() - i - 9 == 12 && m[i + 9..].bytes().all(|b| b.is_ascii_hexdigit()) => {
            &m[..i]
        }
        _ => m,
    }
}

fn digest(e: &Value) -> String {
    sha256_plain(format!("{}\n{}", s(e, "code"), strip_output_digest(s(e, "message"))).as_bytes())
}

/// Collapses runs of identical consecutive lines; the text of the first line is
/// kept verbatim and the run length is stated.
fn collapse_runs(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut it = text.split('\n').peekable();
    while let Some(line) = it.next() {
        let mut n = 1;
        while it.peek() == Some(&line) && !line.trim().is_empty() {
            it.next();
            n += 1;
        }
        out.push(line.to_string());
        if n > 2 {
            out.push(format!("[previous line repeated {} more times]", n - 1));
        } else if n == 2 {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

fn first_line(m: &str) -> String {
    let l = m.lines().next().unwrap_or("");
    if l.len() <= HISTORY_LINE {
        return l.to_string();
    }
    let mut end = HISTORY_LINE;
    while !l.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &l[..end])
}

fn current_entry(e: &Value, recovery_unavailable: &mut bool) -> Value {
    let mut o = Map::new();
    for k in ["attempt", "stage", "code"] {
        o.insert(k.into(), e[k].clone());
    }
    let msg = strip_output_digest(s(e, "message"));
    o.insert("message".into(), json!(msg));
    // Label which object the failure describes: never the accepted revision.
    let subject = if e["candidate_revision"].is_string() {
        "rejected-scratch-candidate"
    } else {
        "rejected-proposal"
    };
    o.insert("subject".into(), json!(subject));
    for k in ["base_revision", "candidate_revision", "proposed"] {
        if !e[k].is_null() {
            o.insert(k.into(), e[k].clone());
        }
    }
    if let Some(c) = e["check_output"].as_object() {
        let mut out = Map::new();
        for k in ["check", "status", "route"] {
            if !c[k].is_null() {
                out.insert(k.into(), c[k].clone());
            }
        }
        if c["status_certain"] == json!(false) {
            out.insert("status_certain".into(), json!(false));
        }
        let incomplete = c["incomplete"] == json!(true);
        if incomplete {
            out.insert("incomplete".into(), json!(true));
        }
        // A printed handle does not make recovery available: the proposal
        // interface has no tool loop, so the handle is not offered.
        if incomplete || !c["recovery_handle"].is_null() {
            *recovery_unavailable = true;
        }
        if incomplete {
            out.insert("recovery".into(), json!("unavailable"));
        }
        let text = collapse_runs(c["output"].as_str().unwrap_or(""));
        // Lines the message already carries are not repeated.
        let kept: Vec<&str> = text
            .lines()
            .filter(|l| !msg.lines().any(|m| m == *l))
            .collect();
        out.insert("output".into(), json!(kept.join("\n")));
        o.insert("check_output".into(), Value::Object(out));
    }
    Value::Object(o)
}

/// One superseded diagnostic group: repeated attempts are represented once.
struct Group {
    digest: String,
    attempts: Vec<u64>,
    stage: String,
    code: String,
    line: String,
    full: Option<String>,
    rejected: Vec<String>,
}

fn size(v: &[Value]) -> usize {
    crate::json::canonical(&Value::Array(v.to_vec())).len()
}

/// Projects raw feedback (oldest first, newest = current failure).
pub fn project(raw: &[Value], policy: &FeedbackPolicy) -> HarnessResult<Projection> {
    let raw_bytes = size(raw);
    let Some((cur, earlier)) = raw.split_last() else {
        return Ok(Projection {
            entries: vec![],
            report: json!({"entries_in": 0, "entries_out": 0, "unit": "bytes"}),
        });
    };
    let cur_digest = digest(cur);
    let mut recovery_unavailable = false;
    let current = current_entry(cur, &mut recovery_unavailable);
    let mut groups: Vec<Group> = Vec::new();
    for e in earlier {
        let dg = digest(e);
        let kind = e["proposed"]["kind"]
            .as_str()
            .or_else(|| e["proposed"].as_str())
            .unwrap_or("")
            .to_string();
        let attempt = e["attempt"].as_u64().unwrap_or(0);
        if let Some(g) = groups.iter_mut().find(|g| g.digest == dg) {
            g.attempts.push(attempt);
            if !kind.is_empty() && !g.rejected.contains(&kind) {
                g.rejected.push(kind);
            }
            continue;
        }
        let msg = strip_output_digest(s(e, "message"));
        groups.push(Group {
            digest: dg,
            attempts: vec![attempt],
            stage: s(e, "stage").into(),
            code: s(e, "code").into(),
            line: first_line(msg),
            // Oracle violations are security-relevant: kept whole.
            full: (s(e, "stage") == "oracle").then(|| msg.to_string()),
            rejected: if kind.is_empty() { vec![] } else { vec![kind] },
        });
    }
    let render = |gs: &[Group]| -> Vec<Value> {
        let mut v: Vec<Value> = gs
            .iter()
            .map(|g| {
                let mut o = json!({"attempts": g.attempts, "stage": g.stage, "code": g.code,
                                   "rejected": g.rejected});
                if g.digest == cur_digest {
                    o["same_as_current"] = json!(true);
                } else {
                    o["message"] = json!(g.full.clone().unwrap_or_else(|| g.line.clone()));
                }
                o
            })
            .collect();
        v.push(current.clone());
        v
    };
    let mut dropped = 0usize;
    // Obsolete history goes first, oldest first; oracle groups last.
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by_key(|&i| (groups[i].full.is_some(), i));
    let mut gone: Vec<usize> = Vec::new();
    let mut entries;
    loop {
        let live: Vec<Group> = groups
            .iter()
            .enumerate()
            .filter(|(i, _)| !gone.contains(i))
            .map(|(_, g)| Group {
                digest: g.digest.clone(),
                attempts: g.attempts.clone(),
                stage: g.stage.clone(),
                code: g.code.clone(),
                line: g.line.clone(),
                full: g.full.clone(),
                rejected: g.rejected.clone(),
            })
            .collect();
        entries = render(&live);
        if size(&entries) <= policy.max_bytes || gone.len() == groups.len() {
            break;
        }
        gone.push(order[gone.len()]);
        dropped += 1;
    }
    let bytes = size(&entries);
    let enlarged = bytes > policy.max_bytes;
    if bytes > policy.hard_max_bytes {
        return Err(HarnessDiagnostic::new(
            "SPX-HPD111",
            format!(
                "session bound exhausted: the current failure needs {bytes} feedback bytes, over the approved {}; stopping rather than cutting the error",
                policy.hard_max_bytes
            ),
        ));
    }
    let report = json!({"unit": "bytes", "note": "labelled byte policy; request tokens are counted by the request budget",
        "entries_in": raw.len(), "entries_out": entries.len(), "raw_bytes": raw_bytes,
        "projected_bytes": bytes, "max_bytes": policy.max_bytes, "enlarged_within_approval": enlarged,
        "dropped_history_groups": dropped, "current_diagnostic_digest": cur_digest,
        "recovery": if recovery_unavailable { json!("unavailable") } else { Value::Null }});
    Ok(Projection { entries, report })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fail(n: u32, code: &str, msg: &str) -> Value {
        json!({"attempt": n, "stage": "checks", "code": code, "message": msg,
               "base_revision": "rev-B", "proposed": {"kind": "replace_function_body"}})
    }
    fn big_check(n: usize) -> Value {
        let out: String = (0..n).map(|i| format!("error line {i}\n")).collect();
        json!({"check": "unit", "status": 1, "status_certain": true, "route": "tail",
               "incomplete": false, "recovery_handle": "cv-0001", "recovery_project_id": "p",
               "output": out,
               "delivered": {"raw_tokens": 9999, "saved_tokens": 5, "tokenizer": "t"}})
    }

    #[test]
    fn four_large_failures_stay_bounded_and_current_is_exact() {
        let mut raw = vec![];
        for n in 1..=4 {
            let mut e = fail(
                n,
                &format!("SPX-C{n}"),
                &format!("old failure {n} {}", "x".repeat(6000)),
            );
            e["check_output"] = big_check(300);
            raw.push(e);
        }
        let cur_msg = raw[3]["message"].as_str().unwrap().to_string();
        let p = project(&raw, &FeedbackPolicy::default()).unwrap();
        let last = p.entries.last().unwrap();
        assert_eq!(last["message"], cur_msg.as_str());
        assert!(last["check_output"]["output"]
            .as_str()
            .unwrap()
            .contains("error line 299"));
        assert!(
            size(&p.entries) < size(&raw) / 2,
            "{} vs {}",
            size(&p.entries),
            size(&raw)
        );
        assert!(p.entries[..3]
            .iter()
            .all(|e| e["message"].as_str().unwrap().len() <= 125));
        let wire = Value::Array(p.entries.clone()).to_string();
        assert!(!wire.contains("raw_tokens") && !wire.contains("cv-0001"));
    }

    #[test]
    fn repeated_diagnostic_is_one_group_and_stop_inputs_are_untouched() {
        let raw: Vec<Value> = (1..=3)
            .map(|n| fail(n, "SPX-D", "same diagnostic"))
            .collect();
        let before = raw.clone();
        let p = project(&raw, &FeedbackPolicy::default()).unwrap();
        assert_eq!(p.entries.len(), 2);
        assert_eq!(p.entries[0]["attempts"], json!([1, 2]));
        assert_eq!(p.entries[0]["same_as_current"], true);
        assert!(p.entries[0].get("message").is_none());
        assert_eq!(
            raw, before,
            "raw history (digest source of the no-progress stop) is unchanged"
        );
    }

    #[test]
    fn candidate_failure_is_labelled_not_current_revision() {
        let mut e = fail(1, "SPX-X", "boom");
        e["candidate_revision"] = json!("rev-A");
        let p = project(&[e], &FeedbackPolicy::default()).unwrap();
        let c = &p.entries[0];
        assert_eq!(c["subject"], "rejected-scratch-candidate");
        assert_eq!(c["candidate_revision"], "rev-A");
        assert_eq!(c["base_revision"], "rev-B");
        let q = project(&[fail(1, "SPX-X", "boom")], &FeedbackPolicy::default()).unwrap();
        assert_eq!(q.entries[0]["subject"], "rejected-proposal");
    }

    #[test]
    fn recovery_is_reported_unavailable_never_offered() {
        let mut e = fail(1, "SPX-X", "boom; output 0123456789ab");
        let mut c = big_check(3);
        c["incomplete"] = json!(true);
        e["check_output"] = c;
        let p = project(&[e], &FeedbackPolicy::default()).unwrap();
        assert_eq!(p.entries[0]["check_output"]["recovery"], "unavailable");
        assert_eq!(p.entries[0]["message"], "boom");
        assert_eq!(p.report["recovery"], "unavailable");
    }

    #[test]
    fn oversize_current_enlarges_within_approval_or_stops() {
        let big = "e".repeat(20_000);
        let raw = vec![fail(1, "SPX-X", &big)];
        let p = project(&raw, &FeedbackPolicy::default()).unwrap();
        assert_eq!(p.entries[0]["message"], big.as_str());
        assert_eq!(p.report["enlarged_within_approval"], true);
        let tight = FeedbackPolicy {
            max_bytes: 1000,
            hard_max_bytes: 5000,
        };
        let e = project(&raw, &tight).err().unwrap();
        assert_eq!(e.code, "SPX-HPD111");
    }

    #[test]
    fn oracle_diagnostic_survives_history_compaction() {
        let mut o = fail(
            1,
            "SPX-HPD114",
            "edits the oracle: tests/a.spx and a very long tail",
        );
        o["stage"] = json!("oracle");
        let raw = vec![o, fail(2, "SPX-Y", "other")];
        let p = project(&raw, &FeedbackPolicy::default()).unwrap();
        assert_eq!(
            p.entries[0]["message"],
            "edits the oracle: tests/a.spx and a very long tail"
        );
    }

    #[test]
    fn one_error_short_output_is_not_longer_than_baseline() {
        let mut e = fail(
            1,
            "SPX-X",
            "candidate rejected: check failed; output 0123456789ab",
        );
        e["check_output"] = big_check(2);
        let baseline = json!({"attempt": 1, "stage": "checks", "code": "SPX-X",
            "message": e["message"], "check_output": e["check_output"]});
        let p = project(&[e], &FeedbackPolicy::default()).unwrap();
        assert!(
            size(&p.entries) <= size(&[baseline]),
            "{} > {}",
            size(&p.entries),
            size(&[json!(0)])
        );
    }

    #[test]
    fn identical_lines_collapse_and_next_request_shrinks() {
        let rep = "warning: same\n".repeat(500) + "error: real\n";
        assert!(collapse_runs(&rep).contains("[previous line repeated 499 more times]"));
        assert!(collapse_runs(&rep).ends_with("error: real\n"));
        let mut raw = vec![];
        for n in 1..=4 {
            let mut e = fail(n, "SPX-Z", &format!("failure {n}"));
            let mut c = big_check(1);
            c["output"] = json!(rep.clone());
            e["check_output"] = c;
            raw.push(e);
        }
        let p = project(&raw, &FeedbackPolicy::default()).unwrap();
        let old = json!({"feedback": raw}).to_string().len();
        let new = json!({"feedback": p.entries}).to_string().len();
        assert!(new * 4 < old, "{new} vs {old}");
    }
}
