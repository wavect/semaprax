//! Closed repair evidence, with constructor grammar supplied by its owner.
use super::payload_schemas::{digest, document, object, text};
use super::{invalid, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) const ID: &str = "urn:semaprax.project-candidate-repair-catalog.v1";

pub(super) fn schema(documents: &BTreeMap<String, Value>) -> Result<Value> {
    let intent = documents
        .get("urn:semaprax.semantic-change-intent.v1")
        .ok_or_else(|| invalid("repair schema constructor owner is absent"))?;
    let forms = intent["$defs"]["intent"]["oneOf"]
        .as_array()
        .ok_or_else(|| invalid("repair schema intention alternatives are absent"))?;
    let selected = |kind: &str| -> Result<Value> {
        let mut matching = forms
            .iter()
            .filter(|form| form["properties"]["kind"]["const"] == kind);
        let form = matching
            .next()
            .ok_or_else(|| invalid("repair schema intention is absent"))?;
        if matching.next().is_some() {
            return Err(invalid("repair schema intention is ambiguous"));
        }
        Ok(form.clone())
    };
    let body = selected("replace_function_body")?;
    let repair = selected("repair_diagnostic")?;
    let mut change = documents
        .get("urn:semaprax.semantic-change.v1")
        .cloned()
        .ok_or_else(|| invalid("repair schema change owner is absent"))?;
    for key in ["$id", "$schema", "$defs"] {
        change
            .as_object_mut()
            .ok_or_else(|| invalid("repair change schema is malformed"))?
            .remove(key);
    }
    change["properties"]["intent"] = json!({"$ref":"#/$defs/body_intent"});
    let common = |suffix: &str| {
        vec![
            ("repair_id", digest()),
            ("target", body["properties"]["target"].clone()),
            ("change", json!({"$ref":format!("#/$defs/change{suffix}")})),
            (
                "semantic_change_intent",
                json!({"$ref":format!("#/$defs/repair_intent{suffix}")}),
            ),
            ("validated_candidate_revision", digest()),
            ("validation", json!({"const":"normal_full_candidate_apply"})),
            ("tests", json!({"const":"not_run"})),
            ("source_authority", json!({"const":false})),
        ]
    };
    let mut literal = common("_literal");
    literal.extend([
        (
            "class",
            json!({"const":"retag_integer_literal_to_retained_return_type"}),
        ),
        ("from_type", json!({"enum":["i64","i32","u8","usize"]})),
        ("expected_type", json!({"enum":["i64","i32","u8","usize"]})),
        (
            "preserved_integer_value",
            json!({"type":"integer","minimum":i64::MIN,"maximum":u64::MAX}),
        ),
        (
            "evidence_owner",
            json!({"const":"retained_target_return_type_and_full_candidate_admission"}),
        ),
    ]);
    // The field and lexical selectors reuse the same constructor constraints.
    let expressions = intent["$defs"]["expression"]["oneOf"]
        .as_array()
        .ok_or_else(|| invalid("repair expression schema is absent"))?;
    let mut literals = Vec::new();
    for kind in ["i64", "i32", "u8", "usize"] {
        let matches = expressions
            .iter()
            .filter(|form| form["properties"]["kind"]["const"] == kind)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(invalid(
                "repair literal constructor is missing or ambiguous",
            ));
        }
        literals.push(matches[0].clone());
    }
    let mut literal_body = body.clone();
    literal_body["properties"]["body"] = json!({"oneOf":literals});
    let mut literal_repair = repair.clone();
    literal_repair["properties"]["rejected_intent"] = literal_body.clone();
    let mut literal_change = change.clone();
    literal_change["properties"]["intent"] = json!({"$ref":"#/$defs/body_intent_literal"});
    let place = expressions
        .iter()
        .find(|form| form["properties"]["kind"]["const"] == "field_place")
        .ok_or_else(|| invalid("repair field-place schema is absent"))?;
    let mut borrowed = common("");
    borrowed.extend([
        (
            "class",
            json!({"const":"borrow_owned_byte_field_without_staging"}),
        ),
        ("diagnostic_code", json!({"const":"SPX-T266"})),
        (
            "replacement_count",
            json!({"type":"integer","minimum":1,"maximum":4096}),
        ),
        (
            "replacements",
            json!({"type":"array","minItems":1,"maxItems":4096,"items":object(vec![
                ("field",place["properties"]["target"].clone()),
                ("root",place["properties"]["root"].clone()),
            ])}),
        ),
        (
            "evidence_owner",
            json!({"const":"closed_builtin_projection_pattern_and_full_candidate_admission"}),
        ),
    ]);
    let mut schema = document(
        "semaprax.project-candidate-repair-catalog.v1",
        vec![
            ("attempt_revision", digest()),
            ("base_candidate_revision", digest()),
            ("base_project_revision", digest()),
            (
                "repairs",
                json!({"type":"array","maxItems":1,"items":{"oneOf":[object(literal),object(borrowed)]}}),
            ),
            ("availability_reason", text()),
            (
                "legacy_identity_repair",
                json!({"const":"assign_function_id_is_a_breaking_identity_rebase_and_not_a_stable_identity_preserving_candidate_change"}),
            ),
            ("tests", json!({"const":"not_run"})),
            ("source_authority", json!({"const":false})),
            (
                "nonclaims",
                json!({"const":["not_general_diagnostic_repair","no_invalid_source_or_hir_admission","no_automatic_repair_selection"]}),
            ),
        ],
    );
    schema["$defs"] = json!({
        "expression":{"$ref":"urn:semaprax.typed-expression.v1"},
        "body_intent":body,"repair_intent":repair,"change":change,
        "body_intent_literal":literal_body,"repair_intent_literal":literal_repair,"change_literal":literal_change,
    });
    Ok(schema)
}
