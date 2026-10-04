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
    let (mut export, mut output, mut session) = (None::<String>, None::<String>, None::<String>);
    let mut flags = args.iter();
    while let Some(a) = flags.next() {
        match a.as_str() {
            "--json" => json = true,
            "--export" | "--output" | "--session" => {
                let Some(v) = flags.next() else {
                    return Outcome::usage(format!("`{a}` needs a value"));
                };
                match a.as_str() {
                    "--export" => export = Some(v.clone()),
                    "--output" => output = Some(v.clone()),
                    _ => session = Some(v.clone()),
                }
            }
            s if s.starts_with("--") => {
                return Outcome::usage(format!("unknown report flag `{s}`"))
            }
            s if path.is_none() => path = Some(s),
            _ => return Outcome::usage("report takes one observations file"),
        }
    }
    let Some(path) = path else {
        return Outcome::usage("usage: report <observations.jsonl> [--json] [--export token-observation [--output f] [--session id]]");
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
    if let Some(kind) = export {
        if kind != "token-observation" {
            return Outcome::usage("--export supports only `token-observation`");
        }
        let text =
            super::export::to_jsonl(&events, session.as_deref().unwrap_or("harness-session"));
        return match output {
            Some(o) => {
                let o = env.cwd.join(o);
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&o)
                {
                    Ok(mut f) => match std::io::Write::write_all(&mut f, text.as_bytes()) {
                        Ok(()) => Outcome::ok(format!(
                            "exported {} row(s) to {}\n",
                            events.len(),
                            o.display()
                        )),
                        Err(e) => bad(format!("cannot write {}: {e}", o.display())),
                    },
                    Err(e) => bad(format!("cannot create {}: {e}", o.display())),
                }
            }
            None => Outcome::ok(text),
        };
    }
    let report = build_report(&events, dropped, traffic);
    if json {
        Outcome::ok(format!("{}\n", crate::json::canonical(&report.json)))
    } else {
        Outcome::ok(report.text())
    }
}
