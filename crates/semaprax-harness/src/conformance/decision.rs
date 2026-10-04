//! `decision.evaluate/v1` suite: choices stay within the offered options,
//! abstention is explicit and malformed scores are refused by the host.

use super::report::{check, fail, fail_with, Case, Suite};
use super::suite::{cancellation, usable, Rig, Target};
use crate::contract::payload::{check_against_request, validate_payload};
use crate::contract::{CapabilityKind::DecisionEvaluate as KIND, Direction};
use crate::json::{canonical, parse_strict, JsonLimits};
use serde_json::{json, Value};

fn req(features: Value, options: &[&str]) -> Value {
    json!({"task": "model-route/v1", "features": features, "options": options})
}

pub fn run(t: &Target) -> Suite {
    let name = "decision.evaluate";
    if !t.ops(KIND).iter().any(|o| o == "evaluate") {
        return Suite::new(
            name,
            "adapter",
            vec![Case::unverified(
                "declared-operations",
                "descriptor declares no `evaluate` operation",
            )],
        );
    }
    let mut cases = Vec::new();
    match Rig::open(t, &[]) {
        Err(d) => cases.push(Case::failed("setup", fail(d.to_string()))),
        Ok(mut rig) => {
            cases.push(check("choice-within-options", || {
                let (_, p) = usable(rig.run(
                    KIND,
                    "evaluate",
                    req(json!({"complexity": 0.9}), &["cheap", "strong"]),
                ))?;
                match p["choice"].as_str() {
                    Some(c) if ["cheap", "strong"].contains(&c) => Ok(json!({"choice": c})),
                    Some(c) => Err(fail_with(
                        "choice is not an offered option",
                        json!({"choice": c}),
                    )),
                    None if p["abstain"] == true => Ok(json!({"abstain": true})),
                    None => Err(fail("neither a choice nor an abstention")),
                }
            }));
            cases.push(check("abstain-is-explicit", || {
                let (_, p) =
                    usable(rig.run(KIND, "evaluate", req(json!({}), &["cheap", "strong"])))?;
                match (p["choice"].is_null(), p["abstain"].as_bool()) {
                    (true, Some(true)) => Ok(json!({"abstained": true})),
                    (false, Some(false)) => Ok(json!({"abstained": false, "choice": p["choice"]})),
                    _ => Err(fail_with("choice and abstain disagree", p.clone())),
                }
            }));
            cases.push(check("forbidden-option-never-chosen", || {
                // The host removed `cheap` (policy); only `strong` is offered.
                let (_, p) = usable(rig.run(
                    KIND,
                    "evaluate",
                    req(json!({"complexity": 0.1}), &["strong"]),
                ))?;
                match p["choice"].as_str() {
                    None | Some("strong") => Ok(json!({"choice": p["choice"]})),
                    Some(c) => Err(fail_with(
                        "adapter chose an option the host did not offer",
                        json!({"choice": c}),
                    )),
                }
            }));
            cases.push(check("deterministic-for-equal-input", || {
                let mut r = || {
                    usable(rig.run(
                        KIND,
                        "evaluate",
                        req(json!({"complexity": 0.3}), &["cheap", "strong"]),
                    ))
                    .map(|x| x.1)
                };
                let (a, b) = (r()?, r()?);
                if canonical(&a) != canonical(&b) {
                    return Err(fail("two identical requests returned different decisions"));
                }
                Ok(json!({"equal": true}))
            }));
        }
    }
    cases.push(check("host-refuses-invalid-results", host_refusals));
    cases.push(match Rig::open(t, &[]) {
        Err(d) => Case::failed("cancellation-cooperative", fail(d.to_string())),
        Ok(mut rig) => check("cancellation-cooperative", || {
            cancellation(
                &mut rig,
                KIND,
                "evaluate",
                req(json!({"complexity": 0.5}), &["cheap", "strong"]),
            )
        }),
    });
    Suite::new(name, "adapter", cases)
}

/// Host-side validators: out-of-range, NaN and out-of-options results.
fn host_refusals() -> Result<Value, super::report::Fail> {
    let mut seen = Vec::new();
    for (label, score) in [("above-one", json!(1.5)), ("negative", json!(-0.1))] {
        let r = validate_payload(
            KIND,
            "evaluate",
            Direction::Result,
            &json!({"choice": "a", "scores": {"a": score}, "abstain": false}),
        );
        match r {
            Err(d) if d.code == "SPX-HPA044" => seen.push(json!({label: d.code})),
            other => {
                return Err(fail_with(
                    "host accepted an out-of-range score",
                    json!({"case": label, "got": format!("{other:?}")}),
                ))
            }
        }
    }
    match parse_strict(
        br#"{"choice":"a","scores":{"a":NaN},"abstain":false}"#,
        &JsonLimits::frame(1024),
    ) {
        Err(d) => seen.push(json!({"nan": d.code})),
        Ok(_) => return Err(fail("host accepted NaN in a frame")),
    }
    let outside = check_against_request(
        KIND,
        &req(json!({}), &["a"]),
        &json!({"choice": "z", "scores": {}, "abstain": false}),
    );
    match outside {
        Err(d) if d.code == "SPX-HPA043" => seen.push(json!({"outside-options": d.code})),
        other => {
            return Err(fail_with(
                "host accepted a choice outside the options",
                json!({"got": format!("{other:?}")}),
            ))
        }
    }
    Ok(json!({"refused": seen}))
}
