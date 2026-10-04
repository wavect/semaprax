//! Update operations: check, apply, rollback, revoke, status, maintenance.
//!
//! Network access happens only inside [`check`] (and only through the injected
//! fetcher, only when `offline` is false). Activation is a single atomic state
//! write; sessions that already pinned a revision keep loading it from the
//! immutable snapshot store, and only a session started afterwards sees the
//! new one ([`session_pin`], [`effective_set`]).

use super::bounded;
use super::d;
use super::fetch::Fetcher;
use super::resolve::{resolve, Channel};
use super::stage::{self, Gate};
use super::state::{
    store_dir, FileRec, Kind, Pending, Rejected, Revision, Source, State, KEEP_PREVIOUS,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::skills::official::{self, OfficialFile, OfficialSet};
use crate::skills::snapshot;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Ctx<'a> {
    pub home: &'a Path,
    /// `None` makes every network-needing operation behave as offline.
    pub fetcher: Option<&'a dyn Fetcher>,
    /// Injected clock (seconds); never read from the system here.
    pub now: u64,
    /// No network at all; staged candidates and cached state still work.
    pub offline: bool,
    /// Locked mode: also refuses any change of the active revisions.
    pub frozen: bool,
    pub gate: Option<Gate<'a>>,
    pub catalog: &'a OfficialSet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceReport {
    pub id: String,
    /// up-to-date | activated | pending | rejected | unavailable | cached | error
    pub state: &'static str,
    pub active: Option<String>,
    pub active_commit: Option<String>,
    pub candidate: Option<String>,
    pub candidate_commit: Option<String>,
    pub reasons: Vec<String>,
    pub message: Option<String>,
    pub diff: Option<stage::Diff>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub sources: Vec<SourceReport>,
    pub notice: Option<String>,
    /// True when no upstream request was made.
    pub offline: bool,
}

impl Report {
    pub fn to_json(&self) -> Value {
        json!({
            "schema": "semaprax.updates-report.v1",
            "offline": self.offline, "notice": self.notice,
            "sources": self.sources.iter().map(|s| json!({
                "id": s.id, "state": s.state, "active": s.active, "active_commit": s.active_commit,
                "candidate": s.candidate, "candidate_commit": s.candidate_commit,
                "reasons": s.reasons, "message": s.message,
                "diff": s.diff.as_ref().map(|d| json!({"added": d.added, "removed": d.removed, "modified": d.modified})),
            })).collect::<Vec<_>>(),
        })
    }

    pub fn render(&self) -> String {
        let mut o = String::new();
        for s in &self.sources {
            o.push_str(&format!(
                "{} {} active={} candidate={} reasons=[{}]{}\n",
                s.id,
                s.state,
                s.active.as_deref().unwrap_or("-"),
                s.candidate.as_deref().unwrap_or("-"),
                s.reasons.join(","),
                s.message
                    .as_ref()
                    .map(|m| format!(" ({m})"))
                    .unwrap_or_default()
            ));
        }
        if let Some(n) = &self.notice {
            o.push_str(&format!("notice: {n}\n"));
        }
        o
    }
}

fn scratch(home: &Path, id: &str) -> PathBuf {
    home.join("updates").join("scratch").join(id)
}

/// Source record of one embedded catalog skill, active at its recorded revision.
pub fn catalog_source(
    set: &OfficialSet,
    k: &official::OfficialSkill,
    store: &Path,
    scratch: &Path,
) -> HarnessResult<Source> {
    let snap = set.materialize(&k.id, store, scratch)?;
    let mut src = Source {
        id: k.id.clone(),
        kind: Kind::Skill,
        repo: k.repo.clone(),
        subpath: k.subpath.clone(),
        channel: if k.channel.is_empty() {
            "latest-stable".into()
        } else {
            k.channel.clone()
        },
        head_branch: k.head_branch.clone(),
        files: k
            .files
            .iter()
            .map(|f| (f.path.clone(), f.upstream_path.clone()))
            .collect(),
        active: None,
        previous: vec![],
        pending: None,
        rejected: None,
        held: None,
        revoked: vec![],
        unavailable: false,
        resolved_head: None,
    };
    let res = super::resolve::Resolved {
        commit: k.commit.clone().unwrap_or_default(),
        tag: k.tag.clone(),
        version: k.version.clone(),
        head_branch: None,
    };
    let upstream: BTreeMap<String, (String, String)> = k
        .files
        .iter()
        .map(|f| {
            (
                f.path.clone(),
                (f.upstream_path.clone(), f.git_blob_sha.clone()),
            )
        })
        .collect();
    src.active = Some(stage::describe(&src, &res, &snap, &upstream)?);
    Ok(src)
}

/// Register catalog skills that carry an embedded, verified revision.
pub fn seed(ctx: &Ctx, st: &mut State) -> HarnessResult<()> {
    for k in ctx.catalog.embedded_skills() {
        if st.sources.contains_key(&k.id) || k.bundle_digest.is_none() || k.commit.is_none() {
            continue;
        }
        let src = catalog_source(
            ctx.catalog,
            k,
            &store_dir(ctx.home),
            &scratch(ctx.home, &k.id),
        )?;
        st.sources.insert(k.id.clone(), src);
    }
    Ok(())
}

fn load(ctx: &Ctx) -> HarnessResult<State> {
    let mut st = State::load(ctx.home)?;
    seed(ctx, &mut st)?;
    Ok(st)
}

fn rep(src: &Source, state: &'static str) -> SourceReport {
    SourceReport {
        id: src.id.clone(),
        state,
        active: src.active.as_ref().map(|a| a.version.clone()),
        active_commit: src.active.as_ref().map(|a| a.commit.clone()),
        candidate: None,
        candidate_commit: None,
        reasons: vec![],
        message: None,
        diff: None,
    }
}

fn is_revoked(src: &Source, r: &Revision) -> bool {
    src.revoked.iter().any(|x| *x == r.commit || *x == r.digest)
}

/// Replace a revoked active revision by the newest safe previous one, or mark
/// the source unavailable. Revoked revisions are never reactivated.
fn apply_revocations(src: &mut Source, now: u64) {
    if src
        .pending
        .as_ref()
        .is_some_and(|p| src.revoked.contains(&p.rev.commit) || src.revoked.contains(&p.rev.digest))
    {
        src.pending = None;
    }
    let Some(active) = src.active.clone() else {
        return;
    };
    if !is_revoked(src, &active) {
        return;
    }
    let pos = src.previous.iter().position(|p| !is_revoked(src, p));
    match pos {
        Some(i) => {
            let mut safe = src.previous.remove(i);
            safe.activated_at = now;
            src.held = Some(active.commit);
            src.active = Some(safe);
            src.unavailable = false;
        }
        None => src.unavailable = true,
    }
}

fn reject(src: &mut Source, commit: &str, version: &str, e: &HarnessDiagnostic) -> SourceReport {
    src.rejected = Some(Rejected {
        commit: commit.into(),
        version: version.into(),
        code: e.code.into(),
        message: bounded(&e.message),
    });
    let mut r = rep(src, "rejected");
    r.candidate = Some(version.into());
    r.candidate_commit = Some(commit.into());
    r.message = Some(format!("{}: {}", e.code, bounded(&e.message)));
    r
}

fn activate(src: &mut Source, mut rev: Revision, now: u64) {
    rev.activated_at = now;
    if let Some(old) = src.active.take() {
        if !src.previous.iter().any(|p| p.commit == old.commit) && old.commit != rev.commit {
            src.previous.insert(0, old);
        }
    }
    src.previous.truncate(KEEP_PREVIOUS);
    src.active = Some(rev);
    src.pending = None;
    src.rejected = None;
    src.held = None;
    src.unavailable = false;
}

fn check_source(
    ctx: &Ctx,
    policy: &super::state::Policy,
    src: &mut Source,
    f: &dyn Fetcher,
) -> HarnessResult<SourceReport> {
    for r in f.revoked(&src.repo)? {
        if !src.revoked.contains(&r) {
            src.revoked.push(r);
        }
    }
    apply_revocations(src, ctx.now);
    // A recorded tag must still point where it did (moved tag = refusal).
    let recorded: Vec<(String, String)> = src
        .active
        .iter()
        .chain(src.pending.iter().map(|p| &p.rev))
        .filter_map(|r| r.tag.clone().map(|t| (t, r.commit.clone())))
        .collect();
    for (tag, commit) in recorded {
        if let Some(now) = super::resolve::tag_commit(f, &src.repo, &tag)? {
            if now != commit {
                let e = d(
                    "SPX-HPU003",
                    format!("tag `{tag}` moved to a different commit"),
                );
                src.pending = None;
                return Ok(reject(src, &now, &tag, &e));
            }
        }
    }
    let channel = Channel::parse(&src.channel)?;
    let res = resolve(f, &src.repo, &channel, &src.head_branch)?;
    if let Some(b) = &res.head_branch {
        src.resolved_head = Some((b.clone(), res.commit.clone()));
    }
    if src.revoked.contains(&res.commit) {
        let e = d("SPX-HPU011", "the resolved revision is revoked upstream");
        return Ok(reject(src, &res.commit, &res.version, &e));
    }
    if src.active.as_ref().is_some_and(|a| a.commit == res.commit) && !src.unavailable {
        src.pending = None;
        src.rejected = None;
        return Ok(rep(src, "up-to-date"));
    }
    if src.held.as_ref().is_some_and(|h| *h != res.commit) {
        src.held = None;
    }
    let store = store_dir(ctx.home);
    let reuse = src
        .pending
        .as_ref()
        .filter(|p| p.rev.commit == res.commit && snapshot::open(&store, &p.rev.digest).is_ok())
        .cloned();
    let (rev, mut reasons, diff) = match reuse {
        Some(p) => {
            let diff = stage::diff_of(src.active.as_ref(), &p.rev);
            (p.rev, p.reasons, diff)
        }
        None => match stage::stage(f, src, &res, &store, &scratch(ctx.home, &src.id), ctx.gate) {
            Ok(s) => (s.rev, s.reasons, s.diff),
            Err(e) if e.code == "SPX-HPU009" => return Err(e),
            Err(e) => return Ok(reject(src, &res.commit, &res.version, &e)),
        },
    };
    if src.held.as_ref() == Some(&res.commit) {
        reasons.push("rolled-back-hold".into());
    }
    if src.unavailable {
        reasons.push("recovering-from-revoked".into());
    }
    if reasons.is_empty() && !(policy.approved && policy.auto_content && src.kind == Kind::Skill) {
        reasons.push("auto-update-not-approved".into());
    }
    reasons.sort();
    reasons.dedup();
    let (cand, commit) = (rev.version.clone(), rev.commit.clone());
    let mut r;
    if reasons.is_empty() {
        activate(src, rev, ctx.now);
        r = rep(src, "activated");
    } else {
        src.rejected = None;
        src.pending = Some(Pending {
            rev,
            reasons: reasons.clone(),
        });
        r = rep(src, "pending");
    }
    r.candidate = Some(cand);
    r.candidate_commit = Some(commit);
    r.reasons = reasons;
    r.diff = Some(diff);
    Ok(r)
}

fn cached_report(st: &State, ids: &[String], state: &'static str) -> Vec<SourceReport> {
    st.sources
        .values()
        .filter(|s| ids.is_empty() || ids.contains(&s.id))
        .map(|s| {
            let mut r = rep(s, if s.unavailable { "unavailable" } else { state });
            if let Some(p) = &s.pending {
                r.state = if s.unavailable {
                    "unavailable"
                } else {
                    "pending"
                };
                r.candidate = Some(p.rev.version.clone());
                r.candidate_commit = Some(p.rev.commit.clone());
                r.reasons = p.reasons.clone();
            }
            if let Some(j) = &s.rejected {
                r.message = Some(format!("{}: {}", j.code, j.message));
                r.candidate = Some(j.version.clone());
                r.candidate_commit = Some(j.commit.clone());
                if s.pending.is_none() && !s.unavailable {
                    r.state = "rejected";
                }
            }
            r
        })
        .collect()
}

fn select_ids(st: &State, ids: &[String]) -> HarnessResult<()> {
    for i in ids {
        if !st.sources.contains_key(i) {
            return Err(d(
                "SPX-HPU001",
                format!("`{i}` is not a registered update source"),
            ));
        }
    }
    Ok(())
}

/// Explicit `updates check`: resolve, stage, validate and (policy permitting)
/// activate. Offline/frozen makes zero requests and reports cached state. A
/// network failure becomes a bounded notice, never an error.
pub fn check(ctx: &Ctx, ids: &[String]) -> HarnessResult<Report> {
    let mut st = load(ctx)?;
    select_ids(&st, ids)?;
    let Some(f) = ctx.fetcher.filter(|_| !ctx.offline) else {
        return Ok(Report {
            sources: cached_report(&st, ids, "cached"),
            notice: st.notice.clone(),
            offline: true,
        });
    };
    let policy = st.policy.clone();
    let mut out = Vec::new();
    let mut notice = None;
    let keys: Vec<String> = st
        .sources
        .keys()
        .filter(|k| ids.is_empty() || ids.contains(k))
        .cloned()
        .collect();
    for k in keys {
        let src = st.sources.get_mut(&k).unwrap();
        match check_source(ctx, &policy, src, f) {
            Ok(r) => out.push(r),
            Err(e) => {
                let msg = format!("{}: {}", e.code, bounded(&e.message));
                let mut r = rep(src, "error");
                r.message = Some(msg.clone());
                out.push(r);
                notice = Some(format!(
                    "update check failed, using cached revisions ({msg})"
                ));
                if e.code == "SPX-HPU009" {
                    break; // outage: do not hammer the remaining sources
                }
            }
        }
    }
    st.last_check = ctx.now;
    st.notice = notice.as_deref().map(bounded);
    st.save(ctx.home)?;
    Ok(Report {
        sources: out,
        notice: st.notice.clone(),
        offline: false,
    })
}

/// Session-start maintenance hook: runs [`check`] only when the user approved
/// routine checks and the TTL has expired. It cannot fail the session: any
/// error becomes a bounded notice.
pub fn maintenance(ctx: &Ctx) -> Report {
    let quiet = |n: Option<String>| Report {
        sources: vec![],
        notice: n,
        offline: true,
    };
    let st = match State::load(ctx.home) {
        Ok(s) => s,
        Err(e) => return quiet(Some(bounded(&format!("{}: {}", e.code, e.message)))),
    };
    if !st.policy.approved || ctx.offline || ctx.fetcher.is_none() {
        return quiet(st.notice);
    }
    if ctx.now < st.last_check.saturating_add(st.policy.ttl_secs) {
        return quiet(st.notice);
    }
    match check(ctx, &[]) {
        Ok(r) => r,
        Err(e) => quiet(Some(bounded(&format!("{}: {}", e.code, e.message)))),
    }
}

/// Activate the staged candidate. Needs no network. Review reasons require
/// `approve`; nothing is applied under `--frozen`.
pub fn apply(ctx: &Ctx, id: &str, approve: bool) -> HarnessResult<Report> {
    if ctx.frozen {
        return Err(d(
            "SPX-HPU010",
            "--frozen forbids changing the active revisions",
        ));
    }
    let mut st = load(ctx)?;
    select_ids(&st, &[id.to_string()])?;
    let src = st.sources.get_mut(id).unwrap();
    let p = src.pending.clone().ok_or_else(|| {
        d(
            "SPX-HPU001",
            format!("`{id}` has no staged update; run `updates check` first"),
        )
    })?;
    if src.revoked.contains(&p.rev.commit) || src.revoked.contains(&p.rev.digest) {
        return Err(d("SPX-HPU011", "the staged revision is revoked"));
    }
    let blocking: Vec<&String> = p
        .reasons
        .iter()
        .filter(|r| *r != "auto-update-not-approved")
        .collect();
    if !blocking.is_empty() && !approve {
        return Err(d(
            "SPX-HPU008",
            format!(
                "`{id}` {} needs explicit review ({}); re-run with --approve",
                p.rev.version,
                p.reasons.join(", ")
            ),
        ));
    }
    snapshot::open(&store_dir(ctx.home), &p.rev.digest)?;
    activate(src, p.rev, ctx.now);
    let r = rep(src, "activated");
    st.save(ctx.home)?;
    Ok(Report {
        sources: vec![r],
        notice: st.notice.clone(),
        offline: ctx.offline,
    })
}

/// Return to the newest retained non-revoked revision. User settings and
/// local skills are not part of this state and stay untouched.
pub fn rollback(ctx: &Ctx, id: &str) -> HarnessResult<Report> {
    if ctx.frozen {
        return Err(d(
            "SPX-HPU010",
            "--frozen forbids changing the active revisions",
        ));
    }
    let mut st = load(ctx)?;
    select_ids(&st, &[id.to_string()])?;
    let src = st.sources.get_mut(id).unwrap();
    let store = store_dir(ctx.home);
    let pos = src
        .previous
        .iter()
        .position(|p| !is_revoked(src, p) && snapshot::open(&store, &p.digest).is_ok())
        .ok_or_else(|| {
            d(
                "SPX-HPU014",
                format!("`{id}` has no retained safe revision to roll back to"),
            )
        })?;
    let mut target = src.previous.remove(pos);
    target.activated_at = ctx.now;
    let displaced = src.active.replace(target).map(|a| a.commit);
    src.held = displaced;
    src.pending = None;
    src.unavailable = false;
    let r = rep(src, "up-to-date");
    st.save(ctx.home)?;
    Ok(Report {
        sources: vec![r],
        notice: st.notice.clone(),
        offline: ctx.offline,
    })
}

/// Record a revocation (commit or digest). Active revoked revisions fall back
/// to the newest safe previous one, or the source becomes unavailable.
pub fn revoke(ctx: &Ctx, id: &str, target: &str) -> HarnessResult<Report> {
    let mut st = load(ctx)?;
    select_ids(&st, &[id.to_string()])?;
    let src = st.sources.get_mut(id).unwrap();
    if !src.revoked.iter().any(|r| r == target) {
        src.revoked.push(target.to_string());
    }
    apply_revocations(src, ctx.now);
    let r = rep(
        src,
        if src.unavailable {
            "unavailable"
        } else {
            "up-to-date"
        },
    );
    st.save(ctx.home)?;
    Ok(Report {
        sources: vec![r],
        notice: st.notice.clone(),
        offline: ctx.offline,
    })
}

/// Cached state only; never touches the network.
pub fn status(ctx: &Ctx, ids: &[String]) -> HarnessResult<Report> {
    let st = load(ctx)?;
    select_ids(&st, ids)?;
    Ok(Report {
        sources: cached_report(&st, ids, "up-to-date"),
        notice: st.notice.clone(),
        offline: true,
    })
}

pub fn approve_policy(
    home: &Path,
    auto_content: bool,
    ttl_secs: Option<u64>,
    timeout_ms: Option<u64>,
    gh: Option<String>,
) -> HarnessResult<()> {
    let mut st = State::load(home)?;
    st.policy.approved = true;
    st.policy.auto_content = auto_content;
    if let Some(t) = ttl_secs {
        st.policy.ttl_secs = t;
    }
    if let Some(t) = timeout_ms {
        st.policy.timeout_ms = t.clamp(100, 120_000);
    }
    if gh.is_some() {
        st.policy.gh = gh;
    }
    st.save(home)
}

/// Register an additional source (a skill outside the catalog or an adapter
/// package). The same resolver and pipeline handle every kind.
pub fn add_source(home: &Path, src: Source) -> HarnessResult<()> {
    super::fetch::parse_repo(&src.repo)?;
    Channel::parse(&src.channel)?;
    let mut st = State::load(home)?;
    if st.sources.contains_key(&src.id) {
        return Err(d(
            "SPX-HPU001",
            format!("update source `{}` already exists", src.id),
        ));
    }
    st.sources.insert(src.id.clone(), src);
    st.save(home)
}

/// Revision a session started now should lock for source `id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPin {
    pub version: String,
    pub commit: String,
    pub digest: String,
    pub unavailable: bool,
}

pub fn session_pin(ctx: &Ctx, id: &str) -> HarnessResult<Option<SessionPin>> {
    let st = load(ctx)?;
    Ok(st.sources.get(id).and_then(|s| {
        s.active.as_ref().map(|a| SessionPin {
            version: a.version.clone(),
            commit: a.commit.clone(),
            digest: a.digest.clone(),
            unavailable: s.unavailable,
        })
    }))
}

/// The curated set a session started now should use: the embedded set with
/// every activated skill update overlaid (an unavailable skill is withdrawn).
pub fn effective_set(home: &Path) -> HarnessResult<OfficialSet> {
    effective_set_from(home, OfficialSet::embedded())
}

/// [`effective_set`] over an explicit base catalog.
pub fn effective_set_from(home: &Path, mut set: OfficialSet) -> HarnessResult<OfficialSet> {
    let st = State::load(home)?;
    let store = store_dir(home);
    for src in st.sources.values().filter(|s| s.kind == Kind::Skill) {
        let Some(a) = &src.active else { continue };
        if src.unavailable {
            if let Some(k) = set.skills.iter_mut().find(|k| k.id == src.id) {
                k.embedded = false;
            }
            continue;
        }
        let Some(k) = set.find(&src.id) else { continue };
        if k.commit.as_deref() == Some(a.commit.as_str()) {
            continue;
        }
        let snap = snapshot::open(&store, &a.digest)?;
        let mut assets = BTreeMap::new();
        for f in &a.files {
            assets.insert(
                f.path.clone(),
                std::fs::read(snap.files_dir.join(&f.path)).map_err(|e| {
                    d(
                        "SPX-HPU009",
                        format!("cannot read snapshot file {}: {e}", f.path),
                    )
                })?,
            );
        }
        let files = a.files.iter().map(|f: &FileRec| OfficialFile {
            path: f.path.clone(),
            upstream_path: f.upstream_path.clone(),
            git_blob_sha: f.git_blob_sha.clone(),
            sha256: f.sha256.clone(),
            bytes: f.bytes,
        });
        set.with_revision(
            &src.id,
            official::Revision {
                version: a.version.clone(),
                tag: a.tag.clone(),
                commit: a.commit.clone(),
                bundle_digest: a.digest.clone(),
                files: files.collect(),
            },
            assets,
        )?;
    }
    Ok(set)
}
