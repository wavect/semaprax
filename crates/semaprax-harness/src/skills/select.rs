//! Deterministic skill selection.

use super::catalog::Catalog;
use super::MAX_TAG_SELECTED;

/// Deterministic task tags for a task kind (e.g. `api-reuse`).
pub fn task_tags(kind: &str) -> Vec<String> {
    let mut t = vec![kind.to_string()];
    match kind {
        "api-reuse" => t.push("reuse".into()),
        "docs" => t.push("documentation".into()),
        "mechanical-edit" => t.push("edit".into()),
        _ => {}
    }
    t
}

/// Hook for future decision-task recommendation among the approved set. The
/// default recommends nothing; recommendations can only name approved skills.
pub trait Recommender {
    fn recommend(&self, approved: &[String], task_kind: &str) -> Vec<String>;
}

pub struct NoRecommender;

impl Recommender for NoRecommender {
    fn recommend(&self, _approved: &[String], _task_kind: &str) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// Indexes into `Catalog::entries`.
    pub selected: Vec<usize>,
    /// Explicit selections that matched nothing (or only conflicting bundles).
    pub unresolved: Vec<String>,
}

/// Explicit user selections first (name or exact digest), then tag matches
/// ranked by overlap and name. Conflicting bundles are never chosen by name.
pub fn select(catalog: &Catalog, tags: &[String], explicit: &[String]) -> Selection {
    let mut sel = Selection::default();
    for want in explicit {
        let idx = catalog
            .entries
            .iter()
            .position(|e| e.digest == *want || (e.name == *want && !e.conflict));
        match idx {
            Some(i) if !sel.selected.contains(&i) => sel.selected.push(i),
            Some(_) => {}
            None => sel.unresolved.push(want.clone()),
        }
    }
    let mut scored: Vec<(usize, usize)> = catalog
        .entries
        .iter()
        .enumerate()
        .filter(|(i, e)| !e.conflict && !sel.selected.contains(i))
        .map(|(i, e)| (i, e.tags.iter().filter(|t| tags.contains(t)).count()))
        .filter(|(_, s)| *s > 0)
        .collect();
    scored.sort_by(|(ia, sa), (ib, sb)| {
        sb.cmp(sa)
            .then_with(|| catalog.entries[*ia].name.cmp(&catalog.entries[*ib].name))
    });
    sel.selected
        .extend(scored.into_iter().take(MAX_TAG_SELECTED).map(|(i, _)| i));
    sel
}
