//! Provider prompt-cache state of a dispatched attempt, classified from the
//! receipt's own cache categories and kept apart from the warm repository or
//! index cache (`RepoCache`). A state the receipt cannot establish is
//! `Unknown`, never guessed from timing or from the arm's configuration.

use crate::receipt::Usage;
use std::collections::BTreeSet;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheState {
    /// The request created cache entries (or used none) with no earlier cache for this model.
    Cold,
    /// Cached input was read.
    Warm,
    /// A cache was written earlier for this model, yet this request read nothing
    /// and wrote nothing: it lapsed or was invalidated.
    Expired,
    /// The receipt does not report the cache categories needed to decide.
    Unknown,
}

impl CacheState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
            Self::Expired => "expired",
            Self::Unknown => "unknown",
        }
    }
}

/// Repository/index cache of the trial (index warm-up), independent of the provider cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoCache {
    Cold,
    Warm,
}

impl RepoCache {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
        }
    }
}

/// Remembers which models have written a cache entry in this campaign.
#[derive(Default)]
pub struct CacheTracker {
    written: Mutex<BTreeSet<String>>,
}

impl CacheTracker {
    pub fn classify(&self, model: &str, u: &Usage) -> CacheState {
        let mut seen = self.written.lock().expect("cache tracker");
        let wrote = u.cache_write.is_some_and(|w| w > 0);
        let state = match (u.cache_read, u.cache_write) {
            (Some(r), _) if r > 0 => CacheState::Warm,
            (Some(0), Some(0)) if seen.contains(model) => CacheState::Expired,
            (Some(0), Some(_)) => CacheState::Cold,
            _ => CacheState::Unknown,
        };
        if wrote || state == CacheState::Warm {
            seen.insert(model.to_string());
        }
        state
    }
}

/// Aggregate label of a trial's attempts: the common state, `mixed`, or `none`.
pub fn trial_label(states: &[CacheState]) -> &'static str {
    match states.split_first() {
        None => "none",
        Some((f, rest)) if rest.iter().all(|s| s == f) => f.as_str(),
        Some(_) => "mixed",
    }
}

/// The shareable prefix of a prompt: everything before the task text. A warm-up
/// request may carry only this, so no task answer or request text is leaked.
pub fn warm_prefix(prompt: &str) -> &str {
    prompt.split("\n\n## Task\n").next().unwrap_or(prompt)
}

/// True when `prefix` carries reference-solution content that the pristine
/// project does not already contain (a warm-up that leaked an answer).
pub fn leaks_reference(
    prefix: &str,
    reference: &[(String, String)],
    pristine: &std::collections::BTreeMap<String, String>,
) -> bool {
    reference.iter().any(|(path, text)| {
        text.lines()
            .map(str::trim)
            .filter(|l| l.len() >= 24)
            .any(|l| prefix.contains(l) && !pristine.values().any(|p| p.contains(l)))
            || (pristine.get(path).map(String::as_str) != Some(text.as_str())
                && text.len() >= 24
                && prefix.contains(text.as_str()))
    })
}
