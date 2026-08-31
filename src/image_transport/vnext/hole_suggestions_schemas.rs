//! A finite suggestion grammar copied from the owning expression constructors.
use super::payload_schemas::{digest, document, object, text};
use super::{invalid, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) const ID: &str = "urn:semaprax.project-hole-fill-suggestions.v1";

pub(super) fn schema(documents: &BTreeMap<String, Value>) -> Result<Value> {
    let owner = documents
        .get("urn:semaprax.semantic-change-intent.v1")
        .ok_or_else(|| invalid("hole suggestion constructor owner is absent"))?;
    let forms = owner["$defs"]["expression"]["oneOf"]
        .as_array()
        .ok_or_else(|| invalid("hole suggestion expression alternatives are absent"))?;
    let select = |kind: &str| -> Result<Value> {
        let mut matches = forms
            .iter()
            .filter(|form| form["properties"]["kind"]["const"] == kind);
        let form = matches
            .next()
            .ok_or_else(|| invalid("hole suggestion constructor is absent"))?;
        if matches.next().is_some() {
            return Err(invalid("hole suggestion constructor is ambiguous"));
        }
        Ok(form.clone())
    };
    let place = select("place")?;
    let mut call = select("call")?;
    // Suggestions never contain nested calls, invented defaults or unresolved
    // placeholders. Full fill replay still owns every semantic assertion.
    call["properties"]["arguments"]["items"] = place.clone();
    call["properties"]["arguments"]["maxItems"] = json!(64);
    let count = json!({"type":"integer","minimum":0,"maximum":32});
    Ok(document(
        "semaprax.project-hole-fill-suggestions.v1",
        vec![
            ("draft_revision", digest()),
            (
                "hole_id",
                json!({"type":"string","minLength":1,"maxLength":128,"x-max-utf8-bytes":128}),
            ),
            ("context_revision", digest()),
            ("last_valid_revision", digest()),
            ("expected_type_id", text()),
            ("considered", count.clone()),
            ("rejected", count),
            ("search_exhausted", json!({"type":"boolean"})),
            (
                "suggestions",
                json!({"type":"array","maxItems":32,"items":object(vec![
                    ("expression",json!({"oneOf":[place,call]})),
                    ("preview_draft_revision",digest()),
                ])}),
            ),
            ("validation", json!({"const":"ordinary_fill_source_replay"})),
            ("tests", json!({"const":"not_run"})),
            ("source_authority", json!({"const":false})),
            ("draft_retained", json!({"const":false})),
            (
                "nonclaims",
                json!({"const":["not_intent_correctness","not_runtime_contract_proof","not_complete_expression_search","not_liveness_inference"]}),
            ),
        ],
    ))
}
