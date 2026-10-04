//! Session-start update hook of `harness run` (HN-05). A run is a new session:
//! it performs the bounded, policy-gated maintenance check (never failing the
//! run), loads the effective curated set (embedded + activated revisions) and
//! releases the previous run's revision locks so the next run uses a newly
//! activated revision. Other sessions keep their locks and keep loading the
//! locked revision from the immutable store. `--frozen`/`--offline` perform no
//! update request at all.

use super::cli::RunOptions;
use crate::cli::Environment;
use crate::skills::modes::StateStore;
use crate::skills::official::{embedded_cached, OfficialSet};
use crate::updates::ops::{self, Ctx};
use crate::updates::state::State;
use crate::updates::{DirectoryFetcher, Fetcher, GitHubCliFetcher};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The session id `harness run` shares with the `skills` verb.
pub const RUN_SESSION: &str = "default";

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn fetcher(o: &RunOptions, home: &Path, env: &Environment) -> Option<Box<dyn Fetcher>> {
    if o.frozen || o.offline {
        return None;
    }
    if let Some(dir) = &o.updates_fixture {
        return DirectoryFetcher::load(dir)
            .ok()
            .map(|f| Box::new(f) as Box<dyn Fetcher>);
    }
    let policy = State::load(home).ok()?.policy;
    let gh = o.updates_gh.clone().or(policy.gh.map(PathBuf::from))?;
    GitHubCliFetcher::new(gh, env, Duration::from_millis(policy.timeout_ms))
        .ok()
        .map(|f| Box::new(f) as Box<dyn Fetcher>)
}

/// Maintenance (policy-gated, TTL, bounded) then the effective set. Notes are
/// bounded status lines; nothing here can fail the run.
pub fn prepare(o: &RunOptions, env: &Environment, project_id: &str) -> (OfficialSet, Vec<String>) {
    let Some(home) = env.harness_home.clone() else {
        return (OfficialSet::embedded(), vec![]);
    };
    let mut notes = Vec::new();
    let f = fetcher(o, &home, env);
    let ctx = Ctx {
        home: &home,
        fetcher: f.as_deref(),
        now: o.updates_now.unwrap_or_else(now),
        offline: o.offline || o.frozen || f.is_none(),
        frozen: o.frozen,
        gate: None,
        catalog: embedded_cached(),
    };
    if !o.frozen && !o.offline {
        let rep = ops::maintenance(&ctx);
        if let Some(n) = &rep.notice {
            notes.push(format!("update check: {n}"));
        }
        for s in rep.sources.iter().filter(|s| s.state == "activated") {
            notes.push(format!(
                "curated source `{}` updated to {} for new sessions",
                s.id,
                s.active.as_deref().unwrap_or("?")
            ));
        }
    }
    let set = match ops::effective_set(&home) {
        Ok(s) => s,
        Err(e) => {
            notes.push(format!(
                "update state unusable, embedded skills used: {} {}",
                e.code, e.message
            ));
            OfficialSet::embedded()
        }
    };
    // A run is a new session: forget the previous run's revision locks that no
    // longer match the effective set (explicit other sessions keep theirs).
    let store = StateStore::new(home);
    if let Ok(mut st) = store.load_session(project_id, RUN_SESSION) {
        let stale: Vec<String> = st
            .locks
            .iter()
            .filter(|(id, (dg, _))| {
                set.find(id)
                    .is_some_and(|k| k.bundle_digest.as_deref() != Some(dg.as_str()))
            })
            .map(|(id, _)| id.clone())
            .collect();
        if !stale.is_empty() {
            for id in &stale {
                st.locks.remove(id);
            }
            let _ = store.save_session(project_id, RUN_SESSION, &mut st);
        }
    }
    (set, notes)
}
