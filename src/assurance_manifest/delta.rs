//! Delta between two already-generated Assurance Manifest v1 envelopes.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../../docs/ASSURANCE-MANIFEST-V1.md)
//! "Delta" for the exact bucket definitions this module implements.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::diagnostic::Diagnostic;

use super::lattice::{dominates, AssuranceClass};
use super::verify::verify_envelope;

pub(super) const DELTA_SCHEMA: &str = "semaprax.assurance-manifest-delta.v1";

fn payload_of(envelope: &str) -> Value {
    let value: Value =
        serde_json::from_str(envelope).expect("verify_envelope already accepted this JSON");
    value["payload"].clone()
}

struct Entry {
    classification: AssuranceClass,
    assumption_ids: Vec<String>,
}

fn entries(payload: &Value) -> BTreeMap<String, Entry> {
    let mut map = BTreeMap::new();
    for obligation in payload["obligations"].as_array().into_iter().flatten() {
        let id = obligation["id"].as_str().unwrap_or_default().to_owned();
        let classification =
            AssuranceClass::from_token(obligation["classification"].as_str().unwrap_or_default())
                .unwrap_or(AssuranceClass::Open);
        let assumption_ids = obligation["assumption_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect();
        map.insert(
            id,
            Entry {
                classification,
                assumption_ids,
            },
        );
    }
    map
}

fn review_by_map(payload: &Value) -> BTreeMap<String, Option<String>> {
    let mut map = BTreeMap::new();
    for assumption in payload["assumptions"].as_array().into_iter().flatten() {
        let id = assumption["id"].as_str().unwrap_or_default().to_owned();
        let review_by = assumption["review_by"].as_str().map(str::to_owned);
        map.insert(id, review_by);
    }
    map
}

fn quote(value: &str) -> String {
    crate::diagnostic::quote_json(value)
}

/// Independently verify both envelopes, then classify every obligation `id`
/// that appears in either payload into exactly one bucket: `added`,
/// `removed`, `strengthened`, `weakened`, `reclassified`,
/// `assumption_changed`, or `stale`. An `id` unchanged in every respect is
/// not reported. `as_of`, when `Some`, is an ISO-8601 `YYYY-MM-DD` date
/// compared against candidate assumption `review_by` dates in ASCII byte
/// order (both share the fixed-width form, so byte order is date order);
/// when `None`, `stale` is always empty rather than making this function
/// impure by default.
pub fn delta(
    base_envelope: &str,
    candidate_envelope: &str,
    as_of: Option<&str>,
) -> Result<String, Diagnostic> {
    verify_envelope(base_envelope)?;
    verify_envelope(candidate_envelope)?;
    let base_payload = payload_of(base_envelope);
    let candidate_payload = payload_of(candidate_envelope);
    let base = entries(&base_payload);
    let candidate = entries(&candidate_payload);
    let candidate_review_by = review_by_map(&candidate_payload);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut strengthened = Vec::new();
    let mut weakened = Vec::new();
    let mut reclassified = Vec::new();
    let mut assumption_changed = Vec::new();
    let mut stale = Vec::new();

    let mut ids: Vec<&String> = base.keys().chain(candidate.keys()).collect();
    ids.sort();
    ids.dedup();

    for id in ids {
        match (base.get(id), candidate.get(id)) {
            (None, Some(_)) => added.push(id.clone()),
            (Some(_), None) => removed.push(id.clone()),
            (Some(before), Some(after)) => {
                if before.classification != after.classification {
                    if dominates(after.classification, before.classification) {
                        strengthened.push((
                            id.clone(),
                            before.classification,
                            after.classification,
                        ));
                    } else if dominates(before.classification, after.classification) {
                        weakened.push((id.clone(), before.classification, after.classification));
                    } else {
                        reclassified.push((
                            id.clone(),
                            before.classification,
                            after.classification,
                        ));
                    }
                } else if before.assumption_ids != after.assumption_ids {
                    assumption_changed.push(id.clone());
                }
                if let Some(as_of) = as_of {
                    let is_stale = after.assumption_ids.iter().any(|assumption_id| {
                        candidate_review_by
                            .get(assumption_id)
                            .and_then(Option::as_ref)
                            .is_some_and(|review_by| review_by.as_str() <= as_of)
                    });
                    if is_stale {
                        stale.push(id.clone());
                    }
                }
            }
            (None, None) => unreachable!("id came from base or candidate keys"),
        }
    }

    let render_ids =
        |ids: &[String]| -> String { ids.iter().map(|id| quote(id)).collect::<Vec<_>>().join(",") };
    let render_transitions = |transitions: &[(String, AssuranceClass, AssuranceClass)]| -> String {
        transitions
            .iter()
            .map(|(id, from, to)| {
                format!(
                    "{{\"id\":{},\"from\":{},\"to\":{}}}",
                    quote(id),
                    quote(from.token()),
                    quote(to.token())
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    };

    Ok(format!(
        "{{\"schema\":\"{}\",\"added\":[{}],\"removed\":[{}],\"strengthened\":[{}],\"weakened\":[{}],\
\"reclassified\":[{}],\"assumption_changed\":[{}],\"stale\":[{}]}}",
        DELTA_SCHEMA,
        render_ids(&added),
        render_ids(&removed),
        render_transitions(&strengthened),
        render_transitions(&weakened),
        render_transitions(&reclassified),
        render_ids(&assumption_changed),
        render_ids(&stale),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance_manifest::obligation::{MethodRecord, Obligation, ObligationKind};
    use crate::assurance_manifest::render::{render, RenderInput};

    fn fixture_sha256() -> String {
        format!("sha256:{}", "0".repeat(64))
    }

    fn envelope(obligations: &[Obligation]) -> String {
        render(&RenderInput {
            source_path_text: "examples/meaning.spx",
            revision: "r",
            source_sha256: &fixture_sha256(),
            obligations,
            assumptions: &[],
            max_bytes: 65_536,
            max_obligations: 1024,
        })
    }

    #[test]
    fn identical_manifests_have_an_empty_delta() {
        let obligation = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let base = envelope(std::slice::from_ref(&obligation));
        let candidate = envelope(&[obligation]);
        let result = delta(&base, &candidate, None).unwrap();
        assert!(result.contains("\"added\":[]"));
        assert!(result.contains("\"removed\":[]"));
        assert!(result.contains("\"strengthened\":[]"));
        assert!(result.contains("\"weakened\":[]"));
        assert!(result.contains("\"reclassified\":[]"));
    }

    #[test]
    fn added_and_removed_are_detected() {
        let kept = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let removed_one = Obligation::new(ObligationKind::Precondition, "app.g", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let added_one = Obligation::new(ObligationKind::Precondition, "app.h", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let base = envelope(&[kept.clone(), removed_one]);
        let candidate = envelope(&[kept, added_one]);
        let result = delta(&base, &candidate, None).unwrap();
        assert!(result.contains("app.h"));
        assert!(result.contains("app.g"));
    }

    #[test]
    fn a_dominance_increase_is_strengthened_not_reclassified() {
        let before = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::TestEvidenced, "t", "1"));
        let after = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::RuntimeGuarded, "t", "1"));
        let base = envelope(&[before]);
        let candidate = envelope(&[after]);
        let result = delta(&base, &candidate, None).unwrap();
        assert!(result.contains("\"strengthened\":[{"));
        assert!(result.contains("\"reclassified\":[]"));
    }

    #[test]
    fn an_incomparable_change_is_reclassified_not_forced_either_way() {
        let before = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::CompilerProved, "t", "1"));
        let after = Obligation::new(ObligationKind::Precondition, "app.f", "require:0")
            .with_method(MethodRecord::new(AssuranceClass::SmtProved, "t", "1"));
        let base = envelope(&[before]);
        let candidate = envelope(&[after]);
        let result = delta(&base, &candidate, None).unwrap();
        assert!(result.contains("\"strengthened\":[]"));
        assert!(result.contains("\"weakened\":[]"));
        assert!(result.contains("\"reclassified\":[{"));
    }
}
