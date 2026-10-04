//! `command.view/v1` suite: post-execution views never lose critical errors
//! silently and never carry the exit status.

use super::report::{check, fail, fail_with, Case, Suite};
use super::suite::{cancellation, completed, usable, Rig, Target};
use crate::contract::CapabilityKind::CommandView as KIND;
use crate::json::canonical;
use serde_json::{json, Value};

const STDOUT: &str = "compiling crate one\ncompiling crate one\ncompiling crate one\nwarning: unused import\nerror[E0425]: planted_critical_a is missing\nlinking done\n";
const STDERR: &str = "note: some noise\nthread 'main' panicked: planted_critical_b\n";

fn payload() -> Value {
    json!({"form": "post-execution", "argv": ["cargo", "test"], "stdout": STDOUT, "stderr": STDERR})
}

pub fn run(t: &Target) -> Suite {
    let name = "command.view";
    if !t.ops(KIND).iter().any(|o| o == "view") {
        return Suite::new(
            name,
            "adapter",
            vec![Case::unverified(
                "declared-operations",
                "descriptor declares no `view` operation",
            )],
        );
    }
    let mut cases = Vec::new();
    match Rig::open(t, &[]) {
        Err(d) => cases.push(Case::failed("setup", fail(d.to_string()))),
        Ok(mut rig) => {
            cases.push(check("critical-lines-survive-or-loss-is-declared", || {
                let (_, p) = usable(rig.run(KIND, "view", payload()))?;
                let v = &p["view"];
                let text = v["text"].as_str().unwrap_or("");
                let missing: Vec<&str> = ["planted_critical_a", "planted_critical_b"].into_iter().filter(|m| !text.contains(m)).collect();
                let ev = json!({"missing": missing, "lossless": v["lossless"], "omissions": v["omissions"]});
                if missing.is_empty() {
                    return Ok(ev);
                }
                let declared = v["lossless"] == false
                    && v["omissions"].as_u64().unwrap_or(0) >= missing.len() as u64
                    && v["recovery_handle"].as_str().is_some();
                if declared {
                    Ok(ev)
                } else {
                    Err(fail_with(
                        "a critical error line was dropped without a lossy marker covering it and a recovery handle",
                        ev,
                    ))
                }
            }));
            cases.push(check("lossless-claim-is-honest", || {
                let (_, p) = usable(rig.run(KIND, "view", payload()))?;
                let v = &p["view"];
                let lossless = v["lossless"].as_bool().unwrap_or(false);
                let omissions = v["omissions"].as_u64().unwrap_or(0);
                let ev = json!({"lossless": lossless, "omissions": omissions});
                if lossless {
                    let out: Vec<&str> = v["text"]
                        .as_str()
                        .unwrap_or("")
                        .lines()
                        .map(str::trim)
                        .collect();
                    let dropped: Vec<&str> = STDOUT
                        .lines()
                        .chain(STDERR.lines())
                        .filter(|l| !out.contains(&l.trim()))
                        .collect();
                    if !dropped.is_empty() || omissions != 0 {
                        return Err(fail_with(
                            "view claims lossless but input lines are missing or omissions > 0",
                            json!({"dropped": dropped.len(), "omissions": omissions}),
                        ));
                    }
                } else if omissions == 0 {
                    return Err(fail_with("view claims loss but reports zero omissions", ev));
                }
                Ok(ev)
            }));
            cases.push(check("never-carries-exit-status", || {
                match completed(rig.run(KIND, "view", payload())) {
                    Ok(_) => Ok(json!({"host_validator": "SPX-HPA042 not triggered"})),
                    Err(f) => Err(f),
                }
            }));
            cases.push(check("deterministic-for-equal-input", || {
                let a = usable(rig.run(KIND, "view", payload()))?.1;
                let b = usable(rig.run(KIND, "view", payload()))?.1;
                if canonical(&a) != canonical(&b) {
                    return Err(fail("two identical requests returned different views"));
                }
                Ok(json!({"equal": true}))
            }));
            cases.push(match usable(rig.run(KIND, "view", payload())) {
                Ok((_, p)) => Case {
                    name: "raw-recovery".into(),
                    verdict: super::report::Verdict::Unverified,
                    evidence: json!({"reason": "raw recovery is performed by the host's `recover` verb (HP-08); only handle presence is observed",
                                     "recovery_handle_present": p["view"]["recovery_handle"].is_string()}),
                },
                Err(f) => Case::failed("raw-recovery", f),
            });
        }
    }
    if t.ops(KIND).iter().any(|o| o == "wrap") {
        cases.push(Case::unverified(
            "wrapper-form",
            "wrapper plans are executed once by the host (HP-08); the kit does not execute argv",
        ));
    }
    cases.push(match Rig::open(t, &[]) {
        Err(d) => Case::failed("cancellation-cooperative", fail(d.to_string())),
        Ok(mut rig) => check("cancellation-cooperative", || {
            cancellation(&mut rig, KIND, "view", payload())
        }),
    });
    Suite::new(name, "adapter", cases)
}
