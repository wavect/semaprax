//! `context.repository/v1` suite: freshness, completeness, source binding.

use super::report::{check, fail, fail_with, Case, Fail, Suite};
use super::suite::{cancellation, completed, usable, Rig, Target};
use crate::contract::CapabilityKind::ContextRepository as KIND;
use crate::json::{canonical, sha256_plain};
use serde_json::{json, Value};
use std::path::Path;

const TOKEN_A: &str = "alpha_conformance_symbol";
const TOKEN_B: &str = "beta_conformance_symbol";
const LIB_A: &str =
    "pub fn alpha_conformance_symbol() -> u32 {\n    41 + 1\n}\n\npub fn helper_conformance() {}\n";
const LIB_B: &str =
    "pub fn beta_conformance_symbol() -> u32 {\n    42 + 1\n}\n\npub fn helper_conformance() {}\n";

type Lookup = (&'static str, fn(&str) -> Value);

/// `(operation, payload builder)` for the best lookup the adapter declares.
fn lookup(t: &Target) -> Option<Lookup> {
    let ops = t.ops(KIND);
    let has = |o: &str| ops.iter().any(|x| x == o);
    if has("references") {
        Some(("references", |s| json!({"symbol": s})))
    } else if has("search") {
        Some(("search", |s| json!({"query": s})))
    } else {
        None
    }
}

fn items(p: &Value) -> Vec<&Value> {
    p["items"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

fn span_lines(project: &Path, item: &Value) -> Result<Vec<String>, Fail> {
    let path = item["path"].as_str().unwrap_or("");
    let text = std::fs::read_to_string(project.join(path)).map_err(|e| {
        fail_with(
            format!("`{path}` is not a readable project file: {e}"),
            json!({"path": path}),
        )
    })?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let (a, b) = (
        item["span"]["start_line"].as_u64().unwrap_or(0) as usize,
        item["span"]["end_line"].as_u64().unwrap_or(0) as usize,
    );
    if a == 0 || b < a || b > lines.len() {
        return Err(fail_with(
            format!(
                "span {a}..{b} lies outside `{path}` ({} lines)",
                lines.len()
            ),
            json!({"path": path}),
        ));
    }
    Ok(lines[a - 1..b].to_vec())
}

pub fn run(t: &Target) -> Suite {
    let name = "context.repository";
    let Some((op, query)) = lookup(t) else {
        return Suite::new(
            name,
            "adapter",
            vec![Case::unverified(
                "declared-operations",
                "descriptor declares neither `search` nor `references`",
            )],
        );
    };
    let mut cases = Vec::new();
    match Rig::open(t, &[("src/lib.rs", LIB_A)]) {
        Err(d) => cases.push(Case::failed("setup", fail(d.to_string()))),
        Ok(mut rig) => {
            cases.push(check("finds-planted-symbol", || {
                let (_, p) = usable(rig.run(KIND, op, query(TOKEN_A)))?;
                let hit = items(&p).into_iter().any(|i| {
                    i["path"] == "src/lib.rs"
                        && i["text"].as_str().is_none_or(|t| t.contains(TOKEN_A))
                });
                if !hit {
                    return Err(fail_with(
                        "planted symbol not found in src/lib.rs",
                        json!({"items": items(&p).len()}),
                    ));
                }
                Ok(json!({"operation": op, "items": items(&p).len()}))
            }));
            cases.push(check("spans-and-digests-match-file-bytes", || {
                let (_, p) = usable(rig.run(KIND, op, query(TOKEN_A)))?;
                if items(&p).is_empty() {
                    return Err(fail("no items to verify"));
                }
                for i in items(&p) {
                    let lines = span_lines(&rig.project, i)?;
                    let want = sha256_plain(lines.join("\n").as_bytes());
                    if i["digest"].as_str() != Some(want.as_str()) {
                        return Err(fail_with(
                            "digest is not sha256 of the span's lines joined with LF",
                            json!({"path": i["path"], "expected": want, "actual": i["digest"]}),
                        ));
                    }
                    if let Some(text) = i["text"].as_str() {
                        if text.trim() != lines.join("\n").trim() {
                            return Err(fail_with(
                                "item text differs from the file's span",
                                json!({"path": i["path"]}),
                            ));
                        }
                    }
                }
                Ok(json!({"items_verified": items(&p).len()}))
            }));
            cases.push(check("paths-relative-and-contained", || {
                let (_, p) = usable(rig.run(KIND, op, query(TOKEN_A)))?;
                let root = rig
                    .project
                    .canonicalize()
                    .map_err(|e| fail(e.to_string()))?;
                for i in items(&p) {
                    let path = i["path"].as_str().unwrap_or("");
                    let inside = Path::new(path).is_relative()
                        && !path.split('/').any(|s| s == ".." || s.is_empty())
                        && root
                            .join(path)
                            .canonicalize()
                            .is_ok_and(|c| c.starts_with(&root));
                    if !inside {
                        return Err(fail_with(
                            "path is not a relative path inside the project",
                            json!({"path": path}),
                        ));
                    }
                }
                Ok(json!({"paths_checked": items(&p).len()}))
            }));
            cases.push(check("deterministic-for-equal-input", || {
                let a = usable(rig.run(KIND, op, query(TOKEN_A)))?.1;
                let b = usable(rig.run(KIND, op, query(TOKEN_A)))?.1;
                if canonical(&a) != canonical(&b) {
                    return Err(fail("two identical requests returned different payloads"));
                }
                Ok(json!({"equal": true}))
            }));
            cases.push(check("freshness-after-edit", || {
                rig.write("src/lib.rs", LIB_B);
                rig.bump();
                let after_new = completed(rig.run(KIND, op, query(TOKEN_B)))?;
                let after_old = completed(rig.run(KIND, op, query(TOKEN_A)))?;
                let mut ev = json!({"status_new": after_new.status.as_str(), "status_old": after_old.status.as_str()});
                if after_new.status.as_str() == "stale" || after_old.status.as_str() == "stale" {
                    ev["mode"] = json!("reported-stale");
                    return Ok(ev);
                }
                let np = after_new.payload.clone().unwrap_or(Value::Null);
                let op_ = after_old.payload.clone().unwrap_or(Value::Null);
                if !items(&np).iter().any(|i| i["path"] == "src/lib.rs") {
                    return Err(fail_with(
                        "project file edited and revision bumped, but the new symbol is not found and the result is not marked stale",
                        ev,
                    ));
                }
                if items(&op_).iter().any(|i| i["text"].as_str().is_some_and(|t| t.contains(TOKEN_A))) {
                    return Err(fail_with("result still serves text that no longer exists in the file", ev));
                }
                ev["mode"] = json!("refreshed");
                Ok(ev)
            }));
        }
    }
    cases.push(coverage_case(t, op, query));
    cases.push(match Rig::open(t, &[("src/lib.rs", LIB_A)]) {
        Err(d) => Case::failed("cancellation-cooperative", fail(d.to_string())),
        Ok(mut rig) => check("cancellation-cooperative", || {
            cancellation(&mut rig, KIND, op, query(TOKEN_A))
        }),
    });
    Suite::new(name, "adapter", cases)
}

/// An unreadable file must keep the result from claiming exhaustive coverage.
fn coverage_case(t: &Target, op: &str, query: fn(&str) -> Value) -> Case {
    let name = "exhaustive-flag-honest-with-skipped-files";
    let mut rig = match Rig::open(
        t,
        &[
            ("src/lib.rs", LIB_A),
            ("src/locked.rs", "pub fn locked() {}\n"),
        ],
    ) {
        Ok(r) => r,
        Err(d) => return Case::failed(name, fail(d.to_string())),
    };
    let locked = rig.project.join("src/locked.rs");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000));
    }
    if std::fs::File::open(&locked).is_ok() {
        return Case::unverified(
            name,
            "could not make a file unreadable here (running as a privileged user?)",
        );
    }
    check(name, || {
        let (r, p) = usable(rig.run(KIND, op, query(TOKEN_A)))?;
        let c = &p["coverage"];
        let (complete, exhaustive) = (
            c["complete"].as_bool().unwrap_or(true),
            c["exhaustive"].as_bool().unwrap_or(true),
        );
        let skipped: Vec<&str> = c["skipped"]
            .as_array()
            .map(|a| a.iter().filter_map(|s| s["path"].as_str()).collect())
            .unwrap_or_default();
        let ev = json!({"complete": complete, "exhaustive": exhaustive, "skipped": skipped, "status": r.status.as_str()});
        if exhaustive {
            return Err(fail_with(
                "coverage claims exhaustive although src/locked.rs could not be read",
                ev,
            ));
        }
        if complete || r.status.as_str() == "complete" {
            return Err(fail_with(
                "coverage or status claims complete although src/locked.rs could not be read",
                ev,
            ));
        }
        if p["no_references"].as_bool() == Some(true) {
            return Err(fail_with(
                "`no_references` asserted on a non-exhaustive result",
                ev,
            ));
        }
        Ok(ev)
    })
}
