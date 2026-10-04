//! `report <observations.jsonl> [--json]`

use super::event::Observation;
use super::report::build_report;
use super::sink::{HostTraffic, SUMMARY_SCHEMA};
use crate::cli::{Environment, Outcome};
use crate::diag::HarnessDiagnostic;
use crate::json::{parse_strict, JsonLimits};

const MAX_TRACE_BYTES: u64 = 64 * 1024 * 1024;

pub fn cli_report(args: &[String], env: &Environment) -> Outcome {
    let mut json = false;
    let mut path = None;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            s if s.starts_with("--") => {
                return Outcome::usage(format!("unknown report flag `{s}`"))
            }
            s if path.is_none() => path = Some(s),
            _ => return Outcome::usage("report takes one observations file"),
        }
    }
    let Some(path) = path else {
        return Outcome::usage("usage: report <observations.jsonl> [--json]");
    };
    let path = env.cwd.join(path);
    let bad = |m: String| Outcome::refused(&HarnessDiagnostic::new("SPX-HPO002", m));
    match std::fs::metadata(&path) {
        Ok(m) if m.len() <= MAX_TRACE_BYTES => {}
        Ok(_) => return bad("observation trace too large".into()),
        Err(e) => return bad(format!("cannot read trace: {e}")),
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return bad(format!("cannot read trace: {e}")),
    };
    let limits = JsonLimits::frame(1 << 20);
    let (mut events, mut dropped, mut traffic) = (Vec::new(), 0u64, None);
    for (i, line) in bytes.split(|b| *b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let v = match parse_strict(line, &limits) {
            Ok(v) => v,
            Err(d) => return bad(format!("line {}: {}", i + 1, d.message)),
        };
        if v.get("schema").and_then(|s| s.as_str()) == Some(SUMMARY_SCHEMA) {
            dropped += v.get("dropped").and_then(|d| d.as_u64()).unwrap_or(0);
            traffic = v.get("host_traffic").and_then(|t| {
                Some(HostTraffic {
                    observed: t.get("observed")?.as_u64()?,
                    unobserved: t.get("unobserved")?.as_u64()?,
                })
            });
            continue;
        }
        match Observation::from_json(&v) {
            Ok(e) => events.push(e),
            Err(d) => return bad(format!("line {}: {}", i + 1, d.message)),
        }
    }
    let report = build_report(&events, dropped, traffic);
    if json {
        Outcome::ok(format!("{}\n", crate::json::canonical(&report.json)))
    } else {
        Outcome::ok(report.text())
    }
}
