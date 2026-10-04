//! Transformation lineage: stage-local reductions are shown, but the headline
//! is the end-to-end pair (first raw size vs final model-visible size).

use super::event::{Availability, Observation, Outcome, Role, Stage, TokenCount};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub stage: Stage,
    pub before: Option<TokenCount>,
    pub after: Option<TokenCount>,
}

impl Step {
    /// Stage-local signed reduction; `None` unless both sides are comparable.
    pub fn local_reduction(&self) -> Option<i64> {
        self.before.as_ref()?.checked_sub(self.after.as_ref()?).ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pairing {
    /// Both ends measured with one tokenizer; `reduction` = baseline - final.
    Paired {
        baseline: TokenCount,
        final_: TokenCount,
        reduction: i64,
    },
    Unpaired(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lineage {
    pub payload_id: String,
    pub steps: Vec<Step>,
    pub pairing: Pairing,
    pub final_digest: Option<String>,
}

/// One lineage per payload id, steps ordered by observer sequence. Failed or
/// unavailable stages pass the payload through unchanged and are excluded.
pub fn build(events: &[Observation]) -> Vec<Lineage> {
    let mut by: BTreeMap<&str, Vec<&Observation>> = BTreeMap::new();
    for e in events.iter().filter(|e| e.role == Role::Transform) {
        if e.outcome == Outcome::Ok && e.availability != Availability::Unavailable {
            if let Some(id) = e.payload_id.as_deref() {
                by.entry(id).or_default().push(e);
            }
        }
    }
    by.into_iter()
        .map(|(id, mut evs)| {
            evs.sort_by_key(|e| e.seq);
            let steps: Vec<Step> = evs
                .iter()
                .map(|e| Step {
                    stage: e.stage,
                    before: e.before.clone(),
                    after: e.after.clone(),
                })
                .collect();
            let (first, last) = (evs[0], evs[evs.len() - 1]);
            let pairing = match (&first.before, &last.after) {
                (None, _) => Pairing::Unpaired("missing_baseline"),
                (_, None) => Pairing::Unpaired("missing_final"),
                _ if !last.model_visible => Pairing::Unpaired("no_final_model_visible_envelope"),
                (Some(b), Some(f)) => match b.checked_sub(f) {
                    Ok(reduction) => Pairing::Paired {
                        baseline: b.clone(),
                        final_: f.clone(),
                        reduction,
                    },
                    Err(_) => Pairing::Unpaired("mixed_tokenizer_kinds"),
                },
            };
            Lineage {
                payload_id: id.to_string(),
                steps,
                pairing,
                final_digest: last.after_digest.clone(),
            }
        })
        .collect()
}
