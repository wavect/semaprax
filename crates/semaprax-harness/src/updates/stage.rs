//! Staging pipeline: fetch an exact commit's files, verify every byte, extract
//! into the content-addressed store, validate (HN-03 skill profile or adapter
//! descriptor), diff against the active revision and derive review reasons.
//! A failure at any step leaves the active revision and the store addressable
//! set unchanged; nothing from upstream is executed.

use super::d;
use super::fetch::{Fetcher, TreeEntry};
use super::resolve::Resolved;
use super::sha1::git_blob_sha;
use super::state::{FileRec, Kind, Revision, Source};
use crate::contract::descriptor::Descriptor;
use crate::diag::HarnessResult;
use crate::skills::inventory::{normalized_rel_ok, Bounds, ScanRules};
use crate::skills::snapshot::{self, Snapshot};
use crate::skills::{agentskills, bundle};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const DESCRIPTOR_FILE: &str = "harness-provider.json";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct Staged {
    pub snapshot: Snapshot,
    /// `activated_at` is 0 until activation.
    pub rev: Revision,
    pub diff: Diff,
    /// Why this revision needs explicit review (empty: content-only).
    pub reasons: Vec<String>,
}

/// Extra compatibility gate (behavior smoke supplied by the caller).
pub type Gate<'a> = &'a dyn Fn(&Staged) -> Result<(), String>;

struct Selected {
    rel: String,
    entry: TreeEntry,
}

fn escape(rel: &str) -> bool {
    !normalized_rel_ok(rel) || rel.split('/').any(|s| s == ".." || s == ".")
}

fn select(src: &Source, entries: &[TreeEntry]) -> HarnessResult<Vec<Selected>> {
    let unsafe_entry =
        |p: &str, why: &str| d("SPX-HPU006", format!("upstream entry `{p}` refused: {why}"));
    let mut out = Vec::new();
    if !src.files.is_empty() {
        let by: BTreeMap<&str, &TreeEntry> = entries.iter().map(|e| (e.path.as_str(), e)).collect();
        for (rel, up) in &src.files {
            if escape(rel) || escape(up) {
                return Err(unsafe_entry(up, "path escapes the artifact"));
            }
            let e = by.get(up.as_str()).ok_or_else(|| {
                d(
                    "SPX-HPU007",
                    format!("recorded file `{up}` is missing upstream"),
                )
            })?;
            if !e.is_blob || e.mode == "120000" {
                return Err(unsafe_entry(up, "not a regular file"));
            }
            out.push(Selected {
                rel: rel.clone(),
                entry: (*e).clone(),
            });
        }
        return Ok(out);
    }
    let prefix = src
        .subpath
        .as_deref()
        .map(|p| format!("{}/", p.trim_end_matches('/')))
        .unwrap_or_default();
    for e in entries {
        let Some(rel) = e.path.strip_prefix(prefix.as_str()) else {
            continue;
        };
        if e.mode == "040000" || (!e.is_blob && e.mode != "160000" && e.mode != "120000") {
            continue; // directory object; its blobs are listed individually
        }
        if escape(rel) {
            return Err(unsafe_entry(&e.path, "path escapes the artifact"));
        }
        if !e.is_blob || e.mode == "120000" || e.mode == "160000" {
            return Err(unsafe_entry(
                &e.path,
                "symlinks and submodules are not admitted",
            ));
        }
        out.push(Selected {
            rel: rel.to_string(),
            entry: e.clone(),
        });
    }
    if out.is_empty() {
        return Err(d(
            "SPX-HPU007",
            "upstream subpath holds no files at this commit",
        ));
    }
    Ok(out)
}

fn bounds(kind: Kind) -> Bounds {
    match kind {
        Kind::Skill => Bounds::SKILL,
        Kind::Adapter => Bounds::ADAPTER,
    }
}

pub fn stage(
    f: &dyn Fetcher,
    src: &Source,
    res: &Resolved,
    store: &Path,
    scratch: &Path,
    gate: Option<Gate>,
) -> HarnessResult<Staged> {
    let tree = f.tree(&src.repo, &res.commit)?;
    if tree.truncated {
        return Err(d("SPX-HPU005", "upstream tree listing is truncated"));
    }
    let selected = select(src, &tree.entries)?;
    let b = bounds(src.kind);
    if selected.len() > b.max_files {
        return Err(d("SPX-HPU006", "too many files for one artifact"));
    }
    let _ = std::fs::remove_dir_all(scratch);
    let io = |e: std::io::Error| d("SPX-HPU009", format!("cannot stage: {e}"));
    std::fs::create_dir_all(scratch).map_err(io)?;
    let result = (|| {
        let mut upstream: BTreeMap<String, (String, String)> = BTreeMap::new();
        let mut total = 0u64;
        for s in &selected {
            let e = &s.entry;
            if e.size > b.max_file_bytes {
                return Err(d(
                    "SPX-HPU005",
                    format!("`{}` exceeds the per-file bound", s.rel),
                ));
            }
            total += e.size;
            if total > b.max_total_bytes {
                return Err(d("SPX-HPU005", "artifact exceeds the total size bound"));
            }
            let bytes = f.blob(&src.repo, &e.sha, e.size)?;
            if bytes.len() as u64 != e.size {
                return Err(d(
                    "SPX-HPU005",
                    format!(
                        "`{}` is truncated: {} of {} bytes",
                        s.rel,
                        bytes.len(),
                        e.size
                    ),
                ));
            }
            if git_blob_sha(&bytes) != e.sha {
                return Err(d(
                    "SPX-HPU005",
                    format!("`{}` does not hash to its git blob {}", s.rel, e.sha),
                ));
            }
            let to = scratch.join(&s.rel);
            if let Some(p) = to.parent() {
                std::fs::create_dir_all(p).map_err(io)?;
            }
            std::fs::write(&to, &bytes).map_err(io)?;
            upstream.insert(s.rel.clone(), (e.path.clone(), e.sha.clone()));
        }
        let rules = match src.kind {
            Kind::Skill => ScanRules::skill(),
            Kind::Adapter => ScanRules::adapter(Vec::new()),
        };
        let snap = snapshot::publish(store, scratch, &rules, &b)?;
        Ok((snap, upstream))
    })();
    let _ = std::fs::remove_dir_all(scratch);
    let (snap, upstream) = result?;
    let rev = describe(src, res, &snap, &upstream)?;
    let diff = diff_of(src.active.as_ref(), &rev);
    let reasons = review_reasons(src, &rev, &diff);
    let staged = Staged {
        snapshot: snap,
        rev,
        diff,
        reasons,
    };
    smoke(src, &staged)?;
    if let Some(g) = gate {
        g(&staged).map_err(|m| d("SPX-HPU007", format!("compatibility gate failed: {m}")))?;
    }
    Ok(staged)
}

/// Build the revision record of an extracted snapshot, validating its format.
/// `upstream` maps relative path -> (upstream path, git blob sha).
pub fn describe(
    src: &Source,
    res: &Resolved,
    snap: &Snapshot,
    upstream: &BTreeMap<String, (String, String)>,
) -> HarnessResult<Revision> {
    let (identity, license, requested) = match src.kind {
        Kind::Skill => {
            let text = std::fs::read_to_string(snap.files_dir.join("SKILL.md"))
                .map_err(|e| d("SPX-HPU007", format!("SKILL.md unreadable: {e}")))?;
            let p = agentskills::parse(&src.id, &text)?;
            let want = src
                .active
                .as_ref()
                .map(|a| a.identity.as_str())
                .unwrap_or(&src.id);
            if p.name != want {
                return Err(d(
                    "SPX-HPU004",
                    format!("skill identity changed: `{}` is now `{}`", want, p.name),
                ));
            }
            let mut req = p.requested.clone();
            req.sort();
            req.dedup();
            (p.name, p.license, req)
        }
        Kind::Adapter => {
            let bytes = std::fs::read(snap.files_dir.join(DESCRIPTOR_FILE))
                .map_err(|e| d("SPX-HPU007", format!("{DESCRIPTOR_FILE} unreadable: {e}")))?;
            let desc = Descriptor::parse(&bytes)?;
            let want = src.active.as_ref().map(|a| a.identity.as_str());
            if want.is_some_and(|w| w != desc.provider_id) {
                return Err(d(
                    "SPX-HPU004",
                    format!(
                        "adapter identity changed: `{}` is now `{}`",
                        want.unwrap(),
                        desc.provider_id
                    ),
                ));
            }
            let mut req = BTreeSet::new();
            let pm = &desc.permissions;
            for (k, v) in [
                ("read", &pm.read),
                ("write", &pm.write),
                ("network", &pm.network),
                ("process", &pm.process),
                ("secrets", &pm.secrets),
            ] {
                req.extend(v.iter().map(|x| format!("{k}:{x}")));
            }
            if let Some(u) = &desc.upstream {
                req.insert(format!("meta:upstream={}", u.repository));
            }
            (
                desc.provider_id,
                Some(desc.support.license),
                req.into_iter().collect(),
            )
        }
    };
    let license_sha256 = snap
        .inventory
        .entries()
        .iter()
        .find(|e| matches!(e.path.as_str(), "LICENSE" | "LICENSE.md" | "LICENSE.txt"))
        .map(|e| e.sha256.clone());
    let files = snap
        .inventory
        .entries()
        .iter()
        .map(|e| {
            let (up, blob) = upstream.get(&e.path).cloned().unwrap_or_default();
            FileRec {
                path: e.path.clone(),
                upstream_path: up,
                git_blob_sha: blob,
                sha256: e.sha256.clone(),
                bytes: e.bytes,
                kind: e.kind.as_str().to_string(),
            }
        })
        .collect();
    Ok(Revision {
        commit: res.commit.clone(),
        tag: res.tag.clone(),
        version: res.version.clone(),
        digest: snap.digest.clone(),
        repo: src.repo.clone(),
        license,
        license_sha256,
        identity,
        requested,
        files,
        activated_at: 0,
    })
}

fn smoke(src: &Source, st: &Staged) -> HarnessResult<()> {
    let fail = |m: String| d("SPX-HPU007", format!("compatibility smoke failed: {m}"));
    match src.kind {
        Kind::Skill => match bundle::read_bundle(&st.snapshot.files_dir) {
            Ok(Some(b)) if b.digest == st.snapshot.digest => Ok(()),
            Ok(Some(_)) => Err(fail("bundle digest differs from the snapshot".into())),
            Ok(None) => Err(fail("no skill bundle in the artifact".into())),
            Err(e) => Err(fail(e.message)),
        },
        Kind::Adapter => {
            let bytes = std::fs::read(st.snapshot.files_dir.join(DESCRIPTOR_FILE))
                .map_err(|e| fail(e.to_string()))?;
            let desc = Descriptor::parse(&bytes)?;
            let entry = desc.entry.first().cloned().unwrap_or_default();
            if st.snapshot.inventory.get(&entry).is_none() {
                return Err(fail(format!(
                    "adapter entry `{entry}` is not in the closure"
                )));
            }
            Ok(())
        }
    }
}

pub fn diff_of(old: Option<&Revision>, new: &Revision) -> Diff {
    let o: BTreeMap<&str, &str> = old
        .map(|r| {
            r.files
                .iter()
                .map(|f| (f.path.as_str(), f.sha256.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let n: BTreeMap<&str, &str> = new
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.sha256.as_str()))
        .collect();
    let mut d = Diff::default();
    for (p, h) in &n {
        match o.get(p) {
            None => d.added.push(p.to_string()),
            Some(x) if x != h => d.modified.push(p.to_string()),
            _ => {}
        }
    }
    d.removed = o
        .keys()
        .filter(|p| !n.contains_key(*p))
        .map(|p| p.to_string())
        .collect();
    d
}

pub fn review_reasons(src: &Source, new: &Revision, diff: &Diff) -> Vec<String> {
    let Some(old) = &src.active else {
        return vec!["initial-install".into()];
    };
    let mut r = BTreeSet::new();
    let meta = |rev: &Revision| {
        rev.requested
            .iter()
            .find(|x| x.starts_with("meta:"))
            .cloned()
    };
    if old.repo != new.repo || meta(old) != meta(new) {
        r.insert("changed-publisher".to_string());
    }
    if old.license != new.license || old.license_sha256 != new.license_sha256 {
        r.insert("changed-license".into());
    }
    for x in &new.requested {
        if !x.starts_with("meta:") && !old.requested.contains(x) {
            r.insert(format!("widened-permissions:{x}"));
        }
    }
    match src.kind {
        Kind::Skill => {
            let exec = |p: &String| {
                new.files
                    .iter()
                    .any(|f| &f.path == p && f.kind == "executable-script")
            };
            for p in &diff.added {
                if exec(p) {
                    r.insert(format!("new-executable:{p}"));
                }
            }
            for p in &diff.modified {
                if exec(p) {
                    r.insert(format!("changed-executable:{p}"));
                }
            }
        }
        Kind::Adapter => {
            if !diff.is_empty() {
                r.insert("adapter-code-changed".into());
            }
        }
    }
    r.into_iter().collect()
}

/// Digest-only check that a stored snapshot still validates (used offline).
pub fn verify_stored(store: &Path, digest: &str) -> HarnessResult<Snapshot> {
    snapshot::open(store, digest)
}
