//! Maintainer automation: propose catalog revision bumps with conformance
//! results. Read-only for the user's state: candidates are staged into a
//! throwaway store, validated by the same pipeline as a user update, reported
//! as JSON and deleted. It never edits the catalog and never merges anything;
//! a maintainer reviews the proposal and updates `catalog.json` by hand.

use super::fetch::Fetcher;
use super::ops::{catalog_source, Ctx};
use super::resolve::{resolve, Channel};
use super::stage;
use crate::diag::HarnessResult;
use serde_json::{json, Value};

pub fn propose(ctx: &Ctx, f: &dyn Fetcher, ids: &[String]) -> HarnessResult<Value> {
    let root = ctx.home.join("updates").join("propose");
    let _ = std::fs::remove_dir_all(&root);
    let store = root.join("store");
    let mut out = Vec::new();
    for k in ctx
        .catalog
        .embedded_skills()
        .filter(|k| ids.is_empty() || ids.contains(&k.id))
    {
        let src = catalog_source(ctx.catalog, k, &store, &root.join("scratch-base"))?;
        let entry = (|| -> HarnessResult<Value> {
            let ch = Channel::parse(&src.channel)?;
            let res = resolve(f, &src.repo, &ch, &src.head_branch)?;
            let current = json!({"version": k.version, "tag": k.tag, "commit": k.commit});
            if k.commit.as_deref() == Some(res.commit.as_str()) {
                return Ok(json!({"id": k.id, "status": "current", "current": current}));
            }
            match stage::stage(f, &src, &res, &store, &root.join("scratch"), ctx.gate) {
                Ok(s) => Ok(json!({
                    "id": k.id, "status": "proposed", "current": current,
                    "proposed": {
                        "version": s.rev.version, "tag": s.rev.tag, "commit": s.rev.commit,
                        "bundle_digest": s.rev.digest, "license": s.rev.license,
                        "files": s.rev.files.iter().map(|f| json!({
                            "path": f.path, "upstream_path": f.upstream_path,
                            "git_blob_sha": f.git_blob_sha, "sha256": f.sha256.trim_start_matches("sha256:"),
                            "bytes": f.bytes})).collect::<Vec<_>>(),
                    },
                    "diff": {"added": s.diff.added, "removed": s.diff.removed, "modified": s.diff.modified},
                    "review_reasons": s.reasons,
                    "conformance": {"extraction": "pass", "profile_parse": "pass", "compatibility_smoke": "pass"},
                })),
                Err(e) => Ok(json!({
                    "id": k.id, "status": "failed-conformance", "current": current,
                    "candidate": {"version": res.version, "commit": res.commit},
                    "conformance": {"code": e.code, "message": super::bounded(&e.message)},
                })),
            }
        })()
        .unwrap_or_else(|e| json!({"id": k.id, "status": "unresolved", "error": {"code": e.code, "message": super::bounded(&e.message)}}));
        out.push(entry);
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(json!({
        "schema": "semaprax.catalog-proposal.v1",
        "note": "proposal only: review and edit catalog.json manually; nothing was merged or installed",
        "proposals": out,
    }))
}
