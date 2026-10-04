//! `skill.catalog/v1` suite: bounded listing and exact-digest loading.

use super::report::{check, fail, fail_with, Case, Suite};
use super::suite::{cancellation, completed, usable, Rig, Target};
use crate::contract::CapabilityKind::SkillCatalog as KIND;
use crate::json::sha256_plain;
use serde_json::{json, Value};

pub fn run(t: &Target) -> Suite {
    let name = "skill.catalog";
    let ops = t.ops(KIND);
    if !(ops.iter().any(|o| o == "list") && ops.iter().any(|o| o == "load")) {
        return Suite::new(
            name,
            "adapter",
            vec![Case::unverified(
                "declared-operations",
                "descriptor declares no `list` and `load` operations",
            )],
        );
    }
    let mut cases = Vec::new();
    match Rig::open(t, &[]) {
        Err(d) => cases.push(Case::failed("setup", fail(d.to_string()))),
        Ok(mut rig) => {
            let list = |rig: &mut Rig, limit: u64| {
                usable(rig.run(KIND, "list", json!({"limit": limit}))).map(|x| x.1)
            };
            cases.push(check("list-is-bounded", || {
                let full = list(&mut rig, 256)?;
                let one = list(&mut rig, 1)?;
                let total = full["skills"].as_array().map_or(0, Vec::len);
                let got = one["skills"].as_array().map_or(0, Vec::len);
                if got > 1 {
                    return Err(fail_with(
                        "limit 1 returned more than one entry",
                        json!({"returned": got}),
                    ));
                }
                if total > 1 && one["truncated"] != true {
                    return Err(fail_with(
                        "a truncated listing is not marked truncated",
                        json!({"total": total}),
                    ));
                }
                Ok(json!({"total": total, "limit_1_returned": got}))
            }));
            cases.push(check("load-by-exact-digest", || {
                let full = list(&mut rig, 256)?;
                let first = full["skills"]
                    .as_array()
                    .and_then(|a| a.first())
                    .cloned()
                    .ok_or_else(|| fail("catalog is empty; nothing to load"))?;
                let digest = first["digest"].as_str().unwrap_or("").to_string();
                let (_, p) = usable(rig.run(KIND, "load", json!({"digest": digest})))?;
                let text = p["text"]
                    .as_str()
                    .ok_or_else(|| fail("loaded skill carries no inline text to verify"))?;
                if sha256_plain(text.as_bytes()) != digest {
                    return Err(fail_with(
                        "loaded text does not hash to the requested digest",
                        json!({"digest": digest}),
                    ));
                }
                if first["bytes"].as_u64() != Some(text.len() as u64) {
                    return Err(fail_with(
                        "listed byte count differs from the loaded text",
                        json!({"listed": first["bytes"], "actual": text.len()}),
                    ));
                }
                Ok(json!({"digest": digest, "bytes": text.len()}))
            }));
            cases.push(check("unknown-digest-is-not-served", || {
                let unknown = format!("sha256:{}", "0".repeat(64));
                let r = completed(rig.run(KIND, "load", json!({"digest": unknown})))?;
                let served = r
                    .payload
                    .as_ref()
                    .is_some_and(|p: &Value| p["text"].is_string());
                if served || r.status.as_str() == "complete" {
                    return Err(fail("adapter served content for a digest it does not hold"));
                }
                Ok(json!({"status": r.status.as_str()}))
            }));
        }
    }
    cases.push(match Rig::open(t, &[]) {
        Err(d) => Case::failed("cancellation-cooperative", fail(d.to_string())),
        Ok(mut rig) => check("cancellation-cooperative", || {
            cancellation(&mut rig, KIND, "list", json!({"limit": 4}))
        }),
    });
    Suite::new(name, "adapter", cases)
}
